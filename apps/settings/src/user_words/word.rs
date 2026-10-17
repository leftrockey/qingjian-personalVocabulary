//! 列表里的一个词：词与拼音原样取自用户词表，标签由视图层按需算。

pub struct Word<'a> {
    pub text: &'a str,

    pub pinyin: &'a str,
}
