//! 导入青简 TSV：逐行校验拼音，合进编辑表，记下失败的行。

use std::collections::BTreeMap;

use super::pinyin::{normalize, validate_word};

/// 一次导入的合并结果。
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct ImportReport {
    pub added: usize,
    pub updated: usize,
    pub skipped: usize,

    /// 前几条失败的原因，带行号。
    pub errors: Vec<String>,
}

impl ImportReport {
    pub fn summary(&self) -> String {
        let mut text = format!(
            "新增 {} · 更新 {} · 跳过 {}",
            self.added, self.updated, self.skipped
        );
        if !self.errors.is_empty() {
            text.push('：');
            text.push_str(&self.errors.join("；"));
        }
        text
    }
}

/// 把 `词<Tab>拼音[<Tab>词频]` 合进 `entries`。注释与空行跳过；拼音不合法、词不合法记进 `errors`。
pub fn merge(entries: &mut BTreeMap<String, String>, source: &str) -> ImportReport {
    let source = source.strip_prefix('\u{feff}').unwrap_or(source);
    let mut report = ImportReport::default();
    for (index, line) in source.lines().enumerate() {
        let line = line.trim_end_matches(['\r', '\n']);
        if line.trim().is_empty() || line.trim_start().starts_with('#') {
            continue;
        }
        let mut fields = line.split('\t');
        let (Some(text), Some(pinyin)) = (fields.next(), fields.next()) else {
            report.skipped += 1;
            report.note(index + 1, "缺词或拼音");
            continue;
        };
        let text = match validate_word(text) {
            Ok(text) => text,
            Err(reason) => {
                report.skipped += 1;
                report.note(index + 1, &reason);
                continue;
            }
        };
        let pinyin = match normalize(pinyin) {
            Ok(pinyin) => pinyin,
            Err(reason) => {
                report.skipped += 1;
                report.note(index + 1, &reason);
                continue;
            }
        };
        if entries.insert(text, pinyin).is_some() {
            report.updated += 1;
        } else {
            report.added += 1;
        }
    }
    report
}

impl ImportReport {
    /// 只留前 5 条错误，其余不再列。
    fn note(&mut self, line: usize, reason: &str) {
        if self.errors.len() < 5 {
            self.errors.push(format!("第 {line} 行：{reason}"));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn merges_and_reports_skips() {
        let mut entries = BTreeMap::new();
        entries.insert("青简".to_owned(), "qing jian".to_owned());
        let report = merge(
            &mut entries,
            "# 注释\n青简\tqing jian sheng\t100\n世界\tshijie\n坏行\n\t空拼音\n",
        );
        assert_eq!((report.added, report.updated, report.skipped), (1, 1, 2));
        assert_eq!(entries.get("世界").map(String::as_str), Some("shi jie"));
        assert!(report.errors[0].starts_with("第 4 行："));
    }
}
