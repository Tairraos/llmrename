//! 重命名历史模型：供 undo/redo 与前端交互使用。

use serde::{Deserialize, Serialize};

/// 前端下发的一次重命名请求：文件路径 + 目标文件名（含扩展名，可手动编辑）。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RenameItem {
    pub path: String,
    pub target: String,
}

/// 撤销/重做后的状态快照（返回给前端更新按钮可用态与展示摘要）。
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct HistoryStatus {
    /// 可撤销步数
    pub undoable: usize,
    /// 可重做步数
    pub redoable: usize,
    /// 本次操作摘要（如 "a.jpg → b.jpg"）；无操作时为 None
    pub applied: Option<String>,
}
