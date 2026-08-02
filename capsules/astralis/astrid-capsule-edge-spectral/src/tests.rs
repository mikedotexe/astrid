use super::{
    AUTHORITY, CorrelationFilter, MAX_ROLLUPS, ROLLUP_MAX_BYTES, bounded_limit, correlated_records,
    decode_utf8_bounded, ensure_allowed_keys, parse_jsonl, sanitize_spectral_record, select_window,
    sha256, summarize_activity_link_coverage, summarize_metrics, validate_rollups,
    validate_short_identifier, validate_trace_id,
};
use astrid_guest::serde_json::{Value, json};

const TRACE: &str = "00000000-0000-4000-8000-000000000001";

fn hashed(mut value: Value) -> Value {
    value.as_object_mut().unwrap().remove("record_sha256");
    let digest = sha256(&astrid_guest::serde_json::to_vec(&value).unwrap());
    value["record_sha256"] = json!(digest);
    value
}

#[test]
fn strict_utf8_and_byte_caps_are_enforced() {
    assert!(decode_utf8_bounded(b"ok".to_vec(), 2, "fixture").is_ok());
    assert!(decode_utf8_bounded(b"too big".to_vec(), 2, "fixture").is_err());
    assert!(decode_utf8_bounded(vec![0xff], 2, "fixture").is_err());
}

#[test]
fn jsonl_rejects_bad_complete_records_and_oversized_lines() {
    assert!(parse_jsonl("{}\nnot-json\n", 64, 10, "fixture").is_err());
    assert!(parse_jsonl("[]\n", 64, 10, "fixture").is_err());
    assert!(parse_jsonl("\n", 64, 10, "fixture").is_err());
    assert!(
        parse_jsonl(
            &format!("{{\"x\":\"{}\"}}\n", "a".repeat(80)),
            64,
            10,
            "fixture"
        )
        .is_err()
    );
}

#[test]
fn jsonl_ignores_only_an_unterminated_trailing_fragment() {
    let parsed = parse_jsonl("{\"x\":1}\n{", 64, 10, "fixture").unwrap();
    assert_eq!(parsed.rows.len(), 1);
    assert!(parsed.trailing_partial_ignored);
    let valid_but_unterminated = parse_jsonl("{\"x\":1}\n{\"x\":2}", 64, 10, "fixture").unwrap();
    assert_eq!(valid_but_unterminated.rows.len(), 1);
    assert!(valid_but_unterminated.trailing_partial_ignored);
    let complete = parse_jsonl("{\"x\":1}\n{\n", 64, 10, "fixture");
    assert!(complete.is_err());
}

#[test]
fn recent_projection_caps_match_the_contract() {
    assert_eq!(MAX_ROLLUPS, 1_440);
    assert_eq!(ROLLUP_MAX_BYTES, 1_024);
    let rows = (0..=MAX_ROLLUPS)
        .map(|_| "{}")
        .collect::<Vec<_>>()
        .join("\n")
        + "\n";
    assert!(parse_jsonl(&rows, ROLLUP_MAX_BYTES, MAX_ROLLUPS, "fixture").is_err());
}

#[test]
fn windows_are_fixed_and_relative_to_latest_record() {
    let rows = vec![
        json!({"recorded_at_unix_ms": 0, "fill_pct": 1.0}),
        json!({"recorded_at_unix_ms": 60_000, "fill_pct": 2.0}),
        json!({"recorded_at_unix_ms": 16 * 60_000, "fill_pct": 3.0}),
    ];
    let selected = select_window(&rows, 15);
    assert_eq!(selected.len(), 2);
    let summary = summarize_metrics(&selected);
    assert_eq!(summary["fill_pct"]["mean"], 2.5);
}

#[test]
fn state_sanitization_excludes_unknown_and_secret_fields() {
    let state = json!({
        "schema": "astrid_edge_spectral_state_v2",
        "recorded_at_unix_ms": 1,
        "fill_pct": 68.0,
        "spectral_denominator": {"entropy": 0.9},
        "substrate": {
            "kind": "cpu_edge_covariance_effective_rank",
            "fill_metric": "normalized_covariance_effective_rank",
            "reservoir_dim": 128,
            "full_spectrum_mode_count": 128,
            "exported_eigenvalue_count": 16,
            "denominator_uses_full_spectrum": true
        },
        "exported_spectrum_energy_ratio": 0.91,
        "audio_fresh": true,
        "audio_source": "alsa:hw:1",
        "mode_identity_state": "unstable_near_degenerate",
        "secret": "must-not-leak",
        "prompt": "must-not-leak"
    });
    let sanitized = sanitize_spectral_record(&state);
    let rendered = sanitized.to_string();
    assert!(!rendered.contains("must-not-leak"));
    assert_eq!(sanitized["metrics"]["fill_pct"], 68.0);
    assert_eq!(sanitized["metrics"]["spectral_entropy"], 0.9);
    assert_eq!(sanitized["substrate"]["dimensions"], 128);
    assert_eq!(sanitized["coverage"]["exported_spectrum_mode_count"], 16);
    assert_eq!(sanitized["sensor_provenance"]["audio"]["fresh"], true);
    assert_eq!(sanitized["mode_identity_state"], "unstable_near_degenerate");
}

#[test]
fn correlations_require_exact_explicit_identifiers() {
    let filter = CorrelationFilter::from_args(&json!({"trace_id": TRACE})).unwrap();
    let exact = json!({"trace": {"trace_id": TRACE}, "recorded_at_unix_ms": 10});
    let same_time = json!({"trace": {"trace_id": "00000000-0000-4000-8000-000000000002"}, "recorded_at_unix_ms": 10});
    assert_eq!(filter.matched_fields(&exact).unwrap(), vec!["trace_id"]);
    assert!(filter.matched_fields(&same_time).is_none());
    assert!(CorrelationFilter::from_args(&json!({})).is_err());
}

