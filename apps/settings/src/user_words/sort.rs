//! 「我的词」的排序。

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Sort {
    #[default]
    Word,
    /// 按拼音；同一读音下再按词，同音词挨在一起。
    Reading,
}

impl Sort {
    pub const ALL: [Self; 2] = [Self::Word, Self::Reading];

    pub fn label(self) -> &'static str {
        match self {
            Self::Word => "按词",
            Self::Reading => "按读音",
        }
    }
}
