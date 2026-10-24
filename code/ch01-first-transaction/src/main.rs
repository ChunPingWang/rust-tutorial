//! 第 1 章：印出第一筆交易
//!
//! 需求：系統收到一筆存款，印出「誰、存了多少、餘額變成多少」。

fn main() {
    // 帳戶資料：先用最簡單的變數表示
    let account_id: u32 = 1001;
    let owner = "王小明"; // &str：字串切片，第 2 章會細談
    let mut balance_cents: i64 = 150_000; // 1,500.00 元，以「分」為單位

    // 一筆存款
    let deposit_cents: i64 = 25_050; // 250.50 元

    // 入帳
    balance_cents += deposit_cents;

    println!("帳戶 #{account_id}（{owner}）");
    println!("存入：{}", format_money(deposit_cents));
    println!("餘額：{}", format_money(balance_cents));

    // 嘗試一筆提款
    let withdraw_cents: i64 = 200_000; // 2,000.00 元
    if withdraw_cents > balance_cents {
        println!("提款 {} 失敗：餘額不足", format_money(withdraw_cents));
    } else {
        balance_cents -= withdraw_cents;
        println!("提款成功，餘額：{}", format_money(balance_cents));
    }

    // 浮點數為什麼不能存錢：親眼看一次
    let a: f64 = 0.1;
    let b: f64 = 0.2;
    println!("0.1 + 0.2 == 0.3 ? {}", a + b == 0.3);
}

/// 把「分」格式化成「元.分」的字串，例如 25050 -> "250.50"
fn format_money(cents: i64) -> String {
    let yuan = cents / 100;
    let fen = (cents % 100).abs();
    format!("{yuan}.{fen:02} 元")
}
