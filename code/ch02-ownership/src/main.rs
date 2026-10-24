//! 第 2 章：所有權
//!
//! 需求：一筆交易紀錄建立後，要「交給」帳本保管。
//! 交出去之後，原本的地方就不該再動它，否則會出現兩份帳。

fn main() {
    // --- Copy 型別：數字是可以複製的 ---
    let amount: i64 = 25_050;
    let amount_copy = amount; // i64 實作了 Copy，這裡是複製
    println!("amount = {amount}, amount_copy = {amount_copy}"); // 兩個都能用

    // --- Move 型別：String 不能隱式複製 ---
    let memo = String::from("十月薪資");
    let memo_in_ledger = memo; // 所有權「移動」到 memo_in_ledger
                               // println!("{memo}"); // 編譯錯誤：borrow of moved value: `memo`
    println!("帳本中的備註：{memo_in_ledger}");

    // --- 真的要兩份？明確 clone ---
    let memo2 = String::from("十月獎金");
    let memo2_backup = memo2.clone(); // 明確地複製，成本看得見
    println!("原始：{memo2}，備份：{memo2_backup}");

    // --- 傳進函式也是移動 ---
    let description = String::from("ATM 提款");
    record(description); // 所有權交給 record
                         // println!("{description}"); // 編譯錯誤：description 已經被移走

    // --- 想要回來？用回傳值 ---
    let description = String::from("網路轉帳");
    let description = record_and_return(description);
    println!("還拿得回來：{description}");

    // --- 作用域結束，值被釋放（Drop）---
    {
        let temp = String::from("暫時的備註");
        println!("區塊內：{temp}");
    } // temp 在這裡被釋放，記憶體歸還，不需要我們手動 free

    // --- &str 與 String ---
    let literal: &str = "字面值，存在程式的唯讀區"; // 借用，不擁有
    let owned: String = literal.to_string(); // 配置堆積記憶體，擁有
    println!("{literal} / {owned}");
}

/// 接收交易描述的所有權，記錄後就讓它消失
fn record(description: String) {
    println!("已記錄：{description}");
} // description 在這裡被 drop

/// 接收所有權，用完再還回去
fn record_and_return(description: String) -> String {
    println!("已記錄：{description}");
    description
}
