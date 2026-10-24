//! 練習 4：Cow —— 輸入已經乾淨時不配置。

use std::borrow::Cow;

/// 去掉前後空白、連續空白合併成一個。乾淨的輸入回傳 Borrowed，零配置。
pub fn normalize_memo(memo: &str) -> Cow<'_, str> {
    let trimmed = memo.trim();
    let has_double_space = trimmed.split(' ').any(|w| w.is_empty());
    if trimmed.len() == memo.len() && (trimmed.is_empty() || !has_double_space) {
        return Cow::Borrowed(memo); // 已經乾淨
    }
    Cow::Owned(trimmed.split_whitespace().collect::<Vec<_>>().join(" "))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn clean_input_is_borrowed() {
        let r = normalize_memo("房租 十月");
        assert!(matches!(r, Cow::Borrowed(_)));
        assert_eq!(r, "房租 十月");
    }

    #[test]
    fn dirty_input_is_owned_and_normalized() {
        let r = normalize_memo("  房租   十月  ");
        assert!(matches!(r, Cow::Owned(_)));
        assert_eq!(r, "房租 十月");
    }

    #[test]
    fn empty_and_single_word() {
        assert!(matches!(normalize_memo(""), Cow::Borrowed(_)));
        assert!(matches!(normalize_memo(" a "), Cow::Owned(_)));
    }
}
