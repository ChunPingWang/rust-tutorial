# 附錄 C　延伸學習

讀完 16 章，你已經能用 Rust 寫出一個有測試、有 API、能部署的服務。接下來往哪走，取決於你要用 Rust 做什麼。

## 先補強的基礎

本書刻意略過或簡化的主題，實務上很快會遇到：

| 主題 | 為什麼需要 | 從哪開始 |
|---|---|---|
| **錯誤處理的進階模式** | 錯誤的分層、`Backtrace`、`source()` 鏈 | `std::error::Error` 文件；`error-stack` crate |
| **Iterator trait 的手動實作** | 為自己的型別寫 `next()`，與第 10 章的 `MemoParser` 練習 | The Book 第 13 章 |
| **巨集** | `macro_rules!` 宣告式巨集；過程式巨集（derive 怎麼寫的） | The Little Book of Rust Macros |
| **Pin、Unpin、手寫 Future** | 理解 async 底層；寫自己的執行器或 stream | Asynchronous Programming in Rust（async book） |
| **FFI** | 呼叫 C 函式庫、被 C/Python 呼叫 | The Rustonomicon；`bindgen`、`cbindgen`、`PyO3` |
| **泛型的進階** | 關聯型別 vs 泛型參數、GAT、`PhantomData`、typestate 模式 | Rust for Rustaceans |
| **unsafe 的正確用法** | 寫自己的資料結構、效能關鍵路徑 | The Rustonomicon；`Miri` 工具 |

## 官方資源

- **The Rust Programming Language（The Book）**：官方教材，語法導向，是本書的對照補充。繁體中文翻譯版存在
- **Rust by Example**：每個概念一個可執行範例
- **The Rustonomicon**：unsafe Rust 的黑暗藝術
- **std 文件**：`std::collections`、`std::iter`、`std::sync` 三個模組的文件值得完整讀一遍，很多你以為要自己寫的功能都已經有了
- **Rust API Guidelines**：寫函式庫時的命名與設計慣例
- **This Week in Rust**：每週電子報，追生態變化

## 幾本書

- **Rust for Rustaceans**（Jon Gjengset）：讀完本書之後最該讀的書。講型別系統、trait 設計、unsafe、async 的中階內容
- **Programming Rust**（Blandy, Orendorff, Tindall）：最完整的參考書
- **Zero To Production In Rust**（Luca Palmieri）：用 actix-web/sqlx 從零建一個生產級後端，是本書第 15、16 章的深化
- **Rust Atomics and Locks**（Mara Bos）：第 14 章的底層，原子操作與記憶體順序

## 依方向

### 後端服務

本書第 15 章的延伸。要補的：

- **資料庫**：`sqlx`（編譯期檢查 SQL）、`sea-orm`、`diesel`。把 `Storage` trait 實作成 `PostgresStorage`
- **HTTP 細節**：中介層（tower middleware）、認證、限流、CORS、graceful shutdown
- **gRPC**：`tonic`
- **訊息佇列**：`rdkafka`、`lapin`（RabbitMQ）
- **可觀測性**：`tracing-opentelemetry`、`metrics-exporter-prometheus`

### 命令列工具

本書第 12 章的延伸。Rust 是這個領域的強項（ripgrep、fd、bat、exa 都是 Rust）：

- `clap` 的進階（shell 補全、子命令巢狀、環境變數）
- `indicatif`（進度條）、`console`、`dialoguer`（互動）
- `ratatui`（終端機 UI）
- 跨平台編譯與發佈：`cross`、`cargo-dist`

### 系統程式與嵌入式

- **The Embedded Rust Book**、`embassy`（嵌入式 async）
- 作業系統：Philipp Oppermann 的 *Writing an OS in Rust*
- 網路協定：`tokio` 的底層、`smoltcp`
- 資料庫引擎：讀 `sled`、`redb` 的原始碼

### WebAssembly

- `wasm-bindgen`、`wasm-pack`：Rust 編成 Wasm 給瀏覽器用
- `Leptos`、`Dioxus`、`Yew`：Rust 寫前端
- `wasmtime`：在 Rust 裡執行 Wasm（插件系統）

### 機器學習與資料

Rust 在 ML 的定位不是取代 Python 做研究，而是**推論部署、資料前處理、高效能計算核心**。本教材有一條完整的學習路徑，沿用同一個帳本場景做交易異常偵測：

→ **[機器學習學習路徑](ml/00-路徑總覽.md)**

路徑涵蓋 `ndarray`（矩陣）、從零實作異常偵測與邏輯回歸、`linfa`（Rust 的 scikit-learn）、`candle`（Hugging Face 的深度學習框架）、以及把模型整合進第 15 章的 API。

其他方向：`polars`（DataFrame，比 pandas 快一個數量級）、`arrow`/`datafusion`（查詢引擎）、`burn`（另一個深度學習框架）、`ort`（ONNX Runtime 綁定）。

### 遊戲

- `bevy`：ECS 架構的遊戲引擎，Rust 社群最活躍的專案之一
- `wgpu`：跨平台 GPU API

## 練習的方向

讀書不如動手。幾個規模適中、能練到本書所有概念的專案：

1. **把 `ledger` 的 `Storage` 接上真的資料庫**（sqlx + SQLite）。你會遇到：async trait、連線池、交易（transaction）、migration
2. **寫一個 `ripgrep` 的簡化版**：遞迴走目錄、正規表達式、多執行緒、彩色輸出。練到迭代器、channel、`Path`、錯誤處理
3. **寫一個 Redis 協定的子集伺服器**：tokio、位元組解析、`HashMap` 加過期時間、並行。tokio 官方教材就是這個題目
4. **寫一個 JSON 解析器**（不用 serde）：遞迴下降、enum 表示 JSON 值、`Box` 遞迴、錯誤定位。練到第 5、6、10、13 章
5. **為 `ledger` 加一個 TUI**（ratatui）：即時顯示餘額、輸入交易。練到事件迴圈與狀態管理

## 社群

- **users.rust-lang.org**：問問題，回答品質高
- **r/rust**：新聞與討論
- **Rust 台灣社群**（Rust Taiwan）：Telegram / Facebook 社團，有定期聚會
- **This Week in Rust 的 Call for Participation**：找開源專案貢獻的入口
- **Rustlings**：小練習題，適合複習

## 最後

Rust 的學習曲線在前三章最陡，你已經過了。之後的每個概念（trait、生命週期、async）都是在同一套「所有權 + 型別系統」的基礎上疊加。
卡住的時候回到那兩個問題：**所有權去哪了？誰借了誰？**

編譯器不是敵人。它是一個嚴格但永遠在線的 code reviewer，而且它說的每一句話都有理由。
