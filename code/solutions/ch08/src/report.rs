//! 報表：對帳本的唯讀統計查詢。
//!
//! 練習 2：改成新式佈局 report.rs + report/summary.rs + report/risk.rs，
//! 功能與 mod.rs 形式完全相同，只是 mod.rs 不再需要。

mod risk;
mod summary;

// 把子模組的公開項目提到 report:: 層級
pub use risk::rapid_withdrawals;
pub use summary::{daily_count, outflow_by_account, ranking, top_n, total_deposits};
