# 附錄 A　常見編譯錯誤解讀

Rust 編譯器的錯誤訊息是學習的主要來源。每一個錯誤都有編號（E0382 等），`rustc --explain E0382` 可以看完整說明。
這份附錄按本書出現順序整理，每一條附上**它在帳本系統裡代表什麼 bug**。

## 讀錯誤訊息的方法

```
error[E0382]: borrow of moved value: `memo`          ← 一句話說明，含錯誤編號
 --> src/main.rs:4:15                                 ← 位置：檔案:行:列
  |
2 |     let memo = String::from("十月薪資");
  |         ---- move occurs because `memo` has type `String`, which does not implement the `Copy` trait
3 |     let memo_in_ledger = memo;
  |                          ---- value moved here    ← 原因發生的地方
4 |     println!("{memo}");
  |               ^^^^^^ value borrowed here after move   ← 錯誤發生的地方
  |
  = help: consider cloning the value if the performance cost is acceptable   ← 建議（不一定是最好的解法）
```

讀的順序：**先看 `^^^^` 指的那一行（錯誤處），再往上找 `----` 標的那一行（原因處）**，最後才看 help。help 常建議 `clone`，那通常不是最好的解法。

## 所有權與借用（第 2、3 章）

### E0382：borrow of moved value / use of moved value

**意思**：值的所有權已經移走，你還想用它。
**帳本比喻**：交易紀錄已經交給帳本保管，你手上沒有了。
**解法**：依優先順序：(1) 改成借用 `&`；(2) 調整順序，先用完再移動；(3) 如果真的需要兩份，`clone()`。

### E0502：cannot borrow `x` as mutable because it is also borrowed as immutable

**意思**：有人在讀的時候，你想改。
**帳本比喻**：對帳單還在用，你想刪帳戶。
**解法**：調整順序，讓讀的部分先結束（NLL 會在最後一次使用時結束借用）。或把讀的結果複製出來（例如 `let n = v.len();` 再修改 `v`）。

### E0499：cannot borrow `x` as mutable more than once at a time

**意思**：兩個 `&mut` 同時存在。
**常見情境**：從同一個集合取兩個元素的可變引用（轉帳的來源與目的）。
**解法**：`split_at_mut`、`HashMap::get_disjoint_mut`、或分兩步做。

### E0506：cannot assign to `x` because it is borrowed

**意思**：有引用指著它的時候，你想整個換掉。
**解法**：同 E0502。

### E0596：cannot borrow `x` as mutable, as it is not declared as mutable

**意思**：忘了 `mut`。
**解法**：`let mut x`，或者參數改成 `&mut T`。先想一下：這個東西真的該被改嗎？

### E0384：cannot assign twice to immutable variable

**意思**：忘了 `mut`。
**解法**：加 `mut`。或者用遮蔽（`let x = ...;` 再 `let x = ...;`）如果語義上是「新的值」而不是「修改」。

### E0507：cannot move out of `x` which is behind a shared reference

**意思**：你透過 `&T` 拿到一個非 Copy 的欄位，想把它搬走。
**常見情境**：`match &self.kind { TxKind::Deposit { memo } => memo }` 想把 `String` 拿出來。
**解法**：`.clone()`、或改成回傳 `&String`、或用 `std::mem::take`。

## 型別（第 1、4、5 章）

### E0308：mismatched types

**意思**：型別不符。最常見的錯誤。
**常見情境**：
- `i32` 與 `i64` 相加：Rust 不自動轉型。用 `as i64` 或 `i64::from(x)`
- 函式最後一行多了分號：回傳 `()` 而不是值
- `&String` 傳給要 `String` 的參數：加 `.clone()` 或改參數為 `&str`
- `if` 的兩個分支型別不同

### E0277：the trait bound `T: Trait` is not satisfied

**意思**：這個型別沒有實作需要的 trait。
**常見情境**：
- `println!("{}", x)` 但 `x` 沒有 `Display`：用 `{:?}` 或實作 `Display`
- `HashMap` 的 key 沒有 `Hash + Eq`：derive
- `.sum()` 但型別沒有 `Sum`：第 9 章
- **`Rc<RefCell<T>>` cannot be sent between threads safely**：缺 `Send`，換 `Arc<Mutex<T>>`（第 14 章）
- `?` 用在回傳型別不是 `Result` 的函式：改回傳型別，或用 `match`

