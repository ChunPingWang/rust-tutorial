//! ML-2 練習解答

mod features;
mod logistic;
mod metrics;
mod synth;

use features::{build, split, Scaler, FEATURE_NAMES};
use logistic::LogisticRegression;
use metrics::{roc_auc, Confusion};

fn main() {
    let txs = synth::generate(6_000, 7);
    let feats = build(&txs);
    let (train, test) = split(&feats, 0.8);
    let scaler = Scaler::fit(&train.x);
    let (xtr, xte) = (scaler.transform(&train.x), scaler.transform(&test.x));

    let base = |m: &mut LogisticRegression| {
        m.learning_rate = 0.3;
        m.epochs = 400;
    };

    // ---------- 練習 1：pos_weight ----------
    println!("=== 練習 1：pos_weight 對 precision / recall 的影響（thr=0.5）===");
    for pw in [1.0, 5.0, 20.0] {
        let mut m = LogisticRegression::new(FEATURE_NAMES.len());
        base(&mut m);
        m.pos_weight = pw;
        m.fit(&xtr, &train.y);
        Confusion::from_scores(&test.y, &m.predict_proba(&xte), 0.5)
            .report(&format!("pos_weight={pw}"));
    }
    println!("→ 權重越大 recall 越高、precision 越低：等於把決策邊界往「判詐欺」移。效果類似調低 threshold。\n");

    // ---------- 練習 2：學習率 ----------
    println!("=== 練習 2：學習率（標準化後的特徵）===");
    for lr in [30.0, 3.0, 0.3, 0.01] {
        let mut m = LogisticRegression::new(FEATURE_NAMES.len());
        base(&mut m);
        m.learning_rate = lr;
        m.epochs = 20;
        let losses = m.fit(&xtr, &train.y);
        let shown: Vec<String> = losses
            .iter()
            .step_by(4)
            .map(|l| format!("{l:.3}"))
            .collect();
        println!(
            "  lr={lr:<5} 前 20 epoch 每 4 個印一次：{}",
            shown.join(" → ")
        );
    }
    println!("→ 這個問題是凸的、特徵已標準化，連 lr=30 都收斂。學習率太大的災難要在尺度沒對齊時才看得到：");
    for lr in [0.3, 3.0, 30.0] {
        let mut m = LogisticRegression::new(FEATURE_NAMES.len());
        base(&mut m);
        m.learning_rate = lr;
        m.epochs = 20;
        let losses = m.fit(&train.x, &train.y); // 注意：未標準化的 train.x
        let shown: Vec<String> = losses
            .iter()
            .step_by(4)
            .map(|l| {
                if l.is_finite() {
                    format!("{l:.3}")
                } else {
                    "NaN".into()
                }
            })
            .collect();
        println!("  未標準化 lr={lr:<4} {}", shown.join(" → "));
    }
    println!("→ 未標準化時：lr=3 前幾個 epoch 卡住不動（大尺度欄把 sigmoid 推到飽和、梯度消失），lr=30 直接失控；\n  同樣的 20 個 epoch，標準化版本全部收斂到更低的損失。這就是 ML-1 要標準化的原因。\n");

    // ---------- 練習 3：mini-batch ----------
    println!("=== 練習 3：全批次 vs mini-batch（達到損失 < 0.10 需要幾個 epoch）===");
    for bs in [0usize, 512, 64] {
        let mut m = LogisticRegression::new(FEATURE_NAMES.len());
        base(&mut m);
        m.learning_rate = 0.1;
        m.pos_weight = 5.0;
        m.batch_size = bs;
        let losses = m.fit(&xtr, &train.y);
        let reached = losses
            .iter()
            .position(|&l| l < 0.10)
            .map(|e| e.to_string())
            .unwrap_or("未達到".into());
        println!(
            "  batch={:<5} 第 {} 個 epoch 達標，最終損失 {:.4}",
            if bs == 0 {
                "全部".to_string()
            } else {
                bs.to_string()
            },
            reached,
            losses.last().unwrap()
        );
    }
    println!("→ 同樣看過資料一次，mini-batch 更新了 n/batch 次，所以收斂快得多；代價是每步的梯度比較嘈雜。\n");

    // ---------- 練習 4：L1 ----------
    println!("=== 練習 4：L1 正則化把哪些權重壓到 0 ===");
    let mut m = LogisticRegression::new(FEATURE_NAMES.len());
    base(&mut m);
    m.pos_weight = 5.0;
    m.l1 = 0.02;
    m.epochs = 800;
    m.fit(&xtr, &train.y);
    for (i, name) in FEATURE_NAMES.iter().enumerate() {
        let w = m.weights[i];
        println!(
            "  {:<18} {:>+7.3} {}",
            name,
            w,
            if w.abs() < 0.02 {
                "← 幾乎為 0，可考慮移除"
            } else {
                ""
            }
        );
    }
    println!(
        "AUC（L1 後）：{:.3}",
        roc_auc(&test.y, &m.predict_proba(&xte))
    );
    println!("→ L1 把幾個權重壓到接近 0，AUC 略降：這是「模型更簡單、更好解釋」與「準確率」的交換，l1 的大小控制交換比例。");
    println!("  被壓掉的往往是與保留特徵高度相關的（資訊重複）；但它也可能壓掉業務上有意義的特徵，結果要和領域知識對照。\n");

    // ---------- 練習 5：PR 曲線 ----------
    println!("=== 練習 5：precision-recall 曲線，找 precision ≥ 0.9 時最高的 recall ===");
    let mut m = LogisticRegression::new(FEATURE_NAMES.len());
    base(&mut m);
    m.pos_weight = 5.0;
    m.fit(&xtr, &train.y);
    let p = m.predict_proba(&xte);
    let mut best: Option<(f64, f64, f64)> = None;
    println!("  thr   prec  recall");
    for k in 1..100 {
        let thr = k as f64 / 100.0;
        let c = Confusion::from_scores(&test.y, &p, thr);
        if k % 10 == 0 {
            println!("  {thr:.2}  {:.3}  {:.3}", c.precision(), c.recall());
        }
        if c.precision() >= 0.9 && best.is_none_or(|b| c.recall() > b.2) {
            best = Some((thr, c.precision(), c.recall()));
        }
    }
    if let Some((thr, prec, rec)) = best {
        println!("→ precision ≥ 0.9 下最高 recall = {rec:.3}（thr = {thr:.2}，precision = {prec:.3}）。這就是給業務的「選項表」。");
    }
}
