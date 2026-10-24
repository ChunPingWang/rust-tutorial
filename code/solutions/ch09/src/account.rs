//! 帳戶。

use crate::money::Money;

#[derive(Debug, Clone, PartialEq)]
pub struct Account {
    pub id: u32,
    pub owner: String,
    // 餘額是私有的：外部只能透過 Ledger 的過帳邏輯改它，不能直接賦值
    balance: Money,
}

impl Account {
    pub fn new(id: u32, owner: &str, initial: Money) -> Account {
        Account {
            id,
            owner: owner.to_string(),
            balance: initial,
        }
    }

    pub fn balance(&self) -> Money {
        self.balance
    }

    // pub(crate)：只有這個 crate 內部（也就是 Ledger）能呼叫
    pub(crate) fn set_balance(&mut self, balance: Money) {
        self.balance = balance;
    }
}
