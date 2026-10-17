//! 「我的词」视图：用户词表（搜索、筛选、排序、翻页），每行可编辑读音、核对基础词库读音；
//! 新增 / 编辑在对话框里改，删除在对话框里点两下；导入导出 TSV。

use bamboo_slip::taffy::{
    AlignItems, Dimension, FlexDirection, JustifyContent, LengthPercentage, LengthPercentageAuto,
    NodeId, Rect, Size, Style,
};
use bamboo_slip::{
    Button, RenderError, Segmented, SettingRow, TextField, Ui, Visual, dialog, group, paragraph,
    section_title,
};

use crate::message::{FieldId, Message};
use crate::stats::group_digits;
use crate::user_words::{Filter, PAGE_SIZE, PersonalWords, Sort, normalize};
use crate::view::Page;
use crate::view::parts::actions::with_notice;
use crate::view::parts::rows::{pager, row, tag, text};

/// 搜索框的宽度（点）。
const SEARCH_WIDTH: f32 = 240.0;

/// 编辑卡片宽度（点）。
const DIALOG_WIDTH: f32 = 440.0;

pub fn personal(ui: &mut Ui<Message>, page: &Page) -> Result<Vec<NodeId>, RenderError> {
    // 进「我的词」时已经读过；这里（比如离屏建搜索索引）兜底用空表，不读盘
    let empty;
    let book = match page.settings.personal.as_ref() {
        Some(book) => book,
        None => {
            empty = PersonalWords::empty();
            &empty
        }
    };
    let theme = &page.theme;
    let query = page
        .settings
        .fields
        .get(&FieldId::UserWordSearch)
        .map(|edit| edit.text().to_owned())
        .unwrap_or_default();
    let words = book.matching(&query);
    let mut out = vec![paragraph(ui, theme, &summary_line(book), None)?];

    if book.changed_on_disk() {
        out.push(paragraph(
            ui,
            theme,
            "输入法在别处改过这个词表：为免覆盖，点编辑卡片里的「保存」会被拒绝，先点「重新载入」",
            None,
        )?);
    }

    let filter = Segmented::new(
        &Filter::ALL.map(Filter::label),
        Filter::ALL
            .iter()
            .position(|f| *f == book.filter)
            .unwrap_or(0),
    )
    .build(ui, theme, "user-words-filter", |index| {
        Message::UserWordsFilter(Filter::ALL[index])
    })?;
    let sort = Segmented::new(
        &Sort::ALL.map(Sort::label),
        Sort::ALL.iter().position(|s| *s == book.sort).unwrap_or(0),
    )
    .build(ui, theme, "user-words-sort", |index| {
        Message::UserWordsSort(Sort::ALL[index])
    })?;
    let mut rows = vec![
        SettingRow::new("筛选").control(filter).build(ui, theme)?,
        SettingRow::new("排序").control(sort).build(ui, theme)?,
    ];
    if let Some(edit) = page.settings.fields.get(&FieldId::UserWordSearch) {
        let id = FieldId::UserWordSearch;
        let search = TextField::new(edit)
            .placeholder("词或拼音")
            .width(SEARCH_WIDTH)
            .build(
                ui,
                theme,
                ("field", id),
                move |edit| Message::Edit(id, edit),
                None,
                Some(Message::Cancel(id)),
            )?;
        rows.push(SettingRow::new("搜索").control(search).build(ui, theme)?);
    }
    out.push(group(ui, theme, &rows)?);

    let pages = PersonalWords::pages(words.len());
    let current = book.page.min(pages - 1);
    out.push(section_title(
        ui,
        theme,
        &format!("{} 个词", group_digits(words.len() as u64)),
    )?);
    let mut rows = Vec::new();
    for word in words.iter().skip(current * PAGE_SIZE).take(PAGE_SIZE) {
        rows.push(word_row(ui, page, book, word.text, word.pinyin)?);
    }
    if rows.is_empty() {
        let message = if book.entries().is_empty() {
            "还没有用户词：输入法里选过的生词、云端学到的词会攒在这里；也可以自己新增一个"
        } else if query.trim().is_empty() && book.filter == Filter::All {
            "没有用户词"
        } else {
            "没有符合的词"
        };
        rows.push(SettingRow::new(message).build(ui, theme)?);
    }
    if pages > 1 {
        rows.push(pager(
            ui,
            page,
            current,
            pages,
            "user-words-pager",
            Message::UserWordsPage(current.saturating_sub(1)),
            Message::UserWordsPage(current + 1),
        )?);
    }
    out.push(group(ui, theme, &rows)?);

    out.extend(manage(ui, page, book)?);
    if page.settings.user_word_editor.is_some() {
        editor(ui, page, book)?;
    }
    Ok(out)
}

