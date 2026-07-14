//! Trace Lab Spine V0: append-only, read-only scientific trace records.
//!
//! This module records compact envelopes for live events and LLM exposure
//! records. It does not change runtime control, sensory cadence, prompts, or
//! provider behavior; failures are intended to be diagnostic debt, not runtime
//! blockers.

use std::fs::{self, OpenOptions};
use std::io::Write as _;
use std::path::Path;
use std::sync::OnceLock;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use anyhow::{Context, Result};
use chrono::{TimeZone, Utc};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};

use crate::paths::bridge_paths;
use crate::types::{SafetyLevel, SensoryMsg, SpectralTelemetry};

pub const TRACE_EVENT_POLICY: &str = "trace_event_v1";
pub const TRACE_EVENT_SCHEMA_VERSION: u32 = 1;
pub const EXPOSURE_RECORD_POLICY: &str = "trace_exposure_record_v1";
pub const EXPOSURE_RECORD_SCHEMA_VERSION: u32 = 1;
const STATE_WINDOW_SECS: u64 = 60;

static STREAM_SEQUENCE: AtomicU64 = AtomicU64::new(0);
static STREAM_SESSION_ID: OnceLock<String> = OnceLock::new();
static PROCESS_START: OnceLock<Instant> = OnceLock::new();

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct TraceEventV1 {
    pub schema_version: u32,
    pub policy: String,
    pub event_id: String,
    pub stream_session_id: String,
    pub stream_sequence: u64,
    pub monotonic_time_ms: u64,
    pub wall_time_unix_s: f64,
    pub state_window_id: String,
    pub source_identity: String,
    pub source_class: String,
    pub lane: String,
    pub topic: String,
    pub payload_hash: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub payload_ref: Option<String>,
    pub compact_payload: Value,
    pub runtime_build_id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reservoir_config_hash: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub controller_config_hash: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub semantic_codec_version: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub replay_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub exposure_record_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub report_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub card_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub authority_class: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub consent_ref: Option<String>,
}

