//! 對帳單：某個帳戶在一段期間內的交易明細。
//!
//! 對帳單「借用」帳本裡的資料，不複製。因此它的型別帶有生命週期參數 `'a`，
//! 意思是：「這張對帳單活得不能比它借用的帳本久」。

use crate::{Account, Ledger, LedgerError, Money, Transaction, TxKind};

/// 帳戶對帳單。`'a` 是它借用的帳本的生命週期。
#[derive(Debug)]
pub struct Statement<'a> {
    pub account: &'a Account,
    pub entries: Vec<&'a Transaction>,
    pub from_day: u32,
    pub to_day: u32,
}

impl<'a> Statement<'a> {
    /// 期間內的淨變動（流入 − 流出）
    pub fn net_change(&self) -> Money {
        self.entries
            .iter()
            .map(|tx| match tx.kind {
                TxKind::Deposit { .. } => tx.amount,
                TxKind::Withdraw { .. } => -tx.amount,
                TxKind::Transfer { to, .. } if to == self.account.id => tx.amount,
                TxKind::Transfer { .. } => -tx.amount,
            })
            .sum()
    }

    /// 分頁：回傳一頁的交易。回傳的切片借用自 self.entries，
    /// 而 entries 裡的 &Transaction 又借用自帳本。省略規則幫我們推出回傳的生命週期跟 &self 一樣。
    pub fn page(&self, page: usize, per_page: usize) -> &[&'a Transaction] {
        let start = (page * per_page).min(self.entries.len());
        let end = (start + per_page).min(self.entries.len());
        &self.entries[start..end]
    }

    /// 練習 1：只回傳某種交易。回傳 'a 而不是跟 &self：元素借自帳本，對帳單丟掉後仍可用。
    pub fn filter_kind(&self, label: &str) -> Vec<&'a Transaction> {
        self.entries
            .iter()
            .copied()
            .filter(|tx| kind_label(&tx.kind) == label)
            .collect()
    }

    /// 金額最大的一筆。回傳型別是 Option<&'a Transaction> 而不是 Option<&Transaction>：
    /// 它借用的是帳本（'a），不是 Statement 本身，所以即使 Statement 被丟掉，這個引用還能用。
    pub fn largest(&self) -> Option<&'a Transaction> {
        self.entries.iter().copied().max_by_key(|tx| tx.amount)
    }
}

impl Ledger {
    /// 產生對帳單。回傳值的生命週期與 &self 綁在一起（省略規則）：
    /// 對帳單活著的期間，帳本不能被修改。
    pub fn statement(
        &self,
        account_id: u32,
        from_day: u32,
        to_day: u32,
    ) -> Result<Statement<'_>, LedgerError> {
        let account = self.account(account_id)?;
        let entries = self
            .history()
            .iter()
            .filter(|tx| tx.involves(account_id))
            .filter(|tx| (from_day..=to_day).contains(&tx.day))
            .collect();
        Ok(Statement {
            account,
            entries,
            from_day,
            to_day,
        })
    }
}

/// 兩個輸入引用、一個輸出引用：編譯器不知道輸出借自哪一個，必須標註。
/// `'a` 的意思是：回傳值的有效期間 = a 和 b 中較短的那個。
pub fn longer_memo<'a>(a: &'a Transaction, b: &'a Transaction) -> &'a str {
    if a.memo.chars().count() >= b.memo.chars().count() {
        &a.memo
    } else {
        &b.memo
    }
}

/// 兩個輸入引用但回傳只借自其中一個：用兩個不同的生命週期參數說清楚。
/// 這樣 `label` 可以是一個很短命的暫時字串，不影響回傳值能活多久。
pub fn first_involving<'a, 'b>(
    txs: &'a [Transaction],
    label: &'b str,
    account: u32,
) -> Option<&'a Transaction> {
    let found = txs.iter().find(|tx| tx.involves(account));
    if found.is_none() {
        eprintln!("[{label}] 帳戶 {account} 沒有交易");
    }
    found
}

/// 練習 2：以第一個空白分成兩半。**不需要**標註：只有一個輸入引用，省略規則 2 把兩個輸出都綁到它。
pub fn split_memo(memo: &str) -> (&str, &str) {
    match memo.find(' ') {
        Some(i) => (&memo[..i], memo[i + 1..].trim_start()),
        None => (memo, ""),
    }
}

/// 'static：活到程式結束的引用。字串字面值就是 &'static str。
pub fn kind_label(kind: &TxKind) -> &'static str {
    match kind {
        TxKind::Deposit { .. } => "存款",
        TxKind::Withdraw { .. } => "提款",
        TxKind::Transfer { .. } => "轉帳",
    }
}

/// 一個借用輸入字串的小型解析器：示範「結構體存 &str」的典型用法（零複製解析）。
pub struct MemoParser<'s> {
    rest: &'s str,
}

impl<'s> MemoParser<'s> {
    pub fn new(memo: &'s str) -> Self {
        Self { rest: memo }
    }

    // 練習 3：若把下面 next_word 的回傳型別改成 Option<&str>，省略規則會把它綁到 &mut self。
    // 呼叫端 `words.push(w)` 之後再呼叫 `parser.next_word()` 就會報
    //   error[E0499]: cannot borrow `parser` as mutable more than once at a time
    // 因為第一個 w 還借著 parser。明寫 's 告訴編譯器：w 借的是原始字串，不是 parser。

    /// 每次回傳一個以空白分隔的詞，借自原始字串，沒有任何配置。
    pub fn next_word(&mut self) -> Option<&'s str> {
        let trimmed = self.rest.trim_start();
        if trimmed.is_empty() {
            return None;
        }
        let end = trimmed.find(char::is_whitespace).unwrap_or(trimmed.len());
        let (word, rest) = trimmed.split_at(end);
        self.rest = rest;
        Some(word)
    }
}

/// 練習 4：實作 Iterator，就能 `for word in MemoParser::new(&memo)`，
/// 也能用全部的迭代器方法（collect、count、filter…）。
impl<'s> Iterator for MemoParser<'s> {
    type Item = &'s str;

    fn next(&mut self) -> Option<Self::Item> {
        self.next_word()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn split_memo_cases() {
        assert_eq!(split_memo("房租 十月"), ("房租", "十月"));
        assert_eq!(split_memo("房租   十月 台北"), ("房租", "十月 台北"));
        assert_eq!(split_memo("房租"), ("房租", ""));
    }

    #[test]
    fn parser_is_an_iterator_borrowing_the_source() {
        let memo = String::from("  a bb  ccc ");
        let words: Vec<&str> = MemoParser::new(&memo).collect();
        assert_eq!(words, vec!["a", "bb", "ccc"]);
        assert_eq!(MemoParser::new(&memo).count(), 3);
        assert_eq!(MemoParser::new(&memo).filter(|w| w.len() > 1).count(), 2);
    }
}
