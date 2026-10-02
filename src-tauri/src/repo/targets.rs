//! 拖放路径收集：把用户拖入/选择的「目录或图片文件」归一化为文件列表。
//! 目录递归扫描；扩展名不在允许列表但本身是文件的可直接收（用户在列表里可改目标名）。

use std::path::Path;

use crate::repo::sort;
use crate::types::{AppError, AssetEntry, Result};

/// 从多个输入路径收集文件。
/// - 目录：递归扫描其中匹配 extensions 的文件
/// - 文件：直接收（即使扩展名不在列表，用户可手动改目标名；重命名时校验扩展名）
pub fn collect_targets(
    paths: &[String],
    extensions: &[String],
    max_depth: usize,
) -> Result<Vec<AssetEntry>> {
    let mut out = Vec::new();
    // 去重（同一文件可能被多次拖入）
    let mut seen = std::collections::HashSet::new();
    for p in paths {
        let path = Path::new(p);
        // 相对路径的基准：目录用它自身，单文件用其父目录
        let root = if path.is_dir() {
            path.to_path_buf()
        } else {
            path.parent().unwrap_or(path).to_path_buf()
        };
        if path.is_dir() {
            collect_dir(&root, path, extensions, max_depth, 0, &mut seen, &mut out)?;
        } else if path.is_file() {
            push_entry(&root, path, &mut seen, &mut out);
        } else {
            return Err(AppError::invalid(format!(
                "路径不存在：{p}（请检查文件或文件夹）"
            )));
        }
    }
    out.sort_by(|a, b| sort::natural_cmp(&a.filename, &b.filename));
    Ok(out)
}

fn collect_dir(
    root: &Path,
    dir: &Path,
    extensions: &[String],
    max_depth: usize,
    depth: usize,
    seen: &mut std::collections::HashSet<String>,
    out: &mut Vec<AssetEntry>,
) -> Result<()> {
    if depth > max_depth {
        return Err(AppError::invalid(format!(
            "目录嵌套过深（超过 {max_depth} 层）：{}",
            dir.display()
        )));
    }
    let rd = std::fs::read_dir(dir)
        .map_err(|e| AppError::dir(format!("无法读取目录 {}：{e}", dir.display())))?;
    let mut subdirs = Vec::new();
    for ent in rd {
        let ent = match ent {
            Ok(e) => e,
            Err(_) => continue,
        };
        let ft = match ent.file_type() {
            Ok(t) => t,
            Err(_) => continue,
        };
        if ft.is_dir() {
            subdirs.push(ent.path());
        } else if ft.is_file() {
            let name = ent.file_name().to_string_lossy().into_owned();
            if extensions.is_empty() || has_ext(&name, extensions) {
                push_entry(root, &ent.path(), seen, out);
            }
        }
    }
    // 子目录按名称稳定排序后递归
    subdirs.sort();
    for sub in subdirs {
        collect_dir(root, &sub, extensions, max_depth, depth + 1, seen, out)?;
    }
    Ok(())
}

fn push_entry(
    root: &Path,
    path: &Path,
    seen: &mut std::collections::HashSet<String>,
    out: &mut Vec<AssetEntry>,
) {
    let key = path.to_string_lossy().into_owned();
    if !seen.insert(key.clone()) {
        return;
    }
    let meta = match path.metadata() {
        Ok(m) => m,
        Err(_) => return,
    };
    let relative_path = path
        .strip_prefix(root)
        .map(|p| p.to_string_lossy().into_owned())
        .unwrap_or_else(|_| path.to_string_lossy().into_owned());
    out.push(AssetEntry {
        path: key,
        filename: path
            .file_name()
            .map(|f| f.to_string_lossy().into_owned())
            .unwrap_or_default(),
        relative_path,
        size_bytes: meta.len(),
        modified_secs: meta
            .modified()
            .ok()
            .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
            .map(|d| d.as_secs() as i64),
    });
}

fn has_ext(name: &str, extensions: &[String]) -> bool {
    name.rsplit_once('.')
        .map(|(_, e)| extensions.iter().any(|x| x == &e.to_lowercase()))
        .unwrap_or(false)
}

/// 自然排序（与 scanner 一致，供列表展示稳定排序）。
pub fn natural_cmp(a: &str, b: &str) -> std::cmp::Ordering {
    let ab = a.as_bytes();
    let bb = b.as_bytes();
    let (mut ai, mut bi) = (0, 0);
    loop {
        if ai >= a.len() || bi >= b.len() {
            return a.len().cmp(&b.len());
        }
        let ad = ab[ai].is_ascii_digit();
        let bd = bb[bi].is_ascii_digit();
        match (ad, bd) {
            (true, true) => {
                let (av, an) = digit_block(a, ai);
                let (bv, bn) = digit_block(b, bi);
                let ord = av.cmp(&bv);
                if ord != std::cmp::Ordering::Equal {
                    return ord;
                }
                ai += an;
                bi += bn;
            }
            (true, false) => return std::cmp::Ordering::Less,
            (false, true) => return std::cmp::Ordering::Greater,
            (false, false) => {
                let ord = ab[ai].cmp(&bb[bi]);
                if ord != std::cmp::Ordering::Equal {
                    return ord;
                }
                ai += 1;
                bi += 1;
            }
        }
    }
}

fn digit_block(s: &str, pos: usize) -> (u64, usize) {
    let bytes = s.as_bytes();
    let mut end = pos;
    while end < bytes.len() && bytes[end].is_ascii_digit() {
        end += 1;
    }
    let trimmed = s[pos..end].trim_start_matches('0');
    let value = if trimmed.is_empty() {
        0
    } else {
        trimmed.parse().unwrap_or(u64::MAX)
    };
    (value, end - pos)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn collect_targets_sets_relative_paths() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path().join("assets");
        std::fs::create_dir_all(root.join("sub/deep")).unwrap();
        std::fs::write(root.join("b.jpg"), "x").unwrap();
        std::fs::write(root.join("sub/a.jpg"), "x").unwrap();
        std::fs::write(root.join("sub/deep/c.png"), "x").unwrap();

        let entries = collect_targets(
            &[root.to_string_lossy().into_owned()],
            &["jpg".into(), "png".into()],
            8,
        )
        .unwrap();

        let rel: Vec<String> = entries.iter().map(|e| e.relative_path.clone()).collect();
        // 按文件名自然排序：a.jpg < b.jpg < c.png
        assert_eq!(
            rel,
            vec!["sub/a.jpg", "b.jpg", "sub/deep/c.png"]
                .into_iter()
                .map(String::from)
                .collect::<Vec<_>>()
        );
        // 直接拖入单个文件：相对路径即文件名
        let entries = collect_targets(
            &[root.join("sub/a.jpg").to_string_lossy().into_owned()],
            &[],
            8,
        )
        .unwrap();
        assert_eq!(entries[0].relative_path, "a.jpg");
    }
}
