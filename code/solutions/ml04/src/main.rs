//! ML-4 練習解答

mod features;
#[allow(dead_code)]
mod metrics;
mod synth;

use std::time::Instant;

use anyhow::Result;
use candle_core::{DType, Device, Module, Tensor, D};
use candle_nn::{linear, AdamW, Linear, Optimizer, ParamsAdamW, VarBuilder, VarMap};
use ndarray::{s, Array1, Array2};

use features::{build, split, Scaler, FEATURE_NAMES};
use metrics::{roc_auc, Confusion};

/// 可設定寬度與 dropout 的 MLP
struct Mlp {
    l1: Linear,
    l2: Linear,
    l3: Linear,
    dropout: f32,
}

impl Mlp {
    fn new(vs: VarBuilder, hidden: (usize, usize), dropout: f32) -> Result<Self> {
        Ok(Self {
            l1: linear(FEATURE_NAMES.len(), hidden.0, vs.pp("l1"))?,
            l2: linear(hidden.0, hidden.1, vs.pp("l2"))?,
            l3: linear(hidden.1, 1, vs.pp("l3"))?,
            dropout,
        })
    }

    /// 練習 2：forward 需要知道現在是訓練還是推論。訓練時隨機丟掉一部分神經元（並把剩下的放大補償），
    /// 推論時不丟。這是 dropout 正則化的標準做法。
    fn forward(&self, x: &Tensor, train: bool) -> Result<Tensor> {
        let mut x = self.l1.forward(x)?.relu()?;
        if train && self.dropout > 0.0 {
            x = candle_nn::ops::dropout(&x, self.dropout)?;
        }
        let mut x = self.l2.forward(&x)?.relu()?;
        if train && self.dropout > 0.0 {
            x = candle_nn::ops::dropout(&x, self.dropout)?;
        }
        Ok(self.l3.forward(&x)?.squeeze(D::Minus1)?)
    }

    fn predict_proba(&self, x: &Tensor) -> Result<Array1<f64>> {
        let p = candle_nn::ops::sigmoid(&self.forward(x, false)?)?;
        Ok(Array1::from(
            p.to_vec1::<f32>()?
                .into_iter()
                .map(f64::from)
                .collect::<Vec<_>>(),
        ))
    }
}

fn bce_with_logits(logits: &Tensor, y: &Tensor, pos_weight: f64) -> Result<Tensor> {
    let relu = logits.maximum(&logits.zeros_like()?)?;
    let zy = (logits * y)?;
    let log_term = (logits.abs()?.neg()?.exp()? + 1.0)?.log()?;
    let per = ((relu - zy)? + log_term)?;
    let w = ((y * (pos_weight - 1.0))? + 1.0)?;
    Ok(((per * &w)?.sum_all()? / w.sum_all()?)?)
}

fn to_tensor(x: &Array2<f64>, dev: &Device) -> Result<Tensor> {
    let (n, d) = x.dim();
    Ok(Tensor::from_vec(
        x.iter().map(|&v| v as f32).collect::<Vec<_>>(),
        (n, d),
        dev,
    )?)
}

fn to_tensor_1d(y: &Array1<f64>, dev: &Device) -> Result<Tensor> {
    Ok(Tensor::from_vec(
        y.iter().map(|&v| v as f32).collect::<Vec<_>>(),
        y.len(),
        dev,
    )?)
}

struct TrainResult {
    train_f1: f64,
    test_f1: f64,
    test_auc: f64,
    epochs_run: usize,
    ms: u128,
}

