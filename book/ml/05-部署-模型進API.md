# ML-5　部署：模型進 API

> **場景**：模型訓練好了。風控團隊的要求：「每筆交易過帳前打分。分數高的直接擋，中間的掛起等人工審核，低的放行。
> 打分不能讓過帳變慢，而且每個決定都要能追溯：當時模型看到了什麼數字？」
> 這一章把 ML-4 的權重檔載進一個 axum 服務。這是整條路徑的目的地，也是 Rust 在 ML 裡最有價值的位置。

## Why：為什麼推論要用 Rust

Python 訓練、Python 部署是最常見的做法，但在過帳路徑上會遇到：

- **延遲**：Python 服務的一次推論往往是毫秒級（框架開銷、GIL、序列化），過帳本身只要微秒
- **資源**：每個 Python 推論服務要帶整個 runtime 與框架，記憶體以 GB 計
- **多一跳**：帳本服務呼叫打分服務，多一次網路往返、多一個故障點
- **部署**：Python 環境的相依管理是運維的惡夢

Rust 的做法：模型嵌進業務服務的**同一個行程**。ML-4 量過單筆推論 3 微秒，比 HTTP 往返小三個數量級。權重檔 1.6KB，執行檔單一檔案。

## What：部署時要對的三件事

### 1. 結構與命名要一致

```rust
// ML-5 的 model.rs：和 ML-4 **完全相同**
struct Mlp { l1: Linear, l2: Linear, l3: Linear }
fn new(vs: VarBuilder, n: usize) -> Result<Self> {
    Ok(Self { l1: linear(n, 16, vs.pp("l1"))?, l2: linear(16, 8, vs.pp("l2"))?, l3: linear(8, 1, vs.pp("l3"))? })
}
```

safetensors 只存參數，不存結構。載入端要先建一樣的結構、一樣的名字，`varmap.load()` 才對得上。差一個字就是錯誤（而且是明確的錯誤，不是靜默載入垃圾）。

實務上訓練與推論的模型定義應該放在同一個 crate 裡共用（第 8 章的 lib），本路徑為了讓每章獨立而複製了一份。

### 2. 特徵計算要一致：training-serving skew

這是部署 ML 最常見的失敗原因。訓練時 `log_amount = ln(金額元)`，部署時寫成 `ln(金額分)`，差了 `ln 100 ≈ 4.6`，模型的輸入全部位移，表現莫名其妙變差，而且**不會報錯**。

```rust
fn to_features(req: &ScoreRequest) -> Result<RawFeatures, ApiError> {
    let log_amount = amount.ln();                                      // 必須和 features.rs 的 build() 一致
    v.insert("amount_vs_acct", (log_amount - req.account_mean_log_amount) / req.account_sd_log_amount.max(0.1));
    v.insert("hour_sin", hour_rad.sin());
    v.insert("is_night", if req.hour < 5 { 1.0 } else { 0.0 });       // 同樣的 5
    // ...
}
```

防範方法：

- 特徵計算的程式碼**共用**（同一個函式訓練與推論都呼叫），不要寫兩份
- `scaler.json` 存特徵名稱與順序，載入時用名稱對應而不是位置
- 缺特徵是**錯誤**，不是預設 0：`raw.values.get(name).with_context(|| format!("缺少特徵 {name}"))?`
- 回應裡附上模型實際吃到的特徵值，讓呼叫端與稽核能核對

### 3. Scaler 要一起載

```rust
pub fn load(dir: &Path) -> Result<Self> {
    let scaler: ScalerFile = serde_json::from_str(&fs::read_to_string(dir.join("scaler.json"))?)?;
    let mut varmap = VarMap::new();
    let model = Mlp::new(VarBuilder::from_varmap(&varmap, DType::F32, &device), n)?;
    varmap.load(dir.join("fraud_mlp.safetensors"))?;
    // ...
}
```

模型 + Scaler + 特徵順序 = 一個部署單位。少一個就不能用。正式專案會把三者打包成一個目錄並加版本號。

## How：打分服務

完整程式碼在 `code/ml05-serving/`。`cargo run --release` 啟動，`cargo run --release -- bench` 離線量吞吐。

### Scorer：與 HTTP 無關的核心

