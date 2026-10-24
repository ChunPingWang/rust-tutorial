//! 整合測試共用的輔助程式碼。放在 tests/common/mod.rs（而不是 tests/common.rs），
//! cargo 才不會把它當成一個獨立的測試 crate 去執行。

// 每個整合測試檔都會各自編譯這個模組，不一定每個檔都用到所有函式，所以允許 dead_code
#![allow(dead_code)]

use ledger::{Ledger, Money, Transaction, TxKind};

pub fn seeded_ledger() -> Ledger {
    let mut l = Ledger::new();
    l.open(1001, "王小明", Money::from_yuan(50_000)).unwrap();
    l.open(1002, "李小華", Money::from_yuan(2_000)).unwrap();
    l.open(1003, "張大同", Money::from_yuan(800)).unwrap();
    l
}

pub fn tx(id: u64, day: u32, kind: TxKind, yuan: i64) -> Transaction {
    Transaction {
        id,
        day,
        kind,
        amount: Money::from_yuan(yuan),
        memo: format!("tx {id}"),
    }
}
