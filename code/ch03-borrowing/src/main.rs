//! 第 3 章：借用
//!
//! 需求：帳本是一串交易備註與金額。要能「查詢」總額、「列印」明細，
//! 也要能「入帳」新交易。查詢不該搬走帳本，入帳要能修改帳本。

fn main() {
    // 帳本：先用兩個平行的 Vec 表示（第 4 章會改成結構體）
    let mut memos: Vec<String> = vec![String::from("十月薪資"), String::from("ATM 提款")];
    let mut amounts: Vec<i64> = vec![5_000_000, -300_000];

    // --- 不可變借用：唸出來給人看，不需要所有權 ---
    print_ledger(&memos, &amounts);
    let total = sum(&amounts);
    println!("目前餘額：{}", format_money(total));
    // memos 和 amounts 還在，因為剛剛只是借出去

    // --- 可變借用：入帳要修改帳本 ---
    post(&mut memos, &mut amounts, "網路轉帳", -120_050);
    print_ledger(&memos, &amounts);

    // --- 同時存在多個不可變借用：沒問題 ---
    let first = &memos[0];
    let second = &memos[1];
    println!("前兩筆：{first} / {second}");

    // --- 可變借用期間不能有其他借用 ---
    let biggest = largest_memo(&memos);
    println!("最長的備註：{biggest}");
    // 下面這行如果放在 biggest 還會被使用之前，就會編譯錯誤：
    // memos.push(String::from("新的"));  // cannot borrow `memos` as mutable because it is also borrowed as immutable
    // 把 push 放在 biggest 最後一次使用之後就沒事（非詞法生命週期，NLL）
    memos.push(String::from("利息"));
    amounts.push(1_234);

    // --- 切片：借用一段連續範圍 ---
    let recent: &[i64] = &amounts[1..]; // 第 2 筆到最後
    println!(
        "最近 {} 筆的小計：{}",
        recent.len(),
        format_money(sum(recent))
    );

    // 字串切片同理
    let memo = &memos[0];
    let prefix: &str = &memo[..6]; // "十月" 兩個中文字各 3 bytes
    println!("備註前綴：{prefix}");
}

/// 借用兩個 Vec 唸出來。參數型別用 &[T]（切片）而不是 &Vec<T>，更通用。
fn print_ledger(memos: &[String], amounts: &[i64]) {
    println!("--- 帳本（{} 筆）---", memos.len());
    for i in 0..memos.len() {
        println!("{:<12} {:>12}", memos[i], format_money(amounts[i]));
    }
}

/// 加總。只需要讀，所以借用。
fn sum(amounts: &[i64]) -> i64 {
    let mut total = 0;
    for a in amounts {
        // a 的型別是 &i64，* 解引用取值；對 Copy 型別直接相加也行，Rust 會自動處理
        total += *a;
    }
    total
}

/// 入帳。需要修改，所以可變借用。memo 只是讀，用 &str。
fn post(memos: &mut Vec<String>, amounts: &mut Vec<i64>, memo: &str, amount: i64) {
    memos.push(memo.to_string()); // 從 &str 建一個自己擁有的 String 放進去
    amounts.push(amount);
}

/// 回傳借用：回傳的 &String 借用自參數 memos。第 10 章會解釋為什麼這裡不用寫生命週期。
fn largest_memo(memos: &[String]) -> &String {
    let mut best = &memos[0];
    for m in memos {
        if m.len() > best.len() {
            best = m;
        }
    }
    best
}

fn format_money(cents: i64) -> String {
    let sign = if cents < 0 { "-" } else { "" };
    let yuan = (cents / 100).abs();
    let fen = (cents % 100).abs();
    format!("{sign}{yuan}.{fen:02} 元")
}
