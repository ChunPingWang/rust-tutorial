//! ML-4：用 candle 訓練一個小型神經網路（MLP）做詐欺分類，並把權重存成 safetensors。

mod features;
mod metrics;
mod synth;

use std::path::Path;
use std::time::Instant;

use anyhow::Result;
use candle_core::{DType, Device, Module, Tensor, D};
use candle_nn::{linear, AdamW, Linear, Optimizer, ParamsAdamW, VarBuilder, VarMap};
use ndarray::{Array1, Array2};
use serde::{Deserialize, Serialize};

use features::{build, split, Scaler, FEATURE_NAMES};
use metrics::{roc_auc, Confusion};

/// 多層感知機：9 → 16 → 8 → 1。
/// 兩層隱藏層加 ReLU 讓它能學到特徵的交互作用（ML-2 邏輯回歸學不到的東西）。
struct Mlp {
    l1: Linear,
    l2: Linear,
    l3: Linear,
}

impl Mlp {
    fn new(vs: VarBuilder) -> Result<Self> {
        // vs.pp("l1")：參數的命名空間。存檔時權重會叫 l1.weight、l1.bias，載入時靠名字對上
        Ok(Self {
            l1: linear(FEATURE_NAMES.len(), 16, vs.pp("l1"))?,
            l2: linear(16, 8, vs.pp("l2"))?,
            l3: linear(8, 1, vs.pp("l3"))?,
        })
    }

    /// 回傳 logit（未經 sigmoid）。訓練時用 logit 算損失較穩定，推論時再過 sigmoid。
    fn forward(&self, x: &Tensor) -> Result<Tensor> {
        let x = self.l1.forward(x)?.relu()?;
        let x = self.l2.forward(&x)?.relu()?;
        Ok(self.l3.forward(&x)?.squeeze(D::Minus1)?) // (n, 1) → (n,)
    }

    fn predict_proba(&self, x: &Tensor) -> Result<Tensor> {
        Ok(candle_nn::ops::sigmoid(&self.forward(x)?)?)
    }
}

/// 加權二元交叉熵（logit 版本，數值穩定）：
/// loss = w · [ max(z,0) − z·y + ln(1 + e^{−|z|}) ]
fn weighted_bce_with_logits(logits: &Tensor, y: &Tensor, pos_weight: f64) -> Result<Tensor> {
    let zeros = logits.zeros_like()?;
    let relu = logits.maximum(&zeros)?;
    let zy = (logits * y)?;
    let log_term = (logits.abs()?.neg()?.exp()? + 1.0)?.log()?;
    let per_sample = ((relu - zy)? + log_term)?;
    // 權重：y=1 → pos_weight，y=0 → 1
    let w = ((y * (pos_weight - 1.0))? + 1.0)?;
    let weighted = (per_sample * &w)?;
    Ok((weighted.sum_all()? / w.sum_all()?)?)
}

/// ndarray → Tensor（f32）。candle 的 CPU 後端 f32 比 f64 快很多，精度對這個問題足夠。
fn to_tensor(x: &Array2<f64>, dev: &Device) -> Result<Tensor> {
    let (n, d) = x.dim();
    let data: Vec<f32> = x.iter().map(|&v| v as f32).collect(); // 標準列優先佈局，iter 順序正確
    Ok(Tensor::from_vec(data, (n, d), dev)?)
}

fn to_tensor_1d(y: &Array1<f64>, dev: &Device) -> Result<Tensor> {
    let data: Vec<f32> = y.iter().map(|&v| v as f32).collect();
    Ok(Tensor::from_vec(data, y.len(), dev)?)
}

fn to_ndarray_1d(t: &Tensor) -> Result<Array1<f64>> {
    Ok(Array1::from(
        t.to_vec1::<f32>()?
            .into_iter()
            .map(f64::from)
            .collect::<Vec<_>>(),
    ))
}

/// Scaler 的參數要跟模型一起存，推論時才能做同樣的標準化
#[derive(Serialize, Deserialize)]
struct ScalerFile {
    feature_names: Vec<String>,
    mean: Vec<f64>,
    std: Vec<f64>,
}

