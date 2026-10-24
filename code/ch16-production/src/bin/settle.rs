//! 第 14 章：多執行緒結算。
//!
//! 情境：夜間結算要處理大量交易。單執行緒太慢，機器有多核。
//! 這個程式示範四種並行模式，以及編譯器如何擋住資料競爭。
//!
//! 執行：cargo run --release --bin settle

use std::collections::HashMap;
use std::sync::mpsc;
use std::sync::{Arc, Mutex, RwLock};
use std::thread;
use std::time::Instant;

use chrono::NaiveDate;
use ledger::{report, Ledger, LedgerError, Money, Transaction, TxKind};

const ACCOUNTS: u32 = 10_000;
const TXS: u64 = 1_000_000;
const WORKERS: usize = 8;

fn main() {
    let ledger = build_ledger();
    let txs = generate(TXS);
    println!(
        "{} 個帳戶，{} 筆交易，{} 個工作執行緒\n",
        ACCOUNTS, TXS, WORKERS
    );

    // ---------- 模式 0：單執行緒基準 ----------
    let t = Instant::now();
    let mut single = ledger.clone();
    let mut rejected = 0;
    for tx in txs.iter().cloned() {
        if verify(&tx) && single.apply(tx).is_err() {
            rejected += 1;
        }
    }
    println!(
        "[單執行緒]        {:>6} ms，拒絕 {} 筆，總額 {}",
        t.elapsed().as_millis(),
        rejected,
        single.total_balance()
    );

    // ---------- 模式 1：Arc<Mutex<Ledger>> + 平均分配 ----------
    // 最直接的做法：一把大鎖。正確，但所有執行緒搶同一把鎖，不會比單執行緒快多少。
    let t = Instant::now();
    let shared = Arc::new(Mutex::new(ledger.clone()));
    let rejected_count = Arc::new(Mutex::new(0usize));
    let chunks: Vec<Vec<Transaction>> = txs
        .chunks(TXS as usize / WORKERS)
        .map(|c| c.to_vec())
        .collect();
    let handles: Vec<_> = chunks
        .into_iter()
        .map(|chunk| {
            let shared = Arc::clone(&shared);
            let rejected_count = Arc::clone(&rejected_count);
            // move：閉包取得 chunk、shared、rejected_count 的所有權，才能送進另一個執行緒
            thread::spawn(move || {
                let mut local_rejected = 0;
                for tx in chunk {
                    if !verify(&tx) {
                        continue; // 驗證在鎖外面做：這是平行的部分
                    }
                    // lock() 回傳 MutexGuard；guard 離開作用域自動解鎖（RAII，第 2 章的 Drop）
                    let mut ledger = shared.lock().unwrap();
                    if ledger.apply(tx).is_err() {
                        local_rejected += 1;
                    }
                } // 每一圈結束 guard 釋放，別的執行緒才有機會
                *rejected_count.lock().unwrap() += local_rejected;
            })
        })
        .collect();
    for h in handles {
        h.join().unwrap(); // 等待執行緒結束；如果它 panic 了，這裡會拿到 Err
    }
    let result = shared.lock().unwrap();
    println!(
        "[Arc<Mutex> 大鎖]  {:>6} ms，拒絕 {} 筆，總額 {}",
        t.elapsed().as_millis(),
        rejected_count.lock().unwrap(),
        result.total_balance()
    );
    drop(result);

    // ---------- 模式 2：channel + 單一寫入者 ----------
    // 多個執行緒做「解析/驗證」這種可平行的前置工作，透過 channel 把結果送給唯一一個持有帳本的執行緒。
    // 帳本不需要鎖：只有一個執行緒碰它。這是 Go 的「不要用共享記憶體溝通，用溝通來共享記憶體」。
    let t = Instant::now();
    let (tx_send, tx_recv) = mpsc::channel::<Transaction>();
    let producers: Vec<_> = txs
        .chunks(TXS as usize / WORKERS)
        .map(|chunk| {
            let chunk = chunk.to_vec();
            let sender = tx_send.clone(); // mpsc = multi-producer, single-consumer：sender 可以 clone
            thread::spawn(move || {
                for tx in chunk {
                    // 前置工作（驗證）在多個 producer 平行做，過帳交給唯一的 consumer
                    if verify(&tx) {
                        sender.send(tx).unwrap();
                    }
                }
            }) // sender 在這裡 drop
        })
        .collect();
    drop(tx_send); // 丟掉主執行緒的 sender，否則 recv 永遠等不到「所有 sender 都關了」
    let consumer = thread::spawn({
        let mut ledger = ledger.clone();
        move || {
            let mut rejected = 0;
            for tx in tx_recv {
                // 所有 sender drop 後迴圈自然結束
                if ledger.apply(tx).is_err() {
                    rejected += 1;
                }
            }
            (ledger, rejected)
        }
    });
    for p in producers {
        p.join().unwrap();
    }
    let (result, rejected) = consumer.join().unwrap();
    println!(
        "[channel 單寫者]   {:>6} ms，拒絕 {} 筆，總額 {}",
        t.elapsed().as_millis(),
        rejected,
        result.total_balance()
    );

    // ---------- 模式 3：分片（sharding）----------
    // 真正的平行：把帳戶分成 N 組，每組一個帳本一個執行緒。
    // 只涉及同一組的交易可以完全平行；跨組的轉帳另外處理。
    let t = Instant::now();
    let shard_of = |account: u32| (account % WORKERS as u32) as usize;
    let mut buckets: Vec<Vec<Transaction>> = vec![Vec::new(); WORKERS];
    let mut cross_shard = Vec::new();
    for tx in &txs {
        match tx.kind {
            TxKind::Deposit { account } | TxKind::Withdraw { account } => {
                buckets[shard_of(account)].push(tx.clone())
            }
            TxKind::Transfer { from, to } if shard_of(from) == shard_of(to) => {
                buckets[shard_of(from)].push(tx.clone())
            }
            TxKind::Transfer { .. } => cross_shard.push(tx.clone()),
        }
    }
    let mut shards: Vec<Ledger> = (0..WORKERS).map(|_| Ledger::new()).collect();
    for acct in ledger.accounts() {
        shards[shard_of(acct.id)]
            .open(acct.id, &acct.owner, acct.balance())
            .unwrap();
    }
    // thread::scope：可以借用外面的資料（這裡是 buckets），不需要 Arc，因為 scope 保證執行緒在離開前結束
    let shard_rejected: Vec<usize> = thread::scope(|s| {
        let handles: Vec<_> = shards
            .iter_mut()
            .zip(buckets.iter())
            .map(|(shard, bucket)| {
                s.spawn(move || {
                    bucket
                        .iter()
                        .filter(|tx| verify(tx) && shard.apply((*tx).clone()).is_err())
                        .count()
                })
            })
            .collect();
        handles.into_iter().map(|h| h.join().unwrap()).collect()
    });
    // 跨分片轉帳：合併之後單執行緒處理（真實系統會用兩階段提交）
    let mut merged = Ledger::new();
    for shard in &shards {
        for a in shard.accounts() {
            merged.open(a.id, &a.owner, a.balance()).unwrap();
        }
    }
    let mut cross_rejected = 0;
    for tx in cross_shard.iter().cloned() {
        if verify(&tx) && merged.apply(tx).is_err() {
            cross_rejected += 1;
        }
    }
    let total_rejected: usize = shard_rejected.iter().sum::<usize>() + cross_rejected;
    println!(
        "[分片 ×{}]         {:>6} ms，拒絕 {} 筆，總額 {}（跨分片轉帳 {} 筆另外處理）",
        WORKERS,
        t.elapsed().as_millis(),
        total_rejected,
        merged.total_balance(),
        cross_shard.len()
    );

    // ---------- 模式 4：RwLock：很多讀者、偶爾寫 ----------
    // 報表查詢遠多於過帳的場合。RwLock 允許多個讀者同時進入，寫者獨佔。
    let t = Instant::now();
    let shared = Arc::new(RwLock::new(single));
    let readers: Vec<_> = (0..WORKERS)
        .map(|i| {
            let shared = Arc::clone(&shared);
            thread::spawn(move || {
                let ledger = shared.read().unwrap(); // 多個 read() 可以同時持有
                let top = report::top_n(&ledger, 1);
                (i, top.first().map(|t| t.id))
            })
        })
        .collect();
    let answers: Vec<_> = readers.into_iter().map(|h| h.join().unwrap()).collect();
    println!(
        "[RwLock 讀者 ×{}]   {:>6} ms，每個讀者看到的最大交易：{:?}",
        WORKERS,
        t.elapsed().as_millis(),
        answers.iter().map(|a| a.1).collect::<Vec<_>>()
    );

    println!();
    println!("注意：四種模式的「拒絕筆數」與「總額」不同。這不是 bug，是順序：");
    println!("  同一帳戶的提款誰先到決定誰被拒。平行處理改變了交易順序，結果就不同。");
    println!("  金融系統要求可重現，所以真正可用的只有保留每個帳戶內部順序的做法（分片），");
    println!(
        "  而且跨分片的交易要有明確的排序規則。並行不只是「變快」，還要先定義「什麼叫正確」。"
    );

    // ---------- 編譯器擋住的東西 ----------
    // 下面每一段打開註解都是編譯錯誤。請一個一個試，讀錯誤訊息。
    //
    // (a) 沒有鎖就跨執行緒修改：
    //     let mut plain = ledger.clone();
    //     thread::spawn(move || plain.apply(txs[0].clone()));
    //     plain.total_balance();   // error[E0382]: borrow of moved value: `plain`
    //     → 編譯器說 plain 已經被移進執行緒了。你不可能在兩個執行緒同時持有它。
    //
    // (b) 用第 13 章的 Rc<RefCell<>>：
    //     let rc = std::rc::Rc::new(std::cell::RefCell::new(ledger.clone()));
    //     let rc2 = std::rc::Rc::clone(&rc);
    //     thread::spawn(move || rc2.borrow_mut().total_balance());
    //     → error[E0277]: `Rc<RefCell<Ledger>>` cannot be sent between threads safely
    //       the trait `Send` is not implemented for `Rc<RefCell<Ledger>>`
    //     → Rc 的計數不是原子的，RefCell 的借用標記也不是。編譯器知道，所以拒絕。
    //
    // (c) 把 MutexGuard 送進另一個執行緒：
    //     let guard = shared.write().unwrap();
    //     thread::spawn(move || drop(guard));
    //     → error[E0277]: `MutexGuard<'_, Ledger>` cannot be sent between threads safely
    //     → 鎖必須在取得它的執行緒釋放。
    //
    // 這三個錯誤加起來，就是前言裡「事故二」在 Rust 不可能發生的原因。

    let _ = (LedgerError::Overflow, Money::ZERO); // 讓 import 不閒置
}

