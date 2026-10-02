//! 视觉模型调用：流式（SSE）请求与边界解析（见 docs/design/vision-model-contract.md）。
//!
//! 流式增量通过 `on_delta` 回调逐段抛给上层（Runtime 层负责转成 UI 事件），
//! 本层不感知 Tauri，保持依赖方向铁律。

use std::path::Path;
use std::time::Duration;

use base64::Engine;
use futures_util::StreamExt;
use serde_json::json;

use crate::types::{AppError, ChatChunk, ChatResponse, ExtractedFieldsJson, ModelConfig, Result};

/// 视觉模型调用过程事件：供 Runtime 层转成 UI 分阶段进度提示。
pub enum VisionEvent {
    /// 即将发起请求（携带完整请求 URL）
    Connecting { url: String },
    /// 流式增量文本
    Delta(String),
    /// 流结束，开始解析模型返回
    Parsing,
}

pub type VisionEventCallback<'a> = dyn FnMut(VisionEvent) + Send + 'a;

/// 按绝对路径提取要素（拖放/AI 填充场景，未受目录约束）。
/// 请求为流式；`on_delta` 逐段收到模型的原始增量文本（可为空实现）。
/// `user_hint` 为用户在界面上补充的要求（可空），追加进用户提示词。
pub async fn extract_abs(
    path: &Path,
    pattern: &str,
    model: &ModelConfig,
    user_hint: Option<&str>,
    on_event: &mut VisionEventCallback<'_>,
) -> Result<String> {
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
    let body = build_body(model, &fields, &mime, &b64, user_hint);
    let content = send_and_collect(model, body, on_event).await?;
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
    fields: &[String],
    mime: &str,
    b64: &str,
    user_hint: Option<&str>,
) -> serde_json::Value {
    json!({
        "model": model.model,
        "messages": [
            { "role": "system", "content": crate::service::prompts::system_prompt() },
            {
                "role": "user",
                "content": [
                    { "type": "text", "text": crate::service::prompts::user_prompt(fields, user_hint) },
                    {
                        "type": "image_url",
                        "image_url": { "url": format!("data:{mime};base64,{b64}") }
                    }
                ]
            }
        ],
        "temperature": 0.2,
        "max_tokens": 1024,
        "stream": true
    })
}

/// 发送流式请求并收集完整 content；每段增量调用 `on_delta`。
/// 若服务端不支持流式（响应体不是 SSE），回退为整体 JSON 解析。
async fn send_and_collect(
    model: &ModelConfig,
    body: serde_json::Value,
    on_event: &mut VisionEventCallback<'_>,
) -> Result<String> {
    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(model.timeout_secs.max(1)))
        .build()
        .map_err(|e| AppError::internal(format!("构建 HTTP 客户端失败：{e}")))?;

    let url = format!("{}/chat/completions", model.base_url.trim_end_matches('/'));
    on_event(VisionEvent::Connecting { url: url.clone() });
    let mut req = client.post(&url).json(&body);
    if !model.api_key.trim().is_empty() {
        req = req.bearer_auth(model.api_key.trim());
    }
    let resp = req.send().await.map_err(|e| {
        AppError::vision(format!("请求模型失败：{e}（检查 base_url / 网络 / 超时）"))
    })?;
    let status = resp.status();
    if !status.is_success() {
        let text = resp
            .text()
            .await
            .map_err(|e| AppError::vision(format!("读取模型响应失败：{e}")))?;
        // 尽量透传服务端 error.message
        let hint = serde_json::from_str::<serde_json::Value>(&text)
            .ok()
            .and_then(|v| v["error"]["message"].as_str().map(|s| s.to_string()))
            .unwrap_or_else(|| text.chars().take(200).collect());
        return Err(AppError::vision(format!(
            "模型返回 HTTP {status}：{hint}（检查 API Key / 配额 / 模型名）"
        )));
    }

    // 流式读取，按行解析 SSE；同时保留原始响应体供非 SSE 回退解析
    let mut content = String::new();
    let mut raw_body = String::new();
    let mut saw_sse = false;
    let mut buffer = String::new();
    let mut stream = resp.bytes_stream();
    while let Some(chunk) = stream.next().await {
        let bytes = chunk.map_err(|e| AppError::vision(format!("读取流式响应失败：{e}")))?;
        let piece = String::from_utf8_lossy(&bytes).into_owned();
        raw_body.push_str(&piece);
        buffer.push_str(&piece);
        // 处理完整行（保留最后一段不完整的）
        while let Some(pos) = buffer.find('\n') {
            let line: String = buffer.drain(..=pos).collect();
            let line = line.trim_end_matches(['\n', '\r']);
            if let Some(payload) = line.strip_prefix("data: ") {
                saw_sse = true;
                if payload.trim() == "[DONE]" {
                    return finish(content, raw_body, saw_sse, on_event);
                }
                content.push_str(&apply_chunk(payload, on_event)?);
            }
        }
    }
    finish(content, raw_body, saw_sse, on_event)
}

