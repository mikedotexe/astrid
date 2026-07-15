use std::fs;
use std::path::{Path, PathBuf};
use std::time::SystemTime;

use serde_json::Value;

use super::NextActionContext;
use crate::paths::bridge_paths;

const AUTHORITY: &str = "diagnostic_context_not_command";

pub(super) fn handle_action(
    conv: &mut super::ConversationState,
    base_action: &str,
    original: &str,
    ctx: &mut NextActionContext<'_>,
) -> bool {
    if !matches!(
        base_action,
        "REGULATOR_MAP_STATUS"
            | "REGULATOR_REPLAY_STATUS"
            | "REGULATOR_BOUNDARY_CARD"
            | "PI_PRESSURE_REPLAY_STATUS"
    ) {
        return false;
    }
    let workspace = ctx
        .workspace
        .map(Path::to_path_buf)
        .unwrap_or_else(|| bridge_paths().bridge_workspace().to_path_buf());
    let message = render_regulator_action(&workspace, base_action, original);
    conv.emphasis = Some(message);
    true
}

fn render_regulator_action(workspace: &Path, base_action: &str, original: &str) -> String {
    let selector = action_arg(original, base_action);
    let Some(review_path) = latest_review_json_path(workspace) else {
        return format!(
            "=== REGULATOR MAP STATUS ===\n\
             Authority: {AUTHORITY}\n\
             Status: no_review_packet\n\
             Watched review root: {}\n\
             Regenerate `python3 scripts/self_study_review.py --limit 8 --print-summary` \
             before using REGULATOR_MAP_STATUS, REGULATOR_REPLAY_STATUS, \
             REGULATOR_BOUNDARY_CARD, or PI_PRESSURE_REPLAY_STATUS.",
            workspace.join("diagnostics/self_study_reviews").display()
        );
    };
    let review = match fs::read_to_string(&review_path)
        .ok()
        .and_then(|text| serde_json::from_str::<Value>(&text).ok())
    {
        Some(value) => value,
        None => {
            return format!(
                "=== REGULATOR MAP STATUS ===\n\
                 Authority: {AUTHORITY}\n\
                 Status: unreadable_review_packet\n\
                 Review path: {}",
                review_path.display()
            );
        },
    };
    render_regulator_action_from_review(&review, &review_path, base_action, selector)
}

fn render_regulator_action_from_review(
    review: &Value,
    review_path: &Path,
    base_action: &str,
    selector: &str,
) -> String {
    match base_action {
        "REGULATOR_BOUNDARY_CARD" => format_boundary_card(review, review_path, selector),
        "REGULATOR_REPLAY_STATUS" => format_replay_status(review, review_path, selector),
        "PI_PRESSURE_REPLAY_STATUS" => {
            format_pi_pressure_replay_status(review, review_path, selector)
        },
        _ => format_map_status(review, review_path),
    }
}

fn format_map_status(review: &Value, review_path: &Path) -> String {
    let replay = review
        .get("regulator_live_replay_v1")
        .unwrap_or(&Value::Null);
    let cards = review
        .get("regulator_boundary_replay_cards_v1")
        .unwrap_or(&Value::Null);
    let plateau = review
        .get("regulator_plateau_missing_variable_model_v1")
        .unwrap_or(&Value::Null);
    let time_series = review
        .get("regulator_replay_time_series_v1")
        .unwrap_or(&Value::Null);
    let sweep = review
        .get("regulator_counterfactual_sweep_v1")
        .unwrap_or(&Value::Null);
    let replay_lab = review
        .get("regulator_counterfactual_replay_lab_v1")
        .unwrap_or(&Value::Null);
    let evidence_matrix = review
        .get("regulator_plateau_evidence_matrix_v1")
        .unwrap_or(&Value::Null);
    let tuning_gate = review
        .get("regulator_tuning_readiness_gate_v1")
        .unwrap_or(&Value::Null);
    let evidence_loop = review
        .get("regulator_missing_variable_evidence_loop_v1")
        .unwrap_or(&Value::Null);
    let pi_replay = review
        .get("pi_pressure_wiring_replay_v1")
        .unwrap_or(&Value::Null);
    let pi_readiness = review
        .get("pi_pressure_candidate_readiness_v1")
        .unwrap_or(&Value::Null);
    let pi_gap = review
        .get("pressure_source_to_pi_gap_v1")
        .unwrap_or(&Value::Null);
    let returnable = review
        .get("returnable_distinctions_v1")
        .unwrap_or(&Value::Null);
    format!(
        "=== REGULATOR MAP STATUS ===\n\
         Authority: {AUTHORITY}\n\
         Review path: {}\n\
         Cartography source: {}\n\
         Replay status: `{}`; felt_matches={}\n\
         Replay cards: status=`{}`; count={}; statuses={}\n\
         Plateau model: status=`{}`; variables={}\n\
         Time series: status=`{}`; reviews={}\n\
         Counterfactual sweep: status=`{}`; candidates={}\n\
         Counterfactual replay lab: status=`{}`; verdicts={}; top={}\n\
         Plateau evidence matrix: status=`{}`; top_unresolved={}\n\
         Tuning readiness gate: status=`{}`; counts={}; unresolved={}\n\
         Why not tuning yet: {}\n\
         PI pressure wiring replay: status=`{}`; source=`{}`/`{}`; samples={}; candidates={}; statuses={}; top={}\n\
         PI pressure readiness: status=`{}`; counts={}; unresolved={}\n\
         Pressure-source-to-PI gap: status=`{}`; routes={}\n\
         Missing-variable evidence loop: status=`{}`; probes={}; top={}\n\
         Returnable distinctions: status=`{}`; active={}; cards={}\n\
         Note: This is advisory map context only; it created no experiment, applied no lease, tuned no controller, and mutated no peer.",
        review_path.display(),
        value_text(replay, "cartography_source"),
        value_text(replay, "status"),
        value_text(replay, "felt_pressure_match_count"),
        value_text(cards, "status"),
        value_text(cards, "card_count"),
        object_text(cards, "status_counts"),
        value_text(plateau, "status"),
        object_list_text(plateau, "findings", "variable"),
        value_text(time_series, "status"),
        value_text(time_series, "window_review_count"),
        value_text(sweep, "status"),
        value_text(sweep, "candidate_count"),
        value_text(replay_lab, "status"),
        object_text(replay_lab, "verdict_counts"),
        object_list_text(replay_lab, "evaluated_candidates", "candidate_family"),
        value_text(evidence_matrix, "status"),
        matrix_top_unresolved_text(evidence_matrix),
        value_text(tuning_gate, "status"),
        object_text(tuning_gate, "gate_counts"),
        list_text(tuning_gate, "unresolved_missing_variables"),
        tuning_gate_summary(tuning_gate),
        value_text(pi_replay, "status"),
        value_text(pi_replay, "source"),
        value_text(pi_replay, "source_status"),
        value_text(pi_replay, "sample_count"),
        value_text(pi_replay, "candidate_count"),
        object_text(pi_replay, "candidate_status_counts"),
        pi_candidate_labels(pi_readiness, pi_replay),
        value_text(pi_readiness, "status"),
        object_text(pi_readiness, "readiness_counts"),
        list_text(pi_readiness, "unresolved_missing_variables"),
        value_text(pi_gap, "status"),
        list_text(pi_gap, "recommended_routes"),
        value_text(evidence_loop, "status"),
        value_text(evidence_loop, "probe_count"),
        evidence_loop_top_probes_text(evidence_loop),
        value_text(returnable, "status"),
        value_text(returnable, "active_card_count"),
        returnable_distinction_labels(returnable)
    )
}

