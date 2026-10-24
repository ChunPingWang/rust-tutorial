//! 練習 1：第二個執行檔。cargo run --bin stats
//! 多個 bin 時放在 src/bin/，每個檔案一個執行檔，名稱就是檔名。

use std::collections::BTreeMap;

use ledger::{Ledger, Money, Transaction, TxKind};

fn main() {
    let mut ledger = Ledger::new();
    ledger
        .open(1001, "王小明", Money::from_yuan(50_000))
        .unwrap();
    ledger
        .open(1002, "李小華", Money::from_yuan(2_000))
        .unwrap();
    ledger.apply_all(
        (1..=6u64)
            .map(|i| Transaction {
                id: i,
                day: 1,
                kind: if i % 2 == 0 {
                    TxKind::Deposit { account: 1001 }
                } else {
                    TxKind::Transfer {
                        from: 1001,
                        to: 1002,
                    }
                },
                amount: Money::from_cents(i as i64 * 100),
                memo: String::new(),
            })
            .collect(),
    );

    // 每個帳戶的交易筆數：一筆轉帳算兩個帳戶各一筆
    let mut counts: BTreeMap<u32, usize> = BTreeMap::new();
    for tx in ledger.history() {
        for id in tx.affected_accounts() {
            *counts.entry(id).or_insert(0) += 1;
        }
    }
    for (id, n) in counts {
        println!("帳戶 {id}：{n} 筆");
    }
}
