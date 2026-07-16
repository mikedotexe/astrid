pub(crate) use self::next_action::{
    action_preflight_report, canonicalize_next_action_text, extract_residue_from_next_action,
    extract_search_topic, parse_next_action,
};
pub(crate) use self::readiness::read_source_status as read_astrid_source_status;
pub use self::reservoir::configure_reservoir_service;
use self::state::{ConversationState, Mode, SpectralSample, WitnessDepthV1, choose_mode};

const fn reflective_mode_for_relational_reply(mode: Mode) -> bool {
    matches!(
        mode,
        Mode::Mirror
            | Mode::Witness
            | Mode::Introspect
            | Mode::Daydream
            | Mode::Aspiration
            | Mode::Contemplate
    )
}
use crate::agency;
use crate::codec::{
    NAMED_CODEC_DIMS, apply_spectral_feedback, apply_spectral_feedback_with_report, blend_warmth,
    codec_delivery_fidelity_v1, codec_vibrancy_continuity_v1, craft_warmth_vector,
    cross_spectral_friction_review_v1, encode_text, interpret_spectral, legacy_warmth_mapping_v1,
};
use crate::condition_metrics;
use crate::db::BridgeDb;
use crate::journal::{
    read_local_journal_body_for_continuity, read_remote_journal_body, scan_remote_journal_dir,
};
use crate::managed_dir;
use crate::memory::{self, RemoteMemorySummary};
use crate::paths::bridge_paths;
use crate::rescue_policy::{self, STABLE_CORE_TARGET_FILL_PCT};
use crate::types::{SafetyLevel, SensoryMsg};
use crate::ws::BridgeState;

static VOICE_HEALTH_WRITE_LOCK: OnceLock<Mutex<()>> = OnceLock::new();

fn persist_codec_delivery_fidelity_v1(
    minime_workspace: &Path,
    delivery_record: &Value,
) -> std::io::Result<PathBuf> {
    let runtime = minime_workspace.join("runtime");
    fs::create_dir_all(&runtime)?;
    let path = runtime.join("codec_delivery_fidelity_v1.json");
    let payload = serde_json::to_vec_pretty(delivery_record).map_err(std::io::Error::other)?;
    fs::write(&path, payload)?;
    Ok(path)
}

fn blocked_codec_delivery_record_v1(
    exchange: u64,
    chunk_index: usize,
    chunk_total: u32,
    blocked_reason: &str,
    feedback_overflow_report: Option<&crate::codec::CodecOverflowReportV1>,
    cross_spectral_friction_review: &crate::codec::CrossSpectralFrictionReviewV1,
) -> Value {
    let candidate_delivery_review = json!({
        "policy": "codec_delivery_fidelity_v1",
        "delivery_state": "blocked_before_send",
        "actual_delivery_available": false,
        "sent_vector_available": false,
        "blocked_reason": blocked_reason,
        "live_vector_write": false,
        "live_gain_write": false,
        "authority": "read_only_candidate_delivery_preflight_not_sent_vector_or_live_codec_change",
    });
    json!({
        "updated_at_unix_ms": chrono::Utc::now().timestamp_millis(),
        "exchange": exchange,
        "chunk_index": chunk_index,
        "chunk_total": chunk_total,
        "delivery_state": "blocked_before_send",
        "actual_delivery_available": false,
        "sent_vector_available": false,
        "blocked_reason": blocked_reason,
        "codec_delivery_fidelity_v1": Value::Null,
        "candidate_delivery_review_v1": candidate_delivery_review,
        "feedback_overflow_report_v1": feedback_overflow_report,
        "cross_spectral_friction_review_v1": cross_spectral_friction_review,
        "right_to_ignore": true,
        "live_vector_write": false,
        "live_gain_write": false,
        "live_eligible_now": false,
        "auto_approved": false,
        "grants_approval": false,
        "authority": "read_only_blocked_candidate_evidence_not_sent_vector_gain_ceiling_transport_or_policy_change",
    })
}
