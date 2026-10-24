//! ML-5：把詐欺模型嵌進帳本 API。
//!
//! ```text
//! cargo run --release                          # 啟動 API（port 3100）
//! cargo run --release -- bench                 # 不開 API，直接量推論吞吐
//! curl -X POST localhost:3100/score -H 'content-type: application/json' -d '{
//!   "account": 1001, "kind": "withdraw", "amount": "48000.00", "hour": 2,
//!   "balance_before": "48000.00", "account_mean_log_amount": 7.2, "account_sd_log_amount": 0.5, "txs_last_24h": 4 }'
//! ```

mod model;
mod scorer_trait;
mod synth;

use std::collections::HashMap;
use std::path::Path;
use std::sync::Arc;
use std::time::Instant;

use axum::extract::State;
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum::routing::{get, post};
use axum::{Json, Router};
use serde::{Deserialize, Serialize};
use tracing::info;

use model::{RawFeatures, Scorer};
use scorer_trait::{FraudScorer, LinearScorer};

/// 請求：API 層只要求「交易本身的資訊」加上「帳戶的歷史統計」。
/// 歷史統計在真實系統裡來自帳本（第 15 章的 Ledger），這裡由呼叫端提供以保持範例獨立。
#[derive(Deserialize)]
struct ScoreRequest {
    account: u32,
    kind: String,
    amount: String,
    hour: u8,
    balance_before: String,
    account_mean_log_amount: f64,
    account_sd_log_amount: f64,
    txs_last_24h: u32,
}

#[derive(Serialize)]
struct ScoreResponse {
    account: u32,
    fraud_probability: f32,
    decision: &'static str,
    /// 讓呼叫端看到模型實際吃了什麼：除錯與稽核都需要
    features: HashMap<String, f64>,
    latency_us: u128,
}

#[derive(Clone)]
struct AppState {
    scorer: Arc<dyn FraudScorer>, // 練習 3：trait object，可以是 MLP 或線性模型
    shadow: Option<Arc<dyn FraudScorer>>, // 練習 2：影子模型，只打分不決策
    thresholds: Thresholds,
    metrics: metrics_exporter_prometheus::PrometheusHandle,
}

/// 兩段式門檻：高於 block 直接拒絕，介於 review 與 block 之間掛起人工審核。這是業務決定。
#[derive(Clone, Copy)]
struct Thresholds {
    review: f64,
    block: f64,
}

#[derive(Debug)]
struct ApiError(StatusCode, String);

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        (self.0, Json(serde_json::json!({ "error": self.1 }))).into_response()
    }
}

fn parse_yuan(s: &str) -> Result<f64, ApiError> {
    s.trim()
        .parse::<f64>()
        .map_err(|_| ApiError(StatusCode::BAD_REQUEST, format!("金額格式錯誤：{s:?}")))
}

/// 把 API 的請求轉成 ML-1 定義的 9 個特徵。這段邏輯必須和訓練時 features.rs 的 build() 一致，
/// 否則就是「訓練/推論偏差（training-serving skew）」，模型上線後表現會莫名變差。
fn to_features(req: &ScoreRequest) -> Result<RawFeatures, ApiError> {
    let amount = parse_yuan(&req.amount)?;
    let balance = parse_yuan(&req.balance_before)?;
    if amount <= 0.0 {
        return Err(ApiError(StatusCode::BAD_REQUEST, "金額必須為正".into()));
    }
    if req.hour >= 24 {
        return Err(ApiError(
            StatusCode::BAD_REQUEST,
            "hour 必須在 0..24".into(),
        ));
    }
    let log_amount = amount.ln();
    let hour_rad = req.hour as f64 / 24.0 * std::f64::consts::TAU;
    let mut v = HashMap::new();
    v.insert("log_amount".into(), log_amount);
    v.insert(
        "amount_vs_acct".into(),
        (log_amount - req.account_mean_log_amount) / req.account_sd_log_amount.max(0.1),
    );
    v.insert("amount_to_balance".into(), amount / balance.max(0.01));
    v.insert("hour_sin".into(), hour_rad.sin());
    v.insert("hour_cos".into(), hour_rad.cos());
    v.insert("is_night".into(), if req.hour < 5 { 1.0 } else { 0.0 });
    v.insert(
        "is_withdraw".into(),
        if req.kind == "withdraw" { 1.0 } else { 0.0 },
    );
    v.insert(
        "is_transfer".into(),
        if req.kind == "transfer" { 1.0 } else { 0.0 },
    );
    v.insert("velocity_24h".into(), req.txs_last_24h as f64);
    Ok(RawFeatures { values: v })
}

