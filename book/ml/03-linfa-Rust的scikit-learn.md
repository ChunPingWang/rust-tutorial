# ML-3　linfa：Rust 的 scikit-learn

> **場景**：ML-2 的手寫模型證明了概念，但風控團隊想試更多演算法：「能不能把帳戶依行為自動分成幾類？不同類用不同門檻。」
> 每個演算法都自己寫不划算。linfa 是 Rust 的 scikit-learn：統一的 `fit`/`predict` 介面，十幾個演算法 crate。

## Why：為什麼需要一個框架

ML-2 的 60 行邏輯回歸是教學用的。生產環境要的是：

- **更好的最佳化器**：L-BFGS 比梯度下降收斂快十倍，不用調學習率
- **統一介面**：換演算法只改一行，前處理與評估程式碼不動
- **經過驗證的實作**：數值穩定性、邊界情況、效能

linfa 的設計刻意對齊 scikit-learn：`Dataset`（資料 + 標記）、`Fit` trait（訓練）、`Predict` trait（預測）、`Transformer` trait（前處理）。會 sklearn 的人幾乎可以直接對照。

## What：linfa 的核心抽象

### Dataset

```rust
use linfa::prelude::*;

let ds = Dataset::new(x, y)                     // x: Array2<f64>，y: Array1<L>
    .with_feature_names(FEATURE_NAMES.to_vec());
ds.records()   ds.targets()   ds.nsamples()   ds.nfeatures()
```

**標記的型別很重要**：分類演算法要求 target 是「類別」型別（`bool`、`usize`、`&str`，實作了 `Label` trait），不是 `f64`。
ML-2 用 `Array1<f64>` 存 0/1，這裡要轉成 `Array1<bool>`。這是 Rust 型別系統在幫你：回歸與分類的 target 本來就是不同的東西。

無標記的資料用 `DatasetBase::from(x)`（分群用）。

### 三個 trait

| trait | 方法 | 誰實作 |
|---|---|---|
| `Fit` | `.fit(&dataset) -> Result<FittedModel>` | 所有演算法的「參數」型別（`LogisticRegression`、`KMeans::params(k)`） |
| `Predict` | `.predict(&dataset)` 或 `.predict(&records)` | 訓練好的模型 |
| `Transformer` | `.transform(dataset)` | 前處理（標準化、PCA） |

訓練前與訓練後是**不同的型別**：`LogisticRegression` 是超參數設定，`.fit()` 回傳 `FittedLogisticRegression`。你不可能對沒訓練的模型呼叫 `predict`，編譯器不讓你。sklearn 裡這是執行期的 `NotFittedError`。這是第 9 章 typestate 模式的實際應用。

### 各演算法 crate

| crate | 內容 |
|---|---|
| `linfa-linear` | 線性回歸、GLM |
| `linfa-logistic` | 邏輯回歸（二元與多類） |
| `linfa-trees` | 決策樹 |
| `linfa-svm` | SVM |
| `linfa-bayes` | 樸素貝氏 |
| `linfa-clustering` | K-means、DBSCAN、GMM |
| `linfa-reduction` | PCA、擴散映射 |
| `linfa-preprocessing` | 標準化、正規化、計數向量 |
| `linfa-nn` | 最近鄰 |

每個是獨立 crate，只加你要的，編譯快。

## How：邏輯回歸三行，再加 K-means

完整程式碼在 `code/ml03-linfa/`。`cargo run --release`。

### 標準化：LinearScaler 取代手寫 Scaler

```rust
use linfa_preprocessing::linear_scaling::LinearScaler;

let scaler = LinearScaler::standard().fit(&train_ds)?;    // 只用訓練集
let train_ds = scaler.transform(train_ds);
let test_ds = scaler.transform(test_ds);
```

ML-1 的原則不變：fit 訓練集，transform 兩邊。差別是這次 `transform` 消耗 `Dataset` 的所有權（第 2 章），回傳新的。

### 邏輯回歸

```rust
use linfa_logistic::LogisticRegression;

let model = LogisticRegression::default()
    .max_iterations(500)
    .alpha(1e-3)             // L2 正則化強度
    .fit(&train_ds)?;

let pred: Array1<bool> = model.predict(&test_ds);
let cm = pred.confusion_matrix(&test_ds)?;
println!("precision {:.3} recall {:.3} F1 {:.3}", cm.precision(), cm.recall(), cm.f1_score());
```

```
classes    | true       | false
true       | 51         | 7
false      | 1          | 1141

precision 0.981  recall 0.879  F1 0.927  accuracy 0.993
AUC 0.996
```

和 ML-2 手寫版本（同樣不加類別權重）比：

```
手寫（pos_weight=1）   precision 1.000  recall 0.759  F1 0.863   AUC 0.993
linfa                  precision 0.981  recall 0.879  F1 0.927   AUC 0.996
```

AUC 幾乎一樣，linfa 的 F1 更好。差異來自 L-BFGS 收斂得更徹底，以及正則化的強度不同。權重方向完全一致：

```
特徵                 linfa    手寫
log_amount          +2.132  +0.622
is_withdraw         +4.815  +0.374
is_transfer         +4.071  +0.272
velocity_24h        +1.261  +0.425
```

linfa 的權重整體大 3 到 10 倍，因為 ML-2 的 L2 把權重壓得更小。同一個問題、同一個模型族、不同的正則化，得到「方向相同、尺度不同」的解。這是正常的。

