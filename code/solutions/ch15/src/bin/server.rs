//! 第 15 章：帳本 HTTP API。
//!
//! ```text
//! cargo run --bin server
//! curl localhost:3000/accounts/1001
//! curl -X POST localhost:3000/transactions -H 'content-type: application/json' \
//!      -d '{"type":"deposit","account":1001,"amount":"250.50","memo":"API 入帳"}'
//! curl localhost:3000/report
//! ```

use std::sync::Arc;
use std::time::Duration;

use axum::extract::{Path, Query, State};
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum::routing::{get, post};
use axum::{Json, Router};
use chrono::{Local, NaiveDate};
use serde::{Deserialize, Serialize};
use tokio::sync::RwLock;
use tracing::{info, warn};

use ledger::{report, JsonFileStorage, Ledger, LedgerError, Money, Storage, Transaction, TxKind};

/// 共享狀態。tokio 的 RwLock 和 std 的差別：等待鎖時不會卡住執行緒，而是讓出給其他任務。
#[derive(Clone)]
struct AppState {
    ledger: Arc<RwLock<Ledger>>,
    next_id: Arc<std::sync::atomic::AtomicU64>,
}

// ---------- API 的請求/回應型別：和核心型別分開（第 12 章的中介型別原則） ----------

#[derive(Serialize)]
struct AccountView {
    id: u32,
    owner: String,
    balance: String, // 給人看的 "1234.56"，而不是分
    balance_cents: i64,
}

#[derive(Deserialize)]
struct NewTransaction {
    #[serde(flatten)]
    kind: TxKind,
    /// 接受 "250.50" 這種字串，用 Money 的 FromStr 解析
    amount: String,
    #[serde(default)]
    memo: String,
    /// 不給就用今天
    date: Option<NaiveDate>,
}

#[derive(Serialize)]
struct Posted {
    id: u64,
    balances: Vec<AccountView>,
}

#[derive(Serialize)]
struct ReportView {
    total_balance: String,
    total_deposits: String,
    transactions: usize,
    rapid_withdrawal_accounts: Vec<u32>,
}

/// API 層的錯誤：把 LedgerError 對應到 HTTP 狀態碼
enum ApiError {
    NotFound(String),
    BadRequest(String),
    Conflict(String),
}

impl From<LedgerError> for ApiError {
    fn from(e: LedgerError) -> Self {
        match e {
            LedgerError::AccountNotFound(_) => ApiError::NotFound(e.to_string()),
            LedgerError::InsufficientFunds { .. } | LedgerError::Overflow => {
                ApiError::Conflict(e.to_string())
            }
            LedgerError::InvalidAmount(_)
            | LedgerError::SameAccountTransfer(_)
            | LedgerError::DuplicateAccount(_) => ApiError::BadRequest(e.to_string()),
        }
    }
}

/// 讓 ApiError 可以直接當 handler 的回傳值
impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        let (status, message) = match self {
            ApiError::NotFound(m) => (StatusCode::NOT_FOUND, m),
            ApiError::BadRequest(m) => (StatusCode::BAD_REQUEST, m),
            ApiError::Conflict(m) => (StatusCode::CONFLICT, m),
        };
        warn!(%status, %message, "請求失敗");
        (status, Json(serde_json::json!({ "error": message }))).into_response()
    }
}

// ---------- handlers：每一個都是 async fn ----------

async fn get_account(
    State(state): State<AppState>,
    Path(id): Path<u32>,
) -> Result<Json<AccountView>, ApiError> {
    let ledger = state.ledger.read().await; // .await：等鎖的時候讓出執行緒
    let acct = ledger.account(id)?;
    Ok(Json(view(acct)))
}

async fn list_accounts(State(state): State<AppState>) -> Json<Vec<AccountView>> {
    let ledger = state.ledger.read().await;
    let mut v: Vec<AccountView> = ledger.accounts().map(view).collect();
    v.sort_by_key(|a| a.id);
    Json(v)
}

async fn post_transaction(
    State(state): State<AppState>,
    Json(body): Json<NewTransaction>,
) -> Result<(StatusCode, Json<Posted>), ApiError> {
    let amount: Money = body
        .amount
        .parse()
        .map_err(|e: ledger::money::ParseMoneyError| ApiError::BadRequest(e.to_string()))?;
    let id = state
        .next_id
        .fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    let tx = Transaction {
        id,
        date: body.date.unwrap_or_else(|| Local::now().date_naive()),
        kind: body.kind,
        amount,
        memo: body.memo,
    };
    let affected = tx.affected_accounts();

    // 寫鎖的範圍：只包住 apply 與讀回餘額
    let balances = {
        let mut ledger = state.ledger.write().await;
        ledger.apply(tx)?;
        affected
            .iter()
            .filter_map(|id| ledger.account(*id).ok())
            .map(view)
            .collect()
    };
    info!(id, "過帳成功");
    Ok((StatusCode::CREATED, Json(Posted { id, balances })))
}

