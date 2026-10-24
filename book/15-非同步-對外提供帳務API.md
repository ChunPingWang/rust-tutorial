# 第 15 章　非同步：對外提供帳務 API

> **場景**：訂單系統、客服系統、App 後端都要即時查餘額、即時入帳。尖峰時幾千個連線同時掛著，大部分時間在等網路。
> 每個連線開一個執行緒？幾千個執行緒的記憶體與切換成本會先把機器壓垮。
> 我們要一個 HTTP API，用少量執行緒服務大量連線。

## Why：為什麼執行緒不夠，需要 async

第 14 章的執行緒適合 **CPU 密集**的工作：驗證、計算，每個核心一個執行緒剛好。

API 伺服器是另一種形狀：**I/O 密集**。一個請求的生命週期裡，99% 的時間在等：等客戶端送完請求、等資料庫回應、等下游服務。
如果每個連線佔一個執行緒，這個執行緒 99% 的時間在睡覺，但它的堆疊記憶體（預設 2MB）和作業系統排程成本一直在。一萬個連線 = 20GB 堆疊。

非同步（async）的想法：**一個任務在等待時，把執行緒讓給別的任務**。幾千個任務共用幾個執行緒（通常等於核心數）。等待不佔資源。

其他語言的做法對照：

- **Go**：goroutine，執行期自動排程，語法上看不到 async。簡單，但執行期有 GC 與排程器的成本
- **JavaScript / Python**：`async`/`await`，單執行緒事件迴圈
- **Java**：虛擬執行緒（Loom），接近 Go
- **Rust**：`async`/`await` 語法 + **可選的執行器**（tokio 是事實標準）。語言只定義「什麼是 Future」，怎麼跑由套件決定。零成本：一個 async 任務編譯後是一個狀態機，沒有堆疊、沒有 GC

Rust 選這條路是因為它要能跑在沒有作業系統的嵌入式裝置上，不能把排程器寫死在語言裡。代價是初學者要多認識一個概念（執行器）。

## What：async/await、Future、執行器

### 語法

```rust
async fn get_account(id: u32) -> Result<AccountView, ApiError> {
    let ledger = state.ledger.read().await;    // 等鎖：等待期間執行緒去服務別的請求
    let acct = ledger.account(id)?;
    Ok(view(acct))
}
```

- `async fn` 的回傳值不是 `T`，而是 `impl Future<Output = T>`：一個「以後會算出 T」的東西
- `.await` 等待一個 Future 完成。**只能在 async 函式裡用**
- `.await` 是讓出點（yield point）：只有在這裡任務才會暫停、執行緒才會去跑別的任務

**Future 是惰性的。** 呼叫 `async fn` 什麼都不會發生，直到你 `.await` 它或把它交給執行器。這和 JavaScript 的 Promise（建立即執行）不同。

### 執行器：tokio

```rust
#[tokio::main]
async fn main() {
    // 這裡的程式碼在 tokio 執行器上跑
}
```

`#[tokio::main]` 展開成「建立執行器、把 async main 丟進去跑」。沒有執行器，async 程式碼不會動。
tokio 提供：多執行緒排程器、非同步的 TCP/檔案/計時器、非同步的 `Mutex`/`RwLock`/channel。

**同步鎖 vs 非同步鎖：**

| | `std::sync::RwLock` | `tokio::sync::RwLock` |
|---|---|---|
| 等鎖時 | 卡住整個執行緒 | 讓出執行緒，`.await` |
| guard 可以跨 `.await` 嗎 | **不可以**（會卡死其他任務） | 可以 |
| 效能 | 更快 | 稍慢 |
| 用在 | 鎖的範圍內沒有 `.await`，且極短 | 持鎖期間需要 `.await`（例如持鎖時查資料庫） |

範例用 tokio 的 `RwLock`，因為 handler 裡 `read().await` 本身就是一個 await。

### 不要在 async 裡做 CPU 密集工作

```rust
async fn bad() {
    heavy_computation();     // 卡住執行緒 2 秒，這段時間這個執行緒上的其他幾百個任務全部停擺
}
async fn good() {
    tokio::task::spawn_blocking(|| heavy_computation()).await.unwrap();   // 丟到專用的執行緒池
}
```

