//! 「词库 → 我的词」的数据：读 `user-words.tsv`，搜索、筛选、排序、分页，保存前比对文件有没有被输入法改过。
//! 拼音归一化在 `pinyin`，基础词库读音核对在 `reading_check`，导入合并在 `import`，编辑草稿在 `editor`。

mod editor;
mod filter;
mod import;
mod pinyin;
mod reading_check;
mod sort;
mod word;

pub use editor::WordEditor;
pub use filter::Filter;
pub use import::ImportReport;
pub use pinyin::{normalize, validate_word};
pub use reading_check::ReadingCheck;
pub use sort::Sort;
pub use word::Word;

pub use qingjian_learning::user_words::{FILE, Stamp};

use std::cell::Cell;
use std::collections::BTreeMap;
use std::path::PathBuf;

use crate::system::paths;

/// 一页列几个词。
pub const PAGE_SIZE: usize = 50;

/// 「我的词」的视图模型：整张用户词表、打开时的快照、搜索筛选排序状态与读音索引。
#[derive(Default)]
pub struct PersonalWords {
    /// 打开时文件里的样子，保存前比对；文件内容没变就不算冲突（输入法原子写会换 mtime）。
    original: BTreeMap<String, String>,

    /// 正在编辑的表。
    entries: BTreeMap<String, String>,

    /// 读文件时跳过的坏行。
    pub skipped: usize,

    /// 上次读文件时的戳：变了才重读，判断别处有没有改过。视图是 `&self`，用 `Cell` 记。
    stamp: Cell<Option<Stamp>>,

    /// 打开之后别处改过文件（内容与 `original` 不同）。设上就一直留着，直到重新载入或保存成功：
    /// 只靠戳比会漏——戳更新一次之后下一帧就判不出冲突，保存时会把输入法的改动覆盖掉。
    external: Cell<bool>,

    /// 基础词库读音索引；没有随包词库时为 `None`，读音核对整项跳过。
    reading: Option<ReadingCheck>,

    pub filter: Filter,

    pub sort: Sort,

    /// 第几页，从 0 起。
    pub page: usize,

    /// 改过还没保存：页面上提示「保存后生效」。
    pub dirty: bool,
}

impl PersonalWords {
    /// 读文件；读到坏行也照开，条数报在 `skipped`。基础词库读音索引由 [`Self::load`] 懒加载。
    fn read() -> Self {
        let path = path();
        // 先取戳再读：读的过程中文件又被写了的话，戳停在旧值，下一次检查还能发现
        let stamp = qingjian_learning::user_words::stamp(&path);
        let (entries, skipped) = match qingjian_learning::user_words::read(&path) {
            Ok((entries, skipped)) => (entries, skipped),
            Err(error) => {
                tracing::warn!(path = %path.display(), %error, "读用户词失败，按空表打开");
                (BTreeMap::new(), 0)
            }
        };
        Self {
            original: entries.clone(),
            entries,
            skipped,
            stamp: Cell::new(stamp),
            external: Cell::new(false),
            reading: None,
            filter: Filter::default(),
            sort: Sort::default(),
            page: 0,
            dirty: false,
        }
    }

    /// 进「我的词」时读文件，并加载基础词库读音索引。
    pub fn load() -> Self {
        let mut book = Self::read();
        book.reading = ReadingCheck::load();
        book
    }

    /// 不读盘的空白表（离屏建页兜底），也不加载基础词库。
    pub fn empty() -> Self {
        Self::default()
    }

    pub fn entries(&self) -> &BTreeMap<String, String> {
        &self.entries
    }

    /// 输入法在页面打开期间改过文件：文件戳变了才重读一遍比对内容（内容相同不算冲突：
    /// 输入法原子写会换 mtime）。每帧建视图时调，不重复读盘；判出冲突后一直算冲突，直到重新载入。
    pub fn changed_on_disk(&self) -> bool {
        let file = path();
        let stamp = qingjian_learning::user_words::stamp(&file);
        if stamp != self.stamp.get() {
            self.stamp.set(stamp);
            if let Ok((entries, _)) = qingjian_learning::user_words::read(&file) {
                self.external.set(entries != self.original);
            }
        }
        self.external.get()
    }

    /// 词数、同音组数、读音不同的条数。
    pub fn summary(&self) -> (usize, usize, usize) {
        let groups = self.homophone_groups();
        let readings = self
            .entries
            .iter()
            .filter(|(text, pinyin)| self.reading_differs(text, pinyin))
            .count();
        (self.entries.len(), groups, readings)
    }

    fn homophone_groups(&self) -> usize {
        let mut counts: BTreeMap<&str, usize> = BTreeMap::new();
        for pinyin in self.entries.values() {
            *counts.entry(pinyin.as_str()).or_default() += 1;
        }
        counts.values().filter(|count| **count > 1).count()
    }

