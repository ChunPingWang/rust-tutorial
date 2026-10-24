use std::collections::HashSet;

use crate::{Ledger, TxKind};

/// 同一帳戶連續三筆以上提款的帳戶清單
pub fn rapid_withdrawals(ledger: &Ledger) -> Vec<u32> {
    let withdraws: Vec<u32> = ledger
        .history()
        .iter()
        .filter_map(|tx| match tx.kind {
            TxKind::Withdraw { account } => Some(account),
            _ => None,
        })
        .collect();
    let mut flagged: Vec<u32> = withdraws
        .windows(3)
        .filter(|w| w[0] == w[1] && w[1] == w[2])
        .map(|w| w[0])
        .collect::<HashSet<_>>()
        .into_iter()
        .collect();
    flagged.sort();
    flagged
}
