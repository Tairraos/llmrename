use std::collections::HashMap;

use serde::{Deserialize, Serialize};

/// 单个文件的执行结果汇总。
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct RenameResult {
    pub ok_count: usize,
    pub fail_count: usize,
    pub skip_count: usize,
}

impl RenameResult {
    pub fn record(&mut self, status: &str) {
        match status {
            "ok" => self.ok_count += 1,
            "skipped" => self.skip_count += 1,
            _ => self.fail_count += 1,
        }
    }
}

/// 视觉模型返回的字段 JSON（在边界解析，见 vision-model-contract.md）。
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct ExtractedFieldsJson {
    #[serde(flatten)]
    pub fields: HashMap<String, String>,
}

/// 模型一次请求的原始响应（只取需要的字段，parse-at-boundary）。
#[derive(Debug, Clone, Deserialize)]
pub struct ChatResponse {
    pub choices: Vec<ChatChoice>,
}

/// SSE 流式 chunk（OpenAI chat.completion.chunk；字段宽容缺省）。
#[derive(Debug, Clone, Deserialize)]
pub struct ChatChunk {
    #[serde(default)]
    pub choices: Vec<ChatDeltaChoice>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ChatDeltaChoice {
    #[serde(default)]
    pub delta: ChatDelta,
    /// 最后一块携带 stop
    #[serde(default)]
    pub finish_reason: Option<String>,
}

#[derive(Debug, Clone, Default, Deserialize)]
pub struct ChatDelta {
    #[serde(default)]
    pub content: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ChatChoice {
    pub message: ChatMessage,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ChatMessage {
    pub content: Option<String>,
}
