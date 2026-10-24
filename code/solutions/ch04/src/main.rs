//! 第 4 章練習解答

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
struct Money(i64);

impl Money {
    fn format(self) -> String {
        let sign = if self.0 < 0 { "-" } else { "" };
        format!("{sign}{}.{:02} 元", (self.0 / 100).abs(), (self.0 % 100).abs())
    }
}

#[derive(Debug, Clone, PartialEq)]
struct Account {
    id: u32,
    owner: String,
    balance: Money,
}

#[derive(Debug, Clone)]
struct Transaction {
    id: u64,
    account_id: u32,
    amount: Money, // 正存款、負提款（第 5 章會換成 enum）
    memo: String,
}

impl Transaction {
    /// 練習 1：正負號轉成「存入/提出」
    fn describe(&self) -> String {
        let (verb, abs) = if self.amount.0 >= 0 {
            ("存入", self.amount)
        } else {
            ("提出", Money(-self.amount.0))
        };
        format!("#{} 帳戶 {} {} {}（{}）", self.id, self.account_id, verb, abs.format(), self.memo)
    }
}

/// 練習 2：Ledger
struct Ledger {
    accounts: Vec<Account>,
    transactions: Vec<Transaction>,
}

impl Ledger {
    /// 回傳 Option<&Account> 而不是 Option<Account>：
    /// 查詢不該複製整個帳戶（owner 是 String，clone 要配置記憶體），
    /// 而且呼叫端通常只是看一眼。借用比擁有便宜，也符合「查帳不必交出帳本」。
    fn find_account(&self, id: u32) -> Option<&Account> {
        self.accounts.iter().find(|a| a.id == id)
    }
}

/// 練習 3：具名欄位版本。
/// 使用體驗：建構要寫 Money { cents: 100 } 比 Money(100) 囉嗦；存取 m.cents 比 m.0 好讀。
/// 經驗法則：只有一個欄位、欄位意義不言自明（newtype）用元組結構體；兩個以上欄位用具名。
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
struct MoneyNamed {
    cents: i64,
}

fn main() {
    let tx = Transaction { id: 1, account_id: 1001, amount: Money(5_000_000), memo: "十月薪資".into() };
    let tx2 = Transaction { id: 2, account_id: 1001, amount: Money(-300_000), memo: "ATM 提款".into() };
    println!("{}", tx.describe());
    println!("{}", tx2.describe());

    let ledger = Ledger {
        accounts: vec![Account { id: 1001, owner: "王小明".into(), balance: Money(100) }],
        transactions: vec![tx, tx2],
    };
    println!("{:?}", ledger.find_account(1001).map(|a| &a.owner));
    println!("{:?}", ledger.find_account(9999));
    println!("{} 筆交易", ledger.transactions.len());

    let m = MoneyNamed { cents: 25_050 };
    println!("{} 分", m.cents);
    let _ = ledger.accounts[0].balance;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn describe_uses_direction_words() {
        let t = Transaction { id: 7, account_id: 1, amount: Money(-250), memo: "x".into() };
        assert_eq!(t.describe(), "#7 帳戶 1 提出 2.50 元（x）");
    }

    #[test]
    fn find_account_borrows() {
        let l = Ledger { accounts: vec![Account { id: 1, owner: "a".into(), balance: Money(0) }], transactions: vec![] };
        assert_eq!(l.find_account(1).map(|a| a.id), Some(1));
        assert!(l.find_account(2).is_none());
    }
}