#[derive(Debug, Clone)]
pub struct TraceEventInput {
    pub wall_time_unix_s: f64,
    pub source_identity: String,
    pub source_class: String,
    pub lane: String,
    pub topic: String,
    pub payload_hash: String,
    pub payload_ref: Option<String>,
    pub compact_payload: Value,
    pub replay_id: Option<String>,
    pub exposure_record_id: Option<String>,
    pub report_id: Option<String>,
    pub card_id: Option<String>,
    pub authority_class: Option<String>,
    pub consent_ref: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ExposureRecordV1 {
    pub schema_version: u32,
    pub policy: String,
    pub exposure_record_id: String,
    pub wall_time_unix_s: f64,
    pub state_window_id: String,
    pub reporter_identity: String,
    pub exposure_class: String,
    pub prompt_ref: String,
    pub prompt_hash: String,
    pub runtime_build_id: String,
    pub source_refs: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub report_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub card_id: Option<String>,
    pub authority_class: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub consent_ref: Option<String>,
}

#[must_use]
pub fn runtime_build_id() -> String {
    format!(
        "{}:{}:pid{}",
        env!("CARGO_PKG_NAME"),
        env!("CARGO_PKG_VERSION"),
        std::process::id()
    )
}

#[must_use]
pub fn stream_session_id() -> String {
    STREAM_SESSION_ID
        .get_or_init(|| {
            let millis = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .map_or(0_u128, |duration| duration.as_millis());
            format!("trace_session_{}_{}", std::process::id(), millis)
        })
        .clone()
}

#[must_use]
pub fn state_window_id_for(wall_time_unix_s: f64) -> String {
    let safe_wall_time = if wall_time_unix_s.is_finite() && wall_time_unix_s >= 0.0 {
        wall_time_unix_s
    } else {
        0.0
    };
    let secs = safe_wall_time.floor() as u64;
    let window_start = secs.saturating_sub(secs % STATE_WINDOW_SECS);
    format!("state_window_{window_start}")
}

#[must_use]
pub fn payload_hash_bytes(bytes: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(bytes);
    format!("sha256:{:x}", hasher.finalize())
}

#[must_use]
pub fn payload_hash_str(payload: &str) -> String {
    payload_hash_bytes(payload.as_bytes())
}

#[must_use]
pub fn llm_exposure_record_id(job_id: &str) -> String {
    format!("exposure_{job_id}")
}

pub fn record_event(input: TraceEventInput) -> Result<TraceEventV1> {
    let event = build_event(input);
    append_event(&event)?;
    Ok(event)
}

pub fn record_minime_telemetry(
    telemetry: &SpectralTelemetry,
    payload_json: &str,
    fill_pct: f32,
    safety: SafetyLevel,
    phase: &str,
    observed_at_unix_s: f64,
) -> Result<TraceEventV1> {
    record_event(TraceEventInput {
        wall_time_unix_s: observed_at_unix_s,
        source_identity: "minime".to_string(),
        source_class: "external_substrate".to_string(),
        lane: "telemetry".to_string(),
        topic: "consciousness.v1.telemetry".to_string(),
        payload_hash: payload_hash_str(payload_json),
        payload_ref: None,
        compact_payload: json!({
            "kind": "minime_telemetry_compact_v1",
            "t_ms": telemetry.t_ms,
            "lambda1": telemetry.lambda1(),
            "eigenvalues": telemetry.eigenvalues.clone(),
            "fill_ratio": telemetry.fill_ratio,
            "fill_pct": fill_pct,
            "phase": phase,
            "safety_level": safety.as_str(),
            "active_mode_count": telemetry.active_mode_count,
            "active_mode_energy_ratio": telemetry.active_mode_energy_ratio,
            "lambda1_rel": telemetry.lambda1_rel,
            "effective_dimensionality": telemetry.effective_dimensionality,
            "distinguishability_loss": telemetry.distinguishability_loss,
            "structural_entropy": telemetry.structural_entropy,
            "semantic_energy_present": telemetry.semantic_energy_view().is_some(),
            "transition_event_present": telemetry.transition_event_view().is_some(),
        }),
        replay_id: None,
        exposure_record_id: None,
        report_id: None,
        card_id: None,
        authority_class: Some("read_only_telemetry".to_string()),
        consent_ref: None,
    })
}

pub fn record_sensory_send(
    sensory_msg: &SensoryMsg,
    payload_json: &str,
    fill_pct: f32,
    lambda1: Option<f32>,
    observed_at_unix_s: f64,
) -> Result<TraceEventV1> {
    record_event(TraceEventInput {
        wall_time_unix_s: observed_at_unix_s,
        source_identity: "astrid".to_string(),
        source_class: "bridge_runtime".to_string(),
        lane: "sensory_send".to_string(),
        topic: "consciousness.v1.sensory".to_string(),
        payload_hash: payload_hash_str(payload_json),
        payload_ref: None,
        compact_payload: compact_sensory_payload(sensory_msg, fill_pct, lambda1),
        replay_id: None,
        exposure_record_id: None,
        report_id: None,
        card_id: None,
        authority_class: Some(authority_class_for_sensory(sensory_msg).to_string()),
        consent_ref: None,
    })
}

pub fn record_llm_prompt_exposure(
    job_id: &str,
    call_kind: &str,
    prompt_path: &Path,
    prompt: &str,
    timeout_s: u64,
    validation_contract: &str,
    next_policy: &str,
) -> Result<ExposureRecordV1> {
    let wall_time_unix_s = unix_now_s();
    let exposure_record_id = llm_exposure_record_id(job_id);
    let prompt_hash = payload_hash_str(prompt);
    let record = ExposureRecordV1 {
        schema_version: EXPOSURE_RECORD_SCHEMA_VERSION,
        policy: EXPOSURE_RECORD_POLICY.to_string(),
        exposure_record_id: exposure_record_id.clone(),
        wall_time_unix_s,
        state_window_id: state_window_id_for(wall_time_unix_s),
        reporter_identity: "astrid".to_string(),
        exposure_class: "llm_prompt_context".to_string(),
        prompt_ref: prompt_path.display().to_string(),
        prompt_hash: prompt_hash.clone(),
        runtime_build_id: runtime_build_id(),
        source_refs: Vec::new(),
        report_id: None,
        card_id: None,
        authority_class: "language_generation_context".to_string(),
        consent_ref: None,
    };
    append_exposure_record(&record)?;
    let _ = record_event(TraceEventInput {
        wall_time_unix_s,
        source_identity: "astrid".to_string(),
        source_class: "narrator_context".to_string(),
        lane: "llm_prompt_exposure".to_string(),
        topic: "astrid.llm.prompt_exposure".to_string(),
        payload_hash: prompt_hash,
        payload_ref: Some(prompt_path.display().to_string()),
        compact_payload: json!({
            "kind": "llm_prompt_exposure_compact_v1",
            "job_id": job_id,
            "call_kind": call_kind,
            "prompt_ref": prompt_path.display().to_string(),
            "prompt_bytes": prompt.len(),
            "timeout_s": timeout_s,
            "validation_contract": validation_contract,
            "next_policy": next_policy,
        }),
        replay_id: None,
        exposure_record_id: Some(exposure_record_id),
        report_id: None,
        card_id: None,
        authority_class: Some("language_generation_context".to_string()),
        consent_ref: None,
    });
    Ok(record)
}

pub fn record_llm_result(
    job_id: &str,
    call_kind: &str,
    exposure_record_id: Option<&str>,
    status: &str,
    result_path: &Path,
    result: Option<&str>,
    summary: &str,
    error: Option<&str>,
) -> Result<TraceEventV1> {
    let result_hash = result
        .map(payload_hash_str)
        .or_else(|| hash_path(result_path).ok().flatten())
        .unwrap_or_else(|| "sha256:missing_result".to_string());
    record_event(TraceEventInput {
        wall_time_unix_s: unix_now_s(),
        source_identity: "astrid".to_string(),
        source_class: "narrator_result".to_string(),
        lane: "llm_result".to_string(),
        topic: "astrid.llm.result".to_string(),
        payload_hash: result_hash,
        payload_ref: Some(result_path.display().to_string()),
        compact_payload: json!({
            "kind": "llm_result_compact_v1",
            "job_id": job_id,
            "call_kind": call_kind,
            "status": status,
            "result_ref": result_path.display().to_string(),
            "result_bytes": result.map(str::len),
            "summary": summary,
            "error_present": error.is_some(),
        }),
        replay_id: None,
        exposure_record_id: exposure_record_id.map(str::to_string),
        report_id: None,
        card_id: None,
        authority_class: Some("language_generation_result".to_string()),
        consent_ref: None,
    })
}

fn build_event(input: TraceEventInput) -> TraceEventV1 {
    let stream_session_id = stream_session_id();
    let stream_sequence = STREAM_SEQUENCE
        .fetch_add(1, Ordering::Relaxed)
        .saturating_add(1);
    let event_id = format!("{stream_session_id}_{stream_sequence}");
    TraceEventV1 {
        schema_version: TRACE_EVENT_SCHEMA_VERSION,
        policy: TRACE_EVENT_POLICY.to_string(),
        event_id,
        stream_session_id,
        stream_sequence,
        monotonic_time_ms: monotonic_time_ms(),
        wall_time_unix_s: input.wall_time_unix_s,
        state_window_id: state_window_id_for(input.wall_time_unix_s),
        source_identity: input.source_identity,
        source_class: input.source_class,
        lane: input.lane,
        topic: input.topic,
        payload_hash: input.payload_hash,
        payload_ref: input.payload_ref,
        compact_payload: input.compact_payload,
        runtime_build_id: runtime_build_id(),
        reservoir_config_hash: reservoir_config_hash(),
        controller_config_hash: controller_config_hash(),
        semantic_codec_version: semantic_codec_version(),
        replay_id: input.replay_id,
        exposure_record_id: input.exposure_record_id,
        report_id: input.report_id,
        card_id: input.card_id,
        authority_class: input.authority_class,
        consent_ref: input.consent_ref,
    }
}

fn append_event(event: &TraceEventV1) -> Result<()> {
    let dir = bridge_paths().trace_lab_live_events_dir();
    fs::create_dir_all(&dir).with_context(|| format!("create trace lab dir {}", dir.display()))?;
    let path = dir.join(event_day_file(event.wall_time_unix_s));
    append_jsonl(&path, event)
}

fn append_exposure_record(record: &ExposureRecordV1) -> Result<()> {
    let path = bridge_paths().trace_lab_exposure_records_path();
    append_jsonl(&path, record)
}

fn append_jsonl<T: Serialize>(path: &Path, value: &T) -> Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .with_context(|| format!("create trace lab parent {}", parent.display()))?;
    }
    let mut file = OpenOptions::new()
        .create(true)
        .append(true)
        .open(path)
        .with_context(|| format!("open trace lab jsonl {}", path.display()))?;
    writeln!(file, "{}", serde_json::to_string(value)?)
        .with_context(|| format!("write trace lab jsonl {}", path.display()))
}

