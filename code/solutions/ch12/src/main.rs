//! `ledger` 命令列工具。
//!
//! ```text
//! ledger import --accounts data/accounts.csv --transactions data/transactions.csv --out ledger.json
//! ledger report --ledger ledger.json --format text
//! ledger statement --ledger ledger.json 1001 --from 2026-10-01 --to 2026-10-31
//! ```

use std::path::PathBuf;

use anyhow::{bail, Context, Result};
use chrono::NaiveDate;
use clap::{Parser, Subcommand, ValueEnum};
use serde::Serialize;

use ledger::report::{self, CsvReport, ReportFormat, TextReport};
use ledger::{import, JsonFileStorage, Ledger, Storage, TxKind};

/// 小額支付帳本工具
#[derive(Parser)]
#[command(name = "ledger", version, about)]
struct Cli {
    /// 練習 3：全域選項，放在任何子命令前後都可以
    #[arg(long, global = true)]
    verbose: bool,

    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// 從 CSV 匯入帳戶與交易，過帳後存成 JSON
    Import {
        /// 帳戶 CSV（id,owner,balance）
        #[arg(long)]
        accounts: PathBuf,
        /// 交易 CSV（id,date,type,account,from,to,amount,memo）
        #[arg(long)]
        transactions: PathBuf,
        /// 輸出的帳本 JSON
        #[arg(long, default_value = "ledger.json")]
        out: PathBuf,
    },
    /// 產生報表
    Report {
        #[arg(long, default_value = "ledger.json")]
        ledger: PathBuf,
        #[arg(long, value_enum, default_value_t = Format::Text)]
        format: Format,
    },
    /// 練習 1：把交易歷史匯出成 CSV
    Export {
        #[arg(long, default_value = "ledger.json")]
        ledger: PathBuf,
        #[arg(long, default_value = "history.csv")]
        out: PathBuf,
    },
    /// 某帳戶的對帳單
    Statement {
        #[arg(long, default_value = "ledger.json")]
        ledger: PathBuf,
        /// 帳戶 ID
        account: u32,
        #[arg(long)]
        from: NaiveDate,
        #[arg(long)]
        to: NaiveDate,
    },
}

#[derive(Clone, Copy, ValueEnum)]
enum Format {
    Text,
    Csv,
}

fn main() -> Result<()> {
    let cli = Cli::parse();
    match cli.command {
        Command::Import {
            accounts,
            transactions,
            out,
        } => cmd_import(&accounts, &transactions, &out, cli.verbose),
        Command::Report { ledger, format } => cmd_report(&ledger, format),
        Command::Export { ledger, out } => cmd_export(&ledger, &out),
        Command::Statement {
            ledger,
            account,
            from,
            to,
        } => cmd_statement(&ledger, account, from, to),
    }
}

fn load_ledger(path: &PathBuf) -> Result<Ledger> {
    // with_context：在底層錯誤外面包一層「我當時在做什麼」
    JsonFileStorage::new(path)
        .load()
        .with_context(|| format!("載入帳本 {} 失敗", path.display()))
}

fn cmd_import(
    accounts: &PathBuf,
    transactions: &PathBuf,
    out: &PathBuf,
    verbose: bool,
) -> Result<()> {
    let account_rows = import::read_accounts(accounts).context("讀取帳戶 CSV")?;
    let txs = import::read_transactions(transactions).context("讀取交易 CSV")?;

    let mut ledger = Ledger::new();
    for row in &account_rows {
        ledger.open(row.id, &row.owner, row.balance)?; // LedgerError 自動轉成 anyhow::Error
    }
    let total = txs.len();
    let rejected = ledger.apply_all(txs);
    if verbose {
        for tx in ledger.history() {
            println!("  成功 {tx}");
        }
    }

    println!(
        "帳戶 {} 個，交易 {} 筆，成功 {} 筆，拒絕 {} 筆",
        account_rows.len(),
        total,
        total - rejected.len(),
        rejected.len()
    );
    for (id, err) in &rejected {
        eprintln!("  拒絕 #{id}：{err}");
    }

    JsonFileStorage::new(out)
        .save(&ledger)
        .with_context(|| format!("寫入 {}", out.display()))?;
    println!("已存檔：{}", out.display());
    Ok(())
}

fn cmd_report(path: &PathBuf, format: Format) -> Result<()> {
    let ledger = load_ledger(path)?;
    let formatter: Box<dyn ReportFormat> = match format {
        Format::Text => Box::new(TextReport),
        Format::Csv => Box::new(CsvReport { with_header: true }),
    };
    print!("{}", formatter.render(&ledger));
    if matches!(format, Format::Text) {
        let flagged = report::rapid_withdrawals(&ledger);
        if !flagged.is_empty() {
            println!("⚠ 連續提款警示帳戶：{flagged:?}");
        }
    }
    Ok(())
}

fn cmd_statement(path: &PathBuf, account: u32, from: NaiveDate, to: NaiveDate) -> Result<()> {
    if from > to {
        bail!("--from ({from}) 不能晚於 --to ({to})");
    }
    let ledger = load_ledger(path)?;
    let stmt = ledger.statement(account, from, to)?;
    println!(
        "帳戶 #{} {} 對帳單 {} ~ {}",
        stmt.account.id, stmt.account.owner, from, to
    );
    for tx in &stmt.entries {
        println!("  {tx}");
    }
    println!(
        "共 {} 筆，淨變動 {} 元，目前餘額 {} 元",
        stmt.entries.len(),
        stmt.net_change(),
        stmt.account.balance()
    );
    Ok(())
}

/// 練習 1：平面的中介型別。csv crate 不支援 serde(flatten) 的 enum，所以把 TxKind 攤成三個欄位。
#[derive(Serialize)]
struct HistoryRow<'a> {
    id: u64,
    date: chrono::NaiveDate,
    #[serde(rename = "type")]
    kind: &'static str,
    account: Option<u32>,
    from: Option<u32>,
    to: Option<u32>,
    amount: String,
    memo: &'a str,
}

fn cmd_export(path: &PathBuf, out: &PathBuf) -> Result<()> {
    let ledger = load_ledger(path)?;
    let mut wtr = csv::Writer::from_path(out).with_context(|| format!("建立 {}", out.display()))?;
    for tx in ledger.history() {
        let (kind, account, from, to) = match tx.kind {
            TxKind::Deposit { account } => ("deposit", Some(account), None, None),
            TxKind::Withdraw { account } => ("withdraw", Some(account), None, None),
            TxKind::Transfer { from, to } => ("transfer", None, Some(from), Some(to)),
        };
        wtr.serialize(HistoryRow {
            id: tx.id,
            date: tx.date,
            kind,
            account,
            from,
            to,
            amount: tx.amount.to_string(),
            memo: &tx.memo,
        })?;
    }
    wtr.flush()?;
    println!("已匯出 {} 筆到 {}", ledger.history().len(), out.display());
    Ok(())
}
