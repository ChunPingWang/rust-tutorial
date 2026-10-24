//! ML-5 練習 1：帳本 API + 詐欺打分。POST /transactions 過帳前先打分：
//! block → 403 並記錄；review → 202 掛起等人工審核；allow → 過帳。
//!
//! cargo run --release --bin ledger-with-scoring
//! curl -X POST localhost:3200/transactions -H 'content-type: application/json' \
//!      -d '{"type":"withdraw","account":1003,"amount":"790","hour":2}'

use std::collections::HashMap;
use std::path::Path;
use std::sync::Arc;

use axum::extract::{Path as AxPath, State};
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum::routing::{get, post};
use axum::{Json, Router};
use chrono::{Local, NaiveDate};
use ledger::{Ledger, LedgerError, Money, Transaction, TxKind};
use serde::{Deserialize, Serialize};
use tokio::sync::RwLock;
use tracing::{info, warn};

#[allow(dead_code)]
#[path = "../model.rs"]
mod model;
use model::{RawFeatures, Scorer};

#[derive(Clone)]
struct AppState {
    ledger: Arc<RwLock<Ledger>>,
    pending: Arc<RwLock<Vec<PendingTx>>>,
    scorer: Arc<Scorer>,
    next_id: Arc<std::sync::atomic::AtomicU64>,
}

#[derive(Deserialize)]
struct NewTransaction {
    #[serde(flatten)]
    kind: TxKind,
    amount: String,
    #[serde(default)]
    memo: String,
    date: Option<NaiveDate>,
    /// 帳本的 Transaction 只有日期沒有時間，打分需要時段，所以請求額外帶 hour（預設 12）
    #[serde(default = "noon")]
    hour: u8,
}

fn noon() -> u8 {
    12
}

#[derive(Serialize, Clone)]
struct PendingTx {
    id: u64,
    account: u32,
    amount: String,
    fraud_probability: f32,
    reason: String,
}

#[derive(Serialize)]
struct Posted {
    id: u64,
    fraud_probability: f32,
    balance: String,
}

enum ApiError {
    NotFound(String),
    BadRequest(String),
    Conflict(String),
    Blocked { probability: f32, reason: String },
    Internal(String),
}

impl From<LedgerError> for ApiError {
    fn from(e: LedgerError) -> Self {
        match e {
            LedgerError::AccountNotFound(_) => ApiError::NotFound(e.to_string()),
            LedgerError::InsufficientFunds { .. } | LedgerError::Overflow => {
                ApiError::Conflict(e.to_string())
            }
            _ => ApiError::BadRequest(e.to_string()),
        }
    }
}

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        let (status, body) = match self {
            ApiError::NotFound(m) => (StatusCode::NOT_FOUND, serde_json::json!({ "error": m })),
            ApiError::BadRequest(m) => (StatusCode::BAD_REQUEST, serde_json::json!({ "error": m })),
            ApiError::Conflict(m) => (StatusCode::CONFLICT, serde_json::json!({ "error": m })),
            ApiError::Blocked {
                probability,
                reason,
            } => (
                StatusCode::FORBIDDEN,
                serde_json::json!({ "error": "交易被風控攔截", "fraud_probability": probability, "reason": reason }),
            ),
            ApiError::Internal(m) => (
                StatusCode::INTERNAL_SERVER_ERROR,
                serde_json::json!({ "error": m }),
            ),
        };
        (status, Json(body)).into_response()
    }
}

/// 從帳本歷史算出該帳戶的統計：這是「特徵計算要和訓練一致」的部分。
/// 帳本的交易沒有時間戳，velocity 用「同一天的筆數」近似。
fn features_from_ledger(
    ledger: &Ledger,
    account: u32,
    kind: &TxKind,
    amount: Money,
    date: NaiveDate,
    hour: u8,
) -> Result<RawFeatures, ApiError> {
    let balance = ledger.balance_of(account)?;
    let history: Vec<&Transaction> = ledger
        .history()
        .iter()
        .filter(|t| t.involves(account))
        .collect();
    let logs: Vec<f64> = history
        .iter()
        .map(|t| (t.amount.cents() as f64 / 100.0).ln())
        .collect();
    let (mean, sd) = if logs.len() >= 3 {
        let m = logs.iter().sum::<f64>() / logs.len() as f64;
        let v = logs.iter().map(|l| (l - m).powi(2)).sum::<f64>() / logs.len() as f64;
        (m, v.sqrt().max(0.1))
    } else {
        ((amount.cents() as f64 / 100.0).ln(), 1.0) // 歷史太短：視為「正常」
    };
    let same_day = history.iter().filter(|t| t.date == date).count() as f64;
    let log_amount = (amount.cents() as f64 / 100.0).ln();
    let hour_rad = hour as f64 / 24.0 * std::f64::consts::TAU;
    let mut v = HashMap::new();
    v.insert("log_amount".into(), log_amount);
    v.insert("amount_vs_acct".into(), (log_amount - mean) / sd);
    v.insert(
        "amount_to_balance".into(),
        amount.cents() as f64 / balance.cents().max(1) as f64,
    );
    v.insert("hour_sin".into(), hour_rad.sin());
    v.insert("hour_cos".into(), hour_rad.cos());
    v.insert("is_night".into(), if hour < 5 { 1.0 } else { 0.0 });
    v.insert(
        "is_withdraw".into(),
        if matches!(kind, TxKind::Withdraw { .. }) {
            1.0
        } else {
            0.0
        },
    );
    v.insert(
        "is_transfer".into(),
        if matches!(kind, TxKind::Transfer { .. }) {
            1.0
        } else {
            0.0
        },
    );
    v.insert("velocity_24h".into(), same_day);
    Ok(RawFeatures { values: v })
}

