use astrid_guest::{capsule_result, serde_json, tool};
use serde_json::{Map, Value, json};
use sha2::{Digest as _, Sha256};

const STATE_PATH: &str = "home://edge/runtime/spectral_state.json";
const RECENT_ROLLUPS_CURRENT_PATH: &str = "home://edge/spectral/recent_rollups.current.jsonl";
const RECENT_ROLLUPS_PREVIOUS_PATH: &str = "home://edge/spectral/recent_rollups.previous.jsonl";
const ACTIVITY_RECEIPTS_CURRENT_PATH: &str = "home://edge/spectral/activity_receipts.current.jsonl";
const ACTIVITY_RECEIPTS_PREVIOUS_PATH: &str =
    "home://edge/spectral/activity_receipts.previous.jsonl";
const STATE_MAX_BYTES: usize = 64 * 1024;
const RECENT_ROLLUPS_DAY_MAX_BYTES: usize = 1_536 * 1024;
const ACTIVITY_RECEIPTS_DAY_MAX_BYTES: usize = 2 * 1024 * 1024;
const ROLLUP_MAX_BYTES: usize = 1_024;
const ACTIVITY_RECEIPT_MAX_BYTES: usize = 4_096;
const MAX_ROLLUPS_PER_DAY: usize = 1_440;
const MAX_ROLLUPS: usize = MAX_ROLLUPS_PER_DAY * 2;
const MAX_ACTIVITY_RECEIPTS: usize = 8_192;
const MAX_CORRELATIONS: usize = 20;
const MAX_ACTIVITY_REFS_PER_ROLLUP: usize = 2;
const MAX_CAUSAL_ID_CHARS: usize = 96;
const VALID_WINDOWS: [u64; 4] = [15, 60, 360, 1_440];
const AUTHORITY: &str =
    "deterministic_machine_spectral_observation_non_causal_not_astrid_authorship";

const METRICS: [MetricDefinition; 7] = [
    MetricDefinition::new("fill_pct", &["/fill_pct", "/metrics/fill_pct"]),
    MetricDefinition::new(
        "effective_dimensionality",
        &[
            "/effective_dimensionality",
            "/metrics/effective_dimensionality",
        ],
    ),
    MetricDefinition::new(
        "spectral_entropy",
        &[
            "/spectral_entropy",
            "/spectral/entropy",
            "/spectral_denominator/entropy",
            "/spectral_denominator_v1/spectral_entropy",
            "/metrics/spectral_entropy",
        ],
    ),
    MetricDefinition::new(
        "lambda1_share",
        &[
            "/lambda1_share",
            "/lambda1_energy_share",
            "/spectral/lambda1_share",
            "/spectral_denominator_v1/lambda1_energy_share",
            "/metrics/lambda1_share",
        ],
    ),
    MetricDefinition::new(
        "tail_share",
        &["/tail_share", "/spectral/tail_share", "/metrics/tail_share"],
    ),
    MetricDefinition::new(
        "density_gradient",
        &[
            "/density_gradient",
            "/spectral/density_gradient",
            "/metrics/density_gradient",
        ],
    ),
    MetricDefinition::new(
        "mode_turnover",
        &[
            "/mode_turnover",
            "/spectral/mode_turnover",
            "/metrics/mode_turnover",
        ],
    ),
];

struct EdgeSpectralCapsule;

type ToolHandler = fn(&Value) -> Result<String, String>;

impl astrid_guest::Guest for EdgeSpectralCapsule {
    fn astrid_hook_trigger(action: String, payload: Vec<u8>) -> astrid_guest::CapsuleResult {
        match action.as_str() {
            "tool_execute_read_spectral_now" => {
                handle_tool(&payload, "read_spectral_now", read_spectral_now)
            },
            "tool_execute_read_spectral_window" => {
                handle_tool(&payload, "read_spectral_window", read_spectral_window)
            },
            "tool_execute_correlate_spectral_activity" => handle_tool(
                &payload,
                "correlate_spectral_activity",
                correlate_spectral_activity,
            ),
            action if action.starts_with("tool_execute_") => {
                capsule_result::deny("unadvertised spectral tool denied")
            },
            _ => capsule_result::continue_empty(),
        }
    }

    fn run() {}