fn finish(
    content: String,
    raw_body: String,
    saw_sse: bool,
    on_event: &mut VisionEventCallback<'_>,
) -> Result<String> {
    // 流已结束，进入解析阶段（[DONE] 与自然结束两条路径都经过这里）
    on_event(VisionEvent::Parsing);
    if saw_sse {
        return Ok(content);
    }
    // 服务端不支持流式：整体按非流式 JSON 解析
    let resp: ChatResponse = serde_json::from_str(&raw_body)
        .map_err(|e| AppError::vision(format!("解析模型响应 JSON 失败：{e}")))?;
    parse_content(resp)
}

/// 解析一个 SSE chunk，返回其中的增量文本并触发事件回调。
fn apply_chunk(payload: &str, on_event: &mut VisionEventCallback<'_>) -> Result<String> {
    let chunk: ChatChunk = serde_json::from_str(payload)
        .map_err(|e| AppError::vision(format!("解析流式 chunk 失败：{e}（片段：{payload}）")))?;
    let mut acc = String::new();
    if let Some(choice) = chunk.choices.into_iter().next() {
        if let Some(t) = choice.delta.content {
            if !t.is_empty() {
                on_event(VisionEvent::Delta(t.clone()));
                acc = t;
            }
        } else if let Some(r) = choice.delta.reasoning_content {
            // 思考过程：仅回显给前端调试台，不进入累计内容（不参与 JSON 解析）
            if !r.is_empty() {
                on_event(VisionEvent::Delta(r));
            }
        }
    }
    Ok(acc)
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

/// 剥离代码围栏并解析 JSON 对象；兼容模型常见的不规范输出：
/// 包 ``` 围栏、字符串用单引号、在 JSON 前后加寒暄/解释文字
/// （如「Sure, here is the JSON object with the extracted fields:」）。
fn parse_json_text(content: &str) -> Result<ExtractedFieldsJson> {
    let cleaned = strip_fence(content.trim());
    // 候选：整段原文 + 所有花括号配平的片段（寒暄/围栏外的正文原样忽略）
    let mut candidates: Vec<&str> = vec![cleaned];
    candidates.extend(json_object_candidates(cleaned));
    for candidate in candidates {
        if let Ok(v) = serde_json::from_str(candidate) {
            return Ok(v);
        }
        // 模型常输出 {'人物': '一男一女'}：字符串外的单引号换成双引号再试
        if let Ok(v) = serde_json::from_str(&single_quotes_to_double(candidate)) {
            return Ok(v);
        }
    }
    // 全部失败：对原始整段再解析一次以产出 serde 详情（错误信息可执行性）
    let detail = serde_json::from_str::<ExtractedFieldsJson>(cleaned)
        .err()
        .map(|e| e.to_string())
        .unwrap_or_default();
    Err(AppError::vision(format!(
        "模型返回不是合法 JSON 对象：{detail}（响应片段：{}）",
        cleaned.chars().take(120).collect::<String>()
    )))
}

/// 扫描文本中所有「花括号配平」的片段（尊重字符串字面量内的花括号与转义），
/// 从左到右逐个起点尝试。字节级扫描安全：{ } " \ 均为 ASCII，
/// 不会出现在 UTF-8 多字节序列内部。
fn json_object_candidates(s: &str) -> Vec<&str> {
    let mut out = Vec::new();
    let bytes = s.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] != b'{' {
            i += 1;
            continue;
        }
        let mut depth = 0usize;
        let mut in_string = false;
        let mut j = i;
        while j < bytes.len() {
            let c = bytes[j];
            if in_string {
                if c == b'\\' {
                    j += 2; // 跳过转义对（越界由外层 while 兜住）
                    continue;
                }
                if c == b'"' {
                    in_string = false;
                }
            } else {
                match c {
                    b'"' => in_string = true,
                    b'{' => depth += 1,
                    b'}' => {
                        depth -= 1;
                        if depth == 0 {
                            out.push(&s[i..=j]);
                            break;
                        }
                    }
                    _ => {}
                }
            }
            j += 1;
        }
        i += 1;
    }
    out
}

