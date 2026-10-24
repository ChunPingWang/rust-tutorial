//! 交易。

use std::fmt;

use chrono::NaiveDate;
use serde::{Deserialize, Serialize};

use crate::money::Money;

/// 交易種類。
///
/// serde 屬性控制 JSON 長相：
/// `{"type": "transfer", "from": 1001, "to": 1002}` 而不是 `{"Transfer": {"from": ..}}`
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum TxKind {
    Deposit { account: u32 },
    Withdraw { account: u32 },
    Transfer { from: u32, to: u32 },
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Transaction {
    pub id: u64,
    /// 真正的日期型別了（chrono::NaiveDate：不帶時區的日期）
    pub date: NaiveDate,
    // flatten：把 kind 的欄位攤平到 Transaction 同一層，JSON 更好讀
    #[serde(flatten)]
    pub kind: TxKind,
    pub amount: Money,
    #[serde(default)] // 缺這個欄位就用預設值（空字串）
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
            "#{} {} {} {} 元（{}）",
            self.id, self.date, self.kind, self.amount, self.memo
        )
    }
}