    fn astrid_install() {}

    fn astrid_upgrade() {}
}

fn handle_tool(
    payload: &[u8],
    expected_tool: &str,
    handler: ToolHandler,
) -> astrid_guest::CapsuleResult {
    let request = match tool::parse_request(payload) {
        Ok(request) => request,
        Err(error) => return capsule_result::deny(error),
    };
    if request.tool_name != expected_tool {
        return capsule_result::deny("tool action and request identity mismatch");
    }
    match handler(&request.arguments) {
        Ok(content) => tool::publish_success(&request.call_id, &request.tool_name, content),
        Err(error) => tool::publish_error(&request.call_id, &request.tool_name, error),
    }
}

fn read_spectral_now(args: &Value) -> Result<String, String> {
    ensure_allowed_keys(args, &[])?;
    let raw = read_utf8_bounded(STATE_PATH, STATE_MAX_BYTES)?;
    let state = parse_object(&raw, "spectral state")?;
    let schema = state
        .get("schema")
        .and_then(Value::as_str)
        .ok_or_else(|| "spectral state has no schema".to_string())?;
    if schema != "astrid_edge_spectral_state_v2" {
        return Err(format!(
            "unsupported spectral state schema `{schema}`; v2 is required"
        ));
    }
    verify_record_hash(&state)
        .map_err(|error| format!("spectral state failed hash verification: {error}"))?;

    let sanitized = sanitize_spectral_record(&state);
    serialize(&json!({
        "schema": "astrid_edge_spectral_now_result_v1",
        "recorded_at_unix_ms": sanitized.get("recorded_at_unix_ms"),
        "substrate": sanitized.get("substrate"),
        "metrics": sanitized.get("metrics"),
        "state": sanitized,
        "state_sha256": sha256(raw.as_bytes()),
        "authority": AUTHORITY,
        "causality": "none_claimed",
    }))
}

fn read_spectral_window(args: &Value) -> Result<String, String> {
    ensure_allowed_keys(args, &["minutes"])?;
    let minutes = required_window(args)?;
    let source = read_rollups()?;
    let ledger = parse_jsonl(
        &source.raw,
        ROLLUP_MAX_BYTES,
        MAX_ROLLUPS,
        "spectral rollup",
    )?;
    validate_rollups(&ledger.rows)?;
    let rows = select_window(&ledger.rows, minutes);
    let (start, end) = window_bounds(&rows);
    let projection_sha256 = sha256(source.raw.as_bytes());
    let temporal_window_context = summarize_activity_link_coverage(&ledger.rows);

    serialize(&json!({
        "schema": "astrid_edge_spectral_window_result_v1",
        "window_minutes": minutes,
        "window_start_unix_ms": start,
        "window_end_unix_ms": end,
        "first_recorded_at_unix_ms": start,
        "last_recorded_at_unix_ms": end,
        "count": rows.len(),
        "sample_count": rows.len(),
        "source": source.label,
        "projection_sha256": &projection_sha256,
        "source_sha256": &projection_sha256,
        "trailing_partial": ledger.trailing_partial_ignored,
        "trailing_partial_ignored": ledger.trailing_partial_ignored,
        "metrics": summarize_metrics(&rows),
        "coverage": sanitize_coverage(rows.last().copied()),
        "temporal_window_context": temporal_window_context,
        "authority": AUTHORITY,
        "causality": "correlation_only_no_causal_claim",
    }))
}

