# 第 13 章　智慧指標：Box、Rc、RefCell

> **場景**：程式要從「跑完就結束的批次」變成「長時間執行的櫃台系統」。
> 同一個行程裡有櫃台 A、櫃台 B（要過帳）、稽核（只讀）、風控（只讀，而且它的規則是「金額超過一萬 **且**（提款 **或** 轉帳）」這種可以無限組合的規則樹）。
> 四個元件都要存取**同一個**帳本。但第 2 章說每個值只有一個擁有者。

## Why：單一擁有者不夠用的時候

所有權規則在 95% 的情況下夠用：資料有一個清楚的擁有者，其他人借用。但有兩種情境它不夠：

**情境一：真的有多個擁有者。** 櫃台 A、櫃台 B、稽核都「持有」帳本，誰先結束都不該讓帳本消失。借用（`&Ledger`）做不到：借用要求擁有者活得比借用者久，但這四個元件的生命週期彼此獨立。

**情境二：型別的大小編譯期不知道。** 規則樹 `Rule::And(Rule, Rule)` 裡面包著 `Rule`，無限遞迴，大小算不出來。

智慧指標（smart pointer）解決這兩件事。它們是「擁有一塊堆積記憶體的結構體」，實作了 `Deref`（可以像普通引用一樣用 `.`）和 `Drop`（離開作用域時釋放）。你已經用過兩個：`String` 和 `Vec` 都是智慧指標。

## What：四個智慧指標與一個模式

### `Box<T>`：單一擁有者，資料在堆積

最簡單的智慧指標。三個用途：

```rust
// 1. 遞迴型別
enum Rule {
    AmountAbove(Money),
    And(Box<Rule>, Box<Rule>),    // 沒有 Box → error[E0072]: recursive type has infinite size
}

// 2. trait object（第 9 章）
let formats: Vec<Box<dyn ReportFormat>> = ...;

// 3. 大型資料不想在堆疊上搬動
let big: Box<[u8; 1_000_000]> = Box::new([0; 1_000_000]);
```

`Box<Rule>` 的大小是一個指標（8 bytes），所以 `Rule` 的大小有限了。使用上幾乎透明：`a.matches(tx)` 直接呼叫，不用手動解引用。

### `Rc<T>`：多個擁有者，單執行緒

Reference Counted。`Rc::clone(&x)` 不複製資料，只把計數 +1；最後一個 `Rc` 被 drop 時計數歸零，資料釋放。

```rust
let shared = Rc::new(ledger);
let a = Rc::clone(&shared);    // 計數 2
let b = Rc::clone(&shared);    // 計數 3
drop(a);                        // 計數 2
```

慣例寫 `Rc::clone(&x)` 而不是 `x.clone()`，讓讀者一眼看出這是便宜的計數增加，不是深層複製。

**`Rc<T>` 是唯讀的。** 你只能拿到 `&T`，不能拿到 `&mut T`。因為有多個擁有者，如果每個都能改，就違反了借用規則。要改，需要搭配下一個。

### `RefCell<T>`：把借用檢查搬到執行期

```rust
let cell = RefCell::new(ledger);
let r1 = cell.borrow();         // 回傳 Ref<Ledger>，像 &Ledger
let r2 = cell.borrow();         // 多個讀者 OK
drop(r1); drop(r2);
let mut w = cell.borrow_mut();  // 回傳 RefMut<Ledger>，像 &mut Ledger
```

借用規則不變（多讀或一寫），但檢查從編譯期變成**執行期**。違反規則不是編譯錯誤，是 panic：

```
already borrowed: BorrowMutError
```

這叫**內部可變性（interior mutability）**：從外面看 `cell` 是不可變的（`&RefCell<T>`），裡面的值卻可以改。
不想 panic 用 `try_borrow_mut()`，回傳 `Result`。

什麼時候值得付這個代價？當**你確定邏輯上沒有同時讀寫，但編譯器證明不了**的時候。例如四個元件透過 `Rc` 共享帳本，每次操作都是「借 → 用 → 立刻還」，不會重疊，但編譯器無法跨越四個結構體追蹤這件事。

### `Rc<RefCell<T>>`：單執行緒共享可變狀態的標準組合

