//! 视觉模型调用：在边界解析请求/响应（见 docs/design/vision-model-contract.md）。

use std::path::Path;
use std::time::Duration;

use base64::Engine;
use serde_json::json;

use crate::types::{AppError, AssetEntry, ChatResponse, ExtractedFieldsJson, ModelConfig, Result};

/// 对单个资产调用视觉模型，返回「字段名 → 值」JSON 字符串（序列化后的 map）。
pub async fn extract(
    dir: &Path,
    asset: &AssetEntry,
    pattern: &str,
    model: &ModelConfig,
) -> Result<String> {
    let fields = crate::service::prompts::fields_from_pattern(pattern);
    if fields.is_empty() {
        return Err(AppError::template("模板中没有 {字段} 占位符"));
    }
    let mime = mime_for(&asset.filename);
    let b64 = read_base64(dir, asset)?;
    let body = build_body(model, asset, pattern, &fields, &mime, &b64);
    let resp = post_json(model, body).await?;
    let content = parse_content(resp)?;
    let extracted: ExtractedFieldsJson = parse_json_text(&content)?;
    serde_json::to_string(&extracted.fields)
        .map_err(|e| AppError::vision(format!("序列化提取结果失败：{e}")))
}

fn read_base64(dir: &Path, asset: &AssetEntry) -> Result<String> {
    let path = dir.join(&asset.filename);
    let bytes = std::fs::read(&path).map_err(|e| {
        AppError::fs(format!(
            "读取图片失败 {}：{e}（文件可能已被移动或删除）",
            path.display()
        ))
    })?;
    Ok(base64::engine::general_purpose::STANDARD.encode(bytes))
}

/// 按绝对路径提取要素（拖放/AI 填充场景，未受目录约束）。
pub async fn extract_abs(path: &Path, pattern: &str, model: &ModelConfig) -> Result<String> {
    let filename = path
        .file_name()
        .map(|f| f.to_string_lossy().into_owned())
        .unwrap_or_default();
    let fields = crate::service::prompts::fields_from_pattern(pattern);
    if fields.is_empty() {
        return Err(AppError::template("模板中没有 {字段} 占位符"));
    }
    let mime = mime_for(&filename);
    let bytes = std::fs::read(path).map_err(|e| {
        AppError::fs(format!(
            "读取图片失败 {}：{e}（文件可能已被移动或删除）",
            path.display()
        ))
    })?;
    let b64 = base64::engine::general_purpose::STANDARD.encode(bytes);
    let asset = AssetEntry {
        path: path.to_string_lossy().into_owned(),
        filename,
        size_bytes: 0,
        modified_secs: None,
    };
    let body = build_body(model, &asset, pattern, &fields, &mime, &b64);
    let resp = post_json(model, body).await?;
    let content = parse_content(resp)?;
    let extracted: ExtractedFieldsJson = parse_json_text(&content)?;
    serde_json::to_string(&extracted.fields)
        .map_err(|e| AppError::vision(format!("序列化提取结果失败：{e}")))
}

fn mime_for(filename: &str) -> String {
    let ext = filename
        .rsplit_once('.')
        .map(|(_, e)| e.to_lowercase())
        .unwrap_or_default();
    match ext.as_str() {
        "jpg" | "jpeg" => "image/jpeg".into(),
        "png" => "image/png".into(),
        "webp" => "image/webp".into(),
        "gif" => "image/gif".into(),
        "bmp" => "image/bmp".into(),
        "tiff" | "tif" => "image/tiff".into(),
        _ => "image/jpeg".into(),
    }
}

fn build_body(
    model: &ModelConfig,
    asset: &AssetEntry,
    pattern: &str,
    fields: &[String],
    mime: &str,
    b64: &str,
) -> serde_json::Value {
    json!({
        "model": model.model,
        "messages": [
            { "role": "system", "content": crate::service::prompts::system_prompt() },
            {
                "role": "user",
                "content": [
                    { "type": "text", "text": crate::service::prompts::user_prompt(&asset.filename, pattern, fields) },
                    {
                        "type": "image_url",
                        "image_url": { "url": format!("data:{mime};base64,{b64}") }
                    }
                ]
            }
        ],
        "temperature": 0.2,
        "max_tokens": 1024
    })
}

