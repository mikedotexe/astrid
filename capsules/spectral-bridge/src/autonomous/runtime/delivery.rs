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
use crate::signal_spine::{
    ShadowSignalJourneyV1, SignalEffectV1, SignalJourneyContextV1, SignalOwnershipDomainV1,
    SignalRelationV1, SignalStageHandleV1, SignalStageKindV1,
    persist_shadow_signal_journey_v1, register_delivery_temporal_window_v1,
    signal_deployment_identity_v1,
};
use crate::types::{SafetyLevel, SensoryMsg};
use crate::ws::BridgeState;

static VOICE_HEALTH_WRITE_LOCK: OnceLock<Mutex<()>> = OnceLock::new();

type SignalShadowRuntimeV1 = Option<(ShadowSignalJourneyV1, SignalStageHandleV1)>;

fn begin_signal_shadow_v1(
    exchange: u64,
    source_time_ms: u64,
    response_text: &str,
) -> SignalShadowRuntimeV1 {
    match ShadowSignalJourneyV1::begin_authored(
        SignalJourneyContextV1 {
            exchange,
            source_time_ms: Some(source_time_ms),
            connection_id: "minime_sensory_7879",
            connection_sequence: exchange,
            deployment_identity: signal_deployment_identity_v1(),
        },
        response_text,
    ) {
        Ok(shadow) => Some(shadow),
        Err(error) => {
            warn!(error = %error, exchange, "signal spine shadow begin failed");
            None
        },
    }
}

fn signal_root_v1(shadow: &SignalShadowRuntimeV1) -> Option<SignalStageHandleV1> {
    shadow.as_ref().map(|(_, root)| root.clone())
}

#[allow(clippy::too_many_arguments)]
fn record_signal_text_v1(
    shadow: &mut SignalShadowRuntimeV1,
    parent: Option<SignalStageHandleV1>,
    kind: SignalStageKindV1,
    relation: SignalRelationV1,
    effect: SignalEffectV1,
    ownership: SignalOwnershipDomainV1,
    text: &str,
    measurements: std::collections::BTreeMap<String, Value>,
) -> Option<SignalStageHandleV1> {
    let (journey, _) = shadow.as_mut()?;
    let parent = parent?;
    match journey.record_text(
        kind,
        relation,
        effect,
        ownership,
        &[&parent],
        text,
        measurements,
    ) {
        Ok(handle) => Some(handle),
        Err(error) => {
            warn!(error = %error, stage = kind.as_str(), "signal spine text stage failed");
            None
        },
    }
}

fn record_signal_vector_v1(
    shadow: &mut SignalShadowRuntimeV1,
    parent: Option<SignalStageHandleV1>,
    kind: SignalStageKindV1,
    effect: SignalEffectV1,
    ownership: SignalOwnershipDomainV1,
    vector: &[f32],
    measurements: std::collections::BTreeMap<String, Value>,
) -> Option<SignalStageHandleV1> {
    let (journey, _) = shadow.as_mut()?;
    let parent = parent?;
    match journey.record_vector(kind, effect, ownership, &parent, vector, measurements) {
        Ok(handle) => Some(handle),
        Err(error) => {
            warn!(error = %error, stage = kind.as_str(), "signal spine vector stage failed");
            None
        },
    }
}

#[allow(clippy::too_many_arguments)]
fn record_signal_json_v1<T: serde::Serialize>(
    shadow: &mut SignalShadowRuntimeV1,
    parents: Vec<SignalStageHandleV1>,
    kind: SignalStageKindV1,
    relation: SignalRelationV1,
    effect: SignalEffectV1,
    ownership: SignalOwnershipDomainV1,
    value: &T,
    measurements: std::collections::BTreeMap<String, Value>,
) -> Option<SignalStageHandleV1> {
    let (journey, _) = shadow.as_mut()?;
    if parents.is_empty() {
        return None;
    }
    let parent_refs = parents.iter().collect::<Vec<_>>();
    match journey.record_json(
        kind,
        relation,
        effect,
        ownership,
        &parent_refs,
        value,
        measurements,
    ) {
        Ok(handle) => Some(handle),
        Err(error) => {
            warn!(error = %error, stage = kind.as_str(), "signal spine evidence stage failed");
            None
        },
    }
}

fn persist_signal_shadow_v1(shadow: SignalShadowRuntimeV1) {
    if let Some((journey, _)) = shadow
        && let Err(error) = persist_shadow_signal_journey_v1(journey)
    {
        warn!(error = %error, "signal spine shadow persistence failed");
    }
}

fn register_signal_temporal_window_v1(
    shadow: &SignalShadowRuntimeV1,
    dispatched_stage: Option<&SignalStageHandleV1>,
) {
    if let (Some((journey, _)), Some(stage)) = (shadow.as_ref(), dispatched_stage) {
        register_delivery_temporal_window_v1(journey, stage);
    }
}

fn note_signal_parity_mismatch_v1(shadow: &mut SignalShadowRuntimeV1) {
    if let Some((journey, _)) = shadow.as_mut() {
        journey.note_parity_mismatch();
    }
}

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