```rust
struct Teller { ledger: Rc<RefCell<Ledger>> }
struct Auditor { ledger: Rc<RefCell<Ledger>> }

impl Teller {
    fn post(&mut self, tx: Transaction) {
        self.ledger.borrow_mut().apply(tx);    // 這一行結束，RefMut 就釋放
    }
}
impl Auditor {
    fn check(&self) {
        let ledger = self.ledger.borrow();
        println!("{}", ledger.total_balance());
    }
}
```

`Rc` 負責「誰擁有」，`RefCell` 負責「誰能改」。兩個關注點分開。

**只在單執行緒。** `Rc` 的計數不是原子操作，`RefCell` 的借用標記也不是。編譯器會拒絕你把它們送進執行緒（第 14 章會看到錯誤訊息，以及對應的 `Arc<Mutex<T>>`）。

### `Weak<T>`：不擁有的引用

`Rc` 有一個陷阱：循環引用。A 持有 B 的 `Rc`，B 持有 A 的 `Rc`，兩者計數永遠不歸零，記憶體洩漏。
解法是其中一方用 `Weak`：

```rust
let weak = Rc::downgrade(&shared);    // 不增加 strong count
match weak.upgrade() {                // 試著變回 Rc
    Some(rc) => { /* 資料還在 */ }
    None => { /* 所有 Rc 都被 drop 了 */ }
}
```

範例裡風控元件用 `Weak`：它是「旁觀者」，不該延長帳本的壽命。帳本被釋放後，風控的 `upgrade()` 得到 `None`，優雅停止。

典型用途：樹的子節點指向父節點、觀察者模式的觀察者清單、快取。

### 什麼時候用哪個

```
需要多個擁有者嗎？
├─ 否 → 直接擁有（T），或借用（&T / &mut T），或 Box<T>（遞迴/dyn/大型）
└─ 是 → 跨執行緒嗎？
         ├─ 否 → Rc<T>；要改 → Rc<RefCell<T>>；要打破循環 → Weak<T>
         └─ 是 → Arc<T>；要改 → Arc<Mutex<T>> 或 Arc<RwLock<T>>（第 14 章）
```

**預設不用這些。** 大部分 Rust 程式碼是普通的值與引用。看到到處都是 `Rc<RefCell<>>` 的程式，通常是在用 Rust 寫 Java，重新思考誰該擁有什麼往往能消掉大半。

### `Cell<T>`：Copy 型別的輕量版

`RefCell` 的簡化版，只給 `Copy` 型別用，沒有借用追蹤，用 `get()`/`set()` 整個換掉。適合計數器、旗標。

### `Cow<'a, T>`：寫時複製

Clone-on-Write。「大部分時候只讀借用的資料，偶爾需要修改時才複製一份」：

```rust
use std::borrow::Cow;
fn normalize(memo: &str) -> Cow<'_, str> {
    if memo.contains(' ') {
        Cow::Owned(memo.replace(' ', "_"))    // 需要改 → 配置新字串
    } else {
        Cow::Borrowed(memo)                   // 不需要改 → 零成本借用
    }
}
```

## How：規則樹與櫃台模擬

完整程式碼在 `code/ch13-smart-pointers/`。執行 `cargo run --bin desk`。

### 規則樹

```rust
pub enum Rule {
    AmountAbove(Money),
    IsWithdraw,
    IsTransfer,
    Involves(u32),
    MemoContains(String),
    And(Box<Rule>, Box<Rule>),
    Or(Box<Rule>, Box<Rule>),
    Not(Box<Rule>),
}

impl Rule {
    pub fn matches(&self, tx: &Transaction) -> bool {
        match self {
            Rule::AmountAbove(limit) => tx.amount > *limit,
            Rule::And(a, b) => a.matches(tx) && b.matches(tx),    // Box 自動解引用
            Rule::Not(r) => !r.matches(tx),
            // ...
        }
    }
    pub fn and(self, other: Rule) -> Rule { Rule::And(Box::new(self), Box::new(other)) }
}
```

建構輔助讓規則讀起來像句子：

```rust
let rule = Rule::AmountAbove(Money::from_yuan(1_000))
    .and(Rule::IsWithdraw.or(Rule::IsTransfer));
```

這是「規則引擎」的最小版本。真實系統裡規則會從設定檔或資料庫載入（serde 可以直接反序列化這個 enum，`Box` 也支援）。

### 四個元件共享帳本

