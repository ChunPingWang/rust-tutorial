//! 儲存層。第 9 章手刻的文字格式換成 serde_json：一行序列化、一行反序列化。

use std::fs;
use std::path::PathBuf;

use thiserror::Error;

use crate::Ledger;

#[derive(Debug, Error)]
pub enum StorageError {
    // #[from] 自動產生 impl From<std::io::Error> for StorageError，? 就能轉換
    #[error("I/O 錯誤：{0}")]
    Io(#[from] std::io::Error),

    #[error("JSON 格式錯誤：{0}")]
    Json(#[from] serde_json::Error),

    #[error("找不到已儲存的帳本")]
    NotFound,
}

pub trait Storage {
    fn save(&mut self, ledger: &Ledger) -> Result<(), StorageError>;
    fn load(&self) -> Result<Ledger, StorageError>;
    fn describe(&self) -> String {
        String::from("未命名的儲存後端")
    }
}

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

/// JSON 檔案儲存。比較一下第 9 章 TextFileStorage 的 80 行手刻解析。
pub struct JsonFileStorage {
    path: PathBuf,
}

impl JsonFileStorage {
    pub fn new(path: impl Into<PathBuf>) -> Self {
        Self { path: path.into() }
    }
}

impl Storage for JsonFileStorage {
    fn save(&mut self, ledger: &Ledger) -> Result<(), StorageError> {
        let json = serde_json::to_string_pretty(ledger)?;
        fs::write(&self.path, json)?;
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
        Ok(serde_json::from_str(&text)?)
    }

    fn describe(&self) -> String {
        format!("JSON 檔 {}", self.path.display())
    }
}
