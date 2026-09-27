// Measurement identity belongs beside the value, before later prompt truncation.
fn published_measurement_source(telemetry: &SpectralTelemetry) -> String {
    let path = telemetry
        .stable_core
        .as_ref()
        .and_then(|v| v.get("covariance_path"))
        .and_then(serde_json::Value::as_str)
        .filter(|s| !s.is_empty());
    let path = path.map_or_else(
        || "generation path unavailable".to_owned(),
        |s| serde_json::to_string(&s.chars().take(100).collect::<String>()).unwrap_or_default(),
    );
    format!("Minime telemetry; {path}; engine_t_ms={}", telemetry.t_ms)
}

fn finite_measurement(value: Option<&serde_json::Value>) -> Option<f64> {
    value
        .and_then(serde_json::Value::as_f64)
        .filter(|v| v.is_finite() && *v >= 0.0)
}

fn display_percentage(value: Option<f64>) -> String {
    value.map_or_else(|| "unavailable".to_owned(), |v| format!("{v:.0}%"))
}

fn display_measurement(value: Option<f64>, precision: usize) -> String {
    value.map_or_else(|| "unavailable".to_owned(), |v| format!("{v:.precision$}"))
}

/// A separately clocked file snapshot. Never infer absent fields as zero, or use
/// its published-source snapshot to label the current telemetry packet.
#[must_use]
pub(crate) fn live_reservoir_clause_from_value(
    value: &serde_json::Value,
    now_s: f64,
) -> Option<String> {
    let uncentered = value.get("uncentered")?;
    let lambda1 = finite_measurement(uncentered.get("top8")?.as_array()?.first())?;
    let share = finite_measurement(uncentered.get("lambda1_share"))
        .filter(|v| *v <= 1.0)
        .map(|v| v * 100.0);
    let nodes = display_measurement(finite_measurement(value.get("esn_n")), 0);
    let rows = display_measurement(finite_measurement(value.get("window_rows")), 0);
    let centered = value.get("centered");
    let fluctuation = centered
        .and_then(|c| c.get("top8"))
        .and_then(serde_json::Value::as_array)
        .filter(|values| !values.is_empty())
        .map_or_else(
            || "unavailable".to_owned(),
            |values| {
                values
                    .iter()
                    .take(3)
                    .map(|v| display_measurement(finite_measurement(Some(v)), 3))
                    .collect::<Vec<_>>()
                    .join("/")
            },
        );
    let dim = display_measurement(
        finite_measurement(centered.and_then(|c| c.get("effective_dim"))),
        1,
    );
    let fill = display_percentage(
        finite_measurement(value.get("engine_style_fill_pct_top8")).filter(|v| *v <= 100.0),
    );
    let dump_time = finite_measurement(value.get("dump_mtime_unix_s"));
    let engine_time = display_measurement(finite_measurement(value.get("engine_t_ms")), 0);
    let age = dump_time
        .filter(|t| now_s.is_finite() && now_s >= *t)
        .map(|t| (now_s - t) / 60.0);
    let time = display_measurement(dump_time, 3);
    let freshness = match (dump_time, age) {
        (_, Some(age)) => format!("view {age:.1} min old"),
        (Some(_), None) => "age unavailable (clock mismatch)".to_owned(),
        (None, None) => "age unavailable (no dump time)".to_owned(),
    };
    Some(format!(
        " Live reservoir snapshot [Minime diagnostics/live_reservoir_spectrum.json; \
         dump_unix_s={time}; engine_t_ms={engine_time}; {freshness}; {nodes} nodes, {rows} states]. \
         Uncentered reservoir: λ₁ {lambda1:.2}; leading share {} of uncentered trace. \
         Centered reservoir fluctuations: modes {fluctuation}; effective dimension {dim}. \
         Reservoir-derived engine-style fill [uncentered top-8]: {fill}. \
         These are separate from the published cascade and its fill.",
        display_percentage(share)
    ))
}
