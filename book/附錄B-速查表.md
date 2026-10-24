# 附錄 B　速查表

一頁看完本書用到的語法。每一項標註首次出現的章節。

## 專案與工具（1）

```bash
cargo new name            # 新專案          cargo check     # 只檢查，最快
cargo run -- args         # 執行            cargo build --release
cargo test name           # 跑測試          cargo fmt / cargo clippy
cargo add serde --features derive           cargo doc --open
cargo run --bin server    # 多個 bin 時指定  cargo tree
```

## 變數與型別（1）

```rust
let x = 5;                let mut y = 5;         const MAX: u32 = 100;
let x: i64 = 1_000_000;   let f = 2.5_f64;       let b = true;   let c = '王';
let s: &str = "字面值";    let s = String::from("擁有");   let t = (1, "a", 2.0);  t.0
let arr = [1, 2, 3];      let arr = [0u8; 16];   arr.len()
x as i64                  i64::from(x)           "42".parse::<i32>()
```

整數：`i8..i128` `u8..u128` `isize` `usize`；浮點：`f32` `f64`；溢位安全：`checked_add` `saturating_sub` `wrapping_mul`

## 流程控制（1）

```rust
if a > b { } else if a == b { } else { }       let v = if cond { 1 } else { 2 };
loop { break value; }     while cond { }        for i in 0..n { }   for i in (0..=n).rev() { }
for x in &vec { }         for (i, x) in vec.iter().enumerate() { }
'outer: for .. { break 'outer; }
```

## 函式與閉包（1、7）

```rust
fn add(a: i64, b: i64) -> i64 { a + b }        // 最後一個無分號表達式是回傳值
fn nothing() { }                                // 回傳 ()
let f = |x: i64| x * 2;    let g = |x| { let y = x + 1; y * 2 };    move || captured
```

## 所有權與借用（2、3）

```rust
let b = a;            // 移動（非 Copy）/ 複製（Copy）
let b = a.clone();    // 明確深層複製
fn f(s: String)       // 取得所有權          fn f(s: &str)        // 借用（偏好）
fn f(v: &mut Vec<T>)  // 可變借用            fn f(v: &[T])        // 切片（偏好）
&v[1..3]  &s[..5]     // 切片               *r                   // 解引用
```

規則：多個 `&T` **或** 一個 `&mut T`；引用不能比值活得久；借用到最後一次使用結束。

## 結構體與方法（4）

```rust
#[derive(Debug, Clone, PartialEq)]
struct Account { id: u32, owner: String }
struct Money(i64);                 // newtype          struct Marker;   // 單元
let a = Account { id, owner: "x".into() };         let b = Account { id: 2, ..a.clone() };
impl Account {
    fn new(id: u32) -> Self { .. }          // 關聯函式  Account::new(1)
    fn get(&self) -> u32 { .. }             // 借用
    fn set(&mut self, v: u32) { .. }        // 可變借用
    fn into_owner(self) -> String { .. }    // 消耗
}
```

derive 常用：`Debug` `Clone` `Copy` `PartialEq` `Eq` `PartialOrd` `Ord` `Hash` `Default` `Serialize` `Deserialize`

## 列舉與模式比對（5）

```rust
enum TxKind { Deposit { account: u32 }, Transfer { from: u32, to: u32 }, Pending }
match kind {
    TxKind::Deposit { account } => ..,
    TxKind::Transfer { from, .. } if from == 1 => ..,     // 守衛、忽略其餘
    TxKind::Deposit { .. } | TxKind::Pending => ..,       // 多重
    _ => ..,
}
match n { 0..=9 => .., x @ 10..=99 => .., _ => .. }
if let Some(x) = opt { }        let Some(x) = opt else { return; };      matches!(k, TxKind::Pending)
```

## Option 與 Result（5、6）

```rust
Option<T>: Some(v) / None        Result<T, E>: Ok(v) / Err(e)
opt.map(f)  .and_then(f)  .unwrap_or(d)  .unwrap_or_else(f)  .unwrap_or_default()  .ok_or(err)  .filter(p)  .is_some()
res.map(f)  .map_err(f)   .and_then(f)   .unwrap_or(d)       .ok()                 .is_ok()
value?                            // Err/None 時提早回傳；可自動 From 轉換錯誤型別
.unwrap()  .expect("原因")        // 失敗即 panic；生產程式碼避免
fn main() -> Result<(), Box<dyn std::error::Error>>     // main 也可以用 ?
```

自訂錯誤（thiserror）：
```rust
#[derive(Debug, thiserror::Error)]
enum MyError {
    #[error("帳戶 {0} 不存在")] NotFound(u32),
    #[error("I/O：{0}")] Io(#[from] std::io::Error),
}
```

## 集合（7）

```rust
let mut v = vec![1, 2];  v.push(3);  v.pop();  v.len();  v[0];  v.get(0);  v.sort();  v.sort_by_key(|x| ..);
let mut m = HashMap::new();  m.insert(k, v);  m.get(&k);  m.get_mut(&k);  m.contains_key(&k);  m.remove(&k);
*m.entry(k).or_insert(0) += 1;          m.get_disjoint_mut([&k1, &k2])
BTreeMap  HashSet  BTreeSet  VecDeque   // 有序 map、集合、有序集合、雙端佇列
"中文".chars().count()   .len()（bytes）  .split(',')  .trim()  .to_string()  .as_str()  format!("{a}-{b}")
```

## 迭代器（7）

