//! 第 13 章：櫃台模擬。
//!
//! 情境：同一個程式裡有「櫃台」「稽核」「風控」三個元件，都要存取同一個帳本。
//! 櫃台要改（過帳），稽核與風控只讀。全部在同一個執行緒（多執行緒版本在第 14 章）。
//!
//! 執行：cargo run --bin desk

use std::cell::RefCell;
use std::rc::{Rc, Weak};

use chrono::NaiveDate;
use ledger::{Ledger, Money, Rule, Transaction, TxKind};

/// 櫃台：需要修改帳本
struct Teller {
    name: String,
    ledger: Rc<RefCell<Ledger>>, // 共享所有權 + 內部可變性
    next_id: u64,
}

impl Teller {
    fn post(&mut self, kind: TxKind, amount: Money, memo: &str) {
        let tx = Transaction {
            id: self.next_id,
            date: NaiveDate::from_ymd_opt(2026, 10, 20).unwrap(),
            kind,
            amount,
            memo: memo.to_string(),
        };
        self.next_id += 1;
        // borrow_mut：執行期取得 &mut Ledger。這一行結束時借用自動歸還。
        match self.ledger.borrow_mut().apply(tx) {
            Ok(()) => println!("[{}] 過帳成功：{kind} {amount} 元", self.name),
            Err(e) => println!("[{}] 過帳失敗：{e}", self.name),
        }
    }
}

/// 稽核：只讀。練習 1：記住上次看到的歷史長度，用新增的交易重算預期總額。
struct Auditor {
    ledger: Rc<RefCell<Ledger>>,
    seen: usize,
    expected_total: Money,
}

impl Auditor {
    fn check(&mut self) {
        let ledger = self.ledger.borrow(); // 執行期取得 &Ledger
        for tx in &ledger.history()[self.seen..] {
            self.expected_total = match tx.kind {
                TxKind::Deposit { .. } => self.expected_total + tx.amount,
                TxKind::Withdraw { .. } => self.expected_total - tx.amount,
                TxKind::Transfer { .. } => self.expected_total,
            };
        }
        self.seen = ledger.history().len();
        let total = ledger.total_balance();
        let ok = total == self.expected_total;
        println!(
            "[稽核] 總額 {total} 元，預期 {} 元，{}",
            self.expected_total,
            if ok { "正確" } else { "異常！" }
        );
    }
}

/// 風控：只讀，持有規則樹
struct RiskMonitor {
    ledger: Weak<RefCell<Ledger>>, // Weak：不擁有，帳本被釋放後 upgrade 會得到 None
    rule: Rule,
}

impl RiskMonitor {
    fn scan(&self) {
        // upgrade：試著把 Weak 變回 Rc。帳本還在就 Some
        let Some(ledger) = self.ledger.upgrade() else {
            println!("[風控] 帳本已不存在，停止掃描");
            return;
        };
        let ledger = ledger.borrow();
        let hits: Vec<u64> = ledger
            .history()
            .iter()
            .filter(|tx| self.rule.matches(tx))
            .map(|tx| tx.id)
            .collect();
        println!("[風控] 規則深度 {}，命中交易：{hits:?}", self.rule.depth());
    }
}