/// 一次訓練實驗。validation 不為 None 時啟用早停（練習 3）。
fn train(
    dev: &Device,
    hidden: (usize, usize),
    dropout: f32,
    epochs: usize,
    train_x: &Array2<f64>,
    train_y: &Array1<f64>,
    valid: Option<(&Array2<f64>, &Array1<f64>)>,
    test_x: &Array2<f64>,
    test_y: &Array1<f64>,
) -> Result<TrainResult> {
    let varmap = VarMap::new();
    let model = Mlp::new(
        VarBuilder::from_varmap(&varmap, DType::F32, dev),
        hidden,
        dropout,
    )?;
    let mut opt = AdamW::new(
        varmap.all_vars(),
        ParamsAdamW {
            lr: 5e-3,
            weight_decay: 1e-4,
            ..Default::default()
        },
    )?;
    let (xtr, ytr) = (to_tensor(train_x, dev)?, to_tensor_1d(train_y, dev)?);
    let xte = to_tensor(test_x, dev)?;
    let valid_t = valid.map(|(x, y)| (to_tensor(x, dev).unwrap(), to_tensor_1d(y, dev).unwrap()));

    let t = Instant::now();
    let n = xtr.dim(0)?;
    let (mut best_valid, mut since_best, mut epochs_run) = (f32::INFINITY, 0usize, 0usize);
    for epoch in 0..epochs {
        for start in (0..n).step_by(256) {
            let len = 256.min(n - start);
            let loss = bce_with_logits(
                &model.forward(&xtr.narrow(0, start, len)?, true)?,
                &ytr.narrow(0, start, len)?,
                5.0,
            )?;
            opt.backward_step(&loss)?;
        }
        epochs_run = epoch + 1;
        // 練習 3：早停。驗證損失連續 5 個 epoch 沒進步就停。
        if let Some((vx, vy)) = &valid_t {
            let vloss = bce_with_logits(&model.forward(vx, false)?, vy, 5.0)?.to_scalar::<f32>()?;
            if vloss < best_valid - 1e-4 {
                best_valid = vloss;
                since_best = 0;
            } else {
                since_best += 1;
                if since_best >= 5 {
                    break;
                }
            }
        }
    }
    let f1 = |x: &Tensor, y: &Array1<f64>| -> Result<(f64, f64)> {
        let p = model.predict_proba(x)?;
        Ok((Confusion::from_scores(y, &p, 0.5).f1(), roc_auc(y, &p)))
    };
    let (train_f1, _) = f1(&xtr, train_y)?;
    let (test_f1, test_auc) = f1(&xte, test_y)?;
    Ok(TrainResult {
        train_f1,
        test_f1,
        test_auc,
        epochs_run,
        ms: t.elapsed().as_millis(),
    })
}

