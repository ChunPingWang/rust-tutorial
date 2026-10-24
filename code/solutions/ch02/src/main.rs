//! 第 2 章練習解答

fn main() {
    // 練習 1
    let memo = make_memo("存款", 25_050);
    println!("{memo}");

    // 練習 2：(u32, String) 不是 Copy，因為 String 不是 Copy。
    // tuple 只有在「所有元素都是 Copy」時才是 Copy。
    let holder = (1001u32, String::from("王小明"));
    let moved = holder;
    // println!("{:?}", holder); // error[E0382]: borrow of moved value: `holder`
    println!("{:?}", moved);

    // 對照：(u32, &str) 是 Copy，因為 &str 是 Copy（它只是借用）
    let holder2 = (1002u32, "李小華");
    let copied = holder2;
    println!("{:?} {:?}", holder2, copied); // 兩個都能用

    // 練習 3：兩個 String 都還回來，呼叫端要解構 tuple 再重新綁定。很囉嗦，這就是借用的理由。
    let a = String::from("十月薪資");
    let b = String::from("ATM 提款");
    let (a, b) = record_two(a, b);
    println!("{a} / {b}");
}

/// 練習 1：參數 &str 因為只是讀；回傳 String 因為是新產生的資料，呼叫端要擁有它
fn make_memo(kind: &str, amount: i64) -> String {
    format!("{kind} {}.{:02} 元", amount / 100, amount % 100)
}

/// 練習 3
fn record_two(first: String, second: String) -> (String, String) {
    println!("已記錄：{first}、{second}");
    (first, second)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn memo_format() {
        assert_eq!(make_memo("存款", 25_050), "存款 250.50 元");
    }

    #[test]
    fn record_two_returns_both() {
        let (a, b) = record_two("a".into(), "b".into());
        assert_eq!((a.as_str(), b.as_str()), ("a", "b"));
    }
}
