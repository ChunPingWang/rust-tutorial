//! 帳本：帳戶集合 + 過帳邏輯 + 交易歷史。

use std::collections::HashMap;

use crate::account::Account;
use crate::error::LedgerError;
use crate::money::Money;
use crate::transaction::{Transaction, TxKind};

#[derive(Debug, Default, Clone, PartialEq)]
pub struct Ledger {
    accounts: HashMap<u32, Account>,
    history: Vec<Transaction>,
}

impl Ledger {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn open(&mut self, id: u32, owner: &str, initial: Money) -> Result<(), LedgerError> {
        if self.accounts.contains_key(&id) {
            return Err(LedgerError::DuplicateAccount(id));
        }
        self.accounts.insert(id, Account::new(id, owner, initial));
        Ok(())
    }

    pub fn account(&self, id: u32) -> Result<&Account, LedgerError> {
        self.accounts
            .get(&id)
            .ok_or(LedgerError::AccountNotFound(id))
    }

    pub fn balance_of(&self, id: u32) -> Result<Money, LedgerError> {
        self.account(id).map(|a| a.balance())
    }

    pub fn accounts(&self) -> impl Iterator<Item = &Account> {
        self.accounts.values()
    }

    pub fn history(&self) -> &[Transaction] {
        &self.history
    }

    fn account_mut(&mut self, id: u32) -> Result<&mut Account, LedgerError> {
        self.accounts
            .get_mut(&id)
            .ok_or(LedgerError::AccountNotFound(id))
    }

    fn credit(&mut self, id: u32, amount: Money) -> Result<(), LedgerError> {
        let acct = self.account_mut(id)?;
        let new_balance = acct
            .balance()
            .checked_add(amount)
            .ok_or(LedgerError::Overflow)?;
        acct.set_balance(new_balance);
        Ok(())
    }

    fn debit(&mut self, id: u32, amount: Money) -> Result<(), LedgerError> {
        let acct = self.account_mut(id)?;
        if amount > acct.balance() {
            return Err(LedgerError::InsufficientFunds {
                account: id,
                balance: acct.balance(),
                requested: amount,
            });
        }
        let new_balance = acct
            .balance()
            .checked_sub(amount)
            .ok_or(LedgerError::Overflow)?;
        acct.set_balance(new_balance);
        Ok(())
    }

    /// 過帳。成功時交易進入歷史紀錄。
    pub fn apply(&mut self, tx: Transaction) -> Result<(), LedgerError> {
        if !tx.amount.is_positive() {
            return Err(LedgerError::InvalidAmount(tx.amount));
        }
        match tx.kind {
            TxKind::Deposit { account } => self.credit(account, tx.amount)?,
            TxKind::Withdraw { account } => self.debit(account, tx.amount)?,
            TxKind::Transfer { from, to } => {
                if from == to {
                    return Err(LedgerError::SameAccountTransfer(from));
                }
                self.account(to)?; // 先確認目的帳戶存在
                self.debit(from, tx.amount)?;
                self.credit(to, tx.amount)
                    .expect("目的帳戶已驗證存在，且 debit 成功後總額不變，不應溢位");
            }
        }
        self.history.push(tx);
        Ok(())
    }

    /// 批次過帳：回傳每筆被拒交易的 (id, 原因)
    pub fn apply_all(&mut self, txs: Vec<Transaction>) -> Vec<(u64, LedgerError)> {
        let mut rejected = Vec::new();
        for tx in txs {
            let id = tx.id;
            if let Err(e) = self.apply(tx) {
                rejected.push((id, e));
            }
        }
        rejected
    }

    /// 所有帳戶餘額總和（有了 impl Sum for Money，可以直接 .sum()）
    pub fn total_balance(&self) -> Money {
        self.accounts.values().map(|a| a.balance()).sum()
    }

