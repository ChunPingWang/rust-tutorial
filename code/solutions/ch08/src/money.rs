//! 金額型別。以「分」為單位的 i64，內部欄位私有，只能透過方法建立與讀取。

/// 金額（分）。
///
/// 內部的 `i64` 是私有的：crate 外部不能寫 `Money(100)`，
/// 必須用 [`Money::from_cents`] 或 [`Money::from_yuan`]。這讓我們能在建構時加入檢查。
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Default)]
pub struct Money(i64);

impl Money {
    pub const ZERO: Money = Money(0);

    /// 練習 3：加入下限檢查。
    ///
    /// 若直接把 from_cents 改成回傳 Result，所有呼叫端（ledger.rs、report、main、測試）都要加 `?` 或 unwrap，
    /// 在這個小專案裡約十處。這是「不變量集中在建構函式」的成本；好處是之後任何地方拿到 Money 都保證合法。
    /// 折衷做法：保留 from_cents 給「已知合法」的內部呼叫（違反時 panic，因為那是 bug），
    /// 另提供 try_from_cents 給外部輸入（CSV、API）使用。
    pub const MIN_CENTS: i64 = -1_000_000_000_000;

    pub fn from_cents(cents: i64) -> Money {
        Money::try_from_cents(cents).expect("內部建立的金額低於下限，這是程式 bug")
    }

    pub fn try_from_cents(cents: i64) -> Option<Money> {
        if cents < Money::MIN_CENTS {
            None
        } else {
            Some(Money(cents))
        }
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
