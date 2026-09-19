use serde::{Deserialize, Serialize};

/// 视觉模型配置。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ModelConfig {
    /// API 端点（可含 /v1），如 https://api.openai.com/v1
    pub base_url: String,
    /// API Key，仅存本机配置文件
    pub api_key: String,
    /// 模型名
    pub model: String,
    /// 单请求超时秒数
    pub timeout_secs: u64,
}

impl Default for ModelConfig {
    fn default() -> Self {
        Self {
            base_url: "https://api.openai.com/v1".into(),
            api_key: String::new(),
            model: "gpt-4o".into(),
            timeout_secs: 60,
        }
    }
}

/// 重命名模板：{字段} 占位符模式，如 `{人物}_{场景}_{动作}_{日夜}`。
/// 字段名支持 Unicode 字母数字（含中文）与下划线。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TemplateConfig {
    pub pattern: String,
}

impl Default for TemplateConfig {
    fn default() -> Self {
        Self {
            pattern: "{人物}_{场景}_{动作}_{日夜}".into(),
        }
    }
}

/// 重命名选项。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RenameOptions {
    /// 扫描时允许的扩展名（不含点）。
    pub extensions: Vec<String>,
    /// 新文件名禁止包含的词汇（命中则跳过）。
    pub blacklist: Vec<String>,
    /// 允许的目标文件后缀（含点，小写）。
    pub allowed_suffixes: Vec<String>,
}

impl Default for RenameOptions {
    fn default() -> Self {
        Self {
            extensions: vec![
                "jpg".into(),
                "jpeg".into(),
                "png".into(),
                "webp".into(),
                "gif".into(),
                "bmp".into(),
                "tiff".into(),
            ],
            blacklist: vec![],
            allowed_suffixes: vec![
                ".jpg".into(),
                ".jpeg".into(),
                ".png".into(),
                ".webp".into(),
                ".gif".into(),
                ".bmp".into(),
                ".tiff".into(),
            ],
        }
    }
}

/// 应用整体配置（前端一次下发 / 保存）。
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct AppConfig {
    pub model: ModelConfig,
    pub template: TemplateConfig,
    pub options: RenameOptions,
}

/// 从模板 pattern 中提取 {字段名} 列表（去重、保序）。
/// 字段名为 Unicode 字母数字（含中文）与下划线的组合。
pub fn extract_fields(pattern: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut seen = std::collections::HashSet::new();
    let bytes = pattern.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'{' {
            if let Some(close_rel) = bytes[i + 1..].iter().position(|&b| b == b'}') {
                let name = &pattern[i + 1..i + 1 + close_rel];
                let name = name.trim();
                if !name.is_empty()
                    && name.chars().all(is_field_char)
                    && seen.insert(name.to_string())
                {
                    out.push(name.to_string());
                }
                i += close_rel + 2;
                continue;
            }
        }
        i += 1;
    }
    out
}

/// 字段名合法字符：Unicode 字母数字（含中文）或下划线。
fn is_field_char(c: char) -> bool {
    c.is_alphanumeric() || c == '_'
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extracts_fields_dedup_preserving_order() {
        assert_eq!(
            extract_fields("{date}_{camera}_{scene}_{date}"),
            vec!["date", "camera", "scene"]
        );
    }

    #[test]
    fn ignores_malformed_placeholders() {
        assert_eq!(extract_fields("{date} {bad-name} {ok}"), vec!["date", "ok"]);
        assert_eq!(extract_fields("no fields"), Vec::<String>::new());
    }

    #[test]
    fn extracts_chinese_fields() {
        assert_eq!(
            extract_fields("{人物}_{人数}x_{场景}_{动作}_{季节}_{造型}_{天气}_{日夜}"),
            vec!["人物", "人数", "场景", "动作", "季节", "造型", "天气", "日夜"]
        );
        // 中文+英文混排、重复去重
        assert_eq!(
            extract_fields("{人物}_{scene}_{人物}"),
            vec!["人物", "scene"]
        );
        // 含空格/连字符的占位符仍不视为字段
        assert_eq!(extract_fields("{人物 名}"), Vec::<String>::new());
    }

    #[test]
    fn default_template_uses_chinese_fields() {
        assert_eq!(
            TemplateConfig::default().pattern,
            "{人物}_{场景}_{动作}_{日夜}"
        );
    }

    #[test]
    fn config_defaults_roundtrip() {
        let cfg = AppConfig::default();
        let json = serde_json::to_string(&cfg).unwrap();
        let back: AppConfig = serde_json::from_str(&json).unwrap();
        assert_eq!(cfg, back);
    }
}
