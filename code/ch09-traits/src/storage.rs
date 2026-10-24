//! 儲存層：把帳本存起來、讀回來。
//!
//! 核心邏輯（Ledger）不知道也不關心資料存在哪裡。這個 trait 定義了「儲存後端」要能做什麼，
//! 記憶體、文字檔、未來的資料庫各自實作它。

use std::fmt;
use std::fs;
use std::path::PathBuf;

use crate::{Account, Ledger, Money, Transaction, TxKind};

/// 儲存層的錯誤
#[derive(Debug)]
pub enum StorageError {
    Io(std::io::Error),
    Corrupt { line: usize, reason: String },
    NotFound,
}

impl fmt::Display for StorageError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            StorageError::Io(e) => write!(f, "I/O 錯誤：{e}"),
            StorageError::Corrupt { line, reason } => write!(f, "第 {line} 行格式錯誤：{reason}"),
            StorageError::NotFound => write!(f, "找不到已儲存的帳本"),
        }
    }
}

impl std::error::Error for StorageError {}

/// From：讓 `?` 能自動把 io::Error 轉成 StorageError
impl From<std::io::Error> for StorageError {
    fn from(e: std::io::Error) -> Self {
        StorageError::Io(e)
    }
}

/// 儲存後端要能做的事。
///
/// 這就是「介面」：任何型別只要實作了 save 與 load，就能當儲存後端。
pub trait Storage {
    fn save(&mut self, ledger: &Ledger) -> Result<(), StorageError>;
    fn load(&self) -> Result<Ledger, StorageError>;

    /// 預設方法：實作者可以不寫，也可以覆寫
    fn describe(&self) -> String {
        String::from("未命名的儲存後端")
    }
}

// ---------- 實作一：記憶體 ----------

/// 把帳本的快照存在記憶體裡。測試用，或當作「上一次成功的狀態」。
#[derive(Debug, Default)]
pub struct InMemoryStorage {
    snapshot: Option<Ledger>,
}

impl Storage for InMemoryStorage {
    fn save(&mut self, ledger: &Ledger) -> Result<(), StorageError> {
        self.snapshot = Some(ledger.clone());
        Ok(())
    }

    fn load(&self) -> Result<Ledger, StorageError> {
        self.snapshot.clone().ok_or(StorageError::NotFound)
    }

    fn describe(&self) -> String {
        String::from("記憶體快照")
    }
}

// ---------- 實作二：純文字檔 ----------

/// 用一個簡單的自訂逐行格式存成文字檔。第 12 章會換成 serde + CSV/JSON。
///
/// 格式：
/// ```text
/// A,<id>,<owner>,<balance_cents>
/// T,<id>,<day>,<kind>,<a>,<b>,<amount_cents>,<memo>
/// ```
pub struct TextFileStorage {
    path: PathBuf,
}

impl TextFileStorage {
    pub fn new(path: impl Into<PathBuf>) -> Self {
        Self { path: path.into() }
    }
}

impl Storage for TextFileStorage {
    fn save(&mut self, ledger: &Ledger) -> Result<(), StorageError> {
        let mut out = String::new();
        let mut accounts: Vec<&Account> = ledger.accounts().collect();
        accounts.sort_by_key(|a| a.id);
        for a in accounts {
            out.push_str(&format!("A,{},{},{}\n", a.id, a.owner, a.balance().cents()));
        }
        for tx in ledger.history() {
            let (kind, a, b) = match tx.kind {
                TxKind::Deposit { account } => ("D", account, 0),
                TxKind::Withdraw { account } => ("W", account, 0),
                TxKind::Transfer { from, to } => ("X", from, to),
            };
            out.push_str(&format!(
                "T,{},{},{},{},{},{},{}\n",
                tx.id,
                tx.day,
                kind,
                a,
                b,
                tx.amount.cents(),
                tx.memo
            ));
        }
        fs::write(&self.path, out)?; // io::Error 經 From 自動轉成 StorageError
        Ok(())
    }

    fn load(&self) -> Result<Ledger, StorageError> {
        let text = match fs::read_to_string(&self.path) {
            Ok(t) => t,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                return Err(StorageError::NotFound)
            }
            Err(e) => return Err(e.into()),
        };
        let mut accounts = Vec::new();
        let mut history = Vec::new();
        for (i, line) in text.lines().enumerate() {
            let line_no = i + 1;
            let corrupt = |reason: &str| StorageError::Corrupt {
                line: line_no,
                reason: reason.to_string(),
            };
            let fields: Vec<&str> = line.splitn(8, ',').collect();
            match fields.first().copied() {
                Some("A") if fields.len() == 4 => {
                    let id = fields[1].parse().map_err(|_| corrupt("帳戶 ID"))?;
                    let cents = fields[3].parse().map_err(|_| corrupt("餘額"))?;
                    accounts.push(Account::new(id, fields[2], Money::from_cents(cents)));
                }
                Some("T") if fields.len() == 8 => {
                    let a: u32 = fields[4].parse().map_err(|_| corrupt("帳戶 a"))?;
                    let b: u32 = fields[5].parse().map_err(|_| corrupt("帳戶 b"))?;
                    let kind = match fields[3] {
                        "D" => TxKind::Deposit { account: a },
                        "W" => TxKind::Withdraw { account: a },
                        "X" => TxKind::Transfer { from: a, to: b },
                        _ => return Err(corrupt("交易種類")),
                    };
                    history.push(Transaction {
                        id: fields[1].parse().map_err(|_| corrupt("交易 ID"))?,
                        day: fields[2].parse().map_err(|_| corrupt("日期"))?,
                        kind,
                        amount: Money::from_cents(fields[6].parse().map_err(|_| corrupt("金額"))?),
                        memo: fields[7].to_string(),
                    });
                }
                _ => return Err(corrupt("無法辨識的行")),
            }
        }
        Ok(Ledger::restore(accounts, history))
    }

    fn describe(&self) -> String {
        format!("文字檔 {}", self.path.display())
    }
}
