//! 第 14 章練習解答。cargo run --release --bin settle_solutions [--deadlock]

use std::collections::BTreeMap;
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, Instant};

use chrono::NaiveDate;
use ledger::{Ledger, Money, Transaction, TxKind};
use rayon::prelude::*;

const ACCOUNTS: u32 = 10_000;
const TXS: u64 = 1_000_000;

fn verify(tx: &Transaction) -> bool {
    let mut h = tx.id ^ (tx.amount.cents() as u64);
    for _ in 0..400 {
        h = h.wrapping_mul(0x9E3779B97F4A7C15).rotate_left(17) ^ 0xD1B54A32D192ED03;
    }
    h != 0
}

fn main() {
    if std::env::args().nth(1).as_deref() == Some("--deadlock") {
        deadlock_demo(true);
        return;
    }
    let ledger = build_ledger();
    let txs = generate(TXS);

    // ---------- 練習 1：可重現的分片 ----------
    // 規則：
    //   (a) 同一帳戶的交易永遠在同一分片，且保持原始順序（bucket 按 txs 順序 push）
    //   (b) 跨分片轉帳收集起來，按交易 ID 排序後單執行緒處理
    //   (c) 兩次執行結果必須相同
    println!("=== 練習 1：可重現的分片 ===");
    let r1 = sharded_settle(&ledger, &txs, 8);
    let r2 = sharded_settle(&ledger, &txs, 8);
    let r3 = sharded_settle(&ledger, &txs, 4);
    println!("8 分片：拒絕 {} 筆，總額 {}", r1.0, r1.1);
    println!("8 分片再跑一次：拒絕 {} 筆，總額 {}", r2.0, r2.1);
    println!("同樣設定跑兩次相同？{}", r1 == r2);
    println!(
        "4 分片：拒絕 {} 筆，總額 {}（與 8 分片相同？{}）",
        r3.0,
        r3.1,
        r1 == r3
    );
    println!("→ 同一組設定可重現；但分片數改變了「哪些轉帳算跨分片」，順序隨之改變，結果就不同。");
    println!(
        "  所以「分片數」是結算結果的一部分，要和程式版本一起記錄。而且它與單執行緒的結果仍不同，"
    );
    println!("  因為跨分片轉帳被延後；要完全等價於序列處理，必須依照交易間的相依關係排程，不只是分片。\n");

    // ---------- 練習 2：rayon ----------
    println!("=== 練習 2：rayon ===");
    let t = Instant::now();
    let n_seq = txs.iter().filter(|tx| verify(tx)).count();
    let seq = t.elapsed();
    let t = Instant::now();
    let n_par = txs.par_iter().filter(|tx| verify(tx)).count(); // 只改 iter → par_iter
    let par = t.elapsed();
    println!(
        "驗證 {} 筆：序列 {} ms，rayon {} ms（{} 個執行緒），結果相同？{}",
        TXS,
        seq.as_millis(),
        par.as_millis(),
        rayon::current_num_threads(),
        n_seq == n_par
    );
    println!("程式碼差異：一個字（iter → par_iter）。rayon 自動切工作、管執行緒池、合併結果。\n");

    // ---------- 練習 3：死鎖 ----------
    println!("=== 練習 3：死鎖 ===");
    println!("用 --deadlock 參數執行會真的卡住（請用 Ctrl-C 或 timeout 結束）。");
    deadlock_demo(false);
    println!();

    // ---------- 練習 4：執行緒數 vs 時間 ----------
    println!("=== 練習 4：channel 模式的加速比 ===");
    let base = channel_settle(&ledger, &txs, 1);
    println!("{:>5} {:>8} {:>8}", "執行緒", "ms", "加速比");
    for workers in [1usize, 2, 4, 8, 16] {
        let ms = channel_settle(&ledger, &txs, workers);
        println!(
            "{workers:>5} {ms:>8} {:>8.2}",
            base as f64 / ms.max(1) as f64
        );
    }
    println!(
        "機器有 {} 個邏輯核心。超過核心數後不再變快，甚至變慢（切換成本）；",
        thread::available_parallelism()
            .map(|n| n.get())
            .unwrap_or(0)
    );
    println!("而且消費者是序列的，加速比受 Amdahl 定律限制：驗證平行化了，過帳沒有。");
}

