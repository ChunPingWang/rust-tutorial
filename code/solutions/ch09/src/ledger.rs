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
