//! 第 5 章練習解答

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
struct Money(i64);

/// 練習 1：新增 Fee 變體後，cargo check 會在每個 match TxKind 的地方報 E0004：
/// describe()、affected_accounts()、apply() 三處（原第 5 章程式碼），involves() 用 matches! 不會報錯但邏輯會漏。
/// 這就是 enum + match 的重構安全網。
#[derive(Debug, Clone, PartialEq)]
enum TxKind {
    Deposit { account: u32 },
    Withdraw { account: u32 },
    Transfer { from: u32, to: u32 },
    Fee { account: u32 },
}

impl TxKind {
    fn affected_accounts(&self) -> Vec<u32> {
        match self {
            TxKind::Deposit { account }
            | TxKind::Withdraw { account }
            | TxKind::Fee { account } => vec![*account],
            TxKind::Transfer { from, to } => vec![*from, *to],
        }
    }
}

#[derive(Debug)]
struct Account {
    id: u32,
    balance: Money,
}

struct Ledger {
    accounts: Vec<Account>,
}

impl Ledger {
    fn find(&self, id: u32) -> Option<&Account> {
        self.accounts.iter().find(|a| a.id == id)
    }

    /// 練習 2：帳本為空時 max_by_key 回傳 None，剛好就是我們要的語義
    fn largest_account(&self) -> Option<&Account> {
        self.accounts.iter().max_by_key(|a| a.balance)
    }

    /// 練習 3：方法鏈，不用 match。filter 把「有值但不符合條件」變成 None
    fn rich_balance(&self, id: u32) -> Option<Money> {
        self.find(id)
            .map(|a| a.balance)
            .filter(|b| *b > Money(1_000_000))
    }
}

fn main() {
    let ledger = Ledger {
        accounts: vec![
            Account {
                id: 1,
                balance: Money(500),
            },
            Account {
                id: 2,
                balance: Money(2_000_000),
            },
        ],
    };
    for k in [
        TxKind::Deposit { account: 1 },
        TxKind::Withdraw { account: 1 },
        TxKind::Transfer { from: 1, to: 2 },
        TxKind::Fee { account: 1 },
    ] {
        println!("{k:?} 影響 {:?}", k.affected_accounts());
    }
    println!("{:?}", ledger.largest_account().map(|a| a.id)); // Some(2)
    println!(
        "{:?}",
        Ledger { accounts: vec![] }.largest_account().is_none()
    ); // true
    println!(
        "{:?} {:?} {:?}",
        ledger.rich_balance(1),
        ledger.rich_balance(2),
        ledger.rich_balance(9)
    );
    // None, Some(Money(2000000)), None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn largest_of_empty_is_none() {
        assert!(Ledger { accounts: vec![] }.largest_account().is_none());
    }

    #[test]
    fn rich_balance_filters() {
        let l = Ledger {
            accounts: vec![
                Account {
                    id: 1,
                    balance: Money(999_999),
                },
                Account {
                    id: 2,
                    balance: Money(1_000_001),
                },
            ],
        };
        assert_eq!(l.rich_balance(1), None);
        assert_eq!(l.rich_balance(2), Some(Money(1_000_001)));
        assert_eq!(l.rich_balance(3), None);
    }
}