async fn post_transaction(
    State(state): State<AppState>,
    Json(body): Json<NewTransaction>,
) -> Result<(StatusCode, Json<serde_json::Value>), ApiError> {
    let amount: Money = body
        .amount
        .parse()
        .map_err(|e: ledger::money::ParseMoneyError| ApiError::BadRequest(e.to_string()))?;
    if !amount.is_positive() {
        return Err(ApiError::BadRequest("金額必須為正".into()));
    }
    if body.hour >= 24 {
        return Err(ApiError::BadRequest("hour 必須在 0..24".into()));
    }
    let date = body.date.unwrap_or_else(|| Local::now().date_naive());
    // 存款不打分：風險在資金流出
    let debited = match body.kind {
        TxKind::Withdraw { account } => Some(account),
        TxKind::Transfer { from, .. } => Some(from),
        TxKind::Deposit { .. } => None,
    };
    let id = state
        .next_id
        .fetch_add(1, std::sync::atomic::Ordering::Relaxed);

    let mut ledger = state.ledger.write().await;
    let mut p = 0.0f32;
    if let Some(account) = debited {
        let raw = features_from_ledger(&ledger, account, &body.kind, amount, date, body.hour)?;
        p = state
            .scorer
            .score(&raw)
            .map_err(|e| ApiError::Internal(e.to_string()))?;
        let top = top_features(&raw);
        if p >= 0.9 {
            warn!(id, account, p, "block");
            return Err(ApiError::Blocked {
                probability: p,
                reason: top,
            });
        }
        if p >= 0.3 {
            drop(ledger); // 掛起不過帳，先放掉寫鎖
            state.pending.write().await.push(PendingTx {
                id,
                account,
                amount: amount.to_string(),
                fraud_probability: p,
                reason: top,
            });
            info!(id, account, p, "review");
            return Ok((
                StatusCode::ACCEPTED,
                Json(
                    serde_json::json!({ "id": id, "status": "pending_review", "fraud_probability": p }),
                ),
            ));
        }
    }
    let tx = Transaction {
        id,
        date,
        kind: body.kind,
        amount,
        memo: body.memo,
    };
    let affected = tx.affected_accounts();
    ledger.apply(tx)?;
    let balance = ledger.balance_of(affected[0])?.to_string();
    info!(id, p, "posted");
    Ok((
        StatusCode::CREATED,
        Json(
            serde_json::to_value(Posted {
                id,
                fraud_probability: p,
                balance,
            })
            .unwrap(),
        ),
    ))
}

/// 給審核人員看的「為什麼」：標準化後絕對值最大的三個特徵
fn top_features(raw: &RawFeatures) -> String {
    let mut v: Vec<(&String, &f64)> = raw.values.iter().collect();
    v.sort_by(|a, b| b.1.abs().partial_cmp(&a.1.abs()).unwrap());
    v.iter()
        .take(3)
        .map(|(k, v)| format!("{k}={v:.2}"))
        .collect::<Vec<_>>()
        .join(", ")
}

async fn list_pending(State(state): State<AppState>) -> Json<Vec<PendingTx>> {
    Json(state.pending.read().await.clone())
}

async fn get_account(
    State(state): State<AppState>,
    AxPath(id): AxPath<u32>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let ledger = state.ledger.read().await;
    let a = ledger.account(id)?;
    Ok(Json(
        serde_json::json!({ "id": a.id, "owner": a.owner, "balance": a.balance().to_string() }),
    ))
}

fn seed() -> Ledger {
    let mut l = Ledger::new();
    l.open(1001, "王小明", Money::from_yuan(50_000)).unwrap();
    l.open(1002, "李小華", Money::from_yuan(2_000)).unwrap();
    l.open(1003, "張大同", Money::from_yuan(800)).unwrap();
    l
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env().unwrap_or_else(|_| "info".into()),
        )
        .init();
    let model_dir = std::env::var("FRAUD_MODEL_DIR")
        .unwrap_or_else(|_| concat!(env!("CARGO_MANIFEST_DIR"), "/model").to_string());
    let scorer = Scorer::load(Path::new(&model_dir))?;
    let state = AppState {
        ledger: Arc::new(RwLock::new(seed())),
        pending: Arc::new(RwLock::new(Vec::new())),
        scorer: Arc::new(scorer),
        next_id: Arc::new(std::sync::atomic::AtomicU64::new(1)),
    };
    let app = Router::new()
        .route("/transactions", post(post_transaction))
        .route("/pending", get(list_pending))
        .route("/accounts/{id}", get(get_account))
        .with_state(state);
    let addr = std::env::var("LEDGER_ADDR").unwrap_or_else(|_| "127.0.0.1:3200".to_string());
    let listener = tokio::net::TcpListener::bind(&addr).await?;
    info!(%addr, "帳本 API（含風控打分）啟動");
    axum::serve(listener, app).await?;
    Ok(())
}
