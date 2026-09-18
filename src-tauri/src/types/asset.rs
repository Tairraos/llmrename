use serde::{Deserialize, Serialize};

/// 资产库中一个待处理的图片文件。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AssetEntry {
    /// 绝对路径
    pub path: String,
    /// 文件名（含扩展名）
    pub filename: String,
    /// 大小（字节）
    pub size_bytes: u64,
    /// 修改时间（Unix 秒）
    pub modified_secs: Option<i64>,
}

impl AssetEntry {
    /// 提取扩展名（小写，不含点）。无扩展名时返回空字符串。
    pub fn ext(&self) -> String {
        self.filename
            .rsplit_once('.')
            .map(|(_, e)| e.to_lowercase())
            .unwrap_or_default()
    }
}
