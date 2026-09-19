//! 模板渲染：把模型提取的字段填入模板，产出 base 名。

use serde_json::Value;

/// 用字段 map（JSON 字符串 → map）渲染模板。
/// 未知字段替换为空字符串；值清理掉非法字符（保留字母数字、下划线、连字符、点、中文）。
pub fn render_plan(pattern: &str, fields_json: &str) -> String {
    let map: std::collections::HashMap<String, String> =
        serde_json::from_str(fields_json).unwrap_or_default();
    render(pattern, &map)
}

/// 用字段 map 渲染模板（预览用）。
/// 按 char 迭代：花括号外的非 ASCII 字面量（如中文）原样保留。
pub fn render(pattern: &str, map: &std::collections::HashMap<String, String>) -> String {
    let mut out = String::new();
    let chars: Vec<char> = pattern.chars().collect();
    let mut i = 0;
    while i < chars.len() {
        if chars[i] == '{' {
            if let Some(close_rel) = chars[i + 1..].iter().position(|&c| c == '}') {
                let name: String = chars[i + 1..i + 1 + close_rel].iter().collect();
                let val = map.get(name.trim()).map(|s| s.as_str()).unwrap_or("");
                out.push_str(&sanitize(val));
                i += close_rel + 2;
                continue;
            }
        }
        out.push(chars[i]);
        i += 1;
    }
    out
}

/// 清理文件名不安全字符。
pub fn sanitize(s: &str) -> String {
    let mut out = String::new();
    let mut last_ok = false;
    for c in s.chars() {
        let ok = c.is_alphanumeric()
            || c == '_'
            || c == '-'
            || c == '.'
            || c == ' '
            || c == '('
            || c == ')';
        if ok {
            if c == ' ' && last_ok {
                out.push('_');
            } else {
                out.push(c);
            }
            last_ok = c != ' ';
        }
    }
    out
}

/// 辅助：serde_json::Value 转字段 map（供 render_plan 复用）。
pub fn value_to_map(v: &Value) -> std::collections::HashMap<String, String> {
    let mut m = std::collections::HashMap::new();
    if let Some(obj) = v.as_object() {
        for (k, val) in obj {
            m.insert(k.clone(), val.as_str().unwrap_or("").to_string());
        }
    }
    m
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn renders_fields() {
        let mut m = std::collections::HashMap::new();
        m.insert("date".into(), "2026-09-18".into());
        m.insert("scene".into(), "city night".into());
        assert_eq!(render("{date}_{scene}", &m), "2026-09-18_city_night");
    }

    #[test]
    fn unknown_field_renders_empty() {
        let m = std::collections::HashMap::new();
        assert_eq!(render("{a}{b}", &m), "");
    }

    #[test]
    fn sanitize_removes_unsafe() {
        assert_eq!(sanitize("a/b\\c:d"), "abcd");
        assert_eq!(sanitize("ok name"), "ok_name");
        assert_eq!(sanitize("中文 名称"), "中文_名称");
    }

    #[test]
    fn renders_chinese_fields_and_literals() {
        let mut m = std::collections::HashMap::new();
        m.insert("人物".into(), "woman".into());
        m.insert("日夜".into(), "night".into());
        // 花括号外的中文字面量原样保留（回归：字节渲染会产生乱码）
        assert_eq!(render("{人物}_照_{日夜}", &m), "woman_照_night");
    }
}
