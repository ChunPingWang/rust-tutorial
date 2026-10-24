# 練習解答

每一章練習的參考解答。每份解答都有對應的可編譯專案在 `code/solutions/`，程式碼經過 `cargo test` 或實際執行驗證，文中的數字是實跑結果。

先自己做再看解答。卡住時先看「思路」那一段，不要直接看程式碼。

## 主線

| 章 | 解答 | 可編譯專案 |
|---|---|---|
| 1 | [千分位、apply、溢位](ch01-解答.md) | `code/solutions/ch01` |
| 2 | [make_memo、tuple 的 Copy、還回兩個 String](ch02-解答.md) | `code/solutions/ch02` |
| 3 | [find、apply_interest、E0499 與 E0502](ch03-解答.md) | `code/solutions/ch03` |
| 4 | [describe、Ledger::find_account、具名欄位 Money](ch04-解答.md) | `code/solutions/ch04` |
| 5 | [新增 Fee 變體、largest_account、Option 方法鏈](ch05-解答.md) | `code/solutions/ch05` |
| 6 | [AccountFrozen、ParseError、main 回傳 Result](ch06-解答.md) | `code/solutions/ch06` |
| 7 | [monthly_statement、balance_check、for 取代 partition、chunks](ch07-解答.md) | `code/solutions/ch07` |
| 8 | [第二個 bin、新式模組佈局、try_from_cents、文件](ch08-解答.md) | `code/solutions/ch08` |
| 9 | [JsonReport、Mul\<i64\>、FromStr、save_and_verify](ch09-解答.md) | `code/solutions/ch09` |
| 10 | [filter_kind、split_memo、生命週期標註、Iterator](ch10-解答.md) | `code/solutions/ch10` |
| 11 | [風控與分頁測試、突變實驗、proptest](ch11-解答.md) | `code/solutions/ch11` |
| 12 | [export 子命令、千分位解析、--verbose、cargo tree](ch12-解答.md) | `code/solutions/ch12` |
| 13 | [修正稽核、Rule 序列化、RefCell panic、Cow](ch13-解答.md) | `code/solutions/ch13` |
| 14 | [可重現分片、rayon、死鎖、加速比](ch14-解答.md) | `code/solutions/ch14` |
| 15 | [對帳單端點、oneshot 測試、阻塞實驗、背景存檔](ch15-解答.md) | `code/solutions/ch15` |
| 16 | [release 大小、cargo audit、Docker、clippy pedantic、指標](ch16-解答.md) | `code/solutions/ch16` |

## 機器學習路徑

| 章 | 解答 | 可編譯專案 |
|---|---|---|
| ML-1 | [新特徵、時間窗口、帳戶矩陣、min-max](ml01-解答.md) | `code/solutions/ml01` |
| ML-2 | [pos_weight、學習率、mini-batch、L1、PR 曲線](ml02-解答.md) | `code/solutions/ml02` |
| ML-3 | [手肘法、群別特徵、決策樹、PCA](ml03-解答.md) | `code/solutions/ml03` |
| ML-4 | [過擬合、dropout、早停、GPU、safetensors 命名](ml04-解答.md) | `code/solutions/ml04` |
| ML-5 | [整合進帳本 API、影子模型、線性模型、指標、測試](ml05-解答.md) | `code/solutions/ml05` |

## 執行

```bash
cd code/solutions/ch11 && cargo test
cd code/solutions/ch14 && cargo run --release --bin settle_solutions
cd code/solutions/ml05 && cargo test --release
```

第 8 章以後的解答專案是該章 `ledger` crate 的完整副本加上練習的修改，可以用 `diff -r code/ch09-traits code/solutions/ch09` 看改了哪些地方。
