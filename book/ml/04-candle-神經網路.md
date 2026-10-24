# ML-4　candle：神經網路與可部署的權重

> **場景**：ML-2 漏網的詐欺是「白天、金額不大、但連續且對象陌生」這種**組合**。線性模型學不到組合。
> 風控團隊也預告：特徵會從 9 個長到幾十個（裝置指紋、地理位置、商家類別）。
> 需要一個能學非線性、能擴展、而且**訓練完能存成檔案丟進 API** 的模型。

## Why：為什麼是 candle

Rust 的深度學習框架有三個主要選擇：

| | candle | burn | tch-rs |
|---|---|---|---|
| 出品 | Hugging Face | 社群 | 社群 |
| 定位 | **推論為主**，也能訓練 | 訓練為主，後端可換 | PyTorch C++ 綏定 |
| 相依 | 純 Rust（可選 CUDA/Metal） | 純 Rust | 需要 libtorch（數百 MB） |
| API 風格 | 像 PyTorch | 像 PyTorch，更多泛型 | 就是 PyTorch |
| 生態 | 大量現成模型（LLM、Whisper、Stable Diffusion） | 成長中 | 可直接載 PyTorch 模型 |

對帳本的場景：模型小、推論要嵌進 API、不想帶 libtorch，candle 最合適。它的 `Tensor` API 與 PyTorch 幾乎一一對應，而且 Hugging Face 用它在生產環境跑 LLM 推論，成熟度夠。

## What：Tensor、自動微分、模組、優化器

### Tensor

```rust
let dev = Device::Cpu;                                      // 或 Device::new_cuda(0)? / new_metal(0)?
let x = Tensor::from_vec(data_f32, (n, 9), &dev)?;          // 建立
x.shape()  x.dim(0)?  x.dtype()                             // (n, 9)、n、F32
x.narrow(0, start, len)?                                    // 取一段列（零複製）
x.matmul(&w)?  x.relu()?  x.exp()?  x.log()?  x.abs()?       // 運算，全部回傳 Result
(&a + &b)?  (&a * 2.0)?  a.maximum(&b)?                     // 運算子也回傳 Result
x.sum_all()?  x.mean(D::Minus1)?  x.squeeze(D::Minus1)?
x.to_vec1::<f32>()?  x.to_scalar::<f32>()?                   // 拿回 Rust 值
```

**每個運算都回傳 `Result`**。形狀不合、dtype 不合、裝置不合都是 `Err`，不是 panic。寫起來到處是 `?`，但這是第 6 章的哲學：失敗是值。

### 自動微分

candle 的 `Tensor` 會記錄運算圖。`loss.backward()?` 算出所有參與運算的 `Var`（可訓練參數）的梯度。你不需要手推梯度，ML-2 那條 `Xᵀ(p−y)/n` 框架幫你算。

### VarMap 與 VarBuilder

```rust
let varmap = VarMap::new();                                    // 持有所有可訓練參數
let vs = VarBuilder::from_varmap(&varmap, DType::F32, &dev);   // 負責建立並命名
let l1 = linear(9, 16, vs.pp("l1"))?;                          // 建一層，參數叫 l1.weight、l1.bias
```

`pp` = push prefix，命名空間。**名字就是存檔格式**：`varmap.save("x.safetensors")` 存的是 `{"l1.weight": ..., "l1.bias": ...}`，載入時靠名字對回去。所以載入前要先用同樣的結構建好模型。

### 模型 = 一個 struct + forward

```rust
struct Mlp { l1: Linear, l2: Linear, l3: Linear }

impl Mlp {
    fn forward(&self, x: &Tensor) -> Result<Tensor> {
        let x = self.l1.forward(x)?.relu()?;
        let x = self.l2.forward(&x)?.relu()?;
        Ok(self.l3.forward(&x)?.squeeze(D::Minus1)?)
    }
}
```

9 → 16 → 8 → 1，兩層 ReLU。ReLU（`max(0, x)`）是非線性的來源：沒有它，三層線性層疊起來還是一個線性模型。

### 優化器

```rust
let mut opt = AdamW::new(varmap.all_vars(), ParamsAdamW { lr: 5e-3, weight_decay: 1e-4, ..Default::default() })?;
// 訓練迴圈裡：
opt.backward_step(&loss)?;      // backward + 更新參數，一行
```

