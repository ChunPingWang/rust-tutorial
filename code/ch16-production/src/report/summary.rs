use std::collections::{BTreeMap, HashMap};

use chrono::NaiveDate;

use crate::{Ledger, Money, Transaction, TxKind};

pub fn total_deposits(ledger: &Ledger) -> Money {
    ledger
        .history()
        .iter()
        .filter(|tx| matches!(tx.kind, TxKind::Deposit { .. }))
        .map(|tx| tx.amount)
        .sum()
}

pub fn outflow_by_account(ledger: &Ledger) -> HashMap<u32, Money> {
    let mut map: HashMap<u32, Money> = HashMap::new();
    for tx in ledger.history() {
        let debited = match tx.kind {
            TxKind::Withdraw { account } => Some(account),
            TxKind::Transfer { from, .. } => Some(from),
            TxKind::Deposit { .. } => None,
        };
        if let Some(id) = debited {
            *map.entry(id).or_insert(Money::ZERO) += tx.amount;
        }
    }
    map
}

pub fn daily_count(ledger: &Ledger) -> BTreeMap<NaiveDate, usize> {
    let mut map = BTreeMap::new();
    for tx in ledger.history() {
        *map.entry(tx.date).or_insert(0) += 1;
    }
    map
}

pub fn top_n(ledger: &Ledger, n: usize) -> Vec<&Transaction> {
    let mut sorted: Vec<&Transaction> = ledger.history().iter().collect();
    sorted.sort_by_key(|tx| std::cmp::Reverse(tx.amount));
    sorted.truncate(n);
    sorted
}

pub fn ranking(ledger: &Ledger) -> Vec<(u32, &str, Money)> {
    let mut v: Vec<(u32, &str, Money)> = ledger
        .accounts()
        .map(|a| (a.id, a.owner.as_str(), a.balance()))
        .collect();
    v.sort_by_key(|&(_, _, bal)| std::cmp::Reverse(bal));
    v
}
