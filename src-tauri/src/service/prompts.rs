//! 给视觉模型的提示词（英文，稳定可审计）。
//! 变更即提交，让模型行为可复现（见 docs/design/vision-model-contract.md）。

use std::collections::HashSet;

/// 系统提示词：约束只输出 JSON。
pub fn system_prompt() -> &'static str {
    "You are a meticulous photo asset naming assistant. \
     You will be given an image and a filename. \
     Extract the requested attributes from the image, and return ONLY a single JSON object \
     (no markdown, no commentary, no surrounding text) where every key is one of the requested \
     field names. Values must be short file-system-safe strings: lowercase, use underscores \
     instead of spaces, no path separators, no slashes, no double quotes. \
     If an attribute cannot be determined from the image, use the value \"unknown\". \
     Never invent a value that contradicts the image."
}

/// 用户提示词：说明要提取哪些字段。
pub fn user_prompt(filename: &str, template: &str, fields: &[String]) -> String {
    let field_list = fields.join(", ");
    format!(
        "Asset filename: {filename}\nRename template: {template}\n\n\
         Extract exactly these field names as JSON keys: [{field_list}].\n\
         Reply with only the JSON object."
    )
}

/// 从模板提取字段名（复用 types 层的解析，去重保序）。
pub fn fields_from_pattern(pattern: &str) -> Vec<String> {
    crate::types::extract_fields(pattern)
}

/// 模板中的字段集合（用于判断模型返回是否覆盖全部字段）。
pub fn missing_fields(fields: &[String], provided: &HashSet<String>) -> Vec<String> {
    fields
        .iter()
        .filter(|f| !provided.contains(*f))
        .cloned()
        .collect()
}
