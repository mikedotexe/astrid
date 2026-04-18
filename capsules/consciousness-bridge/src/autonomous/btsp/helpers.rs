use std::path::Path;

use serde::Serialize;
use serde::de::DeserializeOwned;
use serde_json::{Value, json};
use tracing::warn;

use super::signal::{append_signal_event, learning_note_for_outcome};
use super::{ActiveSovereigntyProposal, BTSPEpisodeRecord, NominatedResponse, ResponseOutcomeNote};

pub(super) fn response_matches_choice(
    response: &NominatedResponse,
    normalized_choice: &str,
) -> bool {
    if response.action.eq_ignore_ascii_case("regime") {
        let expected = response
            .parameters
            .get("regime")
            .and_then(Value::as_str)
            .unwrap_or_default()
            .to_ascii_uppercase();
        return normalized_choice == format!("REGIME:{expected}");
    }
    normalize_choice(&response.action) == normalized_choice
}

pub(super) fn normalize_choice(raw: &str) -> String {
    let trimmed = raw.trim().trim_start_matches("NEXT:").trim();
    if trimmed.contains(':') && trimmed.to_ascii_uppercase().starts_with("REGIME:") {
        return trimmed.to_ascii_uppercase();
    }
    trimmed
        .split_whitespace()
        .next()
        .unwrap_or(trimmed)
        .trim_matches(|ch: char| !ch.is_ascii_alphanumeric() && ch != '_')
        .to_ascii_uppercase()
}

pub(super) fn recompute_reply_state(proposal: &ActiveSovereigntyProposal) -> String {
    if proposal
        .owner_reply_state
        .values()
        .any(|state| state == "adopted")
    {
        return "adopted".to_string();
    }
    if proposal
        .owner_reply_state
        .values()
        .any(|state| state == "answered")
    {
        return "answered".to_string();
    }
    if proposal
        .owner_reply_state
        .values()
        .any(|state| state == "witnessed")
    {
        return "witnessed".to_string();
    }
    if proposal
        .owner_reply_state
        .values()
        .any(|state| state == "declined")
    {
        return "declined".to_string();
    }
    "unseen".to_string()
}

pub(super) fn load_json_or_default<T: DeserializeOwned + Default>(path: &Path) -> T {
    let Ok(raw) = std::fs::read_to_string(path) else {
        return T::default();
    };
    serde_json::from_str(&raw).unwrap_or_default()
}

pub(super) fn atomic_write_json<T: Serialize>(path: &Path, value: &T) {
    if let Some(parent) = path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    let Ok(json) = serde_json::to_string_pretty(value) else {
        warn!(path = %path.display(), "btsp: failed to serialize runtime json");
        return;
    };
    let tmp_path = path.with_extension("tmp");
    if std::fs::write(&tmp_path, json).is_err() {
        warn!(path = %tmp_path.display(), "btsp: failed to write temp runtime json");
        return;
    }
    if std::fs::rename(&tmp_path, path).is_err() {
        let _ = std::fs::remove_file(path);
        if std::fs::rename(&tmp_path, path).is_err() {
            warn!(path = %path.display(), "btsp: failed to replace runtime json");
        }
    }
}

pub(super) fn trim_chars(text: &str, max_chars: usize) -> String {
    if text.chars().count() <= max_chars {
        return text.to_string();
    }
    text.chars().take(max_chars).collect()
}

pub(super) fn build_non_adoption_outcome(
    proposal: &ActiveSovereigntyProposal,
    owner: &str,
    response_id: &str,
    note_prefix: &str,
    controller_health: Option<&Value>,
) -> ResponseOutcomeNote {
    let (target_nearness, distress_or_recovery, opening_vs_reconcentration, details) =
        classify_live_state(controller_health, None);
    ResponseOutcomeNote {
        proposal_id: proposal.proposal_id.clone(),
        response_id: response_id.to_string(),
        owner: owner.to_string(),
        recorded_at_unix_s: now_unix_s(),
        target_nearness,
        distress_or_recovery,
        opening_vs_reconcentration,
        note: format!("{note_prefix} {details}"),
    }
}

