# 第 9 章　Trait 與泛型：多種儲存後端與報表格式

> **場景**：三個需求同時到來：
> 1. 帳本要能存檔、讀回。現在存文字檔，明年接資料庫，核心邏輯不該跟著改
> 2. 報表要有純文字版（給主管）和 CSV 版（給財務匯入試算表），之後可能還有 JSON
> 3. 工程師抱怨 `Money(a.0 + b.0)` 和 `.format()` 太醜，想直接寫 `a + b` 和 `println!("{a}")`
>
> 三個需求背後是同一個概念：**定義「能做什麼」，而不是「是什麼」**。

## Why：為什麼需要 trait

需求 1 和 2 是經典的「同一件事，多種做法」。物件導向語言用介面（interface）解決。Rust 的 trait 類似介面，但多了幾個關鍵能力：

- **可以為別人的型別實作你的 trait**，也可以為你的型別實作別人的 trait。例如為 `Money` 實作標準庫的 `Display`、`Add`，讓它能被 `println!` 和 `+` 使用。這在 Java 做不到（你不能讓 `Integer` 實作你的介面）。
- **靜態分派預設零成本**。泛型函式 `fn render<R: ReportFormat>(r: &R)` 在編譯期為每個實際型別生成一份專用程式碼，沒有虛擬函式表的間接跳轉。需要執行期多型時才用 `dyn`，而且是你明確選擇。
- **trait 是型別系統的語言**。第 2 章的 `Copy`、第 4 章 derive 的 `Debug`、`Clone`，第 7 章 `HashMap` key 需要的 `Hash + Eq`，全部都是 trait。理解 trait 之後，前八章一堆「先照寫」的東西會突然連起來。

需求 3 看似只是語法糖，其實是同一件事：`+` 運算子是 `Add` trait 的方法，`{}` 是 `Display` trait 的方法。實作了 trait，型別就「能做」那件事。

## What：trait、impl、泛型、trait object

### 定義與實作

```rust
pub trait Storage {
    fn save(&mut self, ledger: &Ledger) -> Result<(), StorageError>;
    fn load(&self) -> Result<Ledger, StorageError>;

    fn describe(&self) -> String {                 // 預設方法
        String::from("未命名的儲存後端")
    }
}

impl Storage for InMemoryStorage {
    fn save(&mut self, ledger: &Ledger) -> Result<(), StorageError> { ... }
    fn load(&self) -> Result<Ledger, StorageError> { ... }
    fn describe(&self) -> String { String::from("記憶體快照") }   // 覆寫預設
}
```

`impl Trait for Type` 區塊只能放 trait 宣告的方法。預設方法讓 trait 可以之後加新方法而不破壞既有實作。

### 使用 trait 的三種方式

**1. 泛型參數 + trait bound（靜態分派）**

```rust
pub fn render_to_string<R: ReportFormat>(format: &R, ledger: &Ledger) -> String {
    format.render(ledger)
}
```

讀法：「對任何實作了 `ReportFormat` 的型別 `R`」。編譯器會為 `TextReport` 和 `CsvReport` 各生成一份 `render_to_string`，這叫**單態化（monomorphization）**。呼叫是直接呼叫，可內聯，零成本。

多個 bound 用 `+`，多個泛型參數時用 `where` 比較好讀：

```rust
fn largest<T: PartialOrd + Copy>(items: &[T]) -> Option<T>

fn process<S, R>(storage: &mut S, format: &R) -> String
where
    S: Storage,
    R: ReportFormat + Clone,
{ ... }
```

**2. `impl Trait`（靜態分派的簡寫）**

```rust
pub fn file_name(format: &impl ReportFormat, base: &str) -> String
```

和泛型參數等價，參數只用一次時比較簡潔。回傳位置也能用：

```rust
pub fn accounts(&self) -> impl Iterator<Item = &Account>     // 第 8 章見過
```

回傳 `impl Trait` 的意思是「某個實作了這個 trait 的具體型別，但我不告訴你是哪個」。閉包和迭代器的型別很長或寫不出來，這個寫法必要。

**3. `dyn Trait`（動態分派）**

```rust
let formats: Vec<Box<dyn ReportFormat>> = vec![
    Box::new(TextReport),
    Box::new(CsvReport { with_header: false }),
];
for f in &formats {
    f.render(&ledger);     // 執行期透過 vtable 找到正確的 render
}
```

`dyn ReportFormat` 是 **trait object**：一個「實作了這個 trait 的某種東西」。它的大小編譯期不知道（`TextReport` 是 0 bytes，`CsvReport` 是 1 byte），所以必須放在指標後面：`Box<dyn T>`、`&dyn T`、`Rc<dyn T>`。
呼叫方法會多一次指標跳轉（查 vtable），通常可以忽略，但不能內聯。

**什麼時候用哪個：**

