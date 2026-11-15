//! Mojuan core —— 与 UI 完全解耦的 Markdown 引擎与文件服务。
//!
//! - `doc`：Markdown → 结构化文档模型（标题/段落/代码/表格/列表/图片…）
//! - `highlight`：代码语义高亮（主题无关）
//! - `files`：工作区目录、读写、新建/重命名/删除
//! - `search`：文件名 + 全文检索
//! - `store`：设置与最近记录持久化

pub mod doc;
pub mod files;
pub mod http;
pub mod highlight;
pub mod large;
pub mod math;
pub mod mermaid;
pub mod search;
pub mod store;
pub mod update;
pub mod vindex;

pub use doc::{Align, Block, Doc, ListItem, Run, Stats, TocEntry};
pub use highlight::TokenStyle;
pub use store::{Settings, Store, Theme, ViewMode};

/// 应用信息
pub const APP_NAME: &str = "墨卷";
pub const APP_DIR: &str = "Mojuan";
pub const VERSION: &str = env!("CARGO_PKG_VERSION");
