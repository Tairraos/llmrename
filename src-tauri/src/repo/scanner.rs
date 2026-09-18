//! 目录扫描：按扩展名过滤、自然排序。

use std::path::Path;

use crate::repo::sort;
use crate::types::{AppError, AssetEntry, Result};

/// 扫描目录下（非递归）匹配扩展名的文件，按文件名自然排序（photo2 < photo10）。
pub fn scan(dir: &Path, extensions: &[String]) -> Result<Vec<AssetEntry>> {
    if !dir.is_dir() {
        return Err(AppError::dir(format!("目录不存在：{}", dir.display())));
    }
    let lower: Vec<String> = extensions.iter().map(|e| e.to_lowercase()).collect();

    let mut entries = Vec::new();
    let rd = std::fs::read_dir(dir)
        .map_err(|e| AppError::dir(format!("无法读取目录 {}：{e}", dir.display())))?;
    for ent in rd {
        let ent = match ent {
            Ok(e) => e,
            Err(_) => continue,
        };
        let ft = match ent.file_type() {
            Ok(t) => t,
            Err(_) => continue,
        };
        if !ft.is_file() {
            continue;
        }
        let name = ent.file_name().to_string_lossy().into_owned();
        if !lower.is_empty() && !has_ext(&name, &lower) {
            continue;
        }
        let meta = match ent.metadata() {
            Ok(m) => m,
            Err(_) => continue,
        };
        entries.push(AssetEntry {
            path: ent.path().to_string_lossy().into_owned(),
            filename: name,
            size_bytes: meta.len(),
            modified_secs: meta
                .modified()
                .ok()
                .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
                .map(|d| d.as_secs() as i64),
        });
    }
    entries.sort_by(|a, b| sort::natural_cmp(&a.filename, &b.filename));
    Ok(entries)
}

fn has_ext(name: &str, extensions: &[String]) -> bool {
    name.rsplit_once('.')
        .map(|(_, e)| extensions.iter().any(|x| x == &e.to_lowercase()))
        .unwrap_or(false)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn scan_filters_and_sorts_naturally() {
        let dir = tempfile::tempdir().unwrap();
        for name in ["photo10.jpg", "photo2.png", "note.txt", "photo1.JPG"] {
            std::fs::write(dir.path().join(name), b"x").unwrap();
        }
        let exts = vec!["jpg".to_string(), "png".to_string()];
        let found = scan(dir.path(), &exts).unwrap();
        let names: Vec<_> = found.iter().map(|a| a.filename.as_str()).collect();
        assert_eq!(names, vec!["photo1.JPG", "photo2.png", "photo10.jpg"]);
    }

    #[test]
    fn scan_missing_dir_is_error() {
        let r = scan(Path::new("/nonexistent/definitely/not/here"), &[]);
        assert!(r.is_err());
    }

    #[test]
    fn ext_extraction() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("IMG_0420.JPG"), b"x").unwrap();
        let found = scan(dir.path(), &["jpg".into()]).unwrap();
        assert_eq!(found[0].ext(), "jpg");
        let p = PathBuf::from("noext");
        assert_eq!(
            p.extension().map(|e| e.to_string_lossy().into_owned()),
            None
        );
    }
}