async fn get_report(State(state): State<AppState>) -> Json<ReportView> {
    let ledger = state.ledger.read().await;
    Json(ReportView {
        total_balance: ledger.total_balance().to_string(),
        total_deposits: report::total_deposits(&ledger).to_string(),
        transactions: ledger.history().len(),
        rapid_withdrawal_accounts: report::rapid_withdrawals(&ledger),
    })
}

// ---------- 練習 1：對帳單 ----------

#[derive(Deserialize)]
struct StatementQuery {
    from: NaiveDate,
    to: NaiveDate,
}

/// 回應型別**擁有**資料。Statement<'a> 借用了鎖裡的帳本，不能跨出 handler，
/// 所以在持鎖期間把需要的欄位複製出來。這是「借用的視圖」與「跨邊界的資料」的典型衝突與解法。
#[derive(Serialize)]
struct StatementView {
    account: u32,
    owner: String,
    from: NaiveDate,
    to: NaiveDate,
    entries: Vec<String>,
    net_change: String,
    balance: String,
}

async fn get_statement(
    State(state): State<AppState>,
    Path(id): Path<u32>,
    Query(q): Query<StatementQuery>,
) -> Result<Json<StatementView>, ApiError> {
    if q.from > q.to {
        return Err(ApiError::BadRequest(format!(
            "from ({}) 不能晚於 to ({})",
            q.from, q.to
        )));
    }
    let ledger = state.ledger.read().await;
    let stmt = ledger.statement(id, q.from, q.to)?; // 借用 ledger
    Ok(Json(StatementView {
        account: stmt.account.id,
        owner: stmt.account.owner.clone(),
        from: q.from,
        to: q.to,
        entries: stmt.entries.iter().map(|t| t.to_string()).collect(),
        net_change: stmt.net_change().to_string(),
        balance: stmt.account.balance().to_string(),
    })) // stmt 與 ledger 的借用在這裡結束；StatementView 不含任何引用
}

/// 練習 3 的對照：把 tokio::time::sleep 換成 std::thread::sleep 會阻塞整個執行緒，
/// 同一執行緒上的其他請求全部跟著等。用環境變數切換以便實驗。
async fn slow_health_blocking() -> &'static str {
    std::thread::sleep(Duration::from_millis(500)); // 錯誤示範！
    "ok（阻塞版：這 500ms 期間本執行緒上的其他請求都被卡住）"
}

/// 模擬一個慢的外部呼叫（例如查匯率）：示範 await 期間不會阻塞其他請求
async fn slow_health() -> &'static str {
    tokio::time::sleep(Duration::from_millis(500)).await;
    "ok（這個請求睡了 500ms，但同時間其他請求照常服務）"
}

fn view(a: &ledger::Account) -> AccountView {
    AccountView {
        id: a.id,
        owner: a.owner.clone(),
        balance: a.balance().to_string(),
        balance_cents: a.balance().cents(),
    }
}

fn seed() -> Ledger {
    let mut l = Ledger::new();
    l.open(1001, "王小明", Money::from_yuan(50_000)).unwrap();
    l.open(1002, "李小華", Money::from_yuan(2_000)).unwrap();
    l.open(1003, "張大同", Money::from_yuan(800)).unwrap();
    l
}

static SNAPSHOT_STATE: std::sync::OnceLock<AppState> = std::sync::OnceLock::new();

pub fn app(ledger: Ledger) -> Router {
    let state = AppState {
        ledger: Arc::new(RwLock::new(ledger)),
        next_id: Arc::new(std::sync::atomic::AtomicU64::new(1)),
    };
    let _ = SNAPSHOT_STATE.set(state.clone()); // 給背景存檔任務用；測試裡多次呼叫 app() 只會記住第一個
    Router::new()
        .route("/accounts", get(list_accounts))
        .route("/accounts/{id}", get(get_account))
        .route("/transactions", post(post_transaction))
        .route("/report", get(get_report))
        .route("/health/slow", get(slow_health))
        .route("/health/slow-blocking", get(slow_health_blocking))
        .route("/accounts/{id}/statement", get(get_statement))
        .with_state(state)
}

