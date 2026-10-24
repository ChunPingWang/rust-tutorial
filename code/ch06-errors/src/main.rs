//! 第 6 章：錯誤處理
//!
//! 需求：不合法的交易要被拒絕，而且要說清楚「為什麼」。
//! 批次處理一萬筆交易時，一筆失敗不能讓整個批次崩潰。

use std::fmt;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
struct Money(i64);

impl Money {
    fn format(self) -> String {
        let yuan = self.0 / 100;
        let fen = (self.0 % 100).abs();
        format!("{yuan}.{fen:02} 元")
    }
}

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
}

#[derive(Debug)]
struct Account {
    id: u32,
    balance: Money,
}

/// 帳本可能發生的每一種錯誤。這是 API 的一部分：呼叫者可以針對不同錯誤做不同處理。
#[derive(Debug, Clone, PartialEq, Eq)]
enum LedgerError {
    AccountNotFound(u32),
    InsufficientFunds {
        account: u32,
        balance: Money,
        requested: Money,
    },
    InvalidAmount(Money),
    SameAccountTransfer(u32),
    Overflow,
}

/// 讓錯誤能印成人看得懂的訊息
impl fmt::Display for LedgerError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            LedgerError::AccountNotFound(id) => write!(f, "帳戶 {id} 不存在"),
            LedgerError::InsufficientFunds {
                account,
                balance,
                requested,
            } => write!(
                f,
                "帳戶 {account} 餘額不足：有 {}，要 {}",
                balance.format(),
                requested.format()
            ),
            LedgerError::InvalidAmount(m) => write!(f, "金額不合法：{}", m.format()),
            LedgerError::SameAccountTransfer(id) => write!(f, "不能轉帳給自己（帳戶 {id}）"),
            LedgerError::Overflow => write!(f, "金額溢位"),
        }
    }
}

/// 標記這是一個「錯誤型別」。空的 impl 就夠了，Debug + Display 是它的前提。
impl std::error::Error for LedgerError {}

struct Ledger {
    accounts: Vec<Account>,
}

impl Ledger {
    /// Option → Result：找不到就變成一個有意義的錯誤。ok_or 是最常用的橋。
    fn find_mut(&mut self, id: u32) -> Result<&mut Account, LedgerError> {
        self.accounts
            .iter_mut()
            .find(|a| a.id == id)
            .ok_or(LedgerError::AccountNotFound(id))
    }

    fn balance_of(&self, id: u32) -> Result<Money, LedgerError> {
        self.accounts
            .iter()
            .find(|a| a.id == id)
            .map(|a| a.balance)
            .ok_or(LedgerError::AccountNotFound(id))
    }

    fn credit(&mut self, id: u32, amount: Money) -> Result<(), LedgerError> {
        let acct = self.find_mut(id)?; // ? ：錯了就直接回傳錯誤
        let new_balance = acct
            .balance
            .0
            .checked_add(amount.0)
            .ok_or(LedgerError::Overflow)?;
        acct.balance = Money(new_balance);
        Ok(())
    }

    fn debit(&mut self, id: u32, amount: Money) -> Result<(), LedgerError> {
        let acct = self.find_mut(id)?;
        if amount > acct.balance {
            return Err(LedgerError::InsufficientFunds {
                account: id,
                balance: acct.balance,
                requested: amount,
            });
        }
        acct.balance = Money(acct.balance.0 - amount.0);
        Ok(())
    }

    /// 過帳。成功回傳 ()，失敗回傳為什麼。
    fn apply(&mut self, tx: &Transaction) -> Result<(), LedgerError> {
        if tx.amount.0 <= 0 {
            return Err(LedgerError::InvalidAmount(tx.amount));
        }
        match tx.kind {
            TxKind::Deposit { account } => self.credit(account, tx.amount),
            TxKind::Withdraw { account } => self.debit(account, tx.amount),
            TxKind::Transfer { from, to } => {
                if from == to {
                    return Err(LedgerError::SameAccountTransfer(from));
                }
                // 先驗證再修改：確保失敗時帳本不會只改了一半
                self.balance_of(to)?;
                self.debit(from, tx.amount)?;
                self.credit(to, tx.amount)
                    .expect("目的帳戶剛驗證過存在，溢位以外不該失敗");
                Ok(())
            }
        }
    }
}

