//! 词库页顶上的视图切换与「我的词」的消息处理：加载、搜索、排序、编辑卡片、保存冲突、导入导出。

use std::path::Path;

use bamboo_slip::TextEdit;

use super::Settings;
use crate::message::{DictionariesView, FieldId, Message};
use crate::user_words::{FILE, PersonalWords, WordEditor, normalize, validate_word};

impl Settings {
    /// 进词库页、切到「我的词」时读文件；搜索框清空。
    pub(super) fn open_user_words(&mut self) {
        self.personal = Some(PersonalWords::load());
        self.fields
            .insert(FieldId::UserWordSearch, TextEdit::new(String::new()));
        self.dictionaries_view = DictionariesView::Personal;
    }

    pub(super) fn open_user_word_editor(&mut self, text: Option<&str>) {
        let Some(book) = self.personal.as_ref() else {
            return;
        };
        let existing: Option<(&str, &String)> = text.and_then(|text| {
            book.entries()
                .get_key_value(text)
                .map(|(text, pinyin)| (text.as_str(), pinyin))
        });
        let (editor, word, pinyin) = match existing {
            Some((text, pinyin)) => (WordEditor::edit(text), text.to_owned(), pinyin.clone()),
            None => (WordEditor::new(), String::new(), String::new()),
        };
        self.fields
            .insert(FieldId::UserWordText, TextEdit::new(word));
        self.fields
            .insert(FieldId::UserWordPinyin, TextEdit::new(pinyin));
        self.user_word_editor = Some(editor);
    }

    pub(super) fn close_user_word_editor(&mut self) {
        self.user_word_editor = None;
        self.fields.remove(&FieldId::UserWordText);
        self.fields.remove(&FieldId::UserWordPinyin);
    }

    /// 卡片里的词与拼音；校验不过的原因交给卡片。
    fn user_word_draft(&self) -> Result<(String, String), String> {
        let field = |id| {
            self.fields
                .get(&id)
                .map(|edit: &TextEdit| edit.text().to_owned())
                .unwrap_or_default()
        };
        let text = validate_word(&field(FieldId::UserWordText))?;
        let pinyin = normalize(&field(FieldId::UserWordPinyin))?;
        Ok((text, pinyin))
    }

    /// 把编辑卡片里的内容并进表里；`delete` 时删掉原来那条。失败把原因留在卡片里。
    pub(super) fn save_user_word(&mut self, delete: bool) {
        let Some(editor) = self.user_word_editor.clone() else {
            return;
        };
        let old = editor.editing().map(str::to_owned);
        if let Some(old) = &old {
            let gone = self
                .personal
                .as_ref()
                .is_none_or(|book| book.entries().get(old).is_none());
            if gone {
                self.set_word_error("这个词已经不在列表里了".to_owned());
                return;
            }
        }
        let draft = if delete {
            Ok(None)
        } else {
            self.user_word_draft().map(Some)
        };
        let draft = match draft {
            Ok(draft) => draft,
            Err(reason) => {
                self.set_word_error(reason);
                return;
            }
        };
        let Some(book) = self.personal.as_mut() else {
            return;
        };
        match (old.as_deref(), draft) {
            (Some(old), Some((text, pinyin))) => book.replace(old, &text, pinyin),
            (Some(old), None) => {
                book.remove(old);
            }
            (None, Some((text, pinyin))) => book.set(&text, pinyin),
            (None, None) => {}
        }
        match book.save() {
            Ok(()) => {
                self.bump_user_words_revision();
                self.close_user_word_editor();
                self.notice = Some(("user-words", "已保存，输入法约一秒后生效".to_owned()));
            }
            Err(reason) => {
                // 文件在别处变了（或写不进去）：扔掉这次改动，显示磁盘上那份
                book.reload();
                book.discard();
                self.set_word_error(reason);
            }
        }
    }

    /// 错误留在卡片里；卡片已经关了就不再管。
    fn set_word_error(&mut self, reason: String) {
        if let Some(editor) = self.user_word_editor.as_mut() {
            editor.error = Some(reason);
            editor.confirm_delete = false;
        }
    }

    /// 配置里的 `[user_words] revision` 加一：输入法靠配置热加载知道用户词文件变了。
    fn bump_user_words_revision(&mut self) {
        let revision = self.config.user_words.revision.wrapping_add(1);
        self.write(
            "user_words",
            "revision",
            i64::try_from(revision).unwrap_or(i64::MAX),
        );
    }

