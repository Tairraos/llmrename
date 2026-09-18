//! JSONL 日志：追加写入重命名记录。

use std::io::Write;
use std::path::Path;

use crate::types::{AppError, LogEntry, Result};

pub const LOG_FILE: &str = "rename_log.jsonl";

/// 追加一条日志（单行 JSON）。
pub fn append(dir: &Path, entry: &LogEntry) -> Result<()> {
    let path = dir.join(LOG_FILE);
    let mut line = serde_json::to_string(entry)
        .map_err(|e| AppError::internal(format!("序列化日志失败：{e}")))?;
    line.push('\n');
    let mut f = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(&path)
        .map_err(|e| AppError::fs(format!("打开日志失败：{e}（{}）", path.display())))?;
    f.write_all(line.as_bytes())
        .map_err(|e| AppError::fs(format!("追加日志失败：{e}（{}）", path.display())))
}

/// 读取最近的 N 条（最新在前）。文件不存在返回空。
pub fn recent(dir: &Path, limit: usize) -> Result<Vec<LogEntry>> {
    let path = dir.join(LOG_FILE);
    if !path.exists() {
        return Ok(Vec::new());
    }
    let raw = std::fs::read_to_string(&path)
        .map_err(|e| AppError::fs(format!("读取日志失败：{e}（{}）", path.display())))?;
    let mut out: Vec<LogEntry> = Vec::new();
    for line in raw.lines() {
        if line.trim().is_empty() {
            continue;
        }
        match serde_json::from_str::<LogEntry>(line) {
            Ok(e) => out.push(e),
            Err(_) => {
                // 单行损坏不阻断整体
            }
        }
    }
    out.reverse();
    if out.len() > limit {
        out.truncate(limit);
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample() -> LogEntry {
        LogEntry::ok(
            "a.jpg".into(),
            "b.jpg".into(),
            Some("{\"date\":\"2026-09-18\"}".into()),
            Some("b".into()),
        )
    }

    #[test]
    fn append_and_read_recent() {
        let dir = tempfile::tempdir().unwrap();
        append(dir.path(), &sample()).unwrap();
        append(
            dir.path(),
            &LogEntry::failed("c.jpg".into(), "模型调用失败".into()),
        )
        .unwrap();
        let all = recent(dir.path(), 100).unwrap();
        assert_eq!(all.len(), 2);
        // 最新在前
        assert_eq!(all[0].asset, "c.jpg");
        assert_eq!(all[1].asset, "a.jpg");
    }

    #[test]
    fn missing_log_returns_empty() {
        let dir = tempfile::tempdir().unwrap();
        assert!(recent(dir.path(), 10).unwrap().is_empty());
    }

    #[test]
    fn limit_applies_after_reverse() {
        let dir = tempfile::tempdir().unwrap();
        for _ in 0..5 {
            append(dir.path(), &sample()).unwrap();
        }
        let got = recent(dir.path(), 2).unwrap();
        assert_eq!(got.len(), 2);
    }
}
