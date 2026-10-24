//! # ledger
//!
//! 小額支付帳本的核心邏輯。這是一個 library crate，
//! 不含任何 I/O 或命令列處理，那些在 `main.rs`。
//!
//! 模組結構：
//! - [`money`]：金額型別
//! - [`account`]：帳戶
//! - [`transaction`]：交易與交易種類
//! - [`error`]：錯誤型別
//! - [`ledger`]：帳本（核心狀態與過帳邏輯）
//! - [`report`]：報表（唯讀的統計查詢與輸出格式）
//! - [`storage`]：儲存後端（記憶體、文字檔）

// 宣告子模組。`mod money;` 會去找 src/money.rs 或 src/money/mod.rs
pub mod account;
pub mod error;
pub mod ledger;
pub mod money;
pub mod report;
pub mod storage;
pub mod transaction;

// 重新匯出（re-export）：讓使用者寫 `ledger::Money` 而不用 `ledger::money::Money`
pub use account::Account;
pub use error::LedgerError;
pub use ledger::Ledger;
pub use money::Money;
pub use storage::{save_and_verify, InMemoryStorage, Storage, StorageError, TextFileStorage};
pub use transaction::{Transaction, TxKind};
