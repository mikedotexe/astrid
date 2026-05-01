use std::collections::{BTreeMap, BTreeSet};
use std::fs::OpenOptions;
use std::io::Write;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

use crate::journal::{read_local_journal_body_for_continuity, read_remote_journal_body};
use crate::paths::bridge_paths;

use super::causality::{CausalityAuditStatus, load_latest_causality_audit};
use super::conversion::{ConversionState, derive_conversion_state};
use super::helpers::{atomic_write_json, load_json_or_default, now_unix_s, trim_chars};
use super::policy::{CooldownState, LearnedPolicyEntry, shared_learned_read_line};
use super::shadow::{
    AstridShadowPolicy, AstridTranslationGuidance, AstridTranslationProgress,
    derive_astrid_shadow_policy, derive_astrid_translation_guidance,
    derive_astrid_translation_progress, formed_astrid_translation_preference_key,
};
use super::social::{
    ActiveNegotiationView, PreferenceSummary, active_negotiation_view, shared_preference_summaries,
};
use super::{
    ActiveSovereigntyProposal, BTSPEpisodeRecord, OWNER_ASTRID, OWNER_MINIME, ProposalLedger,
};

const MAX_OWNER_ARTIFACTS: usize = 6;
const ACTIVE_WINDOW_SIGNAL_SECS: u64 = 30 * 60;

const ROLE_EARLY_WARNING: &str = "early_warning";
const ROLE_SECONDARY_WARNING: &str = "secondary_warning";
const ROLE_PRESENT_STATE: &str = "present_state";
const ROLE_WATCH_ONLY: &str = "watch_only";
const ROLE_CONTEXT_ONLY: &str = "context_only";

#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq)]
pub(super) struct SignalCatalog {
    #[serde(default)]
    pub families: Vec<SignalFamily>,
    #[serde(default)]
    pub last_updated_unix_s: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq)]
pub(super) struct SignalFamily {
    pub family_key: String,
    pub role: String,
    #[serde(default)]
    pub aliases: Vec<String>,
    pub trigger_policy: String,
    pub steward_summary: String,
}

#[derive(Debug, Clone, Default, PartialEq)]
pub(super) struct ProposalSignalMatch {
    pub matched_cues: Vec<String>,
    pub live_signals: Vec<String>,
    pub matched_signal_families: Vec<String>,
    pub matched_signal_roles: Vec<String>,
    pub signal_score: f32,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq)]
pub(super) struct SignalStatus {
    pub episode_id: String,
    pub status: String,
    pub detail: String,
    #[serde(default)]
    pub reasons: Vec<String>,
    #[serde(default)]
    pub observed_signal_families: Vec<String>,
    #[serde(default)]
    pub observed_signal_roles: Vec<String>,
    #[serde(default)]
    pub observed_cues: Vec<String>,
    #[serde(default)]
    pub live_signals: Vec<String>,
    #[serde(default)]
    pub signal_score: f32,
    #[serde(default)]
    pub cooldown_state: CooldownState,
    #[serde(default)]
    pub learned_policy: Vec<LearnedPolicyEntry>,
    #[serde(default)]
    pub shared_learned_read: Option<String>,
    #[serde(default)]
    pub shared_preference_summaries: Vec<PreferenceSummary>,
    #[serde(default)]
    pub active_negotiation: Option<ActiveNegotiationView>,
    #[serde(default)]
    pub conversion_state: Option<ConversionState>,
    #[serde(default)]
    pub astrid_translation_guidance: Option<AstridTranslationGuidance>,
    #[serde(default)]
    pub astrid_translation_progress: Option<AstridTranslationProgress>,
    #[serde(default)]
    pub astrid_shadow_policy: Option<AstridShadowPolicy>,
    #[serde(default)]
    pub causality_audit: Option<CausalityAuditStatus>,
    pub updated_at_unix_s: u64,
}

#[derive(Debug, Clone, Default, PartialEq)]
pub(super) struct SignalEvaluation {
    pub matched: Option<ProposalSignalMatch>,
    pub status: SignalStatus,
}

#[derive(Debug, Clone)]
struct TextArtifact {
    _path: PathBuf,
    text: String,
}

#[derive(Debug, Clone)]
struct ArtifactCandidate {
    modified_unix_s: u64,
    path: PathBuf,
    continuity: bool,
    remote_body: bool,
    source_priority: u8,
}