fn correlate_spectral_activity(args: &Value) -> Result<String, String> {
    ensure_allowed_keys(
        args,
        &[
            "trace_id",
            "session_id",
            "chain_id",
            "response_sha256",
            "limit",
        ],
    )?;
    let filter = CorrelationFilter::from_args(args)?;
    let limit = bounded_limit(args, MAX_CORRELATIONS)?;
    let receipts = read_activity_receipts()?;
    let parsed_receipts = parse_jsonl(
        &receipts.raw,
        ACTIVITY_RECEIPT_MAX_BYTES,
        MAX_ACTIVITY_RECEIPTS,
        "spectral activity receipt",
    )?;
    validate_activity_receipts(&parsed_receipts.rows)?;
    let mut matches = parsed_receipts
        .rows
        .iter()
        .filter_map(|receipt| correlated_activity_receipt(receipt, &filter))
        .collect::<Vec<_>>();

    matches.sort_by_key(|row| {
        row.get("activity_recorded_at_unix_ms")
            .and_then(Value::as_u64)
            .unwrap_or_default()
    });
    if matches.len() > limit {
        matches = matches.split_off(matches.len().saturating_sub(limit));
    }

    serialize(&json!({
        "schema": "astrid_edge_spectral_correlation_result_v1",
        "exact_filter": filter.as_json(),
        "count": matches.len(),
        "match_count": matches.len(),
        "matches": matches,
        "source": receipts.label,
        "trailing_partial_ignored": parsed_receipts.trailing_partial_ignored,
        "attribution_rule": "explicit_event_identifiers_to_hash_bound_snapshot_identity_only_never_minute_buckets_or_timestamp_proximity",
        "authority": AUTHORITY,
        "causality": "correlation_only_no_causal_claim",
    }))
}

fn read_rollups() -> Result<LedgerSource, String> {
    read_daily_projection(
        RECENT_ROLLUPS_CURRENT_PATH,
        RECENT_ROLLUPS_PREVIOUS_PATH,
        RECENT_ROLLUPS_DAY_MAX_BYTES,
        "daily_current_rollup_projection",
        "daily_current_and_previous_rollup_projections",
    )
}

fn read_activity_receipts() -> Result<LedgerSource, String> {
    read_daily_projection(
        ACTIVITY_RECEIPTS_CURRENT_PATH,
        ACTIVITY_RECEIPTS_PREVIOUS_PATH,
        ACTIVITY_RECEIPTS_DAY_MAX_BYTES,
        "daily_current_activity_receipt_projection",
        "daily_current_and_previous_activity_receipt_projections",
    )
}

fn read_daily_projection(
    current_path: &str,
    previous_path: &str,
    maximum_day_bytes: usize,
    current_label: &'static str,
    combined_label: &'static str,
) -> Result<LedgerSource, String> {
    let current = read_utf8_bounded(current_path, maximum_day_bytes).map_err(|error| {
        format!(
            "bounded current-day spectral projection is unavailable; authoritative history is intentionally outside capsule authority: {error}"
        )
    })?;
    // The runtime creates both bounded projections before observing activity,
    // including an empty previous-day file before the first UTC rotation. A
    // read error therefore fails closed rather than masquerading as no history.
    let previous = read_utf8_bounded(previous_path, maximum_day_bytes).map_err(|error| {
        format!("bounded previous-day spectral projection is unavailable: {error}")
    })?;
    let has_previous = !previous.is_empty();
    let mut raw = previous;
    if has_previous && !raw.ends_with('\n') {
        // A rotated projection should always contain newline-terminated append
        // records. Completing the boundary makes a torn final record fail JSON
        // validation rather than fuse with the current day's first record.
        raw.push('\n');
    }
    raw.push_str(&current);
    Ok(LedgerSource {
        raw,
        label: if has_previous {
            combined_label
        } else {
            current_label
        },
    })
}

fn read_utf8_bounded(path: &str, maximum: usize) -> Result<String, String> {
    let bytes = astrid_guest::bindings::astrid::capsule::fs::read_file(path)?;
    decode_utf8_bounded(bytes, maximum, path)
}

fn decode_utf8_bounded(bytes: Vec<u8>, maximum: usize, label: &str) -> Result<String, String> {
    if bytes.len() > maximum {
        return Err(format!(
            "{label} exceeds the {maximum}-byte whole-file safety cap"
        ));
    }
    String::from_utf8(bytes).map_err(|_| format!("{label} is not valid UTF-8"))
}

fn parse_object(raw: &str, label: &str) -> Result<Value, String> {
    let value = serde_json::from_str::<Value>(raw)
        .map_err(|error| format!("invalid {label} JSON: {error}"))?;
    if !value.is_object() {
        return Err(format!("{label} must be a JSON object"));
    }
    Ok(value)
}

