# 第 8 章　模組與專案結構：從單檔到 crate

> **場景**：`main.rs` 已經三百行。新同事加入，想改報表邏輯，卻得先讀完整個檔案。
> 更麻煩的是：有人在報表函式裡直接寫 `account.balance = ...`，繞過了所有的驗證。
> 我們需要把程式拆成模組，並且**用可見性規則保證餘額只能透過過帳邏輯修改**。

## Why：模組不只是分檔案

分檔案的理由每個人都懂：好找、好讀、好分工。Rust 的模組系統多做一件事：**封裝（encapsulation）**。

帳本有幾個不變量（invariant），是整個系統正確性的基礎：

- 所有 `Money` 的值都是合法的（例如之後若規定不能為負）
- 帳戶餘額只能透過 `credit`/`debit` 改變，而這兩個函式有溢位與餘額檢查
- 每一筆成功過帳的交易都在 `history` 裡

在單檔程式裡，任何一行都可以寫 `acct.balance = Money(-999)`，不變量只靠自律。
拆成模組之後，`balance` 欄位是私有的，外部模組連寫都寫不出來，編譯器幫你守住不變量。

另一個理由：**lib 與 bin 分離**。核心邏輯放在 library crate，命令列入口只是它的一個薄薄的使用者。
這樣核心邏輯可以被測試（第 11 章）、被另一個 bin 重用（第 15 章的 API 伺服器）、甚至發佈到 crates.io。

## What：crate、模組、可見性

### 名詞

- **package**：一個 `Cargo.toml` 管理的單位，`cargo new` 產生的東西
- **crate**：編譯單位。一個 package 可以有**一個** library crate（`src/lib.rs`）和**多個** binary crate（`src/main.rs`、`src/bin/*.rs`）
- **module**：crate 內部的命名空間，用 `mod` 宣告

### 宣告模組

```rust
// src/lib.rs
pub mod money;      // 去找 src/money.rs（或 src/money/mod.rs）
pub mod report;     // 去找 src/report.rs 或 src/report/mod.rs
```

`mod xxx;` 是**宣告**，告訴編譯器「有這個模組，去對應的檔案找內容」。不宣告的檔案不會被編譯，即使它在 `src/` 裡。

子模組有兩種檔案佈局：

```
src/report.rs          +  src/report/summary.rs    （新式，推薦）
src/report/mod.rs      +  src/report/summary.rs    （舊式，仍然常見）
```

本書的 `report` 用了 `mod.rs` 形式讓你見過它，兩者功能完全相同。

也可以內嵌：

```rust
mod tests {
    // 直接寫在這裡，第 11 章的單元測試會這樣用
}
```

### 路徑與 `use`

```rust
use crate::money::Money;              // 絕對路徑，從 crate 根開始
use super::summary;                   // 相對路徑，父模組
use self::risk::rapid_withdrawals;    // 相對路徑，目前模組（self 通常省略）
use std::collections::{HashMap, BTreeMap};   // 標準庫，大括號合併
```

`use` 只是建立一個短名稱，不會「載入」任何東西（模組已經由 `mod` 宣告載入了）。
Python 的 `import` 同時做了這兩件事，Rust 分開。

### 可見性

**預設全部私有**。私有的意思是：**只有目前模組和它的子模組看得到**。

| 寫法 | 誰看得到 |
|---|---|
| （無） | 本模組及子模組 |
| `pub` | 所有人（如果外層模組也 pub 到那層） |
| `pub(crate)` | 本 crate 內部任何地方，crate 外不行 |
| `pub(super)` | 父模組 |
| `pub(in path)` | 指定路徑以內 |

結構體的 `pub` 和欄位的 `pub` 是分開的：

```rust
pub struct Account {
    pub id: u32,          // 外部可讀寫
    pub owner: String,
    balance: Money,       // 外部看不到，連讀都不能
}
```

欄位有私有的結構體，外部就不能用 `Account { ... }` 語法建立它，必須走 `Account::new`。這就是「建構函式可以加檢查」的保證來源。

enum 的變體跟著 enum 一起 pub，不能單獨控制。

### 重新匯出（re-export）

```rust
// src/lib.rs
pub mod money;
pub use money::Money;    // 使用者可以寫 ledger::Money，不用 ledger::money::Money
```

這讓你的內部檔案結構可以自由調整，而不影響使用者看到的 API。慣例：**lib.rs 把最常用的型別 re-export 到根層級**。

### lib 與 bin

```toml
[lib]
name = "ledger"
path = "src/lib.rs"

[[bin]]
name = "ledger"
path = "src/main.rs"
```