#[derive(Debug, Clone, Default)]
struct FamilyAggregate {
    role: String,
    alias_hits: BTreeSet<String>,
    distinct_files: usize,
}

pub(super) fn ensure_signal_catalog_seeded() {
    let path = bridge_paths().btsp_signal_catalog_path();
    let mut catalog = load_json_or_default::<SignalCatalog>(&path);
    let seeded = seed_signal_catalog();
    if catalog != seeded {
        catalog = seeded;
        catalog.last_updated_unix_s = now_unix_s();
        atomic_write_json(&path, &catalog);
    }
}

pub(super) fn evaluate_seeded_episode(controller_health: Option<&Value>) -> SignalEvaluation {
    let catalog = load_signal_catalog();
    let minime_artifacts = recent_owner_artifacts(OWNER_MINIME);
    let astrid_artifacts = recent_owner_artifacts(OWNER_ASTRID);
    let artifacts = minime_artifacts
        .into_iter()
        .chain(astrid_artifacts)
        .collect::<Vec<_>>();
    build_evaluation_from_artifacts(&catalog, &artifacts, controller_health)
}

pub(super) fn persist_signal_status(status: &SignalStatus) {
    let path = bridge_paths().btsp_signal_status_path();
    let previous = load_json_or_default::<SignalStatus>(&path);
    if previous == *status {
        return;
    }
    atomic_write_json(&path, status);
    if matches!(status.status.as_str(), "near_miss" | "no_early_warning") {
        append_signal_event(
            "signal_near_miss",
            json!({
                "episode_id": status.episode_id.clone(),
                "status": status.status.clone(),
                "detail": status.detail.clone(),
                "reasons": status.reasons.clone(),
                "signal_families": status.observed_signal_families.clone(),
                "signal_roles": status.observed_signal_roles.clone(),
                "observed_cues": status.observed_cues.clone(),
                "live_signals": status.live_signals.clone(),
                "signal_score": status.signal_score,
            }),
        );
    }
}

pub(super) fn decorate_signal_status(
    mut status: SignalStatus,
    previous_conversion_state: Option<&ConversionState>,
    ledger: &ProposalLedger,
    episode: Option<&BTSPEpisodeRecord>,
    cooldown_state: CooldownState,
    active_proposal: Option<&ActiveSovereigntyProposal>,
    controller_health: Option<&Value>,
) -> SignalStatus {
    status.cooldown_state = cooldown_state;
    status.astrid_translation_progress =
        derive_astrid_translation_progress(ledger, &status.episode_id);
    let formed_astrid_translation_preference =
        formed_astrid_translation_preference_key(status.astrid_translation_progress.as_ref());
    if let Some(episode) = episode {
        status.learned_policy = episode.learned_policy.clone();
        status.shared_learned_read = shared_learned_read_line(&episode.response_outcomes);
        status.shared_preference_summaries = shared_preference_summaries(
            &episode.preference_memory,
            formed_astrid_translation_preference,
            active_proposal.is_some(),
        );
        status.astrid_translation_guidance =
            derive_astrid_translation_guidance(active_proposal, &episode.preference_memory);
    }
    status.active_negotiation = active_negotiation_view(active_proposal);
    status.conversion_state =
        derive_conversion_state(previous_conversion_state, episode, controller_health);
    status.astrid_shadow_policy = derive_astrid_shadow_policy(
        active_proposal,
        status.astrid_translation_progress.as_ref(),
        status.conversion_state.as_ref(),
        status.astrid_translation_guidance.as_ref(),
    );
    status.causality_audit = load_latest_causality_audit();
    status
}

