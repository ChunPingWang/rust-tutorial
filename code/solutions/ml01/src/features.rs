//! 特徵工程（ML-1 練習解答版：加了練習 1、2 的新特徵，以及練習 3、4 的函式）。

use std::collections::{HashMap, HashSet};

use ndarray::{Array1, Array2, Axis};

use crate::synth::{Kind, LabeledTx};

pub const FEATURE_NAMES: [&str; 11] = [
    "log_amount",
    "amount_vs_acct",
    "amount_to_balance",
    "hour_sin",
    "hour_cos",
    "is_night",
    "is_withdraw",
    "is_transfer",
    "velocity_24h",
    "is_new_counterparty", // 練習 1：轉帳對象是否第一次出現（非轉帳 = 0）
    "velocity_1h",         // 練習 2：過去 1 小時的筆數，與 24h 版比較鑑別力
];

#[allow(dead_code)] // ids 在 ML-3 的練習 2 才用到
pub struct Features {
    pub x: Array2<f64>,
    pub y: Array1<f64>,
    pub ids: Vec<u64>,
}

#[derive(Debug, Clone)]
pub struct Scaler {
    pub mean: Array1<f64>,
    pub std: Array1<f64>,
}

impl Scaler {
    pub fn fit(x: &Array2<f64>) -> Scaler {
        let mean = x.mean_axis(Axis(0)).expect("至少要有一列");
        let std = x
            .std_axis(Axis(0), 0.0)
            .mapv(|s| if s < 1e-12 { 1.0 } else { s });
        Scaler { mean, std }
    }

    pub fn transform(&self, x: &Array2<f64>) -> Array2<f64> {
        (x - &self.mean) / &self.std
    }
}

/// 練習 4：min-max 標準化（壓到 0..1）。
/// 對有極端值的欄（amount_vs_acct 最大 30）會把絕大多數資料壓在很窄的區間，鑑別力變差；
/// z-score 不會被單一極端值主導得那麼嚴重。有離群值時偏好 z-score（或 robust scaler：中位數與 IQR）。
#[derive(Debug, Clone)]
pub struct MinMaxScaler {
    pub min: Array1<f64>,
    pub range: Array1<f64>,
}

impl MinMaxScaler {
    pub fn fit(x: &Array2<f64>) -> MinMaxScaler {
        let min = x.fold_axis(Axis(0), f64::INFINITY, |a, b| a.min(*b));
        let max = x.fold_axis(Axis(0), f64::NEG_INFINITY, |a, b| a.max(*b));
        let range = (&max - &min).mapv(|r| if r < 1e-12 { 1.0 } else { r });
        MinMaxScaler { min, range }
    }

    pub fn transform(&self, x: &Array2<f64>) -> Array2<f64> {
        (x - &self.min) / &self.range
    }
}

pub fn build(txs: &[LabeledTx]) -> Features {
    let n = txs.len();
    let mut x = Array2::<f64>::zeros((n, FEATURE_NAMES.len()));
    let mut y = Array1::<f64>::zeros(n);
    let mut history: HashMap<u32, (Vec<f64>, Vec<u32>)> = HashMap::new();
    // 練習 1：每個帳戶轉過帳的對象
    let mut counterparties: HashMap<u32, HashSet<u32>> = HashMap::new();

    for (i, tx) in txs.iter().enumerate() {
        let log_amount = (tx.amount_cents as f64 / 100.0).ln();
        let (amounts, times) = history.entry(tx.account).or_default();

        let amount_vs_acct = if amounts.len() >= 3 {
            let mean = amounts.iter().sum::<f64>() / amounts.len() as f64;
            let var =
                amounts.iter().map(|a| (a - mean).powi(2)).sum::<f64>() / amounts.len() as f64;
            (log_amount - mean) / var.sqrt().max(0.1)
        } else {
            0.0
        };
        let velocity = times
            .iter()
            .filter(|&&t| tx.minute.saturating_sub(t) <= 24 * 60)
            .count() as f64;
        let velocity_1h = times
            .iter()
            .filter(|&&t| tx.minute.saturating_sub(t) <= 60)
            .count() as f64;

        let is_new_counterparty = match tx.kind {
            Kind::Transfer => {
                // 合成資料沒有「對象帳戶」欄位，用 (金額分 % 50) 當作對象的替身，示範特徵的計算方式
                let counterparty = (tx.amount_cents % 50) as u32;
                let seen = counterparties.entry(tx.account).or_default();
                if seen.insert(counterparty) {
                    1.0
                } else {
                    0.0
                }
            }
            _ => 0.0,
        };

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
        row[9] = is_new_counterparty;
        row[10] = velocity_1h;
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

/// 練習 3：每個帳戶的平均特徵向量（帳戶數 × 特徵數）。回傳 (矩陣, 帳戶 ID 順序)。這是 ML-3 分群的輸入。
pub fn account_profiles(txs: &[LabeledTx], x: &Array2<f64>) -> (Array2<f64>, Vec<u32>) {
    let mut groups: HashMap<u32, Vec<usize>> = HashMap::new();
    for (i, tx) in txs.iter().enumerate() {
        groups.entry(tx.account).or_default().push(i);
    }
    let mut ids: Vec<u32> = groups.keys().copied().collect();
    ids.sort();
    let mut m = Array2::<f64>::zeros((ids.len(), x.ncols()));
    for (r, id) in ids.iter().enumerate() {
        m.row_mut(r)
            .assign(&x.select(Axis(0), &groups[id]).mean_axis(Axis(0)).unwrap());
    }
    (m, ids)
}