/// 模擬每筆交易都要做的 CPU 密集驗證（真實系統裡是簽章檢查、風控規則、反洗錢比對）。
/// 沒有這種工作的話，過帳本身只是一次 HashMap 查找，平行化的開銷會比收益大。
fn verify(tx: &Transaction) -> bool {
    let mut h = tx.id ^ (tx.amount.cents() as u64);
    for _ in 0..400 {
        h = h.wrapping_mul(0x9E3779B97F4A7C15).rotate_left(17) ^ 0xD1B54A32D192ED03;
    }
    h != 0 // 永遠為真，但編譯器無法省略這段運算
}

fn build_ledger() -> Ledger {
    let mut ledger = Ledger::new();
    for id in 1..=ACCOUNTS {
        ledger
            .open(id, &format!("帳戶{id}"), Money::from_yuan(10_000))
            .unwrap();
    }
    ledger
}

/// 產生可重現的測試交易
fn generate(n: u64) -> Vec<Transaction> {
    let mut state = 42u64;
    let mut next = move || {
        state = state
            .wrapping_mul(6364136223846793005)
            .wrapping_add(1442695040888963407);
        state >> 33
    };
    let date = NaiveDate::from_ymd_opt(2026, 10, 31).unwrap();
    (1..=n)
        .map(|id| {
            let a = (next() % ACCOUNTS as u64) as u32 + 1;
            let b = (next() % ACCOUNTS as u64) as u32 + 1;
            let kind = match next() % 3 {
                0 => TxKind::Deposit { account: a },
                1 => TxKind::Withdraw { account: a },
                _ => TxKind::Transfer { from: a, to: b },
            };
            Transaction {
                id,
                date,
                kind,
                amount: Money::from_cents((next() % 500_000) as i64 + 1),
                memo: String::new(),
            }
        })
        .collect()
}

#[allow(dead_code)]
fn _unused(_: HashMap<u32, Money>) {}