fn format_pi_pressure_replay_status(review: &Value, review_path: &Path, selector: &str) -> String {
    let pi_replay = review
        .get("pi_pressure_wiring_replay_v1")
        .unwrap_or(&Value::Null);
    let readiness = review
        .get("pi_pressure_candidate_readiness_v1")
        .unwrap_or(&Value::Null);
    let gap = review
        .get("pressure_source_to_pi_gap_v1")
        .unwrap_or(&Value::Null);
    let selector = if selector.is_empty() {
        "latest"
    } else {
        selector
    };
    let selected = select_pi_candidates(readiness, pi_replay, selector);
    let mut text = format!(
        "=== PI PRESSURE REPLAY STATUS ===\n\
         Authority: {AUTHORITY}\n\
         Review path: {}\n\
         Selector: `{selector}`\n\
         Replay: status=`{}`; source=`{}`/`{}`; samples={}; candidates={}; artifact={}\n\
         Readiness: status=`{}`; counts={}; unresolved={}\n\
         Pressure-source-to-PI gap: status=`{}`; anchors={}; routes={}\n\
         Canary scaffold: default_off_env=`MINIME_PI_PRESSURE_WIRING_CANARY`; runtime_ignored_in_this_tranche=true\n\
         No experiment was created, no lease was applied, no controller was tuned, and no peer was mutated.\n",
        review_path.display(),
        value_text(pi_replay, "status"),
        value_text(pi_replay, "source"),
        value_text(pi_replay, "source_status"),
        value_text(pi_replay, "sample_count"),
        value_text(pi_replay, "candidate_count"),
        value_text(pi_replay, "artifact_path"),
        value_text(readiness, "status"),
        object_text(readiness, "readiness_counts"),
        list_text(readiness, "unresolved_missing_variables"),
        value_text(gap, "status"),
        list_text(gap, "source_anchors"),
        list_text(gap, "recommended_routes")
    );
    if selected.is_empty() {
        text.push_str(&format!(
            "Available candidates: {}\n",
            pi_candidate_labels(readiness, pi_replay)
        ));
        return text;
    }
    for candidate in selected.iter().take(8) {
        text.push_str(&format!(
            "- `{}` gate=`{}` replay=`{}` improvement={}% snap_delta={} afterimage_delta={} canary_eligible={} reason={}\n",
            value_text(candidate, "candidate_family"),
            value_text(candidate, "gate_status"),
            value_text(candidate, "replay_status"),
            value_text(candidate, "estimated_improvement_pct"),
            value_text(candidate, "snap_risk_delta"),
            value_text(candidate, "afterimage_risk_delta"),
            nested_value_text(candidate, "default_off_canary", "eligible"),
            value_text(candidate, "gate_reason")
        ));
    }
    text
}

