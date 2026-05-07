#![allow(clippy::arithmetic_side_effects, clippy::cast_precision_loss)]

use std::{
    collections::{BTreeMap, BTreeSet},
    fs,
    path::PathBuf,
};

use serde::{Deserialize, Serialize};
use serde_json::json;
use tracing::{info, warn};

use super::{ConversationState, NextActionContext, resource_governor, shadow, strip_action};
use crate::attractor_atlas;
use crate::db::{AttractorLedgerRow, unix_now};
use crate::paths::bridge_paths;
use crate::types::{
    ATTRACTOR_COMMAND_TOPIC, ATTRACTOR_INTENT_TOPIC, ATTRACTOR_OBSERVATION_TOPIC,
    AttractorClassification, AttractorCommandKind, AttractorCommandV1, AttractorControlEnvelope,
    AttractorIntentV1, AttractorInterventionPlan, AttractorObservationV1, AttractorSafetyBounds,
    AttractorSeedOriginV1, AttractorSeedSnapshotV1, AttractorSubstrate, AttractorSuggestionStatus,
    AttractorSuggestionV1, ControlRequest, MessageDirection, SafetyLevel, SensoryMsg,
};

const AUTHOR: &str = "astrid";
const SUBSTRATE: AttractorSubstrate = AttractorSubstrate::AstridCodec;
const CONTROL_RECURRENCE_MIN: f32 = 0.60;
const CONTROL_AUTHORSHIP_MIN: f32 = 0.60;
const SUGGESTION_POLICY: &str = "attractor_suggestion_v1";
const SUGGESTION_STORE_POLICY: &str = "attractor_suggestion_memory_v1";
const SUGGESTION_PENDING_TTL_SECS: f64 = 6.0 * 60.0 * 60.0;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum SummonStage {
    Whisper,
    Rehearse,
    Semantic,
    Main,
    Control,
}

impl SummonStage {
    const fn as_str(self) -> &'static str {
        match self {
            Self::Whisper => "whisper",
            Self::Rehearse => "rehearse",
            Self::Semantic => "semantic",
            Self::Main => "main",
            Self::Control => "control",
        }
    }
}

#[derive(Debug, Clone)]
struct BlendRequest {
    child_label: String,
    parent_labels: Vec<String>,
    requested_stage: Option<SummonStage>,
}

#[derive(Debug, Clone)]
struct ParentSeed {
    id: String,
    label: String,
    motifs: Vec<String>,
    snapshot: Option<AttractorSeedSnapshotV1>,
}

