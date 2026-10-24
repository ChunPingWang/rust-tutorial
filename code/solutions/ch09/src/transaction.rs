//! 交易。

use std::fmt;
use std::str::FromStr;

use crate::money::Money;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum TxKind {
    Deposit { account: u32 },
    Withdraw { account: u32 },
    Transfer { from: u32, to: u32 },
}

#[derive(Debug, Clone, PartialEq)]
pub struct Transaction {
    pub id: u64,
    pub day: u32,
    pub kind: TxKind,
    pub amount: Money,
    pub memo: String,
}

impl Transaction {
    pub fn affected_accounts(&self) -> Vec<u32> {
        match self.kind {
            TxKind::Deposit { account } | TxKind::Withdraw { account } => vec![account],
            TxKind::Transfer { from, to } => vec![from, to],
        }
    }

    pub fn involves(&self, account_id: u32) -> bool {
        self.affected_accounts().contains(&account_id)
    }
}

impl fmt::Display for TxKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            TxKind::Deposit { account } => write!(f, "帳戶 {account} 存入"),
            TxKind::Withdraw { account } => write!(f, "帳戶 {account} 提出"),
            TxKind::Transfer { from, to } => write!(f, "帳戶 {from} 轉給 {to}"),
        }
    }
}

impl fmt::Display for Transaction {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "#{} {} {} 元（{}）",
            self.id, self.kind, self.amount, self.memo
        )
    }
}

/// 練習 3：FromStr。格式 "D:1001"、"W:1001"、"X:1001:1002"。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParseTxKindError(pub String);

impl fmt::Display for ParseTxKindError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "無法解析交易種類：{:?}", self.0)
    }
}

impl std::error::Error for ParseTxKindError {}

impl FromStr for TxKind {
    type Err = ParseTxKindError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let err = || ParseTxKindError(s.to_string());
        let parts: Vec<&str> = s.split(':').collect();
        let id = |i: usize| -> Result<u32, ParseTxKindError> { parts.get(i).ok_or_else(err)?.parse().map_err(|_| err()) };
        match (parts.first().copied(), parts.len()) {
            (Some("D"), 2) => Ok(TxKind::Deposit { account: id(1)? }),
            (Some("W"), 2) => Ok(TxKind::Withdraw { account: id(1)? }),
            (Some("X"), 3) => Ok(TxKind::Transfer { from: id(1)?, to: id(2)? }),
            _ => Err(err()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_all_kinds() {
        assert_eq!("D:1001".parse::<TxKind>().unwrap(), TxKind::Deposit { account: 1001 });
        assert_eq!("W:7".parse::<TxKind>().unwrap(), TxKind::Withdraw { account: 7 });
        assert_eq!("X:1:2".parse::<TxKind>().unwrap(), TxKind::Transfer { from: 1, to: 2 });
    }

    #[test]
    fn rejects_bad_input() {
        for bad in ["", "D", "D:abc", "X:1", "Q:1", "D:1:2"] {
            assert!(bad.parse::<TxKind>().is_err(), "{bad:?}");
        }
    }
}
