//! OpenAI 兼容 `GET /models` 响应的数据形状（parse-at-boundary）。

use serde::Deserialize;

/// 模型列表中的单个条目（未知字段由 serde 忽略）。
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct ModelEntry {
    pub id: String,
}

/// `GET {base_url}/models` 的响应体。
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct ModelList {
    pub data: Vec<ModelEntry>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_openai_style_list() {
        let list: ModelList = serde_json::from_str(
            r#"{"object":"list","data":[{"id":"gpt-4o","object":"model","owned_by":"openai"},{"id":"qwen-vl"}]}"#,
        )
        .unwrap();
        assert_eq!(
            list.data.iter().map(|m| m.id.clone()).collect::<Vec<_>>(),
            vec!["gpt-4o", "qwen-vl"]
        );
    }
}
