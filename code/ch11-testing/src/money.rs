//! 金額型別。以「分」為單位的 i64，內部欄位私有。

use std::fmt;
use std::iter::Sum;
use std::ops::{Add, AddAssign, Neg, Sub};

/// 金額（分）。
///
/// # 範例
///
/// ```
/// use ledger::Money;
///
/// let a = Money::from_yuan(100);
/// let b = Money::from_cents(50);
/// assert_eq!((a + b).cents(), 10_050);
/// assert_eq!(format!("{}", a - b), "99.50");
/// ```
///
/// 溢位時 `checked_add` 回傳 `None`：
///
/// ```
/// use ledger::Money;
/// assert_eq!(Money::from_cents(i64::MAX).checked_add(Money::from_cents(1)), None);
/// ```
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

    pub fn checked_add(self, other: Money) -> Option<Money> {
        self.0.checked_add(other.0).map(Money)
    }

    pub fn checked_sub(self, other: Money) -> Option<Money> {
        self.0.checked_sub(other.0).map(Money)
    }
}

/// 給人看的格式。有了 Display，就能 `println!("{money}")`、`money.to_string()`。
impl fmt::Display for Money {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let sign = if self.0 < 0 { "-" } else { "" };
        let yuan = (self.0 / 100).abs();
        let fen = (self.0 % 100).abs();
        // f.pad 讓 {:>12} 這類寬度/對齊設定也能作用在我們的型別上
        f.pad(&format!("{sign}{yuan}.{fen:02}"))
    }
}

/// 運算子重載：`a + b`。
///
/// 設計決策：溢位時 panic。理由：Money 的加法用在報表加總與測試，
/// 溢位代表資料已經壞掉，屬於程式 bug。帳本的核心過帳邏輯仍然用 checked_add 回傳錯誤。
impl Add for Money {
    type Output = Money;
    fn add(self, rhs: Money) -> Money {
        self.checked_add(rhs).expect("Money 加法溢位")
    }
}

impl AddAssign for Money {
    fn add_assign(&mut self, rhs: Money) {
        *self = *self + rhs;
    }
}

impl Sub for Money {
    type Output = Money;
    fn sub(self, rhs: Money) -> Money {
        self.checked_sub(rhs).expect("Money 減法溢位")
    }
}

impl Neg for Money {
    type Output = Money;
    fn neg(self) -> Money {
        Money(-self.0)
    }
}

/// 讓迭代器可以 `.sum::<Money>()`
impl Sum for Money {
    fn sum<I: Iterator<Item = Money>>(iter: I) -> Money {
        iter.fold(Money::ZERO, |acc, m| acc + m)
    }
}

/// 也支援對 &Money 的迭代器 sum（例如 `accounts.iter().map(|a| &a.balance).sum()`）
impl<'a> Sum<&'a Money> for Money {
    fn sum<I: Iterator<Item = &'a Money>>(iter: I) -> Money {
        iter.fold(Money::ZERO, |acc, m| acc + *m)
    }
}

// 單元測試：和被測程式碼放在同一個檔案，可以存取私有項目。
// #[cfg(test)] 讓這個模組只在 cargo test 時編譯，不會進到正式的執行檔。
#[cfg(test)]
mod tests {
    use super::*; // 把上面所有東西（包含私有的）帶進來

    #[test]
    fn from_yuan_multiplies_by_100() {
        assert_eq!(Money::from_yuan(12).cents(), 1_200);
        assert_eq!(Money::from_yuan(-3).cents(), -300);
    }

    #[test]
    fn display_formats_with_two_decimals() {
        assert_eq!(Money::from_cents(5).to_string(), "0.05");
        assert_eq!(Money::from_cents(-1234).to_string(), "-12.34");
        assert_eq!(format!("[{:>8}]", Money::from_cents(100)), "[    1.00]");
    }

    #[test]
    fn checked_ops_detect_overflow() {
        let max = Money::from_cents(i64::MAX);
        assert!(max.checked_add(Money::from_cents(1)).is_none());
        assert!(Money::from_cents(i64::MIN)
            .checked_sub(Money::from_cents(1))
            .is_none());
        assert_eq!(max.checked_add(Money::ZERO), Some(max));
    }

    #[test]
    #[should_panic(expected = "Money 加法溢位")]
    fn add_operator_panics_on_overflow() {
        let _ = Money::from_cents(i64::MAX) + Money::from_cents(1);
    }

    #[test]
    fn sum_of_iterator() {
        let v = vec![
            Money::from_cents(1),
            Money::from_cents(2),
            Money::from_cents(3),
        ];
        let owned: Money = v.iter().copied().sum();
        let borrowed: Money = v.iter().sum();
        assert_eq!(owned, Money::from_cents(6));
        assert_eq!(borrowed, owned);
        let empty: Money = Vec::<Money>::new().into_iter().sum();
        assert_eq!(empty, Money::ZERO);
    }

    // 私有欄位可以在這裡直接碰：單元測試的特權
    #[test]
    fn internal_representation_is_cents() {
        assert_eq!(Money::from_yuan(1).0, 100);
    }
}
