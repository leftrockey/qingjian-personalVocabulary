//! 用户词的词与拼音校验：拼音归一化成空格分隔的音节，词限制在可放进 TSV 的中文/混输写法。

use qingjian_core::parser;

/// 归一化拼音：返回空格分隔的音节。
///
/// 有空格或撇号按用户分隔；没有分隔符时按全拼切分，取第一条完整切分（`qingjian` → `qing jian`）。
/// 每个音节都做 `lue` / `nue` → `lve` / `nve` 归一；混输词里的单个字母（`C盘` → `c pan`）也接受。
pub fn normalize(input: &str) -> Result<String, String> {
    let input = input.trim();
    if input.is_empty() {
        return Err("拼音不能为空".into());
    }
    if input.chars().any(|c| c.is_control()) {
        return Err("拼音里不能有换行或控制字符".into());
    }
    if input.chars().any(|c| matches!(c, 'ü' | 'Ü')) {
        return Err("用 v 表示 ü，例如 nv、lve".into());
    }
    let lowered = input.to_ascii_lowercase().replace(['\'', '’'], " ");
    // 每个 token 本身是音节（`qing jian`）或混输词的单个字母（`c`）就原样留着；连写的一段（`qingjian`）再切开
    let mut syllables = Vec::new();
    for token in lowered.split_whitespace() {
        if parser::is_syllable(token)
            || (token.len() == 1 && token.bytes().all(|b| b.is_ascii_alphabetic()))
        {
            syllables.push(token.to_owned());
        } else {
            syllables.extend(segment_one(token)?);
        }
    }

    let mut normalized = Vec::with_capacity(syllables.len());
    for syllable in syllables {
        let syllable = canonical(&syllable);
        if parser::is_syllable(&syllable)
            || (syllable.len() == 1 && syllable.bytes().all(|b| b.is_ascii_alphabetic()))
        {
            normalized.push(syllable);
        } else {
            return Err(format!("「{syllable}」不是一个合法音节"));
        }
    }
    Ok(normalized.join(" "))
}

/// 一个音节做 `lue` / `nue` 归一；其他原样。
pub(super) fn canonical(syllable: &str) -> String {
    match syllable {
        "lue" => "lve".to_owned(),
        "nue" => "nve".to_owned(),
        _ => syllable.to_owned(),
    }
}

/// 把已经存好的拼音（如基础词库里的 `qing jian`）按同一规则归一，用于比较。
pub(super) fn canonical_stored(pinyin: &str) -> String {
    pinyin
        .split_whitespace()
        .map(canonical)
        .collect::<Vec<_>>()
        .join(" ")
}

/// 一个连写 token 切成音节：取第一条完整切分（歧义时按最贪心）。
fn segment_one(input: &str) -> Result<Vec<String>, String> {
    let segmentations =
        parser::segment(input).map_err(|_| "拼音切不开，检查一下拼写".to_owned())?;
    let complete = segmentations
        .into_iter()
        .find(|segmentation| segmentation.incomplete_count() == 0)
        .ok_or_else(|| "拼音切不开：末尾似乎没打完".to_owned())?;
    Ok(complete
        .syllables
        .into_iter()
        .map(|syllable| syllable.text)
        .collect())
}

/// 词文本：去首尾空白，拒绝空串、控制字符、tab/换行；最长 32 个字符，至少一个汉字。
pub fn validate_word(input: &str) -> Result<String, String> {
    let word = input.trim();
    if word.is_empty() {
        return Err("词不能为空".into());
    }
    if word.chars().count() > 32 {
        return Err("词最长 32 个字符".into());
    }
    if word
        .chars()
        .any(|c| c.is_control() || c == '\t' || c == '\n' || c == '\r')
    {
        return Err("词里不能有换行或制表符".into());
    }
    if !word.chars().any(is_han) {
        return Err("词里至少要有一个汉字；英文请在英文模式里输入".into());
    }
    Ok(word.to_owned())
}

fn is_han(c: char) -> bool {
    matches!(
        c as u32,
        0x3400..=0x4DBF
            | 0x4E00..=0x9FFF
            | 0xF900..=0xFAFF
            | 0x20000..=0x2FA1F
            | 0x30000..=0x3134F
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalizes_full_pinyin_and_keeps_separators() {
        assert_eq!(normalize("qingjian").unwrap(), "qing jian");
        assert_eq!(normalize(" Qing Jian ").unwrap(), "qing jian");
        assert_eq!(normalize("qing'jian").unwrap(), "qing jian");
        assert_eq!(normalize("xian").unwrap(), "xian");
    }

    #[test]
    fn canonicalizes_lue_and_nue_after_segmentation() {
        assert_eq!(normalize("celue").unwrap(), "ce lve");
        assert_eq!(normalize("celüe").unwrap_err(), "用 v 表示 ü，例如 nv、lve");
        assert_eq!(normalize("nue").unwrap(), "nve");
    }

    #[test]
    fn accepts_single_letter_syllables_for_mixed_words() {
        assert_eq!(normalize("c pan").unwrap(), "c pan");
        assert_eq!(normalize("b zhan").unwrap(), "b zhan");
    }

    #[test]
    fn rejects_unsegmentable_and_unknown_syllables() {
        assert!(normalize("nihoa").is_err());
        assert!(normalize("qing xx").is_err());
        assert!(normalize("").is_err());
    }

    #[test]
    fn glued_tokens_are_each_segmented() {
        // 多段都写成连写：逐段切开再拼
        assert_eq!(normalize("qing jian").unwrap(), "qing jian");
        assert_eq!(normalize("qingjian shijie").unwrap(), "qing jian shi jie");
    }

    #[test]
    fn word_requires_a_han_character_and_fits_the_tsv() {
        assert_eq!(validate_word(" C盘 ").unwrap(), "C盘");
        assert!(validate_word("hello").is_err());
        assert!(validate_word("青\t简").is_err());
        assert!(validate_word("青\n简").is_err());
    }
}
