//! 给视觉模型的提示词（中文输出，文件名值为中文短语）。
//! 变更即提交，让模型行为可复现（见 docs/design/vision-model-contract.md）。

use std::collections::HashSet;

/// 系统提示词：约束只输出 JSON，值用中文。
pub fn system_prompt() -> &'static str {
    "你是一位严谨的照片资产命名助手。\
     你会收到一张图片和它的文件名。\
     请从图片中提取所要求的属性，只返回一个 JSON 对象\
     （不要 markdown、不要解释、不要任何额外文本），\
     每个键都必须是要求的字段名之一。\
     所有值必须是简短的中文短语，适合作为文件名的一部分：\
     不含路径分隔符、斜杠、引号、冒号等文件系统非法字符。\
     若某个属性无法从图片判断，值用 \"未知\"。\
     绝不编造与图片矛盾的值。"
}

/// 用户提示词：说明要提取哪些字段。
pub fn user_prompt(filename: &str, template: &str, fields: &[String]) -> String {
    let field_list = fields.join(", ");
    format!(
        "图片文件名：{filename}\n重命名模板：{template}\n\n\
         请提取以下字段作为 JSON 的键：[{field_list}]。\n\
         只回复 JSON 对象本身。"
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