fn main() -> Result<()> {
    // 練習 4：有 GPU feature 時用 GPU。對 305 個參數、4800 筆資料的模型，GPU 不會比較快：
    // 每個 batch 的運算只有幾微秒，把資料搬到 GPU 與啟動 kernel 的固定成本遠大於運算本身。
    // GPU 的優勢在大矩陣（百萬參數、大 batch）。
    #[cfg(feature = "metal")]
    let dev = Device::new_metal(0)?;
    #[cfg(feature = "cuda")]
    let dev = Device::new_cuda(0)?;
    #[cfg(not(any(feature = "metal", feature = "cuda")))]
    let dev = Device::Cpu;
    println!("裝置：{dev:?}\n");

    let txs = synth::generate(6_000, 7);
    let feats = build(&txs);
    let (train_all, test) = split(&feats, 0.8);
    let scaler = Scaler::fit(&train_all.x);
    let (xtr_all, xte) = (scaler.transform(&train_all.x), scaler.transform(&test.x));

    // ---------- 練習 1：更大的網路、更多 epoch ----------
    println!("=== 練習 1：容量與過擬合（訓練 F1 vs 測試 F1）===");
    println!(
        "{:<22} {:>6} {:>8} {:>8} {:>6} {:>7}",
        "設定", "epoch", "訓練F1", "測試F1", "AUC", "ms"
    );
    for (label, hidden, epochs) in [
        ("16→8, 60 epoch", (16, 8), 60),
        ("64→32, 60 epoch", (64, 32), 60),
        ("64→32, 200 epoch", (64, 32), 200),
    ] {
        let r = train(
            &dev,
            hidden,
            0.0,
            epochs,
            &xtr_all,
            &train_all.y,
            None,
            &xte,
            &test.y,
        )?;
        println!(
            "{label:<22} {:>6} {:>8.3} {:>8.3} {:>6.3} {:>7}",
            r.epochs_run, r.train_f1, r.test_f1, r.test_auc, r.ms
        );
    }
    println!("→ 訓練 F1 隨容量與 epoch 上升、測試 F1 沒跟上甚至下降 = 過擬合。差距就是模型「背答案」的程度。\n");

    // ---------- 練習 2：dropout ----------
    println!("=== 練習 2：dropout（64→32, 200 epoch）===");
    for dropout in [0.0f32, 0.2, 0.5] {
        let r = train(
            &dev,
            (64, 32),
            dropout,
            200,
            &xtr_all,
            &train_all.y,
            None,
            &xte,
            &test.y,
        )?;
        println!(
            "dropout={dropout:<4} 訓練F1 {:.3}  測試F1 {:.3}  AUC {:.3}",
            r.train_f1, r.test_f1, r.test_auc
        );
    }
    println!("→ dropout 拉近訓練與測試的差距（正則化），但太大會讓模型學不夠（0.5 對這麼小的網路太多）。\n");

    // ---------- 練習 3：早停 ----------
    println!("=== 練習 3：早停（從訓練集切 10% 當驗證集）===");
    let n_valid = xtr_all.nrows() / 10;
    let cut = xtr_all.nrows() - n_valid; // 時間順序：最後 10% 當驗證
    let (xtr, ytr) = (
        xtr_all.slice(s![..cut, ..]).to_owned(),
        train_all.y.slice(s![..cut]).to_owned(),
    );
    let (xva, yva) = (
        xtr_all.slice(s![cut.., ..]).to_owned(),
        train_all.y.slice(s![cut..]).to_owned(),
    );
    let r = train(
        &dev,
        (64, 32),
        0.0,
        200,
        &xtr,
        &ytr,
        Some((&xva, &yva)),
        &xte,
        &test.y,
    )?;
    println!(
        "最多 200 epoch，實際跑了 {}；訓練F1 {:.3}  測試F1 {:.3}  AUC {:.3}  {} ms",
        r.epochs_run, r.train_f1, r.test_f1, r.test_auc, r.ms
    );
    println!("→ 早停用驗證集決定「何時停」，不用手調 epoch 數，而且快得多。但 patience=5 對這題太急：第 7 個 epoch 就停，");
    println!("  模型還沒學夠（欠擬合）。實務上 patience 10~20，並且保留「驗證損失最低時」的權重而不是最後一次的。驗證集也要按時間切。\n");

    // ---------- 練習 5：載入「別的框架」存的 safetensors ----------
    println!("=== 練習 5：safetensors 的命名要求 ===");
    // 先用 candle 存一份正確的，再用 safetensors crate 讀出來、把 key 改名存成另一份，模擬 PyTorch 端命名不同
    let varmap = VarMap::new();
    let _m = Mlp::new(
        VarBuilder::from_varmap(&varmap, DType::F32, &dev),
        (16, 8),
        0.0,
    )?;
    let dir = std::env::temp_dir();
    let good = dir.join("sol-ml04-good.safetensors");
    let renamed = dir.join("sol-ml04-renamed.safetensors");
    varmap.save(&good)?;

    let bytes = std::fs::read(&good)?;
    let st = safetensors::SafeTensors::deserialize(&bytes)?;
    println!("檔案裡的 tensor 名稱：{:?}", {
        let mut n: Vec<_> = st.names();
        n.sort();
        n
    });
    // PyTorch 慣用 fc1.weight 之類的名字；改名後 candle 就對不上
    let renamed_tensors: Vec<(String, safetensors::tensor::TensorView)> = st
        .tensors()
        .into_iter()
        .map(|(k, v)| (k.replace("l1", "fc1"), v))
        .collect();
    safetensors::serialize_to_file(renamed_tensors, None, &renamed)?;

    let mut vm2 = VarMap::new();
    let _m2 = Mlp::new(
        VarBuilder::from_varmap(&vm2, DType::F32, &dev),
        (16, 8),
        0.0,
    )?;
    match vm2.load(&renamed) {
        Ok(()) => println!("不該成功"),
        Err(e) => println!("載入改名後的檔案失敗（預期）：{e}"),
    }
    println!("→ varmap.load 用名字對應，找不到 l1.weight 就報錯，而不是靜默載入垃圾。");
    println!(
        "  Python 端存檔時要用相同名字：save_file({{\"l1.weight\": ..., \"l1.bias\": ...}})，"
    );
    println!("  或在 Rust 端載入前用 safetensors crate 做一次名稱轉換。注意 PyTorch 的 Linear.weight 形狀是 (out, in)，與 candle 相同。");
    Ok(())
}
