//! 应用共享状态。

use std::sync::Mutex;

use crate::types::AppConfig;

pub struct AppState {
    /// 最近一次保存/加载的配置（供 command 读取选项等）。
    pub config: Mutex<AppConfig>,
}

impl Default for AppState {
    fn default() -> Self {
        Self {
            config: Mutex::new(AppConfig::default()),
        }
    }
}