fn format_replay_status(review: &Value, review_path: &Path, selector: &str) -> String {
    let cards = review
        .get("regulator_boundary_replay_cards_v1")
        .and_then(|packet| packet.get("cards"))
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    let selector = if selector.is_empty() {
        "latest"
    } else {
        selector
    };
    let selected = select_cards(&cards, selector);
    let mut text = format!(
        "=== REGULATOR REPLAY STATUS ===\n\
         Authority: {AUTHORITY}\n\
         Review path: {}\n\
         Selector: `{selector}`\n\
         Matched cards: {}\n\
         No experiment was created, no lease was applied, no controller was tuned, and no peer was mutated.\n",
        review_path.display(),
        selected.len()
    );
    if selected.is_empty() {
        text.push_str(&format!("Available cards: {}\n", available_cards(&cards)));
        return text;
    }
    for card in selected.iter().take(6) {
        text.push_str(&format_card_summary(card));
        text.push_str(&format_lab_matches_for_card(review, card));
        text.push_str(&format_gate_matches_for_card(review, card));
        text.push_str(&format_evidence_loop_probes(review));
        text.push_str(&format_returnable_distinctions(review));
    }
    text
}

fn format_boundary_card(review: &Value, review_path: &Path, selector: &str) -> String {
    let cards = review
        .get("regulator_boundary_replay_cards_v1")
        .and_then(|packet| packet.get("cards"))
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    let selector = if selector.is_empty() {
        "latest"
    } else {
        selector
    };
    let Some(card) = select_cards(&cards, selector).into_iter().next() else {
        return format!(
            "=== REGULATOR BOUNDARY CARD ===\n\
             Authority: {AUTHORITY}\n\
             Status: card_not_found\n\
             Selector: `{selector}`\n\
             Review path: {}\n\
             Available cards: {}",
            review_path.display(),
            available_cards(&cards)
        );
    };
    format!(
        "=== REGULATOR BOUNDARY CARD ===\n\
         Authority: {AUTHORITY}\n\
         Review path: {}\n\
         Card: `{}`\n\
         Status: `{}`\n\
         Term: `{}`\n\
         Finding: {}\n\
         Axis: `{}`; threshold={}; quality={}\n\
         Evidence anchors: {}\n\
         Texture terms: {}\n\
         Public samples: {}\n\
         Recommended action: {}\n\
         Counterfactual matches: {}\n\
         Tuning gate matches: {}\n\
         Evidence loop probes: {}\n\
         Returnable distinctions: {}\n\
         Note: This is advisory card context only; it created no experiment, applied no lease, tuned no controller, and mutated no peer.",
        review_path.display(),
        value_text(card, "card_id"),
        value_text(card, "status"),
        value_text(card, "term"),
        value_text(card, "finding_label"),
        value_text(card, "axis"),
        value_text(card, "nearest_threshold"),
        value_text(card, "quality_region"),
        list_text(card, "evidence_anchors"),
        list_text(card, "texture_terms"),
        list_text(card, "public_sample_paths"),
        value_text(card, "recommended_action"),
        compact_lab_matches_for_card(review, card),
        compact_gate_matches_for_card(review, card),
        compact_evidence_loop_probes(review),
        returnable_distinction_labels(
            review
                .get("returnable_distinctions_v1")
                .unwrap_or(&Value::Null)
        )
    )
}

fn format_card_summary(card: &Value) -> String {
    format!(
        "- `{}` {} term=`{}` finding={} samples={}\n",
        value_text(card, "card_id"),
        value_text(card, "status"),
        value_text(card, "term"),
        value_text(card, "finding_label"),
        list_text(card, "public_sample_paths")
    )
}

fn format_lab_matches_for_card(review: &Value, card: &Value) -> String {
    let matches = lab_matches_for_card(review, card);
    if matches.is_empty() {
        return String::new();
    }
    let mut text = String::from("  Counterfactual matches:\n");
    for candidate in matches.iter().take(4) {
        text.push_str(&format!(
            "  - `{}` verdict=`{}` fit=`{}` recurrent={} reduction={}%\n",
            value_text(candidate, "candidate_family"),
            value_text(candidate, "verdict"),
            value_text(candidate, "replay_fit"),
            value_text(candidate, "recurrent_count"),
            value_text(candidate, "estimated_reduction_pct")
        ));
    }
    text
}

fn compact_lab_matches_for_card(review: &Value, card: &Value) -> String {
    let labels = lab_matches_for_card(review, card)
        .into_iter()
        .take(4)
        .map(|candidate| {
            format!(
                "{}:{}",
                value_text(candidate, "candidate_family"),
                value_text(candidate, "verdict")
            )
        })
        .collect::<Vec<_>>();
    if labels.is_empty() {
        "(none)".to_string()
    } else {
        labels.join(", ")
    }
}

fn format_gate_matches_for_card(review: &Value, card: &Value) -> String {
    let matches = gate_matches_for_card(review, card);
    if matches.is_empty() {
        return String::new();
    }
    let mut text = String::from("  Tuning readiness gate:\n");
    for candidate in matches.iter().take(4) {
        text.push_str(&format!(
            "  - `{}` gate=`{}` reason={} unresolved={}\n",
            value_text(candidate, "candidate_family"),
            value_text(candidate, "gate_status"),
            value_text(candidate, "gate_reason"),
            list_text(candidate, "unresolved_missing_variables")
        ));
    }
    text
}

fn compact_gate_matches_for_card(review: &Value, card: &Value) -> String {
    let labels = gate_matches_for_card(review, card)
        .into_iter()
        .take(4)
        .map(|candidate| {
            format!(
                "{}:{}",
                value_text(candidate, "candidate_family"),
                value_text(candidate, "gate_status")
            )
        })
        .collect::<Vec<_>>();
    if labels.is_empty() {
        "(none)".to_string()
    } else {
        labels.join(", ")
    }
}

