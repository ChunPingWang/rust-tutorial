# 第 12 章　外部套件：serde、csv、clap

> **場景**：真實資料來了。財務給你兩個 CSV：`accounts.csv` 和 `transactions.csv`，金額寫成 `50000.00`，日期寫成 `2026-10-01`。
> 他們要一個命令列工具：`ledger import` 匯入過帳、`ledger report --format csv` 出報表、`ledger statement 1002 --from ... --to ...` 看對帳單。
> 第 9 章手刻的 80 行文字格式解析器，現在要處理引號、逗號跳脫、空欄位……手刻是浪費生命。

## Why：為什麼現在才講外部套件

前 11 章刻意只用標準庫，有兩個原因。第一，Rust 的核心難點（所有權、借用、trait、生命週期）跟套件無關，先把它們學會，用套件時才看得懂文件裡的簽名。
第二，**手刻過一次，才知道套件幫你省了什麼**。第 9 章的 `TextFileStorage` 有 80 行解析程式碼，這一章換成 serde_json 之後只剩兩行，而且處理了所有你沒想到的邊界情況。

Rust 的套件生態有幾個特色值得知道：

- **標準庫刻意很小**。沒有內建的 JSON、HTTP、隨機數、正規表達式、非同步執行器。這些都在 crates.io，由社群演化出事實標準（serde、tokio、regex、rand）。好處是標準庫不會卡著過時的 API；代價是你要知道該用哪個。
- **語意化版本與鎖定檔**。`Cargo.toml` 寫 `serde = "1"` 表示「1.x 的任何版本」，`Cargo.lock` 記錄實際用的精確版本。應用程式要把 `Cargo.lock` 提交到 git，確保所有人建置結果相同。
- **功能旗標（features）**。套件可以把部分功能做成可選的，不用的不編譯。`serde = { version = "1", features = ["derive"] }` 就是開啟 derive 巨集。

## What：這一章用到的套件

| 套件 | 做什麼 | 為什麼是它 |
|---|---|---|
| `serde` | 序列化框架：資料結構 ↔ 任何格式 | 事實標準，所有格式套件都接它 |
| `serde_json` | JSON 格式 | serde 官方 |
| `csv` | CSV 讀寫，接 serde | 處理引號、跳脫、BOM 等所有邊界 |
| `clap` | 命令列參數解析 | 功能最完整，derive 寫法直覺 |
| `thiserror` | 錯誤型別的樣板產生器 | 讓第 6 章手寫的 Display/Error 變成屬性 |
| `anyhow` | 應用程式層的錯誤處理 | 取代 `Box<dyn Error>`，多了 context |
| `chrono` | 日期時間 | 最廣泛使用（`time` crate 是另一選擇） |
| `tempfile` | 測試用暫存檔 | dev-dependency，不進正式執行檔 |

### 加入相依

```toml
[dependencies]
serde = { version = "1", features = ["derive"] }
serde_json = "1"
csv = "1"
clap = { version = "4", features = ["derive"] }
thiserror = "2"
anyhow = "1"
chrono = { version = "0.4", features = ["serde"] }

[dev-dependencies]
tempfile = "3"
```

`cargo build` 會自動下載。`cargo add serde --features derive` 可以用指令加。`cargo tree` 看完整的相依樹。

### serde：derive 就好

```rust
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize)]
pub struct Account {
    pub id: u32,
    pub owner: String,
    balance: Money,          // 私有欄位也能序列化
}
```

derive 之後，任何 serde 支援的格式都能用：

```rust
let json = serde_json::to_string_pretty(&ledger)?;
let ledger: Ledger = serde_json::from_str(&json)?;
```

常用的屬性，控制外部格式長相：

| 屬性 | 效果 |
|---|---|
| `#[serde(transparent)]` | newtype 直接序列化成內部值：`Money(100)` → `100` |
| `#[serde(tag = "type", rename_all = "snake_case")]` | enum 用 `{"type": "transfer", "from": ...}` 表示 |
| `#[serde(flatten)]` | 把內層結構的欄位攤到外層 |
| `#[serde(rename = "type")]` | 欄位在外部叫別的名字（這裡因為 `type` 是關鍵字） |
| `#[serde(default)]` | 外部缺這個欄位時用 `Default` |
| `#[serde(skip)]` | 不序列化 |
| `#[serde(deserialize_with = "fn_name")]` | 自訂解析函式 |

### csv：接 serde

```rust
let mut rdr = csv::Reader::from_reader(reader);
for result in rdr.deserialize::<TxRow>() {
    let row: TxRow = result?;
}
```

CSV 的每一行對應一個 struct，標頭對應欄位名。`csv::Reader::from_reader` 接受任何 `Read`，所以測試可以用 `"...".as_bytes()`，不碰檔案。

### clap：用結構體描述命令列

