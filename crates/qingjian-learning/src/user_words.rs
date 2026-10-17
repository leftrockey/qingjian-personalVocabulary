//! 用户词表 `user-words.tsv` 的共享读写。
//!
//! 格式与主词库 TSV 相同：`词\t拼音\t词频`，只读词与拼音，词频由输入法统一写成
//! [`DEFAULT_FREQUENCY`]；设置程序的「我的词」与 [`crate::FrequencyLearner`] 共用这套解析，
//! 避免两边各写一份、格式逐渐分叉。

use std::collections::BTreeMap;
use std::path::Path;
use std::time::SystemTime;

use qingjian_core::storage::{read_text_lossy, write_atomic};
use std::io::Write;

use crate::error::LearningError;

/// 用户词文件名，与学习数据（`user.tsv`）同目录。
pub const FILE: &str = "user-words.tsv";

/// 用户词在内存小词库里的词频：排序主要靠用户选择次数，这个值只在同 weight 时起作用。
pub const DEFAULT_FREQUENCY: u32 = 100;

/// 文件的修改时间与长度；用来判断别的进程有没有改过它。
pub type Stamp = (SystemTime, u64);

/// 读用户词表；文件不存在返回空表。返回 (词 → 拼音, 跳过的坏行数)。
pub fn read(path: &Path) -> Result<(BTreeMap<String, String>, usize), LearningError> {
    match read_text_lossy(path)? {
        Some(source) => Ok(parse(&source)),
        None => Ok((BTreeMap::new(), 0)),
    }
}

/// 按行解析；注释、空行跳过，缺词或缺拼音的行计数后跳过。
pub fn parse(source: &str) -> (BTreeMap<String, String>, usize) {
    let source = source.strip_prefix('\u{feff}').unwrap_or(source);
    let mut words = BTreeMap::new();
    let mut skipped = 0;
    for line in source.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let mut fields = line.split('\t');
        let (Some(text), Some(pinyin)) = (fields.next(), fields.next()) else {
            skipped += 1;
            continue;
        };
        let text = text.trim();
        let pinyin = pinyin.trim();
        if text.is_empty() || pinyin.is_empty() {
            skipped += 1;
            continue;
        }
        words.insert(text.to_owned(), pinyin.to_owned());
    }
    (words, skipped)
}

/// 按输入法一致的格式写回。
pub fn render(entries: &BTreeMap<String, String>) -> String {
    let mut out = String::from("# 青简用户词：词\t拼音\t词频，与主词库同格式\n");
    for (text, pinyin) in entries {
        out.push_str(&format!("{text}\t{pinyin}\t{DEFAULT_FREQUENCY}\n"));
    }
    out
}

/// 原子写回；父目录不存在就建出来。
pub fn write(path: &Path, entries: &BTreeMap<String, String>) -> Result<(), LearningError> {
    if let Some(parent) = path
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
    {
        std::fs::create_dir_all(parent)?;
    }
    write_atomic(path, |file| write!(file, "{}", render(entries)))?;
    Ok(())
}

/// 文件戳：修改时间 + 长度。文件不存在为 `None`。
pub fn stamp(path: &Path) -> Option<Stamp> {
    let metadata = std::fs::metadata(path).ok()?;
    Some((metadata.modified().ok()?, metadata.len()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_skips_comments_and_bad_lines() {
        let (words, skipped) =
            parse("\u{feff}# 注释\n你好\tni hao\t1\n\n坏行\n缺拼音\t\n世界\tshi jie\t100\n");
        assert_eq!(skipped, 2);
        assert_eq!(words.get("你好").map(String::as_str), Some("ni hao"));
        assert_eq!(words.get("世界").map(String::as_str), Some("shi jie"));
    }

    #[test]
    fn render_round_trips_without_the_weight() {
        let mut words = BTreeMap::new();
        words.insert("青简".to_owned(), "qing jian".to_owned());
        let (parsed, skipped) = parse(&render(&words));
        assert_eq!(skipped, 0);
        assert_eq!(parsed, words);
    }

    #[test]
    fn missing_file_reads_empty() {
        let path = std::env::temp_dir().join(format!(
            "qingjian-user-words-missing-{}.tsv",
            std::process::id()
        ));
        let _ = std::fs::remove_file(&path);
        let (words, skipped) = read(&path).unwrap();
        assert!(words.is_empty());
        assert_eq!(skipped, 0);
    }
}