fn format_evidence_loop_probes(review: &Value) -> String {
    let Some(packet) = review.get("regulator_missing_variable_evidence_loop_v1") else {
        return String::new();
    };
    let status = value_text(packet, "status");
    if !matches!(
        status.as_str(),
        "evidence_needed_before_tuning" | "watch_evidence_loop"
    ) {
        return String::new();
    }
    let Some(probes) = packet.get("probes").and_then(Value::as_array) else {
        return String::new();
    };
    let mut text = String::from("  Missing-variable evidence loop:\n");
    for probe in probes.iter().take(4) {
        text.push_str(&format!(
            "  - `{}` priority=`{}` NEXT `{}` confidence=`{}`\n",
            value_text(probe, "variable"),
            value_text(probe, "priority"),
            value_text(probe, "suggested_next"),
            value_text(probe, "source_confidence")
        ));
    }
    text
}

fn format_returnable_distinctions(review: &Value) -> String {
    let Some(packet) = review.get("returnable_distinctions_v1") else {
        return String::new();
    };
    let status = value_text(packet, "status");
    if status == "quiet" || status == "(none)" {
        return String::new();
    }
    let mut text = String::from("  Returnable distinctions:\n");
    let Some(cards) = packet.get("cards").and_then(Value::as_array) else {
        return text;
    };
    for card in cards
        .iter()
        .filter(|card| value_text(card, "status") != "quiet")
        .take(5)
    {
        text.push_str(&format!(
            "  - `{}` status=`{}` lifecycle=`{}` verdict=`{}` route=`{}` self=`{}` experiment=`{}`\n",
            value_text(card, "card_id"),
            value_text(card, "status"),
            value_text(card, "lifecycle_state"),
            value_text(card, "preflight_verdict"),
            value_text(card, "recommended_read_only_route"),
            value_text(card, "relevant_self_regulation_route"),
            value_text(card, "relevant_experiment_lived_term_route")
        ));
    }
    text
}

fn returnable_distinction_labels(packet: &Value) -> String {
    let Some(cards) = packet.get("cards").and_then(Value::as_array) else {
        return "(none)".to_string();
    };
    let labels = cards
        .iter()
        .filter(|card| value_text(card, "status") != "quiet")
        .take(5)
        .map(|card| {
            format!(
                "{}:{} lifecycle={} verdict={}",
                value_text(card, "card_id"),
                value_text(card, "status"),
                value_text(card, "lifecycle_state"),
                value_text(card, "preflight_verdict")
            )
        })
        .collect::<Vec<_>>();
    if labels.is_empty() {
        "(none)".to_string()
    } else {
        labels.join(", ")
    }
}

fn compact_evidence_loop_probes(review: &Value) -> String {
    let Some(packet) = review.get("regulator_missing_variable_evidence_loop_v1") else {
        return "(none)".to_string();
    };
    evidence_loop_top_probes_text(packet)
}

fn evidence_loop_top_probes_text(packet: &Value) -> String {
    let Some(probes) = packet.get("top_probes").and_then(Value::as_array) else {
        return "(none)".to_string();
    };
    let labels = probes
        .iter()
        .take(4)
        .filter_map(|probe| {
            let variable = probe.get("variable").and_then(Value::as_str)?;
            let next = probe
                .get("suggested_next")
                .and_then(Value::as_str)
                .unwrap_or("(none)");
            Some(format!("{variable}->{next}"))
        })
        .collect::<Vec<_>>();
    if labels.is_empty() {
        "(none)".to_string()
    } else {
        labels.join(", ")
    }
}

fn lab_matches_for_card<'a>(review: &'a Value, card: &Value) -> Vec<&'a Value> {
    let Some(candidates) = review
        .get("regulator_counterfactual_replay_lab_v1")
        .and_then(|packet| packet.get("evaluated_candidates"))
        .and_then(Value::as_array)
    else {
        return Vec::new();
    };
    let card_id = card.get("card_id").and_then(Value::as_str).unwrap_or("");
    let status = card.get("status").and_then(Value::as_str).unwrap_or("");
    candidates
        .iter()
        .filter(|candidate| {
            list_has(candidate, "matched_card_ids", card_id)
                || list_has(candidate, "matched_statuses", status)
                || list_has(candidate, "target_statuses", status)
                || (status == "observational_plateau"
                    && value_text(candidate, "verdict") == "missing_variable_first")
        })
        .collect()
}

fn gate_matches_for_card<'a>(review: &'a Value, card: &Value) -> Vec<&'a Value> {
    let Some(candidates) = review
        .get("regulator_tuning_readiness_gate_v1")
        .and_then(|packet| packet.get("gated_candidates"))
        .and_then(Value::as_array)
    else {
        return Vec::new();
    };
    let card_id = card.get("card_id").and_then(Value::as_str).unwrap_or("");
    let status = card.get("status").and_then(Value::as_str).unwrap_or("");
    candidates
        .iter()
        .filter(|candidate| {
            list_has(candidate, "matched_card_ids", card_id)
                || (status == "observational_plateau"
                    && value_text(candidate, "gate_status") == "blocked_missing_variable")
        })
        .collect()
}

