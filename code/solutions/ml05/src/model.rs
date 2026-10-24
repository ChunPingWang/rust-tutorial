//! 模型載入與推論。這個模組不知道 HTTP 的存在，可以單獨測試。

use std::collections::HashMap;
use std::path::Path;

use anyhow::{bail, Context, Result};
use candle_core::{DType, Device, Module, Tensor, D};
use candle_nn::{linear, Linear, VarBuilder, VarMap};
use serde::Deserialize;

/// 和 ML-4 **完全相同**的結構與命名。差一個字 load 就會失敗。
struct Mlp {
    l1: Linear,
    l2: Linear,
    l3: Linear,
}

impl Mlp {
    fn new(vs: VarBuilder, n_features: usize) -> Result<Self> {
        Ok(Self {
            l1: linear(n_features, 16, vs.pp("l1"))?,
            l2: linear(16, 8, vs.pp("l2"))?,
            l3: linear(8, 1, vs.pp("l3"))?,
        })
    }

    fn forward(&self, x: &Tensor) -> Result<Tensor> {
        let x = self.l1.forward(x)?.relu()?;
        let x = self.l2.forward(&x)?.relu()?;
        Ok(candle_nn::ops::sigmoid(
            &self.l3.forward(&x)?.squeeze(D::Minus1)?,
        )?)
    }
}

#[derive(Deserialize)]
struct ScalerFile {
    feature_names: Vec<String>,
    mean: Vec<f64>,
    std: Vec<f64>,
}

/// 一筆交易要給模型的原始特徵（**未**標準化）。
/// 欄位對應 ML-1 的 FEATURE_NAMES；API 層負責從交易算出這些值。
#[derive(Debug, Clone)]
pub struct RawFeatures {
    pub values: HashMap<String, f64>,
}

/// 可以在多個請求間共享的打分器。載入一次，之後只讀。
pub struct Scorer {
    model: Mlp,
    feature_names: Vec<String>,
    mean: Vec<f32>,
    std: Vec<f32>,
    device: Device,
    // VarMap 必須活著，Linear 裡的 Tensor 才有效
    _varmap: VarMap,
}

impl Scorer {
    pub fn load(dir: &Path) -> Result<Self> {
        let scaler: ScalerFile = serde_json::from_str(
            &std::fs::read_to_string(dir.join("scaler.json")).context("讀 scaler.json")?,
        )?;
        let n = scaler.feature_names.len();
        if scaler.mean.len() != n || scaler.std.len() != n {
            bail!("scaler.json 的 mean/std 長度與特徵數不符");
        }
        let device = Device::Cpu;
        let mut varmap = VarMap::new();
        let model = Mlp::new(VarBuilder::from_varmap(&varmap, DType::F32, &device), n)?;
        varmap
            .load(dir.join("fraud_mlp.safetensors"))
            .context("載入 safetensors")?;
        Ok(Self {
            model,
            feature_names: scaler.feature_names,
            mean: scaler.mean.iter().map(|&v| v as f32).collect(),
            std: scaler.std.iter().map(|&v| v as f32).collect(),
            device,
            _varmap: varmap,
        })
    }

    #[allow(dead_code)] // 主程式用 FraudScorer::name，這個方法留給 bench/其他呼叫者
    pub fn feature_names(&self) -> &[String] {
        &self.feature_names
    }

    /// 把具名特徵排成模型要的順序、標準化、推論。缺欄位是錯誤，不是預設 0。
    pub fn score(&self, raw: &RawFeatures) -> Result<f32> {
        let mut row = Vec::with_capacity(self.feature_names.len());
        for (i, name) in self.feature_names.iter().enumerate() {
            let v = *raw
                .values
                .get(name)
                .with_context(|| format!("缺少特徵 {name}"))? as f32;
            row.push((v - self.mean[i]) / self.std[i]);
        }
        let x = Tensor::from_vec(row, (1, self.feature_names.len()), &self.device)?;
        Ok(self.model.forward(&x)?.to_vec1::<f32>()?[0])
    }

    /// 批次打分：一次 matmul 比一筆一筆快得多
    pub fn score_batch(&self, rows: &[RawFeatures]) -> Result<Vec<f32>> {
        let n_feat = self.feature_names.len();
        let mut data = Vec::with_capacity(rows.len() * n_feat);
        for raw in rows {
            for (i, name) in self.feature_names.iter().enumerate() {
                let v = *raw
                    .values
                    .get(name)
                    .with_context(|| format!("缺少特徵 {name}"))? as f32;
                data.push((v - self.mean[i]) / self.std[i]);
            }
        }
        let x = Tensor::from_vec(data, (rows.len(), n_feat), &self.device)?;
        Ok(self.model.forward(&x)?.to_vec1::<f32>()?)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn scorer() -> Scorer {
        Scorer::load(Path::new(concat!(env!("CARGO_MANIFEST_DIR"), "/model"))).unwrap()
    }

    fn features(pairs: &[(&str, f64)]) -> RawFeatures {
        RawFeatures {
            values: pairs.iter().map(|(k, v)| (k.to_string(), *v)).collect(),
        }
    }

    #[test]
    fn obvious_fraud_scores_high_and_normal_scores_low() {
        let s = scorer();
        let fraud = features(&[
            ("log_amount", 12.0),
            ("amount_vs_acct", 6.0),
            ("amount_to_balance", 1.0),
            ("hour_sin", 0.5),
            ("hour_cos", 0.87),
            ("is_night", 1.0),
            ("is_withdraw", 1.0),
            ("is_transfer", 0.0),
            ("velocity_24h", 5.0),
        ]);
        let normal = features(&[
            ("log_amount", 7.0),
            ("amount_vs_acct", 0.0),
            ("amount_to_balance", 0.01),
            ("hour_sin", 0.0),
            ("hour_cos", -1.0),
            ("is_night", 0.0),
            ("is_withdraw", 0.0),
            ("is_transfer", 0.0),
            ("velocity_24h", 1.0),
        ]);
        assert!(s.score(&fraud).unwrap() > 0.9);
        assert!(s.score(&normal).unwrap() < 0.1);
    }

    #[test]
    fn missing_feature_is_an_error() {
        let s = scorer();
        assert!(s.score(&features(&[("log_amount", 7.0)])).is_err());
    }

    #[test]
    fn batch_matches_single() {
        let s = scorer();
        let f = features(&[
            ("log_amount", 9.0),
            ("amount_vs_acct", 2.0),
            ("amount_to_balance", 0.3),
            ("hour_sin", 0.5),
            ("hour_cos", 0.87),
            ("is_night", 1.0),
            ("is_withdraw", 1.0),
            ("is_transfer", 0.0),
            ("velocity_24h", 2.0),
        ]);
        let single = s.score(&f).unwrap();
        let batch = s.score_batch(&[f.clone(), f]).unwrap();
        assert!((batch[0] - single).abs() < 1e-6 && (batch[1] - single).abs() < 1e-6);
    }
}