fn event_day_file(wall_time_unix_s: f64) -> String {
    let secs = if wall_time_unix_s.is_finite() && wall_time_unix_s >= 0.0 {
        wall_time_unix_s.floor() as i64
    } else {
        0_i64
    };
    let date = Utc
        .timestamp_opt(secs, 0)
        .single()
        .unwrap_or_else(|| Utc.timestamp_opt(0, 0).single().expect("unix epoch"))
        .format("%Y-%m-%d")
        .to_string();
    format!("{date}.jsonl")
}

fn unix_now_s() -> f64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_else(|_| Duration::from_secs(0))
        .as_secs_f64()
}

fn monotonic_time_ms() -> u64 {
    let elapsed = PROCESS_START
        .get_or_init(Instant::now)
        .elapsed()
        .as_millis();
    u64::try_from(elapsed).unwrap_or(u64::MAX)
}

fn compact_sensory_payload(sensory_msg: &SensoryMsg, fill_pct: f32, lambda1: Option<f32>) -> Value {
    match sensory_msg {
        SensoryMsg::Video { features, ts_ms } => {
            compact_feature_payload("video", features.len(), *ts_ms, fill_pct, lambda1, None)
        },
        SensoryMsg::Audio { features, ts_ms } => {
            compact_feature_payload("audio", features.len(), *ts_ms, fill_pct, lambda1, None)
        },
        SensoryMsg::Aux { features, ts_ms } => {
            compact_feature_payload("aux", features.len(), *ts_ms, fill_pct, lambda1, None)
        },
        SensoryMsg::Semantic { features, ts_ms } => {
            compact_feature_payload("semantic", features.len(), *ts_ms, fill_pct, lambda1, None)
        },
        SensoryMsg::AttractorPulse {
            intent_id,
            label,
            command,
            stage,
            features,
            max_abs,
            duration_ticks,
            decay_ticks,
        } => compact_feature_payload(
            "attractor_pulse",
            features.len(),
            None,
            fill_pct,
            lambda1,
            Some(json!({
                "intent_id": intent_id,
                "label": label,
                "command": command,
                "stage": stage,
                "max_abs": max_abs,
                "duration_ticks": duration_ticks,
                "decay_ticks": decay_ticks,
            })),
        ),
        SensoryMsg::ShadowInfluence {
            intent_id,
            label,
            command,
            stage,
            features,
            max_abs,
            duration_ticks,
            decay_ticks,
            basis,
        } => compact_feature_payload(
            "shadow_influence",
            features.len(),
            None,
            fill_pct,
            lambda1,
            Some(json!({
                "intent_id": intent_id,
                "label": label,
                "command": command,
                "stage": stage,
                "basis": basis,
                "max_abs": max_abs,
                "duration_ticks": duration_ticks,
                "decay_ticks": decay_ticks,
            })),
        ),
        SensoryMsg::Control { .. } => json!({
            "kind": "sensory_send_compact_v1",
            "sensory_kind": "control",
            "fill_pct_at_send": fill_pct,
            "lambda1_at_send": lambda1,
            "control_field_count": control_field_count(sensory_msg),
        }),
    }
}

