//! 邏輯回歸（ML-2 練習解答版：加了 mini-batch 與 L1）。

use ndarray::{s, Array1, Array2};

#[derive(Debug, Clone)]
pub struct LogisticRegression {
    pub weights: Array1<f64>,
    pub bias: f64,
    pub l2: f64,
    /// 練習 4：L1 正則化。會把不重要的權重壓到剛好 0（自動選特徵）
    pub l1: f64,
    pub learning_rate: f64,
    pub epochs: usize,
    pub pos_weight: f64,
    /// 練習 3：mini-batch 大小。0 = 全批次
    pub batch_size: usize,
}

impl LogisticRegression {
    pub fn new(n_features: usize) -> Self {
        Self {
            weights: Array1::zeros(n_features),
            bias: 0.0,
            l2: 1e-3,
            l1: 0.0,
            learning_rate: 0.1,
            epochs: 300,
            pos_weight: 1.0,
            batch_size: 0,
        }
    }

    pub fn predict_proba(&self, x: &Array2<f64>) -> Array1<f64> {
        (x.dot(&self.weights) + self.bias).mapv(sigmoid)
    }

    /// 梯度下降（全批次或 mini-batch）。回傳每個 epoch 結束時對整個訓練集的損失。
    pub fn fit(&mut self, x: &Array2<f64>, y: &Array1<f64>) -> Vec<f64> {
        let n_total = x.nrows();
        let sample_w = y.mapv(|v| if v >= 0.5 { self.pos_weight } else { 1.0 });
        let batch = if self.batch_size == 0 {
            n_total
        } else {
            self.batch_size
        };
        let mut losses = Vec::with_capacity(self.epochs);

        for _ in 0..self.epochs {
            // 練習 3：同樣的資料走一遍，mini-batch 更新 n/batch 次，全批次只更新 1 次
            for start in (0..n_total).step_by(batch) {
                let end = (start + batch).min(n_total);
                let xb = x.slice(s![start..end, ..]);
                let yb = y.slice(s![start..end]);
                let wb = sample_w.slice(s![start..end]);
                let n = (end - start) as f64;
                let p = (xb.dot(&self.weights) + self.bias).mapv(sigmoid);
                let residual = (&p - &yb) * &wb;
                // 練習 4：L1 的梯度是 sign(w)
                let l1_grad = self.weights.mapv(|w| {
                    if w > 0.0 {
                        1.0
                    } else if w < 0.0 {
                        -1.0
                    } else {
                        0.0
                    }
                }) * self.l1;
                let grad_w = xb.t().dot(&residual) / n + &self.weights * self.l2 + l1_grad;
                let grad_b = residual.sum() / n;
                self.weights = &self.weights - &(grad_w * self.learning_rate);
                self.bias -= grad_b * self.learning_rate;
            }
            losses.push(weighted_bce(y, &self.predict_proba(x), &sample_w));
        }
        losses
    }
}

pub fn sigmoid(t: f64) -> f64 {
    1.0 / (1.0 + (-t).exp())
}

fn weighted_bce(y: &Array1<f64>, p: &Array1<f64>, w: &Array1<f64>) -> f64 {
    let eps = 1e-12;
    let total: f64 = y
        .iter()
        .zip(p.iter())
        .zip(w.iter())
        .map(|((&y, &p), &w)| -w * (y * (p + eps).ln() + (1.0 - y) * (1.0 - p + eps).ln()))
        .sum();
    total / w.sum()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn learns_a_linearly_separable_problem() {
        let x = Array2::from_shape_vec((6, 1), vec![-3.0, -2.0, -1.0, 1.0, 2.0, 3.0]).unwrap();
        let y = Array1::from(vec![0.0, 0.0, 0.0, 1.0, 1.0, 1.0]);
        let mut m = LogisticRegression::new(1);
        m.epochs = 500;
        let losses = m.fit(&x, &y);
        assert!(losses.last().unwrap() < &losses[0]);
        assert!(m.weights[0] > 0.0);
    }

    #[test]
    fn minibatch_converges_faster_per_epoch() {
        let x = Array2::from_shape_vec((64, 1), (0..64).map(|i| i as f64 / 8.0 - 4.0).collect())
            .unwrap();
        let y = x.column(0).mapv(|v| if v > 0.0 { 1.0 } else { 0.0 });
        let mut full = LogisticRegression::new(1);
        full.epochs = 20;
        let mut mini = LogisticRegression::new(1);
        mini.epochs = 20;
        mini.batch_size = 8;
        let lf = full.fit(&x, &y);
        let lm = mini.fit(&x, &y);
        assert!(
            lm.last().unwrap() < lf.last().unwrap(),
            "mini {} vs full {}",
            lm.last().unwrap(),
            lf.last().unwrap()
        );
    }
}
