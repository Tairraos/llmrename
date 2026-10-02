//! 给视觉模型的提示词（中文输出，文件名值为中文短语）。
//! 变更即提交，让模型行为可复现（见 docs/design/vision-model-contract.md）。
//!
//! 字段提取指南（FIELD_GUIDES）：每个模板字段对应一段「怎么判断」的说明，
//! 组装进用户提示词；不把文件名/模板发给模型，避免影响模型判断。

use std::collections::HashSet;

/// 系统提示词：约束只输出 JSON，值用中文。
pub fn system_prompt() -> &'static str {
    "你是一位严谨的照片资产命名助手。\
     你会收到一张图片。\
     请从图片中提取所要求的属性，只返回一个 JSON 对象\
     （不要 markdown、不要解释、不要任何额外文本），\
     每个键都必须是要求的字段名之一。\
     所有值必须是简短的中文短语，适合作为文件名的一部分：\
     不含路径分隔符、斜杠、引号、冒号等文件系统非法字符。\
     若某个属性无法从图片判断，值用 \"未知\"。\
     绝不编造与图片矛盾的值。"
}

/// 各模板字段的提取指南（键为模板字段名原文）。
pub const FIELD_GUIDES: &[(&str, &str)] = &[
    (
        "人物",
        "根据人物的性别与年龄段返回类似描述：男孩子，女孩子，男生，女生，男人，女人，一男一女",
    ),
    ("人数", "判断图片里的人数，返回：无人/一人/双人/多人"),
    (
        "场景",
        "先判断室内还是室外；室外返回类似描述：草地，森林，广场，公园，湖边，街道；室内返回类似描述：健身房，卧室，厨房，客厅",
    ),
    (
        "动作",
        "如果是多人图片，判断多人之间的互动内容；如果是单人图片，判断人物在做的事情；判断不出人物在做什么，则判断人物的姿势",
    ),
    ("季节", "室外必须返回：春/夏/秋/冬；室内返回：未知"),
    (
        "造型",
        "描述上下身服装组合：上身类似毛衣，春装，秋装，牛仔衣，皮衣，西装，休闲装；下身类似裙子，牛仔裤，休闲裤，皮裤，牛仔短裤，短裤，短裙",
    ),
    ("天气", "天气返回：雨天，晴天，多云；室内返回：未知"),
    ("日夜", "如果是晚上，返回：夜晚；如果是白天，返回空字符串"),
    ("色调", "返回图片中最多的颜色名：红色调，黄色调，蓝色调，绿色调，灰色调，紫色调"),
    (
        "智能",
        "不提取具体属性，由你综合整张图片内容，直接想一个最合适的完整文件名：中文，不超过 12 个汉字，不要扩展名",
    ),
];

/// 未收录字段的通用指南（兼容旧英文字段等）。
const GENERIC_GUIDE: &str = "根据图片判断，返回一句简短的中文描述";

/// 查字段指南，未收录则用通用指南。
fn guide_for(field: &str) -> &'static str {
    FIELD_GUIDES
        .iter()
        .find(|(name, _)| *name == field)
        .map(|(_, g)| *g)
        .unwrap_or(GENERIC_GUIDE)
}

/// 用户提示词：只列出待提取字段及各自的提取指南，不发送文件名与模板。
/// `user_hint` 为用户在界面上补充的要求（可空），追加在字段指南之后。
pub fn user_prompt(fields: &[String], user_hint: Option<&str>) -> String {
    let items: Vec<String> = fields
        .iter()
        .map(|f| format!("{f}：{}", guide_for(f)))
        .collect();
    let mut prompt = format!(
        "请提取以下字段，并只回复一个 JSON 对象（键为字段名，值为提取结果）：\n{}",
        items.join("\n")
    );
    if let Some(hint) = user_hint.map(str::trim).filter(|s| !s.is_empty()) {
        prompt.push_str(&format!("\n用户补充要求：{hint}"));
    }
    prompt
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn user_prompt_contains_guides_not_filename_or_template() {
        let prompt = user_prompt(&["人物".into(), "日夜".into()], None);
        assert!(prompt.contains("人物"));
        assert!(prompt.contains("男孩子"));
        assert!(prompt.contains("夜晚"), "日夜指南应包含夜晚");
        assert!(!prompt.contains(".jpg"), "不应包含文件名");
        assert!(!prompt.contains("{人物}"), "不应包含模板");
    }

    #[test]
    fn unknown_field_gets_generic_guide() {
        let prompt = user_prompt(&["camera".into()], None);
        assert!(prompt.contains("camera"));
        assert!(prompt.contains("简短的中文描述"));
    }

    #[test]
    fn smart_field_gets_naming_guide() {
        let prompt = user_prompt(&["智能".into()], None);
        assert!(prompt.contains("智能"));
        assert!(prompt.contains("12 个汉字"), "智能指南应包含 12 汉字上限");
        assert!(prompt.contains("完整文件名"));
    }

    #[test]
    fn user_hint_appended_when_present() {
        let with = user_prompt(&["人物".into()], Some("重点突出人物表情"));
        assert!(with.contains("用户补充要求：重点突出人物表情"));
        let blank = user_prompt(&["人物".into()], Some("   "));
        assert!(!blank.contains("用户补充要求"), "空白提示词不追加");
    }

    #[test]
    fn fields_from_pattern_dedup_ordered() {
        let f = fields_from_pattern("{人物}-{场景}-{人物}");
        assert_eq!(f, vec!["人物".to_string(), "场景".to_string()]);
    }
}
