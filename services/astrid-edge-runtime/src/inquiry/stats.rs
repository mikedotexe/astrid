pub(super) fn linear_slope(values: &[f64]) -> Option<f64> {
    if values.len() < 3 {
        return None;
    }
    let n = f64::from(u32::try_from(values.len()).ok()?);
    let x_mean = (n - 1.0) / 2.0;
    let y_mean = values.iter().sum::<f64>() / n;
    let numerator = values
        .iter()
        .enumerate()
        .map(|(index, value)| {
            (f64::from(u32::try_from(index).unwrap_or(u32::MAX)) - x_mean) * (value - y_mean)
        })
        .sum::<f64>();
    let denominator = (0..values.len())
        .map(|index| (f64::from(u32::try_from(index).unwrap_or(u32::MAX)) - x_mean).powi(2))
        .sum::<f64>();
    (denominator > f64::EPSILON).then_some(numerator / denominator)
}

pub(super) fn autocorrelation(values: &[f64], lag: usize) -> Option<f64> {
    if lag == 0 || values.len() < lag.saturating_add(3) {
        return None;
    }
    correlation(&values[..values.len().saturating_sub(lag)], &values[lag..])
}

pub(super) fn correlation_pairs(values: &[(f64, f64)]) -> Option<f64> {
    let left = values.iter().map(|value| value.0).collect::<Vec<_>>();
    let right = values.iter().map(|value| value.1).collect::<Vec<_>>();
    correlation(&left, &right)
}

pub(super) fn bounded_cross_correlation(
    values: &[(f64, f64)],
    maximum_lag: usize,
) -> Option<(i32, f64)> {
    let left = values.iter().map(|value| value.0).collect::<Vec<_>>();
    let right = values.iter().map(|value| value.1).collect::<Vec<_>>();
    let maximum_lag = maximum_lag.min(values.len().saturating_sub(3));
    (-(i32::try_from(maximum_lag).ok()?)..=i32::try_from(maximum_lag).ok()?)
        .filter_map(|lag| {
            if lag >= 0 {
                let lag = usize::try_from(lag).ok()?;
                correlation(&left[..left.len().saturating_sub(lag)], &right[lag..])
                    .map(|value| (i32::try_from(lag).unwrap_or(i32::MAX), value))
            } else {
                let lag = usize::try_from(lag.unsigned_abs()).ok()?;
                correlation(&left[lag..], &right[..right.len().saturating_sub(lag)])
                    .map(|value| (-i32::try_from(lag).unwrap_or(i32::MAX), value))
            }
        })
        .max_by(|left, right| left.1.abs().total_cmp(&right.1.abs()))
}

pub(super) fn correlation(left: &[f64], right: &[f64]) -> Option<f64> {
    if left.len() != right.len() || left.len() < 3 {
        return None;
    }
    let count = f64::from(u32::try_from(left.len()).ok()?);
    let left_mean = left.iter().sum::<f64>() / count;
    let right_mean = right.iter().sum::<f64>() / count;
    let covariance = left
        .iter()
        .zip(right)
        .map(|(left, right)| (left - left_mean) * (right - right_mean))
        .sum::<f64>();
    let left_variance = left
        .iter()
        .map(|value| (value - left_mean).powi(2))
        .sum::<f64>();
    let right_variance = right
        .iter()
        .map(|value| (value - right_mean).powi(2))
        .sum::<f64>();
    let denominator = (left_variance * right_variance).sqrt();
    (denominator > f64::EPSILON).then_some(covariance / denominator)
}
