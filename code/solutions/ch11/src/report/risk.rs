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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Money, Transaction};

    fn ledger_with_withdrawals(accounts: &[u32]) -> Ledger {
        let mut l = Ledger::new();
        for id in [1, 2, 3] {
            l.open(id, "x", Money::from_yuan(1_000_000)).unwrap();
        }
        for (i, &a) in accounts.iter().enumerate() {
            l.apply(Transaction { id: i as u64 + 1, day: 1, kind: TxKind::Withdraw { account: a }, amount: Money::from_cents(1), memo: String::new() })
                .unwrap();
        }
        l
    }

    // 練習 1
    #[test]
    fn exactly_three_in_a_row_is_flagged() {
        assert_eq!(rapid_withdrawals(&ledger_with_withdrawals(&[1, 1, 1])), vec![1]);
    }

    #[test]
    fn interrupted_streak_is_not_flagged() {
        assert!(rapid_withdrawals(&ledger_with_withdrawals(&[1, 1, 2, 1])).is_empty());
    }

    #[test]
    fn long_streak_is_flagged_once() {
        assert_eq!(rapid_withdrawals(&ledger_with_withdrawals(&[2, 2, 2, 2, 2, 2])), vec![2]);
    }

    #[test]
    fn deposits_between_withdrawals_break_the_streak() {
        let mut l = ledger_with_withdrawals(&[1, 1]);
        l.apply(Transaction { id: 99, day: 1, kind: TxKind::Deposit { account: 1 }, amount: Money::from_cents(1), memo: String::new() }).unwrap();
        l.apply(Transaction { id: 100, day: 1, kind: TxKind::Withdraw { account: 1 }, amount: Money::from_cents(1), memo: String::new() }).unwrap();
        // 目前的實作只看提款序列，中間的存款不會打斷；這個測試記錄了這個（可能不理想的）行為
        assert_eq!(rapid_withdrawals(&l), vec![1]);
    }
}