    /// 同一读音下的其他词（最多 `limit` 个），编辑卡片里显示。
    pub fn homophones(&self, text: &str, pinyin: &str, limit: usize) -> Vec<&str> {
        self.entries
            .iter()
            .filter(|(other, reading)| other.as_str() != text && reading.as_str() == pinyin)
            .map(|(other, _)| other.as_str())
            .take(limit)
            .collect()
    }

    /// 这个读音下一共有几个词（含自己）。
    pub fn homophone_count(&self, pinyin: &str) -> usize {
        self.entries
            .values()
            .filter(|reading| reading.as_str() == pinyin)
            .count()
    }

    pub fn reading_differs(&self, text: &str, pinyin: &str) -> bool {
        self.reading
            .as_ref()
            .is_some_and(|check| check.differs(text, pinyin))
    }

    /// 基础词库里这个词的读音（没有为 `None`），编辑卡片里提示用。
    pub fn base_reading(&self, text: &str) -> Option<&str> {
        self.reading
            .as_ref()
            .and_then(|check| check.base_reading(text))
    }

    /// 按筛选、搜索、排序排好的词。
    pub fn matching(&self, query: &str) -> Vec<Word<'_>> {
        let query = query.trim().to_lowercase();
        let mut words: Vec<Word<'_>> = self
            .entries
            .iter()
            .filter(|(text, pinyin)| match self.filter {
                Filter::All => true,
                Filter::Reading => self.reading_differs(text, pinyin),
            })
            .filter(|(text, pinyin)| {
                query.is_empty()
                    || text.to_lowercase().contains(&query)
                    || pinyin.to_lowercase().contains(&query)
            })
            .map(|(text, pinyin)| Word { text, pinyin })
            .collect();
        match self.sort {
            Sort::Word => words.sort_by(|a, b| a.text.cmp(b.text)),
            Sort::Reading => {
                words.sort_by(|a, b| a.pinyin.cmp(b.pinyin).then_with(|| a.text.cmp(b.text)))
            }
        }
        words
    }

    pub fn pages(count: usize) -> usize {
        count.div_ceil(PAGE_SIZE).max(1)
    }

    /// 改一条（`None` 的拼音是纯删除）。词没变、拼音也没变时不标脏。
    pub fn set(&mut self, text: &str, pinyin: String) {
        if self.entries.get(text) == Some(&pinyin) {
            return;
        }
        self.entries.insert(text.to_owned(), pinyin);
        self.dirty = true;
    }

    /// 改名：词变了就连旧键一起换。
    pub fn replace(&mut self, old: &str, text: &str, pinyin: String) {
        if old != text {
            self.entries.remove(old);
        }
        self.entries.insert(text.to_owned(), pinyin);
        self.dirty = true;
    }

    pub fn remove(&mut self, text: &str) -> bool {
        let removed = self.entries.remove(text).is_some();
        self.dirty |= removed;
        removed
    }

    /// 导入合并：已有词更新拼音，新词加入；返回新增、更新、跳过的条数。
    pub fn merge(&mut self, source: &str) -> ImportReport {
        let report = import::merge(&mut self.entries, source);
        self.dirty |= report.added + report.updated > 0;
        report
    }

    /// 保存失败或冲突时恢复「打开时」的内容：页面上和 [保存] 再点的都是这份表。
    pub fn discard(&mut self) {
        self.entries = self.original.clone();
        self.dirty = false;
    }

    /// 保存前再读一次文件：内容与打开时不同就不覆盖，返回 `Err`，调用方扔掉本次改动。
    /// 保存是一个绑定在按钮上的消息，可能发生在这一帧的检查之前，所以这里自己再查一次。
    pub fn save(&mut self) -> Result<(), String> {
        let file = path();
        if self.changed_on_disk() {
            return Err("用户词在别处更新了，没有保存；已显示最新的，请再改一次".to_owned());
        }
        qingjian_learning::user_words::write(&file, &self.entries)
            .map_err(|error| format!("没存上：{error}"))?;
        self.original = self.entries.clone();
        self.stamp.set(qingjian_learning::user_words::stamp(&file));
        self.external.set(false);
        self.dirty = false;
        Ok(())
    }

    /// 把当前筛出来的词写成 TSV 文本。
    pub fn export_text(&self, query: &str) -> String {
        let mut out = String::from("# 青简用户词：词\t拼音\t词频，与主词库同格式\n");
        for word in self.matching(query) {
            out.push_str(&format!(
                "{}\t{}\t{}\n",
                word.text,
                word.pinyin,
                qingjian_learning::user_words::DEFAULT_FREQUENCY
            ));
        }
        out
    }

    /// 输入法在别处改过文件后手动重读，丢掉没保存的改动。
    pub fn reload(&mut self) {
        let file = path();
        let stamp = qingjian_learning::user_words::stamp(&file);
        let (entries, skipped) = qingjian_learning::user_words::read(&file).unwrap_or_default();
        self.original = entries.clone();
        self.entries = entries;
        self.skipped = skipped;
        self.stamp.set(stamp);
        self.external.set(false);
        self.page = 0;
        self.dirty = false;
    }
}