/// 批次處理的結果統計
#[derive(Debug, Default)]
struct BatchReport {
    posted: usize,
    rejected: Vec<(u64, LedgerError)>,
}

fn run_batch(ledger: &mut Ledger, txs: &[Transaction]) -> BatchReport {
    let mut report = BatchReport::default();
    for tx in txs {
        match ledger.apply(tx) {
            Ok(()) => report.posted += 1,
            Err(e) => report.rejected.push((tx.id, e)),
        }
    }
    report
}

/// 解析一行「kind,account,amount」文字。示範把不同來源的錯誤統一成 Box<dyn Error>。
fn parse_line(line: &str) -> Result<Transaction, Box<dyn std::error::Error>> {
    let parts: Vec<&str> = line.split(',').map(|s| s.trim()).collect();
    if parts.len() < 3 {
        return Err(format!("欄位不足：{line:?}").into()); // String → Box<dyn Error>
    }
    let id: u64 = parts[0].parse()?; // ParseIntError → Box<dyn Error>，自動轉換
    let amount = Money(parts[2].parse::<i64>()?);
    let kind = match parts[1] {
        "deposit" => TxKind::Deposit {
            account: parts[3].parse()?,
        },
        "withdraw" => TxKind::Withdraw {
            account: parts[3].parse()?,
        },
        "transfer" => TxKind::Transfer {
            from: parts[3].parse()?,
            to: parts[4].parse()?,
        },
        other => return Err(format!("未知的交易種類：{other}").into()),
    };
    Ok(Transaction { id, kind, amount })
}

fn main() {
    let mut ledger = Ledger {
        accounts: vec![
            Account {
                id: 1001,
                balance: Money(5_000_000),
            },
            Account {
                id: 1002,
                balance: Money(200_000),
            },
        ],
    };

    // 這些是從檔案讀進來的原始文字（第 12 章會真的讀 CSV）
    let raw = [
        "1, deposit,  100000, 1001",
        "2, transfer, 1500000, 1001, 1002",
        "3, withdraw, 9999999, 1002",
        "4, deposit,  1, 9999",
        "5, transfer, 100, 1001, 1001",
        "6, deposit, -500, 1001",
        "7, deposit, abc, 1001",
        "8, refund, 100, 1001",
    ];

    // 解析階段：解析失敗的直接報告，不進入過帳
    let mut txs = Vec::new();
    for line in raw {
        match parse_line(line) {
            Ok(tx) => txs.push(tx),
            Err(e) => println!("解析失敗 [{line}]：{e}"),
        }
    }

    // 過帳階段
    let report = run_batch(&mut ledger, &txs);
    println!(
        "\n過帳成功：{} 筆，拒絕：{} 筆",
        report.posted,
        report.rejected.len()
    );
    for (id, err) in &report.rejected {
        println!("  交易 #{id} 被拒：{err}");
    }

    // 呼叫端可以針對特定錯誤做事
    for (id, err) in &report.rejected {
        if let LedgerError::InsufficientFunds { account, .. } = err {
            println!("  → 通知帳戶 {account} 的持有人：交易 #{id} 因餘額不足失敗");
        }
    }

    println!();
    for acct in &ledger.accounts {
        println!("#{}：{}", acct.id, acct.balance.format());
    }

    // 什麼時候該 panic：程式邏輯錯誤，不是輸入錯誤
    let total: i64 = ledger.accounts.iter().map(|a| a.balance.0).sum();
    assert!(total >= 0, "帳本總額為負，這是程式 bug，不是使用者錯誤");
}