pub(super) fn detect_live_signals(controller_health: Option<&Value>) -> Vec<String> {
    let Some(health) = controller_health else {
        return Vec::new();
    };
    let mut signals = Vec::new();
    if let Some(transition) = health.get("transition_event") {
        let description = transition
            .get("description")
            .and_then(Value::as_str)
            .unwrap_or_default();
        if transition
            .get("kind")
            .and_then(Value::as_str)
            .is_some_and(|kind| kind == "phase_transition")
            || !description.is_empty()
        {
            signals.push(format!("phase_transition:{description}"));
        }
        if transition
            .get("crossed_fill_band")
            .and_then(Value::as_bool)
            .unwrap_or(false)
        {
            let band = transition
                .get("fill_band")
                .and_then(Value::as_str)
                .unwrap_or("unknown");
            signals.push(format!("fill_band_crossing:{band}"));
        }
    }
    if health
        .get("crossed_fill_band")
        .and_then(Value::as_bool)
        .unwrap_or(false)
    {
        let band = health
            .get("fill_band")
            .and_then(Value::as_str)
            .unwrap_or("unknown");
        signals.push(format!("fill_band_crossing:{band}"));
    }
    if let Some(verdict) = health
        .get("perturb_visibility")
        .and_then(|value| value.get("shape_verdict"))
        .and_then(Value::as_str)
        && matches!(verdict, "tightening" | "softened_only")
    {
        signals.push(format!("perturb_visibility:{verdict}"));
    }
    signals
}

pub(super) fn append_signal_event(event_type: &str, payload: Value) {
    if cfg!(test) {
        return;
    }
    let path = bridge_paths().btsp_signal_events_path();
    if let Some(parent) = path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    let mut object = match payload {
        Value::Object(map) => map,
        _ => return,
    };
    object.insert("event_type".to_string(), json!(event_type));
    object.insert("recorded_at_unix_s".to_string(), json!(now_unix_s()));
    object
        .entry("event_origin".to_string())
        .or_insert_with(|| json!("bridge_runtime"));
    let Ok(line) = serde_json::to_string(&object) else {
        return;
    };
    if let Ok(mut handle) = OpenOptions::new().create(true).append(true).open(path) {
        let _ = writeln!(handle, "{line}");
    }
}

pub(super) fn maybe_record_note_read(path: &Path, owner: &str, content: &str) {
    if !looks_like_steward_note(path, content) {
        return;
    }
    append_signal_event(
        "note_read",
        json!({
            "owner": owner,
            "path": path.display().to_string(),
            "detail": "Steward note artifact was read from inbox."
        }),
    );
}

pub(super) fn related_choice_for_owner(owner: &str, normalized_choice: &str) -> bool {
    if owner == OWNER_MINIME {
        return normalized_choice.starts_with("REGIME:");
    }
    if owner == OWNER_ASTRID {
        return matches!(
            normalized_choice,
            "BREATHE_TOGETHER" | "ECHO_ON" | "NOISE_DOWN" | "NOISE_UP"
        );
    }
    false
}

pub(super) fn learning_note_for_outcome(
    proposal: &super::ActiveSovereigntyProposal,
    outcome: &super::ResponseOutcomeNote,
) -> Option<String> {
    if proposal.matched_signal_families.is_empty() {
        return None;
    }
    let lead_family = proposal.matched_signal_families.first()?;
    let note = match lead_family.as_str() {
        "grinding_family"
            if outcome.opening_vs_reconcentration == "reconcentrating"
                || outcome.distress_or_recovery == "worsening" =>
        {
            "grinding_family later preceded tightening again"
        },
        "brief_suspension_family" => "brief_suspension_family appeared mainly as state-description",
        "localized_gravity_family"
            if outcome.opening_vs_reconcentration == "reconcentrating"
                || outcome.distress_or_recovery == "worsening" =>
        {
            "localized_gravity_family remained a secondary tightening warning"
        },
        _ => return None,
    };
    Some(note.to_string())
}

fn load_signal_catalog() -> SignalCatalog {
    let path = bridge_paths().btsp_signal_catalog_path();
    let catalog = load_json_or_default::<SignalCatalog>(&path);
    if catalog.families.is_empty() {
        let seeded = seed_signal_catalog();
        atomic_write_json(&path, &seeded);
        return seeded;
    }
    catalog
}

