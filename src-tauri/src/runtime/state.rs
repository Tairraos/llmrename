//! 应用共享状态。

use std::sync::atomic::AtomicBool;
use std::sync::Mutex;

use crate::service::history::HistoryStore;
use crate::types::AppConfig;

pub struct AppState {
    /// 最近一次保存/加载的配置（供 command 读取选项等）。
    pub config: Mutex<AppConfig>,
    /// 应用内重命名历史（undo/redo，内存态）。
    pub history: Mutex<HistoryStore>,
    /// 大模型填充的停止标志：置位后 ai_fill 在下一个文件边界停止。
    pub ai_fill_stop: AtomicBool,
}

impl Default for AppState {
    fn default() -> Self {
        Self {
            config: Mutex::new(AppConfig::default()),
            history: Mutex::new(HistoryStore::default()),
            ai_fill_stop: AtomicBool::new(false),
        }
    }
}
