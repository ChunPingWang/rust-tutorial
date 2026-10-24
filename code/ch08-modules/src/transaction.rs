//! 交易。

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
    /// 當月第幾天（簡化的日期，第 12 章會換成真正的日期型別）
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

    pub fn describe(&self) -> String {
        let action = match self.kind {
            TxKind::Deposit { account } => format!("帳戶 {account} 存入"),
            TxKind::Withdraw { account } => format!("帳戶 {account} 提出"),
            TxKind::Transfer { from, to } => format!("帳戶 {from} 轉給 {to}"),
        };
        format!(
            "#{} {} {}（{}）",
            self.id,
            action,
            self.amount.format(),
            self.memo
        )
    }
}
