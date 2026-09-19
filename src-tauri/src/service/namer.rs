//! 重命名服务：编排「模板渲染 → 视觉调用 → 防冲突 → 执行改名 → 记日志」。
//! 每个文件独立处理：单个失败不中断整体，结果逐条落盘。

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use crate::repo;
use crate::types::{AppError, RenameOptions, Result};

/// 单个文件重命名的结果（含新旧绝对路径，供 undo 历史与展示）。
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct RenameOutcome {
    /// 原绝对路径
    pub from: PathBuf,
    /// 新绝对路径
    pub to: PathBuf,
    /// ok | failed | skipped
    pub status: String,
    /// 失败/跳过原因
    pub error: Option<String>,
}

/// 按显式目标名批量重命名（前端可编辑目标列）。
///
/// - `items`：RenameItem { path, target }，target 为用户手动编辑或 AI 填充后的完整文件名
/// - 每个文件独立：目标名校验（同目录冲突、扩展名允许）后 rename
/// - 返回每个文件的 RenameOutcome（含新旧绝对路径），由 Runtime 层写日志并记历史
pub fn rename_explicit(
    items: &[crate::types::RenameItem],
    options: &RenameOptions,
) -> Vec<RenameOutcome> {
    items
        .iter()
        .map(|it| {
            let from = PathBuf::from(&it.path);
            let to = match explicit_target(&from, &it.target, options) {
                Ok(t) => t,
                Err(e) => {
                    return RenameOutcome {
                        from,
                        to: PathBuf::new(),
                        status: "skipped".into(),
                        error: Some(e.to_string()),
                    };
                }
            };
            match repo::renamer::rename_path(&from, &to) {
                Ok(()) => RenameOutcome {
                    from,
                    to,
                    status: "ok".into(),
                    error: None,
                },
                Err(e) => RenameOutcome {
                    from,
                    to,
                    status: "failed".into(),
                    error: Some(e.to_string()),
                },
            }
        })
        .collect()
}

/// 校验并构造显式目标绝对路径。
fn explicit_target(from: &Path, target: &str, options: &RenameOptions) -> Result<PathBuf> {
    let target = target.trim();
    if target.is_empty() {
        return Err(AppError::invalid("目标文件名为空（跳过）"));
    }
    if target.contains('/') || target.contains('\\') {
        return Err(AppError::invalid(
            "目标文件名不能包含路径分隔符（仅重命名，不移动）",
        ));
    }
    if target.contains('\0') {
        return Err(AppError::invalid("目标文件名包含非法字符 NUL"));
    }
    // 扩展名允许校验（用户可手动编辑，仍须符合允许列表）
    let ext = target
        .rsplit_once('.')
        .map(|(_, e)| e.to_lowercase())
        .unwrap_or_default();
    if !options.allowed_suffixes.is_empty()
        && !options
            .allowed_suffixes
            .iter()
            .any(|s| s.eq_ignore_ascii_case(&format!(".{ext}")))
    {
        return Err(AppError::template(format!(
            "目标名 {target} 的扩展名 .{ext} 不在允许列表内（跳过）"
        )));
    }
    let parent = from.parent().unwrap_or(Path::new("."));
    let to = parent.join(target);
    if to == from {
        return Err(AppError::invalid("目标名与原文件名相同（跳过）"));
    }
    if to.exists() {
        return Err(AppError::fs(format!(
            "目标文件已存在：{}（跳过，请改目标名）",
            to.display()
        )));
    }
    Ok(to)
}

/// 以「字段名 → 推断值」渲染模板，把未知字段留空（预览用假数据）。
/// 实际渲染（模型返回）在 vision 中完成，这里只用于预览展示。
/// 按 char 迭代，花括号外的非 ASCII 字面量原样保留。
pub fn preview_name(pattern: &str, guesses: &HashMap<String, String>) -> String {
    let mut out = String::new();
    let chars: Vec<char> = pattern.chars().collect();
    let mut i = 0;
    while i < chars.len() {
        if chars[i] == '{' {
            if let Some(close_rel) = chars[i + 1..].iter().position(|&c| c == '}') {
                let name: String = chars[i + 1..i + 1 + close_rel].iter().collect();
                out.push_str(guesses.get(name.trim()).map(|s| s.as_str()).unwrap_or("?"));
                i += close_rel + 2;
                continue;
            }
        }
        out.push(chars[i]);
        i += 1;
    }
    out
}

/// 为预览生成每个字段的占位猜测（不做模型调用）。
pub fn preview_guesses(fields: &[String]) -> HashMap<String, String> {
    let mut m = HashMap::new();
    for f in fields {
        m.insert(f.clone(), field_example(f));
    }
    m
}

fn field_example(field: &str) -> String {
    match field {
        // 推荐的中文字段（视觉模型可从图片中提取）
        "人物" => "woman".into(),
        "人数" => "2".into(),
        "场景" => "street".into(),
        "动作" => "dancing".into(),
        "季节" => "summer".into(),
        "造型" => "hands_on_hips".into(),
        "天气" => "sunny".into(),
        "日夜" => "night".into(),
        // 兼容旧英文字段
        "date" => "2026-09-18".into(),
        "time" => "14-30-05".into(),
        "camera" => "a7m4".into(),
        "scene" => "city_night".into(),
        "location" => "tokyo".into(),
        "subject" => "cat".into(),
        "description" => "sunset_walk".into(),
        "version" => "v1".into(),
        "author" => "tairraos".into(),
        "event" => "wedding".into(),
        _ => "value".into(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn preview_replaces_fields() {
        let mut g = HashMap::new();
        g.insert("date".to_string(), "2026-09-18".to_string());
        g.insert("scene".to_string(), "city".to_string());
        assert_eq!(preview_name("{date}_{scene}", &g), "2026-09-18_city");
    }

    #[test]
    fn preview_keeps_unknown_as_question() {
        let g = HashMap::new();
        assert_eq!(preview_name("a_{x}", &g), "a_?");
    }

    #[test]
    fn preview_handles_chinese_fields() {
        let mut g = HashMap::new();
        g.insert("人物".to_string(), "woman".to_string());
        assert_eq!(preview_name("{人物}_{场景}", &g), "woman_?");
    }
}
