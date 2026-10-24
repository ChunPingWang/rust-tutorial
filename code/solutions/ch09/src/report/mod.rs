//! 報表：對帳本的唯讀統計查詢，以及多種輸出格式。

mod format;
mod risk;
mod summary;

pub use format::{
    file_name, render_all, render_to_string, CsvReport, JsonReport, ReportFormat, TextReport,
};
pub use risk::rapid_withdrawals;
pub use summary::{daily_count, outflow_by_account, ranking, top_n, total_deposits};
