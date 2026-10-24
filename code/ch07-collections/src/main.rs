//! 第 7 章：集合與迭代器
//!
//! 需求：帳戶數量上萬，查找要快。財務要各種統計：
//! 每帳戶月總額、最大的 N 筆、按日分組、按金額排序。

use std::collections::{BTreeMap, HashMap, HashSet};

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
struct Money(i64);

impl Money {
    fn format(self) -> String {
        let yuan = self.0 / 100;
        let fen = (self.0 % 100).abs();
        format!("{yuan}.{fen:02} 元")
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
enum TxKind {
    Deposit { account: u32 },
    Withdraw { account: u32 },
    Transfer { from: u32, to: u32 },
}

#[derive(Debug, Clone)]
struct Transaction {
    id: u64,
    day: u32, // 當月第幾天，簡化用
    kind: TxKind,
    amount: Money,
}

#[derive(Debug)]
struct Account {
    id: u32,
    owner: String,
    balance: Money,
}

/// 帳本改用 HashMap：以帳戶 ID 為 key，O(1) 查找
struct Ledger {
    accounts: HashMap<u32, Account>,
    history: Vec<Transaction>,
}

impl Ledger {
    fn new() -> Self {
        Self {
            accounts: HashMap::new(),
            history: Vec::new(),
        }
    }

    fn open(&mut self, id: u32, owner: &str, initial: Money) {
        self.accounts.insert(
            id,
            Account {
                id,
                owner: owner.to_string(),
                balance: initial,
            },
        );
    }

    /// 轉帳現在可以同時取得兩個 &mut：get_disjoint_mut 對不同的 key 回傳不同的 &mut
    /// （key 重複會 panic，所以先檢查 from != to）
    fn transfer(&mut self, from: u32, to: u32, amount: Money) -> bool {
        if from == to {
            return false;
        }
        let [Some(src), Some(dst)] = self.accounts.get_disjoint_mut([&from, &to]) else {
            return false; // 其中一個帳戶不存在
        };
        if amount > src.balance {
            return false;
        }
        src.balance = Money(src.balance.0 - amount.0);
        dst.balance = Money(dst.balance.0 + amount.0);
        true
    }

    fn apply(&mut self, tx: Transaction) -> bool {
        let ok = match tx.kind {
            TxKind::Deposit { account } => match self.accounts.get_mut(&account) {
                Some(a) => {
                    a.balance = Money(a.balance.0 + tx.amount.0);
                    true
                }
                None => false,
            },
            TxKind::Withdraw { account } => match self.accounts.get_mut(&account) {
                Some(a) if tx.amount <= a.balance => {
                    a.balance = Money(a.balance.0 - tx.amount.0);
                    true
                }
                _ => false,
            },
            TxKind::Transfer { from, to } => self.transfer(from, to, tx.amount),
        };
        if ok {
            self.history.push(tx); // 所有權移進 history
        }
        ok
    }

    // ---------- 以下是報表：全部用迭代器 ----------

    /// 總存款額：filter + map + sum
    fn total_deposits(&self) -> Money {
        let cents: i64 = self
            .history
            .iter()
            .filter(|tx| matches!(tx.kind, TxKind::Deposit { .. }))
            .map(|tx| tx.amount.0)
            .sum();
        Money(cents)
    }

    /// 每個帳戶的「流出」總額（提款 + 轉出）：fold 進 HashMap
    fn outflow_by_account(&self) -> HashMap<u32, Money> {
        self.history.iter().fold(HashMap::new(), |mut acc, tx| {
            let debited = match tx.kind {
                TxKind::Withdraw { account } => Some(account),
                TxKind::Transfer { from, .. } => Some(from),
                TxKind::Deposit { .. } => None,
            };
            if let Some(id) = debited {
                // entry API：沒有就插入預設值，然後回傳 &mut
                let total = acc.entry(id).or_insert(Money(0));
                total.0 += tx.amount.0;
            }
            acc
        })
    }

    /// 按日分組：BTreeMap 讓 key 自動排序，印出來就是按日期的
    fn daily_count(&self) -> BTreeMap<u32, usize> {
        let mut map = BTreeMap::new();
        for tx in &self.history {
            *map.entry(tx.day).or_insert(0) += 1;
        }
        map
    }

    /// 金額最大的 n 筆：排序後取前 n，回傳借用
    fn top_n(&self, n: usize) -> Vec<&Transaction> {
        let mut sorted: Vec<&Transaction> = self.history.iter().collect();
        sorted.sort_by(|a, b| b.amount.cmp(&a.amount)); // 降冪
        sorted.truncate(n);
        sorted
    }

    /// 有過交易的帳戶（去重）：HashSet
    fn active_accounts(&self) -> HashSet<u32> {
        self.history
            .iter()
            .flat_map(|tx| match tx.kind {
                TxKind::Deposit { account } | TxKind::Withdraw { account } => vec![account],
                TxKind::Transfer { from, to } => vec![from, to],
            })
            .collect()
    }

    /// 餘額排名：收集成 Vec 再排序（HashMap 本身無序）
    fn ranking(&self) -> Vec<(u32, &str, Money)> {
        let mut v: Vec<(u32, &str, Money)> = self
            .accounts
            .values()
            .map(|a| (a.id, a.owner.as_str(), a.balance))
            .collect();
        v.sort_by_key(|&(_, _, bal)| std::cmp::Reverse(bal));
        v
    }

    /// 連續三筆以上同帳戶提款：windows 示範
    fn rapid_withdrawals(&self) -> Vec<u32> {
        let withdraws: Vec<u32> = self
            .history
            .iter()
            .filter_map(|tx| match tx.kind {
                TxKind::Withdraw { account } => Some(account),
                _ => None,
            })
            .collect();
        withdraws
            .windows(3)
            .filter(|w| w[0] == w[1] && w[1] == w[2])
            .map(|w| w[0])
            .collect::<HashSet<_>>() // 去重
            .into_iter()
            .collect()
    }
}

fn main() {
    let mut ledger = Ledger::new();
    ledger.open(1001, "王小明", Money(5_000_000));
    ledger.open(1002, "李小華", Money(200_000));
    ledger.open(1003, "張大同", Money(80_000));

    // 用迭代器產生一批測試交易
    let txs: Vec<Transaction> = (1..=12u64)
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
                amount: Money(i as i64 * 10_000),
            }
        })
        .collect();

    // 過帳：into_iter 消耗 Vec，每筆交易的所有權移進 apply
    let (posted, rejected): (Vec<_>, Vec<_>) = txs.into_iter().partition(|tx| {
        // partition 只是分類，這裡順便過帳是為了示範；實務上不要在 closure 裡做副作用
        let id = tx.id;
        let ok = ledger.apply(tx.clone());
        if !ok {
            println!("交易 #{id} 被拒");
        }
        ok
    });
    println!("成功 {} 筆，拒絕 {} 筆\n", posted.len(), rejected.len());

    println!("總存款：{}", ledger.total_deposits().format());

    println!("\n各帳戶流出：");
    let mut outflow: Vec<_> = ledger.outflow_by_account().into_iter().collect();
    outflow.sort(); // HashMap 無序，印之前排一下
    for (id, m) in outflow {
        println!("  {id}: {}", m.format());
    }

    println!("\n每日筆數：");
    for (day, n) in ledger.daily_count() {
        println!("  第 {day} 天：{n} 筆");
    }

    println!("\n金額前三：");
    for tx in ledger.top_n(3) {
        println!("  #{} {:?} {}", tx.id, tx.kind, tx.amount.format());
    }

    println!("\n活躍帳戶：{:?}", {
        let mut v: Vec<_> = ledger.active_accounts().into_iter().collect();
        v.sort();
        v
    });

    println!("\n餘額排名：");
    for (i, (id, owner, bal)) in ledger.ranking().iter().enumerate() {
        println!("  {}. #{id} {owner:<6} {}", i + 1, bal.format());
    }

    println!("\n連續提款警示帳戶：{:?}", ledger.rapid_withdrawals());

    // 一些常用的迭代器小技巧
    let amounts: Vec<i64> = ledger.history.iter().map(|t| t.amount.0).collect();
    println!("\n最大：{:?}", amounts.iter().max());
    println!("最小：{:?}", amounts.iter().min());
    println!(
        "有沒有超過十萬元的：{}",
        amounts.iter().any(|&a| a > 10_000_000)
    );
    println!("全部都是正數：{}", amounts.iter().all(|&a| a > 0));
    println!(
        "第一筆超過五萬元的：{:?}",
        amounts.iter().find(|&&a| a > 5_000_000)
    );
    println!("位置：{:?}", amounts.iter().position(|&a| a > 5_000_000));
    let (small, big): (Vec<i64>, Vec<i64>) = amounts.iter().partition(|&&a| a < 500_000);
    println!("小額 {} 筆，大額 {} 筆", small.len(), big.len());
    let labels: Vec<String> = amounts.iter().take(3).map(|a| Money(*a).format()).collect();
    println!("前三筆：{}", labels.join(" | "));
    let pairs: Vec<(u64, i64)> = ledger
        .history
        .iter()
        .map(|t| t.id)
        .zip(amounts.iter().copied())
        .take(2)
        .collect();
    println!("zip：{pairs:?}");

    // 中文字串要用 chars()，不能用 byte 索引
    let name = "王小明";
    println!(
        "「{name}」有 {} 個字、{} bytes，第一個字是 {:?}",
        name.chars().count(),
        name.len(),
        name.chars().next()
    );
}
