//! 練習 5：用 proptest 重寫不變量測試。
//! proptest 會自動產生輸入、失敗時縮小到最小反例，並把反例存進 proptest-regressions/ 讓它永遠重跑。

mod common;

use common::seeded_ledger;
use ledger::{Money, Transaction, TxKind};
use proptest::prelude::*;

/// 描述「一筆隨機交易」怎麼產生：這是 proptest 的 Strategy
fn arb_tx() -> impl Strategy<Value = Transaction> {
    let account = prop_oneof![Just(1001u32), Just(1002), Just(1003), Just(9999)];
    let kind = prop_oneof![
        account.clone().prop_map(|a| TxKind::Deposit { account: a }),
        account.clone().prop_map(|a| TxKind::Withdraw { account: a }),
        (account.clone(), account).prop_map(|(f, t)| TxKind::Transfer { from: f, to: t }),
    ];
    (any::<u64>(), 1u32..=30, kind, -10_000i64..3_000_000).prop_map(|(id, day, kind, cents)| Transaction {
        id,
        day,
        kind,
        amount: Money::from_cents(cents),
        memo: String::new(),
    })
}

proptest! {
    // 預設跑 256 次，每次一個隨機的交易序列（長度 0..200）
    #[test]
    fn balances_never_negative_and_total_is_conserved(txs in prop::collection::vec(arb_tx(), 0..200)) {
        let mut l = seeded_ledger();
        let mut expected = l.total_balance();
        for tx in txs {
            let (kind, amount) = (tx.kind, tx.amount);
            if l.apply(tx).is_ok() {
                expected = match kind {
                    TxKind::Deposit { .. } => expected + amount,
                    TxKind::Withdraw { .. } => expected - amount,
                    TxKind::Transfer { .. } => expected,
                };
            }
        }
        for a in l.accounts() {
            prop_assert!(a.balance() >= Money::ZERO, "帳戶 {} 餘額為負", a.id);
        }
        prop_assert_eq!(l.total_balance(), expected);
    }

    // 另一個性質：失敗的 apply 不改變任何帳戶
    #[test]
    fn failed_apply_changes_nothing(tx in arb_tx()) {
        let mut l = seeded_ledger();
        let before = l.clone();
        if l.apply(tx).is_err() {
            prop_assert_eq!(l, before);
        }
    }
}