AdamW 是目前的預設選擇：對學習率不敏感、自帶動量。`weight_decay` 是 L2 正則化的現代版。ML-2 手寫的 `w -= lr * grad` 是 SGD，AdamW 是它的進階。

### safetensors

Hugging Face 設計的權重檔格式：一個 JSON header 描述每個 tensor 的名字、dtype、shape、位移，後面接原始 bytes。
取代 PyTorch 的 pickle 格式，因為 **pickle 可以執行任意程式碼**，載入不信任的模型檔等於執行不信任的程式。safetensors 只是資料，載入是安全的。它也支援 mmap 零複製載入，大模型秒開。

## How：訓練、存檔、載回、量延遲

完整程式碼在 `code/ml04-candle/`。第一次 `cargo build --release` 要幾分鐘。

### 資料進 Tensor

```rust
fn to_tensor(x: &Array2<f64>, dev: &Device) -> Result<Tensor> {
    let (n, d) = x.dim();
    let data: Vec<f32> = x.iter().map(|&v| v as f32).collect();
    Ok(Tensor::from_vec(data, (n, d), dev)?)
}
```

ndarray 用 f64，candle 用 f32（CPU 上快很多，精度對這個問題夠）。ML-1 的 `Scaler` 繼續用，標準化後再轉 Tensor。

### 加權損失

candle 有 `binary_cross_entropy_with_logit`，但沒有正例加權。自己寫 10 行：

```rust
fn weighted_bce_with_logits(logits: &Tensor, y: &Tensor, pos_weight: f64) -> Result<Tensor> {
    // 數值穩定版：max(z,0) − z·y + ln(1 + e^{−|z|})
    let relu = logits.maximum(&logits.zeros_like()?)?;
    let zy = (logits * y)?;
    let log_term = (logits.abs()?.neg()?.exp()? + 1.0)?.log()?;
    let per_sample = ((relu - zy)? + log_term)?;
    let w = ((y * (pos_weight - 1.0))? + 1.0)?;          // y=1 → pos_weight，y=0 → 1
    Ok(((per_sample * &w)?.sum_all()? / w.sum_all()?)?)
}
```

對 logit 而不是機率算損失，是為了數值穩定：`ln(sigmoid(z))` 在 z 很負時會變成 `ln(0)`。這個公式在數學上等價但不會爆。所有框架都這樣做。

### 訓練迴圈

```rust
for epoch in 0..60 {
    for start in (0..n).step_by(256) {
        let xb = xtr.narrow(0, start, len)?;
        let yb = ytr.narrow(0, start, len)?;
        let logits = model.forward(&xb)?;
        let loss = weighted_bce_with_logits(&logits, &yb, 5.0)?;
        opt.backward_step(&loss)?;
    }
}
```

```
參數量：305
epoch   0  loss 0.9003
epoch  10  loss 0.0513
epoch  30  loss 0.0348
epoch  59  loss 0.0278
訓練時間：94 ms
```

305 個參數、4,800 筆資料、60 個 epoch，CPU 上 94 毫秒。深度學習不一定要 GPU，小問題用小模型。

### 結果：誠實地比

```
AUC  MLP：0.988
MLP thr=0.5    precision 0.914  recall 0.914  F1 0.914
```

對照 ML-2 邏輯回歸：AUC 0.995、F1 0.881。**MLP 的 AUC 反而略低，F1 略高。** 兩個模型在這份資料上差不多。

這是重要的一課：合成資料的詐欺模式本來就偏線性（金額大、深夜、提款，各自加分），邏輯回歸已經夠好。神經網路的優勢在真實資料裡「組合才可疑」的模式。而它的代價是真的：

- 更難解釋。邏輯回歸的權重表可以直接給監管機關看，MLP 不行（需要 SHAP 之類的事後解釋工具）
- 更容易過擬合，需要更多資料
- 多了一堆超參數（層數、寬度、學習率、epoch）

**先用簡單模型建立基準，複雜模型要證明自己比基準好才值得上。** 這份資料上，邏輯回歸就是正確答案。但 ML-5 會部署 MLP，因為它的部署流程（權重檔 + 結構程式碼）是通用的，換成大模型也一樣。

