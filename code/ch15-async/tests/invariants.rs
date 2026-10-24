//! 不變量測試：用隨機產生的交易序列檢查「無論發生什麼，某些事永遠成立」。
//! 這是屬性式測試（property-based testing）的手工版；正式專案用 proptest 或 quickcheck crate。

mod common;

use common::{day, seeded_ledger};
use ledger::{Money, Transaction, TxKind};

/// 一個極簡的偽隨機數產生器（LCG），讓測試可重現、不需要外部 crate
struct Lcg(u64);
impl Lcg {
    fn next(&mut self) -> u64 {
        self.0 = self
            .0
            .wrapping_mul(6364136223846793005)
            .wrapping_add(1442695040888963407);
        self.0 >> 33
    }
    fn pick<T: Copy>(&mut self, items: &[T]) -> T {
        items[(self.next() as usize) % items.len()]
    }
}

fn random_tx(rng: &mut Lcg, id: u64) -> Transaction {
    let ids = [1001u32, 1002, 1003, 9999]; // 9999 不存在，製造失敗
    let kind = match rng.next() % 3 {
        0 => TxKind::Deposit {
            account: rng.pick(&ids),
        },
        1 => TxKind::Withdraw {
            account: rng.pick(&ids),
        },
        _ => TxKind::Transfer {
            from: rng.pick(&ids),
            to: rng.pick(&ids),
        },
    };
    // 金額含 0 與負數，製造 InvalidAmount
    let cents = (rng.next() % 3_000_000) as i64 - 10_000;
    Transaction {
        id,
        date: day((rng.next() % 28 + 1) as u32),
        kind,
        amount: Money::from_cents(cents),
        memo: String::new(),
    }
}

#[test]
fn balances_never_negative_and_total_is_conserved() {
    for seed in 0..20u64 {
        let mut rng = Lcg(seed);
        let mut l = seeded_ledger();
        let initial_total = l.total_balance();

        let mut expected_total = initial_total;
        for id in 1..=500u64 {
            let tx = random_tx(&mut rng, id);
            let kind = tx.kind;
            let amount = tx.amount;
            if l.apply(tx).is_ok() {
                // 只有成功的交易會改變總額，而且改變量是確定的
                expected_total = match kind {
                    TxKind::Deposit { .. } => expected_total + amount,
                    TxKind::Withdraw { .. } => expected_total - amount,
                    TxKind::Transfer { .. } => expected_total,
                };
            }
        }

        for acct in l.accounts() {
            assert!(
                acct.balance() >= Money::ZERO,
                "seed {seed}: 帳戶 {} 餘額為負",
                acct.id
            );
        }
        assert_eq!(l.total_balance(), expected_total, "seed {seed}: 總額不守恆");
        assert_eq!(
            l.history().len(),
            l.history()
                .iter()
                .map(|t| t.id)
                .collect::<std::collections::HashSet<_>>()
                .len(),
            "seed {seed}: 歷史中有重複的交易 ID"
        );
    }
}