```rust
#[derive(Parser)]
#[command(name = "ledger", version, about)]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// 從 CSV 匯入帳戶與交易，過帳後存成 JSON      ← 這行文件註解會變成 --help 的說明
    Import {
        #[arg(long)]
        accounts: PathBuf,
        #[arg(long, default_value = "ledger.json")]
        out: PathBuf,
    },
    Statement {
        account: u32,                  // 位置參數
        #[arg(long)]
        from: NaiveDate,               // 只要型別實作 FromStr，clap 就能解析
    },
}

let cli = Cli::parse();     // 解析失敗會自動印出錯誤與用法並退出
```

`--help`、`--version`、錯誤訊息、shell 自動補全全部免費。

### thiserror：錯誤型別的屬性版

第 6 章手寫的 30 行變成：

```rust
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum LedgerError {
    #[error("帳戶 {0} 不存在")]
    AccountNotFound(u32),

    #[error("帳戶 {account} 餘額不足：有 {balance} 元，要 {requested} 元")]
    InsufficientFunds { account: u32, balance: Money, requested: Money },
    // ...
}
```

`#[error("...")]` 產生 `Display`，`derive(Error)` 產生 `impl std::error::Error`。還有兩個常用屬性：

```rust
#[error("I/O 錯誤：{0}")]
Io(#[from] std::io::Error),      // #[from]：自動產生 From<io::Error>，? 可以直接轉

#[error("讀取 {path} 失敗：{source}")]
Io { path: String, #[source] source: std::io::Error },   // #[source]：標記底層原因，但不自動 From
```

### anyhow：應用程式層

函式庫用 `thiserror` 定義精確的錯誤 enum（呼叫者要 `match`）。應用程式最外層用 `anyhow`（只要印出來）：

```rust
use anyhow::{bail, Context, Result};      // anyhow::Result<T> = Result<T, anyhow::Error>

fn main() -> Result<()> {
    let ledger = JsonFileStorage::new(path).load()
        .with_context(|| format!("載入帳本 {} 失敗", path.display()))?;
    if from > to {
        bail!("--from ({from}) 不能晚於 --to ({to})");
    }
    Ok(())
}
```

`anyhow::Error` 可以從任何實作 `std::error::Error` 的型別 `?` 轉過來。`.context()` / `.with_context()` 在錯誤外面包一層「我當時在做什麼」，印出來像：

```
Error: 載入帳本 /nope.json 失敗

Caused by:
    找不到已儲存的帳本
```

這個 Caused by 鏈就是 `#[source]` 的用途。

## How：真正的匯入工具

完整程式碼在 `code/ch12-crates/`。`cargo run -- --help` 看用法。

### Money 學會從字串解析

CSV 裡的金額是 `"50000.00"`，需要 `FromStr`：

```rust
impl std::str::FromStr for Money {
    type Err = ParseMoneyError;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        // 處理負號、小數點、"5" 代表 0.50 而 "05" 代表 0.05、溢位
        ...
    }
}
```

有了 `FromStr`，`"250.50".parse::<Money>()` 就能用，clap 也能直接把參數解析成 `Money`。
serde 預設**不**走 `FromStr`（JSON 裡金額是整數），所以 CSV 的欄位用 `#[serde(deserialize_with = "money_from_str")]` 指定。

測試裡有一個 round-trip 測試：任何 `Money` 的 `to_string()` 再 `parse()` 要等於自己。這類「來回測試」對任何序列化邏輯都值得寫。

### 中介型別：外部格式 ≠ 核心型別

```rust
/// CSV 裡的一行交易：欄位是平的，account/from/to 可能為空
#[derive(Deserialize)]
struct TxRow {
    id: u64,
    date: NaiveDate,
    #[serde(rename = "type")]
    kind: String,
    #[serde(default)] account: Option<u32>,
    #[serde(default)] from: Option<u32>,
    #[serde(default)] to: Option<u32>,
    #[serde(deserialize_with = "money_from_str")]
    amount: Money,
    #[serde(default)] memo: String,
}

impl TryFrom<TxRow> for Transaction {
    type Error = String;
    fn try_from(row: TxRow) -> Result<Self, String> {
        let kind = match row.kind.as_str() {
            "deposit" => TxKind::Deposit { account: row.account.ok_or("deposit 需要 account 欄位")? },
            "transfer" => TxKind::Transfer {
                from: row.from.ok_or("transfer 需要 from 欄位")?,
                to: row.to.ok_or("transfer 需要 to 欄位")?,
            },
            other => return Err(format!("未知的交易種類 {other:?}")),
        };
        Ok(Transaction { id: row.id, date: row.date, kind, amount: row.amount, memo: row.memo })
    }
}
```

這是這章最重要的設計決定。CSV 是「平的、欄位可空」的格式，`TxKind` 是「非法狀態無法表示」的 enum（第 5 章）。
**不要為了配合外部格式弄髒核心型別。** 定義一個長得像外部格式的中介型別，在 `TryFrom` 裡做轉換與驗證。核心 `Ledger` 永遠只看到合法的 `Transaction`。