async fn post_json(model: &ModelConfig, body: serde_json::Value) -> Result<ChatResponse> {
    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(model.timeout_secs.max(1)))
        .build()
        .map_err(|e| AppError::internal(format!("构建 HTTP 客户端失败：{e}")))?;

    let url = format!("{}/chat/completions", model.base_url.trim_end_matches('/'));
    let mut req = client.post(&url).json(&body);
    if !model.api_key.trim().is_empty() {
        req = req.bearer_auth(model.api_key.trim());
    }
    let resp = req.send().await.map_err(|e| {
        AppError::vision(format!("请求模型失败：{e}（检查 base_url / 网络 / 超时）"))
    })?;
    let status = resp.status();
    let text = resp
        .text()
        .await
        .map_err(|e| AppError::vision(format!("读取模型响应失败：{e}")))?;

    if !status.is_success() {
        // 尽量透传服务端 error.message
        let hint = serde_json::from_str::<serde_json::Value>(&text)
            .ok()
            .and_then(|v| v["error"]["message"].as_str().map(|s| s.to_string()))
            .unwrap_or_else(|| text.chars().take(200).collect());
        return Err(AppError::vision(format!(
            "模型返回 HTTP {status}：{hint}（检查 API Key / 配额 / 模型名）"
        )));
    }
    serde_json::from_str(&text)
        .map_err(|e| AppError::vision(format!("解析模型响应 JSON 失败：{e}")))
}

fn parse_content(resp: ChatResponse) -> Result<String> {
    let content = resp
        .choices
        .into_iter()
        .next()
        .and_then(|c| c.message.content)
        .ok_or_else(|| AppError::vision("模型响应缺少 choices[0].message.content"))?;
    Ok(content)
}

/// 剥离代码围栏并解析 JSON 对象。
fn parse_json_text(content: &str) -> Result<ExtractedFieldsJson> {
    let trimmed = content.trim();
    // ```json ... ``` 或 ``` ... ```
    let cleaned = strip_fence(trimmed);
    serde_json::from_str(cleaned).map_err(|e| {
        AppError::vision(format!(
            "模型返回不是合法 JSON 对象：{e}（响应片段：{}）",
            cleaned.chars().take(120).collect::<String>()
        ))
    })
}

fn strip_fence(s: &str) -> &str {
    for prefix in ["```json", "```JSON", "```"] {
        if let Some(rest) = s.strip_prefix(prefix) {
            if let Some(end) = rest.rfind("```") {
                return rest[..end].trim();
            }
            return rest.trim();
        }
    }
    s
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn strips_markdown_fence() {
        let s = "```json\n{\"a\":\"1\"}\n```";
        assert_eq!(parse_json_text(s).unwrap().fields["a"], "1");
        let s2 = "```\n{\"a\":\"2\"}\n```";
        assert_eq!(parse_json_text(s2).unwrap().fields["a"], "2");
        let s3 = "{\"a\":\"3\"}";
        assert_eq!(parse_json_text(s3).unwrap().fields["a"], "3");
    }

    #[test]
    fn parse_content_extracts_first_choice() {
        let resp: ChatResponse =
            serde_json::from_str(r#"{"choices":[{"message":{"content":"{\"x\":\"y\"}"}}]}"#)
                .unwrap();
        assert_eq!(parse_content(resp).unwrap(), "{\"x\":\"y\"}");
    }

    #[test]
    fn mime_detection() {
        assert_eq!(mime_for("a.JPG"), "image/jpeg");
        assert_eq!(mime_for("a.png"), "image/png");
        assert_eq!(mime_for("noext"), "image/jpeg");
    }
}
