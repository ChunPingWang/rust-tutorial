//! 交易。

use std::fmt;

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
