//! 目录扫描：按扩展名过滤、自然排序。

use std::path::Path;

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
    entries.sort_by(|a, b| natural_cmp(&a.filename, &b.filename));
    Ok(entries)
}

fn has_ext(name: &str, extensions: &[String]) -> bool {
    name.rsplit_once('.')
        .map(|(_, e)| extensions.iter().any(|x| x == &e.to_lowercase()))
        .unwrap_or(false)
}

/// 自然排序：逐块比较（连续数字按数值，其余文本按字典序）。
/// 例：photo2 < photo10，a2b < a10c。
fn natural_cmp(a: &str, b: &str) -> std::cmp::Ordering {
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

/// 从 pos 开始的一串连续数字块：返回 (去前导零后的数值, 块长度)。
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
    fn natural_compare() {
        assert!(natural_cmp("a2", "a10") == std::cmp::Ordering::Less);
        assert!(natural_cmp("a10", "a2") == std::cmp::Ordering::Greater);
        assert!(natural_cmp("a2b", "a2a") == std::cmp::Ordering::Greater);
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
