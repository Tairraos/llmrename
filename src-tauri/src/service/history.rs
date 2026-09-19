//! 应用内重命名历史（undo/redo）。
//!
//! 设计约束（产品要求）：
//! - 不退出 app 时，每个文件保留最近 10 条历史版本，可任意回溯/重做
//! - 历史只存内存（退出即清）
//! - 一个「op」记录一次文件系统 rename：from → to
//!
//! 文件身份的稳定性：同一文件的连续改名（a→b→c→d）通过「op.to == 上次 op.from」
//! 归并为同一个 id，因此无论操作被多少其它文件的操作穿插，每个文件都独立计数。

use std::path::{Path, PathBuf};

use crate::types::HistoryStatus;

#[derive(Debug, Clone, PartialEq, Eq)]
struct HistoryOp {
    /// 文件身份：同一文件的连续改名链共享同一个 id
    id: u64,
    /// 操作前的绝对路径
    from: PathBuf,
    /// 操作后的绝对路径
    to: PathBuf,
}

/// undo/redo 历史存储。
///
/// `undo` 栈顶是最近一次操作；`redo` 栈保存被撤销的操作。
/// 每文件最多保留 `cap` 条：超限时丢弃该文件最旧的记录。
#[derive(Debug)]
pub struct HistoryStore {
    undo: Vec<HistoryOp>,
    redo: Vec<HistoryOp>,
    cap: usize,
    next_id: u64,
    /// 当前撤销/重做聚焦的文件 id：撤销延续同一文件的链，而不是跨文件交错
    focus: Option<u64>,
}

impl Default for HistoryStore {
    fn default() -> Self {
        Self::new(10)
    }
}

impl HistoryStore {
    pub fn new(cap: usize) -> Self {
        Self {
            undo: Vec::new(),
            redo: Vec::new(),
            cap: cap.max(1),
            next_id: 1,
            focus: None,
        }
    }

    /// 记录一次重命名（from → to）。
    ///
    /// - 若栈中存在 to == from 的 op（同一文件继续改名），继承其 id；否则分配新 id
    /// - 新操作入 undo 栈顶，清空 redo（新分支）
    /// - 若该文件记录数超过 cap，丢弃其最旧的记录
    pub fn record(&mut self, from: &Path, to: &Path) {
        let id = self
            .undo
            .iter()
            .rev()
            .find(|op| op.to == from)
            .map(|op| op.id)
            .unwrap_or_else(|| {
                let id = self.next_id;
                self.next_id += 1;
                id
            });

        self.undo.push(HistoryOp {
            id,
            from: from.to_path_buf(),
            to: to.to_path_buf(),
        });
        self.redo.clear();
        self.focus = None;

        // 该文件记录数
        let count = self.undo.iter().filter(|op| op.id == id).count();
        let excess = count.saturating_sub(self.cap);
        if excess > 0 {
            // 去掉该文件最旧的 excess 条（保留其他文件顺序）
            let mut removed = 0usize;
            let mut kept = Vec::with_capacity(self.undo.len() - excess);
            for op in self.undo.iter() {
                if op.id == id && removed < excess {
                    removed += 1;
                    continue;
                }
                kept.push(op.clone());
            }
            self.undo = kept;
        }
    }

    /// 撤销最近一次操作：返回 (需改名的两个路径 to, from)，调用方执行 rename(to → from)。
    ///
    /// 撤销是「按文件」的：一旦开始撤销某个文件，后续撤销会延续同一文件的链
    /// （该文件的所有历史版本回溯完之后，才轮到其它文件最近的操作）。
    pub fn undo(&mut self) -> Option<(PathBuf, PathBuf)> {
        let len = self.undo.len();
        if len == 0 {
            return None;
        }
        let pos = self
            .focus
            .and_then(|id| self.undo.iter().rposition(|op| op.id == id))
            .unwrap_or(len - 1);
        let op = self.undo.remove(pos);
        self.focus = Some(op.id);
        self.redo.push(op.clone());
        Some((op.to, op.from))
    }

    /// 重做最近一次撤销：返回 (from, to)，调用方执行 rename(from → to)。
    /// 与撤销对称：重做也按文件连续进行。
    pub fn redo(&mut self) -> Option<(PathBuf, PathBuf)> {
        let op = self.redo.pop()?;
        self.focus = Some(op.id);
        self.undo.push(op.clone());
        Some((op.from, op.to))
    }

    /// 撤销/重做/记录后返回给前端的状态。
    pub fn status(&self) -> HistoryStatus {
        let applied = self.undo.last().map(|op| {
            format!(
                "{} → {}",
                op.from
                    .file_name()
                    .map(|f| f.to_string_lossy().into_owned())
                    .unwrap_or_default(),
                op.to
                    .file_name()
                    .map(|f| f.to_string_lossy().into_owned())
                    .unwrap_or_default()
            )
        });
        HistoryStatus {
            undoable: self.undo.len(),
            redoable: self.redo.len(),
            applied,
        }
    }

