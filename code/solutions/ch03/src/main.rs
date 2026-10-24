//! 第 3 章練習解答

fn main() {
    let memos = vec![
        String::from("十月薪資"),
        String::from("ATM 提款"),
        String::from("網路轉帳"),
    ];
    // 練習 1
    println!("{:?}", find(&memos, "ATM")); // Some(1)
    println!("{:?}", find(&memos, "利息")); // None

    // 練習 2
    let mut amounts = vec![100_000, -30_000, 50_000];
    apply_interest(&mut amounts, 150); // 1.5%
    println!("{amounts:?}"); // [101500, -30000, 50750]

    // 練習 3：兩個借用錯誤，打開註解各自觀察
    let mut v = vec![1, 2, 3];
    // let r1 = &mut v;
    // let r2 = &mut v;        // error[E0499]: cannot borrow `v` as mutable more than once at a time
    // r1.push(4); r2.push(5);
    //
    // let first = &v[0];
    // v.push(4);              // error[E0502]: cannot borrow `v` as mutable because it is also borrowed as immutable
    // println!("{first}");
    v.push(4);
    println!("{v:?}");
}

/// 練習 1：回傳第一筆包含關鍵字的索引。
/// Option<usize> 表達「可能找不到」；參數 &[String] 與 &str 都是借用。
fn find(memos: &[String], keyword: &str) -> Option<usize> {
    for (i, m) in memos.iter().enumerate() {
        if m.contains(keyword) {
            return Some(i);
        }
    }
    None
    // 迭代器版本（第 7 章）：memos.iter().position(|m| m.contains(keyword))
}

/// 練習 2：&mut [i64] 切片就夠，因為不改長度。rate_bp 是基點，1 bp = 0.01% = 1/10000。
fn apply_interest(amounts: &mut [i64], rate_bp: i64) {
    for a in amounts.iter_mut() {
        if *a > 0 {
            *a += *a * rate_bp / 10_000;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn find_works() {
        let m = vec!["abc".to_string(), "def".to_string()];
        assert_eq!(find(&m, "e"), Some(1));
        assert_eq!(find(&m, "z"), None);
    }

    #[test]
    fn interest_only_on_positive() {
        let mut a = [10_000, -10_000, 0];
        apply_interest(&mut a, 100);
        assert_eq!(a, [10_100, -10_000, 0]);
    }
}