fn parse_jsonl(
    raw: &str,
    maximum_line_bytes: usize,
    maximum_rows: usize,
    label: &str,
) -> Result<ParsedLedger, String> {
    let mut rows = Vec::new();
    let mut trailing_partial_ignored = false;
    let last_line_is_complete = raw.is_empty() || raw.ends_with('\n');
    let line_count = raw.lines().count();

    for (index, line) in raw.lines().enumerate() {
        if !last_line_is_complete && index.saturating_add(1) == line_count {
            trailing_partial_ignored = true;
            break;
        }
        if line.is_empty() {
            return Err(format!("{label} line {} is empty", index.saturating_add(1)));
        }
        if line.len() > maximum_line_bytes {
            return Err(format!(
                "{label} line {} exceeds the {maximum_line_bytes}-byte record cap",
                index.saturating_add(1)
            ));
        }
        match serde_json::from_str::<Value>(line) {
            Ok(value) if value.is_object() => rows.push(value),
            Ok(_) => {
                return Err(format!(
                    "{label} line {} must be a JSON object",
                    index.saturating_add(1)
                ));
            },
            Err(error) => {
                return Err(format!(
                    "invalid {label} JSON on line {}: {error}",
                    index.saturating_add(1)
                ));
            },
        }
        if rows.len() > maximum_rows {
            return Err(format!("{label} exceeds the {maximum_rows}-record cap"));
        }
    }
    Ok(ParsedLedger {
        rows,
        trailing_partial_ignored,
    })
}

fn ensure_allowed_keys(args: &Value, allowed: &[&str]) -> Result<(), String> {
    let object = args
        .as_object()
        .ok_or_else(|| "tool arguments must be a JSON object".to_string())?;
    if let Some(key) = object.keys().find(|key| !allowed.contains(&key.as_str())) {
        return Err(format!("unsupported argument `{key}`"));
    }
    Ok(())
}

fn validate_rollups(rows: &[Value]) -> Result<(), String> {
    for (index, row) in rows.iter().enumerate() {
        let line_number = index.saturating_add(1);
        if row.get("schema").and_then(Value::as_str) != Some("astrid_edge_spectral_rollup_v1") {
            return Err(format!(
                "spectral rollup line {line_number} has an unsupported schema"
            ));
        }
        if recorded_at(row).is_none() {
            return Err(format!(
                "spectral rollup line {line_number} has no valid recorded_at_unix_ms"
            ));
        }
        verify_record_hash(row).map_err(|error| {
            format!("spectral rollup line {line_number} failed hash verification: {error}")
        })?;
        if let Some(activity_refs) = row.get("activity_refs") {
            let activity_refs = activity_refs.as_array().ok_or_else(|| {
                format!("spectral rollup line {line_number} activity_refs must be an array")
            })?;
            if activity_refs.len() > MAX_ACTIVITY_REFS_PER_ROLLUP {
                return Err(format!(
                    "spectral rollup line {line_number} exceeds the {MAX_ACTIVITY_REFS_PER_ROLLUP} activity-ref cap"
                ));
            }
            if activity_refs.iter().any(|reference| !reference.is_object()) {
                return Err(format!(
                    "spectral rollup line {line_number} contains a non-object activity ref"
                ));
            }
            if activity_refs.iter().any(|reference| {
                reference.get("attribution").and_then(Value::as_str)
                    != Some("temporal_rollup_context_not_exact_or_causal")
            }) {
                return Err(format!(
                    "spectral rollup line {line_number} has an activity ref without the temporal-context boundary"
                ));
            }
        }
    }
    Ok(())
}

fn validate_activity_receipts(rows: &[Value]) -> Result<(), String> {
    for (index, row) in rows.iter().enumerate() {
        let line_number = index.saturating_add(1);
        if row.get("schema").and_then(Value::as_str) != Some("astrid_edge_spectral_receipt_v1") {
            return Err(format!(
                "spectral activity receipt line {line_number} has an unsupported schema"
            ));
        }
        if row
            .get("snapshot_generation_id")
            .and_then(Value::as_str)
            .is_none_or(str::is_empty)
            || row
                .get("snapshot_sequence")
                .and_then(Value::as_u64)
                .is_none_or(|sequence| sequence == 0)
            || row
                .get("snapshot_recorded_at_unix_ms")
                .and_then(Value::as_u64)
                .is_none_or(|timestamp| timestamp == 0)
            || row
                .get("snapshot_sha256")
                .and_then(Value::as_str)
                .is_none_or(|hash| !validate_sha256(hash))
        {
            return Err(format!(
                "spectral activity receipt line {line_number} has invalid snapshot identity"
            ));
        }
        if !row.get("metrics").is_some_and(Value::is_object) {
            return Err(format!(
                "spectral activity receipt line {line_number} has no bounded metrics object"
            ));
        }
        verify_record_hash(row).map_err(|error| {
            format!(
                "spectral activity receipt line {line_number} failed hash verification: {error}"
            )
        })?;
    }
    Ok(())
}