```rust
let shared = Rc::new(RefCell::new(ledger));

let mut teller_a = Teller { ledger: Rc::clone(&shared), .. };
let mut teller_b = Teller { ledger: Rc::clone(&shared), .. };
let auditor = Auditor { ledger: Rc::clone(&shared), .. };
let risk = RiskMonitor { ledger: Rc::downgrade(&shared), .. };     // Weak

println!("{} strong, {} weak", Rc::strong_count(&shared), Rc::weak_count(&shared));   // 4, 1
```

執行輸出：

```
[櫃台 A] 過帳成功：帳戶 1001 存入 100.00 元
[櫃台 B] 過帳成功：帳戶 1002 提出 1500.00 元
[稽核] 總額 50600.00 元，異常！
[櫃台 A] 過帳成功：帳戶 1001 轉給 1002 3000.00 元
[櫃台 B] 過帳失敗：帳戶 1002 餘額不足：有 3500.00 元，要 99999.00 元
[風控] 規則深度 3，命中交易：[1000, 2]
try_borrow_mut 失敗：目前已有讀者
Rc 計數：1
[風控] 帳本已不存在，停止掃描
```

稽核說「異常」是對的：它的預期總額是初始值，但中間有存款與提款。這提醒我們稽核的規則應該是「總額變化 = 存款 − 提款」，練習題會要你修。

### 執行期借用衝突

```rust
let _reader = shared.borrow();
match shared.try_borrow_mut() {
    Ok(_) => ...,
    Err(_) => println!("目前已有讀者"),
}
```

把 `try_borrow_mut` 換成 `borrow_mut()` 執行一次，看 panic 訊息。這就是第 3 章的 E0502，只是搬到了執行期。
`RefCell` 的 panic 通常發生在「某個方法裡 `borrow_mut()`，然後呼叫另一個也 `borrow()` 的方法」。保持借用的範圍極短（一行用完就還）可以避免大多數問題。

### Weak 的效果

```rust
drop(teller_a); drop(teller_b); drop(auditor);
drop(shared);          // 最後一個 Rc → 帳本釋放
risk.scan();           // upgrade() 得到 None，印出「帳本已不存在」
```

如果風控用的是 `Rc` 而不是 `Weak`，帳本就不會在這裡釋放。在長時間執行的服務裡，這種「旁觀者不小心延長了資料壽命」是記憶體洩漏的常見來源。

## 設計筆記：第 14 章之前的提醒

這一章的 `Rc<RefCell<Ledger>>` 是**單執行緒**版本。真實的櫃台系統每個櫃台是一個執行緒或一個請求，那時候：

- `Rc` → `Arc`（原子計數）
- `RefCell` → `Mutex` 或 `RwLock`（真正的鎖）
- `borrow_mut()` 的 panic → `lock()` 的等待

API 形狀幾乎相同，但語義從「違反規則就崩潰」變成「違反規則就排隊」。下一章就做這件事。

## 回顧

- `Box<T>`：單一擁有者、資料在堆積。遞迴型別、trait object、大型資料
- `Rc<T>`：多個擁有者、唯讀、單執行緒。`Rc::clone` 只加計數
- `RefCell<T>`：借用規則搬到執行期，違反就 panic。給 `Rc` 加可變性
- `Weak<T>`：不擁有的引用，打破循環、當旁觀者
- 預設用普通的值與引用；到處 `Rc<RefCell<>>` 通常是設計訊號

## 練習

1. 修正稽核：讓 `Auditor` 記錄它看到的每一筆交易（透過 `history().len()` 的變化），重新計算預期總額，而不是用固定的初始值。
2. 給 `Rule` 加 `#[derive(Serialize, Deserialize)]`，寫一個測試：把上面的複合規則序列化成 JSON，印出來看，再反序列化回來比較相等。
3. 寫一個會 panic 的情境：在 `Teller::post` 裡先 `let l = self.ledger.borrow();` 讀餘額，然後在同一個作用域呼叫 `self.ledger.borrow_mut()`。觀察 panic，然後用縮小作用域的方式修好。
4. 用 `Cow<str>` 實作 `fn normalize_memo(memo: &str) -> Cow<'_, str>`：去掉前後空白、連續空白合併成一個；輸入已經乾淨時不配置。寫測試驗證 `matches!(result, Cow::Borrowed(_))`。

## 下一章

結算批次要處理百萬筆交易，一個執行緒要跑十分鐘。機器有 16 核。
把 `Rc<RefCell<>>` 換成 `Arc<Mutex<>>`，用多執行緒加速，同時親眼看看編譯器怎麼拒絕一段有資料競爭的程式。
