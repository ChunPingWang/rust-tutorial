//! ML-1：用 ndarray 把交易變成特徵矩陣，並看懂它。

mod features;
mod synth;

use ndarray::{s, Array1, Array2, Axis};

use features::{build, split, Scaler, FEATURE_NAMES};

fn main() {
    let txs = synth::generate(6_000, 7);
    let fraud = txs.iter().filter(|t| t.is_fraud).count();
    println!("交易 {} 筆，其中詐欺 {} 筆（{:.1}%）\n", txs.len(), fraud, fraud as f64 / txs.len() as f64 * 100.0);

    // 先看幾筆原始資料
    for tx in txs.iter().take(3).chain(txs.iter().filter(|t| t.is_fraud).take(2)) {
        println!("  {:?}", tx);
    }

    // ---------- 建矩陣 ----------
    let feats = build(&txs);
    let x = &feats.x;
    println!("\n特徵矩陣形狀：{:?}（{} 列 × {} 欄）", x.dim(), x.nrows(), x.ncols());
    println!("第一列：{:.3}", x.row(0));
    println!("前 3 列的前 4 欄：\n{:.3}", x.slice(s![0..3, 0..4]));

    // ---------- 每欄的統計 ----------
    let mean = x.mean_axis(Axis(0)).unwrap();
    let std = x.std_axis(Axis(0), 0.0);
    let min = x.fold_axis(Axis(0), f64::INFINITY, |a, b| a.min(*b));
    let max = x.fold_axis(Axis(0), f64::NEG_INFINITY, |a, b| a.max(*b));
    println!("\n{:<18} {:>9} {:>9} {:>9} {:>9}", "特徵", "平均", "標準差", "最小", "最大");
    for (i, name) in FEATURE_NAMES.iter().enumerate() {
        println!("{:<18} {:>9.3} {:>9.3} {:>9.3} {:>9.3}", name, mean[i], std[i], min[i], max[i]);
    }
    println!("→ 各欄尺度差很多（velocity 到幾十、hour_sin 在 ±1）。模型會被大尺度的欄主導，所以要標準化。");

    // ---------- 標準化 ----------
    let scaler = Scaler::fit(x);
    let xs = scaler.transform(x);
    println!("\n標準化後每欄平均 ≈ 0：{:.1e}", xs.mean_axis(Axis(0)).unwrap().mapv(f64::abs).sum() / 9.0);
    println!("標準化後每欄標準差 ≈ 1：{:.3}", xs.std_axis(Axis(0), 0.0));

    // ---------- 哪些特徵和詐欺有關？分組平均 ----------
    // 用布林遮罩選列：這是 ndarray 沒有直接提供的操作，用 select 做
    let fraud_idx: Vec<usize> = feats.y.iter().enumerate().filter(|(_, &v)| v == 1.0).map(|(i, _)| i).collect();
    let normal_idx: Vec<usize> = feats.y.iter().enumerate().filter(|(_, &v)| v == 0.0).map(|(i, _)| i).collect();
    let fraud_mean = xs.select(Axis(0), &fraud_idx).mean_axis(Axis(0)).unwrap();
    let normal_mean = xs.select(Axis(0), &normal_idx).mean_axis(Axis(0)).unwrap();
    println!("\n{:<18} {:>10} {:>10} {:>8}", "特徵（標準化後）", "正常平均", "詐欺平均", "差距");
    let mut gaps: Vec<(usize, f64)> = (0..9).map(|i| (i, fraud_mean[i] - normal_mean[i])).collect();
    gaps.sort_by(|a, b| b.1.abs().partial_cmp(&a.1.abs()).unwrap());
    for (i, gap) in &gaps {
        println!("{:<18} {:>10.3} {:>10.3} {:>+8.3}", FEATURE_NAMES[*i], normal_mean[*i], fraud_mean[*i], gap);
    }
    println!("→ 差距越大的特徵越有鑑別力。但沒有任何一個差距大到可以單獨當規則。");

    // ---------- 特徵之間的相關係數矩陣 ----------
    // corr = (Xᵀ X) / n，X 已標準化。ndarray 的 .t() 是零成本的轉置視圖，.dot() 是矩陣乘法
    let n = xs.nrows() as f64;
    let corr: Array2<f64> = xs.t().dot(&xs) / n;
    println!("\n相關係數矩陣（部分）：");
    println!("{:<18}", "");
    for i in [0usize, 2, 4, 5, 6, 8] {
        print!("{:<18}", FEATURE_NAMES[i]);
        for j in [0usize, 2, 4, 5, 6, 8] {
            print!("{:>7.2}", corr[[i, j]]);
        }
        println!();
    }
    println!("→ is_night 與 hour_cos 高度相關是預期的（兩者都在描述時段）。高相關的特徵對線性模型是重複資訊。");

    // ---------- 時間切分 ----------
    let (train, test) = split(&feats, 0.8);
    let rate = |y: &Array1<f64>| y.sum() / y.len() as f64 * 100.0;
    println!(
        "\n訓練集 {} 筆（詐欺 {:.1}%），測試集 {} 筆（詐欺 {:.1}%）",
        train.x.nrows(), rate(&train.y), test.x.nrows(), rate(&test.y)
    );
    println!("→ 下一章用訓練集學、測試集驗證。Scaler 只能用訓練集 fit，再套到測試集。");

    // ---------- 一些 ndarray 基本操作 ----------
    println!("\n--- ndarray 速查 ---");
    let a = Array2::<f64>::from_shape_vec((2, 3), vec![1., 2., 3., 4., 5., 6.]).unwrap();
    println!("a =\n{a}");
    println!("a.t() =\n{}", a.t());
    println!("a.sum_axis(Axis(1)) = {}（每列加總）", a.sum_axis(Axis(1)));
    println!("a * 2 + 1 =\n{}", &a * 2.0 + 1.0);
    println!("a.mapv(f64::sqrt) =\n{:.3}", a.mapv(f64::sqrt));
    let v = Array1::from(vec![10., 20., 30.]);
    println!("a + v（廣播：v 加到每一列）=\n{}", &a + &v);
    println!("a.dot(&v) = {}（矩陣 × 向量）", a.dot(&v));
    println!("a.slice(s![.., 1..]) =\n{}", a.slice(s![.., 1..]));
}