| 情境 | 用 |
|---|---|
| 一個函式接受「某種」實作，呼叫端知道是哪一種 | 泛型 / `impl Trait` |
| 一個集合裡要放**不同**型別的實作 | `dyn` |
| 哪一種實作在**執行期**才決定（讀設定檔、使用者選擇） | `dyn` |
| 在意每一奈秒（熱路徑迴圈內） | 泛型 |
| 在意編譯時間與二進位大小（泛型會為每個型別複製一份程式碼） | `dyn` |

### 標準庫的重要 trait

為自己的型別實作這些 trait，是「讓型別融入語言」的方式：

| trait | 作用 | 怎麼得到 |
|---|---|---|
| `Debug` | `{:?}` | derive |
| `Display` | `{}`、`.to_string()` | 手動實作 `fmt` |
| `Clone` / `Copy` | 複製 | derive |
| `PartialEq` / `Eq` | `==` | derive（或手動定義「相等」的意義） |
| `PartialOrd` / `Ord` | `<`、排序 | derive（欄位順序決定比較順序）或手動 |
| `Hash` | 當 HashMap key | derive |
| `Default` | `T::default()` | derive 或手動 |
| `From<T>` / `Into<T>` | 型別轉換，`?` 靠它轉錯誤 | 實作 `From`，`Into` 自動獲得 |
| `Add`、`Sub`、`Mul`、`Neg`、`AddAssign`… | 運算子 | 手動實作 |
| `Iterator` | 可以被 `for` 迭代、用所有迭代器方法 | 實作 `next` |
| `Sum` / `Product` | `.sum()` | 手動 |
| `Error` | 是個錯誤 | 空 impl（需 Debug + Display） |
| `Send` / `Sync` | 可跨執行緒 | 自動推導（第 14 章） |

### 關聯型別

```rust
impl Add for Money {
    type Output = Money;                 // 關聯型別：這個 impl 的「輸出型別」
    fn add(self, rhs: Money) -> Money { ... }
}
```

`Iterator` 的 `type Item` 也是關聯型別。它和泛型參數的差別：泛型參數讓同一個型別可以有多個實作（`impl From<i64> for Money` 和 `impl From<i32> for Money` 可以共存），關聯型別則是「每個實作恰好一個」。

### 孤兒規則

你只能在**你的 trait** 或**你的型別**所在的 crate 裡寫 `impl`。不能在 `ledger` 裡 `impl Display for Vec<Money>`，因為兩者都不是你的。
解法是 newtype：`struct MoneyList(Vec<Money>); impl Display for MoneyList`。這條規則保證全世界對同一組（trait, type）最多只有一個實作，不會打架。

## How：三個需求，一個概念

完整程式碼在 `code/ch09-traits/`。

### 需求 3：Money 融入語言

```rust
impl fmt::Display for Money {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let sign = if self.0 < 0 { "-" } else { "" };
        let yuan = (self.0 / 100).abs();
        let fen = (self.0 % 100).abs();
        f.pad(&format!("{sign}{yuan}.{fen:02}"))    // pad 讓 {:>12} 的對齊也能用
    }
}

impl Add for Money {
    type Output = Money;
    fn add(self, rhs: Money) -> Money {
        self.checked_add(rhs).expect("Money 加法溢位")
    }
}

impl Sum for Money {
    fn sum<I: Iterator<Item = Money>>(iter: I) -> Money {
        iter.fold(Money::ZERO, |acc, m| acc + m)
    }
}
```

效果：

```rust
println!("總餘額：{} 元", ledger.total_balance());
println!("[{:>12}]", money);                  // 右對齊
let total: Money = amounts.iter().copied().sum();
*map.entry(id).or_insert(Money::ZERO) += tx.amount;    // AddAssign
```

**一個設計決策要說清楚**：`Add` 溢位時 panic，而 `Ledger::credit` 用 `checked_add` 回傳錯誤。為什麼不一致？
因為 `+` 的使用場合是報表加總與測試，溢位代表資料已經壞了，是 bug；`credit` 處理的是外部輸入，溢位是「可能發生的輸入錯誤」。
這呼應第 6 章：同一個運算，在不同邊界有不同的錯誤語義。程式碼裡的 doc comment 記錄了這個決定。

### 需求 1：Storage trait

```rust
pub trait Storage {
    fn save(&mut self, ledger: &Ledger) -> Result<(), StorageError>;
    fn load(&self) -> Result<Ledger, StorageError>;
    fn describe(&self) -> String { String::from("未命名的儲存後端") }
}
```

兩個實作：`InMemoryStorage`（存一份 clone，測試用）和 `TextFileStorage`（自訂逐行格式）。使用端完全一樣：

```rust
let mut backends: Vec<Box<dyn Storage>> = vec![
    Box::new(InMemoryStorage::default()),
    Box::new(TextFileStorage::new(&path)),
];
for backend in backends.iter_mut() {
    backend.save(&ledger)?;
    let restored = backend.load()?;
    assert!(restored == ledger);
}
```

