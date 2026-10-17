//! 生词本页两个视图共用的小件：共用的一行 / 文字 / 标签，与生词本自己的翻页消息。

use bamboo_slip::taffy::NodeId;
use bamboo_slip::{RenderError, Ui};

use crate::message::Message;
use crate::view::Page;

pub use crate::view::parts::rows::{row, tag, text};

/// 「第 2 / 5 页」与上一页、下一页。
pub fn pager(
    ui: &mut Ui<Message>,
    page: &Page,
    current: usize,
    pages: usize,
) -> Result<NodeId, RenderError> {
    crate::view::parts::rows::pager(
        ui,
        page,
        current,
        pages,
        "vocabulary-pager",
        Message::VocabularyPage(current.saturating_sub(1)),
        Message::VocabularyPage(current + 1),
    )
}
