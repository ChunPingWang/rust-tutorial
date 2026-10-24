//! 第 10 章命令列入口：生命週期。

use ledger::statement::{first_involving, kind_label, longer_memo, MemoParser};
use ledger::{Ledger, Money, Transaction, TxKind};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut ledger = Ledger::new();
    ledger.open(1001, "王小明", Money::from_yuan(50_000))?;
    ledger.open(1002, "李小華", Money::from_yuan(2_000))?;
    ledger.open(1003, "張大同", Money::from_yuan(800))?;
    ledger.apply_all(sample_transactions());

    // 對帳單借用帳本：帳本活著、沒被修改的期間，對帳單都有效
    let stmt = ledger.statement(1001, 1, 2)?;
    println!(
        "帳戶 #{} {} 第 {}~{} 天對帳單，{} 筆：",
        stmt.account.id,
        stmt.account.owner,
        stmt.from_day,
        stmt.to_day,
        stmt.entries.len()
    );
    for tx in stmt.page(0, 10) {
        println!("  {tx}");
    }
    println!("淨變動：{} 元", stmt.net_change());

    // largest 回傳 Option<&'a Transaction>：借的是帳本，不是 stmt
    let biggest = stmt.largest();
    drop(stmt); // 對帳單丟掉了……
    if let Some(tx) = biggest {
        println!("最大一筆（對帳單已丟棄仍可用）：{tx}"); // ……但這個引用借自帳本，仍然有效
    }

    // 試著在對帳單還活著時修改帳本：
    // let stmt2 = ledger.statement(1002, 1, 4)?;
    // ledger.open(1004, "新帳戶", Money::ZERO)?;   // E0502: cannot borrow `ledger` as mutable
    // println!("{}", stmt2.entries.len());
    // 把 open 移到 stmt2 最後一次使用之後就沒事。生命週期不是找麻煩：它防止對帳單指向已被改掉的資料。

    // 兩個引用、一個回傳
    let h = ledger.history();
    println!("\n較長的備註：{}", longer_memo(&h[0], &h[1]));

    // 回傳借自 txs 而非 label：label 可以是暫時的 String
    let found = {
        let label = format!("查詢 {}", 1003);
        first_involving(h, &label, 1003) // label 在這個區塊結束就死了
    };
    println!(
        "第一筆涉及 1003 的：{}",
        found.map(|t| t.to_string()).unwrap_or_default()
    );
    let _ = first_involving(h, "查詢", 9999);

    // 'static
    for tx in h.iter().take(3) {
        println!("  #{} 是{}", tx.id, kind_label(&tx.kind));
    }

    // 零複製解析器：借用原始字串
    let memo = String::from("  房租  十月  台北 ");
    let mut parser = MemoParser::new(&memo);
    let mut words = Vec::new();
    while let Some(w) = parser.next_word() {
        words.push(w); // 每個 &str 都借自 memo
    }
    println!("\n拆出的詞：{words:?}（原始字串還在：{memo:?}）");

    Ok(())
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
                memo: format!("測試交易 {i}{}", "！".repeat(i as usize % 3)),
            }
        })
        .collect()
}