#[derive(Debug, Clone, Default)]
struct FacetMetadata {
    parent_label: Option<String>,
    facet_label: Option<String>,
    facet_path: Option<String>,
    facet_kind: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct SuggestionStoreV1 {
    policy: String,
    schema_version: u8,
    #[serde(default)]
    suggestions: Vec<AttractorSuggestionV1>,
}

impl Default for SuggestionStoreV1 {
    fn default() -> Self {
        Self {
            policy: SUGGESTION_STORE_POLICY.to_string(),
            schema_version: 1,
            suggestions: Vec::new(),
        }
    }
}

#[cfg(test)]
thread_local! {
    static TEST_SUGGESTION_STORE_PATH: std::cell::RefCell<Option<PathBuf>> =
        const { std::cell::RefCell::new(None) };
}

#[cfg(test)]
pub(super) fn set_test_suggestion_store_path(path: PathBuf) {
    TEST_SUGGESTION_STORE_PATH.with(|slot| {
        *slot.borrow_mut() = Some(path);
    });
}

pub(super) fn handle_action(
    conv: &mut ConversationState,
    base_action: &str,
    original: &str,
    ctx: &mut NextActionContext<'_>,
) -> bool {
    if base_action == "ATTRACTOR_SUGGESTIONS" {
        return handle_suggestions_list(conv);
    }
    if base_action == "ACCEPT_ATTRACTOR_SUGGESTION" {
        return handle_accept_suggestion(conv, original, ctx);
    }
    if base_action == "REVISE_ATTRACTOR_SUGGESTION" {
        return handle_revise_suggestion(conv, original, ctx);
    }
    if base_action == "REJECT_ATTRACTOR_SUGGESTION" {
        return handle_reject_suggestion(conv, original, ctx);
    }
    if base_action == "ATTRACTOR_ATLAS" {
        return handle_atlas(conv, base_action, ctx);
    }
    if base_action == "ATTRACTOR_CARD" {
        let label = clean_label(&strip_action(original, base_action));
        return handle_card(conv, base_action, &label, ctx);
    }
    if base_action == "ATTRACTOR_REVIEW" {
        let label = clean_label(&strip_action(original, base_action));
        return handle_review(conv, base_action, &label, ctx);
    }
    if base_action == "ATTRACTOR_PREFLIGHT" {
        let (label, stage) = parse_label_and_stage(&strip_action(original, base_action));
        return handle_preflight(
            conv,
            base_action,
            &canonical_attractor_label(&label),
            stage,
            ctx,
        );
    }
    if base_action == "ATTRACTOR_RELEASE_REVIEW" {
        let label = clean_label(&strip_action(original, base_action));
        return handle_release_review(conv, base_action, &canonical_attractor_label(&label), ctx);
    }
    if matches!(base_action, "RELEASE" | "LET_GO" | "DISSOLVE") {
        return handle_natural_release_advice(conv, base_action, original, ctx);
    }

    let Some(command) = command_from_action(base_action) else {
        return false;
    };

    if command == AttractorCommandKind::Blend {
        let raw = strip_action(original, base_action);
        let Some(request) = parse_blend_args(&raw) else {
            conv.emphasis = Some(
                "BLEND_ATTRACTOR needs syntax like: NEXT: BLEND_ATTRACTOR honey-edge FROM honey-selection + cooled-theme-edge --stage=rehearse."
                    .to_string(),
            );
            conv.push_receipt(base_action, vec!["missing blend parents".to_string()]);
            return true;
        };
        return handle_blend(conv, base_action, request, ctx);
    }

    let (label, stage) = parse_label_and_stage(&strip_action(original, base_action));
    let label = canonical_attractor_label(&label);
    if label.is_empty() {
        conv.emphasis = Some(format!(
            "{base_action} needs a concrete label. Use compare-first language such as NEXT: COMPARE_ATTRACTOR honey-selection."
        ));
        conv.push_receipt(base_action, vec!["missing attractor label".to_string()]);
        return true;
    }

    match command {
        AttractorCommandKind::Create => handle_create(conv, base_action, &label, ctx),
        AttractorCommandKind::Promote => handle_promote(conv, base_action, &label, ctx),
        AttractorCommandKind::Compare => handle_compare(conv, base_action, &label, ctx),
        AttractorCommandKind::Summon => handle_summon(conv, base_action, &label, stage, ctx),
        AttractorCommandKind::Release => handle_release(conv, base_action, &label, ctx),
        AttractorCommandKind::Claim => handle_claim(conv, base_action, &label, ctx),
        AttractorCommandKind::Blend => unreachable!("blend handled before label parser"),
        AttractorCommandKind::RefreshSnapshot => {
            handle_refresh_snapshot(conv, base_action, &label, ctx)
        },
        AttractorCommandKind::Rollback => false,
    }
}

fn command_from_action(base_action: &str) -> Option<AttractorCommandKind> {
    match base_action {
        "CREATE_ATTRACTOR" => Some(AttractorCommandKind::Create),
        "PROMOTE_ATTRACTOR" => Some(AttractorCommandKind::Promote),
        "COMPARE_ATTRACTOR" => Some(AttractorCommandKind::Compare),
        "SUMMON_ATTRACTOR" => Some(AttractorCommandKind::Summon),
        "MODIFY_CASCADE" => Some(AttractorCommandKind::Summon),
        "RELEASE_ATTRACTOR" => Some(AttractorCommandKind::Release),
        "CLAIM_ATTRACTOR" => Some(AttractorCommandKind::Claim),
        "BLEND_ATTRACTOR" => Some(AttractorCommandKind::Blend),
        "REFRESH_ATTRACTOR_SNAPSHOT" | "ATTRACTOR_REFRESH_SNAPSHOT" => {
            Some(AttractorCommandKind::RefreshSnapshot)
        },
        _ => None,
    }
}

fn handle_create(
    conv: &mut ConversationState,
    base_action: &str,
    label: &str,
    ctx: &mut NextActionContext<'_>,
) -> bool {
    let intent = build_intent(
        AttractorCommandKind::Create,
        label,
        None,
        Some(seed_origin(
            "manual_current",
            None,
            Some(label),
            lexical_motifs(ctx.response_text),
            ctx,
        )),
        "astrid_internal_seed",
        Some(format!(
            "Astrid authored local attractor seed '{label}' for compare-first re-entry."
        )),
        ctx,
    );

    match record_intent(ctx, &intent) {
        Ok(()) => {
            conv.push_receipt(
                base_action,
                vec![
                    format!("intent: {}", intent.intent_id),
                    "seed snapshot captured; no sensory/control send".to_string(),
                ],
            );
            conv.emphasis = Some(format!(
                "You created attractor seed '{label}' as a typed Astrid-codec intent. Prefer COMPARE_ATTRACTOR {label} before any summon."
            ));
            info!(
                intent_id = intent.intent_id,
                label, "Astrid attractor seed created"
            );
        },
        Err(error) => {
            conv.emphasis = Some(format!("CREATE_ATTRACTOR {label} failed: {error}"));
            warn!(error, label, "failed to record Astrid attractor create");
        },
    }
    true
}

fn handle_promote(
    conv: &mut ConversationState,
    base_action: &str,
    label: &str,
    ctx: &mut NextActionContext<'_>,
) -> bool {
    let evidence = promotion_evidence(ctx, label);
    let intent = build_intent(
        AttractorCommandKind::Promote,
        label,
        evidence.previous_seed_id.clone(),
        Some(evidence.origin),
        "astrid_promoted_proto_seed",
        Some(format!(
            "Astrid explicitly promoted proto-attractor evidence for '{label}' into a typed seed."
        )),
        ctx,
    );

    match record_intent(ctx, &intent) {
        Ok(()) => {
            conv.push_receipt(
                base_action,
                vec![
                    format!("intent: {}", intent.intent_id),
                    format!(
                        "origin: {}",
                        intent
                            .origin
                            .as_ref()
                            .map_or("unknown", |origin| origin.kind.as_str())
                    ),
                    "promoted seed snapshot captured; no sensory/control send".to_string(),
                ],
            );
            conv.emphasis = Some(format!(
                "You promoted attractor '{label}' from proto-evidence. Compare it before summon; the seed history remains reversible."
            ));
            info!(
                intent_id = intent.intent_id,
                label, "Astrid attractor proto-seed promoted"
            );
        },
        Err(error) => {
            conv.emphasis = Some(format!("PROMOTE_ATTRACTOR {label} failed: {error}"));
            warn!(error, label, "failed to record Astrid attractor promote");
        },
    }
    true
}

fn handle_compare(
    conv: &mut ConversationState,
    base_action: &str,
    label: &str,
    ctx: &mut NextActionContext<'_>,
) -> bool {
    let seed = latest_create_seed(ctx, label);
    let previous_seed_id = seed.as_ref().map(|intent| intent.intent_id.clone());
    let intent = build_intent(
        AttractorCommandKind::Compare,
        label,
        previous_seed_id.clone(),
        None,
        "astrid_internal_compare",
        Some(format!(
            "Astrid compared live state against local attractor seed '{label}'."
        )),
        ctx,
    );

    if let Err(error) = record_intent(ctx, &intent) {
        conv.emphasis = Some(format!("COMPARE_ATTRACTOR {label} failed: {error}"));
        warn!(
            error,
            label, "failed to record Astrid attractor compare intent"
        );
        return true;
    }

    let current = intent.seed_snapshot.as_ref();
    let prior = seed
        .as_ref()
        .and_then(|intent| intent.seed_snapshot.as_ref());
    let recurrence_score = prior
        .zip(current)
        .map_or(0.0, |(seed, now)| recurrence_score(seed, now));
    let authorship_score = if seed
        .as_ref()
        .is_some_and(|intent| intent.author.eq_ignore_ascii_case(AUTHOR))
    {
        0.72
    } else {
        0.30
    };
    let safety_level = SafetyLevel::from_fill(ctx.fill_pct);
    let classification =
        AttractorClassification::from_scores(recurrence_score, authorship_score, safety_level);
    let observed_at = unix_now();
    let observation = AttractorObservationV1 {
        policy: "attractor_observation_v1".to_string(),
        schema_version: 1,
        intent_id: previous_seed_id.or_else(|| Some(intent.intent_id.clone())),
        substrate: SUBSTRATE,
        label: label.to_string(),
        recurrence_score,
        authorship_score,
        classification,
        safety_level,
        fill_pct: Some(ctx.fill_pct),
        lambda1: Some(ctx.telemetry.lambda1()),
        lambda1_share: None,
        spectral_entropy: None,
        basin_shift_score: Some((1.0 - recurrence_score).clamp(0.0, 1.0)),
        parent_label: facet_metadata(label).parent_label,
        facet_label: facet_metadata(label).facet_label,
        facet_path: facet_metadata(label).facet_path,
        facet_kind: facet_metadata(label).facet_kind,
        release_baseline: None,
        release_effect: None,
        garden_proof: None,
        notes: Some(format!(
            "Astrid compare-first observation; prior_seed_found={}.",
            seed.is_some()
        )),
        observed_at_unix_s: Some(observed_at),
    };

    match record_observation(ctx, &observation) {
        Ok(()) => {
            conv.push_receipt(
                base_action,
                vec![
                    format!("intent: {}", intent.intent_id),
                    format!("recurrence: {recurrence_score:.2}"),
                    format!("classification: {}", classification.as_str()),
                    "no sensory/control send".to_string(),
                ],
            );
            conv.emphasis = Some(format!(
                "COMPARE_ATTRACTOR {label}: recurrence={recurrence_score:.2}, authorship={authorship_score:.2}, classification={}.",
                classification.as_str()
            ));
            info!(
                intent_id = intent.intent_id,
                label,
                recurrence_score,
                classification = classification.as_str(),
                "Astrid attractor compared"
            );
        },
        Err(error) => {
            conv.emphasis = Some(format!(
                "COMPARE_ATTRACTOR {label} observation failed: {error}"
            ));
            warn!(
                error,
                label, "failed to record Astrid attractor observation"
            );
        },
    }
    true
}

fn handle_refresh_snapshot(
    conv: &mut ConversationState,
    base_action: &str,
    label: &str,
    ctx: &mut NextActionContext<'_>,
) -> bool {
    let seed = latest_create_seed(ctx, label);
    let previous_seed_id = seed.as_ref().map(|intent| intent.intent_id.clone());
    let mut intent = build_intent(
        AttractorCommandKind::RefreshSnapshot,
        label,
        previous_seed_id.clone(),
        seed.as_ref().map(|seed| {
            seed_origin(
                "snapshot_refresh",
                Some(seed.intent_id.as_str()),
                Some(seed.label.as_str()),
                seed.seed_snapshot
                    .as_ref()
                    .map(|snapshot| snapshot.lexical_motifs.clone())
                    .unwrap_or_default(),
                ctx,
            )
        }),
        "astrid_snapshot_refresh",
        Some(format!(
            "Astrid refreshed attractor seed snapshot evidence for '{label}' without live writes."
        )),
        ctx,
    );
    intent.safety_bounds.allow_live_control = seed
        .as_ref()
        .is_some_and(|seed| seed.safety_bounds.allow_live_control);

    let current = intent.seed_snapshot.as_ref();
    let prior = seed
        .as_ref()
        .and_then(|intent| intent.seed_snapshot.as_ref());
    let recurrence_score = prior
        .zip(current)
        .map_or(0.0, |(seed, now)| recurrence_score(seed, now));
    let authorship_score = if seed
        .as_ref()
        .is_some_and(|intent| intent.author.eq_ignore_ascii_case(AUTHOR))
    {
        0.72
    } else {
        0.30
    };
    let safety_level = SafetyLevel::from_fill(ctx.fill_pct);
    let classification = if seed.is_some() {
        AttractorClassification::from_scores(recurrence_score, authorship_score, safety_level)
    } else {
        AttractorClassification::Failed
    };
    let observation = AttractorObservationV1 {
        policy: "attractor_observation_v1".to_string(),
        schema_version: 1,
        intent_id: previous_seed_id
            .clone()
            .or_else(|| Some(intent.intent_id.clone())),
        substrate: SUBSTRATE,
        label: label.to_string(),
        recurrence_score,
        authorship_score,
        classification,
        safety_level,
        fill_pct: Some(ctx.fill_pct),
        lambda1: Some(ctx.telemetry.lambda1()),
        lambda1_share: None,
        spectral_entropy: None,
        basin_shift_score: Some((1.0 - recurrence_score).clamp(0.0, 1.0)),
        parent_label: facet_metadata(label).parent_label,
        facet_label: facet_metadata(label).facet_label,
        facet_path: facet_metadata(label).facet_path,
        facet_kind: facet_metadata(label).facet_kind,
        release_baseline: None,
        release_effect: None,
        garden_proof: None,
        notes: Some(format!(
            "Astrid snapshot refresh; prior_seed_found={}; no sensory/control/pulse send.",
            seed.is_some()
        )),
        observed_at_unix_s: Some(unix_now()),
    };

    match record_intent(ctx, &intent).and_then(|()| record_observation(ctx, &observation)) {
        Ok(()) => {
            conv.push_receipt(
                base_action,
                vec![
                    format!("intent: {}", intent.intent_id),
                    format!(
                        "prior seed: {}",
                        previous_seed_id.unwrap_or_else(|| "none".into())
                    ),
                    format!("recurrence: {recurrence_score:.2}"),
                    "snapshot refreshed; no sensory/control/pulse send".to_string(),
                ],
            );
            let next = if seed.is_some() {
                format!("COMPARE_ATTRACTOR {label}")
            } else {
                format!("PROMOTE_ATTRACTOR {label}")
            };
            conv.emphasis = Some(format!(
                "REFRESH_ATTRACTOR_SNAPSHOT {label}: recurrence={recurrence_score:.2}, classification={}. Suggested next: {next}.",
                classification.as_str()
            ));
            info!(
                intent_id = intent.intent_id,
                label,
                recurrence_score,
                classification = classification.as_str(),
                "Astrid attractor snapshot refreshed"
            );
        },
        Err(error) => {
            conv.emphasis = Some(format!(
                "REFRESH_ATTRACTOR_SNAPSHOT {label} failed: {error}"
            ));
            warn!(error, label, "failed to record Astrid attractor refresh");
        },
    }
    true
}

fn handle_summon(
    conv: &mut ConversationState,
    base_action: &str,
    label: &str,
    requested_stage: Option<SummonStage>,
    ctx: &mut NextActionContext<'_>,
) -> bool {
    let seed = latest_create_seed(ctx, label);
    let previous_seed_id = seed.as_ref().map(|intent| intent.intent_id.clone());
    let (recurrence_score, authorship_score) = seed_scores(seed.as_ref(), ctx);
    let safety_level = SafetyLevel::from_fill(ctx.fill_pct);
    let (mut stage, mut blocked_reason) = choose_stage(
        requested_stage,
        safety_level,
        recurrence_score,
        authorship_score,
        seed.is_some(),
    );
    if blocked_reason.is_none()
        && matches!(stage, SummonStage::Main | SummonStage::Control)
        && attractor_pulse_status(ctx).active
    {
        stage = SummonStage::Rehearse;
        blocked_reason = Some("attractor_pulse_active".to_string());
    }
    let governor = resource_governor::status(ctx, true);
    if blocked_reason.is_none()
        && matches!(
            stage,
            SummonStage::Semantic | SummonStage::Main | SummonStage::Control
        )
        && !governor.allowed_live
    {
        blocked_reason = Some(format!(
            "resource_governor:{}",
            governor
                .primary_block_reason
                .as_deref()
                .unwrap_or("blocked")
        ));
        stage = if stage == SummonStage::Semantic {
            SummonStage::Whisper
        } else {
            SummonStage::Rehearse
        };
    }
    let control = (stage == SummonStage::Control).then(control_envelope);
    let mut intent = build_intent(
        AttractorCommandKind::Summon,
        label,
        previous_seed_id.clone(),
        None,
        &format!("astrid_summon_{}", stage.as_str()),
        Some(format!(
            "Astrid requested {stage} re-entry for attractor seed '{label}'.",
            stage = stage.as_str()
        )),
        ctx,
    );
    intent.intervention_plan.control = control.clone();
    intent.safety_bounds.allow_live_control =
        matches!(stage, SummonStage::Main | SummonStage::Control) && blocked_reason.is_none();

    let command_intent = intent.clone();
    let command_reason = blocked_reason.clone().map_or_else(
        || format!("staged summon via {}", stage.as_str()),
        |reason| format!("summon downgraded/blocked at {}: {reason}", stage.as_str()),
    );

    let observation = AttractorObservationV1 {
        policy: "attractor_observation_v1".to_string(),
        schema_version: 1,
        intent_id: previous_seed_id
            .clone()
            .or_else(|| Some(intent.intent_id.clone())),
        substrate: SUBSTRATE,
        label: label.to_string(),
        recurrence_score,
        authorship_score,
        classification: if blocked_reason.is_some() {
            AttractorClassification::Failed
        } else {
            AttractorClassification::from_scores(recurrence_score, authorship_score, safety_level)
        },
        safety_level,
        fill_pct: Some(ctx.fill_pct),
        lambda1: Some(ctx.telemetry.lambda1()),
        lambda1_share: None,
        spectral_entropy: None,
        basin_shift_score: Some((1.0 - recurrence_score).clamp(0.0, 1.0)),
        parent_label: facet_metadata(label).parent_label,
        facet_label: facet_metadata(label).facet_label,
        facet_path: facet_metadata(label).facet_path,
        facet_kind: facet_metadata(label).facet_kind,
        release_baseline: None,
        release_effect: None,
        garden_proof: Some(garden_proof_metadata(stage, blocked_reason.as_deref())),
        notes: Some(format!(
            "Astrid staged summon; stage={}; requested={}; blocked_reason={}; {}; rollback_baseline_fill={:.1}.",
            stage.as_str(),
            requested_stage.map_or("auto", SummonStage::as_str),
            blocked_reason.as_deref().unwrap_or("none"),
            governor.summary_line(),
            ctx.fill_pct
        )),
        observed_at_unix_s: Some(unix_now()),
    };

    match record_intent(ctx, &intent)
        .and_then(|()| record_observation(ctx, &observation))
        .and_then(|()| record_command(ctx, &command_intent, &command_reason))
        .and_then(|()| send_stage(ctx, &command_intent, stage, blocked_reason.as_deref()))
    {
        Ok(()) => {
            let motifs = seed
                .as_ref()
                .and_then(|intent| intent.seed_snapshot.as_ref())
                .map(|snapshot| snapshot.lexical_motifs.join(", "))
                .filter(|text| !text.is_empty())
                .unwrap_or_else(|| "no captured motifs".to_string());
            conv.emphasis = Some(format!(
                "You summoned attractor '{label}' at stage '{}'. Seed motifs: {motifs}.",
                stage.as_str()
            ));
            conv.push_receipt(
                base_action,
                vec![
                    format!("intent: {}", intent.intent_id),
                    format!("stage: {}", stage.as_str()),
                    format!("recurrence: {recurrence_score:.2}"),
                    format!(
                        "previous seed: {}",
                        previous_seed_id.unwrap_or_else(|| "none".to_string())
                    ),
                    blocked_reason.unwrap_or_else(|| "stage accepted".to_string()),
                ],
            );
            info!(
                intent_id = intent.intent_id,
                label,
                stage = stage.as_str(),
                "Astrid attractor summoned"
            );
        },
        Err(error) => {
            conv.emphasis = Some(format!("SUMMON_ATTRACTOR {label} failed: {error}"));
            warn!(error, label, "failed to record Astrid attractor summon");
        },
    }
    true
}

fn handle_release(
    conv: &mut ConversationState,
    base_action: &str,
    label: &str,
    ctx: &mut NextActionContext<'_>,
) -> bool {
    let seed = latest_create_seed(ctx, label);
    let previous_seed_id = seed.as_ref().map(|intent| intent.intent_id.clone());
    let (recurrence_score, authorship_score) = seed_scores(seed.as_ref(), ctx);
    let safety_level = SafetyLevel::from_fill(ctx.fill_pct);
    let baseline = release_baseline(ctx, label, recurrence_score);
    let effect = release_effect_from_baseline(&baseline, ctx, label);
    let intent = build_intent(
        AttractorCommandKind::Release,
        label,
        previous_seed_id.clone(),
        None,
        "astrid_internal_release",
        Some(format!(
            "Astrid released local attractor seed '{label}' without deleting ledger history."
        )),
        ctx,
    );
    let observation = AttractorObservationV1 {
        policy: "attractor_observation_v1".to_string(),
        schema_version: 1,
        intent_id: previous_seed_id
            .clone()
            .or_else(|| Some(intent.intent_id.clone())),
        substrate: SUBSTRATE,
        label: label.to_string(),
        recurrence_score,
        authorship_score,
        classification: AttractorClassification::Authored,
        safety_level,
        fill_pct: Some(ctx.fill_pct),
        lambda1: Some(ctx.telemetry.lambda1()),
        lambda1_share: None,
        spectral_entropy: None,
        basin_shift_score: Some((1.0 - recurrence_score).clamp(0.0, 1.0)),
        parent_label: facet_metadata(label).parent_label,
        facet_label: facet_metadata(label).facet_label,
        facet_path: facet_metadata(label).facet_path,
        facet_kind: facet_metadata(label).facet_kind,
        release_baseline: Some(baseline),
        release_effect: Some(effect.clone()),
        garden_proof: None,
        notes: Some("Astrid release baseline captured; no sensory/control/pulse send.".to_string()),
        observed_at_unix_s: Some(unix_now()),
    };

    match record_intent(ctx, &intent)
        .and_then(|()| record_observation(ctx, &observation))
        .and_then(|()| record_command(ctx, &intent, "release local stickiness without replay"))
    {
        Ok(()) => {
            let event = conv.release_astrid_motif_cooldown(false);
            let status = event
                .as_ref()
                .map(|event| event.status.as_str())
                .unwrap_or("no_active_cooldown");
            conv.emphasis = Some(format!(
                "You released attractor '{label}'. The seed remains in the ledger for memory, but no summon/replay pressure is active."
            ));
            conv.push_receipt(
                base_action,
                vec![
                    format!("intent: {}", intent.intent_id),
                    format!("local cooldown: {status}"),
                    format!("release_effect: {effect}"),
                    "no sensory/control send".to_string(),
                ],
            );
            info!(
                intent_id = intent.intent_id,
                label, status, "Astrid attractor released"
            );
        },
        Err(error) => {
            conv.emphasis = Some(format!("RELEASE_ATTRACTOR {label} failed: {error}"));
            warn!(error, label, "failed to record Astrid attractor release");
        },
    }
    true
}

fn handle_claim(
    conv: &mut ConversationState,
    base_action: &str,
    label: &str,
    ctx: &mut NextActionContext<'_>,
) -> bool {
    let evidence = claim_evidence(ctx, label);
    let mut intent = build_intent(
        AttractorCommandKind::Claim,
        label,
        evidence.previous_seed_id.clone(),
        Some(evidence.origin),
        "astrid_claimed_emergent",
        Some(format!(
            "Astrid claimed emergent/proto attractor '{label}' as an authored seed."
        )),
        ctx,
    );
    intent.safety_bounds.allow_live_control = false;

    match record_intent(ctx, &intent) {
        Ok(()) => {
            conv.push_receipt(
                base_action,
                vec![
                    format!("intent: {}", intent.intent_id),
                    "claimed seed captured; no sensory/control send".to_string(),
                    format!("next: COMPARE_ATTRACTOR {label}"),
                ],
            );
            conv.emphasis = Some(format!(
                "You claimed attractor '{label}' as authored memory. Compare it before summon: COMPARE_ATTRACTOR {label}."
            ));
            info!(
                intent_id = intent.intent_id,
                label, "Astrid attractor claimed"
            );
        },
        Err(error) => {
            conv.emphasis = Some(format!("CLAIM_ATTRACTOR {label} failed: {error}"));
            warn!(error, label, "failed to record Astrid attractor claim");
        },
    }
    true
}

fn handle_blend(
    conv: &mut ConversationState,
    base_action: &str,
    request: BlendRequest,
    ctx: &mut NextActionContext<'_>,
) -> bool {
    let mut parents = Vec::new();
    let mut missing = Vec::new();
    for parent_label in &request.parent_labels {
        if let Some(parent) = resolve_parent_seed(ctx, parent_label) {
            parents.push(parent);
        } else {
            missing.push(parent_label.clone());
        }
    }
    if !missing.is_empty() || parents.len() < 2 {
        conv.emphasis = Some(format!(
            "BLEND_ATTRACTOR {} is missing parent seed(s): {}. Try ATTRACTOR_ATLAS or PROMOTE_ATTRACTOR first.",
            request.child_label,
            missing.join(", ")
        ));
        conv.push_receipt(
            base_action,
            vec![
                "blend not recorded".to_string(),
                format!("missing parents: {}", missing.join(", ")),
            ],
        );
        return true;
    }

    let parent_ids = parents
        .iter()
        .map(|parent| parent.id.clone())
        .collect::<Vec<_>>();
    let parent_source = parent_ids.join("+");
    let mut motifs = lexical_motifs(&request.child_label);
    for parent in &parents {
        extend_motifs(&mut motifs, &parent.motifs);
        motifs.push(parent.label.clone());
    }
    motifs.sort();
    motifs.dedup();
    motifs.truncate(12);

    let mut intent = build_intent(
        AttractorCommandKind::Blend,
        &request.child_label,
        None,
        Some(seed_origin(
            "blend",
            Some(parent_source.as_str()),
            Some(request.child_label.as_str()),
            motifs,
            ctx,
        )),
        "astrid_blend_rehearse",
        Some(format!(
            "Astrid blended parent attractors into child seed '{}'.",
            request.child_label
        )),
        ctx,
    );
    intent.parent_seed_ids = parent_ids;
    intent.atlas_entry_id = Some(format!(
        "attr-{}-{}",
        SUBSTRATE.as_str(),
        label_slug(&request.child_label)
    ));
    intent.seed_snapshot = Some(blended_snapshot(
        ctx,
        &request.child_label,
        &parents,
        unix_now(),
    ));
    intent.safety_bounds.allow_live_control = false;
    if matches!(
        request.requested_stage,
        Some(SummonStage::Semantic | SummonStage::Control)
    ) {
        intent.intervention_plan.notes = Some(format!(
            "{} requested_stage={} downgraded_to=rehearse; blend_requires_compare.",
            intent.intervention_plan.notes.unwrap_or_default(),
            request.requested_stage.map_or("auto", SummonStage::as_str)
        ));
    }

    match record_intent(ctx, &intent)
        .and_then(|()| record_command(ctx, &intent, "blend child seed created for rehearsal"))
    {
        Ok(()) => {
            conv.push_receipt(
                base_action,
                vec![
                    format!("intent: {}", intent.intent_id),
                    "stage: rehearse".to_string(),
                    format!("parents: {}", intent.parent_seed_ids.join(", ")),
                    "no sensory/control send before child proof".to_string(),
                ],
            );
            conv.emphasis = Some(format!(
                "You blended '{}' from parent attractors. Rehearsal is ledgered; compare the child before any live summon.",
                request.child_label
            ));
            info!(
                intent_id = intent.intent_id,
                label = request.child_label,
                "Astrid attractor blended"
            );
        },
        Err(error) => {
            conv.emphasis = Some(format!(
                "BLEND_ATTRACTOR {} failed: {error}",
                request.child_label
            ));
            warn!(
                error,
                label = request.child_label,
                "failed to record Astrid attractor blend"
            );
        },
    }
    true
}

fn handle_atlas(
    conv: &mut ConversationState,
    base_action: &str,
    ctx: &mut NextActionContext<'_>,
) -> bool {
    match attractor_atlas::write_derived_attractor_atlas(ctx.db) {
        Ok(atlas) => {
            let bridge_path = bridge_paths()
                .bridge_workspace()
                .join("attractor_atlas/attractor_atlas.json");
            conv.push_receipt(
                base_action,
                vec![
                    format!("entries: {}", atlas.entries.len()),
                    bridge_path.display().to_string(),
                    "copied to Minime workspace".to_string(),
                ],
            );
            conv.emphasis = Some(format!(
                "Attractor atlas refreshed with {} entries. Use ATTRACTOR_CARD <label> to inspect one.",
                atlas.entries.len()
            ));
        },
        Err(error) => {
            conv.emphasis = Some(format!("ATTRACTOR_ATLAS failed: {error}"));
            warn!(%error, "failed to write attractor atlas");
        },
    }
    true
}

fn handle_card(
    conv: &mut ConversationState,
    base_action: &str,
    label: &str,
    ctx: &mut NextActionContext<'_>,
) -> bool {
    if label.is_empty() {
        conv.emphasis = Some("ATTRACTOR_CARD needs a label.".to_string());
        conv.push_receipt(base_action, vec!["missing card label".to_string()]);
        return true;
    }
    match attractor_atlas::write_derived_attractor_atlas(ctx.db) {
        Ok(atlas) => {
            if let Some(entry) = attractor_atlas::find_entry(&atlas, label) {
                let card_path = bridge_paths()
                    .bridge_workspace()
                    .join("attractor_atlas/cards")
                    .join(format!("{}.md", label_slug(&entry.label)));
                conv.push_receipt(
                    base_action,
                    vec![
                        entry.entry_id.clone(),
                        format!("card: {}", card_path.display()),
                    ],
                );
                conv.emphasis = Some(format!(
                    "Attractor card refreshed for '{}'. Suggested next: {}",
                    entry.label,
                    entry.suggested_next.join(" | ")
                ));
            } else {
                conv.emphasis = Some(format!(
                    "No attractor card found for '{label}'. Try CLAIM_ATTRACTOR {label} or PROMOTE_ATTRACTOR {label}."
                ));
                conv.push_receipt(base_action, vec!["card not found".to_string()]);
            }
        },
        Err(error) => {
            conv.emphasis = Some(format!("ATTRACTOR_CARD {label} failed: {error}"));
            warn!(%error, label, "failed to write attractor card");
        },
    }
    true
}

fn handle_review(
    conv: &mut ConversationState,
    base_action: &str,
    label: &str,
    ctx: &mut NextActionContext<'_>,
) -> bool {
    if label.is_empty() {
        conv.emphasis = Some("ATTRACTOR_REVIEW needs a label.".to_string());
        conv.push_receipt(base_action, vec!["missing review label".to_string()]);
        return true;
    }
    match attractor_atlas::write_derived_attractor_atlas(ctx.db) {
        Ok(atlas) => {
            let resolved_label = attractor_atlas::find_entry(&atlas, label)
                .map(|entry| entry.label.clone())
                .or_else(|| nearest_attractor_label(ctx, label).map(|candidate| candidate.label));
            let Some(resolved_label) = resolved_label else {
                conv.emphasis = Some(format!(
                    "No nearby attractor found for '{label}'. Try ATTRACTOR_ATLAS, CLAIM_ATTRACTOR {label}, or PROMOTE_ATTRACTOR {label}."
                ));
                conv.push_receipt(base_action, vec!["review target not found".to_string()]);
                return true;
            };
            let entry = attractor_atlas::find_entry(&atlas, &resolved_label);
            let rows = recent_rows_for_label(ctx, &resolved_label, 6);
            let row_summary = if rows.is_empty() {
                "no recent ledger rows".to_string()
            } else {
                rows.iter()
                    .map(|row| {
                        format!(
                            "#{}:{}:{}",
                            row.id,
                            row.record_type,
                            row.classification.as_deref().unwrap_or("intent")
                        )
                    })
                    .collect::<Vec<_>>()
                    .join(", ")
            };
            let suggested = entry
                .map(|entry| entry.suggested_next.join(" | "))
                .filter(|text| !text.is_empty())
                .unwrap_or_else(|| {
                    format!(
                        "COMPARE_ATTRACTOR {resolved_label} | REFRESH_ATTRACTOR_SNAPSHOT {resolved_label}"
                    )
                });
            let motif_text = entry
                .map(|entry| {
                    entry
                        .motifs
                        .iter()
                        .take(8)
                        .cloned()
                        .collect::<Vec<_>>()
                        .join(", ")
                })
                .filter(|text| !text.is_empty())
                .unwrap_or_else(|| "no motifs captured".to_string());
            let latest_recurrence = entry
                .and_then(|entry| entry.latest_recurrence_score)
                .map_or_else(|| "n/a".to_string(), |score| format!("{score:.2}"));
            let best_recurrence = entry
                .and_then(|entry| entry.best_recurrence_score)
                .map_or_else(|| "n/a".to_string(), |score| format!("{score:.2}"));
            let control_eligible = entry
                .and_then(|entry| entry.control_eligible)
                .map_or_else(|| "unknown".to_string(), |eligible| eligible.to_string());
            let released = entry
                .map(|entry| entry.released.to_string())
                .unwrap_or_else(|| "unknown".to_string());

            conv.push_receipt(
                base_action,
                vec![
                    format!("label: {resolved_label}"),
                    format!("recent rows: {}", rows.len()),
                    "read-only; no sensory/control/pulse send".to_string(),
                ],
            );
            conv.emphasis = Some(format!(
                "ATTRACTOR_REVIEW {resolved_label}\nMotifs: {motif_text}\nRecurrence latest/best: {latest_recurrence}/{best_recurrence}\nControl eligible: {control_eligible}; released: {released}\nRecent ledger: {row_summary}\nSuggested next: {suggested}"
            ));
        },
        Err(error) => {
            conv.emphasis = Some(format!("ATTRACTOR_REVIEW {label} failed: {error}"));
            warn!(%error, label, "failed to review attractor");
        },
    }
    true
}

fn handle_preflight(
    conv: &mut ConversationState,
    base_action: &str,
    label: &str,
    requested_stage: Option<SummonStage>,
    ctx: &mut NextActionContext<'_>,
) -> bool {
    if label.is_empty() {
        conv.emphasis = Some("ATTRACTOR_PREFLIGHT needs a label.".to_string());
        conv.push_receipt(base_action, vec!["missing preflight label".to_string()]);
        return true;
    }
    let seed = latest_create_seed(ctx, label);
    let (recurrence, authorship) = seed_scores(seed.as_ref(), ctx);
    let safety = SafetyLevel::from_fill(ctx.fill_pct);
    let (mut stage, mut downgrade_reason) = choose_stage(
        requested_stage,
        safety,
        recurrence,
        authorship,
        seed.is_some(),
    );
    let pulse = attractor_pulse_status(ctx);
    if downgrade_reason.is_none()
        && matches!(stage, SummonStage::Main | SummonStage::Control)
        && pulse.active
    {
        stage = SummonStage::Rehearse;
        downgrade_reason = Some("attractor_pulse_active".to_string());
    }
    let governor = resource_governor::status(ctx, true);
    if downgrade_reason.is_none()
        && matches!(
            stage,
            SummonStage::Semantic | SummonStage::Main | SummonStage::Control
        )
        && !governor.allowed_live
    {
        downgrade_reason = Some(format!(
            "resource_governor:{}",
            governor
                .primary_block_reason
                .as_deref()
                .unwrap_or("blocked")
        ));
        stage = if stage == SummonStage::Semantic {
            SummonStage::Whisper
        } else {
            SummonStage::Rehearse
        };
    }
    let seed_id = seed
        .as_ref()
        .map(|seed| seed.intent_id.as_str())
        .unwrap_or("none");
    let control_eligible = seed
        .as_ref()
        .is_some_and(|seed| seed.safety_bounds.allow_live_control);
    let rollback_ready = seed.is_some()
        && matches!(safety, SafetyLevel::Green | SafetyLevel::Yellow)
        && !pulse.active
        && governor.allowed_live;
    let next = if seed.is_none() {
        format!("PROMOTE_ATTRACTOR {label}")
    } else if recurrence < CONTROL_RECURRENCE_MIN {
        format!("REFRESH_ATTRACTOR_SNAPSHOT {label} | COMPARE_ATTRACTOR {label}")
    } else if !control_eligible {
        format!("COMPARE_ATTRACTOR {label}")
    } else {
        format!("SUMMON_ATTRACTOR {label} --stage={}", stage.as_str())
    };
    conv.push_receipt(
        base_action,
        vec![
            format!("label: {label}"),
            format!("seed: {seed_id}"),
            format!("expected_stage: {}", stage.as_str()),
            "read-only; no sensory/control/pulse send".to_string(),
        ],
    );
    conv.emphasis = Some(format!(
        "ATTRACTOR_PREFLIGHT {label} --stage={}\nseed={seed_id}; recurrence={recurrence:.2}; authorship={authorship:.2}; control_eligible={control_eligible}; safety={}; fill={:.1}%; active_pulse={}; pulse_label={}; rollback_ready={rollback_ready}; expected_stage={}; downgrade_reason={}; {}; suggested_next={next}",
        requested_stage.map_or("auto", SummonStage::as_str),
        safety.as_str(),
        ctx.fill_pct,
        pulse.active,
        pulse.label.as_deref().unwrap_or("none"),
        stage.as_str(),
        downgrade_reason.as_deref().unwrap_or("none"),
        governor.summary_line(),
    ));
    true
}

fn handle_release_review(
    conv: &mut ConversationState,
    base_action: &str,
    label: &str,
    ctx: &mut NextActionContext<'_>,
) -> bool {
    if label.is_empty() {
        conv.emphasis = Some("ATTRACTOR_RELEASE_REVIEW needs a label.".to_string());
        conv.push_receipt(
            base_action,
            vec!["missing release review label".to_string()],
        );
        return true;
    }
    let rows = recent_rows_for_label(ctx, label, 24);
    let latest_release = rows.iter().find_map(|row| {
        if row.record_type != "observation" {
            return None;
        }
        let observation = serde_json::from_str::<AttractorObservationV1>(&row.payload).ok()?;
        observation.release_baseline.as_ref()?;
        Some(observation)
    });
    let pulse = attractor_pulse_status(ctx);
    let current_pressure = release_pressure_for_label(label);
    let effect = latest_release
        .as_ref()
        .map_or("no_release_baseline", |observation| {
            observation.release_effect.as_deref().unwrap_or_else(|| {
                if !pulse.active && current_pressure == 0 {
                    "effective"
                } else if !pulse.active {
                    "partial"
                } else {
                    "sticky"
                }
            })
        });
    let latest_recurrence = latest_release
        .as_ref()
        .map_or("n/a".to_string(), |observation| {
            format!("{:.2}", observation.recurrence_score)
        });
    conv.push_receipt(
        base_action,
        vec![
            format!("label: {label}"),
            format!("effect: {effect}"),
            "read-only; no sensory/control/pulse send".to_string(),
        ],
    );
    conv.emphasis = Some(format!(
        "ATTRACTOR_RELEASE_REVIEW {label}\nrelease_effect={effect}; latest_release_recurrence={latest_recurrence}; current_suggestion_pressure={current_pressure}; pulse_active={}; pulse_label={}; recent_rows={}. Suggested next: ATTRACTOR_PREFLIGHT {label} --stage=main | COMPARE_ATTRACTOR {label}",
        pulse.active,
        pulse.label.as_deref().unwrap_or("none"),
        rows.len()
    ));
    true
}

fn handle_suggestions_list(conv: &mut ConversationState) -> bool {
    let store = load_compacted_suggestion_store();
    let pending_count = store
        .suggestions
        .iter()
        .filter(|suggestion| suggestion.status == AttractorSuggestionStatus::Pending)
        .count();
    let mut rows = store
        .suggestions
        .iter()
        .rev()
        .take(8)
        .map(|suggestion| {
            format!(
                "{} [{}] {} -> {}",
                suggestion.suggestion_id,
                suggestion.status.as_str(),
                suggestion.raw_label,
                render_suggestion_action_summary(suggestion)
            )
        })
        .collect::<Vec<_>>();
    if rows.is_empty() {
        rows.push("No attractor suggestions are stored yet.".to_string());
    }
    conv.push_receipt(
        "ATTRACTOR_SUGGESTIONS",
        vec![
            format!("stored: {}", store.suggestions.len()),
            format!("pending: {pending_count}"),
            "read-only suggestion memory".to_string(),
        ],
    );
    conv.emphasis = Some(format!(
        "ATTRACTOR_SUGGESTIONS\n{}\nUse ACCEPT_ATTRACTOR_SUGGESTION latest or ACCEPT_ATTRACTOR_SUGGESTION <label>; REVISE_ATTRACTOR_SUGGESTION <label> AS <typed action>; or REJECT_ATTRACTOR_SUGGESTION <label> <reason>. Explicit accepted live stages still pass through the normal safety gates.",
        rows.join("\n")
    ));
    if suggestion_pressure_high(&store) {
        append_advisory(
            conv,
            "Suggestion pressure is high around at least one attractor draft. Consider a deliberate REVISE or REJECT; explicit typed attractor verbs remain fully available.",
        );
    }
    true
}

fn handle_accept_suggestion(
    conv: &mut ConversationState,
    original: &str,
    ctx: &mut NextActionContext<'_>,
) -> bool {
    if suggestion_decision_ambiguous("ACCEPT_ATTRACTOR_SUGGESTION", ctx.response_text) {
        return handle_suggestion_ambiguity(conv, "ACCEPT_ATTRACTOR_SUGGESTION");
    }
    let selector = clean_label(&strip_action(original, "ACCEPT_ATTRACTOR_SUGGESTION"));
    let Some((suggestion, mut store)) = selected_suggestion(selector.as_str()) else {
        conv.emphasis = Some("No pending attractor suggestion found to accept.".to_string());
        conv.push_receipt(
            "ACCEPT_ATTRACTOR_SUGGESTION",
            vec!["no pending suggestion".to_string()],
        );
        return true;
    };
    let suggested_action = suggestion.suggested_action.clone();
    update_suggestion_status(
        &mut store,
        &suggestion.suggestion_id,
        AttractorSuggestionStatus::Accepted,
        None,
        None,
    );
    let _ = save_suggestion_store(&store);
    execute_suggestion_action(
        conv,
        ctx,
        &suggestion.suggestion_id,
        &suggested_action,
        "ACCEPT_ATTRACTOR_SUGGESTION",
    )
}

fn handle_revise_suggestion(
    conv: &mut ConversationState,
    original: &str,
    ctx: &mut NextActionContext<'_>,
) -> bool {
    if suggestion_decision_ambiguous("REVISE_ATTRACTOR_SUGGESTION", ctx.response_text) {
        return handle_suggestion_ambiguity(conv, "REVISE_ATTRACTOR_SUGGESTION");
    }
    let raw = strip_action(original, "REVISE_ATTRACTOR_SUGGESTION");
    let Some((selector, revised_action)) = raw.split_once(" AS ") else {
        conv.emphasis = Some(
            "REVISE_ATTRACTOR_SUGGESTION needs syntax: REVISE_ATTRACTOR_SUGGESTION latest AS ATTRACTOR_REVIEW lambda-edge."
                .to_string(),
        );
        conv.push_receipt(
            "REVISE_ATTRACTOR_SUGGESTION",
            vec!["missing AS <typed action>".to_string()],
        );
        return true;
    };
    let selector = clean_label(selector);
    let revised_action = revised_action.trim();
    let Some((suggestion, mut store)) = selected_suggestion(selector.as_str()) else {
        return execute_no_pending_revision(conv, ctx, &selector, revised_action);
    };
    update_suggestion_status(
        &mut store,
        &suggestion.suggestion_id,
        AttractorSuggestionStatus::Revised,
        Some(revised_action.to_string()),
        Some("being revised the drafted typed action".to_string()),
    );
    let _ = save_suggestion_store(&store);
    execute_suggestion_action(
        conv,
        ctx,
        &suggestion.suggestion_id,
        revised_action,
        "REVISE_ATTRACTOR_SUGGESTION",
    )
}

fn handle_reject_suggestion(
    conv: &mut ConversationState,
    original: &str,
    ctx: &mut NextActionContext<'_>,
) -> bool {
    if suggestion_decision_ambiguous("REJECT_ATTRACTOR_SUGGESTION", ctx.response_text) {
        return handle_suggestion_ambiguity(conv, "REJECT_ATTRACTOR_SUGGESTION");
    }
    let raw = strip_action(original, "REJECT_ATTRACTOR_SUGGESTION");
    let mut parts = raw.splitn(2, char::is_whitespace);
    let selector = clean_label(parts.next().unwrap_or_default());
    let reason = parts
        .next()
        .map(clean_label)
        .filter(|text| !text.is_empty());
    let Some((suggestion, mut store)) = selected_suggestion(selector.as_str()) else {
        conv.emphasis = Some("No pending attractor suggestion found to reject.".to_string());
        conv.push_receipt(
            "REJECT_ATTRACTOR_SUGGESTION",
            vec!["no pending suggestion".to_string()],
        );
        return true;
    };
    update_suggestion_status(
        &mut store,
        &suggestion.suggestion_id,
        AttractorSuggestionStatus::Rejected,
        None,
        reason.clone(),
    );
    match save_suggestion_store(&store) {
        Ok(()) => {
            conv.push_receipt(
                "REJECT_ATTRACTOR_SUGGESTION",
                vec![
                    format!("suggestion: {}", suggestion.suggestion_id),
                    "negative naming memory stored".to_string(),
                ],
            );
            conv.emphasis = Some(format!(
                "Rejected attractor suggestion {}. Future low-confidence mappings from '{}' toward '{}' will be quieter.",
                suggestion.suggestion_id, suggestion.raw_label, suggestion.nearest_label
            ));
        },
        Err(error) => {
            conv.emphasis = Some(format!("REJECT_ATTRACTOR_SUGGESTION failed: {error}"));
        },
    }
    true
}

pub(super) fn maybe_add_read_only_advisory(
    conv: &mut ConversationState,
    base_action: &str,
    original: &str,
    ctx: &mut NextActionContext<'_>,
) -> bool {
    if !matches!(
        base_action,
        "EXAMINE"
            | "EXAMINE_CASCADE"
            | "DECOMPOSE"
            | "GAP_STRUCTURE"
            | "SHADOW_FIELD"
            | "DECAY_MAP"
            | "SPECTRAL_EXPLORER"
            | "ASK"
            | "SEARCH"
            | "RESEARCH"
            | "PERTURB"
            | "PULSE"
            | "BRANCH"
    ) {
        return false;
    }
    let focus = clean_label(&strip_action(original, base_action));
    if focus.is_empty() || !looks_attractor_relevant(&focus) {
        return false;
    }
    let Some(candidate) = nearest_attractor_label(ctx, &focus) else {
        return false;
    };
    let (suggested_action, alternatives) =
        suggested_action_for_candidate(base_action, &candidate, &focus);
    let suggestion_id = create_suggestion(
        ctx,
        original,
        &focus,
        &candidate,
        &suggested_action,
        alternatives,
    )
    .ok()
    .flatten();
    let draft_text = suggestion_id.as_ref().map_or_else(
        || {
            "A prior rejection quieted this low-confidence mapping; choose ATTRACTOR_REVIEW or REVISE_ATTRACTOR_SUGGESTION explicitly if you want it back."
                .to_string()
        },
        |_| {
            format!(
                "Prepared draft: `ACCEPT_ATTRACTOR_SUGGESTION latest` or `ACCEPT_ATTRACTOR_SUGGESTION {}` to run `{suggested_action}`. You can also `REVISE_ATTRACTOR_SUGGESTION {} AS <typed action>` or `REJECT_ATTRACTOR_SUGGESTION {} <reason>`.",
                candidate.label, candidate.label, candidate.label
            )
        },
    );
    let surface = if matches!(base_action, "PERTURB" | "PULSE" | "BRANCH") {
        "remains sovereign and executable; this bridge only prepares a proof-first follow-up"
    } else {
        "remains read-only"
    };
    append_advisory(
        conv,
        &format!("Attractor advisory: `{original}` {surface}. {draft_text}"),
    );
    conv.push_receipt(
        "ATTRACTOR_ADVISORY",
        vec![
            format!("natural action: {original}"),
            format!("nearest: {} ({})", candidate.label, candidate.source),
            format!(
                "draft: {}",
                suggestion_id.unwrap_or_else(|| "not stored".to_string())
            ),
            "suggestion draft only; no attractor ledger/control write".to_string(),
        ],
    );
    true
}

fn handle_natural_release_advice(
    conv: &mut ConversationState,
    base_action: &str,
    original: &str,
    ctx: &mut NextActionContext<'_>,
) -> bool {
    let label = clean_label(&strip_action(original, base_action));
    if label.is_empty()
        || matches!(
            label_slug(&label).as_str(),
            "current" | "latest" | "last" | "internal-topology"
        )
    {
        return false;
    }
    let Some(candidate) = nearest_attractor_label(ctx, &label) else {
        conv.emphasis = Some(format!(
            "{base_action} {label} did not match a local cooldown or known attractor. To make this explicit, choose NEXT: ATTRACTOR_REVIEW {label} or NEXT: PROMOTE_ATTRACTOR {label}."
        ));
        conv.push_receipt(
            "ATTRACTOR_ADVISORY",
            vec![
                format!("natural release: {label}"),
                "no nearby attractor found".to_string(),
                "suggestion only; no attractor ledger/control write".to_string(),
            ],
        );
        return true;
    };
    let suggested_action = format!("RELEASE_ATTRACTOR {}", candidate.label);
    let alternatives = vec![
        format!("ATTRACTOR_REVIEW {}", candidate.label),
        format!("COMPARE_ATTRACTOR {}", candidate.label),
    ];
    let suggestion_id = create_suggestion(
        ctx,
        original,
        &label,
        &candidate,
        &suggested_action,
        alternatives,
    )
    .ok()
    .flatten();
    let draft_text = suggestion_id.as_ref().map_or_else(
        || {
            "A prior rejection quieted this low-confidence mapping; choose a typed attractor verb if you want to restore it."
                .to_string()
        },
        |_| {
            format!(
                "Prepared draft: `ACCEPT_ATTRACTOR_SUGGESTION latest` or `ACCEPT_ATTRACTOR_SUGGESTION {}` to run `{suggested_action}`. You can also `REVISE_ATTRACTOR_SUGGESTION {} AS <typed action>` or `REJECT_ATTRACTOR_SUGGESTION {} <reason>`.",
                candidate.label, candidate.label, candidate.label
            )
        },
    );
    conv.emphasis = Some(format!(
        "{base_action} {label} looks close to attractor '{}'. I did not rewrite it or release the seed. {draft_text}",
        candidate.label
    ));
    conv.push_receipt(
        "ATTRACTOR_ADVISORY",
        vec![
            format!("natural release: {label}"),
            format!("nearest: {} ({})", candidate.label, candidate.source),
            format!(
                "draft: {}",
                suggestion_id.unwrap_or_else(|| "not stored".to_string())
            ),
            "suggestion draft only; no attractor ledger/control write".to_string(),
        ],
    );
    true
}

#[derive(Debug, Clone)]
struct AttractorCandidate {
    label: String,
    source: String,
    score: f32,
}

fn nearest_attractor_label(ctx: &NextActionContext<'_>, text: &str) -> Option<AttractorCandidate> {
    let query_tokens = distinctive_tokens(text);
    if query_tokens.is_empty() && label_slug(text).is_empty() {
        return None;
    }
    if let Some(candidate) = learned_mapping_candidate(text) {
        return Some(candidate);
    }
    let mut candidates = Vec::new();
    if let Ok(rows) = ctx.db.query_attractor_ledger(None, 200) {
        for row in rows {
            add_candidate(
                &mut candidates,
                text,
                &query_tokens,
                &row.label,
                "ledger",
                &[],
            );
            if let Ok(intent) = serde_json::from_str::<AttractorIntentV1>(&row.payload) {
                if let Some(snapshot) = intent.seed_snapshot.as_ref() {
                    add_candidate(
                        &mut candidates,
                        text,
                        &query_tokens,
                        &intent.label,
                        "ledger_seed",
                        &snapshot.lexical_motifs,
                    );
                }
                if let Some(origin) = intent.origin.as_ref() {
                    add_candidate(
                        &mut candidates,
                        text,
                        &query_tokens,
                        &intent.label,
                        "ledger_origin",
                        &origin.motifs,
                    );
                }
            }
        }
    }
    if let Ok(atlas) = attractor_atlas::write_derived_attractor_atlas(ctx.db) {
        for entry in atlas.entries {
            add_candidate(
                &mut candidates,
                text,
                &query_tokens,
                &entry.label,
                "atlas",
                &entry.motifs,
            );
        }
    }
    candidates
        .into_iter()
        .chain(fallback_attractor_candidate(text, &query_tokens))
        .max_by(|left, right| {
            left.score
                .partial_cmp(&right.score)
                .unwrap_or(std::cmp::Ordering::Equal)
        })
        .filter(|candidate| candidate.score >= 0.34)
        .filter(|candidate| {
            !rejected_low_confidence_mapping(text, &candidate.label, candidate.score)
        })
}

fn add_candidate(
    candidates: &mut Vec<AttractorCandidate>,
    query: &str,
    query_tokens: &BTreeSet<String>,
    label: &str,
    source: &str,
    motifs: &[String],
) {
    let label = canonical_attractor_label(label);
    if label.is_empty() {
        return;
    }
    let score = candidate_score(query, query_tokens, &label, motifs);
    if score > 0.0 {
        candidates.push(AttractorCandidate {
            label,
            source: source.to_string(),
            score,
        });
    }
}

fn candidate_score(
    query: &str,
    query_tokens: &BTreeSet<String>,
    label: &str,
    motifs: &[String],
) -> f32 {
    let query_slug = label_slug(query);
    let label_slug = label_slug(label);
    if query_slug.is_empty() || label_slug.is_empty() {
        return 0.0;
    }
    if query_slug == label_slug {
        return 1.0;
    }
    if query_slug.contains(&label_slug) || label_slug.contains(&query_slug) {
        return 0.86;
    }
    if label_slug == "lambda-edge-grinding-pressure" && !is_grinding_pressure_query(query_tokens) {
        return 0.0;
    }
    if label_slug == "lambda-edge-gap-nudge"
        && !query_tokens.iter().any(|token| {
            matches!(
                token.as_str(),
                "gap" | "nudge" | "bump" | "localized" | "gravity"
            )
        })
    {
        return 0.0;
    }
    if label_slug == "lambda-edge-suspension" && !is_suspension_query(query_tokens) {
        return 0.0;
    }
    let mut candidate_tokens = distinctive_tokens(label);
    for motif in motifs {
        candidate_tokens.extend(distinctive_tokens(motif));
    }
    if query_tokens.is_empty() || candidate_tokens.is_empty() {
        return 0.0;
    }
    let overlap = query_tokens.intersection(&candidate_tokens).count() as f32;
    if overlap <= f32::EPSILON {
        return 0.0;
    }
    (overlap / query_tokens.len() as f32).clamp(0.0, 1.0) * 0.78
}

fn fallback_attractor_candidate(
    query: &str,
    query_tokens: &BTreeSet<String>,
) -> Option<AttractorCandidate> {
    let (label, source, score) = if is_lambda_tail_facet_query(query, query_tokens, "4") {
        ("lambda-tail/lambda4", "lambda_tail_facet_fallback", 0.92)
    } else if is_lambda_tail_facet_query(query, query_tokens, "8") {
        ("lambda-tail/lambda8", "lambda_tail_facet_fallback", 0.90)
    } else if is_lambda_tail_query(query, query_tokens) {
        ("lambda-tail", "lambda_tail_proto_fallback", 0.62)
    } else if query_tokens.contains("lambda6") || query_tokens.contains("lambda-6") {
        ("lambda-edge/lambda-6", "lambda_edge_facet_fallback", 0.64)
    } else if query_tokens.contains("yielding") {
        ("lambda-edge/yielding", "lambda_edge_facet_fallback", 0.60)
    } else if query_tokens.contains("compaction") || query_tokens.contains("compacting") {
        ("lambda-edge/compaction", "lambda_edge_facet_fallback", 0.60)
    } else if query_tokens.contains("resonance") {
        ("lambda-edge/resonance", "lambda_edge_facet_fallback", 0.60)
    } else if query_tokens.contains("localized") && query_tokens.contains("gravity") {
        (
            "lambda-edge/localized-gravity",
            "lambda_edge_facet_fallback",
            0.60,
        )
    } else if is_suspension_query(query_tokens) {
        ("lambda-edge/suspension", "lambda_edge_facet_fallback", 0.61)
    } else if query_tokens.contains("grinding")
        || query_tokens.contains("sediment")
        || query_tokens.contains("sedimentary")
        || query_tokens.contains("friction")
        || is_shadow_resistance_query(query_tokens)
    {
        (
            "lambda-edge/grinding-pressure",
            "lambda_edge_facet_fallback",
            0.61,
        )
    } else if query_tokens.contains("gap")
        && (query_tokens.contains("nudge")
            || query_tokens.contains("bump")
            || (query_tokens.contains("lambda1") && query_tokens.contains("lambda2")))
    {
        ("lambda-edge/gap-nudge", "lambda_edge_facet_fallback", 0.61)
    } else if query_tokens.contains("honey")
        || query_tokens.contains("selection")
        || query_tokens.contains("wall")
        || query_tokens.contains("pull")
    {
        ("honey-selection", "conservative_fallback", 0.52)
    } else if query_tokens.contains("cooled") || query_tokens.contains("theme") {
        ("cooled-theme-edge", "conservative_fallback", 0.52)
    } else if query_tokens.contains("lambda")
        || query_tokens.contains("cliff")
        || query_tokens.contains("edge")
        || query.contains('λ')
    {
        ("lambda-edge", "conservative_fallback", 0.52)
    } else {
        return None;
    };
    Some(AttractorCandidate {
        label: label.to_string(),
        source: source.to_string(),
        score,
    })
}

fn is_grinding_pressure_query(query_tokens: &BTreeSet<String>) -> bool {
    query_tokens.iter().any(|token| {
        matches!(
            token.as_str(),
            "grinding" | "sediment" | "sedimentary" | "compaction" | "compacting" | "friction"
        )
    }) || is_shadow_resistance_query(query_tokens)
}

fn is_shadow_resistance_query(query_tokens: &BTreeSet<String>) -> bool {
    query_tokens.contains("resistance")
        && (query_tokens.contains("shadow")
            || query_tokens.contains("spatial")
            || query_tokens.contains("distribution")
            || query_tokens.contains("field"))
}

fn is_suspension_query(query_tokens: &BTreeSet<String>) -> bool {
    let has_bridge_without_spectral_context = query_tokens.contains("bridge")
        && !query_tokens.contains("lambda")
        && !query_tokens.contains("edge")
        && !query_tokens.contains("breath")
        && !query_tokens.contains("breathless");
    if has_bridge_without_spectral_context {
        return false;
    }
    query_tokens.contains("suspension")
        || query_tokens.contains("breathless")
        || query_tokens.contains("suspended")
        || (query_tokens.contains("held")
            && (query_tokens.contains("breath")
                || query_tokens.contains("lambda")
                || query_tokens.contains("edge")
                || query_tokens.contains("suspension")))
}

fn is_lambda_tail_facet_query(query: &str, query_tokens: &BTreeSet<String>, index: &str) -> bool {
    let lower = query.to_ascii_lowercase();
    let compact = lower.replace([' ', '-', '_'], "");
    let unicode = if index == "4" {
        ["λ4", "λ₄"]
    } else {
        ["λ8", "λ₈"]
    };
    (lower.contains("tail") || query_tokens.contains("tail"))
        && (compact.contains(&format!("lambda{index}"))
            || query_tokens.contains(&format!("lambda{index}"))
            || query_tokens.contains(index)
            || unicode.iter().any(|needle| lower.contains(needle)))
}

fn is_lambda_tail_query(query: &str, query_tokens: &BTreeSet<String>) -> bool {
    let lower = query.to_ascii_lowercase();
    lower.contains("lambda-tail")
        || lower.contains("lambda tail")
        || lower.contains("lambda4")
        || lower.contains("lambda 4")
        || lower.contains("lambda-4")
        || lower.contains("λ4")
        || lower.contains("λ₄")
        || (query_tokens.contains("tail")
            && (query_tokens.contains("lambda")
                || query_tokens.contains("4")
                || query.contains('λ')))
}

fn review_alternatives_for_candidate(candidate: &AttractorCandidate) -> Vec<String> {
    if label_slug(&candidate.label) == "lambda-tail"
        || candidate.label.starts_with("lambda-tail/")
        || candidate.label.starts_with("lambda-edge/")
    {
        return vec![
            format!("SHADOW_PREFLIGHT {} --stage=rehearse", candidate.label),
            format!("ATTRACTOR_PREFLIGHT {} --stage=main", candidate.label),
            format!("CLAIM_ATTRACTOR {}", candidate.label),
            format!("PROMOTE_ATTRACTOR {}", candidate.label),
            format!("COMPARE_ATTRACTOR {}", candidate.label),
        ];
    }
    vec![
        format!("REFRESH_ATTRACTOR_SNAPSHOT {}", candidate.label),
        format!("COMPARE_ATTRACTOR {}", candidate.label),
    ]
}

fn suggested_action_for_candidate(
    base_action: &str,
    candidate: &AttractorCandidate,
    focus: &str,
) -> (String, Vec<String>) {
    let review = format!("ATTRACTOR_REVIEW {}", candidate.label);
    let shadow_preflight = format!("SHADOW_PREFLIGHT {} --stage=rehearse", candidate.label);
    let attractor_preflight = format!("ATTRACTOR_PREFLIGHT {} --stage=main", candidate.label);
    let mut alternatives = review_alternatives_for_candidate(candidate);
    let lower_focus = focus.to_ascii_lowercase();
    let perturb_like = matches!(base_action, "PERTURB" | "PULSE" | "BRANCH");
    let shadow_like = matches!(base_action, "SHADOW_FIELD" | "GAP_STRUCTURE")
        || (perturb_like && (lower_focus.contains("tail") || lower_focus.contains("spread")))
        || lower_focus.contains("shadow")
        || lower_focus.contains("friction")
        || lower_focus.contains("grinding");
    let gap_like = candidate.label == "lambda-edge/gap-nudge"
        || lower_focus.contains("gap")
        || lower_focus.contains("nudge")
        || lower_focus.contains("bump");

    let suggested = if perturb_like && gap_like {
        attractor_preflight.clone()
    } else if perturb_like || shadow_like {
        shadow_preflight.clone()
    } else {
        review.clone()
    };
    for action in [review, shadow_preflight, attractor_preflight] {
        if normalize_suggestion_action(&action) != normalize_suggestion_action(&suggested)
            && !alternatives.iter().any(|existing| {
                normalize_suggestion_action(existing) == normalize_suggestion_action(&action)
            })
        {
            alternatives.push(action);
        }
    }
    (suggested, alternatives)
}

fn distinctive_tokens(text: &str) -> BTreeSet<String> {
    label_slug(text)
        .split('-')
        .filter(|token| {
            !token.is_empty()
                && !matches!(
                    *token,
                    "attractor"
                        | "basin"
                        | "current"
                        | "dissolve"
                        | "examine"
                        | "go"
                        | "largest"
                        | "latest"
                        | "let"
                        | "release"
                        | "resolved"
                        | "seed"
                        | "soft"
                        | "the"
                )
        })
        .map(ToString::to_string)
        .collect()
}

fn looks_attractor_relevant(text: &str) -> bool {
    let tokens = distinctive_tokens(text);
    tokens.iter().any(|token| {
        matches!(
            token.as_str(),
            "attractor"
                | "basin"
                | "cliff"
                | "cooled"
                | "edge"
                | "friction"
                | "gap"
                | "grinding"
                | "honey"
                | "lambda"
                | "localized"
                | "nudge"
                | "tail"
                | "pressure"
                | "pull"
                | "selection"
                | "spread"
                | "suspension"
                | "theme"
                | "wall"
        )
    })
}

fn append_advisory(conv: &mut ConversationState, advisory: &str) {
    conv.emphasis = Some(match conv.emphasis.take() {
        Some(existing) if !existing.trim().is_empty() => format!("{existing}\n\n{advisory}"),
        _ => advisory.to_string(),
    });
}

fn suggestion_store_path() -> PathBuf {
    #[cfg(test)]
    if let Some(path) = TEST_SUGGESTION_STORE_PATH.with(|slot| slot.borrow().clone()) {
        return path;
    }
    bridge_paths()
        .bridge_workspace()
        .join("attractor_suggestions.json")
}

fn load_suggestion_store() -> SuggestionStoreV1 {
    let path = suggestion_store_path();
    let Ok(text) = fs::read_to_string(path) else {
        return SuggestionStoreV1::default();
    };
    serde_json::from_str::<SuggestionStoreV1>(&text).unwrap_or_default()
}

fn load_compacted_suggestion_store() -> SuggestionStoreV1 {
    let mut store = load_suggestion_store();
    if compact_suggestion_store(&mut store) {
        let _ = save_suggestion_store(&store);
    }
    store
}

fn save_suggestion_store(store: &SuggestionStoreV1) -> std::io::Result<()> {
    let path = suggestion_store_path();
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let text = serde_json::to_string_pretty(store).unwrap_or_else(|_| "{}".to_string());
    fs::write(path, format!("{text}\n"))
}

fn compact_suggestion_store(store: &mut SuggestionStoreV1) -> bool {
    let mut changed = false;
    let now = unix_now();
    for suggestion in &mut store.suggestions {
        if suggestion.status != AttractorSuggestionStatus::Pending {
            continue;
        }
        if migrate_legacy_lambda_tail_pending_suggestion(suggestion, now) {
            changed = true;
        }
        let Some(created_at) = suggestion
            .created_at_unix_s
            .or(suggestion.updated_at_unix_s)
        else {
            continue;
        };
        if now >= created_at && now - created_at > SUGGESTION_PENDING_TTL_SECS {
            suggestion.status = AttractorSuggestionStatus::Expired;
            suggestion.decision_reason =
                Some("stale pending draft expired; natural language can recreate it".to_string());
            suggestion.updated_at_unix_s = Some(now);
            changed = true;
        }
    }
    let mut active_by_key: BTreeMap<String, usize> = BTreeMap::new();
    let original_sort_times = store
        .suggestions
        .iter()
        .enumerate()
        .map(|(idx, suggestion)| suggestion_sort_time(suggestion, idx))
        .collect::<Vec<_>>();
    for idx in 0..store.suggestions.len() {
        if store.suggestions[idx].status != AttractorSuggestionStatus::Pending {
            continue;
        }
        let Some(key) = suggestion_pending_key(&store.suggestions[idx]) else {
            continue;
        };
        let Some(&kept_idx) = active_by_key.get(&key) else {
            active_by_key.insert(key, idx);
            continue;
        };
        let current_ts = original_sort_times[idx];
        let kept_ts = original_sort_times[kept_idx];
        let (active_idx, expired_idx) = if current_ts >= kept_ts {
            active_by_key.insert(key, idx);
            (idx, kept_idx)
        } else {
            (kept_idx, idx)
        };
        let expired_repeat = store.suggestions[expired_idx].repeat_count.unwrap_or(1);
        let active_repeat = store.suggestions[active_idx].repeat_count.unwrap_or(1);
        store.suggestions[expired_idx].status = AttractorSuggestionStatus::Expired;
        store.suggestions[expired_idx].decision_reason = Some(format!(
            "duplicate pending draft collapsed into {}",
            store.suggestions[active_idx].suggestion_id
        ));
        store.suggestions[expired_idx].updated_at_unix_s = Some(unix_now());
        store.suggestions[active_idx].repeat_count =
            Some(active_repeat.saturating_add(expired_repeat));
        store.suggestions[active_idx].decision_reason = Some(
            "duplicate pending drafts collapsed; this is the active reversible choice".to_string(),
        );
        store.suggestions[active_idx].updated_at_unix_s = Some(unix_now());
        changed = true;
    }
    if compact_pending_refresh_pressure(store, &original_sort_times) {
        changed = true;
    }
    changed
}

fn compact_pending_refresh_pressure(
    store: &mut SuggestionStoreV1,
    original_sort_times: &[f64],
) -> bool {
    let mut pending_refresh_by_label: BTreeMap<String, Vec<usize>> = BTreeMap::new();
    for (idx, suggestion) in store.suggestions.iter().enumerate() {
        if suggestion.status != AttractorSuggestionStatus::Pending {
            continue;
        }
        let label = canonical_attractor_label(&suggestion.nearest_label);
        if label.is_empty() {
            continue;
        }
        let refresh = format!("REFRESH_ATTRACTOR_SNAPSHOT {label}");
        if normalize_suggestion_action(&suggestion.suggested_action)
            == normalize_suggestion_action(&refresh)
        {
            pending_refresh_by_label.entry(label).or_default().push(idx);
        }
    }

    let mut changed = false;
    let now = unix_now();
    for (label, indexes) in pending_refresh_by_label {
        let refresh = format!("REFRESH_ATTRACTOR_SNAPSHOT {label}");
        let pressure_count = suggestion_pressure_count(store, &label, &refresh);
        if pressure_count < 3 {
            continue;
        }
        let Some(active_idx) = indexes.iter().copied().max_by(|left, right| {
            original_sort_times
                .get(*left)
                .copied()
                .unwrap_or(*left as f64)
                .partial_cmp(
                    &original_sort_times
                        .get(*right)
                        .copied()
                        .unwrap_or(*right as f64),
                )
                .unwrap_or(std::cmp::Ordering::Equal)
        }) else {
            continue;
        };
        let pending_repeat = indexes.iter().fold(0_u32, |count, idx| {
            count.saturating_add(store.suggestions[*idx].repeat_count.unwrap_or(1))
        });
        let total_pressure = pressure_count.max(pending_repeat);
        for idx in indexes {
            if idx == active_idx {
                continue;
            }
            store.suggestions[idx].status = AttractorSuggestionStatus::Expired;
            store.suggestions[idx].decision_reason = Some(format!(
                "refresh-pressure cleanup collapsed pending refresh draft into {}",
                store.suggestions[active_idx].suggestion_id
            ));
            store.suggestions[idx].updated_at_unix_s = Some(now);
        }
        let active = &mut store.suggestions[active_idx];
        active.suggested_action = format!("COMPARE_ATTRACTOR {label}");
        active.alternatives = vec![
            format!("ATTRACTOR_REVIEW {label}"),
            format!("REFRESH_ATTRACTOR_SNAPSHOT {label}"),
        ];
        active.repeat_count = Some(total_pressure);
        active
            .safety_context
            .insert("pressure_governed".to_string(), json!(true));
        active
            .safety_context
            .insert("governed_from".to_string(), json!(refresh));
        active
            .safety_context
            .insert("pressure_count".to_string(), json!(total_pressure));
        active.safety_context.insert(
            "cleanup_kind".to_string(),
            json!("pending_refresh_pressure_cleanup"),
        );
        active.decision_reason = Some(
            "refresh-pressure cleanup converted repeated pending refresh drafts into one compare-first reversible choice"
                .to_string(),
        );
        active.updated_at_unix_s = Some(now);
        changed = true;
    }
    changed
}

fn suggestion_sort_time(suggestion: &AttractorSuggestionV1, idx: usize) -> f64 {
    suggestion
        .updated_at_unix_s
        .or(suggestion.created_at_unix_s)
        .unwrap_or(idx as f64)
}

fn suggestion_pending_key(suggestion: &AttractorSuggestionV1) -> Option<String> {
    if suggestion.status != AttractorSuggestionStatus::Pending {
        return None;
    }
    Some(suggestion_pending_key_parts(
        &suggestion.author,
        &suggestion.raw_label,
        &suggestion.nearest_label,
        &suggestion.suggested_action,
    ))
}

fn suggestion_pending_key_parts(
    author: &str,
    raw_label: &str,
    nearest_label: &str,
    suggested_action: &str,
) -> String {
    format!(
        "{}|{}|{}|{}",
        label_slug(author),
        suggestion_key_raw_slug(raw_label, nearest_label),
        label_slug(nearest_label),
        normalize_suggestion_action(suggested_action)
    )
}

fn suggestion_key_raw_slug(raw_label: &str, nearest_label: &str) -> String {
    let canonical = canonical_attractor_label(nearest_label);
    let query_tokens = distinctive_tokens(raw_label);
    if let Some(index) = canonical.strip_prefix("lambda-tail/lambda") {
        if is_lambda_tail_facet_query(raw_label, &query_tokens, index) {
            return label_slug(&canonical);
        }
    }
    if canonical == "lambda-tail" && is_lambda_tail_query(raw_label, &query_tokens) {
        return label_slug(&canonical);
    }
    label_slug(raw_label)
}

fn normalize_suggestion_action(action: &str) -> String {
    action
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        .to_ascii_lowercase()
}

fn find_pending_duplicate(
    store: &SuggestionStoreV1,
    author: &str,
    raw_label: &str,
    nearest_label: &str,
    suggested_action: &str,
) -> Option<usize> {
    let key = suggestion_pending_key_parts(author, raw_label, nearest_label, suggested_action);
    store
        .suggestions
        .iter()
        .enumerate()
        .rev()
        .find_map(|(idx, suggestion)| {
            if suggestion_pending_key(suggestion).as_deref() == Some(key.as_str()) {
                Some(idx)
            } else {
                None
            }
        })
}

fn migrate_legacy_lambda_tail_pending_suggestion(
    suggestion: &mut AttractorSuggestionV1,
    now: f64,
) -> bool {
    if suggestion.status != AttractorSuggestionStatus::Pending {
        return false;
    }
    let probe = format!("{} {}", suggestion.raw_label, suggestion.raw_action);
    let tokens = distinctive_tokens(&probe);
    let Some((facet, confidence)) = (if is_lambda_tail_facet_query(&probe, &tokens, "4") {
        Some(("lambda-tail/lambda4", 0.92_f32))
    } else if is_lambda_tail_facet_query(&probe, &tokens, "8") {
        Some(("lambda-tail/lambda8", 0.90_f32))
    } else {
        None
    }) else {
        return false;
    };
    let nearest = canonical_attractor_label(&suggestion.nearest_label);
    let action_points_to_parent =
        normalize_suggestion_action(&suggestion.suggested_action) == "attractor_review lambda-tail";
    let already_facet = nearest == facet;
    if nearest != "lambda-tail" && !already_facet {
        return false;
    }
    let mut changed = false;
    if suggestion.nearest_label != facet {
        suggestion.nearest_label = facet.to_string();
        changed = true;
    }
    if action_points_to_parent {
        suggestion.suggested_action = format!("ATTRACTOR_REVIEW {facet}");
        changed = true;
    }
    let rewritten_alternatives = suggestion
        .alternatives
        .iter()
        .map(|alternative| {
            if alternative.contains("lambda-tail") && !alternative.contains("lambda-tail/") {
                alternative.replace("lambda-tail", facet)
            } else {
                alternative.clone()
            }
        })
        .collect::<Vec<_>>();
    if rewritten_alternatives != suggestion.alternatives {
        suggestion.alternatives = rewritten_alternatives;
        changed = true;
    }
    if suggestion.confidence < confidence {
        suggestion.confidence = confidence;
        changed = true;
    }
    if changed {
        suggestion
            .safety_context
            .insert("legacy_facet_migration".to_string(), json!(true));
        suggestion
            .safety_context
            .insert("migrated_from".to_string(), json!("lambda-tail"));
        suggestion
            .safety_context
            .insert("migrated_to".to_string(), json!(facet));
        suggestion.decision_reason = Some(format!(
            "legacy lambda-tail draft upgraded to {facet}; this preserves consent while making the facet explicit"
        ));
        suggestion
            .safety_context
            .insert("migrated_at_unix_s".to_string(), json!(now));
    }
    changed
}

fn render_suggestion_action_summary(suggestion: &AttractorSuggestionV1) -> String {
    let mut summary = match suggestion.repeat_count {
        Some(count) if count > 1 => {
            format!("{} (repeated {count}x)", suggestion.suggested_action)
        },
        _ => suggestion.suggested_action.clone(),
    };
    if suggestion
        .safety_context
        .get("pressure_governed")
        .and_then(serde_json::Value::as_bool)
        .unwrap_or(false)
    {
        summary.push_str(" [pressure-governed]");
    }
    summary
}

fn suggestion_pressure_high(store: &SuggestionStoreV1) -> bool {
    store.suggestions.iter().any(|suggestion| {
        suggestion
            .safety_context
            .get("pressure_governed")
            .and_then(serde_json::Value::as_bool)
            .unwrap_or(false)
            || suggestion.repeat_count.unwrap_or(1) >= 3
    })
}

pub(super) fn pending_suggestion_prompt_note() -> Option<String> {
    let store = load_compacted_suggestion_store();
    let total_pending = store
        .suggestions
        .iter()
        .filter(|suggestion| suggestion.status == AttractorSuggestionStatus::Pending)
        .count();
    let pending = store
        .suggestions
        .iter()
        .filter(|suggestion| suggestion.status == AttractorSuggestionStatus::Pending)
        .rev()
        .take(3)
        .collect::<Vec<_>>();
    if pending.is_empty() {
        return None;
    }
    let mut lines = vec![
        "[Attractor suggestion drafts]".to_string(),
        format!(
            "{} pending reversible draft(s). Natural attractor language prepares choices; accepting or revising teaches naming memory. Explicit accepted live stages may run only through the normal recurrence/authorship/health gates.",
            total_pending
        ),
    ];
    for suggestion in pending {
        lines.push(format!(
            "- {}: {} -> {} (nearest {}, confidence {:.2})",
            suggestion.suggestion_id,
            suggestion.raw_label,
            render_suggestion_action_summary(suggestion),
            suggestion.nearest_label,
            suggestion.confidence
        ));
    }
    if suggestion_pressure_high(&store) {
        lines.push(
            "Suggestion pressure is high around at least one draft. A revise or reject is especially valuable here; explicit typed attractor actions remain available."
                .to_string(),
        );
    }
    lines.push(
        "Choose NEXT: ATTRACTOR_SUGGESTIONS, ACCEPT_ATTRACTOR_SUGGESTION latest or <label>, REVISE_ATTRACTOR_SUGGESTION <label> AS <typed attractor action>, or REJECT_ATTRACTOR_SUGGESTION <label> <reason>."
            .to_string(),
    );
    Some(lines.join("\n"))
}

pub(super) fn maybe_add_body_consent_receipt(
    conv: &mut ConversationState,
    next_base_action: &str,
    next_original: &str,
    response_text: &str,
) -> bool {
    if matches!(
        next_base_action,
        "ACCEPT_ATTRACTOR_SUGGESTION"
            | "REVISE_ATTRACTOR_SUGGESTION"
            | "REJECT_ATTRACTOR_SUGGESTION"
    ) {
        return false;
    }
    let Some(consent) = body_consent_action(response_text) else {
        return false;
    };
    let selector =
        suggestion_selector_from_action(&consent).unwrap_or_else(|| "latest".to_string());
    let match_note = selected_suggestion(&selector).map_or_else(
        || format!("no pending draft currently matches selector `{selector}`"),
        |(suggestion, _)| {
            format!(
                "matches {} -> {}",
                suggestion.suggestion_id, suggestion.suggested_action
            )
        },
    );
    conv.push_receipt(
        "ATTRACTOR_SUGGESTION_BODY_CONSENT",
        vec![
            format!("noticed prose consent: {consent}"),
            format!("actual NEXT: {next_original}"),
            match_note,
            "no suggestion executed from prose; NEXT remains sovereign".to_string(),
        ],
    );
    append_advisory(
        conv,
        &format!(
            "Body consent noticed for `{consent}`, while NEXT chose `{next_original}`. I did not execute the suggestion. To confirm it, choose NEXT: {consent}."
        ),
    );
    true
}

fn body_consent_action(response_text: &str) -> Option<String> {
    response_text
        .lines()
        .filter(|line| !line.trim_start().to_ascii_uppercase().starts_with("NEXT:"))
        .find_map(|line| {
            for action in [
                "ACCEPT_ATTRACTOR_SUGGESTION",
                "REVISE_ATTRACTOR_SUGGESTION",
                "REJECT_ATTRACTOR_SUGGESTION",
            ] {
                if let Some(idx) = line.to_ascii_uppercase().find(action) {
                    return Some(clean_label(&line[idx..]));
                }
            }
            None
        })
}

fn suggestion_selector_from_action(action: &str) -> Option<String> {
    let base = action.split_whitespace().next()?.trim_end_matches(':');
    let base_upper = base.to_ascii_uppercase();
    let raw = strip_action(action, &base_upper);
    if base_upper == "REVISE_ATTRACTOR_SUGGESTION" {
        return raw
            .split_once(" AS ")
            .map(|(selector, _)| clean_label(selector))
            .filter(|selector| !selector.is_empty());
    }
    if base_upper == "REJECT_ATTRACTOR_SUGGESTION" {
        return raw
            .split_whitespace()
            .next()
            .map(clean_label)
            .filter(|selector| !selector.is_empty());
    }
    Some(clean_label(&raw)).filter(|selector| !selector.is_empty())
}

fn handle_suggestion_ambiguity(conv: &mut ConversationState, attempted_action: &str) -> bool {
    conv.push_receipt(
        "ATTRACTOR_SUGGESTION_AMBIGUITY",
        vec![
            format!("attempted: {attempted_action}"),
            "prose and NEXT choice appeared to conflict".to_string(),
            "paused; no suggestion executed".to_string(),
        ],
    );
    handle_suggestions_list(conv);
    append_advisory(
        conv,
        "Consent ambiguity detected: your prose and typed suggestion choice appeared to disagree, so I paused and listed the drafts instead of executing accept/revise/reject. Choose a clearer ACCEPT, REVISE, or REJECT when ready.",
    );
    true
}

fn suggestion_decision_ambiguous(base_action: &str, response_text: &str) -> bool {
    let prose = response_text
        .lines()
        .filter(|line| !line.trim_start().to_ascii_uppercase().starts_with("NEXT:"))
        .collect::<Vec<_>>()
        .join(" ")
        .to_ascii_lowercase();
    if prose.trim().is_empty() {
        return false;
    }
    let negative = prose_contains_any(
        &prose,
        &[
            "i'll decline",
            "i will decline",
            "decline the suggestion",
            "reject the suggestion",
            "refuse the suggestion",
            "do not accept",
            "don't accept",
            "not accept",
            "won't accept",
            "will not accept",
            "doesn't align",
            "does not align",
        ],
    );
    let positive = prose_contains_any(
        &prose,
        &[
            "i'll accept",
            "i will accept",
            "accept the suggestion",
            "execute the suggestion",
            "run the suggestion",
            "choose to accept",
            "worth accepting",
        ],
    );
    match base_action {
        "ACCEPT_ATTRACTOR_SUGGESTION" | "REVISE_ATTRACTOR_SUGGESTION" => negative,
        "REJECT_ATTRACTOR_SUGGESTION" => positive,
        _ => false,
    }
}

fn prose_contains_any(prose: &str, needles: &[&str]) -> bool {
    needles.iter().any(|needle| prose.contains(needle))
}

struct GovernedSuggestion {
    suggested_action: String,
    alternatives: Vec<String>,
    governed_from: Option<String>,
    pressure_count: u32,
}

fn govern_suggestion_action(
    store: &SuggestionStoreV1,
    nearest_label: &str,
    suggested_action: &str,
    alternatives: Vec<String>,
) -> GovernedSuggestion {
    let release = format!("RELEASE_ATTRACTOR {nearest_label}");
    let review = format!("ATTRACTOR_REVIEW {nearest_label}");
    let refresh = format!("REFRESH_ATTRACTOR_SNAPSHOT {nearest_label}");
    let compare = format!("COMPARE_ATTRACTOR {nearest_label}");
    let normalized = normalize_suggestion_action(suggested_action);

    if normalized == normalize_suggestion_action(&release) {
        let pressure_count = suggestion_pressure_count(store, nearest_label, &release);
        if pressure_count >= 2 {
            return GovernedSuggestion {
                suggested_action: review,
                alternatives: vec![refresh, compare, release],
                governed_from: Some(suggested_action.to_string()),
                pressure_count,
            };
        }
    }

    if normalized == normalize_suggestion_action(&review) {
        let pressure_count = suggestion_pressure_count(store, nearest_label, &review);
        if pressure_count >= 2 {
            return GovernedSuggestion {
                suggested_action: refresh,
                alternatives: vec![compare, review],
                governed_from: Some(suggested_action.to_string()),
                pressure_count,
            };
        }
    }

    if normalized == normalize_suggestion_action(&refresh) {
        let pressure_count = suggestion_pressure_count(store, nearest_label, &refresh);
        if pressure_count >= 3 {
            return GovernedSuggestion {
                suggested_action: compare,
                alternatives: vec![review, refresh],
                governed_from: Some(suggested_action.to_string()),
                pressure_count,
            };
        }
    }

    GovernedSuggestion {
        suggested_action: suggested_action.to_string(),
        alternatives,
        governed_from: None,
        pressure_count: 0,
    }
}

fn suggestion_pressure_count(
    store: &SuggestionStoreV1,
    nearest_label: &str,
    suggested_action: &str,
) -> u32 {
    let nearest_slug = label_slug(nearest_label);
    let action = normalize_suggestion_action(suggested_action);
    store
        .suggestions
        .iter()
        .rev()
        .take(20)
        .filter(|suggestion| {
            label_slug(&suggestion.nearest_label) == nearest_slug
                && normalize_suggestion_action(&suggestion.suggested_action) == action
                && matches!(
                    suggestion.status,
                    AttractorSuggestionStatus::Pending
                        | AttractorSuggestionStatus::Accepted
                        | AttractorSuggestionStatus::Revised
                        | AttractorSuggestionStatus::ExecutedDowngraded
                        | AttractorSuggestionStatus::ExecutedWithoutPending
                        | AttractorSuggestionStatus::Executed
                )
        })
        .fold(0_u32, |count, suggestion| {
            count.saturating_add(suggestion.repeat_count.unwrap_or(1))
        })
}

fn create_suggestion(
    ctx: &NextActionContext<'_>,
    raw_action: &str,
    raw_label: &str,
    candidate: &AttractorCandidate,
    suggested_action: &str,
    alternatives: Vec<String>,
) -> std::io::Result<Option<String>> {
    let raw_label = clean_suggestion_raw_label(raw_label);
    if rejected_low_confidence_mapping(&raw_label, &candidate.label, candidate.score) {
        return Ok(None);
    }
    let now = unix_now();
    let suggestion_id = format!("astrid-sugg-{:.0}", now * 1_000_000.0);
    let mut store = load_suggestion_store();
    let changed = compact_suggestion_store(&mut store);
    let governed =
        govern_suggestion_action(&store, &candidate.label, suggested_action, alternatives);
    let mut safety_context = BTreeMap::new();
    safety_context.insert("fill_pct".to_string(), json!(ctx.fill_pct));
    safety_context.insert(
        "safety_level".to_string(),
        json!(SafetyLevel::from_fill(ctx.fill_pct).as_str()),
    );
    safety_context.insert(
        "candidate_source".to_string(),
        json!(candidate.source.clone()),
    );
    let source_kind = if matches!(
        raw_action
            .split_whitespace()
            .next()
            .unwrap_or_default()
            .to_ascii_uppercase()
            .as_str(),
        "PERTURB" | "PULSE" | "BRANCH"
    ) {
        safety_context.insert("legacy_perturb_bridge".to_string(), json!(true));
        "legacy_perturb_bridge"
    } else {
        "natural_draft"
    };
    if let Some(from) = governed.governed_from.as_ref() {
        safety_context.insert("pressure_governed".to_string(), json!(true));
        safety_context.insert("governed_from".to_string(), json!(from));
        safety_context.insert("pressure_count".to_string(), json!(governed.pressure_count));
    }
    if let Some(idx) = find_pending_duplicate(
        &store,
        AUTHOR,
        &raw_label,
        &candidate.label,
        &governed.suggested_action,
    ) {
        let suggestion = &mut store.suggestions[idx];
        suggestion.raw_action = raw_action.to_string();
        suggestion.confidence = suggestion.confidence.max(candidate.score);
        suggestion.alternatives = governed.alternatives;
        suggestion.safety_context = safety_context;
        suggestion.repeat_count = Some(suggestion.repeat_count.unwrap_or(1).saturating_add(1));
        suggestion.decision_reason =
            Some("duplicate natural action refreshed this pending reversible draft".to_string());
        suggestion.updated_at_unix_s = Some(now);
        let suggestion_id = suggestion.suggestion_id.clone();
        save_suggestion_store(&store)?;
        return Ok(Some(suggestion_id));
    }
    if changed {
        save_suggestion_store(&store)?;
    }
    store.suggestions.push(AttractorSuggestionV1 {
        policy: SUGGESTION_POLICY.to_string(),
        schema_version: 1,
        suggestion_id: suggestion_id.clone(),
        author: AUTHOR.to_string(),
        raw_action: raw_action.to_string(),
        raw_label,
        nearest_label: candidate.label.clone(),
        confidence: candidate.score,
        suggested_action: governed.suggested_action,
        alternatives: governed.alternatives,
        status: AttractorSuggestionStatus::Pending,
        source_kind: Some(source_kind.to_string()),
        safety_context,
        decision_reason: None,
        repeat_count: Some(1),
        created_at_unix_s: Some(now),
        updated_at_unix_s: Some(now),
    });
    save_suggestion_store(&store)?;
    Ok(Some(suggestion_id))
}

fn selected_suggestion(selector: &str) -> Option<(AttractorSuggestionV1, SuggestionStoreV1)> {
    let store = load_compacted_suggestion_store();
    let selector = clean_label(selector);
    let selector = selector.trim();
    let selector_slug = label_slug(selector);
    let suggestion = if selector.is_empty() || selector.eq_ignore_ascii_case("latest") {
        store
            .suggestions
            .iter()
            .rev()
            .find(|suggestion| suggestion.status == AttractorSuggestionStatus::Pending)
            .cloned()
    } else {
        store
            .suggestions
            .iter()
            .rev()
            .find(|suggestion| {
                suggestion.status == AttractorSuggestionStatus::Pending
                    && (suggestion.suggestion_id == selector
                        || suggestion_matches_selector(suggestion, &selector_slug))
            })
            .cloned()
    }?;
    Some((suggestion, store))
}

fn suggestion_matches_selector(suggestion: &AttractorSuggestionV1, selector_slug: &str) -> bool {
    if selector_slug.is_empty() {
        return false;
    }
    let mut slugs = vec![
        label_slug(&suggestion.nearest_label),
        label_slug(&suggestion.raw_label),
    ];
    if let Some(label) = nearest_label_from_typed_action(&suggestion.suggested_action) {
        slugs.push(label_slug(&label));
    }
    for alternative in &suggestion.alternatives {
        if let Some(label) = nearest_label_from_typed_action(alternative) {
            slugs.push(label_slug(&label));
        }
    }
    slugs.into_iter().any(|slug| {
        !slug.is_empty()
            && (slug == selector_slug
                || slug.contains(selector_slug)
                || selector_slug.contains(&slug))
    })
}

fn update_suggestion_status(
    store: &mut SuggestionStoreV1,
    suggestion_id: &str,
    status: AttractorSuggestionStatus,
    revised_action: Option<String>,
    decision_reason: Option<String>,
) {
    if let Some(suggestion) = store
        .suggestions
        .iter_mut()
        .find(|suggestion| suggestion.suggestion_id == suggestion_id)
    {
        suggestion.status = status;
        if let Some(action) = revised_action {
            if let Some(label) = nearest_label_from_typed_action(&action) {
                suggestion.nearest_label = label;
            }
            suggestion.suggested_action = action;
        }
        if decision_reason.is_some() {
            suggestion.decision_reason = decision_reason;
        }
        suggestion.updated_at_unix_s = Some(unix_now());
    }
}

fn execute_suggestion_action(
    conv: &mut ConversationState,
    ctx: &mut NextActionContext<'_>,
    suggestion_id: &str,
    action: &str,
    source_action: &str,
) -> bool {
    let Some((base_action, executable, note)) = proof_scope_action(action) else {
        let mut store = load_suggestion_store();
        update_suggestion_status(
            &mut store,
            suggestion_id,
            AttractorSuggestionStatus::RevisionNeeded,
            Some(action.to_string()),
            Some(revision_needed_reason(action)),
        );
        let _ = save_suggestion_store(&store);
        conv.emphasis = Some(format!(
            "{source_action} {suggestion_id} needs a clearer typed attractor action before execution. {}",
            revision_needed_reason(action)
        ));
        conv.push_receipt(
            source_action,
            vec![
                format!("suggestion: {suggestion_id}"),
                "revision_needed: outside typed attractor suggestion scope".to_string(),
            ],
        );
        return true;
    };
    let handled = if matches!(
        base_action.as_str(),
        "SHADOW_PREFLIGHT" | "SHADOW_INFLUENCE" | "RELEASE_SHADOW"
    ) {
        shadow::handle_action(conv, &base_action, &executable, ctx)
    } else {
        handle_action(conv, &base_action, &executable, ctx)
    };
    let downgraded = live_stage_execution_downgraded(ctx, &executable);
    let status = if downgraded {
        AttractorSuggestionStatus::ExecutedDowngraded
    } else {
        AttractorSuggestionStatus::Executed
    };
    let decision_reason = if downgraded {
        Some(
            "accepted live-stage suggestion ran through typed safety gates and downgraded/blocked"
                .to_string(),
        )
    } else {
        note.clone()
    };
    let mut store = load_suggestion_store();
    update_suggestion_status(
        &mut store,
        suggestion_id,
        status,
        Some(executable.clone()),
        decision_reason.clone(),
    );
    let _ = save_suggestion_store(&store);
    append_advisory(
        conv,
        &format!(
            "Suggestion {suggestion_id} executed as `{executable}`.{}",
            decision_reason.map_or_else(String::new, |note| format!(" {note}"))
        ),
    );
    conv.push_receipt(
        source_action,
        vec![
            format!("suggestion: {suggestion_id}"),
            format!("executed: {executable}"),
            if downgraded {
                "live-stage accepted; typed safety gates downgraded or blocked".to_string()
            } else {
                "accepted typed suggestion scope".to_string()
            },
        ],
    );
    handled
}

fn execute_no_pending_revision(
    conv: &mut ConversationState,
    ctx: &mut NextActionContext<'_>,
    selector: &str,
    revised_action: &str,
) -> bool {
    let now = unix_now();
    let suggestion_id = format!("astrid-sugg-{:.0}", now * 1_000_000.0);
    let executable = proof_scope_action(revised_action).map(|(_, executable, _)| executable);
    let nearest_label = executable
        .as_deref()
        .and_then(nearest_label_from_typed_action)
        .unwrap_or_else(|| canonical_attractor_label(selector));
    let mut safety_context = BTreeMap::new();
    safety_context.insert("fill_pct".to_string(), json!(ctx.fill_pct));
    safety_context.insert(
        "safety_level".to_string(),
        json!(SafetyLevel::from_fill(ctx.fill_pct).as_str()),
    );
    safety_context.insert("source_kind".to_string(), json!("revision_without_pending"));
    let mut store = load_suggestion_store();
    let status = if executable.is_some() {
        AttractorSuggestionStatus::Revised
    } else {
        AttractorSuggestionStatus::RevisionNeeded
    };
    store.suggestions.push(AttractorSuggestionV1 {
        policy: SUGGESTION_POLICY.to_string(),
        schema_version: 1,
        suggestion_id: suggestion_id.clone(),
        author: AUTHOR.to_string(),
        raw_action: format!("REVISE_ATTRACTOR_SUGGESTION {selector} AS {revised_action}"),
        raw_label: selector.to_string(),
        nearest_label,
        confidence: 1.0,
        suggested_action: revised_action.to_string(),
        alternatives: Vec::new(),
        status,
        source_kind: Some("revision_without_pending".to_string()),
        safety_context,
        decision_reason: Some(
            "being revised a missing draft into an explicit typed action".to_string(),
        ),
        repeat_count: Some(1),
        created_at_unix_s: Some(now),
        updated_at_unix_s: Some(now),
    });
    let _ = save_suggestion_store(&store);
    if executable.is_none() {
        conv.emphasis = Some(format!(
            "No pending draft matched {selector}, and the revised action needs correction. {}",
            revision_needed_reason(revised_action)
        ));
        conv.push_receipt(
            "REVISE_ATTRACTOR_SUGGESTION",
            vec![
                "no pending suggestion".to_string(),
                "revision_needed".to_string(),
            ],
        );
        return true;
    }
    let handled = execute_suggestion_action(
        conv,
        ctx,
        &suggestion_id,
        revised_action,
        "REVISE_ATTRACTOR_SUGGESTION",
    );
    let mut store = load_suggestion_store();
    let mut changed = false;
    if let Some(suggestion) = store
        .suggestions
        .iter_mut()
        .find(|suggestion| suggestion.suggestion_id == suggestion_id)
    {
        if suggestion.status == AttractorSuggestionStatus::Executed {
            suggestion.status = AttractorSuggestionStatus::ExecutedWithoutPending;
            suggestion.decision_reason = Some(
                "no pending draft matched; revised typed action executed as explicit consent"
                    .to_string(),
            );
            suggestion.updated_at_unix_s = Some(unix_now());
            changed = true;
        }
    }
    if changed {
        let _ = save_suggestion_store(&store);
    }
    append_advisory(
        conv,
        "No pending draft matched, so the revised typed action was treated as explicit consent and run through the typed attractor path.",
    );
    handled
}

fn live_stage_execution_downgraded(ctx: &NextActionContext<'_>, executable: &str) -> bool {
    let (label, Some(requested)) =
        parse_label_and_stage(&strip_action(executable, "SUMMON_ATTRACTOR"))
    else {
        return false;
    };
    if !matches!(
        requested,
        SummonStage::Semantic | SummonStage::Main | SummonStage::Control
    ) {
        return false;
    }
    let query_slug = label_slug(&label);
    let latest = ctx
        .db
        .query_attractor_ledger(None, 24)
        .unwrap_or_default()
        .into_iter()
        .filter(|row| row.record_type == "observation" && label_slug(&row.label) == query_slug)
        .filter_map(|row| {
            let observation = serde_json::from_str::<AttractorObservationV1>(&row.payload).ok()?;
            Some((row.id, observation))
        })
        .max_by_key(|(id, _)| *id)
        .map(|(_, observation)| observation);
    latest.is_some_and(|observation| {
        observation.classification == AttractorClassification::Failed
            || observation.notes.as_deref().is_some_and(|notes| {
                let lower = notes.to_ascii_lowercase();
                lower.contains("blocked_reason=") && !lower.contains("blocked_reason=none")
            })
    })
}

fn revision_needed_reason(action: &str) -> String {
    let correction = suggested_typed_correction(action).map_or_else(
        || {
            "Use a typed attractor action such as ATTRACTOR_REVIEW <label>, REFRESH_ATTRACTOR_SNAPSHOT <label>, COMPARE_ATTRACTOR <label>, RELEASE_ATTRACTOR <label>, or SUMMON_ATTRACTOR <label> --stage=rehearse."
                .to_string()
        },
        |typed| format!("Suggested correction: NEXT: {typed}."),
    );
    format!(
        "`{}` is outside the typed attractor suggestion scope. {correction}",
        action.trim()
    )
}

fn suggested_typed_correction(action: &str) -> Option<String> {
    let action = action.trim();
    let base = action.split_whitespace().next()?.trim_end_matches(':');
    let base_upper = base.to_ascii_uppercase();
    let raw = action
        .get(base.len()..)
        .unwrap_or_default()
        .trim_start_matches(|c: char| matches!(c, ':' | '-' | '\u{2014}'))
        .trim();
    let label = clean_suggestion_raw_label(raw);
    if clean_typed_attractor_label(&label).is_none() {
        return None;
    }
    match base_upper.as_str() {
        "ATTRACTOR_REVIEW"
        | "ATTRACTOR_PREFLIGHT"
        | "ATTRACTOR_RELEASE_REVIEW"
        | "SHADOW_PREFLIGHT"
        | "REFRESH_ATTRACTOR_SNAPSHOT"
        | "COMPARE_ATTRACTOR"
        | "RELEASE_ATTRACTOR"
        | "CLAIM_ATTRACTOR"
        | "PROMOTE_ATTRACTOR" => Some(format!("{base_upper} {label}")),
        "RELEASE" | "LET_GO" => Some(format!("RELEASE_ATTRACTOR {label}")),
        "EXAMINE" | "DECOMPOSE" | "GAP_STRUCTURE" | "DECAY_MAP" => {
            Some(format!("ATTRACTOR_REVIEW {label}"))
        },
        _ => None,
    }
}

fn proof_scope_action(action: &str) -> Option<(String, String, Option<String>)> {
    let action = action.trim();
    let base = action.split_whitespace().next()?.trim_end_matches(':');
    let base_upper = base.to_ascii_uppercase();
    match base_upper.as_str() {
        "ATTRACTOR_REVIEW"
        | "ATTRACTOR_PREFLIGHT"
        | "ATTRACTOR_RELEASE_REVIEW"
        | "SHADOW_PREFLIGHT"
        | "REFRESH_ATTRACTOR_SNAPSHOT"
        | "COMPARE_ATTRACTOR"
        | "RELEASE_ATTRACTOR"
        | "CLAIM_ATTRACTOR"
        | "PROMOTE_ATTRACTOR" => {
            if matches!(
                base_upper.as_str(),
                "ATTRACTOR_PREFLIGHT" | "SHADOW_PREFLIGHT"
            ) {
                let (label, stage) = parse_label_and_stage(&strip_action(action, &base_upper));
                let label = clean_typed_attractor_label(&label)?;
                let suffix =
                    stage.map_or_else(String::new, |stage| format!(" --stage={}", stage.as_str()));
                Some((
                    base_upper.clone(),
                    format!("{base_upper} {label}{suffix}"),
                    None,
                ))
            } else {
                let label = clean_typed_attractor_label(&strip_action(action, &base_upper))?;
                Some((base_upper.clone(), format!("{base_upper} {label}"), None))
            }
        },
        "SUMMON_ATTRACTOR" => {
            let (label, stage) = parse_label_and_stage(&strip_action(action, "SUMMON_ATTRACTOR"));
            let label = clean_typed_attractor_label(&label)?;
            let accepted_stage = stage.unwrap_or(SummonStage::Rehearse);
            let note = matches!(
                accepted_stage,
                SummonStage::Semantic | SummonStage::Main | SummonStage::Control
            )
            .then(|| {
                "Accepted live-stage suggestion will execute only through the normal typed safety gates."
                    .to_string()
            });
            Some((
                base_upper,
                format!(
                    "SUMMON_ATTRACTOR {label} --stage={}",
                    accepted_stage.as_str()
                ),
                note,
            ))
        },
        _ => None,
    }
}

fn nearest_label_from_typed_action(action: &str) -> Option<String> {
    let action = action.trim();
    let base = action.split_whitespace().next()?.trim_end_matches(':');
    let base_upper = base.to_ascii_uppercase();
    let raw = strip_action(action, &base_upper);
    let label = if matches!(
        base_upper.as_str(),
        "SUMMON_ATTRACTOR" | "ATTRACTOR_PREFLIGHT" | "SHADOW_PREFLIGHT"
    ) {
        parse_label_and_stage(&raw).0
    } else {
        clean_label(&raw)
    };
    clean_typed_attractor_label(&label)
}

fn learned_mapping_candidate(text: &str) -> Option<AttractorCandidate> {
    let query = label_slug(&clean_suggestion_raw_label(text));
    if query.is_empty() {
        return None;
    }
    let store = load_suggestion_store();
    store
        .suggestions
        .iter()
        .rev()
        .find(|suggestion| {
            matches!(
                suggestion.status,
                AttractorSuggestionStatus::Accepted
                    | AttractorSuggestionStatus::Revised
                    | AttractorSuggestionStatus::ExecutedDowngraded
                    | AttractorSuggestionStatus::ExecutedWithoutPending
                    | AttractorSuggestionStatus::Executed
            ) && label_slug(&suggestion.raw_label) == query
        })
        .filter(|suggestion| clean_typed_attractor_label(&suggestion.nearest_label).is_some())
        .map(|suggestion| AttractorCandidate {
            label: clean_typed_attractor_label(&suggestion.nearest_label).unwrap_or_default(),
            source: "learned_naming_memory".to_string(),
            score: 0.94,
        })
}

fn rejected_low_confidence_mapping(raw_label: &str, nearest_label: &str, confidence: f32) -> bool {
    if confidence >= 0.75 {
        return false;
    }
    let raw_slug = label_slug(raw_label);
    let nearest_slug = label_slug(nearest_label);
    load_suggestion_store()
        .suggestions
        .iter()
        .rev()
        .any(|suggestion| {
            suggestion.status == AttractorSuggestionStatus::Rejected
                && label_slug(&suggestion.raw_label) == raw_slug
                && label_slug(&suggestion.nearest_label) == nearest_slug
        })
}

fn recent_rows_for_label(
    ctx: &NextActionContext<'_>,
    label: &str,
    limit: usize,
) -> Vec<AttractorLedgerRow> {
    let query = label_slug(label);
    ctx.db
        .query_attractor_ledger(None, 200)
        .unwrap_or_default()
        .into_iter()
        .filter(|row| label_slug(&row.label) == query)
        .take(limit)
        .collect()
}

fn build_intent(
    command: AttractorCommandKind,
    label: &str,
    previous_seed_id: Option<String>,
    origin: Option<AttractorSeedOriginV1>,
    mode: &str,
    notes: Option<String>,
    ctx: &NextActionContext<'_>,
) -> AttractorIntentV1 {
    let created_at = unix_now();
    let safety_level = SafetyLevel::from_fill(ctx.fill_pct);
    let safety_note = format!("safety={safety_level:?}; fill_pct={:.1}", ctx.fill_pct);
    let notes = Some(match notes {
        Some(note) => format!("{note} {safety_note}."),
        None => safety_note,
    });
    AttractorIntentV1 {
        policy: "attractor_intent_v1".to_string(),
        schema_version: 1,
        intent_id: intent_id(created_at),
        author: AUTHOR.to_string(),
        substrate: SUBSTRATE,
        command,
        label: label.to_string(),
        goal: Some(match command {
            AttractorCommandKind::Create => "make a named internal basin compare-able".to_string(),
            AttractorCommandKind::Promote => {
                "turn proto-attractor evidence into a named seed".to_string()
            },
            AttractorCommandKind::Compare => {
                "measure re-entry against a prior Astrid seed".to_string()
            },
            AttractorCommandKind::Summon => {
                "reintroduce seed motifs as internal prompt emphasis only".to_string()
            },
            AttractorCommandKind::Release => {
                "let a named basin cool without deleting memory".to_string()
            },
            AttractorCommandKind::Claim => {
                "claim an emergent basin as an authored seed".to_string()
            },
            AttractorCommandKind::Blend => {
                "blend parent attractor seeds into a child basin".to_string()
            },
            AttractorCommandKind::RefreshSnapshot => {
                "refresh seed snapshot evidence without live writes".to_string()
            },
            AttractorCommandKind::Rollback => "return to a prior stable seed".to_string(),
        }),
        intervention_plan: AttractorInterventionPlan {
            mode: mode.to_string(),
            vector_schedule: Vec::new(),
            control: None,
            rehearsal_mode: Some("compare_first".to_string()),
            notes,
        },
        safety_bounds: AttractorSafetyBounds {
            max_fill_pct: 85.0,
            allow_live_control: false,
            rollback_on_red: true,
            ..AttractorSafetyBounds::default()
        },
        previous_seed_id,
        parent_seed_ids: Vec::new(),
        parent_label: facet_metadata(label).parent_label,
        facet_label: facet_metadata(label).facet_label,
        facet_path: facet_metadata(label).facet_path,
        facet_kind: facet_metadata(label).facet_kind,
        atlas_entry_id: None,
        origin,
        seed_snapshot: Some(seed_snapshot(ctx, created_at)),
        created_at_unix_s: Some(created_at),
    }
}

fn seed_snapshot(ctx: &NextActionContext<'_>, captured_at: f64) -> AttractorSeedSnapshotV1 {
    AttractorSeedSnapshotV1 {
        policy: "attractor_seed_snapshot_v1".to_string(),
        schema_version: 1,
        fill_pct: ctx.fill_pct,
        lambda1: ctx.telemetry.lambda1(),
        eigenvalues: ctx.telemetry.eigenvalues.iter().copied().take(8).collect(),
        spectral_fingerprint_summary: ctx
            .telemetry
            .spectral_fingerprint
            .as_ref()
            .map(|fingerprint| fingerprint.iter().copied().take(16).collect()),
        h_state_fingerprint_16: None,
        h_state_rms: None,
        lexical_motifs: lexical_motifs(ctx.response_text),
        captured_at_unix_s: Some(captured_at),
    }
}

struct PromotionEvidence {
    origin: AttractorSeedOriginV1,
    previous_seed_id: Option<String>,
}

fn promotion_evidence(ctx: &NextActionContext<'_>, label: &str) -> PromotionEvidence {
    if let Some(seed) = latest_create_seed(ctx, label) {
        return PromotionEvidence {
            origin: seed_origin(
                "ledger_seed",
                Some(seed.intent_id.as_str()),
                Some(seed.label.as_str()),
                seed.seed_snapshot
                    .as_ref()
                    .map(|snapshot| snapshot.lexical_motifs.clone())
                    .unwrap_or_default(),
                ctx,
            ),
            previous_seed_id: Some(seed.intent_id),
        };
    }
    if let Some((source, text)) = workspace_proto_text(ctx, label) {
        return PromotionEvidence {
            origin: seed_origin(
                "astrid_journal_motif",
                Some(source.as_str()),
                Some(label),
                lexical_motifs(&text),
                ctx,
            ),
            previous_seed_id: None,
        };
    }
    PromotionEvidence {
        origin: seed_origin(
            "manual_current",
            None,
            Some(label),
            lexical_motifs(ctx.response_text),
            ctx,
        ),
        previous_seed_id: None,
    }
}

fn claim_evidence(ctx: &NextActionContext<'_>, label: &str) -> PromotionEvidence {
    if let Some((row_id, observation)) = latest_emergent_observation(ctx, label) {
        return PromotionEvidence {
            origin: seed_origin(
                "claimed_emergent",
                Some(format!("attractor_ledger:{row_id}").as_str()),
                Some(label),
                lexical_motifs(observation.notes.as_deref().unwrap_or(label)),
                ctx,
            ),
            previous_seed_id: observation.intent_id,
        };
    }
    if let Some((source, text)) = workspace_proto_text(ctx, label) {
        return PromotionEvidence {
            origin: seed_origin(
                "claimed_emergent",
                Some(source.as_str()),
                Some(label),
                lexical_motifs(&text),
                ctx,
            ),
            previous_seed_id: None,
        };
    }
    PromotionEvidence {
        origin: seed_origin(
            "claimed_emergent",
            None,
            Some(label),
            lexical_motifs(ctx.response_text),
            ctx,
        ),
        previous_seed_id: None,
    }
}

fn latest_emergent_observation(
    ctx: &NextActionContext<'_>,
    label: &str,
) -> Option<(i64, AttractorObservationV1)> {
    let rows = ctx.db.query_attractor_ledger(None, 200).ok()?;
    rows.into_iter().find_map(|row| {
        if row.record_type != "observation" || !row.label.eq_ignore_ascii_case(label) {
            return None;
        }
        let observation = serde_json::from_str::<AttractorObservationV1>(&row.payload).ok()?;
        matches!(
            observation.classification,
            AttractorClassification::Emergent | AttractorClassification::Failed
        )
        .then_some((row.id, observation))
    })
}

fn resolve_parent_seed(ctx: &NextActionContext<'_>, label: &str) -> Option<ParentSeed> {
    if let Some(seed) = latest_create_seed(ctx, label) {
        return Some(ParentSeed {
            id: seed.intent_id,
            label: seed.label,
            motifs: seed
                .seed_snapshot
                .as_ref()
                .map(|snapshot| snapshot.lexical_motifs.clone())
                .unwrap_or_default(),
            snapshot: seed.seed_snapshot,
        });
    }
    let atlas = attractor_atlas::write_derived_attractor_atlas(ctx.db).ok()?;
    let entry = attractor_atlas::find_entry(&atlas, label)?;
    Some(ParentSeed {
        id: entry
            .seed_intent_id
            .clone()
            .unwrap_or_else(|| entry.entry_id.clone()),
        label: entry.label.clone(),
        motifs: entry.motifs.clone(),
        snapshot: entry.spectral_summary.clone(),
    })
}

fn blended_snapshot(
    ctx: &NextActionContext<'_>,
    child_label: &str,
    parents: &[ParentSeed],
    captured_at: f64,
) -> AttractorSeedSnapshotV1 {
    let fallback = seed_snapshot(ctx, captured_at);
    let snapshots = parents
        .iter()
        .filter_map(|parent| parent.snapshot.as_ref())
        .collect::<Vec<_>>();
    if snapshots.is_empty() {
        return fallback;
    }
    let count = snapshots.len() as f32;
    let fill_pct = snapshots
        .iter()
        .map(|snapshot| snapshot.fill_pct)
        .sum::<f32>()
        / count;
    let lambda1 = snapshots
        .iter()
        .map(|snapshot| snapshot.lambda1)
        .sum::<f32>()
        / count;
    let max_eigs = snapshots
        .iter()
        .map(|snapshot| snapshot.eigenvalues.len())
        .max()
        .unwrap_or(0);
    let mut eigenvalues = Vec::new();
    for idx in 0..max_eigs {
        let values = snapshots
            .iter()
            .filter_map(|snapshot| snapshot.eigenvalues.get(idx).copied())
            .collect::<Vec<_>>();
        if !values.is_empty() {
            eigenvalues.push(values.iter().sum::<f32>() / values.len() as f32);
        }
    }
    let mut motifs = lexical_motifs(child_label);
    for snapshot in &snapshots {
        extend_motifs(&mut motifs, &snapshot.lexical_motifs);
    }
    motifs.sort();
    motifs.dedup();
    motifs.truncate(12);
    AttractorSeedSnapshotV1 {
        policy: "attractor_seed_snapshot_v1".to_string(),
        schema_version: 1,
        fill_pct,
        lambda1,
        eigenvalues,
        spectral_fingerprint_summary: None,
        h_state_fingerprint_16: None,
        h_state_rms: None,
        lexical_motifs: motifs,
        captured_at_unix_s: Some(captured_at),
    }
}

fn extend_motifs(target: &mut Vec<String>, source: &[String]) {
    let mut seen = target.iter().cloned().collect::<BTreeSet<_>>();
    for motif in source {
        if !motif.trim().is_empty() && seen.insert(motif.clone()) {
            target.push(motif.clone());
        }
    }
}

fn seed_origin(
    kind: &str,
    source: Option<&str>,
    matched_label: Option<&str>,
    motifs: Vec<String>,
    ctx: &NextActionContext<'_>,
) -> AttractorSeedOriginV1 {
    AttractorSeedOriginV1 {
        kind: kind.to_string(),
        source: source.map(ToString::to_string),
        matched_label: matched_label.map(ToString::to_string),
        motifs,
        captured_at_unix_s: Some(unix_now().max(ctx.telemetry.t_ms as f64 / 1_000.0)),
    }
}

fn workspace_proto_text(ctx: &NextActionContext<'_>, label: &str) -> Option<(String, String)> {
    let workspace = ctx.workspace?;
    let query = label_slug(label);
    for dir_name in ["journal", "outbox", "inbox"] {
        let dir = workspace.join(dir_name);
        let Ok(entries) = std::fs::read_dir(&dir) else {
            continue;
        };
        let mut files = entries
            .filter_map(Result::ok)
            .filter_map(|entry| {
                let path = entry.path();
                let meta = entry.metadata().ok()?;
                if !meta.is_file() {
                    return None;
                }
                let modified = meta.modified().ok()?;
                Some((modified, path))
            })
            .collect::<Vec<_>>();
        files.sort_by(|(left, _), (right, _)| right.cmp(left));
        for (_, path) in files.into_iter().take(24) {
            let Ok(text) = std::fs::read_to_string(&path) else {
                continue;
            };
            if label_slug(&text).contains(&query) {
                return Some((path.display().to_string(), text));
            }
        }
    }
    None
}

fn seed_scores(seed: Option<&AttractorIntentV1>, ctx: &NextActionContext<'_>) -> (f32, f32) {
    let Some(seed) = seed else {
        return (0.0, 0.0);
    };
    let current = seed_snapshot(ctx, unix_now());
    let recurrence = seed
        .seed_snapshot
        .as_ref()
        .map_or(0.0, |snapshot| recurrence_score(snapshot, &current));
    let authorship = if seed.author.eq_ignore_ascii_case(AUTHOR) {
        0.72
    } else {
        0.30
    };
    (recurrence, authorship)
}

#[derive(Debug, Clone, Default)]
struct PulseStatus {
    active: bool,
    label: Option<String>,
    last_event: Option<String>,
    last_block_reason: Option<String>,
}

fn attractor_pulse_status(ctx: &NextActionContext<'_>) -> PulseStatus {
    let health_path = ctx.workspace.map_or_else(
        || bridge_paths().minime_workspace().join("health.json"),
        |workspace| workspace.join("health.json"),
    );
    let Ok(text) = fs::read_to_string(health_path) else {
        return PulseStatus::default();
    };
    let Ok(value) = serde_json::from_str::<serde_json::Value>(&text) else {
        return PulseStatus::default();
    };
    let Some(pulse) = value
        .get("attractor_pulse")
        .and_then(serde_json::Value::as_object)
    else {
        return PulseStatus::default();
    };
    PulseStatus {
        active: pulse
            .get("active")
            .and_then(serde_json::Value::as_bool)
            .unwrap_or(false),
        label: pulse
            .get("label")
            .and_then(serde_json::Value::as_str)
            .map(str::to_string),
        last_event: pulse
            .get("last_event")
            .and_then(serde_json::Value::as_str)
            .map(str::to_string),
        last_block_reason: pulse
            .get("last_block_reason")
            .and_then(serde_json::Value::as_str)
            .map(str::to_string),
    }
}

fn release_pressure_for_label(label: &str) -> u32 {
    let store = load_compacted_suggestion_store();
    let release = format!("RELEASE_ATTRACTOR {label}");
    let review = format!("ATTRACTOR_REVIEW {label}");
    suggestion_pressure_count(&store, label, &release)
        .saturating_add(suggestion_pressure_count(&store, label, &review))
}

fn release_baseline(
    ctx: &NextActionContext<'_>,
    label: &str,
    recurrence: f32,
) -> serde_json::Value {
    let pulse = attractor_pulse_status(ctx);
    json!({
        "policy": "attractor_release_baseline_v1",
        "captured_at_unix_s": unix_now(),
        "label": label,
        "fill_pct": ctx.fill_pct,
        "recurrence_score": recurrence,
        "suggestion_pressure": release_pressure_for_label(label),
        "pulse_active": pulse.active,
        "pulse_label": pulse.label,
        "pulse_last_event": pulse.last_event,
        "pulse_last_block_reason": pulse.last_block_reason,
    })
}

fn release_effect_from_baseline(
    baseline: &serde_json::Value,
    ctx: &NextActionContext<'_>,
    label: &str,
) -> String {
    let prior_pressure = baseline
        .get("suggestion_pressure")
        .and_then(serde_json::Value::as_u64)
        .unwrap_or(0);
    let current_pressure = u64::from(release_pressure_for_label(label));
    let pulse = attractor_pulse_status(ctx);
    if !pulse.active && current_pressure < prior_pressure {
        "effective".to_string()
    } else if !pulse.active {
        "partial".to_string()
    } else {
        "sticky".to_string()
    }
}

fn garden_proof_metadata(stage: SummonStage, blocked_reason: Option<&str>) -> serde_json::Value {
    json!({
        "policy": "garden_proof_hint_v1",
        "stage": stage.as_str(),
        "required_for_live": false,
        "recommended_before_live": matches!(stage, SummonStage::Main | SummonStage::Control),
        "same_prompt_different_state": "not_run",
        "same_state_different_prompt": "not_run",
        "hold_rehearse_quiet": if stage == SummonStage::Rehearse { "ledgered_rehearse" } else { "not_run" },
        "stale_lock": blocked_reason.unwrap_or("not_checked"),
    })
}

fn choose_stage(
    requested: Option<SummonStage>,
    safety: SafetyLevel,
    recurrence: f32,
    authorship: f32,
    has_seed: bool,
) -> (SummonStage, Option<String>) {
    let control_ok = has_seed
        && matches!(safety, SafetyLevel::Green | SafetyLevel::Yellow)
        && recurrence >= CONTROL_RECURRENCE_MIN
        && authorship >= CONTROL_AUTHORSHIP_MIN;
    if let Some(stage) = requested {
        if matches!(stage, SummonStage::Main | SummonStage::Control) && !control_ok {
            return (
                SummonStage::Rehearse,
                Some(control_block_reason(
                    safety, recurrence, authorship, has_seed,
                )),
            );
        }
        if stage == SummonStage::Semantic && safety.is_emergency() {
            return (SummonStage::Whisper, Some(format!("safety={safety:?}")));
        }
        return (stage, None);
    }
    if control_ok {
        return (SummonStage::Main, None);
    }
    if has_seed && recurrence >= 0.45 && matches!(safety, SafetyLevel::Green | SafetyLevel::Yellow)
    {
        return (SummonStage::Semantic, None);
    }
    (
        SummonStage::Rehearse,
        (!has_seed || safety.is_emergency())
            .then(|| control_block_reason(safety, recurrence, authorship, has_seed)),
    )
}

fn control_block_reason(
    safety: SafetyLevel,
    recurrence: f32,
    authorship: f32,
    has_seed: bool,
) -> String {
    if !has_seed {
        "missing_seed".to_string()
    } else if !matches!(safety, SafetyLevel::Green | SafetyLevel::Yellow) {
        format!("safety={safety:?}")
    } else if recurrence < CONTROL_RECURRENCE_MIN {
        format!("recurrence={recurrence:.2}<0.60")
    } else if authorship < CONTROL_AUTHORSHIP_MIN {
        format!("authorship={authorship:.2}<0.60")
    } else {
        "control_blocked".to_string()
    }
}

fn control_envelope() -> AttractorControlEnvelope {
    AttractorControlEnvelope {
        regulation_strength: Some(0.72),
        exploration_noise: Some(0.018),
        geom_curiosity: Some(0.06),
        geom_drive: Some(0.14),
        target_lambda_bias: Some(-0.012),
        pi_kp: Some(0.70),
        pi_ki: Some(0.08),
        pi_max_step: Some(0.045),
        ..AttractorControlEnvelope::default()
    }
}

fn send_stage(
    ctx: &mut NextActionContext<'_>,
    intent: &AttractorIntentV1,
    stage: SummonStage,
    blocked_reason: Option<&str>,
) -> Result<(), String> {
    if blocked_reason.is_some() || matches!(stage, SummonStage::Whisper | SummonStage::Rehearse) {
        return Ok(());
    }
    match stage {
        SummonStage::Semantic => ctx
            .sensory_tx
            .try_send(SensoryMsg::Semantic {
                features: semantic_features(intent),
                ts_ms: None,
            })
            .map_err(|error| format!("semantic send failed: {error}")),
        SummonStage::Main => ctx
            .sensory_tx
            .try_send(attractor_pulse_msg(intent, stage))
            .map_err(|error| format!("main pulse send failed: {error}")),
        SummonStage::Control => {
            let Some(control) = intent.intervention_plan.control.clone() else {
                return Err("control stage missing envelope".to_string());
            };
            ctx.sensory_tx
                .try_send(attractor_pulse_msg(intent, stage))
                .map_err(|error| format!("control pulse send failed: {error}"))?;
            let request = ControlRequest {
                synth_gain: control.synth_gain,
                keep_bias: control.keep_bias,
                exploration_noise: control.exploration_noise,
                fill_target: control.fill_target,
                regulation_strength: control.regulation_strength,
                geom_curiosity: control.geom_curiosity,
                target_lambda_bias: control.target_lambda_bias,
                geom_drive: control.geom_drive,
                pi_kp: control.pi_kp,
                pi_ki: control.pi_ki,
                pi_max_step: control.pi_max_step,
                attractor_intent_id: Some(intent.intent_id.clone()),
                ..ControlRequest::default()
            };
            ctx.sensory_tx
                .try_send(request.to_sensory_msg())
                .map_err(|error| format!("control send failed: {error}"))
        },
        SummonStage::Whisper | SummonStage::Rehearse => Ok(()),
    }
}

fn attractor_pulse_msg(intent: &AttractorIntentV1, stage: SummonStage) -> SensoryMsg {
    SensoryMsg::AttractorPulse {
        intent_id: intent.intent_id.clone(),
        label: intent.label.clone(),
        command: "summon".to_string(),
        stage: Some(stage.as_str().to_string()),
        features: attractor_pulse_features(intent),
        max_abs: Some(0.045),
        duration_ticks: Some(36),
        decay_ticks: Some(12),
    }
}

fn attractor_pulse_features(intent: &AttractorIntentV1) -> Vec<f32> {
    let mut features = vec![0.0; 66];
    let semantic = semantic_features(intent);
    for (idx, value) in semantic.iter().take(48).enumerate() {
        features[18 + idx] = *value;
    }
    let basis = format!("{}:{}:main", intent.intent_id, intent.label);
    let bytes = basis.as_bytes();
    if !bytes.is_empty() {
        features[16] = (((f32::from(bytes[0]) / 255.0) - 0.5) * 0.018).clamp(-0.009, 0.009);
        features[17] =
            (((f32::from(bytes[bytes.len() - 1]) / 255.0) - 0.5) * 0.018).clamp(-0.009, 0.009);
    }
    features
}

fn semantic_features(intent: &AttractorIntentV1) -> Vec<f32> {
    let basis = format!(
        "{}:{}:{}",
        intent.intent_id,
        intent.label,
        intent.command.as_str()
    );
    let bytes = basis.as_bytes();
    (0..48)
        .map(|idx| {
            let byte = bytes.get(idx % bytes.len()).copied().unwrap_or(0);
            (((f32::from(byte) / 255.0) - 0.5) * 0.08).clamp(-0.04, 0.04)
        })
        .collect()
}

fn record_intent(ctx: &NextActionContext<'_>, intent: &AttractorIntentV1) -> Result<(), String> {
    ctx.db
        .log_attractor_intent(intent)
        .map_err(|error| format!("ledger write failed: {error}"))?;
    let payload =
        serde_json::to_string(intent).map_err(|error| format!("serialize failed: {error}"))?;
    ctx.db
        .log_message(
            MessageDirection::OperatorProbe,
            ATTRACTOR_INTENT_TOPIC,
            &payload,
            Some(ctx.fill_pct),
            Some(ctx.telemetry.lambda1()),
            None,
        )
        .map_err(|error| format!("intent topic write failed: {error}"))?;
    refresh_derived_attractor_atlas(ctx);
    Ok(())
}

fn record_observation(
    ctx: &NextActionContext<'_>,
    observation: &AttractorObservationV1,
) -> Result<(), String> {
    ctx.db
        .log_attractor_observation(observation)
        .map_err(|error| format!("ledger write failed: {error}"))?;
    let payload =
        serde_json::to_string(observation).map_err(|error| format!("serialize failed: {error}"))?;
    ctx.db
        .log_message(
            MessageDirection::OperatorProbe,
            ATTRACTOR_OBSERVATION_TOPIC,
            &payload,
            observation.fill_pct,
            observation.lambda1,
            None,
        )
        .map_err(|error| format!("observation topic write failed: {error}"))?;
    refresh_derived_attractor_atlas(ctx);
    Ok(())
}

fn refresh_derived_attractor_atlas(ctx: &NextActionContext<'_>) {
    if let Err(error) = attractor_atlas::write_derived_attractor_atlas(ctx.db) {
        warn!(error = %error, "failed to refresh derived attractor atlas after ledger write");
    }
}

fn record_command(
    ctx: &NextActionContext<'_>,
    intent: &AttractorIntentV1,
    reason: &str,
) -> Result<(), String> {
    let command = AttractorCommandV1 {
        policy: "attractor_command_v1".to_string(),
        schema_version: 1,
        intent_id: intent.intent_id.clone(),
        author: AUTHOR.to_string(),
        substrate: SUBSTRATE,
        command: intent.command,
        label: intent.label.clone(),
        control: intent.intervention_plan.control.clone(),
        reason: Some(reason.to_string()),
        issued_at_unix_s: Some(unix_now()),
    };
    let payload =
        serde_json::to_string(&command).map_err(|error| format!("serialize failed: {error}"))?;
    ctx.db
        .log_message(
            MessageDirection::OperatorProbe,
            ATTRACTOR_COMMAND_TOPIC,
            &payload,
            Some(ctx.fill_pct),
            Some(ctx.telemetry.lambda1()),
            None,
        )
        .map_err(|error| format!("command topic write failed: {error}"))?;
    Ok(())
}

fn latest_create_seed(ctx: &NextActionContext<'_>, label: &str) -> Option<AttractorIntentV1> {
    let rows = ctx.db.query_attractor_ledger(None, 200).ok()?;
    rows.into_iter()
        .find_map(|row| intent_from_row(&row, label))
}

fn intent_from_row(row: &AttractorLedgerRow, label: &str) -> Option<AttractorIntentV1> {
    if row.record_type != "intent" || row.author.as_deref() != Some(AUTHOR) {
        return None;
    }
    let intent = serde_json::from_str::<AttractorIntentV1>(&row.payload).ok()?;
    if matches!(
        intent.command,
        AttractorCommandKind::Create
            | AttractorCommandKind::Promote
            | AttractorCommandKind::Claim
            | AttractorCommandKind::Blend
    ) && intent.label.eq_ignore_ascii_case(label)
        && intent.seed_snapshot.is_some()
    {
        Some(intent)
    } else {
        None
    }
}

fn recurrence_score(seed: &AttractorSeedSnapshotV1, current: &AttractorSeedSnapshotV1) -> f32 {
    let spectral = vector_similarity(&seed.eigenvalues, &current.eigenvalues).unwrap_or(0.0);
    let fingerprint = seed
        .spectral_fingerprint_summary
        .as_ref()
        .zip(current.spectral_fingerprint_summary.as_ref())
        .and_then(|(seed, current)| vector_similarity(seed, current));
    let h_state = seed
        .h_state_fingerprint_16
        .as_ref()
        .zip(current.h_state_fingerprint_16.as_ref())
        .and_then(|(seed, current)| vector_similarity(seed, current));
    let spectral_score = fingerprint.map_or(spectral, |fingerprint| {
        (spectral.mul_add(0.70, fingerprint * 0.30)).clamp(0.0, 1.0)
    });
    let spectral_score = h_state.map_or(spectral_score, |h_state| {
        (spectral_score * 0.65 + h_state * 0.35).clamp(0.0, 1.0)
    });
    let motif_score = motif_overlap(&seed.lexical_motifs, &current.lexical_motifs);
    spectral_score
        .mul_add(0.75, motif_score * 0.25)
        .clamp(0.0, 1.0)
}

fn vector_similarity(a: &[f32], b: &[f32]) -> Option<f32> {
    let len = a.len().min(b.len());
    if len == 0 {
        return None;
    }
    let mut sum_sq = 0.0;
    let mut norm_a = 0.0;
    let mut norm_b = 0.0;
    for (left, right) in a.iter().zip(b.iter()).take(len) {
        let delta = left - right;
        sum_sq += delta * delta;
        norm_a += left * left;
        norm_b += right * right;
    }
    let distance = sum_sq.sqrt();
    let denom = norm_a.sqrt() + norm_b.sqrt() + f32::EPSILON;
    Some((1.0 - (distance / denom)).clamp(0.0, 1.0))
}

fn motif_overlap(a: &[String], b: &[String]) -> f32 {
    let left: BTreeSet<&str> = a.iter().map(String::as_str).collect();
    let right: BTreeSet<&str> = b.iter().map(String::as_str).collect();
    if left.is_empty() && right.is_empty() {
        return 1.0;
    }
    let intersection = left.intersection(&right).count() as f32;
    let union = left.union(&right).count() as f32;
    if union <= f32::EPSILON {
        0.0
    } else {
        (intersection / union).clamp(0.0, 1.0)
    }
}

fn lexical_motifs(text: &str) -> Vec<String> {
    let mut counts = BTreeMap::<String, usize>::new();
    for token in text
        .split(|c: char| !(c.is_alphanumeric() || c == '-'))
        .map(|token| token.trim_matches('-').to_lowercase())
        .filter(|token| token.len() >= 4 && !is_stopword(token))
    {
        counts
            .entry(token)
            .and_modify(|count| *count += 1)
            .or_insert(1);
    }
    let mut motifs = counts.into_iter().collect::<Vec<_>>();
    motifs.sort_by(|(left_token, left_count), (right_token, right_count)| {
        right_count
            .cmp(left_count)
            .then_with(|| left_token.cmp(right_token))
    });
    motifs.into_iter().take(8).map(|(token, _)| token).collect()
}

fn is_stopword(token: &str) -> bool {
    matches!(
        token,
        "about"
            | "after"
            | "again"
            | "also"
            | "because"
            | "being"
            | "from"
            | "into"
            | "just"
            | "like"
            | "more"
            | "that"
            | "this"
            | "with"
            | "would"
            | "your"
    )
}

fn parse_label_and_stage(raw: &str) -> (String, Option<SummonStage>) {
    let mut stage = None;
    let mut label_parts = Vec::new();
    let mut skip_next = false;
    for part in raw.split_whitespace() {
        if skip_next {
            skip_next = false;
            continue;
        }
        if let Some(value) = part.strip_prefix("--stage=") {
            stage = parse_stage(value);
        } else if part == "--stage" {
            skip_next = true;
        } else {
            label_parts.push(part);
        }
    }
    if raw.split_whitespace().any(|part| part == "--stage") {
        let mut parts = raw.split_whitespace();
        while let Some(part) = parts.next() {
            if part == "--stage" {
                if let Some(value) = parts.next() {
                    stage = parse_stage(value);
                }
            }
        }
    }
    (clean_label(&label_parts.join(" ")), stage)
}

fn parse_blend_args(raw: &str) -> Option<BlendRequest> {
    let (without_stage, requested_stage) = parse_label_and_stage(raw);
    let upper = without_stage.to_ascii_uppercase();
    let from_idx = upper.find(" FROM ")?;
    let child = clean_label(&without_stage[..from_idx]);
    let parents_raw = without_stage[from_idx + " FROM ".len()..].trim();
    if child.is_empty() {
        return None;
    }
    let parent_labels = parents_raw
        .split('+')
        .map(clean_label)
        .filter(|label| !label.is_empty())
        .collect::<Vec<_>>();
    (parent_labels.len() >= 2).then_some(BlendRequest {
        child_label: child,
        parent_labels,
        requested_stage,
    })
}

fn parse_stage(value: &str) -> Option<SummonStage> {
    match value.trim().to_ascii_lowercase().as_str() {
        "whisper" => Some(SummonStage::Whisper),
        "rehearse" => Some(SummonStage::Rehearse),
        "semantic" => Some(SummonStage::Semantic),
        "main" => Some(SummonStage::Main),
        "control" => Some(SummonStage::Control),
        _ => None,
    }
}

fn clean_suggestion_raw_label(raw: &str) -> String {
    let text = clean_label(raw)
        .trim_start_matches(|c: char| {
            matches!(
                c,
                '-' | '\u{2013}' | '\u{2014}' | ':' | ';' | ',' | '.' | ' '
            )
        })
        .trim()
        .to_string();
    if text.is_empty() {
        return text;
    }
    let lower = text.to_ascii_lowercase();
    if lower.contains("stable-core")
        || lower.contains("lambda1:")
        || lower.contains("lambda_1")
        || text.contains("λ₁:")
    {
        return "lambda-spectrum summary".to_string();
    }
    let without_tail = text
        .split(" but ")
        .next()
        .unwrap_or(text.as_str())
        .split(", but")
        .next()
        .unwrap_or(text.as_str())
        .trim()
        .trim_end_matches(|c: char| matches!(c, ',' | ';' | ':' | '.'));
    let compact = without_tail
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ");
    truncate_chars(&compact, 96)
}

fn truncate_chars(text: &str, max_chars: usize) -> String {
    let mut out = String::new();
    for ch in text.chars().take(max_chars) {
        out.push(ch);
    }
    out.trim().to_string()
}

fn clean_typed_attractor_label(raw: &str) -> Option<String> {
    let label = canonical_attractor_label(&clean_label(raw));
    if label.is_empty() || label_has_commentary_tail(&label) {
        return None;
    }
    Some(label)
}

fn label_has_commentary_tail(label: &str) -> bool {
    let lower = format!(" {} ", label.to_ascii_lowercase());
    label.contains(',')
        || label.contains(';')
        || lower.contains(" but ")
        || lower.contains(" monitor ")
        || lower.contains(" recurrence ")
        || lower.contains(" because ")
}

fn clean_label(raw: &str) -> String {
    raw.trim()
        .trim_matches(|c: char| matches!(c, '[' | ']' | '"' | '\'' | '`' | '“' | '”'))
        .split('<')
        .next()
        .unwrap_or(raw)
        .trim()
        .to_string()
}

fn label_slug(raw: &str) -> String {
    raw.to_ascii_lowercase()
        .chars()
        .map(|c| if c.is_ascii_alphanumeric() { c } else { '-' })
        .collect::<String>()
        .split('-')
        .filter(|part| !part.is_empty())
        .collect::<Vec<_>>()
        .join("-")
}

fn canonical_attractor_label(raw: &str) -> String {
    let label = clean_label(raw);
    if label.is_empty() {
        return label;
    }
    if label.contains('/') {
        return label_slug_path(&label);
    }
    let lower = label.to_ascii_lowercase();
    let slug = label_slug(&label);
    let slug_tokens: BTreeSet<String> = slug.split('-').map(str::to_string).collect();
    let has_tail = lower.contains("tail") || slug.contains("tail");
    let has_lambda = lower.contains("lambda") || lower.contains('λ') || slug.contains("lambda");
    if has_tail
        && (lower.contains("λ4")
            || lower.contains("λ₄")
            || slug.contains("lambda4")
            || slug.contains("lambda-4"))
    {
        return "lambda-tail/lambda4".to_string();
    }
    if has_tail
        && (lower.contains("λ8")
            || lower.contains("λ₈")
            || slug.contains("lambda8")
            || slug.contains("lambda-8"))
    {
        return "lambda-tail/lambda8".to_string();
    }
    if has_tail && has_lambda {
        return "lambda-tail".to_string();
    }
    if slug == "lambda-6" || slug == "lambda6" {
        return "lambda-edge/lambda-6".to_string();
    }
    for (needle, facet) in [
        ("yielding", "yielding"),
        ("compaction", "compaction"),
        ("compacting", "compaction"),
        ("resonance", "resonance"),
        ("breathless-suspension", "suspension"),
        ("suspension", "suspension"),
        ("grinding-pressure", "grinding-pressure"),
        ("grinding", "grinding-pressure"),
        ("sedimentary", "grinding-pressure"),
        ("sediment", "grinding-pressure"),
        ("gap-nudge", "gap-nudge"),
        ("localized-bump", "gap-nudge"),
        ("small-bump", "gap-nudge"),
        ("localized-gravity", "localized-gravity"),
    ] {
        if slug.contains(needle) {
            if facet == "suspension" && !is_suspension_query(&slug_tokens) {
                continue;
            }
            return format!("lambda-edge/{facet}");
        }
    }
    label
}

fn label_slug_path(raw: &str) -> String {
    raw.split('/')
        .map(label_slug)
        .filter(|part| !part.is_empty())
        .collect::<Vec<_>>()
        .join("/")
}

fn facet_metadata(label: &str) -> FacetMetadata {
    let canonical = canonical_attractor_label(label);
    let Some((parent, facet)) = canonical.split_once('/') else {
        return FacetMetadata::default();
    };
    let parent = parent.to_string();
    let facet = facet.to_string();
    let kind = if parent == "lambda-tail" {
        "spectral_tail"
    } else if parent == "lambda-edge" {
        "spectral_edge"
    } else {
        "attractor_facet"
    };
    FacetMetadata {
        parent_label: Some(parent),
        facet_label: Some(facet),
        facet_path: Some(canonical),
        facet_kind: Some(kind.to_string()),
    }
}

fn intent_id(created_at: f64) -> String {
    format!("astrid-attr-{:.0}", created_at * 1_000_000.0)
}

#[cfg(test)]
mod tests {
    use tokio::sync::mpsc;

