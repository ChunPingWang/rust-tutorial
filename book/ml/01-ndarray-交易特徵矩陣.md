# ML-1　ndarray：把交易變成特徵矩陣

> **場景**：風控團隊給了兩年份的交易，其中 3% 被標記為詐欺。第一步不是選模型，是回答：**「一筆交易，用哪幾個數字描述？」**
> 模型只看數字。這一步做得好，簡單模型就夠用；做得差，再深的神經網路也救不回來。

## Why：為什麼需要矩陣庫

機器學習的所有運算都是矩陣運算：n 筆交易 × 9 個特徵是一個 n×9 的矩陣，標準化是「每一欄減平均除標準差」，模型預測是「矩陣乘向量」。
用 `Vec<Vec<f64>>` 寫得出來，但：

- 每一個運算都要自己寫雙重迴圈
- 沒有「軸」的概念，「每一欄的平均」和「每一列的平均」要寫兩套
- 沒有廣播（broadcast）：`矩陣 - 向量` 要手動展開
- 記憶體不連續，效能差

`ndarray` 是 Rust 的 NumPy：n 維陣列、軸運算、廣播、切片視圖、BLAS 加速的矩陣乘法。後面所有 ML 套件（linfa、candle 的資料輸入）都接它。

## What：ndarray 的核心概念

### 型別

```rust
Array1<f64>      // 一維，向量
Array2<f64>      // 二維，矩陣
ArrayD<f64>      // 動態維度
ArrayView2<f64>  // 借用的視圖，不擁有資料（切片的結果）
```

`Array2<f64>` 是 `ArrayBase<OwnedRepr<f64>, Ix2>` 的別名。看文件時會看到這個長名字，別被嚇到。

### 建立

```rust
Array2::<f64>::zeros((n, 9))
Array2::from_shape_vec((2, 3), vec![1., 2., 3., 4., 5., 6.])?
Array1::from(vec![10., 20., 30.])
array![[1., 2.], [3., 4.]]     // 巨集
```

### 索引與切片

```rust
x[[i, j]]                   // 單一元素（注意是雙中括號）
x.row(i)  x.column(j)       // 視圖
x.slice(s![0..3, 1..])      // s! 巨集：列 0~2，欄 1 到最後。回傳視圖，零複製
x.slice(s![.., 0])          // 第 0 欄
x.select(Axis(0), &[1, 5, 9])   // 挑選特定列，複製
```

### 軸運算

`Axis(0)` 是列的方向（往下），`Axis(1)` 是欄的方向（往右）。「對 Axis(0) 做平均」= 把所有列壓成一列 = **每一欄的平均**：

```rust
x.mean_axis(Axis(0))         // 每欄平均，長度 = 欄數
x.sum_axis(Axis(1))          // 每列加總，長度 = 列數
x.std_axis(Axis(0), 0.0)     // 每欄標準差（0.0 = 母體，1.0 = 樣本）
x.fold_axis(Axis(0), f64::INFINITY, |a, b| a.min(*b))   // 每欄最小值
```

這和 NumPy 的 `axis=0` 完全一致。

### 廣播與逐元素運算

```rust
&x * 2.0 + 1.0               // 標量：每個元素
&x + &v                      // v 是 Array1（長度 = 欄數）：加到每一列
(&x - &mean) / &std          // 標準化，一行
x.mapv(f64::ln)              // 逐元素函式，產生新陣列
x.mapv_inplace(|v| v * 2.0)  // 原地
&a * &b                      // 逐元素乘（不是矩陣乘！）
a.dot(&b)                    // 矩陣乘法
x.t()                        // 轉置，零成本視圖
```

**注意 `&`**：`&x + &v` 借用兩者，產生新陣列；`x + v` 會消耗 `x`（所有權，第 2 章）。運算子重載加所有權，在 ndarray 裡會頻繁遇到。

### 什麼時候用 ndarray 之外的東西

