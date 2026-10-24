//! 邏輯回歸：從零實作。
//!
//! 模型：p = σ(x·w + b)，σ(t) = 1 / (1 + e^-t)，把任何實數壓到 (0, 1) 當機率。
//! 損失：二元交叉熵 −[y ln p + (1−y) ln(1−p)]，p 越接近 y 損失越小。
//! 學習：梯度下降。損失對 w 的梯度剛好是 Xᵀ(p − y) / n，非常乾淨。

use ndarray::{Array1, Array2};

#[derive(Debug, Clone)]
pub struct LogisticRegression {
    pub weights: Array1<f64>,
    pub bias: f64,
    pub l2: f64,
    pub learning_rate: f64,
    pub epochs: usize,
    /// 正例權重：詐欺只有 3%，不加權的話模型會偏向「全部判正常」
    pub pos_weight: f64,
}

impl LogisticRegression {
    pub fn new(n_features: usize) -> Self {
        Self {
            weights: Array1::zeros(n_features),
            bias: 0.0,
            l2: 1e-3,
            learning_rate: 0.1,
            epochs: 300,
            pos_weight: 1.0,
        }
    }

    pub fn predict_proba(&self, x: &Array2<f64>) -> Array1<f64> {
        (x.dot(&self.weights) + self.bias).mapv(sigmoid)
    }

    /// 全批次梯度下降。回傳每個 epoch 的損失，方便看有沒有收斂。
    pub fn fit(&mut self, x: &Array2<f64>, y: &Array1<f64>) -> Vec<f64> {
        let n = x.nrows() as f64;
        // 每筆的權重：正例 pos_weight，負例 1
        let sample_w = y.mapv(|v| if v >= 0.5 { self.pos_weight } else { 1.0 });
        let mut losses = Vec::with_capacity(self.epochs);

        for _ in 0..self.epochs {
            let p = self.predict_proba(x);
            // 加權殘差
            let residual = (&p - y) * &sample_w; // 逐元素乘
                                                 // 梯度：Xᵀ r / n + L2 w
            let grad_w = x.t().dot(&residual) / n + &self.weights * self.l2;
            let grad_b = residual.sum() / n;
            self.weights = &self.weights - &(grad_w * self.learning_rate);
            self.bias -= grad_b * self.learning_rate;

            losses.push(weighted_bce(y, &p, &sample_w));
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
        // y = 1 當 x0 > 0
        let x = Array2::from_shape_vec((6, 1), vec![-3.0, -2.0, -1.0, 1.0, 2.0, 3.0]).unwrap();
        let y = Array1::from(vec![0.0, 0.0, 0.0, 1.0, 1.0, 1.0]);
        let mut m = LogisticRegression::new(1);
        m.epochs = 500;
        let losses = m.fit(&x, &y);
        assert!(losses.last().unwrap() < &losses[0], "損失應該下降");
        assert!(m.weights[0] > 0.0);
        let p = m.predict_proba(&x);
        assert!(p[0] < 0.1 && p[5] > 0.9);
    }
}