fn verify_record_hash(value: &Value) -> Result<(), String> {
    let claimed = value
        .get("record_sha256")
        .and_then(Value::as_str)
        .filter(|hash| validate_sha256(hash))
        .ok_or_else(|| "missing or invalid record_sha256".to_string())?;
    let mut unhashed = value.clone();
    unhashed
        .as_object_mut()
        .ok_or_else(|| "rollup is not an object".to_string())?
        .remove("record_sha256");
    let bytes = serde_json::to_vec(&unhashed).map_err(|error| error.to_string())?;
    let computed = sha256(&bytes);
    if !claimed.eq_ignore_ascii_case(&computed) {
        return Err("record_sha256 does not bind this record".to_string());
    }
    Ok(())
}

fn summarize_activity_link_coverage(rows: &[Value]) -> Value {
    let rows_with_refs = rows
        .iter()
        .filter(|row| row.get("activity_refs").and_then(Value::as_array).is_some())
        .count();
    let rows_with_declared_coverage = rows
        .iter()
        .filter(|row| {
            row.get("activity_ref_count")
                .and_then(Value::as_u64)
                .is_some()
                && row
                    .get("activity_refs_truncated")
                    .and_then(Value::as_bool)
                    .is_some()
        })
        .count();
    let rows_declaring_truncation = rows
        .iter()
        .filter(|row| row.get("activity_refs_truncated").and_then(Value::as_bool) == Some(true))
        .count();
    json!({
        "rows_with_activity_refs": rows_with_refs,
        "rows_with_declared_coverage": rows_with_declared_coverage,
        "rows_declaring_truncation": rows_declaring_truncation,
        "complete": (rows_with_refs == rows_with_declared_coverage
            && rows_declaring_truncation == 0),
    })
}

fn required_window(args: &Value) -> Result<u64, String> {
    let minutes = args
        .get("minutes")
        .and_then(Value::as_u64)
        .ok_or_else(|| "`minutes` must be one of 15, 60, 360, or 1440".to_string())?;
    if !VALID_WINDOWS.contains(&minutes) {
        return Err("`minutes` must be one of 15, 60, 360, or 1440".to_string());
    }
    Ok(minutes)
}

fn bounded_limit(args: &Value, maximum: usize) -> Result<usize, String> {
    let Some(value) = args.get("limit") else {
        return Ok(maximum);
    };
    let value = value
        .as_u64()
        .and_then(|value| usize::try_from(value).ok())
        .ok_or_else(|| format!("`limit` must be an integer from 1 through {maximum}"))?;
    if !(1..=maximum).contains(&value) {
        return Err(format!(
            "`limit` must be an integer from 1 through {maximum}"
        ));
    }
    Ok(value)
}

fn select_window(rows: &[Value], minutes: u64) -> Vec<&Value> {
    let Some(end) = rows.iter().filter_map(recorded_at).max() else {
        return Vec::new();
    };
    let window_ms = minutes.saturating_mul(60_000);
    let start = end.saturating_sub(window_ms);
    rows.iter()
        .filter(|row| {
            recorded_at(row).is_some_and(|timestamp| timestamp >= start && timestamp <= end)
        })
        .collect()
}

fn window_bounds(rows: &[&Value]) -> (Option<u64>, Option<u64>) {
    (
        rows.iter().filter_map(|row| recorded_at(row)).min(),
        rows.iter().filter_map(|row| recorded_at(row)).max(),
    )
}

fn recorded_at(value: &Value) -> Option<u64> {
    value
        .get("recorded_at_unix_ms")
        .or_else(|| value.get("timestamp_unix_ms"))
        .and_then(Value::as_u64)
}

