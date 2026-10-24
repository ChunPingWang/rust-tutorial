//! 整合測試：從外部使用者的角度測整個 crate 的公開 API。
//! tests/ 底下每個 .rs 檔都是一個獨立的 crate，只能用 `ledger::` 公開的東西。

mod common;

use common::{seeded_ledger, tx};
use ledger::report;
use ledger::{InMemoryStorage, LedgerError, Money, Storage, TextFileStorage, TxKind};

#[test]
fn end_to_end_batch_then_report() {
    let mut l = seeded_ledger();
    let rejected = l.apply_all(vec![
        tx(1, 1, TxKind::Deposit { account: 1001 }, 1_000),
        tx(
            2,
            1,
            TxKind::Transfer {
                from: 1001,
                to: 1003,
            },
            500,
        ),
        tx(3, 2, TxKind::Withdraw { account: 1002 }, 5_000), // 餘額不足
        tx(4, 2, TxKind::Withdraw { account: 1002 }, 100),
    ]);

    assert_eq!(rejected.len(), 1);
    assert!(matches!(
        rejected[0],
        (3, LedgerError::InsufficientFunds { .. })
    ));

    assert_eq!(report::total_deposits(&l), Money::from_yuan(1_000));
    assert_eq!(
        l.total_balance(),
        Money::from_yuan(50_000 + 2_000 + 800 + 1_000 - 100)
    );

    let daily = report::daily_count(&l);
    assert_eq!(daily.get(&1), Some(&2));
    assert_eq!(daily.get(&2), Some(&1));

    let top = report::top_n(&l, 1);
    assert_eq!(top[0].id, 1);
}

#[test]
fn statement_reflects_only_requested_account_and_period() {
    let mut l = seeded_ledger();
    l.apply_all(vec![
        tx(1, 1, TxKind::Deposit { account: 1001 }, 100),
        tx(
            2,
            2,
            TxKind::Transfer {
                from: 1001,
                to: 1002,
            },
            30,
        ),
        tx(3, 3, TxKind::Withdraw { account: 1002 }, 10),
        tx(4, 5, TxKind::Deposit { account: 1001 }, 1),
    ]);
    let stmt = l.statement(1001, 1, 3).unwrap();
    assert_eq!(
        stmt.entries.iter().map(|t| t.id).collect::<Vec<_>>(),
        vec![1, 2]
    );
    assert_eq!(stmt.net_change(), Money::from_yuan(70));

    let stmt2 = l.statement(1002, 1, 3).unwrap();
    assert_eq!(stmt2.net_change(), Money::from_yuan(20));
}

#[test]
fn storage_round_trip_preserves_ledger() {
    let mut l = seeded_ledger();
    l.apply_all(vec![
        tx(1, 1, TxKind::Deposit { account: 1001 }, 1),
        tx(
            2,
            1,
            TxKind::Transfer {
                from: 1001,
                to: 1002,
            },
            2,
        ),
    ]);

    let mut mem = InMemoryStorage::default();
    mem.save(&l).unwrap();
    assert_eq!(mem.load().unwrap(), l);

    // 用暫存目錄，測試不會留下垃圾、也不會互相干擾
    let path = std::env::temp_dir().join(format!("ledger-test-{}.txt", std::process::id()));
    let mut file = TextFileStorage::new(&path);
    file.save(&l).unwrap();
    let restored = file.load().unwrap();
    std::fs::remove_file(&path).ok();
    assert_eq!(restored, l);
}

#[test]
fn loading_missing_file_is_not_found() {
    let storage = TextFileStorage::new("/definitely/not/here/ledger.txt");
    assert!(matches!(
        storage.load(),
        Err(ledger::StorageError::NotFound)
    ));
}

#[test]
fn loading_corrupt_file_reports_line_number() {
    let path = std::env::temp_dir().join(format!("ledger-corrupt-{}.txt", std::process::id()));
    std::fs::write(&path, "A,1001,王小明,100\nGARBAGE\n").unwrap();
    let result = TextFileStorage::new(&path).load();
    std::fs::remove_file(&path).ok();
    match result {
        Err(ledger::StorageError::Corrupt { line, .. }) => assert_eq!(line, 2),
        other => panic!("預期 Corrupt 錯誤，得到 {other:?}"),
    }
}