其實 `src/lib.rs` 和 `src/main.rs` 同時存在時 cargo 會自動認得，上面的設定可以省略。寫出來是為了讓你看到它的存在，以及之後要加第二個 bin（API 伺服器）時知道怎麼寫。

bin 用 lib 的方式就像用外部套件：

```rust
// src/main.rs
use ledger::{report, Ledger, Money};
```

注意是 `ledger::`（package 名稱）而不是 `crate::`。從 bin 的角度，lib 是另一個 crate。

### 文件註解

```rust
//! 這是模組層級的文件（inner doc），放在檔案開頭

/// 這是項目的文件（outer doc），放在 fn/struct/enum 前面
/// 支援 Markdown，可以用 [`Money::from_cents`] 連結到其他項目
pub fn from_yuan(yuan: i64) -> Money
```

`cargo doc --open` 會產生跟標準庫一樣格式的 HTML 文件。文件註解裡的程式碼區塊會被當測試執行（第 11 章）。

## How：把帳本拆成 crate

完整程式碼在 `code/ch08-modules/`。從這一章起，package 名稱固定為 `ledger`，之後每章在它上面疊加。

### 目錄結構

```
ch08-modules/
├── Cargo.toml
└── src/
    ├── lib.rs           # crate 根：宣告模組、re-export
    ├── main.rs          # 命令列入口
    ├── money.rs         # Money
    ├── account.rs       # Account
    ├── transaction.rs   # TxKind, Transaction
    ├── error.rs         # LedgerError
    ├── ledger.rs        # Ledger：狀態 + 過帳
    └── report/
        ├── mod.rs       # 子模組宣告與 re-export
        ├── summary.rs   # 統計報表
        └── risk.rs      # 風控報表
```

拆分的原則：**一個檔案一個概念**。`report` 再拆成兩個子模組，因為統計與風控是不同團隊負責的。

### lib.rs：門面

```rust
pub mod account;
pub mod error;
pub mod ledger;
pub mod money;
pub mod report;
pub mod transaction;

pub use account::Account;
pub use error::LedgerError;
pub use ledger::Ledger;
pub use money::Money;
pub use transaction::{Transaction, TxKind};
```

### money.rs：私有欄位守住不變量

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Default)]
pub struct Money(i64);      // 欄位沒有 pub

impl Money {
    pub const ZERO: Money = Money(0);
    pub fn from_cents(cents: i64) -> Money { Money(cents) }
    pub fn from_yuan(yuan: i64) -> Money { Money(yuan * 100) }
    pub fn cents(self) -> i64 { self.0 }
    pub fn checked_add(self, other: Money) -> Option<Money> {
        self.0.checked_add(other.0).map(Money)
    }
    // ...
}
```

前幾章到處寫的 `Money(acct.balance.0 + amount.0)` 現在在 `money.rs` 以外都寫不出來了。
所有加減都走 `checked_add`/`checked_sub`，溢位檢查集中在一處。

`.map(Money)`：元組結構體的名稱本身就是一個函式 `fn(i64) -> Money`，可以直接傳給 `map`。

### account.rs：`pub(crate)` 給內部用

```rust
pub struct Account {
    pub id: u32,
    pub owner: String,
    balance: Money,                 // 私有
}

impl Account {
    pub fn balance(&self) -> Money { self.balance }
    pub(crate) fn set_balance(&mut self, balance: Money) { self.balance = balance; }
}
```

`set_balance` 是 `pub(crate)`：`ledger.rs` 需要它，但 crate 外部（例如 `main.rs`，或未來的 API 伺服器）不該直接呼叫。
這就是場景裡「有人繞過驗證直接改餘額」的解法：**不是靠 code review，是靠編譯器**。

### ledger.rs：公開查詢，私有修改

```rust
impl Ledger {
    pub fn account(&self, id: u32) -> Result<&Account, LedgerError> { ... }
    pub fn accounts(&self) -> impl Iterator<Item = &Account> { self.accounts.values() }
    pub fn history(&self) -> &[Transaction] { &self.history }
    pub fn apply(&mut self, tx: Transaction) -> Result<(), LedgerError> { ... }

