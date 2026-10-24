//! 第 5 章：列舉與模式比對
//!
//! 需求：交易有三種：存款、提款、轉帳。轉帳需要兩個帳戶，存款提款只要一個。
//! 「找帳戶」可能找不到，不能用 null。

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
struct Money(i64);

impl Money {
    fn format(self) -> String {
        let yuan = self.0 / 100;
        let fen = (self.0 % 100).abs();
        format!("{yuan}.{fen:02} 元")
    }
}

/// 交易種類：每個變體（variant）可以帶不同的資料
#[derive(Debug, Clone, PartialEq)]
enum TxKind {
    Deposit { account: u32 },
    Withdraw { account: u32 },
    Transfer { from: u32, to: u32 },
}

#[derive(Debug, Clone)]
struct Transaction {
    id: u64,
    kind: TxKind,
    amount: Money,
    memo: String,
}

/// 交易狀態：單純的標籤，沒有資料
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Status {
    Pending,
    Posted,
    Rejected,
}

impl Transaction {
    /// match 必須窮舉所有變體：以後新增一種交易，這裡沒處理就編譯不過
    fn describe(&self) -> String {
        let action = match &self.kind {
            TxKind::Deposit { account } => format!("帳戶 {account} 存入"),
            TxKind::Withdraw { account } => format!("帳戶 {account} 提出"),
            TxKind::Transfer { from, to } => format!("帳戶 {from} 轉給 {to}"),
        };
        format!(
            "#{} {} {}（{}）",
            self.id,
            action,
            self.amount.format(),
            self.memo
        )
    }

    /// 這筆交易會影響哪些帳戶
    fn affected_accounts(&self) -> Vec<u32> {
        match self.kind {
            TxKind::Deposit { account } | TxKind::Withdraw { account } => vec![account],
            TxKind::Transfer { from, to } => vec![from, to],
        }
    }

    /// 是否與某帳戶有關：用 matches! 巨集做布林判斷
    fn involves(&self, account_id: u32) -> bool {
        matches!(
            self.kind,
            TxKind::Deposit { account } | TxKind::Withdraw { account } if account == account_id
        ) || matches!(
            self.kind,
            TxKind::Transfer { from, to } if from == account_id || to == account_id
        )
    }
}

#[derive(Debug)]
struct Account {
    id: u32,
    owner: String,
    balance: Money,
}

/// 帳本：帳戶清單
struct Ledger {
    accounts: Vec<Account>,
}

impl Ledger {
    /// 可能找不到 → Option。這就是 Rust 沒有 null 的方式。
    fn find(&self, id: u32) -> Option<&Account> {
        for acct in &self.accounts {
            if acct.id == id {
                return Some(acct);
            }
        }
        None
    }

    fn find_mut(&mut self, id: u32) -> Option<&mut Account> {
        self.accounts.iter_mut().find(|a| a.id == id)
    }

    /// 餘額查詢：示範各種處理 Option 的方式
    fn balance_of(&self, id: u32) -> Option<Money> {
        // map：有值就轉換，沒值就維持 None
        self.find(id).map(|a| a.balance)
    }

    /// 試著過帳。回傳 Status 說明結果（下一章會改成 Result）
    fn apply(&mut self, tx: &Transaction) -> Status {
        // 大額交易先掛起等人工審核
        if tx.amount.0 >= 10_000_000 {
            return Status::Pending;
        }
        match &tx.kind {
            TxKind::Deposit { account } => {
                // if let：只關心其中一種情況
                if let Some(acct) = self.find_mut(*account) {
                    acct.balance = Money(acct.balance.0 + tx.amount.0);
                    Status::Posted
                } else {
                    Status::Rejected
                }
            }
            TxKind::Withdraw { account } => {
                // let-else：不是想要的形狀就提早離開（Rust 1.65+）
                let Some(acct) = self.find_mut(*account) else {
                    return Status::Rejected;
                };
                if tx.amount > acct.balance {
                    return Status::Rejected;
                }
                acct.balance = Money(acct.balance.0 - tx.amount.0);
                Status::Posted
            }
            TxKind::Transfer { from, to } => {
                // 先檢查兩個帳戶都存在、來源夠錢，再動手改。
                // 注意：不能同時持有兩個 &mut 指向 self.accounts，所以分兩步
                let from_balance = match self.balance_of(*from) {
                    Some(b) => b,
                    None => return Status::Rejected,
                };
                if self.find(*to).is_none() || tx.amount > from_balance {
                    return Status::Rejected;
                }
                if let Some(src) = self.find_mut(*from) {
                    src.balance = Money(src.balance.0 - tx.amount.0);
                }
                if let Some(dst) = self.find_mut(*to) {
                    dst.balance = Money(dst.balance.0 + tx.amount.0);
                }
                Status::Posted
            }
        }
    }
}

fn main() {
    let mut ledger = Ledger {
        accounts: vec![
            Account {
                id: 1001,
                owner: "王小明".to_string(),
                balance: Money(5_000_000),
            },
            Account {
                id: 1002,
                owner: "李小華".to_string(),
                balance: Money(200_000),
            },
        ],
    };

    let txs = vec![
        Transaction {
            id: 1,
            kind: TxKind::Deposit { account: 1001 },
            amount: Money(100_000),
            memo: "利息".to_string(),
        },
        Transaction {
            id: 2,
            kind: TxKind::Transfer {
                from: 1001,
                to: 1002,
            },
            amount: Money(1_500_000),
            memo: "房租".to_string(),
        },
        Transaction {
            id: 3,
            kind: TxKind::Withdraw { account: 1002 },
            amount: Money(9_999_999),
            memo: "買車".to_string(),
        },
        Transaction {
            id: 4,
            kind: TxKind::Deposit { account: 9999 },
            amount: Money(1),
            memo: "幽靈帳戶".to_string(),
        },
        Transaction {
            id: 5,
            kind: TxKind::Deposit { account: 1001 },
            amount: Money(50_000_000),
            memo: "賣房".to_string(),
        },
    ];

    for tx in &txs {
        let status = ledger.apply(tx);
        // match 一個簡單 enum 來決定顯示符號
        let mark = match status {
            Status::Posted => "✓",
            Status::Rejected => "✗",
            Status::Pending => "…",
        };
        println!("{mark} {}", tx.describe());
    }

    println!();
    for acct in &ledger.accounts {
        println!("#{} {}：{}", acct.id, acct.owner, acct.balance.format());
    }

    // Option 的各種用法
    println!();
    println!("1001 餘額：{:?}", ledger.balance_of(1001));
    println!("9999 餘額：{:?}", ledger.balance_of(9999));
    println!(
        "9999 餘額（給預設值）：{}",
        ledger.balance_of(9999).unwrap_or(Money(0)).format()
    );
    match ledger.find(1002) {
        Some(a) => println!("找到 {}", a.owner),
        None => println!("沒有這個帳戶"),
    }

    // 與帳戶 1002 有關的交易
    let related: Vec<u64> = txs
        .iter()
        .filter(|t| t.involves(1002))
        .map(|t| t.id)
        .collect();
    println!("與 1002 有關的交易：{related:?}");
    println!("交易 #2 影響的帳戶：{:?}", txs[1].affected_accounts());

    // match 數值範圖與守衛
    for amount in [Money(50), Money(5_000), Money(50_000_000)] {
        let tier = match amount.0 {
            0..=999 => "零錢",
            1_000..=999_999 => "一般",
            n if n >= 10_000_000 => "大額（需審核）",
            _ => "中額",
        };
        println!("{} → {tier}", amount.format());
    }
}
