//! 無監督的異常偵測：沒有標記時怎麼辦？
//!
//! 想法：正常交易很多、長得都差不多；異常交易很少、長得不一樣。
//! 把每筆交易「離正常中心有多遠」算出來，最遠的那些就是可疑的。

use ndarray::{Array1, Array2, Axis};

/// z 分數異常分數：每個特徵標準化後，取平方和的平方根。
/// 等於假設特徵彼此獨立的 Mahalanobis 距離。
pub fn zscore_anomaly(x_standardized: &Array2<f64>) -> Array1<f64> {
    x_standardized.mapv(|v| v * v).sum_axis(Axis(1)).mapv(f64::sqrt)
}

/// 真正的 Mahalanobis 距離：考慮特徵之間的相關性。
/// d² = (x-μ)ᵀ Σ⁻¹ (x-μ)。這裡 x 已標準化所以 μ=0，Σ 是相關係數矩陣。
/// 用 Cholesky 分解解 Σ⁻¹，不需要外部線性代數套件。
pub fn mahalanobis(x_standardized: &Array2<f64>) -> Array1<f64> {
    let n = x_standardized.nrows() as f64;
    let mut cov = x_standardized.t().dot(x_standardized) / n;
    // 加一點對角線正則化，避免高度相關的特徵讓矩陣奇異
    for i in 0..cov.nrows() {
        cov[[i, i]] += 1e-3;
    }
    let l = cholesky(&cov);
    // d² = ||L⁻¹ (x-μ)||²：對每一列解 L z = x
    x_standardized
        .rows()
        .into_iter()
        .map(|row| {
            let z = forward_substitute(&l, &row.to_owned());
            z.dot(&z).sqrt()
        })
        .collect()
}

/// Cholesky：對稱正定矩陣 A = L Lᵀ，L 是下三角
fn cholesky(a: &Array2<f64>) -> Array2<f64> {
    let n = a.nrows();
    let mut l = Array2::<f64>::zeros((n, n));
    for i in 0..n {
        for j in 0..=i {
            let mut sum = a[[i, j]];
            for k in 0..j {
                sum -= l[[i, k]] * l[[j, k]];
            }
            l[[i, j]] = if i == j { sum.max(1e-12).sqrt() } else { sum / l[[j, j]] };
        }
    }
    l
}

/// 解 L z = b（L 下三角）
fn forward_substitute(l: &Array2<f64>, b: &Array1<f64>) -> Array1<f64> {
    let n = b.len();
    let mut z = Array1::<f64>::zeros(n);
    for i in 0..n {
        let mut sum = b[i];
        for k in 0..i {
            sum -= l[[i, k]] * z[k];
        }
        z[i] = sum / l[[i, i]];
    }
    z
}

/// 把分數的前 top_fraction 標成異常，回傳對應的 threshold
pub fn threshold_for_top(scores: &Array1<f64>, top_fraction: f64) -> f64 {
    let mut sorted: Vec<f64> = scores.to_vec();
    sorted.sort_by(|a, b| b.partial_cmp(a).unwrap());
    let k = ((sorted.len() as f64 * top_fraction) as usize).clamp(1, sorted.len() - 1);
    sorted[k]
}
