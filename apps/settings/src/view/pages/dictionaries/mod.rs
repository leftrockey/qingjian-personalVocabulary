//! 「词库」页：顶上切「随包与导入 / 我的词」。随包与导入是整本词库的开关与导入；「我的词」管
//! 输入法攒下的 `user-words.tsv`（逐条看、改读音、自造词、导入导出）。

mod bundled;
mod personal;

use bamboo_slip::taffy::{FlexDirection, FlexWrap, NodeId, Style};
use bamboo_slip::{RenderError, Segmented, Ui, Visual};

use crate::message::{DictionariesView, Message};
use crate::view::Page;

pub fn dictionaries(ui: &mut Ui<Message>, page: &Page) -> Result<Vec<NodeId>, RenderError> {
    let theme = &page.theme;
    let view = page.settings.dictionaries_view;
    let segmented = Segmented::new(
        &[
            DictionariesView::Bundled.label(),
            DictionariesView::Personal.label(),
        ],
        usize::from(view == DictionariesView::Personal),
    )
    .build(ui, theme, "dictionaries-view", |index| {
        Message::DictionariesView(index == 1)
    })?;
    let mut out = Vec::new();
    out.push(ui.scene().node(
        Style {
            flex_direction: FlexDirection::Row,
            flex_wrap: FlexWrap::Wrap,
            gap: page.gap(12.0),
            ..Style::default()
        },
        Visual::Group,
        &[segmented],
    )?);
    match view {
        DictionariesView::Bundled => out.extend(bundled::bundled(ui, page)?),
        DictionariesView::Personal => out.extend(personal::personal(ui, page)?),
    }
    Ok(out)
}
