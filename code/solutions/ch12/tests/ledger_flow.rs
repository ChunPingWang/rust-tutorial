//! 整合測試：從外部使用者的角度測整個 crate 的公開 API。
//! tests/ 底下每個 .rs 檔都是一個獨立的 crate，只能用 `ledger::` 公開的東西。

mod common;

use common::{day, seeded_ledger, tx};
use ledger::report;
use ledger::{InMemoryStorage, JsonFileStorage, LedgerError, Money, Storage, TxKind};

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
    assert_eq!(daily.get(&day(1)), Some(&2));
    assert_eq!(daily.get(&day(2)), Some(&1));

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
    let stmt = l.statement(1001, day(1), day(3)).unwrap();
    assert_eq!(
        stmt.entries.iter().map(|t| t.id).collect::<Vec<_>>(),
        vec![1, 2]
    );
    assert_eq!(stmt.net_change(), Money::from_yuan(70));

    let stmt2 = l.statement(1002, day(1), day(3)).unwrap();
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

    // tempfile crate（dev-dependency）：暫存目錄在 drop 時自動刪除
    let dir = tempfile::tempdir().unwrap();
    let mut file = JsonFileStorage::new(dir.path().join("ledger.json"));
    file.save(&l).unwrap();
    assert_eq!(file.load().unwrap(), l);
}

#[test]
fn loading_missing_file_is_not_found() {
    let storage = JsonFileStorage::new("/definitely/not/here/ledger.json");
    assert!(matches!(
        storage.load(),
        Err(ledger::StorageError::NotFound)
    ));
}

#[test]
fn loading_corrupt_file_is_a_json_error() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("bad.json");
    std::fs::write(&path, "{ not json").unwrap();
    assert!(matches!(
        JsonFileStorage::new(&path).load(),
        Err(ledger::StorageError::Json(_))
    ));
}

#[test]
fn import_csv_end_to_end() {
    let accounts = "id,owner,balance\n1,甲,100.00\n2,乙,0\n";
    let txs = "id,date,type,account,from,to,amount,memo\n1,2026-10-01,transfer,,1,2,40.00,x\n2,2026-10-01,withdraw,2,,,50.00,too much\n";
    let mut l = ledger::Ledger::new();
    for row in ledger::import::read_accounts_from(accounts.as_bytes()).unwrap() {
        l.open(row.id, &row.owner, row.balance).unwrap();
    }
    let rejected = l.apply_all(ledger::import::read_transactions_from(txs.as_bytes()).unwrap());
    assert_eq!(rejected.len(), 1);
    assert_eq!(l.balance_of(2).unwrap(), Money::from_yuan(40));
}