pub(super) fn push_unique_outcome(
    episode: &mut BTSPEpisodeRecord,
    proposal: &mut ActiveSovereigntyProposal,
    outcome: ResponseOutcomeNote,
) -> bool {
    if proposal.outcomes.iter().any(|existing| {
        existing.proposal_id == outcome.proposal_id
            && existing.owner == outcome.owner
            && existing.response_id == outcome.response_id
    }) {
        return false;
    }
    proposal.outcomes.push(outcome.clone());
    if !episode.response_outcomes.iter().any(|existing| {
        existing.proposal_id == outcome.proposal_id
            && existing.owner == outcome.owner
            && existing.response_id == outcome.response_id
    }) {
        episode.response_outcomes.push(outcome.clone());
    }
    if let Some(learning_note) = learning_note_for_outcome(proposal, &outcome)
        && !episode
            .family_learning_notes
            .iter()
            .any(|existing| existing == &learning_note)
    {
        episode.family_learning_notes.push(learning_note.clone());
        append_signal_event(
            "outcome_scored",
            json!({
                "episode_id": proposal.episode_id.clone(),
                "proposal_id": proposal.proposal_id.clone(),
                "owner": outcome.owner.clone(),
                "response_id": outcome.response_id.clone(),
                "signal_families": proposal.matched_signal_families.clone(),
                "signal_roles": proposal.matched_signal_roles.clone(),
                "learning_note": learning_note,
                "detail": outcome.note.clone()
            }),
        );
    } else {
        append_signal_event(
            "outcome_scored",
            json!({
                "episode_id": proposal.episode_id.clone(),
                "proposal_id": proposal.proposal_id.clone(),
                "owner": outcome.owner.clone(),
                "response_id": outcome.response_id.clone(),
                "signal_families": proposal.matched_signal_families.clone(),
                "signal_roles": proposal.matched_signal_roles.clone(),
                "detail": outcome.note.clone()
            }),
        );
    }
    true
}

pub(super) fn classify_live_state(
    controller_health: Option<&Value>,
    before_fill: Option<f32>,
) -> (String, String, String, String) {
    let Some(health) = controller_health else {
        return (
            "unknown".to_string(),
            "unknown".to_string(),
            "unknown".to_string(),
            "No live telemetry was available while this outcome was recorded.".to_string(),
        );
    };

    let target_fill = health
        .get("target_fill_pct")
        .and_then(Value::as_f64)
        .unwrap_or(55.0) as f32;
    let current_fill = health
        .get("fill_pct")
        .and_then(Value::as_f64)
        .unwrap_or(target_fill as f64) as f32;
    let fill_band = health
        .get("fill_band")
        .and_then(Value::as_str)
        .unwrap_or("unknown");
    let phase = health
        .get("phase")
        .and_then(Value::as_str)
        .unwrap_or("unknown");
    let shape_verdict = health
        .get("perturb_visibility")
        .and_then(|value| value.get("shape_verdict"))
        .and_then(Value::as_str)
        .unwrap_or("unknown");

    let prior_fill = before_fill.unwrap_or(current_fill);
    let prior_gap = (target_fill - prior_fill).abs();
    let current_gap = (target_fill - current_fill).abs();
    let target_nearness = if current_gap + 0.75 < prior_gap {
        "positive".to_string()
    } else if prior_gap + 0.75 < current_gap {
        "negative".to_string()
    } else {
        "mixed".to_string()
    };
    let distress_or_recovery = match fill_band {
        "near" | "over" if current_fill >= prior_fill => "recovery".to_string(),
        "under" if current_fill < prior_fill => "worsening".to_string(),
        _ => "mixed".to_string(),
    };
    let opening_vs_reconcentration = match shape_verdict {
        "tightening" => "reconcentrating".to_string(),
        "softened_only" => "mixed".to_string(),
        _ if matches!(phase, "plateau" | "expanding") => "opening".to_string(),
        _ => "mixed".to_string(),
    };
    let details = format!(
        "phase={phase}, fill_band={fill_band}, shape_verdict={shape_verdict}, fill_pct={current_fill:.1}."
    );
    (
        target_nearness,
        distress_or_recovery,
        opening_vs_reconcentration,
        details,
    )
}

pub(super) fn now_unix_s() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}
