//! 第 6 章練習解答

use std::fmt;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
struct Money(i64);

impl fmt::Display for Money {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}.{:02} 元", self.0 / 100, (self.0 % 100).abs())
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
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
    frozen: bool, // 練習 1
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum LedgerError {
    AccountNotFound(u32),
    AccountFrozen(u32), // 練習 1
    InsufficientFunds { account: u32, balance: Money, requested: Money },
    InvalidAmount(Money),
    SameAccountTransfer(u32),
    Overflow,
}

impl fmt::Display for LedgerError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            LedgerError::AccountNotFound(id) => write!(f, "帳戶 {id} 不存在"),
            LedgerError::AccountFrozen(id) => write!(f, "帳戶 {id} 已凍結，不能提款或轉出"),
            LedgerError::InsufficientFunds { account, balance, requested } => {
                write!(f, "帳戶 {account} 餘額不足：有 {balance}，要 {requested}")
            }
            LedgerError::InvalidAmount(m) => write!(f, "金額不合法：{m}"),
            LedgerError::SameAccountTransfer(id) => write!(f, "不能轉帳給自己（帳戶 {id}）"),
            LedgerError::Overflow => write!(f, "金額溢位"),
        }
    }
}

impl std::error::Error for LedgerError {}

struct Ledger {
    accounts: Vec<Account>,
}

impl Ledger {
    fn find_mut(&mut self, id: u32) -> Result<&mut Account, LedgerError> {
        self.accounts.iter_mut().find(|a| a.id == id).ok_or(LedgerError::AccountNotFound(id))
    }

    fn credit(&mut self, id: u32, amount: Money) -> Result<(), LedgerError> {
        // 凍結的帳戶仍可收款
        let acct = self.find_mut(id)?;
        acct.balance = Money(acct.balance.0.checked_add(amount.0).ok_or(LedgerError::Overflow)?);
        Ok(())
    }

    fn debit(&mut self, id: u32, amount: Money) -> Result<(), LedgerError> {
        let acct = self.find_mut(id)?;
        if acct.frozen {
            return Err(LedgerError::AccountFrozen(id));
        }
        if amount > acct.balance {
            return Err(LedgerError::InsufficientFunds { account: id, balance: acct.balance, requested: amount });
        }
        acct.balance = Money(acct.balance.0 - amount.0);
        Ok(())
    }

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
                self.find_mut(to)?;
                self.debit(from, tx.amount)?;
                self.credit(to, tx.amount).expect("目的帳戶已驗證");
                Ok(())
            }
        }
    }
}

// ---------- 練習 2：自訂 ParseError ----------

#[derive(Debug, PartialEq, Eq)]
enum ParseError {
    MissingField { line: String, expected: usize, got: usize },
    BadNumber { field: &'static str, value: String },
    UnknownKind(String),
}

impl fmt::Display for ParseError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ParseError::MissingField { line, expected, got } => write!(f, "欄位不足：{line:?} 需要 {expected} 個，只有 {got} 個"),
            ParseError::BadNumber { field, value } => write!(f, "欄位 {field} 不是數字：{value:?}"),
            ParseError::UnknownKind(k) => write!(f, "未知的交易種類：{k:?}"),
        }
    }
}

impl std::error::Error for ParseError {}

fn parse_line(line: &str) -> Result<Transaction, ParseError> {
    let parts: Vec<&str> = line.split(',').map(str::trim).collect();
    let need = |n: usize| -> Result<(), ParseError> {
        if parts.len() < n {
            Err(ParseError::MissingField { line: line.to_string(), expected: n, got: parts.len() })
        } else {
            Ok(())
        }
    };
    // map_err 把 ParseIntError 轉成帶欄位名稱的 BadNumber
    let num = |field: &'static str, s: &str| -> Result<i64, ParseError> {
        s.parse::<i64>().map_err(|_| ParseError::BadNumber { field, value: s.to_string() })
    };
    need(4)?;
    let id = num("id", parts[0])? as u64;
    let amount = Money(num("amount", parts[2])?);
    let kind = match parts[1] {
        "deposit" => TxKind::Deposit { account: num("account", parts[3])? as u32 },
        "withdraw" => TxKind::Withdraw { account: num("account", parts[3])? as u32 },
        "transfer" => {
            need(5)?;
            TxKind::Transfer { from: num("from", parts[3])? as u32, to: num("to", parts[4])? as u32 }
        }
        other => return Err(ParseError::UnknownKind(other.to_string())),
    };
    Ok(Transaction { id, kind, amount })
}

// ---------- 練習 3：main 回傳 Result，任何一行解析失敗就整個失敗 ----------
// 這種「全有或全無」策略適合互動式工具：使用者可以立刻修正輸入再跑。
// 夜間批次則適合第 6 章原本的策略：壞的跳過並記錄，其餘照常處理，否則一行壞資料會讓整晚結算停擺。

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut ledger = Ledger {
        accounts: vec![
            Account { id: 1001, balance: Money(5_000_000), frozen: false },
            Account { id: 1002, balance: Money(200_000), frozen: true },
        ],
    };

    let raw = ["1, deposit, 100000, 1002", "2, withdraw, 1000, 1002", "3, transfer, 500, 1002, 1001", "4, transfer, 500, 1001, 1002"];
    let txs: Vec<Transaction> = raw.iter().map(|l| parse_line(l)).collect::<Result<_, _>>()?; // 一行壞就 ?

    for tx in &txs {
        match ledger.apply(tx) {
            Ok(()) => println!("#{} 成功", tx.id),
            Err(e) => println!("#{} 被拒：{e}", tx.id),
        }
    }

    // 練習 2 的錯誤型別示範
    for bad in ["7, deposit, abc, 1001", "8, refund, 100, 1001", "9, deposit"] {
        println!("{}", parse_line(bad).unwrap_err());
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ledger() -> Ledger {
        Ledger { accounts: vec![Account { id: 1, balance: Money(1_000), frozen: true }, Account { id: 2, balance: Money(1_000), frozen: false }] }
    }

    #[test]
    fn frozen_cannot_withdraw_or_send_but_can_receive() {
        let mut l = ledger();
        assert_eq!(l.apply(&Transaction { id: 1, kind: TxKind::Withdraw { account: 1 }, amount: Money(1) }), Err(LedgerError::AccountFrozen(1)));
        assert_eq!(l.apply(&Transaction { id: 2, kind: TxKind::Transfer { from: 1, to: 2 }, amount: Money(1) }), Err(LedgerError::AccountFrozen(1)));
        assert_eq!(l.apply(&Transaction { id: 3, kind: TxKind::Transfer { from: 2, to: 1 }, amount: Money(1) }), Ok(()));
        assert_eq!(l.apply(&Transaction { id: 4, kind: TxKind::Deposit { account: 1 }, amount: Money(1) }), Ok(()));
    }

    #[test]
    fn parse_errors_are_specific() {
        assert!(matches!(parse_line("1, deposit, x, 1"), Err(ParseError::BadNumber { field: "amount", .. })));
        assert!(matches!(parse_line("1, refund, 1, 1"), Err(ParseError::UnknownKind(_))));
        assert!(matches!(parse_line("1, transfer, 1, 1"), Err(ParseError::MissingField { expected: 5, got: 4, .. })));
    }
}
