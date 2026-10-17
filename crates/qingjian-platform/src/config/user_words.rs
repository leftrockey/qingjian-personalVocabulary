//! `[user_words]` 分节：设置程序写完 `user-words.tsv` 后加一，输入法热加载配置时据此重读用户词。
//!
//! 这个计数不是用户设置，只当跨进程的「文件变了」信号；用户词本身仍在学习数据目录的
//! `user-words.tsv`，不放进配置文件。

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct UserWordsConfig {
    /// 设置程序每写一次用户词表就加一；输入法只在配置热加载时看文件，不看这个数本身。
    pub revision: u64,
}