    pub fn undo_len(&self) -> usize {
        self.undo.len()
    }
    pub fn redo_len(&self) -> usize {
        self.redo.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn p(s: &str) -> PathBuf {
        PathBuf::from(format!("/tmp/{s}"))
    }

    #[test]
    fn record_and_undo_redo() {
        let mut h = HistoryStore::default();
        h.record(&p("a.jpg"), &p("b.jpg"));
        assert_eq!(h.undo_len(), 1);
        let (from, to) = h.undo().unwrap();
        assert_eq!(from, p("b.jpg"));
        assert_eq!(to, p("a.jpg"));
        assert_eq!(h.redo_len(), 1);
        let (f2, t2) = h.redo().unwrap();
        assert_eq!(f2, p("a.jpg"));
        assert_eq!(t2, p("b.jpg"));
        assert_eq!(h.undo_len(), 1);
        assert_eq!(h.redo_len(), 0);
    }

    #[test]
    fn status_counts_and_summary() {
        let mut h = HistoryStore::default();
        h.record(&p("a.jpg"), &p("b.jpg"));
        h.record(&p("c.jpg"), &p("d.jpg"));
        let st = h.status();
        assert_eq!(st.undoable, 2);
        assert_eq!(st.redoable, 0);
        assert_eq!(st.applied.as_deref(), Some("c.jpg → d.jpg"));
    }

    #[test]
    fn new_record_clears_redo() {
        let mut h = HistoryStore::default();
        h.record(&p("a.jpg"), &p("b.jpg"));
        h.undo();
        assert_eq!(h.redo_len(), 1);
        h.record(&p("x.jpg"), &p("y.jpg"));
        assert_eq!(h.redo_len(), 0);
    }

    #[test]
    fn per_file_cap_keeps_10() {
        let mut h = HistoryStore::default();
        for i in 0..15 {
            h.record(&p(&format!("f{i}.jpg")), &p(&format!("f{}.jpg", i + 1)));
        }
        // 15 次连续改名 -> 只保留最近 10 条
        assert_eq!(h.undo_len(), 10);
        let st = h.status();
        assert_eq!(st.applied.as_deref(), Some("f14.jpg → f15.jpg"));
    }

    #[test]
    fn mixed_files_each_capped_independently() {
        let mut h = HistoryStore::default();
        // a 文件连续改名 12 次，中间穿插 b 文件 3 次
        for i in 0..12 {
            h.record(&p(&format!("a{i}.jpg")), &p(&format!("a{}.jpg", i + 1)));
            if i < 3 {
                h.record(&p(&format!("b{i}.jpg")), &p(&format!("b{}.jpg", i + 1)));
            }
        }
        // a 链被 cap 到 10，b 完整 3 条 -> 共 13
        assert_eq!(h.undo_len(), 13);

        // 只撤销，a 链的旧记录不该复现
        let mut undone_a0 = 0;
        while let Some((_, to)) = h.undo() {
            if to == p("a0.jpg") {
                undone_a0 += 1;
            }
        }
        assert_eq!(undone_a0, 0, "被裁掉的 a 最旧记录不应再被撤销到");
    }

    #[test]
    fn undo_continues_same_file_before_others() {
        let mut h = HistoryStore::default();
        h.record(&p("a.jpg"), &p("a1.jpg"));
        h.record(&p("b.jpg"), &p("b1.jpg"));
        h.record(&p("a1.jpg"), &p("a2.jpg"));

        // 第一次撤销：最近的一次（a1→a2）
        let (f, t) = h.undo().unwrap();
        assert_eq!((f, t), (p("a2.jpg"), p("a1.jpg")));
        // 第二次撤销：延续 a 文件的链（a→a1），而不是 b 的记录
        let (f, t) = h.undo().unwrap();
        assert_eq!((f, t), (p("a1.jpg"), p("a.jpg")));
        // 第三次：a 链已回溯完，才轮到 b
        let (f, t) = h.undo().unwrap();
        assert_eq!((f, t), (p("b1.jpg"), p("b.jpg")));
    }

    #[test]
    fn redo_symmetric_per_file() {
        let mut h = HistoryStore::default();
        h.record(&p("a.jpg"), &p("a1.jpg"));
        h.record(&p("b.jpg"), &p("b1.jpg"));
        h.record(&p("a1.jpg"), &p("a2.jpg"));

        h.undo().unwrap();
        h.undo().unwrap();
        // 重做也按文件：先 a2←，再 a1←
        let (f, t) = h.redo().unwrap();
        assert_eq!((f, t), (p("a.jpg"), p("a1.jpg")));
        let (f, t) = h.redo().unwrap();
        assert_eq!((f, t), (p("a1.jpg"), p("a2.jpg")));
    }

    #[test]
    fn redo_after_partial_undo() {
        let mut h = HistoryStore::default();
        h.record(&p("a.jpg"), &p("b.jpg"));
        h.record(&p("b.jpg"), &p("c.jpg"));
        assert_eq!(h.undo_len(), 2);

        let (f1, t1) = h.undo().unwrap();
        assert_eq!((f1, t1), (p("c.jpg"), p("b.jpg")));
        let (f2, t2) = h.undo().unwrap();
        assert_eq!((f2, t2), (p("b.jpg"), p("a.jpg")));

        // 任意回溯后再重做
        let (f3, t3) = h.redo().unwrap();
        assert_eq!((f3, t3), (p("a.jpg"), p("b.jpg")));
    }
}