async 執行緒是給「等待」用的。第 14 章的結算放在 `spawn_blocking` 或獨立的執行緒。

### axum：路由、抽取器、回應

```rust
Router::new()
    .route("/accounts/{id}", get(get_account))
    .route("/transactions", post(post_transaction))
    .with_state(state)
```

handler 的參數是**抽取器（extractor）**，axum 從請求裡抽出你要的東西：

```rust
async fn post_transaction(
    State(state): State<AppState>,          // 共享狀態
    Path(id): Path<u32>,                    // URL 路徑參數
    Json(body): Json<NewTransaction>,       // JSON body，自動反序列化，格式錯直接回 422
) -> Result<(StatusCode, Json<Posted>), ApiError>
```

回傳任何實作 `IntoResponse` 的東西：`Json<T>`、`(StatusCode, Json<T>)`、`String`、或你自己的錯誤型別。

## How：帳本 API

完整程式碼在 `code/ch15-async/src/bin/server.rs`。`cargo run --bin server`，另開一個終端機用 curl。

### 共享狀態

```rust
#[derive(Clone)]
struct AppState {
    ledger: Arc<RwLock<Ledger>>,
    next_id: Arc<AtomicU64>,
}
```

第 14 章的 `Arc<RwLock<>>`，換成 tokio 版。axum 要求 state 是 `Clone`（每個請求 clone 一份，`Arc` 的 clone 很便宜）。

### 中介型別：API 的形狀 ≠ 核心型別

```rust
#[derive(Serialize)]
struct AccountView {
    id: u32,
    owner: String,
    balance: String,        // "1234.56" 給人看
    balance_cents: i64,     // 整數給程式用
}

#[derive(Deserialize)]
struct NewTransaction {
    #[serde(flatten)] kind: TxKind,
    amount: String,          // 接受 "250.50"
    #[serde(default)] memo: String,
    date: Option<NaiveDate>, // 不給就用今天
}
```

第 12 章的原則再用一次。API 的欄位形狀是給客戶端方便的（字串金額、可選日期、不需要客戶端給 ID），核心 `Transaction` 不受影響。

### 錯誤 → HTTP 狀態碼

```rust
impl From<LedgerError> for ApiError {
    fn from(e: LedgerError) -> Self {
        match e {
            LedgerError::AccountNotFound(_) => ApiError::NotFound(e.to_string()),
            LedgerError::InsufficientFunds { .. } | LedgerError::Overflow => ApiError::Conflict(e.to_string()),
            LedgerError::InvalidAmount(_) | LedgerError::SameAccountTransfer(_) | ... => ApiError::BadRequest(e.to_string()),
        }
    }
}

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        let (status, message) = match self { ... };
        (status, Json(json!({ "error": message }))).into_response()
    }
}
```

第 6 章定義的精確錯誤 enum 在這裡回報：**每一種業務錯誤對應一個明確的狀態碼**，而且 `match` 窮舉保證新增錯誤時不會忘記對應。
有了 `From`，handler 裡 `ledger.apply(tx)?` 直接把 `LedgerError` 變成 HTTP 回應。

### 寫鎖範圍

```rust
let balances = {
    let mut ledger = state.ledger.write().await;
    ledger.apply(tx)?;
    affected.iter().filter_map(|id| ledger.account(*id).ok()).map(view).collect()
};   // 寫鎖在這裡釋放
info!(id, "過帳成功");
```

解析金額、產生 ID、建 `Transaction` 都在取鎖之前。第 14 章的原則：鎖的範圍只包必要的部分。

### 實際跑

```
$ curl localhost:3000/accounts/1001
{"id":1001,"owner":"王小明","balance":"50000.00","balance_cents":5000000}

$ curl -X POST localhost:3000/transactions -H 'content-type: application/json' \
       -d '{"type":"transfer","from":1001,"to":1002,"amount":"1000"}'
{"id":2,"balances":[{"id":1001,...,"balance":"49000.00"},{"id":1002,...,"balance":"3000.00"}]}   [201]

$ curl -X POST localhost:3000/transactions -d '{"type":"withdraw","account":1003,"amount":"99999"}'
{"error":"帳戶 1003 餘額不足：有 800.00 元，要 99999.00 元"}   [409]

$ curl localhost:3000/accounts/9999
{"error":"帳戶 9999 不存在"}   [404]

$ curl -X POST localhost:3000/transactions -d '{"type":"refund"}'
Failed to deserialize the JSON body into the target type: missing field `amount`   [422]
```

