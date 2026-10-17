//! 基础词库的「词 → 读音」索引：用户词的读音不在基础词库同词条的读音集合里时，在列表里标「读音不同」。
//! 只读随包的基础词库（`dict.qj` 优先，退化到 `dict.tsv`），找不到就整项跳过。

use std::collections::{BTreeMap, BTreeSet};
use std::path::PathBuf;
use std::sync::OnceLock;

use qingjian_dictionary::Dictionary;

use super::pinyin::canonical_stored;
use crate::system::paths;

/// 基础词库的索引进程内只建一次：十三万条，每次进页重建会让「我的词」明显一顿。
static SHARED: OnceLock<Option<BTreeMap<String, BTreeSet<String>>>> = OnceLock::new();

pub struct ReadingCheck {
    /// 词 → 基础词库里的读音（已归一）。
    readings: &'static BTreeMap<String, BTreeSet<String>>,
}

impl ReadingCheck {
    pub fn load() -> Option<Self> {
        let readings = SHARED.get_or_init(index);
        readings.as_ref().map(|readings| Self { readings })
    }

    /// 用户词的读音与基础词库同词条对不上；基础词库没有这个词时不算。
    pub fn differs(&self, text: &str, pinyin: &str) -> bool {
        self.readings
            .get(text)
            .is_some_and(|readings| !readings.contains(&canonical_stored(pinyin)))
    }

    /// 基础词库给这个词的第一个读音，编辑卡片里提示用。
    pub fn base_reading(&self, text: &str) -> Option<&str> {
        self.readings
            .get(text)
            .and_then(|readings| readings.iter().next())
            .map(String::as_str)
    }
}

fn index() -> Option<BTreeMap<String, BTreeSet<String>>> {
    let path = dictionary_path()?;
    let dictionary = Dictionary::from_path(&path)
        .inspect_err(
            |error| tracing::warn!(path = %path.display(), %error, "基础词库读不了，跳过读音核对"),
        )
        .ok()?;
    let mut readings: BTreeMap<String, BTreeSet<String>> = BTreeMap::new();
    for entry in dictionary.entries() {
        readings
            .entry(entry.text.to_owned())
            .or_default()
            .insert(canonical_stored(entry.pinyin));
    }
    Some(readings)
}

/// 随包基础词库：macOS 在 `Qingjian.app/Contents/Resources/`，其他平台在随包根的 `data/generated/`；
/// 开发布局没有生成数据时退回样例词库。
fn dictionary_path() -> Option<PathBuf> {
    let named = |dir: PathBuf| {
        ["dict.qj", "dict.tsv"]
            .into_iter()
            .map(|name| dir.join(name))
            .find(|path| path.is_file())
    };
    paths::mac_resources()
        .into_iter()
        .find_map(named)
        .or_else(|| {
            [
                "data/generated/dict.qj",
                "data/generated/dict.tsv",
                "assets/sample/dict.tsv",
            ]
            .into_iter()
            .find_map(paths::bundled)
        })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn check(entries: &[(&str, &str)]) -> ReadingCheck {
        let mut readings: BTreeMap<String, BTreeSet<String>> = BTreeMap::new();
        for (text, pinyin) in entries {
            readings
                .entry((*text).to_owned())
                .or_default()
                .insert((*pinyin).to_owned());
        }
        ReadingCheck {
            readings: Box::leak(Box::new(readings)),
        }
    }

    #[test]
    fn flags_only_words_the_base_dictionary_knows_with_another_reading() {
        let check = check(&[("青简", "qing jian"), ("策略", "ce lve")]);
        assert!(!check.differs("青简", "qing jian"));
        assert!(check.differs("青简", "qing jian sheng"));
        // 基础词库没有这个词：云端学的、自造的，不标
        assert!(!check.differs("自造词", "zi zao ci"));
        // 存下的 lue 与基础词库的 lve 按同一规则归一
        assert!(!check.differs("策略", "ce lue"));
        assert_eq!(check.base_reading("青简"), Some("qing jian"));
    }
}
