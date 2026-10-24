//! 第 9 章命令列入口：示範 trait 的使用。

use ledger::report::{self, CsvReport, JsonReport, ReportFormat, TextReport};
use ledger::{save_and_verify, InMemoryStorage, Ledger, Money, Storage, TextFileStorage, Transaction, TxKind};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut ledger = Ledger::new();
    ledger.open(1001, "王小明", Money::from_yuan(50_000))?;
    ledger.open(1002, "李小華", Money::from_yuan(2_000))?;
    ledger.open(1003, "張大同", Money::from_yuan(800))?;
    let rejected = ledger.apply_all(sample_transactions());
    println!("拒絕 {} 筆", rejected.len());

    // Display：Money 現在可以直接印，也支援寬度對齊
    println!("總餘額：{} 元", ledger.total_balance());
    println!(
        "對齊示範：[{:>12}] [{:<12}]",
        ledger.total_balance(),
        Money::from_cents(-1234)
    );

    // 運算子
    let a = Money::from_yuan(100);
    let b = Money::from_cents(50);
    println!(
        "{a} + {b} = {}，{a} - {b} = {}，-{a} = {}",
        a + b,
        a - b,
        -a
    );

    // 泛型：靜態分派
    println!("\n--- TextReport ---");
    print!("{}", report::render_to_string(&TextReport, &ledger));
    println!(
        "--- CsvReport → {} ---",
        report::file_name(&CsvReport { with_header: true }, "balances")
    );
    print!(
        "{}",
        report::render_to_string(&CsvReport { with_header: true }, &ledger)
    );

    // trait object：動態分派，格式清單可以在執行期決定
    let formats: Vec<Box<dyn ReportFormat>> = vec![
        Box::new(TextReport),
        Box::new(CsvReport { with_header: false }),
        Box::new(JsonReport), // 練習 1
    ];
    print!("{}", JsonReport.render(&ledger));
    let rendered = report::render_all(&formats, &ledger);
    println!(
        "\n動態分派產生了 {} 份報表，第二份有 {} 行",
        rendered.len(),
        rendered[1].lines().count()
    );

    // Storage trait：兩種後端，同一段使用程式碼
    let path = std::env::temp_dir().join("ledger-ch09.txt");
    let mut backends: Vec<Box<dyn Storage>> = vec![
        Box::new(InMemoryStorage::default()),
        Box::new(TextFileStorage::new(&path)),
    ];
    for backend in backends.iter_mut() {
        backend.save(&ledger)?;
        let restored = backend.load()?;
        println!(
            "{}：存檔後讀回，與原帳本相同？{}",
            backend.describe(),
            restored == ledger
        );
    }

    // 看一下文字檔長什麼樣
    println!("\n{} 的前三行：", path.display());
    for line in std::fs::read_to_string(&path)?.lines().take(3) {
        println!("  {line}");
    }

    // 練習 2、3、4
    println!("\n{} × 3 = {}", Money::from_yuan(100), Money::from_yuan(100) * 3);
    // let nonsense = Money::from_yuan(1) * Money::from_yuan(1); // E0369: cannot multiply `Money` by `Money`
    println!("{:?}", "X:1001:1002".parse::<TxKind>());
    println!("{}", "Q:1".parse::<TxKind>().unwrap_err());
    let mut mem = InMemoryStorage::default();
    println!("save_and_verify：{}", save_and_verify(&mut mem, &ledger)?);

    // 泛型函式搭配 trait bound：對任何可比較、可複製的東西找最大值
    let amounts: Vec<Money> = ledger.history().iter().map(|t| t.amount).collect();
    println!("\n最大金額：{:?}", largest(&amounts));
    println!("最大 ID：{:?}", largest(&[3u64, 9, 4]));
    println!("空切片：{:?}", largest::<Money>(&[]));

    Ok(())
}

/// 泛型函式：T 必須能比較（PartialOrd）且能複製（Copy）
fn largest<T: PartialOrd + Copy>(items: &[T]) -> Option<T> {
    let mut iter = items.iter();
    let mut best = *iter.next()?;
    for &item in iter {
        if item > best {
            best = item;
        }
    }
    Some(best)
}

fn sample_transactions() -> Vec<Transaction> {
    (1..=12u64)
        .map(|i| {
            let kind = match i % 4 {
                0 => TxKind::Deposit {
                    account: 1001 + (i as u32 % 3),
                },
                1 => TxKind::Withdraw { account: 1002 },
                2 => TxKind::Transfer {
                    from: 1001,
                    to: 1003,
                },
                _ => TxKind::Withdraw { account: 1002 },
            };
            Transaction {
                id: i,
                day: (i as u32 + 2) / 3,
                kind,
                amount: Money::from_cents(i as i64 * 10_000),
                memo: format!("測試交易 {i}"),
            }
        })
        .collect()
}
