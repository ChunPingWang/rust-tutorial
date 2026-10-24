//! ML-3 練習解答

mod features;
mod metrics;
mod synth;

use std::collections::HashMap;

use linfa::prelude::*;
use linfa_clustering::KMeans;
use linfa_logistic::LogisticRegression;
use linfa_preprocessing::linear_scaling::LinearScaler;
use linfa_reduction::Pca;
use linfa_trees::{DecisionTree, SplitQuality};
use ndarray::{concatenate, Array1, Array2, Axis};

use features::{build, split, FEATURE_NAMES};
use metrics::{roc_auc, Confusion};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let txs = synth::generate(6_000, 7);
    let feats = build(&txs);
    let (train, test) = split(&feats, 0.8);
    let to_bool = |y: &Array1<f64>| y.mapv(|v| v >= 0.5);

    // ---------- 練習 1：手肘法 ----------
    println!("=== 練習 1：K-means 手肘法（帳戶行為分群）===");
    let (account_matrix, _ids, _fraud) = aggregate_by_account(&txs, &feats.x);
    let acct_ds = DatasetBase::from(account_matrix.clone());
    let acct_ds = LinearScaler::standard().fit(&acct_ds)?.transform(acct_ds);
    let mut prev: Option<f64> = None;
    for k in 2..=8 {
        let km = KMeans::params(k)
            .max_n_iterations(300)
            .tolerance(1e-5)
            .fit(&acct_ds)?;
        let inertia = km.inertia();
        let drop = prev
            .map(|p| format!("（比上一個少 {:.0}%）", (p - inertia) / p * 100.0))
            .unwrap_or_default();
        println!("  k={k}  inertia {inertia:>8.1} {drop}");
        prev = Some(inertia);
    }
    println!("→ 下降幅度明顯變小的地方就是手肘；這份資料大約在 k=3~4。手肘法是啟發式，最終 k 要看業務能不能解釋每一群。\n");

    // ---------- 練習 2：群別當特徵 ----------
    println!("=== 練習 2：把「帳戶所屬群」one-hot 成 3 欄加進特徵 ===");
    let k = 3;
    let km = KMeans::params(k)
        .max_n_iterations(300)
        .tolerance(1e-5)
        .fit(&acct_ds)?;
    let clusters: Array1<usize> = km.predict(&acct_ds);
    let cluster_of: HashMap<u32, usize> =
        _ids.iter().copied().zip(clusters.iter().copied()).collect();
    let one_hot = |x: &Array2<f64>, ids: &[u64]| -> Array2<f64> {
        let mut extra = Array2::<f64>::zeros((x.nrows(), k));
        for (r, id) in ids.iter().enumerate() {
            let account = txs[(*id - 1) as usize].account;
            extra[[r, cluster_of[&account]]] = 1.0;
        }
        concatenate![Axis(1), x.view(), extra.view()]
    };
    let names_plus: Vec<&str> = FEATURE_NAMES
        .iter()
        .copied()
        .chain(["cluster_0", "cluster_1", "cluster_2"])
        .collect();

    let fit_lr = |xtr: Array2<f64>,
                  xte: Array2<f64>,
                  names: Vec<&str>|
     -> Result<(f64, Confusion), Box<dyn std::error::Error>> {
        let tr = Dataset::new(xtr, to_bool(&train.y)).with_feature_names(names.clone());
        let te = Dataset::new(xte, to_bool(&test.y)).with_feature_names(names);
        let sc = LinearScaler::standard().fit(&tr)?;
        let (tr, te) = (sc.transform(tr), sc.transform(te));
        let m = LogisticRegression::default()
            .max_iterations(500)
            .alpha(1e-3)
            .fit(&tr)?;
        let p = m.predict_probabilities(te.records()).mapv(|p| 1.0 - p);
        Ok((
            roc_auc(&test.y, &p),
            Confusion::from_scores(&test.y, &p, 0.5),
        ))
    };
    let (auc0, c0) = fit_lr(train.x.clone(), test.x.clone(), FEATURE_NAMES.to_vec())?;
    let (auc1, c1) = fit_lr(
        one_hot(&train.x, &train.ids),
        one_hot(&test.x, &test.ids),
        names_plus,
    )?;
    c0.report("原 9 特徵");
    c1.report("加群別 12 特徵");
    println!("AUC：{auc0:.3} → {auc1:.3}");
    println!("→ 注意這裡的群別是用「全部交易」算出來的，包含測試期間的資料，有輕微洩漏；嚴謹做法是只用訓練期間的交易分群。\n");

    // ---------- 練習 3：決策樹 ----------
    println!("=== 練習 3：決策樹 ===");
    let tr = Dataset::new(train.x.clone(), to_bool(&train.y).mapv(|b| b as usize))
        .with_feature_names(FEATURE_NAMES.to_vec());
    let te = Dataset::new(test.x.clone(), to_bool(&test.y).mapv(|b| b as usize))
        .with_feature_names(FEATURE_NAMES.to_vec());
    let tree = DecisionTree::params()
        .split_quality(SplitQuality::Gini)
        .max_depth(Some(5))
        .min_weight_leaf(5.0)
        .fit(&tr)?;
    let pred = tree.predict(&te);
    let cm = pred.confusion_matrix(&te)?;
    println!(
        "深度 ≤ 5 的樹：precision {:.3}  recall {:.3}  F1 {:.3}",
        cm.precision(),
        cm.recall(),
        cm.f1_score()
    );
    println!("特徵重要性（被用來切分的次數加權）：");
    let mut imp: Vec<(usize, f64)> = tree
        .feature_importance()
        .iter()
        .copied()
        .enumerate()
        .collect();
    imp.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap());
    for (i, v) in imp.iter().take(5) {
        println!("  {:<18} {:.3}", FEATURE_NAMES[*i], v);
    }
    println!(
        "→ 樹能表達「A 且 B」的交互作用（一條路徑就是一組 AND 條件），這是邏輯回歸做不到的。\n"
    );

    // ---------- 練習 4：PCA ----------
    println!("=== 練習 4：PCA 降到 2 維 ===");
    let ds = Dataset::new(feats.x.clone(), to_bool(&feats.y));
    let ds = LinearScaler::standard().fit(&ds)?.transform(ds);
    let pca = Pca::params(2).fit(&ds)?;
    let proj = pca.predict(&ds); // n × 2
                                 // 解釋變異比例 = 投影後各主成分的變異 / 原始總變異。標準化後每欄變異為 1，總變異 = 特徵數。
    let total_var = FEATURE_NAMES.len() as f64;
    let comp_var = proj.var_axis(Axis(0), 0.0);
    println!(
        "前兩個主成分各解釋 {:.1}% 與 {:.1}% 的總變異，合計 {:.1}%",
        comp_var[0] / total_var * 100.0,
        comp_var[1] / total_var * 100.0,
        (comp_var[0] + comp_var[1]) / total_var * 100.0
    );
    let fraud: Vec<usize> = feats
        .y
        .iter()
        .enumerate()
        .filter(|(_, &v)| v == 1.0)
        .map(|(i, _)| i)
        .collect();
    let normal: Vec<usize> = feats
        .y
        .iter()
        .enumerate()
        .filter(|(_, &v)| v == 0.0)
        .map(|(i, _)| i)
        .collect();
    let fm = proj.select(Axis(0), &fraud).mean_axis(Axis(0)).unwrap();
    let nm = proj.select(Axis(0), &normal).mean_axis(Axis(0)).unwrap();
    println!(
        "正常交易在 2D 的中心：({:.2}, {:.2})；詐欺的中心：({:.2}, {:.2})",
        nm[0], nm[1], fm[0], fm[1]
    );
    let path = std::env::temp_dir().join("ml03-pca.csv");
    let mut csv = String::from("pc1,pc2,is_fraud\n");
    for (i, row) in proj.rows().into_iter().enumerate() {
        csv.push_str(&format!(
            "{:.4},{:.4},{}\n",
            row[0], row[1], feats.y[i] as u8
        ));
    }
    std::fs::write(&path, csv)?;
    println!(
        "座標已存到 {}（可用試算表或 Python 畫散佈圖）",
        path.display()
    );
    println!("→ 兩群中心距離遠、但兩個主成分只解釋部分變異，代表 2D 投影看得出趨勢但會有重疊；用全部 9 維的模型才分得乾淨。");
    Ok(())
}

fn aggregate_by_account(
    txs: &[synth::LabeledTx],
    x: &Array2<f64>,
) -> (Array2<f64>, Vec<u32>, Vec<f64>) {
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
        m.row_mut(r)
            .assign(&x.select(Axis(0), rows).mean_axis(Axis(0)).unwrap());
        fraud.push(rows.iter().filter(|&&i| txs[i].is_fraud).count() as f64 / rows.len() as f64);
    }
    (m, ids, fraud)
}
