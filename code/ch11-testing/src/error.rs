//! 錯誤型別。

use crate::money::Money;
use std::fmt;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LedgerError {
    AccountNotFound(u32),
    DuplicateAccount(u32),
    InsufficientFunds {
        account: u32,
        balance: Money,
        requested: Money,
    },
    InvalidAmount(Money),
    SameAccountTransfer(u32),
    Overflow,
}

impl fmt::Display for LedgerError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            LedgerError::AccountNotFound(id) => write!(f, "帳戶 {id} 不存在"),
            LedgerError::DuplicateAccount(id) => write!(f, "帳戶 {id} 已存在"),
            LedgerError::InsufficientFunds {
                account,
                balance,
                requested,
            } => write!(
                f,
                "帳戶 {account} 餘額不足：有 {balance} 元，要 {requested} 元"
            ),
            LedgerError::InvalidAmount(m) => write!(f, "金額不合法：{m} 元"),
            LedgerError::SameAccountTransfer(id) => write!(f, "不能轉帳給自己（帳戶 {id}）"),
            LedgerError::Overflow => write!(f, "金額溢位"),
        }
    }
}

impl std::error::Error for LedgerError {}
