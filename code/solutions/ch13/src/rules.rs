//! 風控規則：一棵可以任意組合的規則樹。
//!
//! 「金額超過一萬元 **且** （是提款 **或** 來源是帳戶 1002）」這種規則是遞迴結構：
//! 規則裡面包著規則。遞迴型別的大小編譯期算不出來，必須透過 Box 放到堆積上。

use serde::{Deserialize, Serialize};

use crate::{Money, Transaction, TxKind};

/// 練習 2：Rule 可以序列化。serde 對 Box<T> 的支援是透明的：JSON 裡看不到 Box。
///
/// 注意：`#[serde(tag = "...")]`（內部標籤）不能用在 tuple 變體（And(Box, Box)），
/// 編譯器會直接報錯。這裡用預設的外部標籤：`{"and": [{...}, {...}]}`。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Rule {
    /// 金額嚴格大於
    AmountAbove(Money),
    /// 是提款
    IsWithdraw,
    /// 是轉帳
    IsTransfer,
    /// 涉及某帳戶
    Involves(u32),
    /// 備註包含關鍵字
    MemoContains(String),
    // 下面三個是遞迴變體。沒有 Box 的話：
    //   error[E0072]: recursive type `Rule` has infinite size
    And(Box<Rule>, Box<Rule>),
    Or(Box<Rule>, Box<Rule>),
    Not(Box<Rule>),
}

impl Rule {
    pub fn matches(&self, tx: &Transaction) -> bool {
        match self {
            Rule::AmountAbove(limit) => tx.amount > *limit,
            Rule::IsWithdraw => matches!(tx.kind, TxKind::Withdraw { .. }),
            Rule::IsTransfer => matches!(tx.kind, TxKind::Transfer { .. }),
            Rule::Involves(id) => tx.involves(*id),
            Rule::MemoContains(kw) => tx.memo.contains(kw.as_str()),
            // Box 會自動解引用：a.matches(tx) 直接呼叫 Rule 的方法
            Rule::And(a, b) => a.matches(tx) && b.matches(tx),
            Rule::Or(a, b) => a.matches(tx) || b.matches(tx),
            Rule::Not(r) => !r.matches(tx),
        }
    }

    // 建構輔助：讓規則讀起來像句子
    pub fn and(self, other: Rule) -> Rule {
        Rule::And(Box::new(self), Box::new(other))
    }

    pub fn or(self, other: Rule) -> Rule {
        Rule::Or(Box::new(self), Box::new(other))
    }

    #[allow(clippy::should_implement_trait)]
    pub fn not(self) -> Rule {
        Rule::Not(Box::new(self))
    }

    /// 規則樹的深度：遞迴走訪
    pub fn depth(&self) -> usize {
        match self {
            Rule::And(a, b) | Rule::Or(a, b) => 1 + a.depth().max(b.depth()),
            Rule::Not(r) => 1 + r.depth(),
            _ => 1,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::NaiveDate;

    fn tx(kind: TxKind, yuan: i64, memo: &str) -> Transaction {
        Transaction {
            id: 1,
            date: NaiveDate::from_ymd_opt(2026, 10, 1).unwrap(),
            kind,
            amount: Money::from_yuan(yuan),
            memo: memo.to_string(),
        }
    }

    #[test]
    fn composite_rule_evaluates_recursively() {
        // 大額提款，或任何涉及 1002 且備註有「現金」的交易
        let rule = Rule::AmountAbove(Money::from_yuan(10_000))
            .and(Rule::IsWithdraw)
            .or(Rule::Involves(1002).and(Rule::MemoContains("現金".into())));

        assert!(rule.matches(&tx(TxKind::Withdraw { account: 1 }, 20_000, "")));
        assert!(!rule.matches(&tx(TxKind::Deposit { account: 1 }, 20_000, "")));
        assert!(rule.matches(&tx(TxKind::Deposit { account: 1002 }, 1, "現金存入")));
        assert!(!rule.matches(&tx(TxKind::Deposit { account: 1002 }, 1, "轉帳")));
        assert_eq!(rule.depth(), 3);
    }

    // 練習 2
    #[test]
    fn rule_round_trips_through_json() {
        let rule = Rule::AmountAbove(Money::from_yuan(10_000))
            .and(Rule::IsWithdraw)
            .or(Rule::Involves(1002).and(Rule::MemoContains("現金".into())));
        let json = serde_json::to_string_pretty(&rule).unwrap();
        println!("{json}");
        let back: Rule = serde_json::from_str(&json).unwrap();
        assert_eq!(back, rule);
        assert!(json.contains("\"and\""));
    }

    #[test]
    fn not_inverts() {
        let rule = Rule::IsTransfer.not();
        assert!(rule.matches(&tx(TxKind::Withdraw { account: 1 }, 1, "")));
        assert!(!rule.matches(&tx(TxKind::Transfer { from: 1, to: 2 }, 1, "")));
    }
}
