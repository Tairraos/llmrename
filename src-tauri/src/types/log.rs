use serde::{Deserialize, Serialize};

/// 日志条目：一次重命名的结果（成功 / 失败 / 跳过）。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LogEntry {
    /// Unix 秒
    pub ts: i64,
    /// ok | failed | skipped
    pub status: String,
    /// 原文件名（含扩展名）
    pub asset: String,
    /// 目标文件名（失败/跳过时为 None）
    pub target: Option<String>,
    /// 视觉模型提取的要素快照（JSON 字符串）
    pub fields: Option<String>,
    /// 渲染后的 base 名（含字段替换，不含去重后缀）
    pub base: Option<String>,
    /// 错误信息（仅失败/跳过）
    pub error: Option<String>,
}

impl LogEntry {
    pub fn ok(asset: String, target: String, fields: Option<String>, base: Option<String>) -> Self {
        Self {
            ts: now(),
            status: "ok".into(),
            asset,
            target: Some(target),
            fields,
            base,
            error: None,
        }
    }

    pub fn failed(asset: String, error: String) -> Self {
        Self {
            ts: now(),
            status: "failed".into(),
            asset,
            target: None,
            fields: None,
            base: None,
            error: Some(error),
        }
    }

    pub fn skipped(asset: String, error: String) -> Self {
        Self {
            ts: now(),
            status: "skipped".into(),
            asset,
            target: None,
            fields: None,
            base: None,
            error: Some(error),
        }
    }
}

fn now() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0)
}
