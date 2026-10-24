//! 報表：對帳本的唯讀統計查詢。
//!
//! 這個模組用目錄形式（report/mod.rs + report/summary.rs + report/risk.rs），
//! 示範模組可以再有子模組。

mod risk;
mod summary;

// 把子模組的公開項目提到 report:: 層級
pub use risk::rapid_withdrawals;
pub use summary::{daily_count, outflow_by_account, ranking, top_n, total_deposits};