fn main() -> Result<()> {
    let dev = Device::Cpu; // 有 CUDA/Metal 時改 Device::new_cuda(0)? / Device::new_metal(0)?，其餘程式碼不變
    let txs = synth::generate(6_000, 7);
    let feats = build(&txs);
    let (train, test) = split(&feats, 0.8);
    let scaler = Scaler::fit(&train.x);

    let xtr = to_tensor(&scaler.transform(&train.x), &dev)?;
    let ytr = to_tensor_1d(&train.y, &dev)?;
    let xte = to_tensor(&scaler.transform(&test.x), &dev)?;
    println!("訓練 {:?}，測試 {:?}\n", xtr.shape(), xte.shape());

    // ---------- 建模型與優化器 ----------
    // VarMap 持有所有可訓練參數；VarBuilder 負責建立並命名它們
    let varmap = VarMap::new();
    let vs = VarBuilder::from_varmap(&varmap, DType::F32, &dev);
    let model = Mlp::new(vs)?;
    let n_params: usize = varmap.all_vars().iter().map(|v| v.elem_count()).sum();
    println!("參數量：{n_params}（9×16+16 + 16×8+8 + 8×1+1）");

    let mut opt = AdamW::new(
        varmap.all_vars(),
        ParamsAdamW {
            lr: 5e-3,
            weight_decay: 1e-4,
            ..Default::default()
        },
    )?;

    // ---------- 訓練：mini-batch ----------
    let n = xtr.dim(0)?;
    let batch = 256;
    let epochs = 60;
    let pos_weight = 5.0;
    let t = Instant::now();
    for epoch in 0..epochs {
        let mut total = 0.0;
        let mut steps = 0;
        for start in (0..n).step_by(batch) {
            let len = batch.min(n - start);
            let xb = xtr.narrow(0, start, len)?; // 取一段列，零複製視圖
            let yb = ytr.narrow(0, start, len)?;
            let logits = model.forward(&xb)?;
            let loss = weighted_bce_with_logits(&logits, &yb, pos_weight)?;
            opt.backward_step(&loss)?; // 反向傳播 + 更新參數，一行
            total += loss.to_scalar::<f32>()? as f64;
            steps += 1;
        }
        if epoch % 10 == 0 || epoch == epochs - 1 {
            println!("epoch {epoch:>3}  loss {:.4}", total / steps as f64);
        }
    }
    println!("訓練時間：{} ms\n", t.elapsed().as_millis());

    // ---------- 評估 ----------
    let proba = to_ndarray_1d(&model.predict_proba(&xte)?)?;
    println!("AUC  MLP：{:.3}", roc_auc(&test.y, &proba));
    for thr in [0.3, 0.5, 0.7] {
        Confusion::from_scores(&test.y, &proba, thr).report(&format!("MLP thr={thr}"));
    }
    println!("→ 和 ML-2 的邏輯回歸比（AUC 0.995，thr=0.5 時 F1 0.881）：資料是合成的、特徵本來就偏線性，所以差距不大。");
    println!("  真實資料裡特徵交互作用多，MLP 的優勢會明顯一些；但也更容易過擬合、更難解釋。\n");

    // ---------- 存檔：權重 + Scaler ----------
    let out_dir = Path::new("model");
    std::fs::create_dir_all(out_dir)?;
    varmap.save(out_dir.join("fraud_mlp.safetensors"))?;
    let scaler_file = ScalerFile {
        feature_names: FEATURE_NAMES.iter().map(|s| s.to_string()).collect(),
        mean: scaler.mean.to_vec(),
        std: scaler.std.to_vec(),
    };
    std::fs::write(
        out_dir.join("scaler.json"),
        serde_json::to_string_pretty(&scaler_file)?,
    )?;
    println!(
        "已存檔：{}（{} bytes）與 scaler.json",
        out_dir.join("fraud_mlp.safetensors").display(),
        std::fs::metadata(out_dir.join("fraud_mlp.safetensors"))?.len()
    );

    // ---------- 載回來驗證：新的 VarMap、同樣的網路結構、load 權重 ----------
    let mut varmap2 = VarMap::new(); // load 需要 &mut
    let vs2 = VarBuilder::from_varmap(&varmap2, DType::F32, &dev);
    let model2 = Mlp::new(vs2)?; // 先建結構（參數是隨機初始化的）
    varmap2.load(out_dir.join("fraud_mlp.safetensors"))?; // 再用名字對上，覆寫參數
    let proba2 = to_ndarray_1d(&model2.predict_proba(&xte)?)?;
    let max_diff = (&proba - &proba2)
        .mapv(f64::abs)
        .fold(0.0, |a: f64, b| a.max(*b));
    println!("載回來的模型與原模型預測最大差異：{max_diff:.2e}（應為 0）");

    // ---------- 單筆推論的延遲：ML-5 的 API 關心這個 ----------
    let one = xte.narrow(0, 0, 1)?;
    let t = Instant::now();
    let iters = 1000;
    for _ in 0..iters {
        let _ = model2.predict_proba(&one)?;
    }
    println!(
        "單筆推論平均：{:.1} µs",
        t.elapsed().as_micros() as f64 / iters as f64
    );

    Ok(())
}