```rust
pub struct Scorer {
    model: Mlp,
    feature_names: Vec<String>,
    mean: Vec<f32>,
    std: Vec<f32>,
    device: Device,
    _varmap: VarMap,     // 必須活著，Linear 裡的 Tensor 才有效
}

impl Scorer {
    pub fn load(dir: &Path) -> Result<Self>
    pub fn score(&self, raw: &RawFeatures) -> Result<f32>
    pub fn score_batch(&self, rows: &[RawFeatures]) -> Result<Vec<f32>>
}
```

`model.rs` 不知道 HTTP 的存在，可以單獨測試。三個單元測試：明顯詐欺分數 > 0.9、明顯正常 < 0.1、缺特徵是錯誤、批次與單筆結果相同。**模型也要有測試**：至少確保「載入成功」且「對已知樣本的方向正確」，否則換一版權重檔壞掉了沒人知道。

`_varmap` 欄位值得一提：`Linear` 內部的 `Tensor` 借自 `VarMap`。如果 `VarMap` 被 drop，參數就沒了。這是第 10 章「結構體裡的引用」在 candle 裡的形式，用「把擁有者一起存進 struct」解決。

### 兩段式決策

```rust
struct Thresholds { review: f64, block: f64 }    // 0.3 / 0.9

let decision = if p >= block { "block" } else if p >= review { "review" } else { "allow" };
```

ML-2 說 threshold 是業務決定。這裡是兩個：高於 0.9 直接擋（precision 在這裡接近 1.0，誤殺極少），0.3 到 0.9 掛起人工審核（recall 高但 precision 只有七成，人來判斷），低於 0.3 放行。
門檻從設定來，不寫死在程式裡。`GET /model` 回報目前的門檻與特徵清單，方便確認線上跑的是哪一版。

### 執行

```
$ curl -X POST localhost:3100/score -d '{"account":1001,"kind":"withdraw","amount":"48000.00","hour":2,
    "balance_before":"48000.00","account_mean_log_amount":7.2,"account_sd_log_amount":0.5,"txs_last_24h":4}'
{"account":1001,"fraud_probability":1.0,"decision":"block",
 "features":{"log_amount":10.78,"amount_vs_acct":7.16,"amount_to_balance":1.0,"is_night":1.0,"velocity_24h":4.0,...},
 "latency_us":134}

$ # 白天小額存款
{"account":1002,"fraud_probability":0.000008,"decision":"allow", ..., "latency_us":52}

$ # 晚上十點、金額是帳戶平均的三倍、轉帳
{"account":1003,"fraud_probability":0.761,"decision":"review", ..., "latency_us":174}

$ # 金額錯誤
{"error":"金額格式錯誤：\"abc\""}   [400]
```

三種決定各一個案例。`latency_us` 是從收到請求到算完的時間：**幾十微秒**，含特徵計算與推論。回應裡的 `features` 讓審核人員看到「模型為什麼這樣判」：帳戶 1001 那筆，金額是帳戶平均的 7 個標準差、把餘額全部提走、深夜、24 小時內第四筆。

### 推論要不要 `spawn_blocking`？

```rust
async fn score(State(state): State<AppState>, Json(req): Json<ScoreRequest>) -> ... {
    let p = state.scorer.score(&raw)?;      // 直接在 async handler 裡算
}
```

第 15 章說 CPU 密集工作不要放在 async 執行緒上。這裡例外：3 微秒的推論比一次 `.await` 的排程開銷還小，直接算反而快。
**判斷標準是絕對時間**：低於 100 微秒直接算；毫秒級以上（大模型、LLM）用 `tokio::task::spawn_blocking` 丟到專用執行緒池，否則一個請求會卡住同一執行緒上的幾百個其他請求。

`Scorer` 放在 `Arc` 裡跨請求共享（第 14 章）。它只有 `&self` 方法，天然 `Sync`，不需要鎖。模型載入時是唯一的寫入，之後全是讀。

### 吞吐量

```
$ cargo run --release -- bench
10000 筆
逐筆：50 ms（5.0 µs/筆）
批次： 1 ms（0.14 µs/筆）
```

