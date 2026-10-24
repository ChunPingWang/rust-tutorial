//! 金額型別。以「分」為單位的 i64，內部欄位私有，只能透過方法建立與讀取。

/// 金額（分）。
///
/// 內部的 `i64` 是私有的：crate 外部不能寫 `Money(100)`，
/// 必須用 [`Money::from_cents`] 或 [`Money::from_yuan`]。這讓我們能在建構時加入檢查。
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Default)]
pub struct Money(i64);

impl Money {
    pub const ZERO: Money = Money(0);

    pub fn from_cents(cents: i64) -> Money {
        Money(cents)
    }

    pub fn from_yuan(yuan: i64) -> Money {
        Money(yuan * 100)
    }

    pub fn cents(self) -> i64 {
        self.0
    }

    pub fn is_positive(self) -> bool {
        self.0 > 0
    }

    /// 溢位安全的加法。回傳 None 表示溢位。
    pub fn checked_add(self, other: Money) -> Option<Money> {
        self.0.checked_add(other.0).map(Money)
    }

    pub fn checked_sub(self, other: Money) -> Option<Money> {
        self.0.checked_sub(other.0).map(Money)
    }

    pub fn format(self) -> String {
        let sign = if self.0 < 0 { "-" } else { "" };
        let yuan = (self.0 / 100).abs();
        let fen = (self.0 % 100).abs();
        format!("{sign}{yuan}.{fen:02} 元")
    }
}