### E0004：non-exhaustive patterns

**意思**：`match` 沒有涵蓋所有情況。
**帳本比喻**：新增了一種交易種類，有地方忘了處理。**這是好事，是重構的安全網。**
**解法**：補上缺的分支。不要用 `_ => {}` 偷懶，除非真的所有其他情況都該忽略。

### E0423 / E0603 / E0451 / E0624：private

**意思**：你想用的東西是私有的（欄位、函式、型別、建構子）。
**帳本比喻**：想直接改餘額，繞過驗證。模組系統在守不變量（第 8 章）。
**解法**：用公開的方法；或者如果你是作者，想清楚它該不該公開。

### E0599：no method named `x` found

**常見原因**：
- 打錯字
- 需要的 trait 沒有 `use` 進來（例如 `std::io::Read`、`std::fmt::Write`）
- 型別不是你以為的（例如 `Option<T>` 而不是 `T`，忘了 `?` 或 `unwrap`）
- `Iterator` 方法用在 `Vec` 上：先 `.iter()`

### E0282：type annotations needed

**意思**：編譯器推不出型別。
**常見情境**：`collect()`、`parse()`、`sum()`。
**解法**：`let v: Vec<_> = ...` 或 `.collect::<Vec<_>>()`。

## 生命週期（第 10 章）

### E0106：missing lifetime specifier

**意思**：簽名裡有引用，但編譯器推不出它借自誰。
**常見情境**：兩個引用參數一個引用回傳；結構體存引用。
**解法**：加 `'a`。先想清楚回傳值借自哪個參數。

### E0597：`x` does not live long enough

**意思**：引用比被借的值活得久。
**帳本比喻**：對帳單指向已經被釋放的帳戶。
**解法**：讓值活久一點（宣告在外層作用域），或改存值而不是引用。

### E0515：cannot return reference to local variable

**意思**：回傳了函式內部變數的引用，函式結束它就沒了。
**解法**：回傳值（`String` 而不是 `&str`）。**函式內產生的新資料一定以值回傳。**

### lifetime may not live long enough

**意思**：你標的生命週期關係和實際不符。
**常見情境**：把兩個獨立的生命週期寫成同一個 `'a`；或 `impl<'a>` 裡的方法回傳 `&self` 借用卻標成 `'a`。
**解法**：重新想清楚每個引用借自誰。

## 並行（第 14 章）

### E0373：closure may outlive the current function, but it borrows `x`

**意思**：`thread::spawn` 的閉包借用了外面的變數，但執行緒可能活得比函式久。
**解法**：`move` 閉包，或用 `thread::scope`。

### E0277：`T` cannot be shared between threads safely（缺 `Sync`）

**解法**：`Mutex`/`RwLock` 包起來；`RefCell` 換 `Mutex`；`Rc` 換 `Arc`。

### future cannot be sent between threads safely（第 15 章）

**意思**：async 函式裡有一個不是 `Send` 的值跨越了 `.await`。
**常見原因**：持著 `std::sync::MutexGuard` 或 `Rc` 跨 await。
**解法**：縮小作用域讓它在 `.await` 之前 drop；或換 tokio 的鎖。

## 警告也要讀

| 警告 | 意思 | 處理 |
|---|---|---|
| unused variable | 沒用到的變數 | 刪掉，或前綴 `_` 表示刻意不用 |
| unused `Result` that must be used | 忽略了可能的錯誤 | `?`、`match`、或明寫 `let _ =`（確定要忽略） |
| field is never read | 結構體欄位沒人讀 | 刪掉，或它只在 Debug 輸出用（考慮是否真的需要） |
| variant is never constructed | enum 變體沒人建立 | 刪掉，或它是未來的擴充點（加 `#[allow(dead_code)]` 並註解原因） |

CI 裡用 `-D warnings` 讓警告變錯誤，強迫處理。

## 當你卡住時

1. 完整讀錯誤訊息，包括 `= note:` 和 `= help:`
2. `rustc --explain EXXXX`
3. 把問題縮小：複製到一個新的 `cargo new` 專案，刪到最小能重現的程式碼。通常刪的過程中就懂了
4. 想清楚**所有權去哪了**、**誰借了誰**。90% 的錯誤是這兩個問題
5. 不要反射性地加 `clone()`、`unwrap()`、`'static`、`Rc<RefCell<>>`。它們讓程式編譯通過，但問題只是被藏起來