    /// 從已有的帳戶與歷史重建帳本（給儲存層用）。
    pub fn restore(accounts: Vec<Account>, history: Vec<Transaction>) -> Ledger {
        Ledger {
            accounts: accounts.into_iter().map(|a| (a.id, a)).collect(),
            history,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 測試用的輔助函式：建一個有兩個帳戶的帳本。測試程式碼也該避免重複。
    fn ledger_with_two_accounts() -> Ledger {
        let mut l = Ledger::new();
        l.open(1, "甲", Money::from_yuan(100)).unwrap();
        l.open(2, "乙", Money::from_yuan(10)).unwrap();
        l
    }

    fn tx(id: u64, kind: TxKind, cents: i64) -> Transaction {
        Transaction {
            id,
            day: 1,
            kind,
            amount: Money::from_cents(cents),
            memo: String::new(),
        }
    }

    #[test]
    fn open_rejects_duplicate_id() {
        let mut l = ledger_with_two_accounts();
        assert_eq!(
            l.open(1, "丙", Money::ZERO),
            Err(LedgerError::DuplicateAccount(1))
        );
    }

    #[test]
    fn deposit_increases_balance_and_records_history() {
        let mut l = ledger_with_two_accounts();
        l.apply(tx(1, TxKind::Deposit { account: 1 }, 2_550))
            .unwrap();
        assert_eq!(l.balance_of(1).unwrap(), Money::from_cents(12_550));
        assert_eq!(l.history().len(), 1);
    }

    #[test]
    fn withdraw_more_than_balance_is_rejected_and_not_recorded() {
        let mut l = ledger_with_two_accounts();
        let err = l
            .apply(tx(1, TxKind::Withdraw { account: 2 }, 1_001))
            .unwrap_err();
        assert_eq!(
            err,
            LedgerError::InsufficientFunds {
                account: 2,
                balance: Money::from_yuan(10),
                requested: Money::from_cents(1_001),
            }
        );
        assert_eq!(l.balance_of(2).unwrap(), Money::from_yuan(10)); // 餘額沒變
        assert!(l.history().is_empty()); // 歷史沒多
    }

    #[test]
    fn withdraw_exact_balance_is_allowed() {
        let mut l = ledger_with_two_accounts();
        l.apply(tx(1, TxKind::Withdraw { account: 2 }, 1_000))
            .unwrap();
        assert_eq!(l.balance_of(2).unwrap(), Money::ZERO);
    }

    #[test]
    fn transfer_moves_money_and_preserves_total() {
        let mut l = ledger_with_two_accounts();
        let before = l.total_balance();
        l.apply(tx(1, TxKind::Transfer { from: 1, to: 2 }, 3_000))
            .unwrap();
        assert_eq!(l.balance_of(1).unwrap(), Money::from_cents(7_000));
        assert_eq!(l.balance_of(2).unwrap(), Money::from_cents(4_000));
        assert_eq!(l.total_balance(), before, "轉帳不能改變總額");
    }

    #[test]
    fn transfer_to_missing_account_does_not_debit_source() {
        let mut l = ledger_with_two_accounts();
        let err = l
            .apply(tx(1, TxKind::Transfer { from: 1, to: 999 }, 3_000))
            .unwrap_err();
        assert_eq!(err, LedgerError::AccountNotFound(999));
        assert_eq!(
            l.balance_of(1).unwrap(),
            Money::from_yuan(100),
            "失敗的轉帳不能扣款"
        );
    }

    #[test]
    fn transfer_to_self_is_rejected() {
        let mut l = ledger_with_two_accounts();
        assert_eq!(
            l.apply(tx(1, TxKind::Transfer { from: 1, to: 1 }, 1)),
            Err(LedgerError::SameAccountTransfer(1))
        );
    }

    #[test]
    fn non_positive_amount_is_rejected() {
        let mut l = ledger_with_two_accounts();
        assert!(matches!(
            l.apply(tx(1, TxKind::Deposit { account: 1 }, 0)),
            Err(LedgerError::InvalidAmount(_))
        ));
        assert!(matches!(
            l.apply(tx(2, TxKind::Deposit { account: 1 }, -5)),
            Err(LedgerError::InvalidAmount(_))
        ));
    }

    #[test]
    fn deposit_overflow_is_an_error_not_a_panic() {
        let mut l = Ledger::new();
        l.open(1, "甲", Money::from_cents(i64::MAX - 10)).unwrap();
        assert_eq!(
            l.apply(tx(1, TxKind::Deposit { account: 1 }, 100)),
            Err(LedgerError::Overflow)
        );
        assert_eq!(l.balance_of(1).unwrap(), Money::from_cents(i64::MAX - 10));
    }

    #[test]
    fn apply_all_continues_after_failures() {
        let mut l = ledger_with_two_accounts();
        let rejected = l.apply_all(vec![
            tx(1, TxKind::Deposit { account: 1 }, 100),
            tx(2, TxKind::Withdraw { account: 2 }, 999_999),
            tx(3, TxKind::Deposit { account: 2 }, 100),
        ]);
        assert_eq!(rejected.len(), 1);
        assert_eq!(rejected[0].0, 2);
        assert_eq!(l.history().len(), 2);
    }
}