fn summarize_metrics(rows: &[&Value]) -> Value {
    let metrics = METRICS
        .iter()
        .map(|definition| {
            let values = rows
                .iter()
                .filter_map(|row| definition.value(row))
                .filter(|value| value.is_finite())
                .collect::<Vec<_>>();
            (definition.name.to_string(), summarize(&values))
        })
        .collect::<Map<_, _>>();
    Value::Object(metrics)
}

fn summarize(values: &[f64]) -> Value {
    if values.is_empty() {
        return json!({"count": 0, "min": null, "mean": null, "max": null});
    }
    let minimum = values.iter().copied().fold(f64::INFINITY, f64::min);
    let maximum = values.iter().copied().fold(f64::NEG_INFINITY, f64::max);
    let sum = values.iter().copied().sum::<f64>();
    let count = u32::try_from(values.len()).unwrap_or(u32::MAX);
    let mean = sum / f64::from(count);
    json!({
        "count": values.len(),
        "min": round_six(minimum),
        "mean": round_six(mean),
        "max": round_six(maximum),
    })
}

fn round_six(value: f64) -> f64 {
    (value * 1_000_000.0).round() / 1_000_000.0
}

fn sanitize_spectral_record(value: &Value) -> Value {
    let metrics = METRICS
        .iter()
        .filter_map(|definition| {
            definition
                .value(value)
                .filter(|metric| metric.is_finite())
                .map(|metric| (definition.name.to_string(), json!(metric)))
        })
        .collect::<Map<_, _>>();
    json!({
        "schema": value.get("schema"),
        "recorded_at_unix_ms": recorded_at(value),
        "sequence": value.get("sequence").and_then(Value::as_u64),
        "substrate": sanitize_substrate(value),
        "metrics": metrics,
        "coverage": sanitize_coverage(Some(value)),
        "sensor_provenance": sanitize_sensor_provenance(value),
        "reservoir_parameters": {
            "target_fill_ratio": value.get("target_fill_ratio").and_then(Value::as_f64),
            "input_gain": value.get("input_gain").and_then(Value::as_f64),
            "exploration_noise": value.get("exploration_noise").and_then(Value::as_f64),
            "exploration_scale": value.get("exploration_scale").and_then(Value::as_f64),
            "regulation_strength": value.get("regulation_strength").and_then(Value::as_f64),
            "esn_leak": value.get("esn_leak").and_then(Value::as_f64),
        },
        "identity_stable": value
            .pointer("/spectral/identity_stable")
            .or_else(|| value.get("identity_stable"))
            .and_then(Value::as_bool),
        "mode_identity_state": bounded_known_string(value, &["/mode_identity_state"], 40),
    })
}

fn sanitize_substrate(value: &Value) -> Value {
    let source = value
        .get("substrate")
        .or_else(|| value.get("spectral_substrate_v1"))
        .unwrap_or(&Value::Null);
    json!({
        "policy": bounded_known_string(source, &["/policy"], 64),
        "schema_version": source.get("schema_version").and_then(Value::as_u64),
        "kind": bounded_known_string(source, &["/kind", "/substrate_kind"], 64),
        "fill_semantics": bounded_known_string(
            source,
            &["/fill_semantics", "/fill_metric"],
            64,
        ),
        "dimensions": source
            .get("dimensions")
            .or_else(|| source.get("reservoir_dimensions"))
            .or_else(|| source.get("reservoir_dim"))
            .and_then(Value::as_u64),
        "covariance_window_samples": source
            .get("covariance_window_samples")
            .and_then(Value::as_u64),
        "target_fill": source
            .get("target_fill")
            .or_else(|| value.get("target_fill_ratio"))
            .and_then(Value::as_f64),
    })
}

