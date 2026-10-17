//! 「我的词」顶上的筛选。

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Filter {
    #[default]
    All,
    /// 读音与基础词库同词条不一致的。
    Reading,
}

impl Filter {
    pub const ALL: [Self; 2] = [Self::All, Self::Reading];

    pub fn label(self) -> &'static str {
        match self {
            Self::All => "全部",
            Self::Reading => "读音核对",
        }
    }
}
