//! 帳戶。

use serde::{Deserialize, Serialize};

use crate::money::Money;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Account {
    pub id: u32,
    pub owner: String,
    // 私有欄位照樣可以 derive Serialize/Deserialize：序列化是型別自己的事
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

    pub(crate) fn set_balance(&mut self, balance: Money) {
        self.balance = balance;
    }
}
