//! ML-3：用 linfa 做邏輯回歸與 K-means。

mod features;
mod logistic;
mod metrics;
mod synth;

use std::collections::HashMap;

use linfa::prelude::*;
use linfa_clustering::KMeans;
use linfa_logistic::LogisticRegression;
use linfa_preprocessing::linear_scaling::LinearScaler;
use ndarray::{Array1, Array2, Axis};

use features::{build, split, FEATURE_NAMES};
use metrics::{roc_auc, Confusion};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let txs = synth::generate(6_000, 7);
    let feats = build(&txs);
    let (train, test) = split(&feats, 0.8);

    // ---------- Dataset：linfa 的核心型別，records（特徵）+ targets（標記） ----------
    // 邏輯回歸的 target 要是「類別」型別（bool、usize、&str…），不是 f64
    let to_bool = |y: &Array1<f64>| y.mapv(|v| v >= 0.5);
    let train_ds = Dataset::new(train.x.clone(), to_bool(&train.y)).with_feature_names(FEATURE_NAMES.to_vec());
    let test_ds = Dataset::new(test.x.clone(), to_bool(&test.y)).with_feature_names(FEATURE_NAMES.to_vec());
    println!("訓練集：{} 筆 × {} 特徵；測試集：{} 筆\n", train_ds.nsamples(), train_ds.nfeatures(), test_ds.nsamples());

    // ---------- 標準化：linfa-preprocessing 的 LinearScaler 取代我們手寫的 Scaler ----------
    // 一樣是 fit（訓練集）→ transform（兩邊）。這是 linfa 所有東西的共同介面：Fit 與 Transformer/Predict trait
    let scaler = LinearScaler::standard().fit(&train_ds)?;
    let train_ds = scaler.transform(train_ds);
    let test_ds = scaler.transform(test_ds);

    // ---------- 邏輯回歸：三行 ----------
    let model = LogisticRegression::default()
        .max_iterations(500)
        .alpha(1e-3) // L2 正則化
        .fit(&train_ds)?;

    let pred: Array1<bool> = model.predict(&test_ds);
    // 陷阱：predict_probabilities 回傳的是「排序後第一個類別」的機率。bool 排序 false < true，
    // 所以它給的是 P(正常)。我們要 P(詐欺)，取 1 − p。權重的正負號也是相對於 false 這個類別。
    // （用 predict() 不會有這個問題，它直接回傳類別。）
    let proba: Array1<f64> = model.predict_probabilities(test_ds.records()).mapv(|p| 1.0 - p);

    // linfa 內建的混淆矩陣與指標
    let cm = pred.confusion_matrix(&test_ds)?;
    println!("=== linfa 邏輯回歸（threshold 0.5）===");
    println!("{cm:?}");
    println!("precision {:.3}  recall {:.3}  F1 {:.3}  accuracy {:.3}", cm.precision(), cm.recall(), cm.f1_score(), cm.accuracy());
    println!("AUC {:.3}（用我們 ML-2 寫的 roc_auc）", roc_auc(&test.y, &proba));

    // 和 ML-2 自己寫的比
    println!("\n=== 對照：ML-2 手寫的邏輯回歸 ===");
    let my_scaler = features::Scaler::fit(&train.x);
    let mut mine = logistic::LogisticRegression::new(FEATURE_NAMES.len());
    mine.learning_rate = 0.3;
    mine.epochs = 400;
    mine.pos_weight = 1.0; // 公平比較：linfa 預設沒有類別加權
    mine.fit(&my_scaler.transform(&train.x), &train.y);
    let my_proba = mine.predict_proba(&my_scaler.transform(&test.x));
    Confusion::from_scores(&test.y, &my_proba, 0.5).report("手寫（pos_weight=1）");
    Confusion::from_scores(&test.y, &proba, 0.5).report("linfa");
    println!("AUC 手寫 {:.3} vs linfa {:.3}", roc_auc(&test.y, &my_proba), roc_auc(&test.y, &proba));
    println!("→ AUC 幾乎一樣。linfa 用 L-BFGS 最佳化、我們用梯度下降；差異來自正則化強度與收斂程度。");

    // 權重比較
    println!("\n{:<18} {:>8} {:>8}   （linfa 的權重取負號，換成「對詐欺類別」的方向）", "特徵", "linfa", "手寫");
    for (i, name) in FEATURE_NAMES.iter().enumerate() {
        println!("{:<18} {:>+8.3} {:>+8.3}", name, -model.params()[i], mine.weights[i]);
    }
    println!("{:<18} {:>+8.3} {:>+8.3}", "intercept", -model.intercept(), mine.bias);
    println!("→ 方向一致、大小不同：linfa 的正則化較弱（alpha 作用方式不同），權重較大。");

    // ---------- 自訂 threshold：用機率而不是 predict() ----------
    println!("\n=== 用機率自訂 threshold ===");
    for thr in [0.2, 0.5, 0.8] {
        Confusion::from_scores(&test.y, &proba, thr).report(&format!("linfa thr={thr}"));
    }

    // ---------- K-means：帳戶分群（無監督）----------
    println!("\n=== K-means：帳戶行為分群 ===");
    // 先把每個帳戶的交易聚合成一個特徵向量（ML-1 練習 3）
    let (account_matrix, account_ids, account_fraud_rate) = aggregate_by_account(&txs, &feats.x);
    let acct_ds = DatasetBase::from(account_matrix.clone());
    let acct_scaler = LinearScaler::standard().fit(&acct_ds)?;
    let acct_ds = acct_scaler.transform(acct_ds);

    let k = 3;
    let kmeans = KMeans::params(k).max_n_iterations(300).tolerance(1e-5).fit(&acct_ds)?;
    let clusters: Array1<usize> = kmeans.predict(&acct_ds);

    println!("{} 個帳戶分成 {k} 群：", account_ids.len());
    for c in 0..k {
        let members: Vec<usize> = clusters.iter().enumerate().filter(|(_, &cl)| cl == c).map(|(i, _)| i).collect();
        let fraud_rate: f64 = members.iter().map(|&i| account_fraud_rate[i]).sum::<f64>() / members.len().max(1) as f64;
        let centroid = kmeans.centroids().row(c);
        // 找這群最突出的兩個特徵（centroid 絕對值最大）
        let mut top: Vec<(usize, f64)> = centroid.iter().copied().enumerate().collect();
        top.sort_by(|a, b| b.1.abs().partial_cmp(&a.1.abs()).unwrap());
        println!(
            "  群 {c}：{:>3} 個帳戶，平均詐欺率 {:>5.1}%，特色：{} {:+.2}、{} {:+.2}",
            members.len(),
            fraud_rate * 100.0,
            FEATURE_NAMES[top[0].0], top[0].1,
            FEATURE_NAMES[top[1].0], top[1].1
        );
    }
    println!("→ 分群沒有用到標記，卻把高詐欺率的帳戶分到了同一群。這種「行為分群」可以用來設定不同的風控門檻。");

    Ok(())
}

/// 每個帳戶：所有交易特徵的平均（帳戶數 × 9），以及該帳戶的詐欺率（評估用）
fn aggregate_by_account(txs: &[synth::LabeledTx], x: &Array2<f64>) -> (Array2<f64>, Vec<u32>, Vec<f64>) {
    let mut groups: HashMap<u32, Vec<usize>> = HashMap::new();
    for (i, tx) in txs.iter().enumerate() {
        groups.entry(tx.account).or_default().push(i);
    }
    let mut ids: Vec<u32> = groups.keys().copied().collect();
    ids.sort();
    let mut m = Array2::<f64>::zeros((ids.len(), x.ncols()));
    let mut fraud = Vec::with_capacity(ids.len());
    for (r, id) in ids.iter().enumerate() {
        let rows = &groups[id];
        let mean = x.select(Axis(0), rows).mean_axis(Axis(0)).unwrap();
        m.row_mut(r).assign(&mean);
        fraud.push(rows.iter().filter(|&&i| txs[i].is_fraud).count() as f64 / rows.len() as f64);
    }
    (m, ids, fraud)
}
