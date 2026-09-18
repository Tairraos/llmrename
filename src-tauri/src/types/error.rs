/// 统一错误：带上下文的用户可读错误。
///
/// 设计原则（见 AGENTS.md 品味不变量）：
/// - 错误必须携带足够上下文（路径、模板、模型摘要），for 用户直接判断
/// - Display 里给出修复提示
#[derive(Debug, thiserror::Error)]
pub enum AppError {
    #[error("配置错误：{0}")]
    Config(String),

    #[error("目录错误：{0}（请检查目录是否存在且可读）")]
    Dir(String),

    #[error("文件系统错误：{0}")]
    Fs(String),

    #[error("模板错误：{0}")]
    Template(String),

    #[error("视觉模型调用失败：{0}")]
    Vision(String),

    #[error("请求参数错误：{0}")]
    Invalid(String),

    #[error("内部错误：{0}")]
    Internal(String),
}

/// Tauri IPC 会把命令错误序列化后传给前端，这里统一序列化为可读字符串。
impl serde::Serialize for AppError {
    fn serialize<S: serde::Serializer>(&self, s: S) -> std::result::Result<S::Ok, S::Error> {
        s.serialize_str(&self.to_string())
    }
}

impl AppError {
    pub fn config(msg: impl Into<String>) -> Self {
        Self::Config(msg.into())
    }
    pub fn dir(msg: impl Into<String>) -> Self {
        Self::Dir(msg.into())
    }
    pub fn fs(msg: impl Into<String>) -> Self {
        Self::Fs(msg.into())
    }
    pub fn template(msg: impl Into<String>) -> Self {
        Self::Template(msg.into())
    }
    pub fn vision(msg: impl Into<String>) -> Self {
        Self::Vision(msg.into())
    }
    pub fn invalid(msg: impl Into<String>) -> Self {
        Self::Invalid(msg.into())
    }
    pub fn internal(msg: impl Into<String>) -> Self {
        Self::Internal(msg.into())
    }
}

impl From<std::io::Error> for AppError {
    fn from(e: std::io::Error) -> Self {
        AppError::fs(e.to_string())
    }
}

pub type Result<T> = std::result::Result<T, AppError>;