明年接資料庫：寫一個 `PostgresStorage`，實作 `Storage`，`Ledger` 一行不改。

`StorageError` 實作了 `From<std::io::Error>`：

```rust
impl From<std::io::Error> for StorageError {
    fn from(e: std::io::Error) -> Self { StorageError::Io(e) }
}
```

所以 `fs::write(&self.path, out)?` 可以直接用 `?`：`?` 看到回傳型別是 `StorageError`，自動呼叫 `From::from` 轉換。這就是第 6 章提到的 `?` 會做 `.into()` 的機制。

### 需求 2：ReportFormat trait

```rust
pub trait ReportFormat {
    fn render(&self, ledger: &Ledger) -> String;
    fn file_extension(&self) -> &'static str;
}
```

`TextReport` 是單元結構體（沒有欄位），`CsvReport { with_header: bool }` 有設定。
`render_to_string` 用泛型，`render_all` 用 `dyn`，程式碼裡兩種都示範了。

### 泛型函式

```rust
fn largest<T: PartialOrd + Copy>(items: &[T]) -> Option<T> {
    let mut iter = items.iter();
    let mut best = *iter.next()?;      // 空切片 → None（? 對 Option 也能用）
    for &item in iter {
        if item > best { best = item; }
    }
    Some(best)
}

largest(&amounts)          // Money
largest(&[3u64, 9, 4])     // u64
```

`T: PartialOrd` 是因為用了 `>`；`T: Copy` 是因為 `best = item` 需要複製。去掉任一個 bound 都會編譯錯誤，而且錯誤訊息會告訴你缺哪個。這是 Rust 泛型與 C++ template 的差異：**bound 在簽名上，錯誤在定義處而不是使用處**。

### 試著弄壞它

1. 把 `impl Sum for Money` 註解掉，看 `total_balance` 裡的 `.sum()` 怎麼報錯。
2. 寫 `fn f(formats: Vec<dyn ReportFormat>)`，看 `the size for values of type dyn ReportFormat cannot be known` 錯誤。這就是為什麼 `dyn` 要在 `Box` 或 `&` 後面。
3. 給 `Storage` 加一個泛型方法 `fn save_as<T: Display>(&self, x: T)`，然後試著建 `Box<dyn Storage>`。你會看到「trait 有泛型方法就不能當 trait object」的錯誤（dyn 不相容）。理解為什麼：vtable 無法為無限多種 `T` 準備入口。

## 設計筆記：什麼時候定義自己的 trait

不要為了「以後可能需要」先定義 trait。一個實作的 trait 是多餘的抽象。定義 trait 的時機：

- 已經有或**確定會有**第二種實作（`Storage`：記憶體 + 檔案，且確定會接資料庫）
- 需要在測試中替換（用 `InMemoryStorage` 測試，不碰磁碟）
- 需要讓使用者擴充（外部可以實作你的 trait）

`ReportFormat` 目前兩個實作，而且需求說「之後可能還有 JSON」，合理。如果報表只有一種，直接寫函式。

## 回顧

- trait 定義「能做什麼」；`impl Trait for Type` 讓型別獲得能力，包括別人的 trait 給你的型別、你的 trait 給別人的型別
- 泛型 + bound 是靜態分派（零成本、單態化）；`dyn Trait` 是動態分派（集合裡放不同型別、執行期決定）
- `Display`、`Add`、`Sum`、`From` 讓自訂型別融入語言；`From` 是 `?` 轉換錯誤的機制
- 一個實作不要抽 trait；有替換需求（第二種實作、測試替身）才抽

## 練習

1. 實作 `JsonReport`（手工拼字串就好，第 12 章會用 serde），加入 `render_all` 的清單。
2. 為 `Money` 實作 `Mul<i64>`，讓 `Money::from_yuan(100) * 3` 合法。注意 `Money * Money` 應該**不**合法（金額乘金額沒有意義），確認你沒有順手實作它。
3. 為 `TxKind` 實作 `FromStr`（`"D:1001"`、`"X:1001:1002"` 這種格式），然後用 `"D:1001".parse::<TxKind>()` 測試。
4. 寫 `fn save_and_verify(storage: &mut impl Storage, ledger: &Ledger) -> Result<bool, StorageError>`：存檔、讀回、比較是否相同。

## 下一章

`report::top_n` 回傳 `Vec<&Transaction>`，`Ledger::accounts` 回傳借用的迭代器。到目前為止編譯器都自己推出了這些引用能活多久。
但財務要一個「對帳單」物件：持有某個帳戶的引用、以及該帳戶所有交易的引用，可以傳來傳去、分頁顯示。
一個結構體裡裝引用，編譯器就需要你幫忙了：生命週期標註。
