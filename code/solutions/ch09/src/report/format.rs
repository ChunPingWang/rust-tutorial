//! 報表輸出格式：同一份資料，多種呈現。

use crate::report::{ranking, total_deposits};
use crate::Ledger;

/// 報表格式要能做的事：把帳本渲染成字串。
pub trait ReportFormat {
    fn render(&self, ledger: &Ledger) -> String;
    fn file_extension(&self) -> &'static str;
}

/// 純文字摘要（給主管看）
pub struct TextReport;

impl ReportFormat for TextReport {
    fn render(&self, ledger: &Ledger) -> String {
        let mut out = String::new();
        out.push_str(&format!("總餘額：{} 元\n", ledger.total_balance()));
        out.push_str(&format!("總存款：{} 元\n", total_deposits(ledger)));
        out.push_str(&format!("交易筆數：{}\n", ledger.history().len()));
        out.push_str("餘額排名：\n");
        for (i, (id, owner, bal)) in ranking(ledger).iter().enumerate() {
            out.push_str(&format!("  {}. #{id} {owner:<6} {bal:>12} 元\n", i + 1));
        }
        out
    }

    fn file_extension(&self) -> &'static str {
        "txt"
    }
}

/// CSV（給財務匯入試算表）
pub struct CsvReport {
    pub with_header: bool,
}

impl ReportFormat for CsvReport {
    fn render(&self, ledger: &Ledger) -> String {
        let mut out = String::new();
        if self.with_header {
            out.push_str("account_id,owner,balance_cents\n");
        }
        for (id, owner, bal) in ranking(ledger) {
            out.push_str(&format!("{id},{owner},{}\n", bal.cents()));
        }
        out
    }

    fn file_extension(&self) -> &'static str {
        "csv"
    }
}

/// 練習 1：JSON 格式（手工拼字串；第 12 章換 serde）
pub struct JsonReport;

impl ReportFormat for JsonReport {
    fn render(&self, ledger: &Ledger) -> String {
        let items: Vec<String> = ranking(ledger)
            .iter()
            .map(|(id, owner, bal)| {
                format!(
                    "{{\"account_id\":{id},\"owner\":{owner:?},\"balance_cents\":{}}}",
                    bal.cents()
                )
            })
            .collect();
        format!(
            "{{\"total_cents\":{},\"accounts\":[{}]}}\n",
            ledger.total_balance().cents(),
            items.join(",")
        )
    }

    fn file_extension(&self) -> &'static str {
        "json"
    }
}

/// 泛型函式：接受任何 ReportFormat。編譯期單態化（monomorphization），零成本。
pub fn render_to_string<R: ReportFormat>(format: &R, ledger: &Ledger) -> String {
    format.render(ledger)
}

/// 同一件事用 impl Trait 寫法，常見於參數只用一次的情況
pub fn file_name(format: &impl ReportFormat, base: &str) -> String {
    format!("{base}.{}", format.file_extension())
}

/// trait object：執行期才知道是哪一種格式（例如由使用者設定決定），用 dyn
pub fn render_all(formats: &[Box<dyn ReportFormat>], ledger: &Ledger) -> Vec<String> {
    formats.iter().map(|f| f.render(ledger)).collect()
}