async fn score(
    State(state): State<AppState>,
    Json(req): Json<ScoreRequest>,
) -> Result<Json<ScoreResponse>, ApiError> {
    let t = Instant::now();
    let raw = to_features(&req)?;
    // 推論是純 CPU、3 微秒，直接在 async handler 裡做沒問題。
    // 如果換成大模型（毫秒級以上），要改用 spawn_blocking，否則會卡住執行緒（第 15 章）。
    let p = state
        .scorer
        .score(&raw)
        .map_err(|e| ApiError(StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    let decision = if f64::from(p) >= state.thresholds.block {
        "block"
    } else if f64::from(p) >= state.thresholds.review {
        "review"
    } else {
        "allow"
    };
    info!(account = req.account, p, decision, "scored");
    // 練習 4：指標。機率分佈直方圖是資料漂移的早期警訊：
    // 若某天 p 的分佈整體往上移，可能是特徵計算壞了或世界變了，比等到客訴快得多。
    metrics::counter!("fraud_score_requests_total", "decision" => decision).increment(1);
    metrics::histogram!("fraud_score_probability").record(f64::from(p));
    metrics::histogram!("fraud_score_latency_us").record(t.elapsed().as_micros() as f64);
    Ok(Json(ScoreResponse {
        account: req.account,
        fraud_probability: p,
        decision,
        features: raw.values,
        latency_us: t.elapsed().as_micros(),
    }))
}

async fn model_info(State(state): State<AppState>) -> Json<serde_json::Value> {
    Json(serde_json::json!({
        "model": state.scorer.name(),
        "shadow": state.shadow.as_ref().map(|s| s.name()),
        "thresholds": { "review": state.thresholds.review, "block": state.thresholds.block },
    }))
}

async fn metrics_endpoint(State(state): State<AppState>) -> String {
    state.metrics.render()
}

fn app(
    scorer: Arc<dyn FraudScorer>,
    shadow: Option<Arc<dyn FraudScorer>>,
    thresholds: Thresholds,
) -> Router {
    // 測試會多次呼叫 app()，recorder 只能裝一次，所以用 OnceLock 保存
    static HANDLE: std::sync::OnceLock<metrics_exporter_prometheus::PrometheusHandle> =
        std::sync::OnceLock::new();
    let metrics = HANDLE
        .get_or_init(|| {
            metrics_exporter_prometheus::PrometheusBuilder::new()
                .install_recorder()
                .expect("metrics recorder")
        })
        .clone();
    let state = AppState {
        scorer,
        shadow,
        thresholds,
        metrics,
    };
    Router::new()
        .route("/score", post(score))
        .route("/model", get(model_info))
        .route("/metrics", get(metrics_endpoint))
        .with_state(state)
}

/// 離線吞吐量測：用 ML-1 的合成資料產生器餵一萬筆
fn bench(scorer: &Scorer) -> anyhow::Result<()> {
    let txs = synth::generate(10_000, 99);
    let rows: Vec<RawFeatures> = txs
        .iter()
        .map(|tx| {
            let amount = tx.amount_cents as f64 / 100.0;
            let kind = match tx.kind {
                synth::Kind::Withdraw => "withdraw",
                synth::Kind::Transfer => "transfer",
                synth::Kind::Deposit => "deposit",
            };
            let req = ScoreRequest {
                account: tx.account,
                kind: kind.into(),
                amount: format!("{amount:.2}"),
                hour: tx.hour,
                balance_before: format!("{:.2}", tx.balance_before_cents as f64 / 100.0),
                account_mean_log_amount: 7.4, // bench 用近似值；真實系統從帳本算
                account_sd_log_amount: 0.6,
                txs_last_24h: 1,
            };
            to_features(&req).map_err(|e| anyhow::anyhow!(e.1))
        })
        .collect::<anyhow::Result<_>>()?;

    let t = Instant::now();
    for r in &rows {
        scorer.score(r)?;
    }
    let single = t.elapsed();
    let t = Instant::now();
    let probs = scorer.score_batch(&rows)?;
    let batch = t.elapsed();
    let flagged = probs.iter().filter(|&&p| p >= 0.5).count();
    let actual = txs.iter().filter(|t| t.is_fraud).count();
    println!("{} 筆", rows.len());
    println!(
        "逐筆：{:>7} ms（{:.1} µs/筆）",
        single.as_millis(),
        single.as_micros() as f64 / rows.len() as f64
    );
    println!(
        "批次：{:>7} ms（{:.2} µs/筆）",
        batch.as_millis(),
        batch.as_micros() as f64 / rows.len() as f64
    );
    println!("p ≥ 0.5：{flagged} 筆；資料中實際詐欺 {actual} 筆（bench 的帳戶統計是近似值，數字僅供參考）");
    Ok(())
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
    let t = Instant::now();
    let scorer = Scorer::load(Path::new(&model_dir))?;
    info!(model_dir, load_ms = t.elapsed().as_millis(), "模型載入完成");

    if std::env::args().nth(1).as_deref() == Some("bench") {
        return bench(&scorer);
    }

    let thresholds = Thresholds {
        review: 0.3,
        block: 0.9,
    };
    // 練習 3：FRAUD_MODEL=linear 時改用線性模型；練習 2：FRAUD_SHADOW_MODEL=mlp|linear 啟用影子
    let primary: Arc<dyn FraudScorer> = match std::env::var("FRAUD_MODEL").as_deref() {
        Ok("linear") => Arc::new(LinearScorer::load(
            &Path::new(&model_dir).join("linear.json"),
        )?),
        _ => Arc::new(scorer),
    };
    let shadow: Option<Arc<dyn FraudScorer>> = match std::env::var("FRAUD_SHADOW_MODEL").as_deref()
    {
        Ok("linear") => Some(Arc::new(LinearScorer::load(
            &Path::new(&model_dir).join("linear.json"),
        )?)),
        Ok("mlp") => Some(Arc::new(Scorer::load(Path::new(&model_dir))?)),
        _ => None,
    };
    info!(
        primary = primary.name(),
        shadow = shadow.as_ref().map(|s| s.name()),
        "模型設定"
    );
    let addr = std::env::var("SCORER_ADDR").unwrap_or_else(|_| "127.0.0.1:3100".to_string());
    let listener = tokio::net::TcpListener::bind(&addr).await?;
    info!(%addr, "詐欺打分 API 啟動");
    axum::serve(listener, app(primary, shadow, thresholds)).await?;
    Ok(())
}

// ---------- 練習 5：三種決策各一個測試 ----------
#[cfg(test)]
mod tests {
    use super::*;
    use axum::body::Body;
    use axum::http::Request;
    use http_body_util::BodyExt;
    use tower::ServiceExt;

    fn test_app() -> Router {
        let dir = Path::new(concat!(env!("CARGO_MANIFEST_DIR"), "/model"));
        let mlp: Arc<dyn FraudScorer> = Arc::new(Scorer::load(dir).unwrap());
        let linear: Arc<dyn FraudScorer> =
            Arc::new(LinearScorer::load(&dir.join("linear.json")).unwrap());
        app(
            mlp,
            Some(linear),
            Thresholds {
                review: 0.3,
                block: 0.9,
            },
        )
    }

    async fn decision_for(body: &str) -> (StatusCode, String, f32) {
        let req = Request::post("/score")
            .header("content-type", "application/json")
            .body(Body::from(body.to_string()))
            .unwrap();
        let res = test_app().oneshot(req).await.unwrap();
        let status = res.status();
        let json: serde_json::Value =
            serde_json::from_slice(&res.into_body().collect().await.unwrap().to_bytes()).unwrap();
        (
            status,
            json["decision"].as_str().unwrap_or("").to_string(),
            json["fraud_probability"].as_f64().unwrap_or(-1.0) as f32,
        )
    }

    #[tokio::test]
    async fn night_drain_is_blocked() {
        let (s, d, p) = decision_for(r#"{"account":1,"kind":"withdraw","amount":"48000","hour":2,"balance_before":"48000","account_mean_log_amount":7.2,"account_sd_log_amount":0.5,"txs_last_24h":4}"#).await;
        assert_eq!((s, d.as_str()), (StatusCode::OK, "block"), "p={p}");
    }

    #[tokio::test]
    async fn daytime_small_deposit_is_allowed() {
        let (s, d, p) = decision_for(r#"{"account":2,"kind":"deposit","amount":"1200","hour":14,"balance_before":"35000","account_mean_log_amount":7.0,"account_sd_log_amount":0.6,"txs_last_24h":1}"#).await;
        assert_eq!((s, d.as_str()), (StatusCode::OK, "allow"), "p={p}");
    }

    #[tokio::test]
    async fn evening_large_transfer_goes_to_review() {
        let (s, d, p) = decision_for(r#"{"account":3,"kind":"transfer","amount":"4500","hour":22,"balance_before":"30000","account_mean_log_amount":7.5,"account_sd_log_amount":0.5,"txs_last_24h":2}"#).await;
        assert_eq!((s, d.as_str()), (StatusCode::OK, "review"), "p={p}");
    }

    #[tokio::test]
    async fn linear_and_mlp_agree_on_extremes() {
        let dir = Path::new(concat!(env!("CARGO_MANIFEST_DIR"), "/model"));
        let mlp = Scorer::load(dir).unwrap();
        let linear = LinearScorer::load(&dir.join("linear.json")).unwrap();
        let req: ScoreRequest = serde_json::from_str(r#"{"account":1,"kind":"withdraw","amount":"48000","hour":2,"balance_before":"48000","account_mean_log_amount":7.2,"account_sd_log_amount":0.5,"txs_last_24h":4}"#).unwrap();
        let raw = to_features(&req).unwrap();
        assert!(FraudScorer::score(&mlp, &raw).unwrap() > 0.9);
        assert!(linear.score(&raw).unwrap() > 0.9);
    }
}
