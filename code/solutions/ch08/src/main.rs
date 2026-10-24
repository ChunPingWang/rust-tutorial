//! 命令列入口。只負責：建立測試資料、呼叫 lib、印出結果。

use ledger::{report, Ledger, Money, Transaction, TxKind};

fn main() {
    let mut ledger = Ledger::new();
    ledger
        .open(1001, "王小明", Money::from_yuan(50_000))
        .unwrap();
    ledger
        .open(1002, "李小華", Money::from_yuan(2_000))
        .unwrap();
    ledger.open(1003, "張大同", Money::from_yuan(800)).unwrap();

    let txs = sample_transactions();
    let rejected = ledger.apply_all(txs);

    println!("過帳完成，拒絕 {} 筆：", rejected.len());
    for (id, err) in &rejected {
        println!("  #{id}：{err}");
    }

    println!("\n總存款：{}", report::total_deposits(&ledger).format());
    println!("總餘額：{}", ledger.total_balance().format());

    println!("\n每日筆數：");
    for (day, n) in report::daily_count(&ledger) {
        println!("  第 {day} 天：{n} 筆");
    }

    println!("\n金額前三：");
    for tx in report::top_n(&ledger, 3) {
        println!("  {}", tx.describe());
    }

    println!("\n餘額排名：");
    for (i, (id, owner, bal)) in report::ranking(&ledger).iter().enumerate() {
        println!("  {}. #{id} {owner:<6} {}", i + 1, bal.format());
    }

    println!("\n連續提款警示：{:?}", report::rapid_withdrawals(&ledger));
}

fn sample_transactions() -> Vec<Transaction> {
    (1..=12u64)
        .map(|i| {
            let kind = match i % 4 {
                0 => TxKind::Deposit {
                    account: 1001 + (i as u32 % 3),
                },
                1 => TxKind::Withdraw { account: 1002 },
                2 => TxKind::Transfer {
                    from: 1001,
                    to: 1003,
                },
                _ => TxKind::Withdraw { account: 1002 },
            };
            Transaction {
                id: i,
                day: (i as u32 + 2) / 3,
                kind,
                amount: Money::from_cents(i as i64 * 10_000),
                memo: format!("測試交易 {i}"),
            }
        })
        .collect()
}
