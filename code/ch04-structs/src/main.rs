//! 第 4 章：結構體
//!
//! 需求：兩個平行的 Vec 太容易對不上了。帳戶、交易、金額都該是明確的型別。

/// 金額：以分為單位的整數。用 newtype 包起來，防止和「數量」「ID」等整數混用。
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
struct Money(i64);

impl Money {
    /// 從「元」建立（只接受整數元，避免浮點數進來）
    fn from_yuan(yuan: i64) -> Money {
        Money(yuan * 100)
    }

    fn cents(self) -> i64 {
        self.0
    }

    fn format(self) -> String {
        let sign = if self.0 < 0 { "-" } else { "" };
        let yuan = (self.0 / 100).abs();
        let fen = (self.0 % 100).abs();
        format!("{sign}{yuan}.{fen:02} 元")
    }
}

/// 帳戶
#[derive(Debug, Clone, PartialEq)]
struct Account {
    id: u32,
    owner: String,
    balance: Money,
}

impl Account {
    /// 關聯函式（associated function）：沒有 self，透過 Account::new 呼叫。類似建構子。
    fn new(id: u32, owner: &str) -> Account {
        Account {
            id, // 欄位名與變數名相同時可以簡寫
            owner: owner.to_string(),
            balance: Money(0),
        }
    }

    /// 方法（method）：第一個參數是 self 的某種形式。&self = 只讀。
    fn summary(&self) -> String {
        format!("#{} {}：{}", self.id, self.owner, self.balance.format())
    }

    /// &mut self = 會修改
    fn deposit(&mut self, amount: Money) {
        self.balance = Money(self.balance.0 + amount.0);
    }

    /// 回傳 bool 表示成功與否（第 6 章會換成更好的錯誤處理）
    fn withdraw(&mut self, amount: Money) -> bool {
        if amount > self.balance {
            return false;
        }
        self.balance = Money(self.balance.0 - amount.0);
        true
    }
}

/// 交易紀錄
#[derive(Debug, Clone)]
struct Transaction {
    id: u64,
    account_id: u32,
    amount: Money, // 正數存款、負數提款（第 5 章會用 enum 改掉這個設計）
    memo: String,
}

/// 單元結構體 + 元組結構體示範：交易序號產生器
struct TxIdGenerator {
    next: u64,
}

impl TxIdGenerator {
    fn new() -> Self {
        // Self 是「目前這個型別」的別名
        Self { next: 1 }
    }

    fn next_id(&mut self) -> u64 {
        let id = self.next;
        self.next += 1;
        id
    }
}

fn main() {
    let mut gen = TxIdGenerator::new();

    let mut alice = Account::new(1001, "王小明");
    println!("{}", alice.summary());

    // 存款
    let tx1 = Transaction {
        id: gen.next_id(),
        account_id: alice.id,
        amount: Money::from_yuan(50_000),
        memo: String::from("十月薪資"),
    };
    alice.deposit(tx1.amount);
    println!("{} 後 → {}", tx1.memo, alice.summary());

    // 提款成功
    let tx2 = Transaction {
        id: gen.next_id(),
        account_id: alice.id,
        amount: Money(300_000),
        memo: String::from("ATM 提款"),
    };
    if alice.withdraw(tx2.amount) {
        println!("{} 後 → {}", tx2.memo, alice.summary());
    }

    // 提款失敗
    let tx3 = Transaction {
        id: gen.next_id(),
        account_id: alice.id,
        amount: Money::from_yuan(100_000),
        memo: String::from("買車"),
    };
    if !alice.withdraw(tx3.amount) {
        println!(
            "交易 #{} 「{}」（帳戶 {}）失敗：餘額不足",
            tx3.id, tx3.memo, tx3.account_id
        );
    }

    // Debug 輸出：{:?} 單行、{:#?} 多行縮排
    println!("{:?}", tx1);
    println!("{:#?}", alice);

    // 結構體更新語法：以現有值為基底建新值
    let bob = Account {
        id: 1002,
        owner: String::from("李小華"),
        ..alice.clone() // 其餘欄位從 alice 複製；這裡要 clone 是因為 owner 是 String
    };
    println!("{}", bob.summary());

    // newtype 的好處：下面這行不會編譯，因為 Money 和 i64 是不同型別
    // let wrong: Money = 100;
    // 也不能把 Money 和 u32 的 id 相加
    // let nonsense = alice.balance + alice.id;

    // derive 出來的比較
    println!("alice == bob ? {}", alice == bob);
    println!("Money(100) < Money(200) ? {}", Money(100) < Money(200));
    println!("餘額（分）：{}", alice.balance.cents());
}