/// 「共 N 个词 · M 组同音词 · K 个读音与基础词库不同」
fn summary_line(book: &PersonalWords) -> String {
    let (total, homophones, readings) = book.summary();
    let mut line = format!(
        "共 {} 个词 · {} 组同音词",
        group_digits(total as u64),
        group_digits(homophones as u64)
    );
    if readings > 0 {
        line.push_str(&format!(
            " · {} 个读音与基础词库不同",
            group_digits(readings as u64)
        ));
    }
    if book.skipped > 0 {
        line.push_str(&format!("（读的时候跳过了 {} 行）", book.skipped));
    }
    line
}

/// 一个词：词（大字）、拼音、同音 / 读音不同标签，右边「编辑」。
fn word_row(
    ui: &mut Ui<Message>,
    page: &Page,
    book: &PersonalWords,
    word: &str,
    pinyin: &str,
) -> Result<NodeId, RenderError> {
    let theme = &page.theme;
    let colors = &theme.colors;
    let mut title = vec![text(ui, page, word, 18.0, 700, colors.text)?];
    let homophones = book.homophone_count(pinyin);
    if homophones > 1 {
        title.push(tag(ui, page, &format!("同音 {homophones}"), false)?);
    }
    if book.reading_differs(word, pinyin) {
        title.push(tag(ui, page, "读音不同", true)?);
    }
    let mut lines = vec![row(ui, page, 10.0, &title)?];
    lines.push(text(
        ui,
        page,
        pinyin,
        theme.font.size,
        400,
        colors.secondary,
    )?);
    let left = ui.scene().node(
        Style {
            flex_direction: FlexDirection::Column,
            gap: page.gap(6.0),
            flex_grow: 1.0,
            flex_shrink: 1.0,
            min_size: Size {
                width: LengthPercentageAuto::length(0.0),
                height: LengthPercentageAuto::auto(),
            },
            ..Style::default()
        },
        Visual::Group,
        &lines,
    )?;
    let edit = Button::new("编辑").build(
        ui,
        theme,
        ("user-word-edit", word),
        Message::OpenUserWord(Some(word.to_owned())),
    )?;
    let space = LengthPercentage::length(page.px(12.0));
    ui.scene().node(
        Style {
            flex_direction: FlexDirection::Row,
            align_items: Some(AlignItems::CENTER),
            justify_content: Some(JustifyContent::SPACE_BETWEEN),
            gap: page.gap(16.0),
            padding: Rect {
                top: space,
                bottom: space,
                left: LengthPercentage::length(0.0),
                right: LengthPercentage::length(0.0),
            },
            ..Style::default()
        },
        Visual::Group,
        &[left, edit],
    )
}

/// 「新增」「导入」「导出」「重新载入」与页面底部的说明。
fn manage(
    ui: &mut Ui<Message>,
    page: &Page,
    book: &PersonalWords,
) -> Result<Vec<NodeId>, RenderError> {
    let theme = &page.theme;
    let mut out = Vec::new();
    let add =
        Button::new("新增词…").build(ui, theme, "user-word-add", Message::OpenUserWord(None))?;
    let reload =
        Button::new("重新载入").build(ui, theme, "user-words-reload", Message::ReloadUserWords)?;
    let import =
        Button::new("导入词表…").build(ui, theme, "user-words-import", Message::ImportUserWords)?;
    let export =
        Button::new("导出词表…").build(ui, theme, "user-words-export", Message::ExportUserWords)?;
    let buttons = row(ui, page, 8.0, &[add, reload, import, export])?;
    let hint = if book.dirty {
        "有改动还没保存：点编辑卡片里的「保存」才写进文件"
    } else {
        "保存后约一秒内生效；输入法没开时下次启动生效"
    };
    let row = with_notice(
        ui,
        page,
        SettingRow::new(hint).control(buttons),
        "user-words",
    )?;
    out.push(section_title(ui, theme, "管理")?);
    out.push(group(ui, theme, &[row])?);
    out.push(paragraph(
        ui,
        theme,
        "删除只从用户词库移除；要连词频学习记录一起清掉，仍在候选窗里按 Shift + 数字",
        None,
    )?);
    Ok(out)
}