    pub(super) fn user_words_message(&mut self, message: Message) {
        let query = self
            .fields
            .get(&FieldId::UserWordSearch)
            .map(|edit| edit.text().to_owned())
            .unwrap_or_default();
        let Some(book) = self.personal.as_mut() else {
            return;
        };
        match message {
            Message::UserWordsFilter(filter) => {
                book.filter = filter;
                book.page = 0;
            }
            Message::UserWordsSort(sort) => {
                book.sort = sort;
                book.page = 0;
            }
            Message::UserWordsPage(page) => book.page = page,
            Message::ReloadUserWords => {
                book.reload();
                let skipped = book.skipped;
                self.close_user_word_editor();
                self.notice = Some((
                    "user-words",
                    if skipped > 0 {
                        format!("已重新载入，跳过了 {skipped} 行格式不对的")
                    } else {
                        "已重新载入".to_owned()
                    },
                ));
            }
            Message::OpenUserWord(text) => {
                let _ = book;
                self.open_user_word_editor(text.as_deref());
            }
            Message::SaveUserWord => self.save_user_word(false),
            Message::DeleteUserWord => {
                let confirmed = self
                    .user_word_editor
                    .as_ref()
                    .is_some_and(|editor| editor.confirm_delete);
                if confirmed {
                    self.save_user_word(true);
                } else if let Some(editor) = self.user_word_editor.as_mut() {
                    editor.confirm_delete = true;
                }
            }
            Message::CloseUserWord => self.close_user_word_editor(),
            Message::UserWordUseBaseReading => {
                // 按输入框里现在写的词查基础词库（新增时还没有原来的词）
                let text = self
                    .fields
                    .get(&FieldId::UserWordText)
                    .map(|edit| edit.text().trim().to_owned())
                    .unwrap_or_default();
                if let Some(reading) = self
                    .personal
                    .as_ref()
                    .and_then(|book| book.base_reading(&text))
                    .map(str::to_owned)
                {
                    self.fields
                        .insert(FieldId::UserWordPinyin, TextEdit::new(reading));
                }
            }
            Message::ImportUserWords => self.import_user_words(),
            Message::ExportUserWords => self.export_user_words(&query),
            _ => {}
        }
    }

    /// 选一个青简 TSV 合进我的词；合并后当场存盘（存不了就只留在内存里，让用户再点保存）。
    fn import_user_words(&mut self) {
        let Some(path) = rfd::FileDialog::new()
            .set_title("导入用户词")
            .add_filter("青简词表", &["tsv", "txt"])
            .pick_file()
        else {
            return;
        };
        let source = match std::fs::read(&path) {
            Ok(bytes) => String::from_utf8_lossy(&bytes).into_owned(),
            Err(error) => {
                self.notice = Some(("user-words", format!("读不了这个文件：{error}")));
                return;
            }
        };
        let Some(book) = self.personal.as_mut() else {
            return;
        };
        let report = book.merge(&source);
        match book.save() {
            Ok(()) => {
                self.bump_user_words_revision();
                self.notice = Some(("user-words", report.summary()));
            }
            Err(reason) => {
                book.reload();
                book.discard();
                self.notice = Some(("user-words", format!("{}；{}", report.summary(), reason)));
            }
        }
    }

    /// 把当前筛出来的词写成 TSV。
    fn export_user_words(&mut self, query: &str) {
        let Some(book) = self.personal.as_ref() else {
            return;
        };
        let text = book.export_text(query);
        let rows = text.lines().count().saturating_sub(1);
        if rows == 0 {
            self.notice = Some(("user-words", "没有可导出的词".to_owned()));
            return;
        }
        let Some(target) = rfd::FileDialog::new()
            .set_title("导出用户词")
            .set_file_name(FILE)
            .add_filter("青简词表", &["tsv"])
            .save_file()
        else {
            return;
        };
        match write_text(&target, &text) {
            Ok(()) => {
                self.notice = Some((
                    "user-words",
                    format!("已导出 {rows} 个词到 {}", target.display()),
                ));
            }
            Err(error) => {
                self.notice = Some(("user-words", format!("写不进去：{error}")));
            }
        }
    }
}

/// 原子写文本：导出中途断掉不留下半个文件。
fn write_text(path: &Path, text: &str) -> std::io::Result<()> {
    qingjian_core::storage::write_atomic_str(path, text)
}
