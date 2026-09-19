//! 模型列表：OpenAI 兼容 `GET {base_url}/models`（见 vision-model-contract.md）。
//! 响应在边界解析为强类型，内部只传 `Vec<String>`（模型 id）。

use crate::types::{AppError, ModelList, Result};

/// 拉取 OpenAI 兼容服务提供的模型 id 列表（排序、去重）。
pub async fn list_models(base_url: &str, api_key: &str, timeout_secs: u64) -> Result<Vec<String>> {
    if base_url.trim().is_empty() {
        return Err(AppError::config(
            "base_url 不能为空（如 https://api.openai.com/v1）",
        ));
    }
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(timeout_secs.max(1)))
        .build()
        .map_err(|e| AppError::internal(format!("构建 HTTP 客户端失败：{e}")))?;
    let url = format!("{}/models", base_url.trim_end_matches('/'));
    let mut req = client.get(&url);
    if !api_key.trim().is_empty() {
        req = req.bearer_auth(api_key.trim());
    }
    let resp = req.send().await.map_err(|e| {
        AppError::vision(format!(
            "请求模型列表失败：{e}（检查 base_url / 网络 / 超时）"
        ))
    })?;
    let status = resp.status();
    let text = resp
        .text()
        .await
        .map_err(|e| AppError::vision(format!("读取模型列表响应失败：{e}")))?;
    if !status.is_success() {
        // 尽量透传服务端 error.message，供用户直接定位（Key 无效 / 端点写错等）
        let hint = serde_json::from_str::<serde_json::Value>(&text)
            .ok()
            .and_then(|v| v["error"]["message"].as_str().map(|s| s.to_string()))
            .unwrap_or_else(|| text.chars().take(200).collect());
        return Err(AppError::vision(format!(
            "模型列表返回 HTTP {status}：{hint}（检查 API Key / base_url）"
        )));
    }
    parse_model_list(&text)
}

/// 边界解析：JSON 文本 → 排序去重的模型 id 列表。
fn parse_model_list(text: &str) -> Result<Vec<String>> {
    let list: ModelList = serde_json::from_str(text).map_err(|e| {
        AppError::vision(format!(
            "模型列表不是合法 JSON：{e}（响应片段：{}）",
            text.chars().take(120).collect::<String>()
        ))
    })?;
    let mut ids: Vec<String> = list.data.into_iter().map(|m| m.id).collect();
    ids.sort();
    ids.dedup();
    Ok(ids)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_and_sorts_dedups_ids() {
        let ids =
            parse_model_list(r#"{"data":[{"id":"b-model"},{"id":"a-model"},{"id":"b-model"}]}"#)
                .unwrap();
        assert_eq!(ids, vec!["a-model", "b-model"]);
    }

    #[test]
    fn rejects_garbage_with_fragment() {
        let err = parse_model_list("<html>404</html>").unwrap_err();
        assert!(err.to_string().contains("模型列表不是合法 JSON"));
        assert!(err.to_string().contains("404"));
    }

    #[test]
    fn empty_data_is_empty_list() {
        assert_eq!(
            parse_model_list(r#"{"data":[]}"#).unwrap(),
            Vec::<String>::new()
        );
    }
}
