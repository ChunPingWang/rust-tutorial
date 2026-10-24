//! 合成交易資料。
//!
//! 真實的詐欺標記資料不能公開，所以我們用統計上像真的分佈產生：
//! - 每個帳戶有自己的「典型金額」（對數常態分佈）與「典型交易時段」
//! - 正常交易：金額在帳戶典型值附近、時段在白天、種類以存款提款為主
//! - 詐欺交易（約 3%）：金額是典型值的好幾倍、偏向深夜、幾乎都是提款或轉帳、
//!   而且常常短時間內連續發生（velocity）
//!
//! 這些「偏向」正是模型要學出來的。注意沒有任何一個特徵能單獨判斷：
//! 白天也有詐欺、深夜也有正常交易。

use rand::prelude::*;
use rand_distr::{LogNormal, Normal};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    Deposit,
    Withdraw,
    Transfer,
}

/// 一筆交易：帳本的 Transaction 加上時間戳與風控團隊的標記。
#[derive(Debug, Clone)]
#[allow(dead_code)] // 這個模組是從 ML-1 複製來的，bench 只用到部分欄位
pub struct LabeledTx {
    pub id: u64,
    pub account: u32,
    pub kind: Kind,
    pub amount_cents: i64,
    /// 距離資料集起點的分鐘數（用來算「過去 24 小時內幾筆」）
    pub minute: u32,
    /// 0..24
    pub hour: u8,
    /// 帳戶在這筆交易前的餘額（分）
    pub balance_before_cents: i64,
    /// 風控標記：true = 詐欺
    pub is_fraud: bool,
}

struct AccountProfile {
    log_mean: f64, // 典型金額的 ln
    log_sd: f64,
    peak_hour: f64,
    balance_cents: i64,
}

pub fn generate(n: usize, seed: u64) -> Vec<LabeledTx> {
    let mut rng = StdRng::seed_from_u64(seed);
    let n_accounts = (n / 25).max(10);

    let mut profiles: Vec<AccountProfile> = (0..n_accounts)
        .map(|_| AccountProfile {
            log_mean: rng.gen_range(5.5..9.0), // e^5.5 ≈ 245 元，e^9 ≈ 8,103 元（以元計，之後 ×100）
            log_sd: rng.gen_range(0.3..0.7),
            peak_hour: rng.gen_range(9.0..19.0),
            balance_cents: rng.gen_range(50_000..5_000_000) * 100,
        })
        .collect();

    let mut txs = Vec::with_capacity(n);
    let mut minute: u32 = 0;
    let mut id = 1;
    // 詐欺傾向成串出現：一旦進入「被盜用」狀態，接下來幾筆都是詐欺
    let mut compromised: Option<(usize, u8)> = None; // (account, 剩餘筆數)

    while txs.len() < n {
        minute += rng.gen_range(1..=6);
        let (acct_idx, is_fraud) = match compromised {
            Some((a, left)) if left > 0 => {
                compromised = if left == 1 { None } else { Some((a, left - 1)) };
                (a, true)
            }
            _ => {
                let a = rng.gen_range(0..n_accounts);
                if rng.gen_bool(0.012) {
                    compromised = Some((a, rng.gen_range(1..=4)));
                    (a, true)
                } else {
                    (a, false)
                }
            }
        };
        let p = &profiles[acct_idx];

        let (kind, amount_yuan, hour) = if is_fraud {
            let kind = if rng.gen_bool(0.6) {
                Kind::Withdraw
            } else {
                Kind::Transfer
            };
            // 金額：典型值的 3~15 倍
            let base = LogNormal::new(p.log_mean, p.log_sd)
                .unwrap()
                .sample(&mut rng);
            let amount = base * rng.gen_range(3.0..15.0);
            // 時段：70% 在深夜
            let hour = if rng.gen_bool(0.7) {
                rng.gen_range(0.0..5.0)
            } else {
                rng.gen_range(0.0..24.0)
            };
            (kind, amount, hour)
        } else {
            let kind = match rng.gen_range(0..10) {
                0..=4 => Kind::Deposit,
                5..=7 => Kind::Withdraw,
                _ => Kind::Transfer,
            };
            let amount = LogNormal::new(p.log_mean, p.log_sd)
                .unwrap()
                .sample(&mut rng);
            let hour = Normal::new(p.peak_hour, 3.0)
                .unwrap()
                .sample(&mut rng)
                .rem_euclid(24.0);
            (kind, amount, hour)
        };

        // 不同帳戶的時間若用同一條 minute 軸會太擠；這裡簡化處理，minute 只用來算 velocity
        let amount_cents = (amount_yuan * 100.0).round().max(1.0) as i64;
        // 提款不能超過餘額（正常交易）；詐欺交易則可能「剛好把帳戶掏空」
        let amount_cents = match kind {
            Kind::Deposit => amount_cents,
            _ if is_fraud && rng.gen_bool(0.3) => p.balance_cents.max(1),
            _ => amount_cents.min(p.balance_cents.max(1)),
        };

        txs.push(LabeledTx {
            id,
            account: 1000 + acct_idx as u32,
            kind,
            amount_cents,
            minute,
            hour: hour as u8,
            balance_before_cents: p.balance_cents,
            is_fraud,
        });
        id += 1;

        let p = &mut profiles[acct_idx];
        p.balance_cents = match kind {
            Kind::Deposit => p.balance_cents + amount_cents,
            _ => (p.balance_cents - amount_cents).max(0),
        };
        // 被掏空的帳戶之後會有存款進來（正常使用者發現後補錢）
        if p.balance_cents < 10_000 {
            p.balance_cents += rng.gen_range(100_000..1_000_000) * 100;
        }
    }
    txs
}