批次推論快 35 倍：一次大矩陣乘法比一萬次小的高效得多（記憶體存取模式、SIMD）。
線上 API 一筆一筆來，用 `score`；夜間重新評估全部歷史交易（例如模型換版後回測），用 `score_batch`。

### 模型版本與回滾

模型是會變的部署產物，和程式碼一樣需要版本控制：

- `model/` 目錄加版本：`model/v3/fraud_mlp.safetensors`，環境變數 `FRAUD_MODEL_DIR` 指定
- `GET /model` 回報版本與訓練日期
- 換版時先 shadow：新模型打分但不決策，記錄下來與舊模型比，確認分佈合理再切換
- 每個決定記錄 `(交易 ID, 模型版本, 機率, 特徵)`，事後可以重放

## 其他部署路線

本路徑用「Rust 訓練 + Rust 推論 + candle」，是最單純的。實務上還有幾條：

| 路線 | 做法 | 適合 |
|---|---|---|
| **Python 訓練 → safetensors → candle** | PyTorch `save_file(state_dict)`，Rust 端定義同結構載入 | 小到中型模型，想在 Rust 服務裡零相依推論。最常見 |
| **Python 訓練 → ONNX → `ort`** | `torch.onnx.export`，Rust 用 ONNX Runtime 綁定 | 任何框架、任何結構，不用在 Rust 重寫模型定義。帶 C++ 相依 |
| **Python 訓練 → ONNX → `tract`** | 同上，但純 Rust 推論引擎 | 嵌入式、Wasm、不能帶 C++ 的環境 |
| **scikit-learn → 手動匯出係數** | 邏輯回歸只有 w 和 b，存成 JSON，Rust 10 行算 σ(x·w+b) | 線性模型。最簡單、最快、最好解釋 |
| **gRPC / HTTP 呼叫 Python 服務** | Rust 業務服務呼叫獨立的模型服務 | 大模型、GPU、模型團隊與業務團隊分離 |

對帳本：ML-2 已經證明邏輯回歸夠用，**最務實的生產方案是最後第二行**：係數存 JSON，連 candle 都不用。本路徑用 MLP 是為了讓部署流程能擴展到任何大小的模型。

## 回顧整條路徑

| 章 | 做了什麼 | 關鍵教訓 |
|---|---|---|
| ML-1 | 交易 → 9 個特徵 | 特徵是領域知識；Scaler 只用訓練集；時間切分 |
| ML-2 | 手寫異常偵測與邏輯回歸 | 準確率無意義；threshold 是業務決定；權重可解釋 |
| ML-3 | linfa | 統一介面；驗證機率的類別方向；分群能找出高風險群 |
| ML-4 | candle MLP | 簡單模型先建基準；safetensors + Scaler 一起存 |
| ML-5 | 嵌進 API | 結構/特徵/Scaler 三者一致；微秒推論；每個決定可追溯 |

Rust 在這條路徑裡的角色很清楚：**不是研究工具，是把模型變成可靠服務的工具**。所有權讓模型與 Scaler 的生命週期明確，型別系統讓缺特徵成為錯誤而不是靜默的 0，`Result` 讓每個失敗點可見，單一執行檔讓部署簡單。這些和第 1 到 16 章教的是同一件事。

## 練習

1. 把 ML-5 的打分整合進第 15 章的 `server.rs`：`POST /transactions` 過帳前先算特徵（帳戶歷史統計從 `Ledger::history()` 算）、打分，`block` 的交易回 403 並記錄原因，`review` 的交易存進一個待審核清單。
2. 實作 shadow mode：環境變數指定第二個模型目錄，每筆請求兩個模型都打分，回應用舊模型，日誌記錄兩者差異。
3. 用 Python + scikit-learn 訓練邏輯回歸，把 `coef_` 與 `intercept_` 存成 JSON，寫一個 `LinearScorer` 實作同樣的 `score` 介面（提示：第 9 章的 trait）。比較它與 MLP 的延遲。
4. 加 Prometheus 指標：請求數（按 decision 分）、延遲直方圖、機率分佈直方圖。機率分佈突然改變是「資料漂移」的早期警訊。
5. 寫一個 `#[tokio::test]`，用 `tower::ServiceExt::oneshot` 打 `/score`，驗證三種 decision 各一個案例。