### 一個真實的陷阱

```rust
let proba = model.predict_probabilities(test_ds.records());
```

第一次跑，AUC 是 **0.004**。模型顯然學對了（混淆矩陣很好），但機率是反的。
原因：`predict_probabilities` 回傳的是**排序後第一個類別**的機率。`bool` 排序 `false < true`，所以它給的是 P(正常)。權重的正負號也是相對於 `false`。

```rust
let proba = model.predict_probabilities(test_ds.records()).mapv(|p| 1.0 - p);   // 要 P(詐欺)
```

這類「哪個類別是正例」的約定每個框架都不同（sklearn 是 `classes_[1]`），**任何時候拿到機率，先用一個已知的樣本驗證方向**。程式碼裡保留了這個教訓。

### 自訂 threshold

`predict()` 固定用 0.5。ML-2 說過 threshold 是業務決定，所以要用機率：

```rust
for thr in [0.2, 0.5, 0.8] {
    Confusion::from_scores(&test.y, &proba, thr).report(..);
}
```

ML-2 寫的 `Confusion` 與 `roc_auc` 在這裡繼續用。自己寫過的評估工具不會因為換框架就作廢。

### K-means：帳戶行為分群

場景的第二個需求。先把每個帳戶的所有交易特徵取平均，得到「帳戶數 × 9」的矩陣（ML-1 練習 3），然後：

```rust
use linfa_clustering::KMeans;

let acct_ds = DatasetBase::from(account_matrix);
let acct_ds = LinearScaler::standard().fit(&acct_ds)?.transform(acct_ds);

let kmeans = KMeans::params(3).max_n_iterations(300).tolerance(1e-5).fit(&acct_ds)?;
let clusters: Array1<usize> = kmeans.predict(&acct_ds);
kmeans.centroids()      // 3 × 9，每群的中心
```

```
240 個帳戶分成 3 群：
  群 0： 78 個帳戶，平均詐欺率  0.6%，特色：hour_sin -1.07、hour_cos +0.84   （早上活躍）
  群 1：120 個帳戶，平均詐欺率  0.9%，特色：hour_cos -0.71、hour_sin +0.64   （下午活躍）
  群 2： 42 個帳戶，平均詐欺率 14.1%，特色：amount_to_balance +1.76、is_night +1.46
```

**分群完全沒有看標記**，卻把高詐欺率的帳戶分到了同一群：這群的特色是「常常大比例動用餘額、深夜活動」。
業務上的用法：群 2 的帳戶套用較低的詐欺 threshold（寧可多確認），群 0、1 套用較高的。或者把「所屬群」當成一個新特徵餵給分類模型。

K-means 需要你決定 k。常見做法是試 k = 2..10，畫 inertia（群內距離和）找「手肘」，或用 silhouette 分數。練習題會做。

## 對照 scikit-learn

| scikit-learn | linfa |
|---|---|
| `StandardScaler().fit(X)` | `LinearScaler::standard().fit(&ds)?` |
| `scaler.transform(X)` | `scaler.transform(ds)` |
| `LogisticRegression(C=1000).fit(X, y)` | `LogisticRegression::default().alpha(1e-3).fit(&ds)?` |
| `model.predict(X)` | `model.predict(&ds)` |
| `model.predict_proba(X)[:, 1]` | `model.predict_probabilities(ds.records())`（注意類別方向） |
| `confusion_matrix(y, pred)` | `pred.confusion_matrix(&ds)?` |
| `KMeans(n_clusters=3).fit(X)` | `KMeans::params(3).fit(&ds)?` |
| `kmeans.labels_` | `kmeans.predict(&ds)` |
| `kmeans.cluster_centers_` | `kmeans.centroids()` |
| `NotFittedError`（執行期） | 型別不同（編譯期） |

## 回顧

- linfa = `Dataset` + `Fit`/`Predict`/`Transformer` trait；訓練前後是不同型別，編譯器防止對未訓練的模型 predict
- 分類的 target 要是類別型別（`bool`/`usize`），不是 `f64`
- 拿到機率先驗證類別方向；`predict()` 固定 0.5，要調 threshold 就用機率
- K-means 不看標記也能分出高風險群；分群結果可以當特徵或當分層門檻的依據
- 自己寫的評估工具（`Confusion`、`roc_auc`）跨框架繼續用

## 練習

1. 對 k = 2..8 各跑一次 K-means，印出 `kmeans.inertia()`，找手肘。
2. 把「帳戶所屬群」加成第 10 個特徵（one-hot 成 3 欄），重訓邏輯回歸。F1 有沒有進步？
3. 用 `linfa-trees` 的 `DecisionTree` 訓練同一份資料，比較 F1，並印出樹的結構（`.export_to_tikz()` 或手動走訪）。樹能學到交互作用，看看它抓到了哪些 ML-2 漏網的案例。
4. 用 `linfa-reduction` 的 PCA 把 9 維特徵降到 2 維，把詐欺與正常交易的座標印出來（或存成 CSV 用別的工具畫），看它們在 2D 平面上分得開不開。

## 下一章

邏輯回歸與決策樹都是「淺」模型。ML-2 漏網的案例需要學到特徵的交互作用，而且未來特徵可能從 9 個變成 90 個。
下一章用 candle 訓練一個小型神經網路，並把權重存成可以部署的檔案。