```rust
v.iter()  v.iter_mut()  v.into_iter()           // &T / &mut T / T
.map(f) .filter(p) .filter_map(f) .flat_map(f) .enumerate() .zip(o) .take(n) .skip(n) .rev() .chain(o) .take_while(p) .peekable()
.collect::<Vec<_>>()  .collect::<HashMap<_,_>>()  .collect::<Result<Vec<_>,_>>()  .collect::<String>()
.sum::<i64>() .count() .min() .max() .max_by_key(f) .fold(init, f) .any(p) .all(p) .find(p) .position(p) .partition(p) .last()
slice.windows(n)  slice.chunks(n)  slice.split_at_mut(i)
```

## 模組（8）

```rust
pub mod account;                  // 載入 src/account.rs 或 src/account/mod.rs
use crate::money::Money;          // 絕對路徑          use super::x;  use self::y;
pub use money::Money;             // re-export
pub / pub(crate) / pub(super) / 預設私有
//! 模組文件        /// 項目文件
```

## Trait 與泛型（9）

```rust
trait Storage { fn save(&mut self, l: &Ledger) -> Result<(), E>;  fn name(&self) -> String { "預設".into() } }
impl Storage for FileStorage { .. }
fn f<T: Storage + Clone>(s: &T)                  // 泛型 + bound（靜態分派）
fn f(s: &impl Storage)                           // 同上，簡寫
fn f<T>(s: &T) where T: Storage                  // where 子句
fn f(s: &dyn Storage)  Box<dyn Storage>          // trait object（動態分派）
fn make() -> impl Iterator<Item = u32>           // 回傳位置的 impl Trait
impl fmt::Display for Money { fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result { write!(f, "..") } }
impl Add for Money { type Output = Money; fn add(self, rhs: Money) -> Money { .. } }
impl From<io::Error> for MyError { fn from(e: io::Error) -> Self { .. } }
```

## 生命週期（10）

```rust
fn longest<'a>(a: &'a str, b: &'a str) -> &'a str
struct View<'a> { data: &'a [u8] }      impl<'a> View<'a> { fn first(&self) -> &'a u8 { .. } }
fn f(s: &str) -> &str                   // 省略：單一輸入
fn f(&self, x: &str) -> &str            // 省略：跟 self
&'static str                            // 字面值；T: 'static = 不含短命引用
Statement<'_>                           // 匿名生命週期
```

## 測試（11）

```rust
#[cfg(test)] mod tests { use super::*;
    #[test] fn it_works() { assert_eq!(1 + 1, 2); assert!(cond, "訊息 {x}"); assert_ne!(a, b); }
    #[test] #[should_panic(expected = "訊息")] fn panics() { .. }
    #[test] fn with_result() -> Result<(), E> { ..; Ok(()) }
    #[test] #[ignore] fn slow() { .. }
}
// tests/*.rs 整合測試（只能用公開 API）；/// 文件註解中的 ``` 區塊是文件測試
// #[tokio::test] async fn ..   非同步測試
```

## 外部套件（12）

```toml
[dependencies]
serde = { version = "1", features = ["derive"] }
[dev-dependencies]
tempfile = "3"
```
```rust
#[derive(Serialize, Deserialize)] #[serde(tag = "type", rename_all = "snake_case")] enum ..
#[serde(transparent)] #[serde(flatten)] #[serde(default)] #[serde(rename = "x")] #[serde(skip)] #[serde(deserialize_with = "f")]
serde_json::to_string_pretty(&v)?    serde_json::from_str(&s)?
#[derive(clap::Parser)] struct Cli { #[arg(long, default_value = "x")] path: PathBuf, #[command(subcommand)] cmd: Cmd }
anyhow: Result<T>  .context("..")  .with_context(|| ..)  bail!("..")
```

## 智慧指標（13）

```rust
Box<T>              // 堆積、單一擁有者；遞迴型別、dyn Trait
Rc<T>               // 多擁有者、唯讀、單執行緒        Rc::clone(&x)  Rc::strong_count(&x)
RefCell<T>          // 執行期借用檢查                 .borrow()  .borrow_mut()  .try_borrow_mut()
Rc<RefCell<T>>      // 單執行緒共享可變
Weak<T>             // Rc::downgrade(&x)  weak.upgrade() -> Option<Rc<T>>
Cell<T>  Cow<'a, T>
```

## 並行（14）

```rust
let h = thread::spawn(move || { .. });  h.join().unwrap();
thread::scope(|s| { s.spawn(|| ..); });                         // 可借用
Arc<Mutex<T>>  Arc<RwLock<T>>      x.lock().unwrap()  x.read().unwrap()  x.write().unwrap()
let (tx, rx) = mpsc::channel();  tx.send(v)?;  for v in rx { }  // 所有 tx drop 後結束
AtomicUsize::new(0).fetch_add(1, Ordering::Relaxed)
Send：可移到別的執行緒   Sync：&T 可跨執行緒分享
```

## 非同步（15）

```rust
#[tokio::main] async fn main() { .. }
async fn f() -> T { other().await }                 // Future 惰性，await 才執行
tokio::spawn(async move { .. })                     // 新任務
tokio::task::spawn_blocking(|| cpu_heavy()).await   // CPU 密集丟出去
tokio::sync::{Mutex, RwLock, mpsc}                  // 可跨 await 的鎖與 channel
tokio::time::sleep(d).await                         // 不要用 std::thread::sleep
axum: Router::new().route("/x/{id}", get(h)).with_state(s);   State(s)  Path(id)  Json(body)  Query(q)
```

## 格式化

```rust
{}  {:?}  {:#?}             // Display / Debug / 多行 Debug
{x}  {0}  {name}            // 變數 / 位置 / 具名
{:>8}  {:<8}  {:^8}  {:08}  {:.2}  {:08.2}  {:x}  {:#b}  {:e}
format!()  print!()  println!()  eprintln!()  write!(f, ..)  panic!()  assert!()  todo!()  unreachable!()  dbg!(x)
```
