//! 評估指標。詐欺只佔 3%，「準確率」毫無意義：全部猜正常就有 97%。
//! 要看的是 precision（抓到的裡面有多少真的是）、recall（真的裡面抓到多少）。

use ndarray::Array1;

#[derive(Debug, Default, Clone, Copy)]
pub struct Confusion {
    pub tp: usize, // 詐欺，判詐欺
    pub fp: usize, // 正常，判詐欺（誤殺：客戶被打擾）
    pub tn: usize,
    pub fn_: usize, // 詐欺，判正常（漏網：錢丟了）
}

impl Confusion {
    /// 以 threshold 把機率切成 0/1 後統計
    pub fn from_scores(y_true: &Array1<f64>, scores: &Array1<f64>, threshold: f64) -> Confusion {
        let mut c = Confusion::default();
        for (&y, &s) in y_true.iter().zip(scores.iter()) {
            match (y >= 0.5, s >= threshold) {
                (true, true) => c.tp += 1,
                (false, true) => c.fp += 1,
                (false, false) => c.tn += 1,
                (true, false) => c.fn_ += 1,
            }
        }
        c
    }

    pub fn precision(&self) -> f64 {
        safe_div(self.tp, self.tp + self.fp)
    }

    pub fn recall(&self) -> f64 {
        safe_div(self.tp, self.tp + self.fn_)
    }

    pub fn f1(&self) -> f64 {
        let (p, r) = (self.precision(), self.recall());
        if p + r == 0.0 {
            0.0
        } else {
            2.0 * p * r / (p + r)
        }
    }

    pub fn accuracy(&self) -> f64 {
        safe_div(self.tp + self.tn, self.tp + self.fp + self.tn + self.fn_)
    }

    pub fn report(&self, name: &str) {
        println!(
            "{name:<22} precision {:.3}  recall {:.3}  F1 {:.3}  accuracy {:.3}   (TP {} FP {} FN {} TN {})",
            self.precision(), self.recall(), self.f1(), self.accuracy(), self.tp, self.fp, self.fn_, self.tn
        );
    }
}

fn safe_div(a: usize, b: usize) -> f64 {
    if b == 0 {
        0.0
    } else {
        a as f64 / b as f64
    }
}

/// ROC AUC：隨機抽一個詐欺與一個正常，模型給詐欺較高分的機率。0.5 = 亂猜，1.0 = 完美。
/// 不依賴 threshold，適合比較模型。
pub fn roc_auc(y_true: &Array1<f64>, scores: &Array1<f64>) -> f64 {
    let mut pairs: Vec<(f64, bool)> = scores
        .iter()
        .zip(y_true.iter())
        .map(|(&s, &y)| (s, y >= 0.5))
        .collect();
    pairs.sort_by(|a, b| a.0.partial_cmp(&b.0).unwrap());
    // 排名法：AUC = (正例的排名和 - n_pos(n_pos+1)/2) / (n_pos * n_neg)
    let n_pos = pairs.iter().filter(|p| p.1).count() as f64;
    let n_neg = pairs.len() as f64 - n_pos;
    let mut rank_sum = 0.0;
    let mut i = 0;
    while i < pairs.len() {
        // 同分的給平均排名
        let mut j = i;
        while j + 1 < pairs.len() && pairs[j + 1].0 == pairs[i].0 {
            j += 1;
        }
        let avg_rank = (i + j) as f64 / 2.0 + 1.0;
        for p in &pairs[i..=j] {
            if p.1 {
                rank_sum += avg_rank;
            }
        }
        i = j + 1;
    }
    (rank_sum - n_pos * (n_pos + 1.0) / 2.0) / (n_pos * n_neg)
}