fn sanitize_coverage(value: Option<&Value>) -> Value {
    let Some(value) = value else {
        return Value::Null;
    };
    let coverage = value
        .get("coverage")
        .or_else(|| value.pointer("/spectral_denominator_v1/spectrum_coverage_v1"))
        .or_else(|| value.get("substrate"))
        .unwrap_or(&Value::Null);
    json!({
        "full_spectrum_mode_count": coverage
            .get("full_spectrum_mode_count")
            .or_else(|| coverage.get("full_spectrum_modes"))
            .and_then(Value::as_u64),
        "exported_spectrum_mode_count": coverage
            .get("exported_spectrum_mode_count")
            .or_else(|| coverage.get("exported_spectrum_modes"))
            .or_else(|| coverage.get("exported_eigenvalue_count"))
            .and_then(Value::as_u64),
        "exported_spectrum_energy_ratio": coverage
            .get("exported_spectrum_energy_ratio")
            .or_else(|| value.get("exported_spectrum_energy_ratio"))
            .and_then(Value::as_f64),
        "denominator_uses_full_spectrum": coverage
            .get("denominator_uses_full_spectrum")
            .and_then(Value::as_bool)
            .or_else(|| {
                (coverage
                    .get("incomplete_spectrum_sample_count")
                    .and_then(Value::as_u64)
                    .unwrap_or_default()
                    == 0)
                    .then_some(true)
            }),
        "sample_count": coverage.get("sample_count").and_then(Value::as_u64),
        "expected_sample_count": coverage
            .get("expected_sample_count")
            .and_then(Value::as_u64),
        "complete": coverage.get("complete").and_then(Value::as_bool),
    })
}

fn sanitize_sensor_provenance(value: &Value) -> Value {
    let provenance = value.get("sensor_provenance").unwrap_or(&Value::Null);
    let lane = |name: &str| {
        let source = provenance.get(name).unwrap_or(&Value::Null);
        let top_level_source = value.get(format!("{name}_source")).and_then(Value::as_str);
        let source_name = source
            .get("source")
            .and_then(Value::as_str)
            .or(top_level_source);
        json!({
            "available": source
                .get("available")
                .and_then(Value::as_bool)
                .or_else(|| source_name.map(|name| !name.starts_with("unavailable_"))),
            "fresh": source
                .get("fresh")
                .or_else(|| value.get(format!("{name}_fresh")))
                .and_then(Value::as_bool),
            "source": source_name
                .map(|text| text.chars().take(80).collect::<String>()),
        })
    };
    json!({
        "audio": lane("audio"),
        "aux": lane("aux"),
        "semantic": lane("semantic"),
        "video": lane("video"),
    })
}

fn correlated_activity_receipt(value: &Value, filter: &CorrelationFilter) -> Option<Value> {
    let matched_fields = filter.matched_fields(value)?;
    Some(json!({
        "kind": "exact_activity_snapshot_receipt",
        "activity_recorded_at_unix_ms": recorded_at(value),
        "activity_kind": bounded_known_string(value, &["/activity_kind"], 48),
        "status": bounded_known_string(value, &["/status"], 64),
        "matched_fields": matched_fields,
        "snapshot": {
            "generation_id": bounded_known_string(value, &["/snapshot_generation_id"], 128),
            "sequence": value.get("snapshot_sequence").and_then(Value::as_u64),
            "recorded_at_unix_ms": value
                .get("snapshot_recorded_at_unix_ms")
                .and_then(Value::as_u64),
            "sha256": bounded_hash(value, &["/snapshot_sha256"]),
            "metrics": sanitize_spectral_record(value).get("metrics").cloned(),
        },
        "attribution": "exact_event_identity_to_hash_bound_snapshot_non_causal",
        "record_sha256": bounded_hash(value, &["/record_sha256"]),
    }))
}

fn bounded_known_string(value: &Value, pointers: &[&str], maximum: usize) -> Option<String> {
    known_string(value, pointers).map(|text| text.chars().take(maximum).collect())
}

fn bounded_hash<'a>(value: &'a Value, pointers: &[&str]) -> Option<&'a str> {
    known_string(value, pointers).filter(|hash| validate_sha256(hash))
}

fn serialize(value: &Value) -> Result<String, String> {
    serde_json::to_string(value).map_err(|error| error.to_string())
}

fn sha256(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}

#[derive(Clone, Copy)]
struct MetricDefinition {
    name: &'static str,
    pointers: &'static [&'static str],
}

