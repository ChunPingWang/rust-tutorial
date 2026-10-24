//! 錯誤型別。用 thiserror 取代手寫的 Display 與 impl Error。

use thiserror::Error;

use crate::money::Money;

#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum LedgerError {
    #[error("帳戶 {0} 不存在")]
    AccountNotFound(u32),

    #[error("帳戶 {0} 已存在")]
    DuplicateAccount(u32),

    #[error("帳戶 {account} 餘額不足：有 {balance} 元，要 {requested} 元")]
    InsufficientFunds {
        account: u32,
        balance: Money,
        requested: Money,
    },

    #[error("金額不合法：{0} 元")]
    InvalidAmount(Money),

    #[error("不能轉帳給自己（帳戶 {0}）")]
    SameAccountTransfer(u32),

    #[error("金額溢位")]
    Overflow,
}