/// 把字符串外的单引号替换为双引号（保留转义与双引号字符串内的内容）。
fn single_quotes_to_double(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut in_string = false;
    let mut chars = s.chars().peekable();
    while let Some(c) = chars.next() {
        match c {
            '"' => {
                out.push(c);
                in_string = !in_string;
            }
            '\\' => {
                // 转义：原样带走下一个字符（避免把 \" 中的引号当作闭合）
                out.push(c);
                if let Some(&n) = chars.peek() {
                    out.push(n);
                    chars.next();
                }
            }
            '\'' if !in_string => out.push('"'),
            _ => out.push(c),
        }
    }
    out
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
    use std::sync::{Arc, Mutex};

    /// 1x1 纯红 PNG（嵌入，测试视觉通路）
    const RED_PNG: &[u8] = &[
        0x89, 0x50, 0x4e, 0x47, 0x0d, 0x0a, 0x1a, 0x0a, 0x00, 0x00, 0x00, 0x0d, 0x49, 0x48, 0x44,
        0x52, 0x00, 0x00, 0x00, 0x01, 0x00, 0x00, 0x00, 0x01, 0x08, 0x02, 0x00, 0x00, 0x00, 0x90,
        0x77, 0x53, 0xde, 0x00, 0x00, 0x00, 0x0c, 0x49, 0x44, 0x41, 0x54, 0x78, 0x9c, 0x63, 0xf8,
        0xcf, 0xc0, 0x00, 0x00, 0x03, 0x01, 0x01, 0x00, 0xc9, 0xfe, 0x92, 0xef, 0x00, 0x00, 0x00,
        0x00, 0x49, 0x45, 0x4e, 0x44, 0xae, 0x42, 0x60, 0x82,
    ];

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
    fn parses_single_quoted_json() {
        // 模型常返回单引号 JSON：{'人物': '一男一女'}
        let j = parse_json_text("{'人物': '一男一女', '动作': '在太空中飞行'}").unwrap();
        assert_eq!(j.fields["人物"], "一男一女");
        assert_eq!(j.fields["动作"], "在太空中飞行");
    }

    #[test]
    fn extracts_json_surrounded_by_prose() {
        // 模型在 JSON 前后加寒暄/解释文字（真实故障形态）
        let s =
            "Sure, here is the JSON object with the extracted fields:\n\n{\"人物\": \"男孩子\"}";
        let j = parse_json_text(s).unwrap();
        assert_eq!(j.fields["人物"], "男孩子");
        let s2 = "{\"a\": \"1\"}\n\nHope this helps! Let me know if you need anything else.";
        assert_eq!(parse_json_text(s2).unwrap().fields["a"], "1");
    }

    #[test]
    fn prose_with_braces_still_finds_real_json() {
        // 寒暄里也含花括号：跳过非 JSON 片段，命中真正的对象
        let s = "Use the format {key: value} like this: {\"a\": \"2\"} done";
        assert_eq!(parse_json_text(s).unwrap().fields["a"], "2");
    }

    #[test]
    fn braces_inside_strings_do_not_confuse_scan() {
        // 字符串值内部的花括号/转义不破坏配平扫描
        let s = "Sure: {\"a\": \"x}y{z\", \"b\": \"quote\\\" brace}\"} end";
        let j = parse_json_text(s).unwrap();
        assert_eq!(j.fields["a"], "x}y{z");
        assert_eq!(j.fields["b"], "quote\" brace}");
    }

    #[test]
    fn prose_plus_single_quotes_combined() {
        // 寒暄 + 单引号 JSON 双重不规范
        let s = "Here you go: {'人物': '一男一女'} thanks";
        assert_eq!(parse_json_text(s).unwrap().fields["人物"], "一男一女");
    }

    #[test]
    fn single_quotes_inside_string_untouched() {
        // 单引号替换需避免破坏双引号字符串内部 / 转义内容
        let converted = single_quotes_to_double("{\"a\": \"it's\", 'b': 'x'}");
        assert_eq!(converted, "{\"a\": \"it's\", \"b\": \"x\"}");
        let j = parse_json_text("{\"a\": \"it's\", 'b': 'x'}").unwrap();
        assert_eq!(j.fields["a"], "it's");
        assert_eq!(j.fields["b"], "x");
    }

    #[test]
    fn non_json_error_is_descriptive() {
        let err = parse_json_text("not json at all").unwrap_err().to_string();
        assert!(
            err.contains("不是合法 JSON 对象"),
            "错误应说明模型返回问题：{err}"
        );
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

    #[test]
    fn apply_chunk_emits_and_collects() {
        let got = Arc::new(Mutex::new(Vec::<String>::new()));
        let g2 = Arc::clone(&got);
        let mut cb = move |ev: VisionEvent| {
            if let VisionEvent::Delta(t) = ev {
                g2.lock().unwrap().push(t);
            }
        };
        let payload = r#"{"choices":[{"delta":{"content":"he"}}]}"#;
        let text = apply_chunk(payload, &mut cb).unwrap();
        assert_eq!(text, "he");
        // 空 choices（usage 收尾块）安全
        let text2 = apply_chunk(r#"{"choices":[]}"#, &mut cb).unwrap();
        assert_eq!(text2, "");
        // 空增量不回调
        apply_chunk(r#"{"choices":[{"delta":{"content":""}}]}"#, &mut cb).unwrap();
        assert_eq!(got.lock().unwrap().len(), 1);
        // 思考过程：仅回显，不进入累计内容
        let text3 = apply_chunk(
            r#"{"choices":[{"delta":{"reasoning_content":"thinking..."}}]}"#,
            &mut cb,
        )
        .unwrap();
        assert_eq!(text3, "");
        assert_eq!(got.lock().unwrap().len(), 2);
        assert_eq!(got.lock().unwrap()[1], "thinking...");
    }

    /// 真实服务冒烟（不入 CI）：LLMRENAME_TEST_KEY=<本地key> cargo test -- --ignored
    /// 默认指向本地 OpenAI 兼容服务的视觉模型；key 必须由环境变量提供（不入库）。
    #[test]
    #[ignore = "需要本地 LLM 服务，cargo test -- --ignored 手动运行"]
    fn vision_stream_smoke_against_local_service() {
        let base = std::env::var("LLMRENAME_TEST_BASE_URL")
            .unwrap_or_else(|_| "http://127.0.0.1:8317/v1".into());
        let key = std::env::var("LLMRENAME_TEST_KEY").expect("设置 LLMRENAME_TEST_KEY");
        let model_name = std::env::var("LLMRENAME_TEST_MODEL")
            .unwrap_or_else(|_| "n/llama-3.2-11b-vision".into());

        let dir = tempfile::tempdir().unwrap();
        let img = dir.path().join("red.png");
        std::fs::write(&img, RED_PNG).unwrap();

        let model = ModelConfig {
            base_url: base,
            api_key: key,
            model: model_name,
            timeout_secs: 120,
        };
        let deltas = Arc::new(Mutex::new(Vec::<String>::new()));
        let d2 = Arc::clone(&deltas);
        let phases = Arc::new(Mutex::new(Vec::<String>::new()));
        let p2 = Arc::clone(&phases);
        let rt = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap();
        let fields_json = rt.block_on(extract_abs(
            &img,
            "{description}",
            &model,
            None,
            &mut |ev: VisionEvent| match ev {
                VisionEvent::Delta(t) => d2.lock().unwrap().push(t),
                VisionEvent::Connecting { .. } => p2.lock().unwrap().push("connecting".into()),
                VisionEvent::Parsing => p2.lock().unwrap().push("parsing".into()),
            },
        ));

        let fields_json = fields_json.expect("流式提取应成功");
        let parsed: std::collections::HashMap<String, String> =
            serde_json::from_str(&fields_json).expect("提取结果应为字段 JSON");
        assert!(
            parsed.contains_key("description"),
            "应包含 description 字段：{fields_json}"
        );
        assert!(
            !deltas.lock().unwrap().is_empty(),
            "流式回调应收到至少一段增量"
        );
        assert_eq!(
            *phases.lock().unwrap(),
            vec!["connecting".to_string(), "parsing".to_string()],
            "分阶段事件应按序发出"
        );
    }

    #[test]
    fn sse_line_parsing() {
        // 模拟 send_and_collect 的行处理逻辑：data: 前缀 / [DONE]
        let mut content = String::new();
        let mut saw_sse = false;
        let mut cb = |_: VisionEvent| {};
        for line in [
            "data: {\"choices\":[{\"delta\":{\"content\":\"a\"}}]}",
            "",
            "data: {\"choices\":[{\"delta\":{\"content\":\"b\"}}]}",
            "data: [DONE]",
        ] {
            if let Some(payload) = line.strip_prefix("data: ") {
                saw_sse = true;
                if payload.trim() == "[DONE]" {
                    break;
                }
                content.push_str(&apply_chunk(payload, &mut cb).unwrap());
            }
        }
        assert!(saw_sse);
        assert_eq!(content, "ab");
    }
}
