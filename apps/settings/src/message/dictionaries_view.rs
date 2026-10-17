//! 词库页的两个子视图。

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum DictionariesView {
    #[default]
    Bundled,
    Personal,
}

impl DictionariesView {
    pub fn label(self) -> &'static str {
        match self {
            Self::Bundled => "随包与导入",
            Self::Personal => "我的词",
        }
    }
}