同樣的原則適用於 API 的請求/回應型別（第 15 章）、資料庫的 row 型別。

### 匯入：收集 Result

```rust
let rows: Result<Vec<AccountRow>, csv::Error> = rdr.deserialize().collect();
```

第 7 章提過：一堆 `Result` 可以 `collect` 成 `Result<Vec<_>, _>`，任何一個 `Err` 就整體 `Err`。帳戶清單用這個策略（任何一行壞就不匯入）。
交易用迴圈逐行處理，為了在錯誤裡附上行號：

```rust
for (i, result) in rdr.deserialize::<TxRow>().enumerate() {
    let line = i + 2;        // 第 1 行是標頭
    let row = result?;
    let tx = Transaction::try_from(row).map_err(|reason| ImportError::Row { line, reason })?;
    txs.push(tx);
}
```

### 執行

```
$ cargo run -- import --accounts data/accounts.csv --transactions data/transactions.csv --out ledger.json
帳戶 3 個，交易 10 筆，成功 7 筆，拒絕 3 筆
  拒絕 #3：帳戶 1002 餘額不足：有 2000.00 元，要 99999.99 元
  拒絕 #4：帳戶 9999 不存在
  拒絕 #10：不能轉帳給自己（帳戶 1001）
已存檔：ledger.json

$ cargo run -- report --format csv
account_id,owner,balance_cents
1001,王小明,4950000
1003,張大同,226184
1002,李小華,145050

$ cargo run -- statement 1002 --from 2026-10-01 --to 2026-10-31
帳戶 #1002 李小華 對帳單 2026-10-01 ~ 2026-10-31
  #5 2026-10-03 帳戶 1002 提出 100.00 元（ATM）
  ...
共 4 筆，淨變動 -549.50 元，目前餘額 1450.50 元
```

`ledger.json` 長這樣（注意 `Money` 是整數、`TxKind` 被攤平成 `type`/`from`/`to`）：

```json
{
  "accounts": {
    "1001": { "id": 1001, "owner": "王小明", "balance": 4950000 }
  },
  "history": [
    { "id": 2, "date": "2026-10-01", "type": "transfer", "from": 1001, "to": 1003, "amount": 150000, "memo": "房租" }
  ]
}
```

### 第 9 章的 Storage trait 回報了

`TextFileStorage` 刪掉，`JsonFileStorage` 補上，`Storage` trait 不動，`main.rs` 只改型別名稱。整合測試改了幾行斷言。核心邏輯零修改。

## 怎麼挑套件

crates.io 上同一件事常有好幾個套件。判斷標準：

1. **下載量與近期更新**：crates.io 頁面右側。一年沒更新的要小心
2. **誰在用它**：被 tokio、serde 等大型專案依賴的套件通常可靠
3. **文件品質**：docs.rs 上有沒有範例
4. **相依數量**：`cargo tree` 看它拖進多少東西。一個小功能拖進 50 個 crate 不划算
5. **`cargo audit`**：檢查已知安全漏洞（第 16 章）

本章的七個套件都是各自領域的事實標準，可以放心用。

## 回顧

- 標準庫刻意小，serde / clap / tokio 等是社群演化出的事實標準
- `Cargo.lock` 提交到 git（應用程式）；`features` 控制可選功能；`dev-dependencies` 只在測試時編譯
- serde：derive + 屬性控制格式；`transparent`、`tag`、`flatten`、`default`、`deserialize_with` 最常用
- 外部格式用中介型別接，`TryFrom` 轉成核心型別，核心型別永遠乾淨
- 函式庫用 `thiserror` 定義精確錯誤；應用程式最外層用 `anyhow` 加 context

## 練習

1. 加一個 `ledger export --format csv --out history.csv` 子命令，用 `csv::Writer` 把 `history` 寫出去（提示：需要一個 `Serialize` 的平面中介型別，因為 `csv` 不支援 `flatten` 的 enum）。
2. 讓 `transactions.csv` 支援金額欄位帶千分位（`"1,500.00"`，注意這在 CSV 裡會被引號包起來）。改 `Money::from_str` 還是改 `money_from_str`？為什麼？
3. 給 `Cli` 加全域選項 `--verbose`，開啟時把每一筆成功的交易也印出來。查 clap 文件的 `global = true`。
4. 用 `cargo tree` 看看這個專案總共拖進了多少 crate。找出最大的來源。

## 下一章

目前程式是「讀檔 → 處理 → 寫檔 → 結束」。接下來要變成長時間執行的服務：櫃台程式、稽核程式、報表程式同時持有同一個帳本。
一個帳本，多個持有者，這違反第 2 章的「單一擁有者」。Rust 的解法是智慧指標。