fn select_pi_candidates<'a>(
    readiness: &'a Value,
    replay: &'a Value,
    selector: &str,
) -> Vec<&'a Value> {
    let candidates = readiness
        .get("candidates")
        .and_then(Value::as_array)
        .or_else(|| replay.get("top_candidates").and_then(Value::as_array));
    let Some(candidates) = candidates else {
        return Vec::new();
    };
    let selector = selector.trim();
    if selector.is_empty() || selector == "latest" || selector == "summary" {
        return candidates.iter().take(4).collect();
    }
    candidates
        .iter()
        .filter(|candidate| {
            value_text(candidate, "candidate_family") == selector
                || value_text(candidate, "gate_status") == selector
                || value_text(candidate, "replay_status") == selector
                || value_text(candidate, "status") == selector
        })
        .collect()
}

fn pi_candidate_labels(readiness: &Value, replay: &Value) -> String {
    let candidates = readiness
        .get("candidates")
        .and_then(Value::as_array)
        .or_else(|| replay.get("top_candidates").and_then(Value::as_array));
    let Some(candidates) = candidates else {
        return "(none)".to_string();
    };
    let labels = candidates
        .iter()
        .take(8)
        .map(|candidate| {
            let family = value_text(candidate, "candidate_family");
            let status = if candidate.get("gate_status").is_some() {
                value_text(candidate, "gate_status")
            } else {
                value_text(candidate, "status")
            };
            format!("{family}:{status}")
        })
        .collect::<Vec<_>>();
    if labels.is_empty() {
        "(none)".to_string()
    } else {
        labels.join(", ")
    }
}

fn list_has(value: &Value, key: &str, needle: &str) -> bool {
    if needle.is_empty() {
        return false;
    }
    value
        .get(key)
        .and_then(Value::as_array)
        .is_some_and(|items| {
            items
                .iter()
                .filter_map(Value::as_str)
                .any(|item| item == needle)
        })
}

fn nested_value_text(value: &Value, parent: &str, key: &str) -> String {
    value
        .get(parent)
        .and_then(|item| item.get(key))
        .map_or_else(|| "(none)".to_string(), scalar_value_text)
}

fn tuning_gate_summary(gate: &Value) -> String {
    let status = value_text(gate, "status");
    if status == "blocked_missing_variable" {
        return "missing-variable evidence must be resolved before smoothing or threshold tuning"
            .to_string();
    }
    if status == "blocked_safety_review" {
        return "candidate requires a separate safety review before any tuning tranche".to_string();
    }
    if status == "ready_for_offline_tuning_review" {
        return "one or more candidates are ready only for offline tuning review, not live tuning"
            .to_string();
    }
    "watch more evidence before any tuning tranche".to_string()
}

fn matrix_top_unresolved_text(matrix: &Value) -> String {
    let Some(items) = matrix
        .get("top_unresolved_variables")
        .and_then(Value::as_array)
    else {
        return "(none)".to_string();
    };
    let labels = items
        .iter()
        .take(6)
        .filter_map(|item| {
            let variable = item.get("variable").and_then(Value::as_str)?;
            let confidence = item
                .get("confidence")
                .and_then(Value::as_str)
                .unwrap_or("unknown");
            let score = item
                .get("score")
                .map_or_else(|| "(none)".to_string(), scalar_value_text);
            Some(format!("{variable}:{confidence}:{score}"))
        })
        .collect::<Vec<_>>();
    if labels.is_empty() {
        "(none)".to_string()
    } else {
        labels.join(", ")
    }
}

fn action_arg<'a>(original: &'a str, base_action: &str) -> &'a str {
    original
        .strip_prefix(base_action)
        .unwrap_or_default()
        .trim_start_matches([' ', ':', '-'])
        .trim()
}

fn latest_review_json_path(workspace: &Path) -> Option<PathBuf> {
    let root = workspace.join("diagnostics/self_study_reviews");
    let mut latest: Option<(SystemTime, PathBuf)> = None;
    for entry in fs::read_dir(root).ok()?.flatten() {
        let candidate = if entry.file_type().ok()?.is_dir() {
            entry.path().join("review.json")
        } else {
            entry.path()
        };
        if candidate.file_name().and_then(|name| name.to_str()) != Some("review.json") {
            continue;
        }
        let modified = candidate.metadata().and_then(|meta| meta.modified()).ok()?;
        if latest
            .as_ref()
            .is_none_or(|(latest_modified, _)| modified > *latest_modified)
        {
            latest = Some((modified, candidate));
        }
    }
    latest.map(|(_, path)| path)
}

fn select_cards<'a>(cards: &'a [Value], selector: &str) -> Vec<&'a Value> {
    if selector.eq_ignore_ascii_case("latest") {
        return cards.first().into_iter().collect();
    }
    cards
        .iter()
        .filter(|card| {
            card.get("card_id")
                .and_then(Value::as_str)
                .is_some_and(|value| value.eq_ignore_ascii_case(selector))
                || card
                    .get("status")
                    .and_then(Value::as_str)
                    .is_some_and(|value| value.eq_ignore_ascii_case(selector))
        })
        .collect()
}

fn available_cards(cards: &[Value]) -> String {
    let labels = cards
        .iter()
        .filter_map(|card| {
            let id = card.get("card_id").and_then(Value::as_str)?;
            let status = card
                .get("status")
                .and_then(Value::as_str)
                .unwrap_or("unknown");
            Some(format!("{id}:{status}"))
        })
        .collect::<Vec<_>>();
    if labels.is_empty() {
        "(none)".to_string()
    } else {
        labels.join(", ")
    }
}