/// 新增 / 编辑的对话框。
fn editor(ui: &mut Ui<Message>, page: &Page, book: &PersonalWords) -> Result<(), RenderError> {
    let theme = &page.theme;
    let Some(editor) = page.settings.user_word_editor.as_ref() else {
        return Ok(());
    };
    let fields = &page.settings.fields;
    let (Some(word), Some(pinyin)) = (
        fields.get(&FieldId::UserWordText),
        fields.get(&FieldId::UserWordPinyin),
    ) else {
        return Ok(());
    };
    let title = ui.scene().node(
        Style::default(),
        Visual::Text {
            text: if editor.editing().is_some() {
                "编辑词"
            } else {
                "新增词"
            }
            .to_owned(),
            style: theme.text(theme.font.size + 2.0, 600, theme.colors.text),
        },
        &[],
    )?;
    let word_field = TextField::new(word)
        .placeholder("词，如 青简")
        .width(DIALOG_WIDTH - 40.0)
        .build(
            ui,
            theme,
            "user-word-text",
            |edit| Message::Edit(FieldId::UserWordText, edit),
            None,
            Some(Message::CloseUserWord),
        )?;
    let word_row = SettingRow::new("词").control(word_field).build(ui, theme)?;
    let pinyin_field = TextField::new(pinyin)
        .placeholder("拼音，如 qing jian 或 qingjian")
        .width(DIALOG_WIDTH - 40.0)
        .build(
            ui,
            theme,
            "user-word-pinyin",
            |edit| Message::Edit(FieldId::UserWordPinyin, edit),
            None,
            Some(Message::CloseUserWord),
        )?;
    let normalized = normalize(pinyin.text());
    let hint = match &normalized {
        Ok(reading) => {
            let homophones = book.homophones(word.text().trim(), reading, 5);
            let mut text = format!("切成：{reading}");
            if !homophones.is_empty() {
                text.push_str(&format!("　同音：{}", homophones.join("、")));
            }
            text
        }
        Err(reason) => reason.clone(),
    };
    let pinyin_row = SettingRow::new("拼音")
        .control(pinyin_field)
        .below(paragraph(ui, theme, &hint, None)?)
        .build(ui, theme)?;
    let mut children = vec![title, word_row, pinyin_row];
    if let Some(base) = book.base_reading(word.text().trim())
        && normalized.as_deref() != Ok(base)
    {
        let use_base = Button::new(&format!("采用基础读音：{base}")).build(
            ui,
            theme,
            "user-word-base-reading",
            Message::UserWordUseBaseReading,
        )?;
        children.push(use_base);
    }
    if let Some(error) = &editor.error {
        children.push(paragraph(ui, theme, error, None)?);
    }
    let mut buttons = Vec::new();
    if editor.editing().is_some() {
        let delete = Button::new(if editor.confirm_delete {
            "再点一次删除"
        } else {
            "删除"
        })
        .build(ui, theme, "user-word-delete", Message::DeleteUserWord)?;
        buttons.push(delete);
    }
    let spacer = ui.scene().node(
        Style {
            flex_grow: 1.0,
            size: Size {
                width: Dimension::auto(),
                height: Dimension::auto(),
            },
            ..Style::default()
        },
        Visual::Group,
        &[],
    )?;
    buttons.push(spacer);
    buttons.push(Button::new("取消").build(
        ui,
        theme,
        "user-word-cancel",
        Message::CloseUserWord,
    )?);
    buttons.push(Button::new("保存").primary().build(
        ui,
        theme,
        "user-word-save",
        Message::SaveUserWord,
    )?);
    children.push(ui.scene().node(
        Style {
            flex_direction: FlexDirection::Row,
            align_items: Some(AlignItems::CENTER),
            gap: page.gap(8.0),
            margin: Rect {
                top: LengthPercentageAuto::length(page.px(4.0)),
                ..Rect::zero()
            },
            ..Style::default()
        },
        Visual::Group,
        &buttons,
    )?);
    dialog(ui, theme, "user-word-dialog", DIALOG_WIDTH, &children)
}
