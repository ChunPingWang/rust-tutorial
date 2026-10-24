//! 特徵工程：把交易變成數字矩陣。
//!
//! 模型只看數字。這個模組決定「一筆交易用哪幾個數字描述」，是整條路徑裡
//! 最需要領域知識的地方：風控人員知道什麼可疑，我們把它翻成特徵。

use std::collections::HashMap;

use ndarray::{Array1, Array2, Axis};

use crate::synth::{Kind, LabeledTx};

/// 特徵名稱，順序就是矩陣的欄順序
pub const FEATURE_NAMES: [&str; 9] = [
    "log_amount",        // ln(金額)：金額跨好幾個數量級，取對數讓分佈接近常態
    "amount_vs_acct",    // 這筆金額相對帳戶歷史平均的 z 分數：對「這個人」來說大不大
    "amount_to_balance", // 金額 / 交易前餘額：是不是在掏空帳戶
    "hour_sin",          // 時段的圓形編碼：23 點和 0 點應該很近
    "hour_cos",
    "is_night", // 0~5 點
    "is_withdraw",
    "is_transfer",
    "velocity_24h", // 同帳戶過去 24 小時內的交易筆數：短時間連續交易
];

#[allow(dead_code)]
pub struct Features {
    /// n × 9
    pub x: Array2<f64>,
    /// n，0 或 1
    pub y: Array1<f64>,
    pub ids: Vec<u64>,
}

/// 用來標準化的參數：訓練時算出來，推論時要用**同一組**，所以要存起來
#[derive(Debug, Clone)]
#[allow(dead_code)] // 這章用 linfa 的 LinearScaler
pub struct Scaler {
    pub mean: Array1<f64>,
    pub std: Array1<f64>,
}

#[allow(dead_code)]
impl Scaler {
    /// 每一欄的平均與標準差
    pub fn fit(x: &Array2<f64>) -> Scaler {
        let mean = x.mean_axis(Axis(0)).expect("至少要有一列");
        let std = x
            .std_axis(Axis(0), 0.0)
            .mapv(|s| if s < 1e-12 { 1.0 } else { s }); // 常數欄避免除以 0
        Scaler { mean, std }
    }

    /// (x - mean) / std，廣播：mean 與 std 是 1×9，會自動對每一列做
    pub fn transform(&self, x: &Array2<f64>) -> Array2<f64> {
        (x - &self.mean) / &self.std
    }

    /// 單筆推論用（ML-5 的 API 會用到）
    #[allow(dead_code)]
    pub fn transform_row(&self, row: &Array1<f64>) -> Array1<f64> {
        (row - &self.mean) / &self.std
    }
}

/// 把交易序列變成特徵矩陣。需要「按時間順序」處理，因為有些特徵依賴歷史。
pub fn build(txs: &[LabeledTx]) -> Features {
    let n = txs.len();
    let mut x = Array2::<f64>::zeros((n, FEATURE_NAMES.len()));
    let mut y = Array1::<f64>::zeros(n);

    // 每個帳戶的歷史：金額的 ln 序列（算 z 分數用）與時間戳（算 velocity 用）
    let mut history: HashMap<u32, (Vec<f64>, Vec<u32>)> = HashMap::new();

    for (i, tx) in txs.iter().enumerate() {
        let log_amount = (tx.amount_cents as f64 / 100.0).ln();
        let (amounts, times) = history.entry(tx.account).or_default();

        let amount_vs_acct = if amounts.len() >= 3 {
            let mean = amounts.iter().sum::<f64>() / amounts.len() as f64;
            let var =
                amounts.iter().map(|a| (a - mean).powi(2)).sum::<f64>() / amounts.len() as f64;
            (log_amount - mean) / var.sqrt().max(0.1)
        } else {
            0.0 // 歷史太短，無法判斷
        };
        let velocity = times
            .iter()
            .filter(|&&t| tx.minute.saturating_sub(t) <= 24 * 60)
            .count() as f64;

        let hour_rad = tx.hour as f64 / 24.0 * std::f64::consts::TAU;
        let mut row = x.row_mut(i);
        row[0] = log_amount;
        row[1] = amount_vs_acct;
        row[2] = tx.amount_cents as f64 / (tx.balance_before_cents.max(1) as f64);
        row[3] = hour_rad.sin();
        row[4] = hour_rad.cos();
        row[5] = if tx.hour < 5 { 1.0 } else { 0.0 };
        row[6] = if tx.kind == Kind::Withdraw { 1.0 } else { 0.0 };
        row[7] = if tx.kind == Kind::Transfer { 1.0 } else { 0.0 };
        row[8] = velocity;
        y[i] = if tx.is_fraud { 1.0 } else { 0.0 };

        amounts.push(log_amount);
        times.push(tx.minute);
    }

    Features {
        x,
        y,
        ids: txs.iter().map(|t| t.id).collect(),
    }
}

/// 時間順序切分：前 ratio 當訓練、後面當測試。
/// 不能隨機切！詐欺偵測的模型要用「過去」預測「未來」，隨機切會讓未來的資訊漏進訓練。
pub fn split(f: &Features, train_ratio: f64) -> (Features, Features) {
    let n = f.x.nrows();
    let k = (n as f64 * train_ratio) as usize;
    let part = |r: std::ops::Range<usize>| Features {
        x: f.x.slice(ndarray::s![r.clone(), ..]).to_owned(),
        y: f.y.slice(ndarray::s![r.clone()]).to_owned(),
        ids: f.ids[r].to_vec(),
    };
    (part(0..k), part(k..n))
}
