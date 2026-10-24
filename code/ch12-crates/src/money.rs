//! 金額型別。以「分」為單位的 i64，內部欄位私有。

use std::fmt;
use std::iter::Sum;
use std::ops::{Add, AddAssign, Neg, Sub};

use serde::{Deserialize, Serialize};

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
/// 序列化成 JSON 時就是一個整數（分）：
///
/// ```
/// use ledger::Money;
/// assert_eq!(serde_json::to_string(&Money::from_yuan(1)).unwrap(), "100");
/// let m: Money = serde_json::from_str("250").unwrap();
/// assert_eq!(m, Money::from_cents(250));
/// ```
// serde(transparent)：序列化時當成內部的 i64，而不是 {"0": 100} 之類的包裝
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Default, Serialize, Deserialize,
)]
#[serde(transparent)]
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

/// 解析 "250.50"、"250"、"-3.07" 這類人類寫法。CSV 裡的金額就是這種格式。
impl std::str::FromStr for Money {
    type Err = ParseMoneyError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let s = s.trim();
        let (negative, s) = match s.strip_prefix('-') {
            Some(rest) => (true, rest),
            None => (false, s),
        };
        let (yuan_str, fen_str) = s.split_once('.').unwrap_or((s, ""));
        if yuan_str.is_empty() || fen_str.len() > 2 {
            return Err(ParseMoneyError(s.to_string()));
        }
        let yuan: i64 = yuan_str
            .parse()
            .map_err(|_| ParseMoneyError(s.to_string()))?;
        let fen: i64 = if fen_str.is_empty() {
            0
        } else {
            // "5" 代表 0.50，"05" 代表 0.05
            let padded = format!("{fen_str:0<2}");
            padded.parse().map_err(|_| ParseMoneyError(s.to_string()))?
        };
        let cents = yuan
            .checked_mul(100)
            .and_then(|y| y.checked_add(fen))
            .ok_or_else(|| ParseMoneyError(s.to_string()))?;
        Ok(Money(if negative { -cents } else { cents }))
    }
}

/// thiserror：#[error("...")] 自動產生 Display，derive(Error) 自動產生 impl std::error::Error
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[error("無法解析金額：{0:?}")]
pub struct ParseMoneyError(String);

impl fmt::Display for Money {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let sign = if self.0 < 0 { "-" } else { "" };
        let yuan = (self.0 / 100).abs();
        let fen = (self.0 % 100).abs();
        f.pad(&format!("{sign}{yuan}.{fen:02}"))
    }
}

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

impl Sum for Money {
    fn sum<I: Iterator<Item = Money>>(iter: I) -> Money {
        iter.fold(Money::ZERO, |acc, m| acc + m)
    }
}

impl<'a> Sum<&'a Money> for Money {
    fn sum<I: Iterator<Item = &'a Money>>(iter: I) -> Money {
        iter.fold(Money::ZERO, |acc, m| acc + *m)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn from_yuan_multiplies_by_100() {
        assert_eq!(Money::from_yuan(12).cents(), 1_200);
    }

    #[test]
    fn display_formats_with_two_decimals() {
        assert_eq!(Money::from_cents(5).to_string(), "0.05");
        assert_eq!(Money::from_cents(-1234).to_string(), "-12.34");
    }

    #[test]
    fn parses_human_formats() {
        assert_eq!(
            "250.50".parse::<Money>().unwrap(),
            Money::from_cents(25_050)
        );
        assert_eq!("250.5".parse::<Money>().unwrap(), Money::from_cents(25_050));
        assert_eq!("250".parse::<Money>().unwrap(), Money::from_cents(25_000));
        assert_eq!("-3.07".parse::<Money>().unwrap(), Money::from_cents(-307));
        assert_eq!(" 0.01 ".parse::<Money>().unwrap(), Money::from_cents(1));
    }

    #[test]
    fn rejects_bad_formats() {
        for bad in ["", "abc", "1.234", ".5", "1.2.3"] {
            assert!(bad.parse::<Money>().is_err(), "{bad:?} 應該解析失敗");
        }
    }

    #[test]
    fn display_and_parse_round_trip() {
        for cents in [0, 1, 99, 100, 12_345, -1, -12_345, i64::MAX / 100] {
            let m = Money::from_cents(cents);
            assert_eq!(m.to_string().parse::<Money>().unwrap(), m);
        }
    }

    #[test]
    fn checked_ops_detect_overflow() {
        assert!(Money::from_cents(i64::MAX)
            .checked_add(Money::from_cents(1))
            .is_none());
    }

    #[test]
    #[should_panic(expected = "Money 加法溢位")]
    fn add_operator_panics_on_overflow() {
        let _ = Money::from_cents(i64::MAX) + Money::from_cents(1);
    }

    #[test]
    fn json_is_plain_integer() {
        let json = serde_json::to_string(&Money::from_cents(-250)).unwrap();
        assert_eq!(json, "-250");
    }
}
