//! ML-2：從零實作異常偵測（無監督）與邏輯回歸（監督）。

mod anomaly;
mod features;
mod logistic;
mod metrics;
mod synth;

use ndarray::Array1;

use features::{build, split, Scaler, FEATURE_NAMES};
use metrics::{roc_auc, Confusion};

fn main() {
    let txs = synth::generate(6_000, 7);
    let feats = build(&txs);
    let (train, test) = split(&feats, 0.8);

    // Scaler 只用訓練集 fit，再套到兩邊。測試集的統計量不能漏進來。
    let scaler = Scaler::fit(&train.x);
    let xtr = scaler.transform(&train.x);
    let xte = scaler.transform(&test.x);
    println!(
        "訓練 {} 筆 / 測試 {} 筆，測試集詐欺 {} 筆\n",
        xtr.nrows(),
        xte.nrows(),
        test.y.sum() as usize
    );

    // ========== 第一部分：沒有標記 ==========
    println!("=== 無監督：異常分數（完全不用標記，標記只用來事後評估）===");
    let z_scores = anomaly::zscore_anomaly(&xte);
    let m_scores = anomaly::mahalanobis(&xte);
    println!("AUC  z-score 異常分數：{:.3}", roc_auc(&test.y, &z_scores));
    println!("AUC  Mahalanobis 距離：{:.3}", roc_auc(&test.y, &m_scores));

    // 標記前 5% 為異常
    let thr = anomaly::threshold_for_top(&m_scores, 0.05);
    Confusion::from_scores(&test.y, &m_scores, thr).report("Mahalanobis top 5%");
    println!(
        "→ 沒有任何標記就能抓到大部分詐欺，代價是誤殺不少正常交易。這是冷啟動時的合理起點。\n"
    );

    // ========== 第二部分：有標記 ==========
    println!("=== 監督：邏輯回歸 ===");
    let mut model = logistic::LogisticRegression::new(FEATURE_NAMES.len());
    model.learning_rate = 0.3;
    model.epochs = 400;
    model.pos_weight = 5.0; // 詐欺只有 3%，給它 5 倍權重
    let losses = model.fit(&xtr, &train.y);
    println!(
        "損失：epoch 0 = {:.4}，epoch 50 = {:.4}，epoch 最後 = {:.4}（持續下降 = 有在學）",
        losses[0],
        losses[50],
        losses.last().unwrap()
    );

    let p_test = model.predict_proba(&xte);
    println!("AUC  邏輯回歸：{:.3}", roc_auc(&test.y, &p_test));
    println!();
    for thr in [0.3, 0.5, 0.7, 0.9] {
        Confusion::from_scores(&test.y, &p_test, thr).report(&format!("邏輯回歸 thr={thr}"));
    }
    println!("→ threshold 是業務決定：寧可多打擾客戶（低 thr、高 recall），還是寧可漏掉（高 thr、高 precision）？\n");

    // 模型學到了什麼？權重可以直接解釋
    println!("=== 權重（標準化特徵，絕對值大 = 影響大）===");
    let mut w: Vec<(usize, f64)> = model.weights.iter().copied().enumerate().collect();
    w.sort_by(|a, b| b.1.abs().partial_cmp(&a.1.abs()).unwrap());
    for (i, wi) in &w {
        println!("  {:<18} {:>+7.3}", FEATURE_NAMES[*i], wi);
    }
    println!("  bias               {:>+7.3}", model.bias);
    println!("→ 正權重：越大越可疑。可以直接拿去跟風控人員對答案：他們的直覺是不是這樣？\n");

    // 看幾筆測試集裡分數最高與「漏網」的
    println!("=== 分數最高的 5 筆 ===");
    let mut idx: Vec<usize> = (0..p_test.len()).collect();
    idx.sort_by(|&a, &b| p_test[b].partial_cmp(&p_test[a]).unwrap());
    for &i in idx.iter().take(5) {
        let tx = &txs[(train.x.nrows() + i) as usize];
        println!(
            "  p={:.3} 實際={} {:?}",
            p_test[i],
            if test.y[i] > 0.5 { "詐欺" } else { "正常" },
            tx
        );
    }
    println!("\n=== 漏網：實際詐欺但 p < 0.5 的前 3 筆 ===");
    let mut missed = 0;
    for &i in &idx {
        if test.y[i] > 0.5 && p_test[i] < 0.5 {
            let tx = &txs[train.x.nrows() + i];
            println!("  p={:.3} {:?}", p_test[i], tx);
            missed += 1;
            if missed == 3 {
                break;
            }
        }
    }
    println!("→ 漏網的通常是「看起來很正常」的詐欺：白天、金額不大。線性模型抓不到特徵之間的交互作用，這是 ML-4 神經網路的動機。");

    // 單筆推論：ML-5 的 API 會這樣用
    let one: Array1<f64> = test.x.row(0).to_owned();
    let p_one = model.predict_proba(&scaler.transform_row(&one).insert_axis(ndarray::Axis(0)))[0];
    println!("\n單筆推論示範：測試集第一筆 p = {p_one:.4}");
}