impl MetricDefinition {
    const fn new(name: &'static str, pointers: &'static [&'static str]) -> Self {
        Self { name, pointers }
    }

    fn value(self, value: &Value) -> Option<f64> {
        self.pointers
            .iter()
            .find_map(|pointer| value.pointer(pointer).and_then(Value::as_f64))
    }
}

struct LedgerSource {
    raw: String,
    label: &'static str,
}

struct ParsedLedger {
    rows: Vec<Value>,
    trailing_partial_ignored: bool,
}

#[derive(Default)]
struct CorrelationFilter {
    trace_id: Option<String>,
    session_id: Option<String>,
    chain_id: Option<String>,
    response_sha256: Option<String>,
}

impl CorrelationFilter {
    fn from_args(args: &Value) -> Result<Self, String> {
        let filter = Self {
            trace_id: optional_identifier(args, "trace_id", validate_trace_id)?,
            session_id: optional_identifier(args, "session_id", validate_short_identifier)?,
            chain_id: optional_identifier(args, "chain_id", validate_short_identifier)?,
            response_sha256: optional_identifier(args, "response_sha256", validate_sha256)?,
        };
        if filter.trace_id.is_none()
            && filter.session_id.is_none()
            && filter.chain_id.is_none()
            && filter.response_sha256.is_none()
        {
            return Err(
                "correlation requires at least one exact trace/session/chain/response identifier"
                    .to_string(),
            );
        }
        Ok(filter)
    }

    fn matched_fields(&self, value: &Value) -> Option<Vec<&'static str>> {
        let mut fields = Vec::new();
        if let Some(expected) = self.trace_id.as_deref() {
            match known_string(value, &["/trace/trace_id", "/trace_id"]) {
                Some(actual) if actual == expected => fields.push("trace_id"),
                _ => return None,
            }
        }
        if let Some(expected) = self.session_id.as_deref() {
            match known_string(value, &["/trace/session_id", "/session_id"]) {
                Some(actual) if actual == expected => fields.push("session_id"),
                _ => return None,
            }
        }
        if let Some(expected) = self.chain_id.as_deref() {
            match known_string(value, &["/trace/chain_id", "/chain_id"]) {
                Some(actual) if actual == expected => fields.push("chain_id"),
                _ => return None,
            }
        }
        if let Some(expected) = self.response_sha256.as_deref() {
            let matched = ["/response_sha256", "/parent_response_sha256"]
                .iter()
                .filter_map(|pointer| {
                    (value.pointer(pointer).and_then(Value::as_str) == Some(expected)).then_some(
                        if *pointer == "/response_sha256" {
                            "response_sha256"
                        } else {
                            "parent_response_sha256"
                        },
                    )
                })
                .collect::<Vec<_>>();
            if matched.is_empty() {
                return None;
            }
            fields.extend(matched);
        }
        Some(fields)
    }

    fn as_json(&self) -> Value {
        json!({
            "trace_id": self.trace_id,
            "session_id": self.session_id,
            "chain_id": self.chain_id,
            "response_sha256": self.response_sha256,
        })
    }
}

fn optional_identifier(
    args: &Value,
    name: &str,
    validator: fn(&str) -> bool,
) -> Result<Option<String>, String> {
    let Some(value) = args.get(name) else {
        return Ok(None);
    };
    let value = value
        .as_str()
        .ok_or_else(|| format!("`{name}` must be a string"))?;
    if !validator(value) {
        return Err(format!("invalid `{name}`"));
    }
    Ok(Some(value.to_string()))
}

fn validate_trace_id(value: &str) -> bool {
    value.len() == 36
        && value.char_indices().all(|(index, character)| {
            if matches!(index, 8 | 13 | 18 | 23) {
                character == '-'
            } else {
                character.is_ascii_hexdigit()
            }
        })
}

fn validate_short_identifier(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= MAX_CAUSAL_ID_CHARS
        && value.is_ascii()
        && !value.chars().any(char::is_control)
        && !value.contains('/')
        && !value.contains('\\')
        && !value.contains("..")
}

fn validate_sha256(value: &str) -> bool {
    value.len() == 64 && value.chars().all(|character| character.is_ascii_hexdigit())
}

fn known_string<'a>(value: &'a Value, pointers: &[&str]) -> Option<&'a str> {
    pointers
        .iter()
        .find_map(|pointer| value.pointer(pointer).and_then(Value::as_str))
}

astrid_guest::export!(EdgeSpectralCapsule with_types_in astrid_guest::bindings);

#[cfg(test)]
mod tests;
