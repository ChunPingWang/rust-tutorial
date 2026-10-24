//! 練習 3：讓不同模型共用同一個介面（第 9 章的 trait）。

use std::path::Path;

use anyhow::{Context, Result};
use serde::Deserialize;

use crate::model::{RawFeatures, Scorer};

pub trait FraudScorer: Send + Sync {
    fn score(&self, raw: &RawFeatures) -> Result<f32>;
    fn name(&self) -> &'static str;
}

impl FraudScorer for Scorer {
    fn score(&self, raw: &RawFeatures) -> Result<f32> {
        Scorer::score(self, raw)
    }
    fn name(&self) -> &'static str {
        "mlp (candle)"
    }
}

/// 邏輯回歸：σ(x·w + b)。10 行，沒有 candle。這是這份資料上最務實的生產方案。
#[derive(Deserialize)]
pub struct LinearScorer {
    feature_names: Vec<String>,
    mean: Vec<f64>,
    std: Vec<f64>,
    weights: Vec<f64>,
    bias: f64,
}

impl LinearScorer {
    pub fn load(path: &Path) -> Result<Self> {
        Ok(serde_json::from_str(
            &std::fs::read_to_string(path).context("讀 linear.json")?,
        )?)
    }
}

impl FraudScorer for LinearScorer {
    fn score(&self, raw: &RawFeatures) -> Result<f32> {
        let mut z = self.bias;
        for (i, name) in self.feature_names.iter().enumerate() {
            let v = *raw
                .values
                .get(name)
                .with_context(|| format!("缺少特徵 {name}"))?;
            z += (v - self.mean[i]) / self.std[i] * self.weights[i];
        }
        Ok((1.0 / (1.0 + (-z).exp())) as f32)
    }
    fn name(&self) -> &'static str {
        "logistic (json)"
    }
}