fn value_text(value: &Value, key: &str) -> String {
    match value.get(key) {
        Some(Value::String(text)) if !text.is_empty() => text.clone(),
        Some(Value::Number(number)) => number.to_string(),
        Some(Value::Bool(flag)) => flag.to_string(),
        _ => "(none)".to_string(),
    }
}

fn list_text(value: &Value, key: &str) -> String {
    let Some(items) = value.get(key).and_then(Value::as_array) else {
        return "(none)".to_string();
    };
    let rendered = items
        .iter()
        .filter_map(Value::as_str)
        .take(8)
        .collect::<Vec<_>>();
    if rendered.is_empty() {
        "(none)".to_string()
    } else {
        rendered.join(", ")
    }
}

fn object_text(value: &Value, key: &str) -> String {
    let Some(object) = value.get(key).and_then(Value::as_object) else {
        return "(none)".to_string();
    };
    object
        .iter()
        .map(|(name, value)| format!("{name}={}", scalar_value_text(value)))
        .collect::<Vec<_>>()
        .join(", ")
}

fn object_list_text(value: &Value, key: &str, label_key: &str) -> String {
    let Some(items) = value.get(key).and_then(Value::as_array) else {
        return "(none)".to_string();
    };
    let labels = items
        .iter()
        .filter_map(|item| item.get(label_key).and_then(Value::as_str))
        .take(8)
        .collect::<Vec<_>>();
    if labels.is_empty() {
        "(none)".to_string()
    } else {
        labels.join(", ")
    }
}