    fn account_mut(&mut self, id: u32) -> Result<&mut Account, LedgerError> { ... }  // 私有
    fn credit(&mut self, id: u32, amount: Money) -> Result<(), LedgerError> { ... }  // 私有
    fn debit(&mut self, id: u32, amount: Money) -> Result<(), LedgerError> { ... }   // 私有
}
```

外部能做的事只有：開戶、查詢、過帳。`credit`/`debit` 是實作細節。

`accounts()` 回傳 `impl Iterator<Item = &Account>`：「某個會產生 `&Account` 的迭代器」，不暴露內部是 `HashMap` 的事實。
以後把 `HashMap` 換成別的容器，簽名不用變。第 9 章會解釋 `impl Trait`。

### report：唯讀，拿 `&Ledger`

```rust
pub fn total_deposits(ledger: &Ledger) -> Money {
    ledger.history().iter()
        .filter(|tx| matches!(tx.kind, TxKind::Deposit { .. }))
        .map(|tx| tx.amount)
        .fold(Money::ZERO, |acc, m| acc.checked_add(m).expect("總額溢位"))
}
```

報表函式只拿 `&Ledger`，型別系統保證它們**不可能**修改帳本。這比「報表團隊答應不會改」可靠得多。

### main.rs：薄薄一層

```rust
use ledger::{report, Ledger, Money, Transaction, TxKind};

fn main() {
    let mut ledger = Ledger::new();
    ledger.open(1001, "王小明", Money::from_yuan(50_000)).unwrap();
    // ...
    let rejected = ledger.apply_all(sample_transactions());
    println!("總存款：{}", report::total_deposits(&ledger).format());
}
```

`main.rs` 不含任何業務邏輯。它建立資料、呼叫 lib、印結果。這裡的 `unwrap` 是可以接受的：測試資料的開戶不可能失敗，失敗代表程式寫錯。

### 試著弄壞它

在 `main.rs` 加這幾行，一個一個試：

```rust
let m = Money(100);                        // E0423/E0603: 欄位私有，不能用元組語法建立
let mut a = ledger.account(1001).unwrap().clone();
a.set_balance(Money::ZERO);                // E0624: method `set_balance` is private
let acct = Account { id: 1, owner: "x".into(), balance: Money::ZERO };  // E0451: field `balance` is private
```

每一個都是編譯錯誤。這些錯誤訊息就是模組系統在替你守不變量。

## 專案結構的慣例

```
src/lib.rs            crate 根，只放 mod 宣告與 re-export，不放邏輯
src/main.rs           bin 入口，薄
src/bin/*.rs          多個 bin 時放這裡，每個檔案一個執行檔
tests/                整合測試（第 11 章）
benches/              效能測試
examples/             範例程式，cargo run --example xxx
```

模組拆分的時機：一個檔案超過三四百行、或者兩個概念需要不同的可見性邊界時。不要一開始就拆成二十個檔案。

## Workspace：多個 package

當專案再長大，例如要把 `ledger-core`、`ledger-cli`、`ledger-server` 分成三個 package 時，用 workspace：

```toml
# 最外層 Cargo.toml
[workspace]
members = ["ledger-core", "ledger-cli", "ledger-server"]
resolver = "2"
```

```toml
# ledger-cli/Cargo.toml
[dependencies]
ledger-core = { path = "../ledger-core" }
```

所有成員共用一個 `target/` 和 `Cargo.lock`，`cargo build` 在根目錄一次建全部。本書的範例為了簡單維持單一 package，但第 15 章會提到何時該拆。

## 回顧

- `mod x;` 宣告模組並載入檔案；`use` 只是取短名
- 預設私有；`pub` 公開、`pub(crate)` 給 crate 內部用
- 私有欄位 + 公開建構函式 + 公開唯讀方法 = 編譯器守住不變量
- lib 放邏輯、bin 放入口；bin 用 `package_name::` 引用 lib
- `lib.rs` re-export 常用型別，隱藏內部目錄結構

## 練習

1. 新增 `src/bin/stats.rs`，用 lib 印出每個帳戶的交易筆數。用 `cargo run --bin stats` 執行。
2. 把 `report/mod.rs` 改成新式的 `report.rs` + `report/` 目錄佈局。確認行為不變。
3. 在 `Money::from_cents` 加入檢查：金額不能小於 `-1_000_000_000_000`（負一百億元）。想一想：回傳型別要改成什麼？這會影響多少呼叫端？（這就是「不變量集中在建構函式」的成本與好處。）
4. 執行 `cargo doc --open`，瀏覽自己 crate 的文件。把 `Ledger::apply` 的文件補完整，說明它的錯誤情況。

## 下一章

目前帳本只存在記憶體裡，程式結束一切消失。財務要存到檔案；未來還要接資料庫。
報表目前只有一種格式，財務要 CSV、主管要純文字摘要。
「同一件事，多種做法，核心邏輯不該關心是哪一種」，這是 trait 的場景。
