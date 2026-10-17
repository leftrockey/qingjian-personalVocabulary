# 来源说明

这个仓库是 [qingjian-team/qingjian](https://github.com/qingjian-team/qingjian) 的个人派生构建，用于试用与验证「设置 → 词库 → 我的词」这一功能，**不是青简官方的上游发布**。

- 上游代码：`qingjian-team/qingjian`，本派生基线为提交 `c97ff2b`（`feat: 生词本：按天的时间线与词表，整句翻译后一并记下`）。
- 本派生新增：用户词表（`user-words.tsv`）的共享读写与输入法侧合并、`[user_words] revision` 热加载信号、设置程序「我的词」视图（搜索、筛选、读音核对、新增/编辑/删除、导入导出）。
- 产品数据（词库、语言模型、释义表）不在此仓库，打包时按 `tools/release/data.lock` 从上游 `data-v7` Release 下载并校验 SHA-256；数据版权与许可随上游，见仓库 `assets/` 各目录的 README 与上游「关于」页。
- 安装包为个人构建：Windows 包未签名、关闭 uiAccess（`QINGJIAN_UIACCESS=0`），与上游 CI 内测版一致；macOS 包为 ad-hoc 签名。

代码许可照仓库 [LICENSE](LICENSE)（GPL-3.0-or-later）；`crates/qingjian-render` 与 `crates/bamboo-slip` 另见各自目录的 `LICENSE.md`。
