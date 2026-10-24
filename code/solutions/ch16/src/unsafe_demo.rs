//! 第 16 章：unsafe 的邊界。
//!
//! 這個模組刻意很小。帳本系統**不需要** unsafe，這裡只是讓你看過一次它長什麼樣、
//! 以及「安全抽象」是什麼意思。

/// 把一個 `&[u8]` 重新詮釋成 `&[u32]`（例如讀取二進位格式的餘額陣列）。
///
/// 這是 unsafe 的典型用途：做一件編譯器無法證明安全、但你能證明的事。
/// 整個 unsafe 區塊被包在一個**安全的函式**裡：函式自己檢查前提，呼叫者不需要 unsafe。
///
/// # 安全性
/// - 長度必須是 4 的倍數，否則回傳 None
/// - 位址必須對齊 4 bytes，否則回傳 None
/// - 任何 4 個 bytes 都是合法的 u32（沒有無效位元模式），所以詮釋是安全的
pub fn bytes_as_u32(bytes: &[u8]) -> Option<&[u32]> {
    if !bytes.len().is_multiple_of(4) {
        return None;
    }
    if bytes.as_ptr().align_offset(std::mem::align_of::<u32>()) != 0 {
        return None;
    }
    // SAFETY: 上面檢查了長度與對齊；u32 沒有無效位元模式；借用規則由輸入的 &[u8] 延續。
    // （每個 unsafe 區塊都應該有一段 SAFETY 註解，說明為什麼它是安全的。這是社群慣例，clippy 可以強制。）
    let slice =
        unsafe { std::slice::from_raw_parts(bytes.as_ptr().cast::<u32>(), bytes.len() / 4) };
    Some(slice)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reinterprets_aligned_bytes() {
        // 用 u32 陣列保證對齊，再借成 bytes
        let source: [u32; 2] = [0x0100_0000u32.to_le(), 7u32.to_le()];
        let bytes: &[u8] = unsafe { std::slice::from_raw_parts(source.as_ptr().cast::<u8>(), 8) };
        let back = bytes_as_u32(bytes).unwrap();
        assert_eq!(back, &source);
    }

    #[test]
    fn rejects_bad_length() {
        assert!(bytes_as_u32(&[1, 2, 3]).is_none());
    }
}