fn main() {
    let mut ledger = Ledger::new();
    ledger
        .open(1001, "王小明", Money::from_yuan(50_000))
        .unwrap();
    ledger
        .open(1002, "李小華", Money::from_yuan(2_000))
        .unwrap();
    let initial_total = ledger.total_balance();

    // 把帳本放進 Rc<RefCell<>>。從這一刻起，ledger 變數的所有權移進去了。
    let shared = Rc::new(RefCell::new(ledger));
    println!("Rc 計數：{}", Rc::strong_count(&shared)); // 1

    let mut teller_a = Teller {
        name: "櫃台 A".into(),
        ledger: Rc::clone(&shared),
        next_id: 1,
    };
    let mut teller_b = Teller {
        name: "櫃台 B".into(),
        ledger: Rc::clone(&shared),
        next_id: 1000,
    };
    let mut auditor = Auditor {
        ledger: Rc::clone(&shared),
        seen: 0,
        expected_total: initial_total,
    };
    let risk = RiskMonitor {
        ledger: Rc::downgrade(&shared), // Weak 不增加 strong count
        rule: Rule::AmountAbove(Money::from_yuan(1_000)).and(Rule::IsWithdraw.or(Rule::IsTransfer)),
    };
    println!(
        "Rc 計數：{}（strong），{}（weak）",
        Rc::strong_count(&shared),
        Rc::weak_count(&shared)
    );

    // 櫃台輪流過帳，稽核與風控隨時可以看
    teller_a.post(
        TxKind::Deposit { account: 1001 },
        Money::from_yuan(100),
        "利息",
    );
    teller_b.post(
        TxKind::Withdraw { account: 1002 },
        Money::from_yuan(1_500),
        "提領",
    );
    auditor.check(); // 練習 1 修正後：預期總額跟著歷史走，應該顯示「正確」
    teller_a.post(
        TxKind::Transfer {
            from: 1001,
            to: 1002,
        },
        Money::from_yuan(3_000),
        "借款",
    );
    teller_b.post(
        TxKind::Withdraw { account: 1002 },
        Money::from_yuan(99_999),
        "買車",
    );
    risk.scan();
    auditor.check();

    // 練習 3：會 panic 的情境與修法。
    // 錯誤版本（打開會 panic: already borrowed: BorrowMutError）：
    //   let l = shared.borrow();          // 讀餘額
    //   let bal = l.balance_of(1001);
    //   shared.borrow_mut().apply(...);   // l 還活著 → panic
    // 正確版本：把讀的部分放進小區塊，借用在區塊結束時歸還。
    let bal = {
        let l = shared.borrow();
        l.balance_of(1001).unwrap()
    }; // l 在這裡釋放
    shared
        .borrow_mut()
        .apply(Transaction {
            id: 9_000,
            date: NaiveDate::from_ymd_opt(2026, 10, 20).unwrap(),
            kind: TxKind::Deposit { account: 1001 },
            amount: Money::from_yuan(1),
            memo: format!("讀到餘額 {bal} 後再寫入"),
        })
        .unwrap();
    println!("[練習 3] 先讀（{bal}）再寫，分開借用，不會 panic");

    // 練習 4
    for m in ["房租 十月", "  房租   十月  "] {
        let n = ledger::text::normalize_memo(m);
        println!(
            "[練習 4] {m:?} → {n:?}（{}）",
            if matches!(n, std::borrow::Cow::Borrowed(_)) {
                "借用，零配置"
            } else {
                "配置了新字串"
            }
        );
    }

    // 內部可變性的代價：借用規則從編譯期搬到執行期。
    // 下面這段如果真的執行會 panic：already borrowed: BorrowMutError
    {
        let _reader = shared.borrow();
        // let mut writer = shared.borrow_mut(); // ← panic!
        // 安全的寫法是 try_borrow_mut：
        match shared.try_borrow_mut() {
            Ok(_) => println!("取得可變借用"),
            Err(_) => println!(
                "try_borrow_mut 失敗：目前已有讀者（這就是第 3 章的借用規則，只是在執行期檢查）"
            ),
        }
    } // _reader 在這裡釋放

    // 釋放所有強引用之後，Weak 就 upgrade 不到了
    drop(teller_a);
    drop(teller_b);
    drop(auditor);
    println!("Rc 計數：{}", Rc::strong_count(&shared)); // 1：只剩 shared 自己
    drop(shared); // 最後一個強引用釋放，帳本被 drop
    risk.scan(); // Weak::upgrade 得到 None

    // Box<dyn Trait>：第 9 章已經用過。這裡看 Box 的另外兩個用途：
    // 1. 遞迴型別（Rule 裡面的 Box<Rule>）—— 上面已示範
    // 2. 把大型資料放到堆積，只在堆疊上搬一個指標
    let big: Box<[u8; 1_000_000]> = Box::new([0u8; 1_000_000]);
    println!(
        "堆疊上的 Box 只有 {} bytes，指向堆積上的 {} bytes",
        std::mem::size_of_val(&big),
        big.len()
    );
}
