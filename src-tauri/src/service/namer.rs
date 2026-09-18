//! 重命名服务：编排「模板渲染 → 视觉调用 → 防冲突 → 执行改名 → 记日志」。
//! 每个文件独立处理：单个失败不中断整体，结果逐条落盘。

use std::collections::{HashMap, HashSet};
use std::path::Path;

use crate::repo;
use crate::types::{
    AppError, AssetEntry, LogEntry, ModelConfig, RenameOptions, RenameResult, Result,
};

/// 执行一批重命名。
///
/// - `dir`：资产目录（绝对路径）
/// - `pattern`：模板
/// - `paths`：用户勾选的绝对路径（属于 dir 内）
/// - `model` / `options`：模型与选项
///
/// 返回汇总计数；详细结果写 JSONL 日志。
pub async fn execute(
    dir: &Path,
    pattern: &str,
    paths: &[String],
    model: &ModelConfig,
    options: &RenameOptions,
    log_dir: &Path,
) -> Result<RenameResult> {
    let fields = crate::service::prompts::fields_from_pattern(pattern);
    if fields.is_empty() {
        return Err(AppError::template("模板中没有 {字段} 占位符"));
    }

    // 将绝对路径映射为目录内相对文件名，并确保属于该目录（防越界）。
    let mut assets: Vec<AssetEntry> = Vec::new();
    for p in paths {
        let pb = Path::new(p);
        let rel = pb.strip_prefix(dir).map_err(|_| {
            AppError::invalid(format!(
                "路径不在资产目录内：{p}（目录：{}）",
                dir.display()
            ))
        })?;
        if rel.components().count() != 1 {
            return Err(AppError::invalid(format!(
                "暂不支持子目录内的文件：{p}（仅支持资产目录根下的文件）"
            )));
        }
        assets.push(AssetEntry {
            path: pb.to_string_lossy().into_owned(),
            filename: rel.to_string_lossy().into_owned(),
            size_bytes: 0,
            modified_secs: None,
        });
    }

    let mut result = RenameResult::default();
    for asset in &assets {
        match rename_one(dir, asset, pattern, model, options).await {
            Ok((target, fields_json, base)) => {
                result.record("ok");
                let entry = LogEntry::ok(
                    asset.filename.clone(),
                    target,
                    Some(fields_json),
                    Some(base),
                );
                let _ = repo::logbook::append(log_dir, &entry);
            }
            Err(e) => match e {
                AppError::Template(msg) => {
                    result.record("skipped");
                    let entry = LogEntry::skipped(asset.filename.clone(), msg);
                    let _ = repo::logbook::append(log_dir, &entry);
                }
                _ => {
                    result.record("failed");
                    let entry = LogEntry::failed(asset.filename.clone(), e.to_string());
                    let _ = repo::logbook::append(log_dir, &entry);
                }
            },
        }
    }
    Ok(result)
}

/// 处理单个文件：提取 → 渲染 → 冲突处理 → 改名。
/// 成功返回 (最终名, 字段JSON, base名)。
async fn rename_one(
    dir: &Path,
    asset: &AssetEntry,
    pattern: &str,
    model: &ModelConfig,
    options: &RenameOptions,
) -> Result<(String, String, String)> {
    let fields_json = crate::service::vision::extract(dir, asset, pattern, model).await?;
    let base = crate::service::renderer::render_plan(pattern, &fields_json);
    let final_name = unique_name(dir, &base, asset, options)?;
    repo::renamer::rename(dir, &asset.filename, &final_name)?;
    Ok((final_name, fields_json, base))
}