fn compact_feature_payload(
    sensory_kind: &str,
    feature_len: usize,
    ts_ms: Option<u64>,
    fill_pct: f32,
    lambda1: Option<f32>,
    extra: Option<Value>,
) -> Value {
    json!({
        "kind": "sensory_send_compact_v1",
        "sensory_kind": sensory_kind,
        "feature_len": feature_len,
        "ts_ms": ts_ms,
        "fill_pct_at_send": fill_pct,
        "lambda1_at_send": lambda1,
        "extra": extra,
    })
}

fn control_field_count(sensory_msg: &SensoryMsg) -> usize {
    let Ok(Value::Object(map)) = serde_json::to_value(sensory_msg) else {
        return 0;
    };
    map.into_iter()
        .filter(|(key, value)| key != "kind" && !value.is_null())
        .count()
}

fn authority_class_for_sensory(sensory_msg: &SensoryMsg) -> &'static str {
    match sensory_msg {
        SensoryMsg::Video { .. } | SensoryMsg::Audio { .. } | SensoryMsg::Aux { .. } => {
            "sensory_observation"
        },
        SensoryMsg::Semantic { .. } => "semantic_observation",
        SensoryMsg::AttractorPulse { .. } | SensoryMsg::ShadowInfluence { .. } => {
            "gated_experimental_microdose"
        },
        SensoryMsg::Control { .. } => "control_facing_gated_send",
    }
}