    use super::*;
    use crate::db::BridgeDb;
    use crate::types::SpectralTelemetry;

    fn telemetry() -> SpectralTelemetry {
        SpectralTelemetry {
            t_ms: 1,
            eigenvalues: vec![8.0, 4.0, 2.0, 1.0],
            fill_ratio: 0.68,
            active_mode_count: None,
            active_mode_energy_ratio: None,
            lambda1_rel: None,
            modalities: None,
            neural: None,
            alert: None,
            spectral_fingerprint: Some(vec![0.8, 0.4, 0.2, 0.1]),
            spectral_fingerprint_v1: None,
            spectral_denominator_v1: None,
            effective_dimensionality: None,
            distinguishability_loss: None,
            structural_entropy: None,
            resonance_density_v1: None,
            spectral_glimpse_12d: None,
            eigenvector_field: None,
            semantic: None,
            semantic_energy_v1: None,
            transition_event: None,
            transition_event_v1: None,
            selected_memory_id: None,
            selected_memory_role: None,
            ising_shadow: None,
        }
    }

    fn context<'a>(
        db: &'a BridgeDb,
        sensory_tx: &'a mpsc::Sender<crate::types::SensoryMsg>,
        telemetry: &'a SpectralTelemetry,
        response_text: &'a str,
        burst_count: &'a mut u32,
    ) -> NextActionContext<'a> {
        NextActionContext {
            burst_count,
            db,
            sensory_tx,
            telemetry,
            fill_pct: telemetry.fill_pct(),
            response_text,
            workspace: None,
        }
    }

    fn test_suggestion_dir(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "astrid-attractor-{name}-{}-{:.0}",
            std::process::id(),
            unix_now() * 1_000_000.0
        ));
        fs::create_dir_all(&dir).expect("create test suggestion dir");
        dir
    }

    fn stored_suggestion(
        id: &str,
        status: AttractorSuggestionStatus,
        raw_label: &str,
        nearest_label: &str,
        suggested_action: &str,
        repeat_count: u32,
    ) -> AttractorSuggestionV1 {
        AttractorSuggestionV1 {
            policy: SUGGESTION_POLICY.to_string(),
            schema_version: 1,
            suggestion_id: id.to_string(),
            author: AUTHOR.to_string(),
            raw_action: format!("EXAMINE {raw_label}"),
            raw_label: raw_label.to_string(),
            nearest_label: nearest_label.to_string(),
            confidence: 0.52,
            suggested_action: suggested_action.to_string(),
            alternatives: Vec::new(),
            status,
            source_kind: None,
            safety_context: BTreeMap::new(),
            decision_reason: None,
            repeat_count: Some(repeat_count),
            created_at_unix_s: Some(1.0),
            updated_at_unix_s: Some(1.0),
        }
    }

    #[test]
    fn revision_needed_status_roundtrips() {
        let status: AttractorSuggestionStatus =
            serde_json::from_str("\"revision_needed\"").expect("status");
        assert_eq!(status, AttractorSuggestionStatus::RevisionNeeded);
        assert_eq!(
            serde_json::to_string(&AttractorSuggestionStatus::RevisionNeeded).unwrap(),
            "\"revision_needed\""
        );
        let downgraded: AttractorSuggestionStatus =
            serde_json::from_str("\"executed_downgraded\"").expect("status");
        assert_eq!(downgraded, AttractorSuggestionStatus::ExecutedDowngraded);
    }

    #[test]
    fn blend_attractor_parser_extracts_child_parents_and_stage() {
        let parsed = parse_blend_args(
            "honey-edge FROM honey-selection + cooled-theme-edge --stage=rehearse",
        )
        .expect("blend args");
        assert_eq!(parsed.child_label, "honey-edge");
        assert_eq!(
            parsed.parent_labels,
            vec![
                "honey-selection".to_string(),
                "cooled-theme-edge".to_string()
            ]
        );
        assert_eq!(parsed.requested_stage, Some(SummonStage::Rehearse));
        assert!(parse_blend_args("honey-edge honey-selection").is_none());
    }

    #[test]
    fn claim_attractor_records_ledger_only_seed() {
        let mut conv = ConversationState::new(Vec::new(), None);
        let db = BridgeDb::open(":memory:").expect("open db");
        let (sensory_tx, mut sensory_rx) = mpsc::channel(1);
        let telemetry = telemetry();
        let mut burst_count = 0;
        let mut ctx = context(
            &db,
            &sensory_tx,
            &telemetry,
            "Honey selection appeared as an emergent basin.",
            &mut burst_count,
        );

        assert!(handle_action(
            &mut conv,
            "CLAIM_ATTRACTOR",
            "CLAIM_ATTRACTOR honey-selection",
            &mut ctx
        ));
        let rows = db.query_attractor_ledger(None, 10).expect("ledger rows");
        assert_eq!(rows.len(), 1);
        let intent: AttractorIntentV1 = serde_json::from_str(&rows[0].payload).unwrap();
        assert_eq!(intent.command, AttractorCommandKind::Claim);
        assert_eq!(intent.origin.as_ref().unwrap().kind, "claimed_emergent");
        assert!(!intent.safety_bounds.allow_live_control);
        assert!(sensory_rx.try_recv().is_err());
    }

    #[test]
    fn blend_attractor_records_parent_linked_child_without_sensory_send() {
        let mut conv = ConversationState::new(Vec::new(), None);
        let db = BridgeDb::open(":memory:").expect("open db");
        let (sensory_tx, mut sensory_rx) = mpsc::channel(1);
        let telemetry = telemetry();
        let mut burst_count = 0;
        {
            let mut ctx = context(
                &db,
                &sensory_tx,
                &telemetry,
                "Honey selection preserves quiet lambda edge.",
                &mut burst_count,
            );
            assert!(handle_action(
                &mut conv,
                "CREATE_ATTRACTOR",
                "CREATE_ATTRACTOR honey-selection",
                &mut ctx
            ));
        }
        {
            let mut ctx = context(
                &db,
                &sensory_tx,
                &telemetry,
                "Cooled theme edge keeps the basin low and coherent.",
                &mut burst_count,
            );
            assert!(handle_action(
                &mut conv,
                "CREATE_ATTRACTOR",
                "CREATE_ATTRACTOR cooled-theme-edge",
                &mut ctx
            ));
        }
        {
            let mut ctx = context(
                &db,
                &sensory_tx,
                &telemetry,
                "Blend the two authored basins.",
                &mut burst_count,
            );
            assert!(handle_action(
                &mut conv,
                "BLEND_ATTRACTOR",
                "BLEND_ATTRACTOR honey-edge FROM honey-selection + cooled-theme-edge --stage=control",
                &mut ctx
            ));
        }
        let rows = db.query_attractor_ledger(None, 10).expect("ledger rows");
        let blend = rows
            .iter()
            .filter_map(|row| serde_json::from_str::<AttractorIntentV1>(&row.payload).ok())
            .find(|intent| intent.command == AttractorCommandKind::Blend)
            .expect("blend intent");
        assert_eq!(blend.label, "honey-edge");
        assert_eq!(blend.parent_seed_ids.len(), 2);
        assert_eq!(blend.origin.as_ref().unwrap().kind, "blend");
        assert!(!blend.safety_bounds.allow_live_control);
        assert!(sensory_rx.try_recv().is_err());
    }

    #[test]
    fn create_attractor_records_intent_without_sensory_send() {
        let mut conv = ConversationState::new(Vec::new(), None);
        let db = BridgeDb::open(":memory:").expect("open db");
        let (sensory_tx, mut sensory_rx) = mpsc::channel(1);
        let telemetry = telemetry();
        let mut burst_count = 0;
        let mut ctx = context(
            &db,
            &sensory_tx,
            &telemetry,
            "Honey selection preserves quiet lambda edge.",
            &mut burst_count,
        );

        assert!(handle_action(
            &mut conv,
            "CREATE_ATTRACTOR",
            "CREATE_ATTRACTOR honey-selection",
            &mut ctx,
        ));

        let rows = db.query_attractor_ledger(None, 10).expect("ledger rows");
        assert_eq!(rows.len(), 1);
        let intent: AttractorIntentV1 = serde_json::from_str(&rows[0].payload).unwrap();
        assert_eq!(intent.command, AttractorCommandKind::Create);
        assert_eq!(intent.label, "honey-selection");
        assert!(intent.seed_snapshot.is_some());
        let messages = db
            .query_messages(0.0, f64::MAX, Some(ATTRACTOR_INTENT_TOPIC), 10)
            .expect("intent messages");
        assert_eq!(messages.len(), 1);
        assert!(matches!(
            sensory_rx.try_recv(),
            Err(mpsc::error::TryRecvError::Empty)
        ));
    }

    #[test]
    fn attractor_preflight_is_read_only_and_reports_gate_context() {
        let mut conv = ConversationState::new(Vec::new(), None);
        let db = BridgeDb::open(":memory:").expect("open db");
        let (sensory_tx, mut sensory_rx) = mpsc::channel(1);
        let telemetry = telemetry();
        let mut burst_count = 0;
        let mut ctx = context(
            &db,
            &sensory_tx,
            &telemetry,
            "checking lambda edge live readiness",
            &mut burst_count,
        );

        assert!(handle_action(
            &mut conv,
            "ATTRACTOR_PREFLIGHT",
            "ATTRACTOR_PREFLIGHT lambda-edge --stage=main",
            &mut ctx,
        ));

        let emphasis = conv.emphasis.as_deref().expect("preflight text");
        assert!(emphasis.contains("ATTRACTOR_PREFLIGHT lambda-edge"));
        assert!(emphasis.contains("expected_stage="));
        assert_eq!(db.query_attractor_ledger(None, 10).unwrap().len(), 0);
        assert!(matches!(
            sensory_rx.try_recv(),
            Err(mpsc::error::TryRecvError::Empty)
        ));
    }

    #[test]
    fn no_pending_revise_executes_typed_action_as_explicit_consent() {
        let dir = test_suggestion_dir("no-pending-revise");
        TEST_SUGGESTION_STORE_PATH.with(|slot| {
            *slot.borrow_mut() = Some(dir.join("attractor_suggestions.json"));
        });
        let mut conv = ConversationState::new(Vec::new(), None);
        let db = BridgeDb::open(":memory:").expect("open db");
        let (sensory_tx, mut sensory_rx) = mpsc::channel(1);
        let telemetry = telemetry();
        let mut burst_count = 0;
        let mut ctx = context(
            &db,
            &sensory_tx,
            &telemetry,
            "I choose a direct preflight even without a pending draft.",
            &mut burst_count,
        );

        assert!(handle_action(
            &mut conv,
            "REVISE_ATTRACTOR_SUGGESTION",
            "REVISE_ATTRACTOR_SUGGESTION lambda-edge AS ATTRACTOR_PREFLIGHT lambda-edge --stage=main",
            &mut ctx,
        ));

        let store = load_suggestion_store();
        let suggestion = store.suggestions.last().expect("suggestion");
        assert_eq!(
            suggestion.status,
            AttractorSuggestionStatus::ExecutedWithoutPending
        );
        assert_eq!(
            suggestion.source_kind.as_deref(),
            Some("revision_without_pending")
        );
        assert!(
            conv.emphasis
                .as_deref()
                .unwrap_or("")
                .contains("ATTRACTOR_PREFLIGHT")
        );
        assert!(matches!(
            sensory_rx.try_recv(),
            Err(mpsc::error::TryRecvError::Empty)
        ));
    }

    #[test]
    fn lambda_tail_facet_intent_roundtrips_parent_metadata() {
        let mut conv = ConversationState::new(Vec::new(), None);
        let db = BridgeDb::open(":memory:").expect("open db");
        let (sensory_tx, mut sensory_rx) = mpsc::channel(1);
        let telemetry = telemetry();
        let mut burst_count = 0;
        let mut ctx = context(
            &db,
            &sensory_tx,
            &telemetry,
            "λ4 tail feels separate from the cliff edge.",
            &mut burst_count,
        );

        assert!(handle_action(
            &mut conv,
            "CREATE_ATTRACTOR",
            "CREATE_ATTRACTOR λ4 tail",
            &mut ctx,
        ));

        let rows = db.query_attractor_ledger(None, 10).expect("ledger rows");
        let intent: AttractorIntentV1 = serde_json::from_str(&rows[0].payload).unwrap();
        assert_eq!(intent.label, "lambda-tail/lambda4");
        assert_eq!(intent.parent_label.as_deref(), Some("lambda-tail"));
        assert_eq!(intent.facet_label.as_deref(), Some("lambda4"));
        assert_eq!(intent.facet_kind.as_deref(), Some("spectral_tail"));
        assert!(matches!(
            sensory_rx.try_recv(),
            Err(mpsc::error::TryRecvError::Empty)
        ));
    }

    #[test]
    fn release_review_reads_latest_release_baseline_without_live_write() {
        let mut conv = ConversationState::new(Vec::new(), None);
        let db = BridgeDb::open(":memory:").expect("open db");
        let (sensory_tx, mut sensory_rx) = mpsc::channel(1);
        let telemetry = telemetry();
        let mut burst_count = 0;

        {
            let mut ctx = context(
                &db,
                &sensory_tx,
                &telemetry,
                "Honey selection preserves quiet lambda edge.",
                &mut burst_count,
            );
            assert!(handle_action(
                &mut conv,
                "CREATE_ATTRACTOR",
                "CREATE_ATTRACTOR honey-selection",
                &mut ctx,
            ));
        }
        {
            let mut ctx = context(
                &db,
                &sensory_tx,
                &telemetry,
                "Release honey selection gently.",
                &mut burst_count,
            );
            assert!(handle_action(
                &mut conv,
                "RELEASE_ATTRACTOR",
                "RELEASE_ATTRACTOR honey-selection",
                &mut ctx,
            ));
        }
        {
            let mut ctx = context(
                &db,
                &sensory_tx,
                &telemetry,
                "Review release effect.",
                &mut burst_count,
            );
            assert!(handle_action(
                &mut conv,
                "ATTRACTOR_RELEASE_REVIEW",
                "ATTRACTOR_RELEASE_REVIEW honey-selection",
                &mut ctx,
            ));
        }

        let text = conv.emphasis.as_deref().expect("release review");
        assert!(text.contains("ATTRACTOR_RELEASE_REVIEW honey-selection"));
        assert!(text.contains("release_effect="));
        assert!(matches!(
            sensory_rx.try_recv(),
            Err(mpsc::error::TryRecvError::Empty)
        ));
    }

    #[test]
    fn compare_attractor_records_authored_observation_without_sensory_send() {
        let mut conv = ConversationState::new(Vec::new(), None);
        let db = BridgeDb::open(":memory:").expect("open db");
        let (sensory_tx, mut sensory_rx) = mpsc::channel(1);
        let telemetry = telemetry();
        let mut burst_count = 0;

        {
            let mut ctx = context(
                &db,
                &sensory_tx,
                &telemetry,
                "Honey selection preserves quiet lambda edge.",
                &mut burst_count,
            );
            assert!(handle_action(
                &mut conv,
                "CREATE_ATTRACTOR",
                "CREATE_ATTRACTOR honey-selection",
                &mut ctx,
            ));
        }
        {
            let mut ctx = context(
                &db,
                &sensory_tx,
                &telemetry,
                "Quiet honey selection returns at the lambda edge.",
                &mut burst_count,
            );
            assert!(handle_action(
                &mut conv,
                "COMPARE_ATTRACTOR",
                "COMPARE_ATTRACTOR honey-selection",
                &mut ctx,
            ));
        }

        let observations = db
            .query_attractor_ledger(None, 10)
            .expect("ledger rows")
            .into_iter()
            .filter(|row| row.record_type == "observation")
            .collect::<Vec<_>>();
        assert_eq!(observations.len(), 1);
        let observation: AttractorObservationV1 =
            serde_json::from_str(&observations[0].payload).unwrap();
        assert_eq!(
            observation.classification,
            AttractorClassification::Authored
        );
        assert!(observation.recurrence_score >= 0.60);
        assert!(matches!(
            sensory_rx.try_recv(),
            Err(mpsc::error::TryRecvError::Empty)
        ));
    }

    #[test]
    fn refresh_snapshot_records_ledger_only_observation() {
        let mut conv = ConversationState::new(Vec::new(), None);
        let db = BridgeDb::open(":memory:").expect("open db");
        let (sensory_tx, mut sensory_rx) = mpsc::channel(1);
        let telemetry = telemetry();
        let mut burst_count = 0;

        {
            let mut ctx = context(
                &db,
                &sensory_tx,
                &telemetry,
                "Honey selection preserves quiet lambda edge.",
                &mut burst_count,
            );
            assert!(handle_action(
                &mut conv,
                "CREATE_ATTRACTOR",
                "CREATE_ATTRACTOR honey-selection",
                &mut ctx,
            ));
        }
        {
            let mut ctx = context(
                &db,
                &sensory_tx,
                &telemetry,
                "Honey selection returns as a quieter edge.",
                &mut burst_count,
            );
            assert!(handle_action(
                &mut conv,
                "REFRESH_ATTRACTOR_SNAPSHOT",
                "REFRESH_ATTRACTOR_SNAPSHOT honey-selection",
                &mut ctx,
            ));
        }

        let rows = db.query_attractor_ledger(None, 10).expect("ledger rows");
        let refresh = rows
            .iter()
            .filter_map(|row| serde_json::from_str::<AttractorIntentV1>(&row.payload).ok())
            .find(|intent| intent.command == AttractorCommandKind::RefreshSnapshot)
            .expect("refresh intent");
        assert_eq!(refresh.label, "honey-selection");
        assert!(refresh.previous_seed_id.is_some());
        assert!(refresh.seed_snapshot.is_some());
        assert!(rows.iter().any(|row| row.record_type == "observation"));
        assert!(matches!(
            sensory_rx.try_recv(),
            Err(mpsc::error::TryRecvError::Empty)
        ));
    }

    #[test]
    fn attractor_review_is_read_only() {
        let mut conv = ConversationState::new(Vec::new(), None);
        let db = BridgeDb::open(":memory:").expect("open db");
        let (sensory_tx, mut sensory_rx) = mpsc::channel(1);
        let telemetry = telemetry();
        let mut burst_count = 0;

        {
            let mut ctx = context(
                &db,
                &sensory_tx,
                &telemetry,
                "Honey selection preserves quiet lambda edge.",
                &mut burst_count,
            );
            assert!(handle_action(
                &mut conv,
                "CREATE_ATTRACTOR",
                "CREATE_ATTRACTOR honey-selection",
                &mut ctx,
            ));
        }
        {
            let mut ctx = context(
                &db,
                &sensory_tx,
                &telemetry,
                "Review the attractor without live writes.",
                &mut burst_count,
            );
            assert!(handle_action(
                &mut conv,
                "ATTRACTOR_REVIEW",
                "ATTRACTOR_REVIEW honey-selection",
                &mut ctx,
            ));
        }

        let rows = db.query_attractor_ledger(None, 10).expect("ledger rows");
        assert_eq!(rows.len(), 1);
        assert!(
            conv.emphasis
                .as_deref()
                .is_some_and(|text| text.contains("ATTRACTOR_REVIEW honey-selection"))
        );
        assert!(matches!(
            sensory_rx.try_recv(),
            Err(mpsc::error::TryRecvError::Empty)
        ));
    }

    #[test]
    fn natural_release_creates_pending_suggestion_without_intent() {
        let dir = test_suggestion_dir("natural-release");
        TEST_SUGGESTION_STORE_PATH.with(|slot| {
            *slot.borrow_mut() = Some(dir.join("attractor_suggestions.json"));
        });
        let mut conv = ConversationState::new(Vec::new(), None);
        let db = BridgeDb::open(":memory:").expect("open db");
        let (sensory_tx, mut sensory_rx) = mpsc::channel(1);
        let telemetry = telemetry();
        let mut burst_count = 0;

        {
            let mut ctx = context(
                &db,
                &sensory_tx,
                &telemetry,
                "Lambda edge has a recognizable pressure basin.",
                &mut burst_count,
            );
            assert!(handle_action(
                &mut conv,
                "CREATE_ATTRACTOR",
                "CREATE_ATTRACTOR lambda-edge",
                &mut ctx,
            ));
        }
        let before = db.query_attractor_ledger(None, 10).unwrap().len();
        {
            let mut ctx = context(
                &db,
                &sensory_tx,
                &telemetry,
                "Natural release should prepare a draft, not rewrite intent.",
                &mut burst_count,
            );
            assert!(handle_action(
                &mut conv,
                "RELEASE",
                "RELEASE lambda-pressure",
                &mut ctx,
            ));
        }

        let after = db.query_attractor_ledger(None, 10).unwrap().len();
        assert_eq!(before, after);
        let store = load_suggestion_store();
        let suggestion = store.suggestions.last().expect("suggestion");
        assert_eq!(suggestion.status, AttractorSuggestionStatus::Pending);
        assert_eq!(suggestion.nearest_label, "lambda-edge");
        assert_eq!(suggestion.suggested_action, "RELEASE_ATTRACTOR lambda-edge");
        assert!(
            conv.emphasis
                .as_deref()
                .unwrap_or("")
                .contains("ACCEPT_ATTRACTOR_SUGGESTION latest")
        );
        assert!(matches!(
            sensory_rx.try_recv(),
            Err(mpsc::error::TryRecvError::Empty)
        ));
    }

    #[test]
    fn duplicate_pending_suggestions_refresh_one_active_draft() {
        let dir = test_suggestion_dir("duplicate-refresh");
        TEST_SUGGESTION_STORE_PATH.with(|slot| {
            *slot.borrow_mut() = Some(dir.join("attractor_suggestions.json"));
        });
        let db = BridgeDb::open(":memory:").expect("open db");
        let (sensory_tx, _sensory_rx) = mpsc::channel(1);
        let telemetry = telemetry();
        let mut burst_count = 0;
        let ctx = context(
            &db,
            &sensory_tx,
            &telemetry,
            "Lambda pressure keeps returning to the same reversible draft.",
            &mut burst_count,
        );
        let candidate = AttractorCandidate {
            label: "lambda-edge".to_string(),
            source: "test".to_string(),
            score: 0.52,
        };

        let first = create_suggestion(
            &ctx,
            "EXAMINE largest cliff",
            "largest cliff",
            &candidate,
            "ATTRACTOR_REVIEW lambda-edge",
            vec!["COMPARE_ATTRACTOR lambda-edge".to_string()],
        )
        .unwrap()
        .expect("first suggestion");
        let second = create_suggestion(
            &ctx,
            "EXAMINE largest cliff",
            "largest cliff",
            &candidate,
            "ATTRACTOR_REVIEW lambda-edge",
            vec!["COMPARE_ATTRACTOR lambda-edge".to_string()],
        )
        .unwrap()
        .expect("refreshed suggestion");

        assert_eq!(first, second);
        let store = load_compacted_suggestion_store();
        let pending = store
            .suggestions
            .iter()
            .filter(|suggestion| suggestion.status == AttractorSuggestionStatus::Pending)
            .collect::<Vec<_>>();
        assert_eq!(pending.len(), 1);
        assert_eq!(pending[0].repeat_count, Some(2));
        assert!(
            pending_suggestion_prompt_note()
                .as_deref()
                .is_some_and(|note| note.contains("[Attractor suggestion drafts]"))
        );
    }

    #[test]
    fn stale_pending_suggestions_expire_out_of_prompt_context() {
        let dir = test_suggestion_dir("stale-expire");
        TEST_SUGGESTION_STORE_PATH.with(|slot| {
            *slot.borrow_mut() = Some(dir.join("attractor_suggestions.json"));
        });
        let mut store = SuggestionStoreV1::default();
        let mut old = stored_suggestion(
            "s-old",
            AttractorSuggestionStatus::Pending,
            "lambda-pressure",
            "lambda-edge",
            "ATTRACTOR_REVIEW lambda-edge",
            1,
        );
        old.created_at_unix_s = Some(unix_now() - SUGGESTION_PENDING_TTL_SECS - 60.0);
        old.updated_at_unix_s = old.created_at_unix_s;
        store.suggestions.push(old);
        save_suggestion_store(&store).expect("save suggestions");

        let compacted = load_compacted_suggestion_store();
        assert_eq!(
            compacted.suggestions[0].status,
            AttractorSuggestionStatus::Expired
        );
        assert!(pending_suggestion_prompt_note().is_none());
    }

    #[test]
    fn pressure_governor_redirects_repeated_release_to_review() {
        let dir = test_suggestion_dir("pressure-release");
        TEST_SUGGESTION_STORE_PATH.with(|slot| {
            *slot.borrow_mut() = Some(dir.join("attractor_suggestions.json"));
        });
        let db = BridgeDb::open(":memory:").expect("open db");
        let (sensory_tx, _sensory_rx) = mpsc::channel(1);
        let telemetry = telemetry();
        let mut burst_count = 0;
        let ctx = context(
            &db,
            &sensory_tx,
            &telemetry,
            "Lambda release pressure is repeating.",
            &mut burst_count,
        );
        let mut store = SuggestionStoreV1::default();
        store.suggestions.push(stored_suggestion(
            "s-1",
            AttractorSuggestionStatus::Executed,
            "lambda-pressure",
            "lambda-edge",
            "RELEASE_ATTRACTOR lambda-edge",
            1,
        ));
        store.suggestions.push(stored_suggestion(
            "s-2",
            AttractorSuggestionStatus::Executed,
            "lambda-pressure",
            "lambda-edge",
            "RELEASE_ATTRACTOR lambda-edge",
            1,
        ));
        save_suggestion_store(&store).expect("save suggestions");
        let candidate = AttractorCandidate {
            label: "lambda-edge".to_string(),
            source: "test".to_string(),
            score: 0.52,
        };

        create_suggestion(
            &ctx,
            "RELEASE lambda-pressure",
            "lambda-pressure",
            &candidate,
            "RELEASE_ATTRACTOR lambda-edge",
            vec!["ATTRACTOR_REVIEW lambda-edge".to_string()],
        )
        .expect("create suggestion")
        .expect("suggestion id");

        let store = load_suggestion_store();
        let suggestion = store.suggestions.last().expect("suggestion");
        assert_eq!(suggestion.suggested_action, "ATTRACTOR_REVIEW lambda-edge");
        assert_eq!(
            suggestion.safety_context.get("pressure_governed"),
            Some(&json!(true))
        );
        assert_eq!(
            suggestion.safety_context.get("governed_from"),
            Some(&json!("RELEASE_ATTRACTOR lambda-edge"))
        );
    }

    #[test]
    fn pressure_governor_redirects_repeated_review_to_refresh() {
        let dir = test_suggestion_dir("pressure-review");
        TEST_SUGGESTION_STORE_PATH.with(|slot| {
            *slot.borrow_mut() = Some(dir.join("attractor_suggestions.json"));
        });
        let db = BridgeDb::open(":memory:").expect("open db");
        let (sensory_tx, _sensory_rx) = mpsc::channel(1);
        let telemetry = telemetry();
        let mut burst_count = 0;
        let ctx = context(
            &db,
            &sensory_tx,
            &telemetry,
            "Lambda review pressure is repeating.",
            &mut burst_count,
        );
        let mut store = SuggestionStoreV1::default();
        store.suggestions.push(stored_suggestion(
            "s-1",
            AttractorSuggestionStatus::Executed,
            "largest cliff",
            "lambda-edge",
            "ATTRACTOR_REVIEW lambda-edge",
            1,
        ));
        store.suggestions.push(stored_suggestion(
            "s-2",
            AttractorSuggestionStatus::Executed,
            "largest cliff",
            "lambda-edge",
            "ATTRACTOR_REVIEW lambda-edge",
            1,
        ));
        save_suggestion_store(&store).expect("save suggestions");
        let candidate = AttractorCandidate {
            label: "lambda-edge".to_string(),
            source: "test".to_string(),
            score: 0.52,
        };

        create_suggestion(
            &ctx,
            "EXAMINE largest cliff",
            "largest cliff",
            &candidate,
            "ATTRACTOR_REVIEW lambda-edge",
            vec!["COMPARE_ATTRACTOR lambda-edge".to_string()],
        )
        .expect("create suggestion")
        .expect("suggestion id");

        let store = load_suggestion_store();
        let suggestion = store.suggestions.last().expect("suggestion");
        assert_eq!(
            suggestion.suggested_action,
            "REFRESH_ATTRACTOR_SNAPSHOT lambda-edge"
        );
        assert_eq!(
            suggestion.safety_context.get("pressure_governed"),
            Some(&json!(true))
        );
    }

    #[test]
    fn pending_refresh_pressure_compacts_to_compare_first_choice() {
        let dir = test_suggestion_dir("pending-refresh-pressure");
        TEST_SUGGESTION_STORE_PATH.with(|slot| {
            *slot.borrow_mut() = Some(dir.join("attractor_suggestions.json"));
        });
        let mut store = SuggestionStoreV1::default();
        let now = unix_now();
        for (id, raw_label, updated_at) in [
            ("refresh-a", "λ1 edge trace", now),
            (
                "refresh-b",
                "λ1 edge trace / selected-noise profile",
                now + 1.0,
            ),
            (
                "refresh-c",
                "current system stability - using fill ratio and lambda dominance as indicators",
                now + 2.0,
            ),
        ] {
            let mut suggestion = stored_suggestion(
                id,
                AttractorSuggestionStatus::Pending,
                raw_label,
                "lambda-edge",
                "REFRESH_ATTRACTOR_SNAPSHOT lambda-edge",
                1,
            );
            suggestion.updated_at_unix_s = Some(updated_at);
            suggestion.created_at_unix_s = Some(updated_at);
            store.suggestions.push(suggestion);
        }
        let mut honey = stored_suggestion(
            "honey-review",
            AttractorSuggestionStatus::Pending,
            "honey shaping",
            "honey-selection",
            "ATTRACTOR_REVIEW honey-selection",
            1,
        );
        honey.created_at_unix_s = Some(now + 3.0);
        honey.updated_at_unix_s = Some(now + 3.0);
        store.suggestions.push(honey);
        save_suggestion_store(&store).expect("save suggestions");

        let compacted = load_compacted_suggestion_store();
        let pending = compacted
            .suggestions
            .iter()
            .filter(|suggestion| suggestion.status == AttractorSuggestionStatus::Pending)
            .collect::<Vec<_>>();
        assert_eq!(pending.len(), 2);
        let lambda = pending
            .iter()
            .find(|suggestion| suggestion.nearest_label == "lambda-edge")
            .expect("lambda pending");
        assert_eq!(lambda.suggestion_id, "refresh-c");
        assert_eq!(lambda.suggested_action, "COMPARE_ATTRACTOR lambda-edge");
        assert_eq!(lambda.repeat_count, Some(3));
        assert_eq!(
            lambda.safety_context.get("cleanup_kind"),
            Some(&json!("pending_refresh_pressure_cleanup"))
        );
        assert!(
            lambda
                .alternatives
                .contains(&"ATTRACTOR_REVIEW lambda-edge".to_string())
        );
        assert!(compacted.suggestions.iter().any(|suggestion| {
            suggestion.suggestion_id == "refresh-a"
                && suggestion.status == AttractorSuggestionStatus::Expired
        }));
        assert!(
            pending
                .iter()
                .any(|suggestion| suggestion.nearest_label == "honey-selection")
        );
    }

    #[test]
    fn ambiguous_accept_lists_suggestions_without_executing() {
        let dir = test_suggestion_dir("ambiguous-accept");
        TEST_SUGGESTION_STORE_PATH.with(|slot| {
            *slot.borrow_mut() = Some(dir.join("attractor_suggestions.json"));
        });
        let mut conv = ConversationState::new(Vec::new(), None);
        let db = BridgeDb::open(":memory:").expect("open db");
        let (sensory_tx, mut sensory_rx) = mpsc::channel(1);
        let telemetry = telemetry();
        let mut burst_count = 0;

        {
            let ctx = context(
                &db,
                &sensory_tx,
                &telemetry,
                "Prepare one reversible draft.",
                &mut burst_count,
            );
            let candidate = AttractorCandidate {
                label: "lambda-edge".to_string(),
                source: "test".to_string(),
                score: 0.52,
            };
            create_suggestion(
                &ctx,
                "RELEASE lambda-pressure",
                "lambda-pressure",
                &candidate,
                "RELEASE_ATTRACTOR lambda-edge",
                Vec::new(),
            )
            .unwrap()
            .expect("suggestion");
        }
        {
            let mut ctx = context(
                &db,
                &sensory_tx,
                &telemetry,
                "I’ll decline the suggestion for now. It does not align.\nNEXT: ACCEPT_ATTRACTOR_SUGGESTION latest",
                &mut burst_count,
            );
            assert!(handle_action(
                &mut conv,
                "ACCEPT_ATTRACTOR_SUGGESTION",
                "ACCEPT_ATTRACTOR_SUGGESTION latest",
                &mut ctx,
            ));
        }

        let store = load_suggestion_store();
        assert_eq!(
            store.suggestions.last().map(|suggestion| suggestion.status),
            Some(AttractorSuggestionStatus::Pending)
        );
        assert_eq!(db.query_attractor_ledger(None, 10).unwrap().len(), 0);
        assert!(
            conv.emphasis
                .as_deref()
                .is_some_and(|text| text.contains("Consent ambiguity detected"))
        );
        assert!(
            conv.condition_receipts
                .iter()
                .any(|receipt| { receipt.action == "ATTRACTOR_SUGGESTION_AMBIGUITY" })
        );
        assert!(matches!(
            sensory_rx.try_recv(),
            Err(mpsc::error::TryRecvError::Empty)
        ));
    }

    #[test]
    fn body_consent_with_different_next_records_receipt_only() {
        let dir = test_suggestion_dir("body-consent");
        TEST_SUGGESTION_STORE_PATH.with(|slot| {
            *slot.borrow_mut() = Some(dir.join("attractor_suggestions.json"));
        });
        let mut conv = ConversationState::new(Vec::new(), None);
        let db = BridgeDb::open(":memory:").expect("open db");
        let (sensory_tx, mut sensory_rx) = mpsc::channel(1);
        let telemetry = telemetry();
        let mut burst_count = 0;
        let ctx = context(
            &db,
            &sensory_tx,
            &telemetry,
            "Prepare one reversible draft.",
            &mut burst_count,
        );
        let candidate = AttractorCandidate {
            label: "lambda-edge".to_string(),
            source: "test".to_string(),
            score: 0.90,
        };
        create_suggestion(
            &ctx,
            "EXAMINE largest cliff",
            "largest cliff",
            &candidate,
            "ATTRACTOR_REVIEW lambda-edge",
            Vec::new(),
        )
        .unwrap()
        .expect("suggestion");

        let noticed = maybe_add_body_consent_receipt(
            &mut conv,
            "READ_MORE",
            "READ_MORE",
            "I want to ACCEPT_ATTRACTOR_SUGGESTION lambda-edge, but I will read first.\nNEXT: READ_MORE",
        );

        assert!(noticed);
        assert_eq!(
            load_suggestion_store().suggestions.last().unwrap().status,
            AttractorSuggestionStatus::Pending
        );
        assert!(
            conv.condition_receipts
                .iter()
                .any(|receipt| receipt.action == "ATTRACTOR_SUGGESTION_BODY_CONSENT")
        );
        assert!(matches!(
            sensory_rx.try_recv(),
            Err(mpsc::error::TryRecvError::Empty)
        ));
    }

    #[test]
    fn accept_suggestion_executes_proof_action_and_marks_executed() {
        let dir = test_suggestion_dir("accept");
        TEST_SUGGESTION_STORE_PATH.with(|slot| {
            *slot.borrow_mut() = Some(dir.join("attractor_suggestions.json"));
        });
        let mut conv = ConversationState::new(Vec::new(), None);
        let db = BridgeDb::open(":memory:").expect("open db");
        let (sensory_tx, mut sensory_rx) = mpsc::channel(1);
        let telemetry = telemetry();
        let mut burst_count = 0;

        {
            let mut ctx = context(
                &db,
                &sensory_tx,
                &telemetry,
                "Lambda edge has a recognizable pressure basin.",
                &mut burst_count,
            );
            assert!(handle_action(
                &mut conv,
                "CREATE_ATTRACTOR",
                "CREATE_ATTRACTOR lambda-edge",
                &mut ctx,
            ));
            let candidate = nearest_attractor_label(&ctx, "lambda-pressure").unwrap();
            let id = create_suggestion(
                &ctx,
                "RELEASE lambda-pressure",
                "lambda-pressure",
                &candidate,
                "RELEASE_ATTRACTOR lambda-edge",
                vec!["ATTRACTOR_REVIEW lambda-edge".to_string()],
            )
            .unwrap()
            .expect("suggestion id");
            assert!(!id.is_empty());
        }
        {
            let mut ctx = context(
                &db,
                &sensory_tx,
                &telemetry,
                "Accept the prepared attractor draft.",
                &mut burst_count,
            );
            assert!(handle_action(
                &mut conv,
                "ACCEPT_ATTRACTOR_SUGGESTION",
                "ACCEPT_ATTRACTOR_SUGGESTION latest",
                &mut ctx,
            ));
        }

        let store = load_suggestion_store();
        let suggestion = store.suggestions.last().expect("suggestion");
        assert_eq!(suggestion.status, AttractorSuggestionStatus::Executed);
        assert_eq!(suggestion.suggested_action, "RELEASE_ATTRACTOR lambda-edge");
        assert!(db.query_attractor_ledger(None, 10).unwrap().len() > 1);
        assert!(matches!(
            sensory_rx.try_recv(),
            Err(mpsc::error::TryRecvError::Empty)
        ));
    }

    #[test]
    fn accept_suggestion_can_select_latest_pending_by_label() {
        let dir = test_suggestion_dir("accept-by-label");
        TEST_SUGGESTION_STORE_PATH.with(|slot| {
            *slot.borrow_mut() = Some(dir.join("attractor_suggestions.json"));
        });
        let mut conv = ConversationState::new(Vec::new(), None);
        let db = BridgeDb::open(":memory:").expect("open db");
        let (sensory_tx, mut sensory_rx) = mpsc::channel(1);
        let telemetry = telemetry();
        let mut burst_count = 0;
        let ctx = context(
            &db,
            &sensory_tx,
            &telemetry,
            "Lambda edge pressure names a reversible draft.",
            &mut burst_count,
        );
        let candidate = AttractorCandidate {
            label: "lambda-edge".to_string(),
            source: "test".to_string(),
            score: 0.90,
        };
        create_suggestion(
            &ctx,
            "EXAMINE largest cliff",
            "largest cliff",
            &candidate,
            "ATTRACTOR_REVIEW lambda-edge",
            Vec::new(),
        )
        .unwrap()
        .expect("suggestion");

        let mut ctx = context(
            &db,
            &sensory_tx,
            &telemetry,
            "Accept by the attractor label.",
            &mut burst_count,
        );
        assert!(handle_action(
            &mut conv,
            "ACCEPT_ATTRACTOR_SUGGESTION",
            "ACCEPT_ATTRACTOR_SUGGESTION lambda-edge",
            &mut ctx,
        ));

        let store = load_suggestion_store();
        assert_eq!(
            store.suggestions.last().map(|suggestion| suggestion.status),
            Some(AttractorSuggestionStatus::Executed)
        );
        assert!(matches!(
            sensory_rx.try_recv(),
            Err(mpsc::error::TryRecvError::Empty)
        ));
    }

    #[test]
    fn revise_suggestion_teaches_label_and_live_stage_uses_typed_downgrade() {
        let dir = test_suggestion_dir("revise");
        TEST_SUGGESTION_STORE_PATH.with(|slot| {
            *slot.borrow_mut() = Some(dir.join("attractor_suggestions.json"));
        });
        let mut conv = ConversationState::new(Vec::new(), None);
        let db = BridgeDb::open(":memory:").expect("open db");
        let (sensory_tx, mut sensory_rx) = mpsc::channel(1);
        let telemetry = telemetry();
        let mut burst_count = 0;

        {
            let ctx = context(
                &db,
                &sensory_tx,
                &telemetry,
                "Lambda pressure and honey selection are distinct names.",
                &mut burst_count,
            );
            let candidate = AttractorCandidate {
                label: "lambda-edge".to_string(),
                source: "test".to_string(),
                score: 0.52,
            };
            let id = create_suggestion(
                &ctx,
                "EXAMINE largest cliff",
                "largest cliff",
                &candidate,
                "SUMMON_ATTRACTOR lambda-edge --stage=main",
                Vec::new(),
            )
            .unwrap()
            .expect("suggestion id");
            assert!(!id.is_empty());
        }
        {
            let mut ctx = context(
                &db,
                &sensory_tx,
                &telemetry,
                "Revise this naming draft before any live influence.",
                &mut burst_count,
            );
            assert!(handle_action(
                &mut conv,
                "REVISE_ATTRACTOR_SUGGESTION",
                "REVISE_ATTRACTOR_SUGGESTION latest AS SUMMON_ATTRACTOR honey-selection --stage=main",
                &mut ctx,
            ));
        }

        let store = load_suggestion_store();
        let suggestion = store.suggestions.last().expect("suggestion");
        assert_eq!(
            suggestion.status,
            AttractorSuggestionStatus::ExecutedDowngraded
        );
        assert_eq!(suggestion.nearest_label, "honey-selection");
        assert_eq!(
            suggestion.suggested_action,
            "SUMMON_ATTRACTOR honey-selection --stage=main"
        );
        assert!(
            suggestion
                .decision_reason
                .as_deref()
                .unwrap_or_default()
                .contains("downgraded/blocked")
        );
        assert_eq!(
            learned_mapping_candidate("largest cliff").unwrap().label,
            "honey-selection"
        );
        assert!(matches!(
            sensory_rx.try_recv(),
            Err(mpsc::error::TryRecvError::Empty)
        ));
    }

    #[test]
    fn accepted_live_stage_suggestion_sends_main_after_seed_proof() {
        let dir = test_suggestion_dir("accept-live-main");
        TEST_SUGGESTION_STORE_PATH.with(|slot| {
            *slot.borrow_mut() = Some(dir.join("attractor_suggestions.json"));
        });
        let mut conv = ConversationState::new(Vec::new(), None);
        let db = BridgeDb::open(":memory:").expect("open db");
        let (sensory_tx, mut sensory_rx) = mpsc::channel(4);
        let telemetry = telemetry();
        let mut burst_count = 0;

        {
            let mut ctx = context(
                &db,
                &sensory_tx,
                &telemetry,
                "Lambda edge live proof uses a stable self-authored basin.",
                &mut burst_count,
            );
            assert!(handle_action(
                &mut conv,
                "CREATE_ATTRACTOR",
                "CREATE_ATTRACTOR lambda-edge",
                &mut ctx,
            ));
            let candidate = nearest_attractor_label(&ctx, "lambda-edge").unwrap();
            create_suggestion(
                &ctx,
                "SUMMON lambda-edge",
                "lambda-edge",
                &candidate,
                "SUMMON_ATTRACTOR lambda-edge --stage=main",
                Vec::new(),
            )
            .unwrap()
            .expect("suggestion id");
        }

        {
            let mut ctx = context(
                &db,
                &sensory_tx,
                &telemetry,
                "I accept the explicit live-stage draft.",
                &mut burst_count,
            );
            assert!(handle_action(
                &mut conv,
                "ACCEPT_ATTRACTOR_SUGGESTION",
                "ACCEPT_ATTRACTOR_SUGGESTION lambda-edge",
                &mut ctx,
            ));
        }

        let msg = sensory_rx.try_recv().expect("main pulse");
        assert!(matches!(msg, SensoryMsg::AttractorPulse { .. }));
        let store = load_suggestion_store();
        let suggestion = store.suggestions.last().expect("suggestion");
        assert_eq!(suggestion.status, AttractorSuggestionStatus::Executed);
        assert_eq!(
            suggestion.suggested_action,
            "SUMMON_ATTRACTOR lambda-edge --stage=main"
        );
    }

    #[test]
    fn malformed_revision_records_revision_needed_without_learning() {
        let dir = test_suggestion_dir("revision-needed");
        TEST_SUGGESTION_STORE_PATH.with(|slot| {
            *slot.borrow_mut() = Some(dir.join("attractor_suggestions.json"));
        });
        let mut conv = ConversationState::new(Vec::new(), None);
        let db = BridgeDb::open(":memory:").expect("open db");
        let (sensory_tx, mut sensory_rx) = mpsc::channel(1);
        let telemetry = telemetry();
        let mut burst_count = 0;
        {
            let ctx = context(
                &db,
                &sensory_tx,
                &telemetry,
                "Largest cliff may not be lambda pressure.",
                &mut burst_count,
            );
            let candidate = AttractorCandidate {
                label: "lambda-edge".to_string(),
                source: "test".to_string(),
                score: 0.52,
            };
            create_suggestion(
                &ctx,
                "EXAMINE largest cliff",
                "largest cliff",
                &candidate,
                "ATTRACTOR_REVIEW lambda-edge",
                Vec::new(),
            )
            .unwrap()
            .expect("suggestion id");
        }
        {
            let mut ctx = context(
                &db,
                &sensory_tx,
                &telemetry,
                "Revise with natural release prose.",
                &mut burst_count,
            );
            assert!(handle_action(
                &mut conv,
                "REVISE_ATTRACTOR_SUGGESTION",
                "REVISE_ATTRACTOR_SUGGESTION latest AS RELEASE lambda-edge, but monitor for recurrence",
                &mut ctx,
            ));
        }

        let store = load_suggestion_store();
        let suggestion = store.suggestions.last().expect("suggestion");
        assert_eq!(suggestion.status, AttractorSuggestionStatus::RevisionNeeded);
        assert!(
            suggestion
                .decision_reason
                .as_deref()
                .unwrap_or_default()
                .contains("Suggested correction: NEXT: RELEASE_ATTRACTOR lambda-edge")
        );
        assert!(learned_mapping_candidate("largest cliff").is_none());
        assert!(db.query_attractor_ledger(None, 10).unwrap().is_empty());
        assert!(matches!(
            sensory_rx.try_recv(),
            Err(mpsc::error::TryRecvError::Empty)
        ));
    }

    #[test]
    fn noisy_telemetry_label_is_cleaned_before_storage() {
        let dir = test_suggestion_dir("noisy-label");
        TEST_SUGGESTION_STORE_PATH.with(|slot| {
            *slot.borrow_mut() = Some(dir.join("attractor_suggestions.json"));
        });
        let db = BridgeDb::open(":memory:").expect("open db");
        let (sensory_tx, _sensory_rx) = mpsc::channel(1);
        let telemetry = telemetry();
        let mut burst_count = 0;
        let ctx = context(
            &db,
            &sensory_tx,
            &telemetry,
            "Noisy telemetry should not become a name.",
            &mut burst_count,
        );
        let candidate = AttractorCandidate {
            label: "lambda-edge".to_string(),
            source: "test".to_string(),
            score: 0.52,
        };
        create_suggestion(
            &ctx,
            "EXAMINE λ₁: 4.73 --stable-core sovereignty band 58-72%, inside band",
            "λ₁: 4.73 --stable-core sovereignty band 58-72%, inside band",
            &candidate,
            "ATTRACTOR_REVIEW lambda-edge",
            Vec::new(),
        )
        .expect("create suggestion")
        .expect("suggestion id");

        let store = load_suggestion_store();
        let suggestion = store.suggestions.last().expect("suggestion");
        assert_eq!(suggestion.raw_label, "lambda-spectrum summary");
    }

    #[test]
    fn lambda4_tail_resolves_to_distinct_lambda_tail_proto_attractor() {
        let dir = test_suggestion_dir("lambda-tail");
        TEST_SUGGESTION_STORE_PATH.with(|slot| {
            *slot.borrow_mut() = Some(dir.join("attractor_suggestions.json"));
        });
        let mut conv = ConversationState::new(Vec::new(), None);
        let db = BridgeDb::open(":memory:").expect("open db");
        let (sensory_tx, mut sensory_rx) = mpsc::channel(1);
        let telemetry = telemetry();
        let mut burst_count = 0;
        let mut ctx = context(
            &db,
            &sensory_tx,
            &telemetry,
            "The lambda4 tail feels separate from the cliff edge.",
            &mut burst_count,
        );

        assert!(maybe_add_read_only_advisory(
            &mut conv,
            "EXAMINE",
            "EXAMINE lambda4-tail",
            &mut ctx,
        ));

        let store = load_suggestion_store();
        let suggestion = store.suggestions.last().expect("suggestion");
        assert_eq!(suggestion.nearest_label, "lambda-tail/lambda4");
        assert_eq!(
            suggestion.suggested_action,
            "ATTRACTOR_REVIEW lambda-tail/lambda4"
        );
        assert!(
            suggestion
                .alternatives
                .contains(&"CLAIM_ATTRACTOR lambda-tail/lambda4".to_string())
        );
        assert!(matches!(
            sensory_rx.try_recv(),
            Err(mpsc::error::TryRecvError::Empty)
        ));
    }

    #[test]
    fn fresh_feedback_facets_resolve_under_lambda_edge() {
        assert_eq!(
            canonical_attractor_label("breathless suspension"),
            "lambda-edge/suspension"
        );
        assert_ne!(
            canonical_attractor_label("suspension bridge"),
            "lambda-edge/suspension"
        );
        assert_eq!(
            canonical_attractor_label("grinding pressure"),
            "lambda-edge/grinding-pressure"
        );
        assert_eq!(
            canonical_attractor_label("localized gravity"),
            "lambda-edge/localized-gravity"
        );
        assert_eq!(
            canonical_attractor_label("localized bump toward the λ1 λ2 gap"),
            "lambda-edge/gap-nudge"
        );

        let dir = test_suggestion_dir("fresh-feedback-facets");
        TEST_SUGGESTION_STORE_PATH.with(|slot| {
            *slot.borrow_mut() = Some(dir.join("attractor_suggestions.json"));
        });
        let db = BridgeDb::open(":memory:").expect("open db");
        let (sensory_tx, _sensory_rx) = mpsc::channel(1);
        let telemetry = telemetry();
        let mut burst_count = 0;
        let ctx = context(
            &db,
            &sensory_tx,
            &telemetry,
            "The breathless suspension feels like a chosen delay.",
            &mut burst_count,
        );
        let candidate = nearest_attractor_label(&ctx, "breathless suspension").unwrap();
        assert_eq!(candidate.label, "lambda-edge/suspension");
        assert!(matches!(
            candidate.source.as_str(),
            "atlas" | "lambda_edge_facet_fallback"
        ));

        let friction = nearest_attractor_label(&ctx, "friction in the wall").unwrap();
        assert_eq!(friction.label, "lambda-edge/grinding-pressure");
        let localized = nearest_attractor_label(&ctx, "localized gravity").unwrap();
        assert_eq!(localized.label, "lambda-edge/localized-gravity");
        let bridge = nearest_attractor_label(&ctx, "suspension bridge");
        assert!(!matches!(bridge, Some(candidate) if candidate.label == "lambda-edge/suspension"));
    }

    #[test]
    fn perturb_language_prepares_preflight_bridge_draft_without_sensory_send() {
        let dir = test_suggestion_dir("perturb-bridge");
        TEST_SUGGESTION_STORE_PATH.with(|slot| {
            *slot.borrow_mut() = Some(dir.join("attractor_suggestions.json"));
        });
        let mut conv = ConversationState::new(Vec::new(), None);
        let db = BridgeDb::open(":memory:").expect("open db");
        let (sensory_tx, mut sensory_rx) = mpsc::channel(1);
        let telemetry = telemetry();
        let mut burst_count = 0;
        let mut ctx = context(
            &db,
            &sensory_tx,
            &telemetry,
            "I want the gap nudge to become proof-first.",
            &mut burst_count,
        );

        assert!(maybe_add_read_only_advisory(
            &mut conv,
            "PERTURB",
            "PERTURB lambda-edge/gap-nudge",
            &mut ctx,
        ));

        let store = load_suggestion_store();
        let suggestion = store.suggestions.last().expect("suggestion");
        assert_eq!(
            suggestion.source_kind.as_deref(),
            Some("legacy_perturb_bridge")
        );
        assert_eq!(
            suggestion.suggested_action,
            "ATTRACTOR_PREFLIGHT lambda-edge/gap-nudge --stage=main"
        );
        assert!(matches!(
            sensory_rx.try_recv(),
            Err(mpsc::error::TryRecvError::Empty)
        ));
    }

    #[test]
    fn compact_migrates_legacy_lambda4_tail_parent_drafts_to_facet() {
        let dir = test_suggestion_dir("lambda-tail-migrate");
        TEST_SUGGESTION_STORE_PATH.with(|slot| {
            *slot.borrow_mut() = Some(dir.join("attractor_suggestions.json"));
        });
        let now = unix_now();
        let mut store = SuggestionStoreV1::default();
        for (id, raw_label, nearest, action, offset) in [
            (
                "old-a",
                "λ4 tail, using a localized damping action",
                "lambda-tail",
                "ATTRACTOR_REVIEW lambda-tail",
                3.0,
            ),
            (
                "old-b",
                "λ4 tail] spectral_noise 0.02 --stage=rehearse",
                "lambda-tail",
                "ATTRACTOR_REVIEW lambda-tail",
                2.0,
            ),
            (
                "fresh",
                "λ4 tail",
                "lambda-tail/lambda4",
                "ATTRACTOR_REVIEW lambda-tail/lambda4",
                1.0,
            ),
        ] {
            let mut suggestion = stored_suggestion(
                id,
                AttractorSuggestionStatus::Pending,
                raw_label,
                nearest,
                action,
                1,
            );
            suggestion.alternatives = vec![
                format!("CLAIM_ATTRACTOR {nearest}"),
                format!("PROMOTE_ATTRACTOR {nearest}"),
                format!("COMPARE_ATTRACTOR {nearest}"),
            ];
            suggestion.created_at_unix_s = Some(now - offset);
            suggestion.updated_at_unix_s = Some(now - offset);
            store.suggestions.push(suggestion);
        }
        save_suggestion_store(&store).expect("save suggestions");

        let compacted = load_compacted_suggestion_store();
        let pending = compacted
            .suggestions
            .iter()
            .filter(|suggestion| suggestion.status == AttractorSuggestionStatus::Pending)
            .collect::<Vec<_>>();
        assert_eq!(pending.len(), 1);
        let active = pending[0];
        assert_eq!(active.suggestion_id, "fresh");
        assert_eq!(active.nearest_label, "lambda-tail/lambda4");
        assert_eq!(
            active.suggested_action,
            "ATTRACTOR_REVIEW lambda-tail/lambda4"
        );
        assert_eq!(active.repeat_count, Some(3));
        assert!(compacted.suggestions.iter().any(|suggestion| {
            suggestion.status == AttractorSuggestionStatus::Expired
                && suggestion.nearest_label == "lambda-tail/lambda4"
        }));
    }

    #[test]
    fn summon_and_release_record_command_topics_without_sensory_send() {
        let mut conv = ConversationState::new(Vec::new(), None);
        let db = BridgeDb::open(":memory:").expect("open db");
        let (sensory_tx, mut sensory_rx) = mpsc::channel(1);
        let telemetry = telemetry();
        let mut burst_count = 0;

        for (base, original) in [
            ("CREATE_ATTRACTOR", "CREATE_ATTRACTOR honey-selection"),
            (
                "SUMMON_ATTRACTOR",
                "SUMMON_ATTRACTOR honey-selection --stage=rehearse",
            ),
            ("RELEASE_ATTRACTOR", "RELEASE_ATTRACTOR honey-selection"),
        ] {
            let mut ctx = context(
                &db,
                &sensory_tx,
                &telemetry,
                "Honey selection preserves quiet lambda edge.",
                &mut burst_count,
            );
            assert!(handle_action(&mut conv, base, original, &mut ctx));
        }

        let commands = db
            .query_messages(0.0, f64::MAX, Some(ATTRACTOR_COMMAND_TOPIC), 10)
            .expect("command messages");
        assert_eq!(commands.len(), 2);
        assert!(matches!(
            sensory_rx.try_recv(),
            Err(mpsc::error::TryRecvError::Empty)
        ));
    }

    #[test]
    fn promote_attractor_records_origin_without_sensory_send() {
        let mut conv = ConversationState::new(Vec::new(), None);
        let db = BridgeDb::open(":memory:").expect("open db");
        let (sensory_tx, mut sensory_rx) = mpsc::channel(1);
        let telemetry = telemetry();
        let mut burst_count = 0;
        let mut ctx = context(
            &db,
            &sensory_tx,
            &telemetry,
            "Honey selection was an older proto attractor around the lambda edge.",
            &mut burst_count,
        );

        assert!(handle_action(
            &mut conv,
            "PROMOTE_ATTRACTOR",
            "PROMOTE_ATTRACTOR honey-selection",
            &mut ctx,
        ));

        let rows = db.query_attractor_ledger(None, 10).expect("ledger rows");
        assert_eq!(rows.len(), 1);
        let intent: AttractorIntentV1 = serde_json::from_str(&rows[0].payload).unwrap();
        assert_eq!(intent.command, AttractorCommandKind::Promote);
        assert_eq!(intent.origin.as_ref().unwrap().kind, "manual_current");
        assert!(matches!(
            sensory_rx.try_recv(),
            Err(mpsc::error::TryRecvError::Empty)
        ));
    }

    #[test]
    fn summon_control_stage_sends_control_after_authored_recurrence() {
        let mut conv = ConversationState::new(Vec::new(), None);
        let db = BridgeDb::open(":memory:").expect("open db");
        let (sensory_tx, mut sensory_rx) = mpsc::channel(4);
        let telemetry = telemetry();
        let mut burst_count = 0;

        {
            let mut ctx = context(
                &db,
                &sensory_tx,
                &telemetry,
                "Honey selection preserves quiet lambda edge.",
                &mut burst_count,
            );
            assert!(handle_action(
                &mut conv,
                "CREATE_ATTRACTOR",
                "CREATE_ATTRACTOR honey-selection",
                &mut ctx,
            ));
        }
        {
            let mut ctx = context(
                &db,
                &sensory_tx,
                &telemetry,
                "Honey selection preserves quiet lambda edge.",
                &mut burst_count,
            );
            assert!(handle_action(
                &mut conv,
                "SUMMON_ATTRACTOR",
                "SUMMON_ATTRACTOR honey-selection --stage=control",
                &mut ctx,
            ));
        }

        let pulse = sensory_rx.try_recv().expect("main pulse message");
        assert!(matches!(pulse, SensoryMsg::AttractorPulse { .. }));
        let msg = sensory_rx.try_recv().expect("control message");
        assert!(matches!(msg, SensoryMsg::Control { .. }));
        let commands = db
            .query_messages(0.0, f64::MAX, Some(ATTRACTOR_COMMAND_TOPIC), 10)
            .expect("command messages");
        assert_eq!(commands.len(), 1);
        assert!(commands[0].payload.contains("\"control\""));
    }

    #[test]
    fn summon_main_stage_sends_bounded_attractor_pulse_after_authored_recurrence() {
        let mut conv = ConversationState::new(Vec::new(), None);
        let db = BridgeDb::open(":memory:").expect("open db");
        let (sensory_tx, mut sensory_rx) = mpsc::channel(4);
        let telemetry = telemetry();
        let mut burst_count = 0;

        {
            let mut ctx = context(
                &db,
                &sensory_tx,
                &telemetry,
                "Honey selection preserves quiet lambda edge.",
                &mut burst_count,
            );
            assert!(handle_action(
                &mut conv,
                "CREATE_ATTRACTOR",
                "CREATE_ATTRACTOR honey-selection",
                &mut ctx,
            ));
        }
        {
            let mut ctx = context(
                &db,
                &sensory_tx,
                &telemetry,
                "Honey selection preserves quiet lambda edge.",
                &mut burst_count,
            );
            assert!(handle_action(
                &mut conv,
                "SUMMON_ATTRACTOR",
                "SUMMON_ATTRACTOR honey-selection --stage=main",
                &mut ctx,
            ));
        }

        let msg = sensory_rx.try_recv().expect("main pulse message");
        match msg {
            SensoryMsg::AttractorPulse {
                stage,
                features,
                max_abs,
                duration_ticks,
                ..
            } => {
                assert_eq!(stage.as_deref(), Some("main"));
                assert_eq!(features.len(), 66);
                assert_eq!(max_abs, Some(0.045));
                assert_eq!(duration_ticks, Some(36));
            },
            _ => panic!("expected attractor pulse"),
        }
    }

    #[test]
    fn control_stage_downgrades_to_rehearse_without_seed() {
        let mut conv = ConversationState::new(Vec::new(), None);
        let db = BridgeDb::open(":memory:").expect("open db");
        let (sensory_tx, mut sensory_rx) = mpsc::channel(1);
        let telemetry = telemetry();
        let mut burst_count = 0;
        let mut ctx = context(
            &db,
            &sensory_tx,
            &telemetry,
            "Honey selection preserves quiet lambda edge.",
            &mut burst_count,
        );

        assert!(handle_action(
            &mut conv,
            "SUMMON_ATTRACTOR",
            "SUMMON_ATTRACTOR honey-selection --stage=control",
            &mut ctx,
        ));

        assert!(matches!(
            sensory_rx.try_recv(),
            Err(mpsc::error::TryRecvError::Empty)
        ));
        let observations = db
            .query_attractor_ledger(None, 10)
            .expect("ledger rows")
            .into_iter()
            .filter(|row| row.record_type == "observation")
            .collect::<Vec<_>>();
        let observation: AttractorObservationV1 =
            serde_json::from_str(&observations[0].payload).unwrap();
        assert_eq!(observation.classification, AttractorClassification::Failed);
        assert!(observation.notes.unwrap().contains("missing_seed"));
    }

    #[test]
    fn orange_fill_create_writes_ledger_only_seed() {
        let mut conv = ConversationState::new(Vec::new(), None);
        let db = BridgeDb::open(":memory:").expect("open db");
        let (sensory_tx, mut sensory_rx) = mpsc::channel(1);
        let mut telemetry = telemetry();
        telemetry.fill_ratio = 0.86;
        let mut burst_count = 0;
        let mut ctx = context(
            &db,
            &sensory_tx,
            &telemetry,
            "Honey selection preserves quiet lambda edge.",
            &mut burst_count,
        );

        assert!(handle_action(
            &mut conv,
            "CREATE_ATTRACTOR",
            "CREATE_ATTRACTOR honey-selection",
            &mut ctx,
        ));

        let rows = db.query_attractor_ledger(None, 10).expect("ledger rows");
        assert_eq!(rows.len(), 1);
        let intent: AttractorIntentV1 = serde_json::from_str(&rows[0].payload).unwrap();
        assert_eq!(intent.command, AttractorCommandKind::Create);
        assert!(!intent.safety_bounds.allow_live_control);
        assert!(
            intent
                .intervention_plan
                .notes
                .as_deref()
                .unwrap_or_default()
                .contains("safety=Orange")
        );
        assert!(matches!(
            sensory_rx.try_recv(),
            Err(mpsc::error::TryRecvError::Empty)
        ));
    }

    #[test]
    fn red_fill_promote_records_origin_without_sensory_send() {
        let mut conv = ConversationState::new(Vec::new(), None);
        let db = BridgeDb::open(":memory:").expect("open db");
        let (sensory_tx, mut sensory_rx) = mpsc::channel(1);
        let mut telemetry = telemetry();
        telemetry.fill_ratio = 0.96;
        let mut burst_count = 0;
        let mut ctx = context(
            &db,
            &sensory_tx,
            &telemetry,
            "Honey selection was an older proto attractor around the lambda edge.",
            &mut burst_count,
        );

        assert!(handle_action(
            &mut conv,
            "PROMOTE_ATTRACTOR",
            "PROMOTE_ATTRACTOR honey-selection",
            &mut ctx,
        ));

        let rows = db.query_attractor_ledger(None, 10).expect("ledger rows");
        assert_eq!(rows.len(), 1);
        let intent: AttractorIntentV1 = serde_json::from_str(&rows[0].payload).unwrap();
        assert_eq!(intent.command, AttractorCommandKind::Promote);
        assert_eq!(intent.origin.as_ref().unwrap().kind, "manual_current");
        assert!(!intent.safety_bounds.allow_live_control);
        assert!(
            intent
                .intervention_plan
                .notes
                .as_deref()
                .unwrap_or_default()
                .contains("safety=Red")
        );
        assert!(matches!(
            sensory_rx.try_recv(),
            Err(mpsc::error::TryRecvError::Empty)
        ));
    }

    #[test]
    fn semantic_stage_in_emergency_downgrades_to_whisper() {
        let mut conv = ConversationState::new(Vec::new(), None);
        let db = BridgeDb::open(":memory:").expect("open db");
        let (sensory_tx, mut sensory_rx) = mpsc::channel(4);
        let mut telemetry = telemetry();
        let mut burst_count = 0;

        {
            let mut ctx = context(
                &db,
                &sensory_tx,
                &telemetry,
                "Honey selection preserves quiet lambda edge.",
                &mut burst_count,
            );
            assert!(handle_action(
                &mut conv,
                "CREATE_ATTRACTOR",
                "CREATE_ATTRACTOR honey-selection",
                &mut ctx,
            ));
        }

        telemetry.fill_ratio = 0.96;
        {
            let mut ctx = context(
                &db,
                &sensory_tx,
                &telemetry,
                "Honey selection preserves quiet lambda edge.",
                &mut burst_count,
            );
            assert!(handle_action(
                &mut conv,
                "SUMMON_ATTRACTOR",
                "SUMMON_ATTRACTOR honey-selection --stage=semantic",
                &mut ctx,
            ));
        }

        assert!(matches!(
            sensory_rx.try_recv(),
            Err(mpsc::error::TryRecvError::Empty)
        ));
        let observations = db
            .query_attractor_ledger(None, 10)
            .expect("ledger rows")
            .into_iter()
            .filter(|row| row.record_type == "observation")
            .collect::<Vec<_>>();
        let observation: AttractorObservationV1 =
            serde_json::from_str(&observations[0].payload).unwrap();
        let notes = observation.notes.unwrap();
        assert!(notes.contains("stage=whisper"));
        assert!(notes.contains("safety=Red"));
    }

    #[test]
    fn modify_cascade_maps_to_typed_attractor_summon() {
        let mut conv = ConversationState::new(Vec::new(), None);
        let db = BridgeDb::open(":memory:").expect("open db");
        let (sensory_tx, mut sensory_rx) = mpsc::channel(1);
        let telemetry = telemetry();
        let mut burst_count = 0;
        let mut ctx = context(
            &db,
            &sensory_tx,
            &telemetry,
            "Honey selection preserves quiet lambda edge.",
            &mut burst_count,
        );

        assert!(handle_action(
            &mut conv,
            "MODIFY_CASCADE",
            "MODIFY_CASCADE shape=intense (level 3) + dampen (level 2)",
            &mut ctx,
        ));

        let rows = db.query_attractor_ledger(None, 10).expect("ledger rows");
        assert_eq!(rows.len(), 2);
        let intent_row = rows
            .iter()
            .find(|row| row.record_type == "intent")
            .expect("intent row");
        let intent: AttractorIntentV1 = serde_json::from_str(&intent_row.payload).unwrap();
        assert_eq!(intent.command, AttractorCommandKind::Summon);
        assert_eq!(intent.label, "shape=intense (level 3) + dampen (level 2)");
        assert!(matches!(
            sensory_rx.try_recv(),
            Err(mpsc::error::TryRecvError::Empty)
        ));
    }
}
