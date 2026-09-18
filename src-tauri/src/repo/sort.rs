//! 字符串自然排序：数字块按数值比较，其余按字典序。
//! 供 scanner / targets 等列表展示复用。

/// 自然排序：逐块比较（连续数字按数值，其余文本按字典序）。
/// 例：photo2 < photo10，a2b < a10c。
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

    #[test]
    fn natural_compare() {
        assert!(natural_cmp("a2", "a10") == std::cmp::Ordering::Less);
        assert!(natural_cmp("a10", "a2") == std::cmp::Ordering::Greater);
        assert!(natural_cmp("a2b", "a2a") == std::cmp::Ordering::Greater);
        assert!(natural_cmp("photo10.jpg", "photo2.png") == std::cmp::Ordering::Greater);
        assert!(natural_cmp("a", "b") == std::cmp::Ordering::Less);
    }
}
