//! 第 11 章的 bin 與第 10 章相同；這章的重點在 `cargo test`。
//! 執行：
//!   cargo test                  全部（單元 + 整合 + 文件測試）
//!   cargo test money            名稱含 money 的測試
//!   cargo test --lib            只跑單元測試
//!   cargo test --test ledger_flow   只跑某個整合測試檔
//!   cargo test --doc            只跑文件測試
//!   cargo test -- --nocapture   顯示測試中的 println!

fn main() {
    println!("這一章請執行 cargo test");
}