pub fn path() -> PathBuf {
    paths::user_dir().unwrap_or_default().join(FILE)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn words(entries: &[(&str, &str)]) -> PersonalWords {
        let entries: BTreeMap<String, String> = entries
            .iter()
            .map(|(text, pinyin)| (text.to_string(), pinyin.to_string()))
            .collect();
        PersonalWords {
            original: entries.clone(),
            entries,
            skipped: 0,
            stamp: Cell::new(None),
            external: Cell::new(false),
            reading: None,
            filter: Filter::default(),
            sort: Sort::default(),
            page: 0,
            dirty: false,
        }
    }

    #[test]
    fn counts_homophone_groups() {
        let book = words(&[
            ("青简", "qing jian"),
            ("轻剑", "qing jian"),
            ("世界", "shi jie"),
            ("青", "qing"),
        ]);
        let (total, groups, _) = book.summary();
        assert_eq!((total, groups), (4, 1));
        assert_eq!(book.homophone_count("qing jian"), 2);
        assert_eq!(book.homophones("青简", "qing jian", 5), ["轻剑"]);
    }

    #[test]
    fn searches_both_word_and_pinyin() {
        let book = words(&[("青简", "qing jian"), ("世界", "shi jie")]);
        let hits: Vec<&str> = book.matching("qing").iter().map(|word| word.text).collect();
        assert_eq!(hits, ["青简"]);
        let hits: Vec<&str> = book.matching("世界").iter().map(|word| word.text).collect();
        assert_eq!(hits, ["世界"]);
    }

    #[test]
    fn sorting_by_reading_keeps_homophones_together() {
        let mut book = words(&[
            ("世界", "shi jie"),
            ("轻剑", "qing jian"),
            ("青简", "qing jian"),
        ]);
        book.sort = Sort::Reading;
        let hits: Vec<&str> = book.matching("").iter().map(|word| word.text).collect();
        assert_eq!(hits, ["轻剑", "青简", "世界"]);
    }

    #[test]
    fn edits_mark_the_book_dirty() {
        let mut book = words(&[("青简", "qing jian")]);
        book.set("青简", "qing jian".to_owned());
        assert!(!book.dirty);
        book.set("青简", "qing jian sheng".to_owned());
        assert!(book.dirty);
        book.remove("青简");
        assert!(book.entries.is_empty());
    }

    #[test]
    fn save_refuses_when_the_file_changed_elsewhere_but_not_on_identical_content() {
        let dir =
            std::env::temp_dir().join(format!("qingjian-personal-save-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        crate::system::paths::set_user_dir(dir.clone());
        let file = dir.join(FILE);
        qingjian_learning::user_words::write(
            &file,
            &BTreeMap::from([
                ("青简".to_owned(), "qing jian".to_owned()),
                ("世界".to_owned(), "shi jie".to_owned()),
            ]),
        )
        .unwrap();

        let mut book = PersonalWords::load();
        // 原子写会换 mtime，内容一样不算冲突
        qingjian_learning::user_words::write(&file, book.entries()).unwrap();
        book.set("青简", "qing jian sheng".to_owned());
        assert!(book.save().is_ok());

        // 别处删了一个词：保存要拒绝，并且不覆盖
        let mut stale = PersonalWords::load();
        qingjian_learning::user_words::write(
            &file,
            &BTreeMap::from([("青简".to_owned(), "qing jian".to_owned())]),
        )
        .unwrap();
        stale.set("青简", "qing jian sheng".to_owned());
        // 先建一帧视图（判出冲突），再保存：冲突标记不能因为戳更新过一次就消失
        assert!(stale.changed_on_disk());
        assert!(stale.changed_on_disk());
        assert!(stale.save().is_err());
        let (on_disk, _) = qingjian_learning::user_words::read(&file).unwrap();
        assert_eq!(on_disk.len(), 1);
        // 重新载入后恢复正常
        stale.reload();
        assert!(!stale.changed_on_disk());
        std::fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn merge_reports_added_and_updated() {
        let mut book = words(&[("青简", "qing jian")]);
        let report = book.merge("青简\tqing jian sheng\n世界\tshi jie\n坏行\n");
        assert_eq!((report.added, report.updated, report.skipped), (1, 1, 1));
        assert_eq!(
            book.entries.get("青简").map(String::as_str),
            Some("qing jian sheng")
        );
    }
}
