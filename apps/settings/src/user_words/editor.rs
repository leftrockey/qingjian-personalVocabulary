//! 「我的词」编辑卡片的草稿：改哪个词、打开时它是什么样、删除点了两下没有、存失败的原因。

/// 编辑中的一条。词与拼音在两个输入框里（`Settings::fields`），保存时一起校验。
#[derive(Debug, Clone, Default)]
pub struct WordEditor {
    /// 改的是哪个词；`None` 是新增。
    pub text: Option<String>,

    /// 保存不了的原因，显示在卡片里。
    pub error: Option<String>,

    /// 「删除」点过一次，等第二次确认。
    pub confirm_delete: bool,
}

impl WordEditor {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn edit(text: &str) -> Self {
        Self {
            text: Some(text.to_owned()),
            error: None,
            confirm_delete: false,
        }
    }

    pub fn editing(&self) -> Option<&str> {
        self.text.as_deref()
    }
}
