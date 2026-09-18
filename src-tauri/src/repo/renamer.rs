//! 重命名执行：校验目标名后调用 fs::rename。

use std::path::Path;

use crate::types::{AppError, Result};

/// 重命名文件：`dir/from` → `dir/to`。
/// 失败时返回带上下文与修复提示的错误。
pub fn rename(dir: &Path, from: &str, to: &str) -> Result<()> {
    let src = dir.join(from);
    let dst = dir.join(to);
    if dst.exists() {
        return Err(AppError::fs(format!(
            "目标文件已存在：{}（请先清理冲突或使用去重选项）",
            dst.display()
        )));
    }
    if !src.exists() {
        return Err(AppError::fs(format!(
            "源文件不存在：{}（可能已被移动或删除）",
            src.display()
        )));
    }
    std::fs::rename(&src, &dst).map_err(|e| {
        AppError::fs(format!(
            "重命名失败 {} → {}：{e}（检查文件是否被占用或权限不足）",
            src.display(),
            dst.display()
        ))
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn renames_file() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("a.jpg"), b"x").unwrap();
        rename(dir.path(), "a.jpg", "b.jpg").unwrap();
        assert!(dir.path().join("b.jpg").exists());
        assert!(!dir.path().join("a.jpg").exists());
    }

    #[test]
    fn rename_fails_when_target_exists() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("a.jpg"), b"x").unwrap();
        std::fs::write(dir.path().join("b.jpg"), b"y").unwrap();
        assert!(rename(dir.path(), "a.jpg", "b.jpg").is_err());
    }

    #[test]
    fn rename_fails_when_source_missing() {
        let dir = tempfile::tempdir().unwrap();
        assert!(rename(dir.path(), "ghost.jpg", "b.jpg").is_err());
    }
}