/// #[tokio::main] 把 async fn main 包進 tokio 執行器。沒有它，async fn 不會跑。
#[tokio::main]
async fn main() {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env().unwrap_or_else(|_| "info".into()),
        )
        .init();

    let addr = std::env::var("LEDGER_ADDR").unwrap_or_else(|_| "127.0.0.1:3000".to_string());
    let listener = tokio::net::TcpListener::bind(&addr)
        .await
        .expect("綁定埠失敗");
    info!(%addr, "帳本 API 啟動");
    let router = app(seed());
    // 練習 4：背景任務，每 10 秒存檔一次。
    // 存檔是阻塞 I/O（serde_json + std::fs），放在 spawn_blocking 讓它在專用執行緒池跑，
    // 不佔用 async 執行緒。tokio::fs 也可以，但 JsonFileStorage 是同步 API，spawn_blocking 最直接。
    if let Some(state) = SNAPSHOT_STATE.get() {
        let state = state.clone();
        let path =
            std::env::var("LEDGER_SNAPSHOT").unwrap_or_else(|_| "ledger-snapshot.json".into());
        tokio::spawn(async move {
            let mut ticker = tokio::time::interval(Duration::from_secs(10));
            loop {
                ticker.tick().await;
                let snapshot = state.ledger.read().await.clone(); // 讀鎖只持有 clone 的瞬間
                let path = path.clone();
                let result = tokio::task::spawn_blocking(move || {
                    JsonFileStorage::new(&path).save(&snapshot)
                })
                .await;
                match result {
                    Ok(Ok(())) => info!("背景存檔完成"),
                    Ok(Err(e)) => warn!(%e, "背景存檔失敗"),
                    Err(e) => warn!(%e, "存檔任務 panic"),
                }
            }
        });
    }
    axum::serve(listener, router).await.unwrap();
}

// ---------- 練習 2：不開埠的整合測試 ----------
#[cfg(test)]
mod tests {
    use super::*;
    use axum::body::Body;
    use axum::http::Request;
    use http_body_util::BodyExt;
    use tower::ServiceExt; // oneshot

    async fn post_json(app: Router, uri: &str, body: &str) -> (StatusCode, serde_json::Value) {
        let req = Request::post(uri)
            .header("content-type", "application/json")
            .body(Body::from(body.to_string()))
            .unwrap();
        let res = app.oneshot(req).await.unwrap();
        let status = res.status();
        let bytes = res.into_body().collect().await.unwrap().to_bytes();
        (
            status,
            serde_json::from_slice(&bytes).unwrap_or(serde_json::Value::Null),
        )
    }

    #[tokio::test]
    async fn deposit_returns_201_and_new_balance() {
        let (status, json) = post_json(
            app(seed()),
            "/transactions",
            r#"{"type":"deposit","account":1001,"amount":"250.50"}"#,
        )
        .await;
        assert_eq!(status, StatusCode::CREATED);
        assert_eq!(json["balances"][0]["balance"], "50250.50");
    }

    #[tokio::test]
    async fn insufficient_funds_returns_409() {
        let (status, json) = post_json(
            app(seed()),
            "/transactions",
            r#"{"type":"withdraw","account":1003,"amount":"99999"}"#,
        )
        .await;
        assert_eq!(status, StatusCode::CONFLICT);
        assert!(json["error"].as_str().unwrap().contains("餘額不足"));
    }

    #[tokio::test]
    async fn statement_endpoint_filters_by_date() {
        let app = app(seed());
        let _ = post_json(
            app.clone(),
            "/transactions",
            r#"{"type":"deposit","account":1001,"amount":"1","date":"2026-10-05"}"#,
        )
        .await;
        let _ = post_json(
            app.clone(),
            "/transactions",
            r#"{"type":"deposit","account":1001,"amount":"2","date":"2026-11-05"}"#,
        )
        .await;
        let res = app
            .oneshot(
                Request::get("/accounts/1001/statement?from=2026-10-01&to=2026-10-31")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(res.status(), StatusCode::OK);
        let json: serde_json::Value =
            serde_json::from_slice(&res.into_body().collect().await.unwrap().to_bytes()).unwrap();
        assert_eq!(json["entries"].as_array().unwrap().len(), 1);
        assert_eq!(json["net_change"], "1.00");
    }

    #[tokio::test]
    async fn statement_with_inverted_range_is_400() {
        let res = app(seed())
            .oneshot(
                Request::get("/accounts/1001/statement?from=2026-10-31&to=2026-10-01")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(res.status(), StatusCode::BAD_REQUEST);
    }
}