- 線性代數（解方程、特徵值、SVD）：`ndarray-linalg`（需要 LAPACK）或 `nalgebra`
- 統計（分位數、直方圖）：`ndarray-stats`
- 隨機陣列：`ndarray-rand`
- DataFrame（混合型別、分組聚合、讀 CSV/Parquet）：`polars`。ndarray 是純數字矩陣，不是表格

## How：帳本交易 → 9 個特徵

完整程式碼在 `code/ml01-ndarray/`。`cargo run --release`。

### 合成資料

真實的詐欺標記不能公開，`synth.rs` 用統計上像真的方式產生：每個帳戶有自己的典型金額（對數常態分佈）與活躍時段；詐欺交易金額是典型值的 3 到 15 倍、七成在深夜、幾乎都是提款或轉帳、傾向連續發生、三成會把帳戶掏空。
重點是**沒有一個特徵能單獨判斷**：白天也有詐欺，深夜也有正常交易。

### 特徵設計

```rust
pub const FEATURE_NAMES: [&str; 9] = [
    "log_amount",        // ln(金額)：跨數量級的量取對數
    "amount_vs_acct",    // 相對該帳戶歷史平均的 z 分數：對「這個人」來說大不大
    "amount_to_balance", // 金額 / 餘額：是不是在掏空
    "hour_sin", "hour_cos", // 時段的圓形編碼：23 點和 0 點要很近
    "is_night",          // 0~5 點
    "is_withdraw", "is_transfer",
    "velocity_24h",      // 同帳戶過去 24 小時交易筆數
];
```

每一條都是把風控人員的直覺翻成數字：

- **為什麼取對數**：100 元和 1,000 元的差距，與 10,000 和 100,000 的差距，在業務上是同一種「十倍」。線性尺度會讓大額主導一切
- **為什麼要「相對帳戶」的特徵**：5 萬元對月薪族是異常，對企業帳戶是日常。絕對金額不夠，要看偏離「這個人的常態」多少
- **為什麼時段要用 sin/cos**：如果直接用 0–23，模型會認為 23 點和 0 點差 23 小時。圓形編碼讓它們相鄰
- **為什麼要 velocity**：盜用者會在被發現前盡快把錢轉走，短時間連續交易是強訊號

這些特徵需要**按時間順序**計算（帳戶歷史、過去 24 小時），所以 `build()` 是一個 `for` 迴圈逐筆累積，用 `HashMap` 存每個帳戶的歷史（第 7 章），結果寫進 `x.row_mut(i)`。

### 看懂你的矩陣

```
特徵                        平均       標準差        最小        最大
log_amount             7.451     1.365     4.079    15.418
amount_vs_acct         0.099     1.676    -7.774    30.729
amount_to_balance      0.016     0.110     0.000     1.000
is_night               0.044     0.205     0.000     1.000
velocity_24h           1.682     1.431     0.000    13.000
```

各欄尺度差很多：`velocity` 到 13，`is_night` 在 0/1。模型（尤其是用距離或梯度的）會被大尺度的欄主導。所以要**標準化**。

### Scaler：fit 與 transform 分開

```rust
pub struct Scaler { mean: Array1<f64>, std: Array1<f64> }

impl Scaler {
    pub fn fit(x: &Array2<f64>) -> Scaler {
        let mean = x.mean_axis(Axis(0)).unwrap();
        let std = x.std_axis(Axis(0), 0.0).mapv(|s| if s < 1e-12 { 1.0 } else { s });
        Scaler { mean, std }
    }
    pub fn transform(&self, x: &Array2<f64>) -> Array2<f64> {
        (x - &self.mean) / &self.std      // 廣播
    }
}
```

為什麼分成兩步？因為**推論時必須用訓練時的 mean/std**。模型學的是「標準化後的空間」，上線時每筆新交易要用同一組參數轉換，否則數字的意義就變了。`Scaler` 是模型的一部分，要跟權重一起存檔（ML-5）。

### 哪些特徵有鑑別力