/// 練習 1
fn sharded_settle(ledger: &Ledger, txs: &[Transaction], shards_n: usize) -> (usize, Money) {
    let shard_of = |a: u32| (a % shards_n as u32) as usize;
    let mut buckets: Vec<Vec<Transaction>> = vec![Vec::new(); shards_n];
    let mut cross: Vec<Transaction> = Vec::new();
    for tx in txs {
        match tx.kind {
            TxKind::Deposit { account } | TxKind::Withdraw { account } => {
                buckets[shard_of(account)].push(tx.clone())
            }
            TxKind::Transfer { from, to } if shard_of(from) == shard_of(to) => {
                buckets[shard_of(from)].push(tx.clone())
            }
            TxKind::Transfer { .. } => cross.push(tx.clone()),
        }
    }
    cross.sort_by_key(|t| t.id); // (b) 明確的全局順序

    let mut shards: Vec<Ledger> = (0..shards_n).map(|_| Ledger::new()).collect();
    for a in ledger.accounts() {
        shards[shard_of(a.id)]
            .open(a.id, &a.owner, a.balance())
            .unwrap();
    }
    let rejected: usize = thread::scope(|s| {
        let hs: Vec<_> = shards
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
        hs.into_iter().map(|h| h.join().unwrap()).sum()
    });
    let mut merged = Ledger::new();
    let mut ids: Vec<&ledger::Account> = shards.iter().flat_map(|s| s.accounts()).collect();
    ids.sort_by_key(|a| a.id); // 合併順序也固定
    for a in ids {
        merged.open(a.id, &a.owner, a.balance()).unwrap();
    }
    let cross_rejected = cross
        .into_iter()
        .filter(|tx| verify(tx) && merged.apply(tx.clone()).is_err())
        .count();
    (rejected + cross_rejected, merged.total_balance())
}

/// 練習 3：兩把鎖、兩個執行緒、反向取鎖 = 死鎖。fixed=true 時改為固定順序。
fn deadlock_demo(really_deadlock: bool) {
    let a = Arc::new(Mutex::new(Ledger::new()));
    let b = Arc::new(Mutex::new(Ledger::new()));
    let (a1, b1, a2, b2) = (
        Arc::clone(&a),
        Arc::clone(&b),
        Arc::clone(&a),
        Arc::clone(&b),
    );

    let order = |first: &Arc<Mutex<Ledger>>, second: &Arc<Mutex<Ledger>>| {
        // 修法：永遠按位址（或帳戶 ID）由小到大取鎖，兩個執行緒就不會互等
        if Arc::as_ptr(first) < Arc::as_ptr(second) {
            (Arc::clone(first), Arc::clone(second))
        } else {
            (Arc::clone(second), Arc::clone(first))
        }
    };

    let t1 = thread::spawn(move || {
        for _ in 0..1_000 {
            let (x, y) = if really_deadlock {
                (Arc::clone(&a1), Arc::clone(&b1))
            } else {
                order(&a1, &b1)
            };
            let _gx = x.lock().unwrap();
            thread::sleep(Duration::from_micros(1)); // 讓交錯更容易發生
            let _gy = y.lock().unwrap();
        }
    });
    let t2 = thread::spawn(move || {
        for _ in 0..1_000 {
            // 反向：先 b 再 a
            let (x, y) = if really_deadlock {
                (Arc::clone(&b2), Arc::clone(&a2))
            } else {
                order(&b2, &a2)
            };
            let _gx = x.lock().unwrap();
            thread::sleep(Duration::from_micros(1));
            let _gy = y.lock().unwrap();
        }
    });
    let t = Instant::now();
    t1.join().unwrap();
    t2.join().unwrap();
    println!(
        "{}：兩個執行緒各取鎖 1000 次，{} ms 完成",
        if really_deadlock {
            "反向取鎖"
        } else {
            "固定順序取鎖"
        },
        t.elapsed().as_millis()
    );
}

/// 練習 4：第 14 章的 channel 模式，參數化 worker 數，回傳毫秒
fn channel_settle(ledger: &Ledger, txs: &[Transaction], workers: usize) -> u128 {
    let t = Instant::now();
    let (send, recv) = std::sync::mpsc::channel::<Transaction>();
    let chunk = txs.len().div_ceil(workers);
    let producers: Vec<_> = txs
        .chunks(chunk)
        .map(|c| {
            let c = c.to_vec();
            let s = send.clone();
            thread::spawn(move || {
                for tx in c {
                    if verify(&tx) {
                        s.send(tx).unwrap();
                    }
                }
            })
        })
        .collect();
    drop(send);
    let consumer = thread::spawn({
        let mut l = ledger.clone();
        move || {
            let mut n = 0;
            for tx in recv {
                if l.apply(tx).is_err() {
                    n += 1;
                }
            }
            n
        }
    });
    for p in producers {
        p.join().unwrap();
    }
    let _ = consumer.join().unwrap();
    t.elapsed().as_millis()
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
fn _unused(_: BTreeMap<u32, Money>) {}