最後一個 422 是 axum 的 `Json` 抽取器自動回的：body 對不上 `NewTransaction` 的形狀，handler 根本不會被呼叫。

### 證明 await 不阻塞

`/health/slow` 睡 500ms。同時發三個 slow 和一個快的請求：

```
$ time (curl /health/slow & curl /health/slow & curl /health/slow & curl /accounts/1002; wait)
{"id":1002,...}
0.513 total
```

四個請求總共 0.5 秒，而不是 1.5 秒。快的請求沒有被慢的擋住。如果 `slow_health` 用的是 `std::thread::sleep`（阻塞整個執行緒）而不是 `tokio::time::sleep`，結果就會不同。這是 async 程式最常見的 bug：在 async 函式裡呼叫了阻塞的 API。

### 日誌：tracing

```rust
info!(id, "過帳成功");
warn!(%status, %message, "請求失敗");
```

`tracing` 是結構化日誌：欄位是 key=value，不是拼進字串。`RUST_LOG=debug cargo run --bin server` 調整層級。
生產環境會接 JSON 輸出與分散式追蹤（OpenTelemetry），`tracing` 的 span 概念是為此設計的。

## 測試 async 程式碼

```rust
#[tokio::test]
async fn deposit_returns_201() {
    let app = app(seed());
    let response = app.oneshot(Request::post("/transactions").body(...)).await.unwrap();
    assert_eq!(response.status(), StatusCode::CREATED);
}
```

`#[tokio::test]` 取代 `#[test]`。axum 的 `Router` 實作了 tower 的 `Service`，可以不開埠直接 `oneshot` 一個請求進去。`app()` 函式刻意和 `main` 分開就是為了這個。練習題會做。

## 設計筆記：什麼時候不要 async

- 命令列工具、批次程式：不需要。第 12 章的 CLI 全程同步，正確
- CPU 密集：用執行緒（第 14 章）
- 只有幾十個連線的內部服務：同步 + 執行緒池（例如 `std::net` + 每連線一執行緒）其實夠用，而且更好除錯
- 需要同時處理成百上千個 I/O 等待：async

async 增加了認知負擔（執行器、`Send` bound、不能跨 await 持同步鎖、`Pin`）。沒有必要時不要用。

## 回顧

- async 解決的是 **I/O 等待**佔多數的場合：少量執行緒服務大量連線。CPU 密集用執行緒
- `async fn` 回傳 Future，惰性，`.await` 才執行；需要執行器（tokio）
- `.await` 是讓出點；在 async 裡呼叫阻塞 API 會卡住整個執行緒上的所有任務
- 同步鎖不可跨 `.await`；持鎖期間要 `.await` 用 tokio 的鎖
- API 的請求/回應型別與核心型別分開；業務錯誤 `match` 成 HTTP 狀態碼
- 把 `Router` 建構抽成函式，測試可以不開埠直接打

## 練習

1. 加 `GET /accounts/{id}/statement?from=2026-10-01&to=2026-10-31`，用 `Query<T>` 抽取器接參數，回傳第 10 章的對帳單。注意 `Statement<'a>` 借用了鎖裡的帳本，不能直接回傳；要轉成一個擁有資料的回應型別。這是「借用的視圖」與「跨邊界的資料」的典型衝突。
2. 寫 `#[tokio::test]`：加 `axum-test` 或直接用 `tower::ServiceExt::oneshot`，測「存款回 201 且餘額正確」「餘額不足回 409」。
3. 把 `slow_health` 的 `tokio::time::sleep` 換成 `std::thread::sleep`，重跑「三個慢 + 一個快」的實驗，觀察差異。
4. 加一個背景任務：`tokio::spawn` 一個每 10 秒把帳本存成 JSON 的迴圈（第 12 章的 `JsonFileStorage`）。思考：存檔是阻塞 I/O，該用 `tokio::fs` 還是 `spawn_blocking`？

## 下一章

API 可以跑了。上線前還有一串清單：release 編譯、clippy、安全稽核、`unsafe` 到底是什麼、怎麼打包成一個容器、怎麼設定與監控。最後一哩。
