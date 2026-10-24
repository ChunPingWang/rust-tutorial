//! ML-1 練習解答

mod features;
mod synth;

use ndarray::Axis;

use features::{account_profiles, build, MinMaxScaler, Scaler, FEATURE_NAMES};

fn main() {
    let txs = synth::generate(6_000, 7);
    let feats = build(&txs);
    let xs = Scaler::fit(&feats.x).transform(&feats.x);

    let fraud_idx: Vec<usize> = feats
        .y
        .iter()
        .enumerate()
        .filter(|(_, &v)| v == 1.0)
        .map(|(i, _)| i)
        .collect();
    let normal_idx: Vec<usize> = feats
        .y
        .iter()
        .enumerate()
        .filter(|(_, &v)| v == 0.0)
        .map(|(i, _)| i)
        .collect();
    let fraud_mean = xs.select(Axis(0), &fraud_idx).mean_axis(Axis(0)).unwrap();
    let normal_mean = xs.select(Axis(0), &normal_idx).mean_axis(Axis(0)).unwrap();

    println!("=== 練習 1、2：新特徵的鑑別力（標準化後的詐欺/正常平均差距）===");
    for name in ["is_new_counterparty", "velocity_1h", "velocity_24h"] {
        let i = FEATURE_NAMES.iter().position(|n| *n == name).unwrap();
        println!(
            "  {:<22} 正常 {:>+7.3}  詐欺 {:>+7.3}  差距 {:>+7.3}",
            name,
            normal_mean[i],
            fraud_mean[i],
            fraud_mean[i] - normal_mean[i]
        );
    }
    println!("→ velocity_1h 的差距比 24h 大：詐欺是「幾分鐘內連續」，24 小時的窗口把訊號稀釋了。");
    println!("  is_new_counterparty 在合成資料裡是替身欄位，差距小是預期的；真實資料裡它通常是強特徵。\n");

    println!("=== 練習 3：帳戶平均特徵矩陣 ===");
    let (profiles, ids) = account_profiles(&txs, &feats.x);
    println!("形狀 {:?}，前三個帳戶：{:?}", profiles.dim(), &ids[..3]);
    println!(
        "帳戶 {} 的平均 log_amount = {:.3}、平均 velocity_24h = {:.2}\n",
        ids[0],
        profiles[[0, 0]],
        profiles[[0, 8]]
    );

    println!("=== 練習 4：min-max vs z-score，看 amount_vs_acct 這一欄 ===");
    let col = 1;
    let mm = MinMaxScaler::fit(&feats.x).transform(&feats.x);
    let mm_col = mm.column(col);
    let z_col = xs.column(col);
    let pct_below = |v: ndarray::ArrayView1<f64>, t: f64| {
        v.iter().filter(|&&x| x < t).count() as f64 / v.len() as f64 * 100.0
    };
    println!(
        "  min-max：{:.1}% 的資料被壓在 0..0.3 的區間（最大值 30 把範圍拉得很寬）",
        pct_below(mm_col, 0.3)
    );
    println!(
        "  z-score：{:.1}% 的資料落在 -1..1",
        z_col.iter().filter(|&&x| x.abs() < 1.0).count() as f64 / z_col.len() as f64 * 100.0
    );
    println!(
        "  min-max 的詐欺/正常平均差距 {:.3} vs z-score {:.3}",
        {
            let f = mm.select(Axis(0), &fraud_idx).mean_axis(Axis(0)).unwrap()[col];
            let n = mm.select(Axis(0), &normal_idx).mean_axis(Axis(0)).unwrap()[col];
            f - n
        },
        fraud_mean[col] - normal_mean[col]
    );
    println!(
        "→ 有離群值的欄用 z-score（或 robust scaler）；min-max 適合有明確上下界的欄（例如 hour）。"
    );
}
