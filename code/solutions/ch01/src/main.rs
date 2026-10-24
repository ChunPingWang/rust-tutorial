//! 第 1 章練習解答

fn main() {
    // 練習 1：千分位
    println!("{}", format_money(175_050)); // 1,750.50 元
    println!("{}", format_money(-123_456_789)); // -1,234,567.89 元
    println!("{}", format_money(5)); // 0.05 元

    // 練習 2：apply
    let mut balance: i64 = 150_000;
    balance = apply(balance, 25_050);
    balance = apply(balance, -200_000);
    println!("餘額：{}", format_money(balance));

    // 練習 3：溢位。debug 模式下這行會 panic：attempt to add with overflow
    // release 模式下會繞回成 -2147483648。兩種都不是帳本要的，所以用 checked_add。
    let x: i32 = 2_147_483_647;
    match x.checked_add(1) {
        Some(v) => println!("{v}"),
        None => println!("i32 溢位被 checked_add 攔下"),
    }
}

/// 練習 1：每三位插一個逗號。先處理整數部分的字串，從尾端數。
fn format_money(cents: i64) -> String {
    let sign = if cents < 0 { "-" } else { "" };
    let yuan = (cents / 100).abs();
    let fen = (cents % 100).abs();
    let digits = yuan.to_string();
    let mut grouped = String::new();
    for (i, ch) in digits.chars().enumerate() {
        // 從左邊數：剩下的位數是 3 的倍數且不是開頭，就先放逗號
        if i > 0 && (digits.len() - i) % 3 == 0 {
            grouped.push(',');
        }
        grouped.push(ch);
    }
    format!("{sign}{grouped}.{fen:02} 元")
}

/// 練習 2：正數存款、負數提款
fn apply(balance: i64, delta: i64) -> i64 {
    if delta < 0 && -delta > balance {
        println!("提款 {} 失敗：餘額不足", format_money(-delta));
        return balance;
    }
    balance + delta
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn thousands_separator() {
        assert_eq!(format_money(175_050), "1,750.50 元");
        assert_eq!(format_money(100_000_000), "1,000,000.00 元");
        assert_eq!(format_money(99_999), "999.99 元");
        assert_eq!(format_money(-5), "-0.05 元");
    }

    #[test]
    fn apply_rejects_overdraft() {
        assert_eq!(apply(100, -200), 100);
        assert_eq!(apply(100, -100), 0);
        assert_eq!(apply(100, 50), 150);
    }
}