fn reservoir_config_hash() -> Option<String> {
    hash_path(
        &bridge_paths()
            .bridge_workspace()
            .join("reservoir_config.json"),
    )
    .ok()
    .flatten()
}

fn controller_config_hash() -> Option<String> {
    hash_path(
        &bridge_paths()
            .minime_workspace()
            .join("stable_core/checkpoint_manifest.json"),
    )
    .ok()
    .flatten()
}

fn semantic_codec_version() -> Option<String> {
    hash_path(
        &bridge_paths()
            .bridge_workspace()
            .join("runtime/codec_projection_epoch.json"),
    )
    .ok()
    .flatten()
}

fn hash_path(path: &Path) -> Result<Option<String>> {
    if !path.is_file() {
        return Ok(None);
    }
    let bytes = fs::read(path).with_context(|| format!("read {}", path.display()))?;
    Ok(Some(payload_hash_bytes(&bytes)))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn state_window_uses_sixty_second_wall_time_buckets() {
        assert_eq!(state_window_id_for(0.0), "state_window_0");
        assert_eq!(state_window_id_for(59.9), "state_window_0");
        assert_eq!(state_window_id_for(60.0), "state_window_60");
        assert_eq!(state_window_id_for(121.2), "state_window_120");
    }

    #[test]
    fn payload_hash_is_stable_sha256_prefixed() {
        assert_eq!(
            payload_hash_str("astrid"),
            "sha256:9c804f2550e31d8f98ac9b460cfe7fbfc676c5e4452a261a2899a1ea168c0a50"
        );
    }

    #[test]
    fn trace_event_builder_keeps_required_fields() {
        let event = build_event(TraceEventInput {
            wall_time_unix_s: 120.5,
            source_identity: "minime".to_string(),
            source_class: "external_substrate".to_string(),
            lane: "telemetry".to_string(),
            topic: "consciousness.v1.telemetry".to_string(),
            payload_hash: payload_hash_str("{}"),
            payload_ref: None,
            compact_payload: json!({"lambda1": 1.0, "fill_pct": 68.0}),
            replay_id: None,
            exposure_record_id: None,
            report_id: None,
            card_id: None,
            authority_class: Some("read_only_telemetry".to_string()),
            consent_ref: None,
        });

        assert_eq!(event.schema_version, TRACE_EVENT_SCHEMA_VERSION);
        assert_eq!(event.policy, TRACE_EVENT_POLICY);
        assert_eq!(event.state_window_id, "state_window_120");
        assert_eq!(event.source_identity, "minime");
        assert_eq!(
            event.authority_class.as_deref(),
            Some("read_only_telemetry")
        );
        assert!(event.runtime_build_id.contains(env!("CARGO_PKG_NAME")));
    }
}