標準化後，比較詐欺與正常交易的每欄平均：

```
特徵（標準化後）          正常平均    詐欺平均     差距
is_night               -0.128      3.531   +3.659
amount_to_balance      -0.117      3.224   +3.341
log_amount             -0.093      2.567   +2.660
amount_vs_acct         -0.085      2.333   +2.417
velocity_24h           -0.042      1.147   +1.189
is_transfer            -0.016      0.439   +0.455
```

差距大的特徵有用。但注意：差距 +3.6 的 `is_night`，正常交易也有 4.4% 在深夜。**用單一特徵當規則會誤殺很多人**，這就是為什麼要用模型把多個特徵合起來看。

布林遮罩選列：ndarray 沒有 `x[mask]` 語法，用 `select(Axis(0), &indices)`。

### 相關係數矩陣

標準化後 `corr = Xᵀ X / n`：

```rust
let corr: Array2<f64> = xs.t().dot(&xs) / n;
```

一行。`t()` 是零成本視圖，`dot` 是 BLAS 矩陣乘法。`log_amount` 與 `amount_to_balance` 相關 0.56，`is_night` 與 `hour_cos` 高度相關（兩者都在描述時段）。高相關的特徵對線性模型是重複資訊，對樹模型無所謂。

### 時間切分

```rust
pub fn split(f: &Features, train_ratio: f64) -> (Features, Features)
```

**前 80% 時間訓練、後 20% 測試，不隨機切。** 詐欺偵測是用過去預測未來。隨機切會讓同一個帳戶被盜的四筆連續交易，兩筆在訓練集兩筆在測試集，模型「看過答案」，測試分數虛高。這叫資料洩漏（leakage），是 ML 專案最常見的錯誤。

同理，`Scaler::fit` 只能用訓練集。

## 對照 NumPy

| NumPy | ndarray |
|---|---|
| `np.zeros((n, 9))` | `Array2::<f64>::zeros((n, 9))` |
| `x[i, j]` | `x[[i, j]]` |
| `x[0:3, 1:]` | `x.slice(s![0..3, 1..])` |
| `x.mean(axis=0)` | `x.mean_axis(Axis(0)).unwrap()` |
| `(x - mu) / sd` | `(&x - &mu) / &sd` |
| `x @ w` | `x.dot(&w)` |
| `x * y`（逐元素） | `&x * &y` |
| `x.T` | `x.t()` |
| `np.log(x)` | `x.mapv(f64::ln)` |
| `x[mask]` | `x.select(Axis(0), &idx)` |

## 回顧

- 特徵工程是領域知識翻成數字：取對數、相對個人常態、圓形編碼時段、velocity
- `Axis(0)` 壓列得每欄統計；廣播讓標準化一行寫完；`&` 避免消耗所有權
- `Scaler` 的 fit/transform 分開，推論時用訓練的參數，要跟模型一起存
- 時間切分不隨機切；Scaler 只用訓練集 fit。資料洩漏是最常見的 ML 錯誤

## 練習

1. 加第 10 個特徵 `is_new_counterparty`（轉帳對象是否第一次出現）。需要在 `build()` 裡多維護一個 `HashMap<u32, HashSet<u32>>`。
2. 把 `velocity_24h` 改成 `velocity_1h`，比較兩者對詐欺/正常的平均差距。哪個更有鑑別力？
3. 用 `x.select` 與 `mean_axis` 算出「每個帳戶的平均特徵向量」，得到一個 帳戶數 × 9 的矩陣。這是 ML-3 分群的輸入。
4. 把 `Scaler` 改成 min-max 標準化（壓到 0..1）。想一想：對有極端值的 `amount_vs_acct`（最大 30），哪種標準化比較好？

## 下一章

有了矩陣，先不用任何套件，自己寫兩個模型：沒有標記時的異常偵測，有標記時的邏輯回歸。寫過一次，之後用 linfa 或 candle 才知道它們在做什麼。
