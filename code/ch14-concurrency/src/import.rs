//! 從 CSV 匯入帳戶與交易。
//!
//! 這一層負責「把外面的髒資料變成乾淨的型別」。它和核心邏輯分開：
//! Ledger 永遠只接受合法的 Transaction，不知道 CSV 的存在。

use std::io::Read;
use std::path::Path;

use chrono::NaiveDate;
use serde::Deserialize;
use thiserror::Error;

use crate::{Money, Transaction, TxKind};

#[derive(Debug, Error)]
pub enum ImportError {
    #[error("讀取 {path} 失敗：{source}")]
    Io {
        path: String,
        #[source]
        source: std::io::Error,
    },

    #[error("CSV 格式錯誤：{0}")]
    Csv(#[from] csv::Error),

    #[error("第 {line} 行：{reason}")]
    Row { line: usize, reason: String },
}

/// CSV 裡的一行帳戶。欄位名稱直接對應 CSV 標頭。
#[derive(Debug, Deserialize)]
pub struct AccountRow {
    pub id: u32,
    pub owner: String,
    /// CSV 裡寫 "50000.00"，用 Money 的 FromStr 解析。
    /// serde 預設不會呼叫 FromStr，所以用 deserialize_with 指定。
    #[serde(deserialize_with = "money_from_str")]
    pub balance: Money,
}

/// CSV 裡的一行交易：外部格式，欄位是平的、可能為空。
/// 這是「輸入用的中介型別」，不是我們的核心 Transaction。
#[derive(Debug, Deserialize)]
struct TxRow {
    id: u64,
    date: NaiveDate,
    #[serde(rename = "type")] // CSV 標頭叫 type，但 type 是 Rust 關鍵字
    kind: String,
    #[serde(default)]
    account: Option<u32>,
    #[serde(default)]
    from: Option<u32>,
    #[serde(default)]
    to: Option<u32>,
    #[serde(deserialize_with = "money_from_str")]
    amount: Money,
    #[serde(default)]
    memo: String,
}

fn money_from_str<'de, D: serde::Deserializer<'de>>(d: D) -> Result<Money, D::Error> {
    let s = String::deserialize(d)?;
    s.parse::<Money>().map_err(serde::de::Error::custom)
}

/// 中介型別 → 核心型別。這裡做所有「外部格式特有」的驗證。
impl TryFrom<TxRow> for Transaction {
    type Error = String;

    fn try_from(row: TxRow) -> Result<Self, Self::Error> {
        let kind = match row.kind.as_str() {
            "deposit" => TxKind::Deposit {
                account: row.account.ok_or("deposit 需要 account 欄位")?,
            },
            "withdraw" => TxKind::Withdraw {
                account: row.account.ok_or("withdraw 需要 account 欄位")?,
            },
            "transfer" => TxKind::Transfer {
                from: row.from.ok_or("transfer 需要 from 欄位")?,
                to: row.to.ok_or("transfer 需要 to 欄位")?,
            },
            other => return Err(format!("未知的交易種類 {other:?}")),
        };
        Ok(Transaction {
            id: row.id,
            date: row.date,
            kind,
            amount: row.amount,
            memo: row.memo,
        })
    }
}

pub fn read_accounts(path: &Path) -> Result<Vec<AccountRow>, ImportError> {
    let file = std::fs::File::open(path).map_err(|e| ImportError::Io {
        path: path.display().to_string(),
        source: e,
    })?;
    read_accounts_from(file)
}

/// 接受任何 Read（檔案、記憶體、網路），方便測試
pub fn read_accounts_from<R: Read>(reader: R) -> Result<Vec<AccountRow>, ImportError> {
    let mut rdr = csv::Reader::from_reader(reader);
    // collect 成 Result<Vec<_>, _>：任何一行失敗整體就失敗
    let rows: Result<Vec<AccountRow>, csv::Error> = rdr.deserialize().collect();
    Ok(rows?)
}

pub fn read_transactions(path: &Path) -> Result<Vec<Transaction>, ImportError> {
    let file = std::fs::File::open(path).map_err(|e| ImportError::Io {
        path: path.display().to_string(),
        source: e,
    })?;
    read_transactions_from(file)
}

pub fn read_transactions_from<R: Read>(reader: R) -> Result<Vec<Transaction>, ImportError> {
    let mut rdr = csv::Reader::from_reader(reader);
    let mut txs = Vec::new();
    for (i, result) in rdr.deserialize::<TxRow>().enumerate() {
        let line = i + 2; // 第 1 行是標頭
        let row = result?;
        let tx = Transaction::try_from(row).map_err(|reason| ImportError::Row { line, reason })?;
        txs.push(tx);
    }
    Ok(txs)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_transactions_csv() {
        let csv = "\
id,date,type,account,from,to,amount,memo
1,2026-10-01,deposit,1001,,,1000.50,薪資
2,2026-10-02,transfer,,1001,1002,30,房租
3,2026-10-03,withdraw,1002,,,5.05,
";
        let txs = read_transactions_from(csv.as_bytes()).unwrap();
        assert_eq!(txs.len(), 3);
        assert_eq!(txs[0].amount, Money::from_cents(100_050));
        assert_eq!(
            txs[1].kind,
            TxKind::Transfer {
                from: 1001,
                to: 1002
            }
        );
        assert_eq!(txs[2].memo, "");
        assert_eq!(txs[2].date, NaiveDate::from_ymd_opt(2026, 10, 3).unwrap());
    }

    #[test]
    fn reports_line_number_for_semantic_errors() {
        let csv = "id,date,type,account,from,to,amount,memo\n1,2026-10-01,transfer,,1001,,10,\n";
        let err = read_transactions_from(csv.as_bytes()).unwrap_err();
        assert!(matches!(err, ImportError::Row { line: 2, .. }), "{err}");
        assert!(err.to_string().contains("to"));
    }

    #[test]
    fn bad_money_is_a_csv_error() {
        let csv = "id,owner,balance\n1,甲,abc\n";
        assert!(matches!(
            read_accounts_from(csv.as_bytes()),
            Err(ImportError::Csv(_))
        ));
    }
}