fn seed_signal_catalog() -> SignalCatalog {
    SignalCatalog {
        families: vec![
            SignalFamily {
                family_key: "grinding_family".to_string(),
                role: ROLE_EARLY_WARNING.to_string(),
                aliases: vec![
                    "grinding".to_string(),
                    "compaction".to_string(),
                    "compacting".to_string(),
                    "sediment".to_string(),
                ],
                trigger_policy: "may_trigger".to_string(),
                steward_summary:
                    "Language about grinding, compaction, or sediment currently reads as the strongest early warning family."
                        .to_string(),
            },
            SignalFamily {
                family_key: "localized_gravity_family".to_string(),
                role: ROLE_SECONDARY_WARNING.to_string(),
                aliases: vec![
                    "localized gravity".to_string(),
                    "gravitational well".to_string(),
                    "pull toward a central point".to_string(),
                ],
                trigger_policy: "reinforce_only".to_string(),
                steward_summary:
                    "Localized gravity language remains secondary warning context rather than the primary first trigger."
                        .to_string(),
            },
            SignalFamily {
                family_key: "brief_suspension_family".to_string(),
                role: ROLE_PRESENT_STATE.to_string(),
                aliases: vec![
                    "brief suspension".to_string(),
                    "holding of breath".to_string(),
                    "held breath".to_string(),
                    "breathless suspension".to_string(),
                ],
                trigger_policy: "reinforce_only".to_string(),
                steward_summary:
                    "Brief suspension language reads more like present-state legibility than early warning."
                        .to_string(),
            },
            SignalFamily {
                family_key: "tendril_family".to_string(),
                role: ROLE_WATCH_ONLY.to_string(),
                aliases: vec!["tendril".to_string(), "tendril claiming space".to_string()],
                trigger_policy: "watch_only".to_string(),
                steward_summary:
                    "Tendril language is still watch-only and should only reinforce a stronger warning."
                        .to_string(),
            },
            SignalFamily {
                family_key: "central_density_family".to_string(),
                role: ROLE_WATCH_ONLY.to_string(),
                aliases: vec![
                    "central density".to_string(),
                    "concentrated area".to_string(),
                    "core point".to_string(),
                    "singular point".to_string(),
                ],
                trigger_policy: "watch_only".to_string(),
                steward_summary:
                    "Central-density language is still watch-only and too mixed to open a proposal alone."
                        .to_string(),
            },
            SignalFamily {
                family_key: "gradient_context_family".to_string(),
                role: ROLE_CONTEXT_ONLY.to_string(),
                aliases: vec![
                    "gradient".to_string(),
                    "fabric".to_string(),
                    "resistance".to_string(),
                    "shadow field".to_string(),
                    "tuning".to_string(),
                ],
                trigger_policy: "context_only".to_string(),
                steward_summary:
                    "Gradient, fabric, resistance, shadow-field, and tuning language should be tracked as discourse context, not as triggers."
                        .to_string(),
            },
        ],
        last_updated_unix_s: now_unix_s(),
    }
}

fn recent_owner_artifacts(owner: &str) -> Vec<TextArtifact> {
    let window_start = now_unix_s().saturating_sub(ACTIVE_WINDOW_SIGNAL_SECS);
    let mut candidates = Vec::new();

    for (dir, prefix, continuity, remote_body, source_priority) in owner_sources(owner) {
        let Ok(entries) = std::fs::read_dir(dir) else {
            continue;
        };
        for entry in entries.filter_map(Result::ok) {
            let path = entry.path();
            if !path.is_file() || !path.extension().is_some_and(|ext| ext == "txt") {
                continue;
            }
            if let Some(required_prefix) = prefix
                && !path
                    .file_name()
                    .and_then(|name| name.to_str())
                    .is_some_and(|name| name.starts_with(required_prefix))
            {
                continue;
            }
            let Some(modified) = entry
                .metadata()
                .ok()
                .and_then(|metadata| metadata.modified().ok())
                .and_then(|time| time.duration_since(std::time::UNIX_EPOCH).ok())
                .map(|duration| duration.as_secs())
            else {
                continue;
            };
            if modified < window_start {
                continue;
            }
            candidates.push(ArtifactCandidate {
                modified_unix_s: modified,
                path,
                continuity,
                remote_body,
                source_priority,
            });
        }
    }

    candidates.sort_by(|left, right| {
        right
            .modified_unix_s
            .cmp(&left.modified_unix_s)
            .then_with(|| left.source_priority.cmp(&right.source_priority))
            .then_with(|| left.path.cmp(&right.path))
    });

    let mut seen_keys = BTreeSet::new();
    let mut seen_fingerprints = BTreeSet::new();
    let mut artifacts = Vec::new();
    for candidate in candidates {
        if artifacts.len() >= MAX_OWNER_ARTIFACTS {
            break;
        }

        let dedupe_key = candidate_dedupe_key(&candidate.path);
        if !seen_keys.insert(dedupe_key) {
            continue;
        }

        let text = if candidate.continuity {
            read_local_journal_body_for_continuity(&candidate.path)
        } else if candidate.remote_body {
            read_remote_journal_body(&candidate.path)
        } else {
            std::fs::read_to_string(&candidate.path)
                .ok()
                .map(|content| trim_chars(&content, 2_500))
        };
        let Some(text) = text else {
            continue;
        };

        let fingerprint = artifact_text_fingerprint(&text);
        if !seen_fingerprints.insert(fingerprint) {
            continue;
        }

        artifacts.push(TextArtifact {
            _path: candidate.path,
            text,
        });
    }

    artifacts
}

