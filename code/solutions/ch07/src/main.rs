//! 第 7 章練習解答

use std::collections::HashMap;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
struct Money(i64);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
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

impl Transaction {
    fn involves(&self, id: u32) -> bool {
        match self.kind {
            TxKind::Deposit { account } | TxKind::Withdraw { account } => account == id,
            TxKind::Transfer { from, to } => from == id || to == id,
        }
    }
}

#[derive(Debug)]
struct Account {
    id: u32,
    balance: Money,
}

struct Ledger {
    accounts: HashMap<u32, Account>,
    history: Vec<Transaction>,
    initial_total: Money, // 練習 2 需要
}

impl Ledger {
    fn new(accounts: Vec<Account>) -> Self {
        let initial_total = Money(accounts.iter().map(|a| a.balance.0).sum());
        Self { accounts: accounts.into_iter().map(|a| (a.id, a)).collect(), history: Vec::new(), initial_total }
    }

    fn apply(&mut self, tx: Transaction) -> bool {
        let ok = match tx.kind {
            TxKind::Deposit { account } => self.accounts.get_mut(&account).map(|a| a.balance.0 += tx.amount.0).is_some(),
            TxKind::Withdraw { account } => match self.accounts.get_mut(&account) {
                Some(a) if tx.amount <= a.balance => {
                    a.balance.0 -= tx.amount.0;
                    true
                }
                _ => false,
            },
            TxKind::Transfer { from, to } if from != to => match self.accounts.get_disjoint_mut([&from, &to]) {
                [Some(src), Some(dst)] if tx.amount <= src.balance => {
                    src.balance.0 -= tx.amount.0;
                    dst.balance.0 += tx.amount.0;
                    true
                }
                _ => false,
            },
            TxKind::Transfer { .. } => false,
        };
        if ok {
            self.history.push(tx);
        }
        ok
    }

    /// 練習 1：與帳戶有關的交易，金額降冪
    fn monthly_statement(&self, account: u32) -> Vec<&Transaction> {
        let mut v: Vec<&Transaction> = self.history.iter().filter(|t| t.involves(account)).collect();
        v.sort_by_key(|t| std::cmp::Reverse(t.amount));
        v
    }

    /// 練習 2：總額守恆檢查
    fn balance_check(&self) -> bool {
        let deposits: i64 = self.history.iter().filter(|t| matches!(t.kind, TxKind::Deposit { .. })).map(|t| t.amount.0).sum();
        let withdrawals: i64 = self.history.iter().filter(|t| matches!(t.kind, TxKind::Withdraw { .. })).map(|t| t.amount.0).sum();
        let total: i64 = self.accounts.values().map(|a| a.balance.0).sum();
        total == self.initial_total.0 + deposits - withdrawals
    }
}

fn main() {
    let mut ledger = Ledger::new(vec![Account { id: 1, balance: Money(10_000) }, Account { id: 2, balance: Money(500) }]);
    let txs: Vec<Transaction> = (1..=10u64)
        .map(|i| Transaction {
            id: i,
            kind: match i % 3 {
                0 => TxKind::Deposit { account: 1 },
                1 => TxKind::Withdraw { account: 2 },
                _ => TxKind::Transfer { from: 1, to: 2 },
            },
            amount: Money(i as i64 * 100),
        })
        .collect();

    // 練習 3：用普通 for 迴圈過帳，比 partition 裡做副作用清楚得多
    let mut rejected = Vec::new();
    for tx in txs {
        let id = tx.id;
        if !ledger.apply(tx) {
            rejected.push(id);
        }
    }
    println!("拒絕：{rejected:?}");

    println!("帳戶 2 的對帳單（金額降冪）：");
    for t in ledger.monthly_statement(2) {
        println!("  #{} {:?} {}", t.id, t.kind, t.amount.0);
    }
    println!("總額守恆：{}", ledger.balance_check());

    // 練習 4：chunks 分批小計
    for (i, chunk) in ledger.history.chunks(3).enumerate() {
        let subtotal: i64 = chunk.iter().map(|t| t.amount.0).sum();
        println!("第 {} 批（{} 筆）小計 {}", i + 1, chunk.len(), subtotal);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn statement_sorted_desc_and_filtered() {
        let mut l = Ledger::new(vec![Account { id: 1, balance: Money(1_000) }, Account { id: 2, balance: Money(0) }]);
        l.apply(Transaction { id: 1, kind: TxKind::Transfer { from: 1, to: 2 }, amount: Money(100) });
        l.apply(Transaction { id: 2, kind: TxKind::Deposit { account: 1 }, amount: Money(999) });
        l.apply(Transaction { id: 3, kind: TxKind::Withdraw { account: 2 }, amount: Money(50) });
        let ids: Vec<u64> = l.monthly_statement(2).iter().map(|t| t.id).collect();
        assert_eq!(ids, vec![1, 3]);
        assert!(l.balance_check());
    }
}