#[test]
fn correlation_uses_explicit_bounded_activity_refs_not_shared_timestamps() {
    let filter = CorrelationFilter::from_args(&json!({"trace_id": TRACE})).unwrap();
    let rollup = json!({
        "schema": "astrid_edge_spectral_rollup_v1",
        "recorded_at_unix_ms": 10,
        "metrics": {"fill_pct": 68.0},
        "activity_refs": [
            {
                "kind": "action",
                "recorded_at_unix_ms": 9,
                "trace": {"trace_id": TRACE, "session_id": "session-1"},
                "response_sha256": "a".repeat(64)
            }
        ]
    });
    let matches = correlated_records(&rollup, &filter);
    assert_eq!(matches.len(), 1);
    assert_eq!(matches[0]["activity_kind"], "action");

    let unrelated = json!({
        "schema": "astrid_edge_spectral_rollup_v1",
        "recorded_at_unix_ms": 10,
        "activity_refs": [{"trace_id": "00000000-0000-4000-8000-000000000002"}]
    });
    assert!(correlated_records(&unrelated, &filter).is_empty());
}

#[test]
fn rollup_schema_timestamp_and_activity_ref_caps_are_strict() {
    assert!(
        validate_rollups(&[hashed(json!({
            "schema": "astrid_edge_spectral_rollup_v1",
            "recorded_at_unix_ms": 1,
            "activity_refs": [{}, {}]
        }))])
        .is_ok()
    );
    assert!(
        validate_rollups(&[hashed(json!({
            "schema": "astrid_edge_spectral_rollup_v1",
            "recorded_at_unix_ms": 1,
            "activity_refs": [{}, {}, {}]
        }))])
        .is_err()
    );
    assert!(
        validate_rollups(&[hashed(json!({
            "schema": "legacy",
            "recorded_at_unix_ms": 1
        }))])
        .is_err()
    );
    assert!(
        validate_rollups(&[hashed(json!({
            "schema": "astrid_edge_spectral_rollup_v1"
        }))])
        .is_err()
    );
    let valid = hashed(json!({
        "schema": "astrid_edge_spectral_rollup_v1",
        "recorded_at_unix_ms": 1,
        "activity_refs": []
    }));
    let mut tampered = valid.clone();
    tampered["recorded_at_unix_ms"] = json!(2);
    assert!(validate_rollups(&[valid]).is_ok());
    assert!(validate_rollups(&[tampered]).is_err());
    let coverage = summarize_activity_link_coverage(&[json!({
        "activity_refs": [{}],
        "activity_ref_count": 2,
        "activity_refs_truncated": true
    })]);
    assert_eq!(coverage["rows_declaring_truncation"], 1);
    assert_eq!(coverage["complete"], false);
}

#[test]
fn response_parentage_is_explicit_not_invented() {
    let hash = "a".repeat(64);
    let filter = CorrelationFilter::from_args(&json!({"response_sha256": hash})).unwrap();
    let parent = json!({"parent_response_sha256": "a".repeat(64)});
    assert_eq!(
        filter.matched_fields(&parent).unwrap(),
        vec!["parent_response_sha256"]
    );
}

#[test]
fn identifier_and_argument_validation_rejects_paths() {
    assert!(validate_trace_id(TRACE));
    assert!(!validate_trace_id("almost-a-trace"));
    assert!(validate_short_identifier("chain-1"));
    assert!(!validate_short_identifier("../peer"));
    assert!(ensure_allowed_keys(&json!({}), &[]).is_ok());
    assert!(ensure_allowed_keys(&json!({"path": "/tmp/state"}), &[]).is_err());
    assert!(bounded_limit(&json!({"limit": "20"}), 20).is_err());
    assert!(bounded_limit(&json!({"limit": 0}), 20).is_err());
    assert!(bounded_limit(&json!({"limit": 21}), 20).is_err());
}

#[test]
fn output_is_machine_observed_and_non_causal() {
    let payload = json!({"authority": AUTHORITY, "causality": "none_claimed"});
    assert_eq!(
        payload["authority"],
        "deterministic_machine_spectral_observation_non_causal_not_astrid_authorship"
    );
}

#[test]
fn manifest_grants_only_two_fixed_read_paths_and_ipc() {
    let manifest = include_str!("../Capsule.toml");
    assert!(manifest.contains("home://edge/runtime/spectral_state.json"));
    assert!(manifest.contains("home://edge/spectral/recent_rollups.jsonl"));
    assert_eq!(manifest.matches("home://").count(), 4);
    for forbidden in [
        "home://edge/spectral/rollups.jsonl",
        "home://edge/spectral/receipts.jsonl",
        "home://edge/tuning",
        "fs_write",
        "network =",
        "process =",
        "shell =",
        "consciousness.v1.control",
    ] {
        assert!(
            !manifest.contains(forbidden),
            "unexpected authority: {forbidden}"
        );
    }
}

#[test]
fn metric_summary_is_deterministic() {
    let rows = [
        json!({"fill_pct": 69.0, "spectral": {"tail_share": 0.2}}),
        json!({"fill_pct": 67.0, "spectral": {"tail_share": 0.4}}),
    ];
    let refs = rows.iter().collect::<Vec<&Value>>();
    assert_eq!(summarize_metrics(&refs), summarize_metrics(&refs));
    assert_eq!(summarize_metrics(&refs)["fill_pct"]["min"], 67.0);
    assert_eq!(summarize_metrics(&refs)["tail_share"]["max"], 0.4);
}