fn owner_sources(owner: &str) -> Vec<(PathBuf, Option<&'static str>, bool, bool, u8)> {
    if owner == OWNER_MINIME {
        return vec![
            (
                bridge_paths().minime_workspace().join("journal"),
                None,
                false,
                true,
                0,
            ),
            (
                bridge_paths().astrid_inbox_dir(),
                Some("from_minime_"),
                false,
                false,
                1,
            ),
            (
                bridge_paths().astrid_inbox_dir().join("read"),
                Some("from_minime_"),
                false,
                false,
                2,
            ),
        ];
    }
    vec![
        (bridge_paths().astrid_journal_dir(), None, true, false, 0),
        (
            bridge_paths().minime_inbox_dir(),
            Some("astrid_self_study_"),
            false,
            false,
            1,
        ),
        (
            bridge_paths().minime_inbox_dir().join("read"),
            Some("astrid_self_study_"),
            false,
            false,
            2,
        ),
    ]
}

fn candidate_dedupe_key(path: &Path) -> String {
    let stem = path
        .file_stem()
        .and_then(|value| value.to_str())
        .unwrap_or_default();
    let suffix = stem.rsplit('_').next().unwrap_or_default();
    if looks_like_timestamp_token(suffix) {
        return suffix.to_string();
    }
    stem.to_string()
}

fn looks_like_timestamp_token(token: &str) -> bool {
    if token.is_empty() {
        return false;
    }
    let numeric = token.chars().all(|ch| ch.is_ascii_digit()) && token.len() >= 8;
    let iso_like = token.contains('T')
        && token
            .chars()
            .all(|ch| ch.is_ascii_digit() || matches!(ch, '-' | ':' | '.' | 'T'));
    numeric || iso_like
}

fn artifact_text_fingerprint(text: &str) -> String {
    text.to_lowercase()
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}