/// 生成防冲突的最终名：后缀校验 → 黑名单 → 追加序号。
fn unique_name(
    dir: &Path,
    base: &str,
    asset: &AssetEntry,
    options: &RenameOptions,
) -> Result<String> {
    let ext = asset.ext();
    let allowed = if options.allowed_suffixes.is_empty() {
        true
    } else {
        options
            .allowed_suffixes
            .iter()
            .any(|s| s.eq_ignore_ascii_case(&format!(".{ext}")))
    };
    if !allowed {
        return Err(AppError::template(format!(
            "文件 {} 的扩展名 .{ext} 不在允许列表内（跳过）",
            asset.filename
        )));
    }
    for word in &options.blacklist {
        if base.to_lowercase().contains(&word.to_lowercase()) {
            return Err(AppError::template(format!(
                "生成名包含黑名单词 “{word}”（跳过）：{base}"
            )));
        }
    }
    let candidate = format!("{base}.{ext}");
    if !dir.join(&candidate).exists() {
        return Ok(candidate);
    }
    // 冲突：追加 _2, _3, ...
    for i in 2..=9999 {
        let c = format!("{base}_{i}.{ext}");
        if !dir.join(&c).exists() {
            return Ok(c);
        }
    }
    Err(AppError::template(format!(
        "无法为 {base}.{ext} 找到可用名（重名过多，跳过）"
    )))
}

/// 以「字段名 → 推断值」渲染模板，把未知字段留空（预览用假数据）。
/// 实际渲染（模型返回）在 vision 中完成，这里只用于预览展示。
pub fn preview_name(pattern: &str, guesses: &HashMap<String, String>) -> String {
    let mut out = String::new();
    let bytes = pattern.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'{' {
            if let Some(close_rel) = bytes[i + 1..].iter().position(|&b| b == b'}') {
                let name = &pattern[i + 1..i + 1 + close_rel];
                out.push_str(guesses.get(name).map(|s| s.as_str()).unwrap_or("?"));
                i += close_rel + 2;
                continue;
            }
        }
        out.push(bytes[i] as char);
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

/// 辅助：从字段 map 构建 HashSet（供 prompts::missing_fields 使用）。
pub fn provided_set(fields: &HashMap<String, String>) -> HashSet<String> {
    fields.keys().cloned().collect()
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
    fn unique_name_appends_suffix_on_conflict() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("a.jpg"), b"x").unwrap();
        std::fs::write(dir.path().join("b.jpg"), b"x").unwrap();
        std::fs::write(dir.path().join("b_2.jpg"), b"x").unwrap();
        let asset = AssetEntry {
            path: dir.path().join("a.jpg").to_string_lossy().into_owned(),
            filename: "a.jpg".into(),
            size_bytes: 1,
            modified_secs: Some(0),
        };
        let options = RenameOptions::default();
        // b.jpg 与 b_2.jpg 已存在 → 应得到 b_3.jpg
        let name = unique_name(dir.path(), "b", &asset, &options).unwrap();
        assert_eq!(name, "b_3.jpg");
    }

    #[test]
    fn unique_name_skips_blacklist_and_bad_suffix() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("a.jpg"), b"x").unwrap();
        let asset = AssetEntry {
            path: dir.path().join("a.jpg").to_string_lossy().into_owned(),
            filename: "a.jpg".into(),
            size_bytes: 1,
            modified_secs: Some(0),
        };
        let mut options = RenameOptions {
            blacklist: vec!["bad".into()],
            ..Default::default()
        };
        assert!(unique_name(dir.path(), "bad_name", &asset, &options).is_err());
        options.blacklist = vec![];
        options.allowed_suffixes = vec![".png".into()];
        assert!(unique_name(dir.path(), "base", &asset, &options).is_err());
    }

    #[test]
    fn execute_requires_fields() {
        // 无字段模板应报错
        let rt = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap();
        let dir = tempfile::tempdir().unwrap();
        let log = tempfile::tempdir().unwrap();
        let r = rt.block_on(execute(
            dir.path(),
            "fixed",
            &[],
            &ModelConfig::default(),
            &RenameOptions::default(),
            log.path(),
        ));
        assert!(r.is_err());
    }

    #[test]
    fn path_outside_dir_rejected() {
        let rt = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap();
        let dir = tempfile::tempdir().unwrap();
        let log = tempfile::tempdir().unwrap();
        let outside = tempfile::tempdir().unwrap();
        let p = outside.path().join("x.jpg");
        std::fs::write(&p, b"x").unwrap();
        let r = rt.block_on(execute(
            dir.path(),
            "{date}",
            &[p.to_string_lossy().into_owned()],
            &ModelConfig::default(),
            &RenameOptions::default(),
            log.path(),
        ));
        assert!(r.is_err());
    }
}