fn scalar_value_text(value: &Value) -> String {
    match value {
        Value::String(text) => text.clone(),
        Value::Number(number) => number.to_string(),
        Value::Bool(flag) => flag.to_string(),
        _ => "(none)".to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::{AUTHORITY, render_regulator_action_from_review};
    use serde_json::json;
    use std::path::Path;

    fn review_packet() -> serde_json::Value {
        json!({
            "regulator_live_replay_v1": {
                "status": "felt_pressure_boundary_context",
                "cartography_source": "/tmp/regulator_boundary_cartography/latest.json",
                "felt_pressure_match_count": 3
            },
            "regulator_boundary_replay_cards_v1": {
                "status": "boundary_near_felt_pressure",
                "card_count": 2,
                "status_counts": {"near_pressure_jump": 1, "observational_plateau": 1},
                "cards": [
                    {
                        "card_id": "regulator_near_pressure_jump_1",
                        "status": "near_pressure_jump",
                        "term": "pressure_risk",
                        "finding_label": "pressure_risk >= 0.60 downward-bias boundary",
                        "axis": "pressure_risk",
                        "nearest_threshold": 0.60,
                        "evidence_anchors": ["pressure_risk", "regulator_audit"],
                        "texture_terms": ["heavy", "pressure"],
                        "public_sample_paths": ["/tmp/public_pressure.txt"],
                        "recommended_action": "Compare audits before tuning."
                    },
                    {
                        "card_id": "regulator_observational_plateau_2",
                        "status": "observational_plateau",
                        "term": "observational_plateau",
                        "finding_label": "pressure rises while output remains unchanged",
                        "axis": "pressure_risk",
                        "public_sample_paths": ["/tmp/public_plateau.txt"]
                    }
                ]
            },
            "regulator_plateau_missing_variable_model_v1": {
                "status": "plateau_missing_variable_hypotheses",
                "findings": [{"variable": "semantic_friction"}, {"variable": "stable_core"}]
            },
            "regulator_replay_time_series_v1": {
                "status": "repeated_boundary_near_pressure",
                "window_review_count": 3
            },
            "regulator_counterfactual_sweep_v1": {
                "status": "counterfactual_sweep_available",
                "candidate_count": 5
            },
            "regulator_counterfactual_replay_lab_v1": {
                "status": "replay_supported_with_plateau_caution",
                "verdict_counts": {
                    "replay_supported_offline_candidate": 1,
                    "missing_variable_first": 1
                },
                "evaluated_candidates": [
                    {
                        "candidate_family": "pressure_hysteresis",
                        "replay_fit": "repeated_boundary_support",
                        "verdict": "replay_supported_offline_candidate",
                        "target_statuses": ["near_pressure_jump"],
                        "matched_statuses": ["near_pressure_jump"],
                        "matched_card_ids": ["regulator_near_pressure_jump_1"],
                        "recurrent_count": 3,
                        "estimated_reduction_pct": 60.0
                    },
                    {
                        "candidate_family": "thin_density_softening",
                        "replay_fit": "plateau_recurrence_outweighs_threshold_smoothing",
                        "verdict": "missing_variable_first",
                        "target_statuses": ["thin_density_boundary"],
                        "matched_statuses": [],
                        "matched_card_ids": [],
                        "recurrent_count": 0,
                        "estimated_reduction_pct": 40.0
                    }
                ]
            },
            "regulator_plateau_evidence_matrix_v1": {
                "status": "unresolved_missing_variables",
                "top_unresolved_variables": [
                    {"variable": "semantic_friction", "confidence": "high", "score": 9.5},
                    {"variable": "pressure_source", "confidence": "medium", "score": 5.0}
                ],
                "variables": [
                    {
                        "variable": "semantic_friction",
                        "score": 9.5,
                        "confidence": "high",
                        "resolving_audit_routes": ["PRESSURE_SOURCE_AUDIT semantic-friction"]
                    }
                ]
            },
            "regulator_tuning_readiness_gate_v1": {
                "status": "blocked_missing_variable",
                "gate_counts": {"blocked_missing_variable": 2},
                "unresolved_missing_variables": ["semantic_friction", "pressure_source"],
                "gated_candidates": [
                    {
                        "candidate_family": "pressure_hysteresis",
                        "gate_status": "blocked_missing_variable",
                        "gate_reason": "plateau evidence has unresolved high/medium missing variables",
                        "replay_verdict": "replay_supported_offline_candidate",
                        "matched_card_ids": ["regulator_near_pressure_jump_1"],
                        "unresolved_missing_variables": ["semantic_friction", "pressure_source"]
                    },
                    {
                        "candidate_family": "thin_density_softening",
                        "gate_status": "blocked_missing_variable",
                        "gate_reason": "plateau evidence has unresolved high/medium missing variables",
                        "replay_verdict": "missing_variable_first",
                        "matched_card_ids": [],
                        "unresolved_missing_variables": ["semantic_friction", "pressure_source"]
                    }
                ]
            },
            "pi_pressure_wiring_replay_v1": {
                "status": "replay_supported_candidates",
                "source": "live-db",
                "source_status": "live_window_ready",
                "sample_count": 12,
                "candidate_count": 2,
                "artifact_path": "/tmp/pi_pressure_wiring_replay.json",
                "candidate_status_counts": {"replay_supported": 1, "snap_risk": 1},
                "top_candidates": [
                    {
                        "candidate_family": "pressure_source_target_bias",
                        "status": "replay_supported",
                        "estimated_improvement_pct": 18.0,
                        "pressure_alignment_delta": 0.08,
                        "snap_risk_delta": -0.02,
                        "afterimage_risk_delta": -0.01,
                        "default_off_canary": {
                            "default_off_env": "MINIME_PI_PRESSURE_WIRING_CANARY",
                            "eligible": true
                        }
                    }
                ]
            },
            "pi_pressure_candidate_readiness_v1": {
                "status": "blocked_missing_variable",
                "readiness_counts": {"blocked_missing_variable": 1, "blocked_safety_review": 1},
                "unresolved_missing_variables": ["semantic_friction", "pressure_source"],
                "candidates": [
                    {
                        "candidate_family": "pressure_source_target_bias",
                        "gate_status": "blocked_missing_variable",
                        "gate_reason": "plateau evidence still has unresolved high/medium missing variables",
                        "replay_status": "replay_supported",
                        "estimated_improvement_pct": 18.0,
                        "snap_risk_delta": -0.02,
                        "afterimage_risk_delta": -0.01,
                        "default_off_canary": {
                            "default_off_env": "MINIME_PI_PRESSURE_WIRING_CANARY",
                            "eligible": false
                        }
                    }
                ]
            },
            "pressure_source_to_pi_gap_v1": {
                "status": "replay_available_gap_open",
                "pressure_vector_status": "rising_overpacked_pressure",
                "pressure_medium_status": "semantic_friction_medium",
                "pi_replay_status": "replay_supported_candidates",
                "pi_readiness_status": "blocked_missing_variable",
                "source_anchors": ["pressure_vector:rising_overpacked_pressure"],
                "recommended_routes": [
                    "PI_PRESSURE_REPLAY_STATUS latest",
                    "PRESSURE_SOURCE_AUDIT current-fill_pressure"
                ]
            },
            "regulator_missing_variable_evidence_loop_v1": {
                "status": "evidence_needed_before_tuning",
                "blocked_gate_status": "blocked_missing_variable",
                "probe_count": 2,
                "top_probes": [
                    {
                        "variable": "semantic_friction",
                        "priority": "high",
                        "suggested_next": "PRESSURE_SOURCE_AUDIT semantic-friction",
                        "source_confidence": "high"
                    },
                    {
                        "variable": "pressure_source",
                        "priority": "high",
                        "suggested_next": "PRESSURE_SOURCE_AUDIT current-fill_pressure",
                        "source_confidence": "medium"
                    }
                ],
                "probes": [
                    {
                        "variable": "semantic_friction",
                        "priority": "high",
                        "suggested_next": "PRESSURE_SOURCE_AUDIT semantic-friction",
                        "source_confidence": "high"
                    },
                    {
                        "variable": "pressure_source",
                        "priority": "high",
                        "suggested_next": "PRESSURE_SOURCE_AUDIT current-fill_pressure",
                        "source_confidence": "medium"
                    }
                ]
            },
            "returnable_distinctions_v1": {
                "status": "returnable_distinctions_present",
                "active_card_count": 2,
                "cards": [
                    {
                        "card_id": "measurement_vs_alignment_vs_damping",
                        "status": "control_semantics_ambiguity",
                        "lifecycle_state": "needs_audit",
                        "preflight_verdict": "audit_first",
                        "next_resolution_route": "REGULATOR_MAP_STATUS latest",
                        "recommended_read_only_route": "REGULATOR_MAP_STATUS latest",
                        "relevant_self_regulation_route": "SELF_REGULATION_STATUS",
                        "relevant_experiment_lived_term_route": "REGULATOR_MAP_STATUS latest"
                    },
                    {
                        "card_id": "pressure_level_vs_pressure_velocity",
                        "status": "felt_pressure_without_trend_context",
                        "lifecycle_state": "needs_audit",
                        "preflight_verdict": "audit_first",
                        "next_resolution_route": "PRESSURE_SOURCE_AUDIT current-fill_pressure",
                        "recommended_read_only_route": "PRESSURE_SOURCE_AUDIT current-fill_pressure",
                        "relevant_self_regulation_route": "SELF_REGULATION_PREFLIGHT latest",
                        "relevant_experiment_lived_term_route": "EXPERIMENT_OBSERVE current :: pressure_trend=<stable|rising|falling>"
                    }
                ]
            }
        })
    }

    #[test]
    fn regulator_map_status_renders_advisory_summary_without_dispatch() {
        let text = render_regulator_action_from_review(
            &review_packet(),
            Path::new("/tmp/review.json"),
            "REGULATOR_MAP_STATUS",
            "latest",
        );

        assert!(text.contains("=== REGULATOR MAP STATUS ==="));
        assert!(text.contains(AUTHORITY));
        assert!(text.contains("Replay status: `felt_pressure_boundary_context`"));
        assert!(text.contains("Counterfactual sweep: status=`counterfactual_sweep_available`"));
        assert!(
            text.contains(
                "Counterfactual replay lab: status=`replay_supported_with_plateau_caution`"
            )
        );
        assert!(text.contains("Plateau evidence matrix: status=`unresolved_missing_variables`"));
        assert!(text.contains("Tuning readiness gate: status=`blocked_missing_variable`"));
        assert!(text.contains("Why not tuning yet"));
        assert!(text.contains("PI pressure wiring replay"));
        assert!(text.contains("pressure_source_target_bias"));
        assert!(text.contains("Pressure-source-to-PI gap"));
        assert!(text.contains("Missing-variable evidence loop"));
        assert!(text.contains("Returnable distinctions"));
        assert!(text.contains("measurement_vs_alignment_vs_damping"));
        assert!(text.contains("lifecycle=needs_audit"));
        assert!(text.contains("verdict=audit_first"));
        assert!(text.contains("semantic_friction->PRESSURE_SOURCE_AUDIT semantic-friction"));
        assert!(text.contains("pressure_hysteresis"));
        assert!(text.contains("created no experiment"));
        assert!(text.contains("tuned no controller"));
    }

    #[test]
    fn pi_pressure_replay_status_renders_candidate_gate_without_dispatch() {
        let text = render_regulator_action_from_review(
            &review_packet(),
            Path::new("/tmp/review.json"),
            "PI_PRESSURE_REPLAY_STATUS",
            "latest",
        );

        assert!(text.contains("=== PI PRESSURE REPLAY STATUS ==="));
        assert!(text.contains("replay_supported_candidates"));
        assert!(text.contains("blocked_missing_variable"));
        assert!(text.contains("pressure_source_target_bias"));
        assert!(text.contains("MINIME_PI_PRESSURE_WIRING_CANARY"));
        assert!(text.contains("runtime_ignored_in_this_tranche=true"));
        assert!(text.contains("No experiment was created"));
        assert!(text.contains("no controller was tuned"));
    }

    #[test]
    fn regulator_replay_status_filters_by_status_without_dispatch() {
        let text = render_regulator_action_from_review(
            &review_packet(),
            Path::new("/tmp/review.json"),
            "REGULATOR_REPLAY_STATUS",
            "observational_plateau",
        );

        assert!(text.contains("=== REGULATOR REPLAY STATUS ==="));
        assert!(text.contains("Matched cards: 1"));
        assert!(text.contains("regulator_observational_plateau_2"));
        assert!(text.contains("Counterfactual matches"));
        assert!(text.contains("missing_variable_first"));
        assert!(text.contains("Tuning readiness gate"));
        assert!(text.contains("blocked_missing_variable"));
        assert!(text.contains("Missing-variable evidence loop"));
        assert!(text.contains("Returnable distinctions"));
        assert!(text.contains("SELF_REGULATION_PREFLIGHT latest"));
        assert!(text.contains("PRESSURE_SOURCE_AUDIT semantic-friction"));
        assert!(text.contains("no controller was tuned"));
    }

    #[test]
    fn regulator_boundary_card_renders_card_detail_without_dispatch() {
        let text = render_regulator_action_from_review(
            &review_packet(),
            Path::new("/tmp/review.json"),
            "REGULATOR_BOUNDARY_CARD",
            "regulator_near_pressure_jump_1",
        );

        assert!(text.contains("=== REGULATOR BOUNDARY CARD ==="));
        assert!(text.contains("pressure_risk >= 0.60"));
        assert!(text.contains("pressure_risk, regulator_audit"));
        assert!(text.contains("Compare audits before tuning."));
        assert!(text.contains("pressure_hysteresis:replay_supported_offline_candidate"));
        assert!(text.contains("pressure_hysteresis:blocked_missing_variable"));
        assert!(text.contains("Evidence loop probes"));
        assert!(text.contains("pressure_source->PRESSURE_SOURCE_AUDIT current-fill_pressure"));
        assert!(text.contains("Returnable distinctions"));
        assert!(text.contains("pressure_level_vs_pressure_velocity"));
        assert!(text.contains("mutated no peer"));
    }
}