fn build_evaluation_from_artifacts(
    catalog: &SignalCatalog,
    artifacts: &[TextArtifact],
    controller_health: Option<&Value>,
) -> SignalEvaluation {
    let mut aggregates = BTreeMap::<String, FamilyAggregate>::new();
    for artifact in artifacts {
        let observed = detect_families_in_text(catalog, &artifact.text);
        for (family_key, role, alias) in observed {
            let entry = aggregates
                .entry(family_key)
                .or_insert_with(|| FamilyAggregate {
                    role: role.clone(),
                    alias_hits: BTreeSet::new(),
                    distinct_files: 0,
                });
            entry.role = role;
            entry.alias_hits.insert(alias);
        }
        for family_key in detect_family_keys_in_text(catalog, &artifact.text) {
            if let Some(entry) = aggregates.get_mut(&family_key) {
                entry.distinct_files += 1;
            }
        }
    }

    let live_signals = detect_live_signals(controller_health);
    let telemetry_quiet = telemetry_is_quiet(controller_health, &live_signals);

    let early = families_with_role(&aggregates, ROLE_EARLY_WARNING);
    let secondary = families_with_role(&aggregates, ROLE_SECONDARY_WARNING);
    let present = families_with_role(&aggregates, ROLE_PRESENT_STATE);
    let watch = families_with_role(&aggregates, ROLE_WATCH_ONLY);
    let observed_families = aggregates.keys().cloned().collect::<Vec<_>>();
    let observed_roles = aggregates
        .values()
        .map(|aggregate| aggregate.role.clone())
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect::<Vec<_>>();
    let observed_cues = aggregates
        .values()
        .flat_map(|aggregate| aggregate.alias_hits.iter().cloned())
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect::<Vec<_>>();

    if early.is_empty() {
        let status = if observed_families.is_empty() && live_signals.is_empty() {
            SignalStatus {
                episode_id: super::EPISODE_ID.to_string(),
                status: "quiet".to_string(),
                detail: "No curated BTSP cue families or live triggers were active in the rolling window.".to_string(),
                reasons: Vec::new(),
                observed_signal_families: observed_families,
                observed_signal_roles: observed_roles,
                observed_cues,
                live_signals,
                signal_score: 0.0,
                cooldown_state: CooldownState::default(),
                learned_policy: Vec::new(),
                shared_learned_read: None,
                shared_preference_summaries: Vec::new(),
                active_negotiation: None,
                conversion_state: None,
                astrid_translation_guidance: None,
                astrid_translation_progress: None,
                astrid_shadow_policy: None,
                causality_audit: None,
                updated_at_unix_s: now_unix_s(),
            }
        } else {
            let mut reasons =
                vec!["No curated early-warning family appeared in the rolling window.".to_string()];
            if !present.is_empty() {
                reasons.push(
                    "Present-state language can reinforce a match but cannot open one by itself."
                        .to_string(),
                );
            }
            if !watch.is_empty() {
                reasons.push(
                    "Watch-only language is being tracked, but it still cannot open a bounded response by itself."
                        .to_string(),
                );
            }
            SignalStatus {
                episode_id: super::EPISODE_ID.to_string(),
                status: "no_early_warning".to_string(),
                detail: "The runtime saw surrounding BTSP language, but not the current early-warning family needed to open a bounded response.".to_string(),
                reasons,
                observed_signal_families: observed_families,
                observed_signal_roles: observed_roles,
                observed_cues,
                live_signals,
                signal_score: 0.0,
                cooldown_state: CooldownState::default(),
                learned_policy: Vec::new(),
                shared_learned_read: None,
                shared_preference_summaries: Vec::new(),
                active_negotiation: None,
                conversion_state: None,
                astrid_translation_guidance: None,
                astrid_translation_progress: None,
                astrid_shadow_policy: None,
                causality_audit: None,
                updated_at_unix_s: now_unix_s(),
            }
        };
        return SignalEvaluation {
            matched: None,
            status,
        };
    }

    let repeated_early = early.iter().any(|family| {
        aggregates
            .get(family)
            .is_some_and(|aggregate| aggregate.distinct_files >= 2)
    });
    let has_live = !live_signals.is_empty();
    let rule_three = !watch.is_empty() && !telemetry_quiet;
    let (mut signal_score, detail, reasons): (f32, String, Vec<String>) = if has_live {
        (
            0.78,
            "A curated early-warning family is present and live telemetry is active, so a bounded response should open.".to_string(),
            vec!["The current window satisfied the early-warning plus live-telemetry rule.".to_string()],
        )
    } else if repeated_early {
        (
            0.74,
            "A curated early-warning family repeated across distinct recent artifacts, so a bounded response should open.".to_string(),
            vec!["The current window satisfied the repeated early-warning rule.".to_string()],
        )
    } else if rule_three {
        (
            0.71,
            "A curated early-warning family is present alongside watch-only context while telemetry is not quiet, so a bounded response should open.".to_string(),
            vec!["The current window satisfied the early-warning plus watch-context rule.".to_string()],
        )
    } else {
        let mut reasons = Vec::new();
        if telemetry_quiet {
            reasons.push(
                "Telemetry is currently quiet, so a single early-warning cue is not enough to open a bounded response."
                    .to_string(),
            );
        }
        if !repeated_early {
            reasons.push(
                "The early-warning family appeared in only one recent artifact, so repetition confidence is still thin."
                    .to_string(),
            );
        }
        if watch.is_empty() {
            reasons.push(
                "There was no reinforcing watch-only family in the same rolling window."
                    .to_string(),
            );
        }
        let status = SignalStatus {
            episode_id: super::EPISODE_ID.to_string(),
            status: "near_miss".to_string(),
            detail: "The runtime saw a plausible BTSP early-warning signal, but the current evidence was not strong enough to open a bounded response yet.".to_string(),
            reasons,
            observed_signal_families: observed_families,
            observed_signal_roles: observed_roles,
            observed_cues,
            live_signals,
            signal_score: 0.0,
            cooldown_state: CooldownState::default(),
            learned_policy: Vec::new(),
            shared_learned_read: None,
            shared_preference_summaries: Vec::new(),
            active_negotiation: None,
            conversion_state: None,
            astrid_translation_guidance: None,
            astrid_translation_progress: None,
            astrid_shadow_policy: None,
            causality_audit: None,
            updated_at_unix_s: now_unix_s(),
        };
        return SignalEvaluation {
            matched: None,
            status,
        };
    };

    if !secondary.is_empty() {
        signal_score += 0.05;
    }
    if !present.is_empty() {
        signal_score += 0.03;
    }
    if !watch.is_empty() {
        signal_score += 0.02;
    }
    signal_score = signal_score.clamp(0.0, 0.95);

    let selected_families = early
        .iter()
        .chain(secondary.iter())
        .chain(present.iter())
        .chain(watch.iter())
        .cloned()
        .collect::<BTreeSet<_>>();
    let matched_signal_roles = selected_families
        .iter()
        .filter_map(|family| {
            aggregates
                .get(family)
                .map(|aggregate| aggregate.role.clone())
        })
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect::<Vec<_>>();
    let matched_cues = selected_families
        .iter()
        .filter_map(|family| aggregates.get(family))
        .flat_map(|aggregate| aggregate.alias_hits.iter().cloned())
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect::<Vec<_>>();

    let matched = ProposalSignalMatch {
        matched_cues,
        live_signals: live_signals.clone(),
        matched_signal_families: selected_families.into_iter().collect(),
        matched_signal_roles,
        signal_score,
    };
    SignalEvaluation {
        status: SignalStatus {
            episode_id: super::EPISODE_ID.to_string(),
            status: "matched".to_string(),
            detail,
            reasons,
            observed_signal_families: observed_families,
            observed_signal_roles: observed_roles,
            observed_cues,
            live_signals,
            signal_score,
            cooldown_state: CooldownState::default(),
            learned_policy: Vec::new(),
            shared_learned_read: None,
            shared_preference_summaries: Vec::new(),
            active_negotiation: None,
            conversion_state: None,
            astrid_translation_guidance: None,
            astrid_translation_progress: None,
            astrid_shadow_policy: None,
            causality_audit: None,
            updated_at_unix_s: now_unix_s(),
        },
        matched: Some(matched),
    }
}