### 存檔與載回

```rust
varmap.save("model/fraud_mlp.safetensors")?;                    // 1,628 bytes
std::fs::write("model/scaler.json", serde_json::to_string_pretty(&scaler_file)?)?;

// 載回：先建同樣的結構，再 load
let mut varmap2 = VarMap::new();
let model2 = Mlp::new(VarBuilder::from_varmap(&varmap2, DType::F32, &dev))?;
varmap2.load("model/fraud_mlp.safetensors")?;
```

```
載回來的模型與原模型預測最大差異：0.00e0
```

**Scaler 必須一起存。** 模型學的是標準化空間，沒有 mean/std 就無法把新交易轉進去。`scaler.json` 還存了特徵名稱與順序，部署時用它驗證輸入的欄位對不對。

### 推論延遲

```
單筆推論平均：3.1 µs
```

三微秒。第 15 章 API 的一次 HTTP 往返是毫秒級，模型打分的成本可以忽略。這就是 Rust 做推論的理由：模型嵌進業務服務的同一個行程，沒有跨服務呼叫，沒有 Python runtime。

## 對照 PyTorch

```python
class Mlp(nn.Module):
    def __init__(self):
        super().__init__()
        self.l1 = nn.Linear(9, 16); self.l2 = nn.Linear(16, 8); self.l3 = nn.Linear(8, 1)
    def forward(self, x):
        return self.l3(F.relu(self.l2(F.relu(self.l1(x))))).squeeze(-1)

opt = torch.optim.AdamW(model.parameters(), lr=5e-3, weight_decay=1e-4)
loss = F.binary_cross_entropy_with_logits(logits, y, pos_weight=torch.tensor(5.0))
loss.backward(); opt.step(); opt.zero_grad()
torch.save(model.state_dict(), "m.pt")       # candle: varmap.save()
```

結構一比一。差別：candle 每個運算回傳 `Result`、`Tensor` 預設不可變（運算產生新 Tensor）、沒有 Python 的動態性（模型結構在編譯期固定）。

另一個常見流程：**Python 訓練、Rust 推論**。PyTorch 的 `safetensors.torch.save_file(model.state_dict(), "m.safetensors")` 存出來的檔案，candle 可以直接 `varmap.load()`，只要 Rust 這邊的結構與命名一致。這是生產環境最常見的分工。

## 回顧

- candle：`Tensor` 運算回傳 `Result`；`VarMap` 持參數、`VarBuilder` 命名；`Module`/`forward` 定義結構；`AdamW::backward_step` 一行訓練
- 對 logit 算損失保證數值穩定；正例加權處理不平衡
- 小模型 CPU 訓練不到 100 ms；推論 3 µs
- 先建簡單模型的基準，複雜模型要證明自己更好。這份資料上邏輯回歸已足夠
- safetensors 存權重（安全、可 mmap）；Scaler 參數與特徵順序一起存，部署時缺一不可
- Python 訓練、Rust 推論是常見分工，safetensors 是橋

## 練習

1. 把隱藏層改成 64 → 32，epoch 改成 200。測試集 F1 有進步嗎？訓練集呢？（分別算兩者，差距大 = 過擬合。）
2. 加 dropout（`candle_nn::ops::dropout`，只在訓練時用）。注意 forward 需要知道現在是訓練還是推論。
3. 實作早停（early stopping）：從訓練集切 10% 當驗證集，每個 epoch 算驗證損失，連續 5 個 epoch 沒進步就停。
4. 把 `Device::Cpu` 改成 `Device::new_metal(0)?`（Mac）或 `new_cuda(0)?`（NVIDIA），需要在 `Cargo.toml` 開啟對應 feature。對這麼小的模型，GPU 有比較快嗎？為什麼？
5. 用 Python + PyTorch 訓練同樣結構的模型，用 `safetensors` 存檔，讓 ML-4 的 `model2` 載入它。確認 `varmap2.load` 對命名的要求。

## 下一章

權重檔有了。把它載進第 15 章的 axum API：每筆進來的交易，過帳前先打分，分數高的掛起等人工審核。
