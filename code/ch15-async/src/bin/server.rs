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

use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum::routing::{get, post};
use axum::{Json, Router};
use chrono::{Local, NaiveDate};
use serde::{Deserialize, Serialize};
use tokio::sync::RwLock;
use tracing::{info, warn};

use ledger::{report, Ledger, LedgerError, Money, Transaction, TxKind};

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

pub fn app(ledger: Ledger) -> Router {
    let state = AppState {
        ledger: Arc::new(RwLock::new(ledger)),
        next_id: Arc::new(std::sync::atomic::AtomicU64::new(1)),
    };
    Router::new()
        .route("/accounts", get(list_accounts))
        .route("/accounts/{id}", get(get_account))
        .route("/transactions", post(post_transaction))
        .route("/report", get(get_report))
        .route("/health/slow", get(slow_health))
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
    axum::serve(listener, app(seed())).await.unwrap();
}