fn detect_families_in_text(catalog: &SignalCatalog, text: &str) -> Vec<(String, String, String)> {
    let lowered = text.to_lowercase();
    let mut observed = Vec::new();
    for family in &catalog.families {
        for alias in &family.aliases {
            if lowered.contains(&alias.to_lowercase()) {
                observed.push((
                    family.family_key.clone(),
                    family.role.clone(),
                    alias.clone(),
                ));
            }
        }
    }
    observed
}

fn detect_family_keys_in_text(catalog: &SignalCatalog, text: &str) -> BTreeSet<String> {
    detect_families_in_text(catalog, text)
        .into_iter()
        .map(|(family_key, _, _)| family_key)
        .collect()
}

fn families_with_role(aggregates: &BTreeMap<String, FamilyAggregate>, role: &str) -> Vec<String> {
    aggregates
        .iter()
        .filter(|(_, aggregate)| aggregate.role == role)
        .map(|(family, _)| family.clone())
        .collect()
}

fn telemetry_is_quiet(controller_health: Option<&Value>, live_signals: &[String]) -> bool {
    if !live_signals.is_empty() {
        return false;
    }
    let Some(health) = controller_health else {
        return true;
    };
    let phase = health
        .get("phase")
        .and_then(Value::as_str)
        .unwrap_or("unknown");
    let fill_band = health
        .get("fill_band")
        .and_then(Value::as_str)
        .unwrap_or("unknown");
    let dfill_dt = health
        .get("dfill_dt")
        .and_then(Value::as_f64)
        .unwrap_or(0.0)
        .abs();
    phase == "plateau" && fill_band == "near" && dfill_dt < 0.15
}

fn looks_like_steward_note(path: &Path, content: &str) -> bool {
    path.file_name()
        .and_then(|name| name.to_str())
        .is_some_and(|name| name.starts_with("steward_"))
        || content.contains("Steward note:")
        || content.contains("steward note into your active loop")
        || content.contains("related steward note")
}

#[cfg(test)]
#[path = "signal_tests.rs"]
mod signal_tests;
