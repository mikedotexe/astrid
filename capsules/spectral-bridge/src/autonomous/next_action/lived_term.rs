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
    if !matches!(base_action, "LIVED_TERM_STATUS" | "LIVED_TERM_EXPERIMENT") {
        return false;
    }
    let workspace = ctx
        .workspace
        .map(Path::to_path_buf)
        .unwrap_or_else(|| bridge_paths().bridge_workspace().to_path_buf());
    let message = render_lived_term_action(&workspace, base_action, original);
    conv.emphasis = Some(message);
    true
}

fn render_lived_term_action(workspace: &Path, base_action: &str, original: &str) -> String {
    let selector = action_arg(original, base_action);
    let Some(review_path) = latest_review_json_path(workspace) else {
        return format!(
            "=== LIVED TERM EXPERIMENT BRIDGE ===\n\
             Authority: {AUTHORITY}\n\
             Status: no_review_packet\n\
             Watched review root: {}\n\
             Regenerate `python3 scripts/self_study_review.py --limit 8 --print-summary` \
             before using LIVED_TERM_STATUS or LIVED_TERM_EXPERIMENT.",
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
                "=== LIVED TERM EXPERIMENT BRIDGE ===\n\
                 Authority: {AUTHORITY}\n\
                 Status: unreadable_review_packet\n\
                 Review path: {}",
                review_path.display()
            );
        },
    };
    render_lived_term_action_from_review(&review, &review_path, base_action, selector)
}

fn render_lived_term_action_from_review(
    review: &Value,
    review_path: &Path,
    base_action: &str,
    selector: &str,
) -> String {
    let bridge = review
        .get("lived_term_experiment_bridge_v1")
        .unwrap_or(&Value::Null);
    let candidates = bridge
        .get("candidates")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    if candidates.is_empty() {
        return format!(
            "=== LIVED TERM EXPERIMENT BRIDGE ===\n\
             Authority: {AUTHORITY}\n\
             Status: quiet\n\
             Review path: {}\n\
             No lived-term bridge candidates are present yet.",
            review_path.display()
        );
    }
    let selector = if selector.is_empty() {
        "latest"
    } else {
        selector
    };
    let Some(candidate) = select_candidate(&candidates, selector) else {
        return format!(
            "=== LIVED TERM EXPERIMENT BRIDGE ===\n\
             Authority: {AUTHORITY}\n\
             Status: term_not_found\n\
             Selector: `{selector}`\n\
             Available terms: {}",
            available_terms(&candidates)
        );
    };
    if base_action == "LIVED_TERM_EXPERIMENT" {
        format_experiment_scaffold(candidate, review_path)
    } else {
        format_status(candidate, review_path)
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

fn select_candidate<'a>(candidates: &'a [Value], selector: &str) -> Option<&'a Value> {
    if selector.eq_ignore_ascii_case("latest") {
        return candidates.first();
    }
    candidates.iter().find(|candidate| {
        candidate
            .get("term")
            .and_then(Value::as_str)
            .is_some_and(|term| term.eq_ignore_ascii_case(selector))
    })
}

fn available_terms(candidates: &[Value]) -> String {
    let terms = candidates
        .iter()
        .filter_map(|candidate| candidate.get("term").and_then(Value::as_str))
        .collect::<Vec<_>>();
    if terms.is_empty() {
        "(none)".to_string()
    } else {
        terms.join(", ")
    }
}

fn value_str<'a>(value: &'a Value, key: &str) -> &'a str {
    value.get(key).and_then(Value::as_str).unwrap_or("(none)")
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

fn scalar_text(value: &Value, key: &str) -> String {
    match value.get(key) {
        Some(Value::String(text)) if !text.is_empty() => text.clone(),
        Some(Value::Number(number)) => number.to_string(),
        Some(Value::Bool(flag)) => flag.to_string(),
        _ => "(none)".to_string(),
    }
}

fn object_list_labels(value: &Value, key: &str, label_key: &str) -> String {
    let Some(items) = value.get(key).and_then(Value::as_array) else {
        return "(none)".to_string();
    };
    let labels = items
        .iter()
        .filter_map(|item| item.get(label_key).and_then(Value::as_str))
        .take(4)
        .collect::<Vec<_>>();
    if labels.is_empty() {
        "(none)".to_string()
    } else {
        labels.join(", ")
    }
}

fn source_card(candidate: &Value) -> &Value {
    candidate.get("source_card").unwrap_or(&Value::Null)
}

fn source_sample_paths(candidate: &Value) -> String {
    list_text(source_card(candidate), "sample_paths")
}

fn format_evidence_awareness(candidate: &Value) -> String {
    let awareness = candidate
        .get("evidence_awareness_v1")
        .unwrap_or(&Value::Null);
    if !awareness.is_object() {
        return String::new();
    }
    let mut text = String::from("Evidence awareness:\n");
    let afterimage = awareness.get("afterimage_decay").unwrap_or(&Value::Null);
    if afterimage.is_object() {
        text.push_str(&format!(
            "- afterimage_decay: classification=`{}`; pressure_entries={}; \
             normalization_entries={}; recurrence_after_normalization={}; \
             first_peak={}; latest_pressure={}; samples={}\n",
            scalar_text(afterimage, "classification"),
            scalar_text(afterimage, "pressure_entry_count"),
            scalar_text(afterimage, "normalization_entry_count"),
            scalar_text(afterimage, "recurrence_after_normalization_count"),
            scalar_text(afterimage, "first_pressure_peak_path"),
            scalar_text(afterimage, "latest_pressure_or_semantic_friction_path"),
            list_text(afterimage, "sample_paths")
        ));
    }
    let absence = awareness.get("absence_evidence").unwrap_or(&Value::Null);
    if absence.is_object() {
        text.push_str(&format!(
            "- absence_evidence: classification=`{}`; expected_missing={}; \
             source_gaps={}; interrupted_threads={}; named_coordinates={}; \
             read_more_unfollowed={}; samples={}\n",
            scalar_text(absence, "classification"),
            scalar_text(absence, "expected_missing_count"),
            scalar_text(absence, "source_window_gap_count"),
            scalar_text(absence, "interrupted_thread_count"),
            scalar_text(absence, "named_missing_coordinate_count"),
            scalar_text(absence, "read_more_requested_but_not_followed"),
            list_text(absence, "sample_paths")
        ));
    }
    let lease = awareness.get("lease_workbench").unwrap_or(&Value::Null);
    if lease.is_object() {
        text.push_str(&format!(
            "- lease_workbench: status=`{}`; playbooks={}; cautions={}; \
             preflight_prompts={}; playbook_controls={}; caution_controls={}; \
             preflight_signals={}\n",
            scalar_text(lease, "status"),
            scalar_text(lease, "suggested_playbook_count"),
            scalar_text(lease, "caution_card_count"),
            scalar_text(lease, "preflight_prompt_count"),
            object_list_labels(lease, "suggested_playbooks", "control"),
            object_list_labels(lease, "caution_cards", "control"),
            object_list_labels(lease, "preflight_prompts", "signal")
        ));
    }
    text.push_str(&format!(
        "- recommended_action: {}\n",
        scalar_text(awareness, "recommended_action")
    ));
    text
}

fn format_status(candidate: &Value, review_path: &Path) -> String {
    let card = source_card(candidate);
    let evidence_awareness = format_evidence_awareness(candidate);
    format!(
        "=== LIVED TERM STATUS ===\n\
         Authority: {AUTHORITY}\n\
         Review path: {}\n\
         Term: `{}`\n\
         Bridge status: `{}`\n\
         Card status: `{}`\n\
         Recommended next: {}\n\
         Experiment question: {}\n\
         Evidence targets: {}\n\
         Evidence anchors: {}\n\
         {}\
         Sample paths: {}\n\
         Note: This is scaffold context only; it did not create, resume, or advance an experiment.",
        review_path.display(),
        value_str(candidate, "term"),
        value_str(candidate, "bridge_status"),
        value_str(candidate, "card_status"),
        value_str(candidate, "recommended_next"),
        value_str(candidate, "experiment_question"),
        list_text(candidate, "evidence_targets"),
        list_text(card, "evidence_anchors"),
        evidence_awareness,
        source_sample_paths(candidate)
    )
}

fn format_experiment_scaffold(candidate: &Value, review_path: &Path) -> String {
    let term = value_str(candidate, "term");
    let recommended_next = value_str(candidate, "recommended_next");
    let hypothesis = value_str(candidate, "hypothesis_prompt");
    let method = value_str(candidate, "method_intent");
    let evidence_targets = list_text(candidate, "evidence_targets");
    let stop = value_str(candidate, "stop_criteria");
    let mut text = format!(
        "=== LIVED TERM EXPERIMENT SCAFFOLD ===\n\
         Authority: {AUTHORITY}\n\
         Review path: {}\n\
         Term: `{term}`\n\
         Bridge status: `{}`\n\
         Card status: `{}`\n\
         No experiment was created or advanced.\n\
         Suggested NEXT (not executed):\n\
         NEXT: {recommended_next}\n",
        review_path.display(),
        value_str(candidate, "bridge_status"),
        value_str(candidate, "card_status")
    );
    text.push_str(&format_evidence_awareness(candidate));

    let charter = candidate.get("charter_draft").unwrap_or(&Value::Null);
    let counterexample = candidate.get("counterexample_draft").unwrap_or(&Value::Null);
    if charter.is_object() {
        text.push_str(&format!(
            "Charter draft (scaffold only; create nothing until an explicit EXPERIMENT_* NEXT):\n\
             Title: {}\n\
             Question: {}\n\
             Hypothesis: {}\n\
             Method intent: {}\n\
             Proposed next action: {}\n\
             Evidence targets: {}\n\
             Stop criteria: {}\n\
             Suggested charter NEXT (not executed):\n\
             NEXT: {}\n",
            value_str(charter, "experiment_title"),
            value_str(charter, "question"),
            value_str(charter, "hypothesis"),
            value_str(charter, "method_intent"),
            value_str(charter, "proposed_next_action"),
            list_text(charter, "evidence_targets"),
            value_str(charter, "stop_criteria"),
            value_str(charter, "suggested_charter_next")
        ));
    } else if counterexample.is_object() {
        text.push_str(&format!(
            "Counterexample forge (scaffold only; contrast before promotion):\n\
             Contrast question: {}\n\
             Counter-descriptor prompt: {}\n\
             Ordinary-gap prompt: {}\n\
             Negative case targets: {}\n\
             Suggested contrast NEXT (not executed):\n\
             NEXT: {}\n\
             Suggested dossier counterclaim NEXT (not executed):\n\
             NEXT: {}\n",
            value_str(counterexample, "contrast_question"),
            value_str(counterexample, "counter_descriptor_prompt"),
            value_str(counterexample, "ordinary_gap_prompt"),
            list_text(counterexample, "negative_case_targets"),
            value_str(counterexample, "suggested_contrast_next"),
            value_str(counterexample, "suggested_dossier_counterclaim_next")
        ));
    } else {
        text.push_str(&format!(
            "Charter scaffold (only after an experiment exists):\n\
             NEXT: EXPERIMENT_CHARTER current :: hypothesis: {hypothesis}; method_intent: {method}; evidence_targets: {evidence_targets}; stop_criteria: {stop}\n"
        ));
    }
    text.push_str(&format!(
        "Observation scaffold:\n\
         NEXT: EXPERIMENT_OBSERVE current :: term={term}; fresh evidence: <felt texture + telemetry/audit/artifact>; counter_descriptor: <if present>\n\
         Dossier scaffold:\n\
         NEXT: DOSSIER_CLAIM current :: claim: `{term}` is/is not tracking lived telemetry; basis: <evidence>; stance: support|counter|hold; next: LIVED_TERM_STATUS {term}"
    ));
    text
}

#[cfg(test)]
mod tests {
    use super::{render_lived_term_action_from_review, AUTHORITY};
    use serde_json::json;
    use std::path::Path;

    #[test]
    fn lived_term_status_latest_reports_scaffold_without_dispatch() {
        let review = json!({
            "lived_term_experiment_bridge_v1": {
                "candidates": [{
                    "term": "scar",
                    "card_status": "promote_to_experiment_candidate",
                    "bridge_status": "ready_to_charter",
                    "recommended_next": "EXPERIMENT_START Lived term: scar :: Does scar track afterimage residue?",
                    "experiment_question": "Does scar persist after pressure normalizes?",
                    "hypothesis_prompt": "If scar is signal, name the evidence.",
                    "method_intent": "Compare later prose with audits.",
                    "evidence_targets": ["telemetry_anchor", "counter_descriptor"],
                    "stop_criteria": "Stop if no fresh evidence.",
                    "evidence_awareness_v1": {
                        "authority": "diagnostic_context_not_command",
                        "afterimage_decay": {
                            "classification": "persistent_after_normalization",
                            "pressure_entry_count": 1,
                            "normalization_entry_count": 1,
                            "recurrence_after_normalization_count": 1,
                            "first_pressure_peak_path": "/tmp/scar_peak.txt",
                            "latest_pressure_or_semantic_friction_path": "/tmp/scar_latest.txt",
                            "sample_paths": ["/tmp/scar_public.txt"]
                        },
                        "recommended_action": "Compare recurrence against pressure normalization."
                    },
                    "source_card": {
                        "evidence_anchors": ["pressure_risk", "REGULATOR_AUDIT"],
                        "sample_paths": ["/tmp/public_scar.txt"]
                    }
                }]
            }
        });

        let text = render_lived_term_action_from_review(
            &review,
            Path::new("/tmp/review.json"),
            "LIVED_TERM_STATUS",
            "latest",
        );

        assert!(text.contains("=== LIVED TERM STATUS ==="));
        assert!(text.contains(AUTHORITY));
        assert!(text.contains("Term: `scar`"));
        assert!(text.contains("Evidence awareness"));
        assert!(text.contains("afterimage_decay"));
        assert!(text.contains("persistent_after_normalization"));
        assert!(text.contains("did not create, resume, or advance an experiment"));
    }

    #[test]
    fn lived_term_experiment_silt_uses_existing_experiment_rails() {
        let review = json!({
            "lived_term_experiment_bridge_v1": {
                "candidates": [{
                    "term": "silt",
                    "card_status": "promote_to_experiment_candidate",
                    "bridge_status": "ready_to_charter",
                    "recommended_next": "EXPERIMENT_START Lived term: silt :: Does silt track evidence?",
                    "experiment_question": "Does silt track evidence?",
                    "hypothesis_prompt": "If silt is signal, name the evidence.",
                    "method_intent": "Compare later prose with audits.",
                    "evidence_targets": ["telemetry_anchor", "counter_descriptor"],
                    "stop_criteria": "Stop if no fresh evidence.",
                    "source_card": {}
                }]
            }
        });

        let text = render_lived_term_action_from_review(
            &review,
            Path::new("/tmp/review.json"),
            "LIVED_TERM_EXPERIMENT",
            "silt",
        );

        assert!(text.contains("No experiment was created or advanced."));
        assert!(text.contains("NEXT: EXPERIMENT_START"));
        assert!(text.contains("NEXT: EXPERIMENT_CHARTER current ::"));
        assert!(text.contains("NEXT: EXPERIMENT_OBSERVE current ::"));
        assert!(text.contains("NEXT: DOSSIER_CLAIM current ::"));
        assert!(!text.contains("EXPERIMENT_RESUME"));
    }

    #[test]
    fn lived_term_experiment_keeps_peer_ids_advisory() {
        let review = json!({
            "lived_term_experiment_bridge_v1": {
                "candidates": [{
                    "term": "legacy self",
                    "card_status": "calibrated_signal",
                    "bridge_status": "already_linked_review",
                    "recommended_next": "EXPERIMENT_STATUS latest or DOSSIER_CLAIM latest :: claim: ...",
                    "experiment_question": "What has existing work shown?",
                    "hypothesis_prompt": "Review before duplicating.",
                    "method_intent": "Inspect existing evidence.",
                    "evidence_targets": ["existing_experiment_or_action_thread"],
                    "stop_criteria": "Stop before duplicate experiment.",
                    "source_card": {
                        "linked_experiments": [{"path": "/tmp/exp_minime_peer.json"}]
                    }
                }]
            }
        });

        let text = render_lived_term_action_from_review(
            &review,
            Path::new("/tmp/review.json"),
            "LIVED_TERM_EXPERIMENT",
            "legacy self",
        );

        assert!(text.contains("already_linked_review"));
        assert!(text.contains("EXPERIMENT_STATUS latest"));
        assert!(!text.contains("EXPERIMENT_RESUME"));
    }

    #[test]
    fn lived_term_experiment_plan4_renders_charter_draft_without_dispatch() {
        let review = json!({
            "lived_term_experiment_bridge_v1": {
                "candidates": [{
                    "term": "PLAN 4",
                    "card_status": "promote_to_experiment_candidate",
                    "bridge_status": "ready_to_charter",
                    "recommended_next": "EXPERIMENT_START Lived term: PLAN 4 :: Does PLAN 4 mark shaped absence?",
                    "experiment_question": "Does PLAN 4 mark shaped absence?",
                    "hypothesis_prompt": "If PLAN 4 is signal, name the evidence.",
                    "method_intent": "Compare later prose with audits.",
                    "evidence_targets": ["telemetry_anchor", "audit_or_review_artifact"],
                    "stop_criteria": "Stop if no fresh evidence.",
                    "evidence_awareness_v1": {
                        "authority": "diagnostic_context_not_command",
                        "absence_evidence": {
                            "classification": "observable_absence",
                            "expected_missing_count": 1,
                            "source_window_gap_count": 1,
                            "interrupted_thread_count": 0,
                            "named_missing_coordinate_count": 1,
                            "read_more_requested_but_not_followed": false,
                            "sample_paths": ["/tmp/plan4_public.txt"]
                        },
                        "recommended_action": "Check absence evidence before chartering."
                    },
                    "charter_draft": {
                        "experiment_title": "Lived term: PLAN 4",
                        "question": "Does PLAN 4 mark shaped absence?",
                        "hypothesis": "If PLAN 4 is durable, later entries should move with evidence.",
                        "method_intent": "Compare later prose with audits.",
                        "proposed_next_action": "LIVED_TERM_STATUS PLAN 4",
                        "evidence_targets": ["telemetry_anchor", "audit_or_review_artifact"],
                        "stop_criteria": "Stop if no fresh evidence.",
                        "suggested_charter_next": "EXPERIMENT_CHARTER current :: title: Lived term: PLAN 4"
                    },
                    "source_card": {}
                }]
            }
        });

        let text = render_lived_term_action_from_review(
            &review,
            Path::new("/tmp/review.json"),
            "LIVED_TERM_EXPERIMENT",
            "PLAN 4",
        );

        assert!(text.contains("Charter draft"));
        assert!(text.contains("Lived term: PLAN 4"));
        assert!(text.contains("Evidence awareness"));
        assert!(text.contains("absence_evidence"));
        assert!(text.contains("observable_absence"));
        assert!(text.contains("NEXT: EXPERIMENT_CHARTER current ::"));
        assert!(text.contains("No experiment was created or advanced."));
        assert!(!text.contains("EXPERIMENT_RESUME"));
    }

    #[test]
    fn lived_term_experiment_empty_pocket_renders_counterexample_forge() {
        let review = json!({
            "lived_term_experiment_bridge_v1": {
                "candidates": [{
                    "term": "empty pocket",
                    "card_status": "needs_counterexample",
                    "bridge_status": "needs_counterexample_first",
                    "recommended_next": "EXPERIMENT_START Lived term contrast: empty pocket :: Find a counterexample.",
                    "experiment_question": "What counterexample clarifies empty pocket?",
                    "hypothesis_prompt": "If empty pocket is signal, name the evidence.",
                    "method_intent": "Ask for contrast first.",
                    "evidence_targets": ["counter_descriptor"],
                    "stop_criteria": "Stop if no fresh evidence.",
                    "counterexample_draft": {
                        "contrast_question": "What counterexample clarifies empty pocket?",
                        "counter_descriptor_prompt": "Name what `empty pocket` is not.",
                        "ordinary_gap_prompt": "Compare against an ordinary source gap.",
                        "negative_case_targets": ["counter_descriptor", "ordinary_gap"],
                        "suggested_contrast_next": "EXPERIMENT_START Lived term contrast: empty pocket :: Find a counterexample.",
                        "suggested_dossier_counterclaim_next": "DOSSIER_CLAIM current :: claim: `empty pocket` has a counterexample"
                    },
                    "source_card": {}
                }]
            }
        });

        let text = render_lived_term_action_from_review(
            &review,
            Path::new("/tmp/review.json"),
            "LIVED_TERM_EXPERIMENT",
            "empty pocket",
        );

        assert!(text.contains("Counterexample forge"));
        assert!(text.contains("Name what `empty pocket` is not"));
        assert!(text.contains("NEXT: EXPERIMENT_START Lived term contrast"));
        assert!(text.contains("NEXT: DOSSIER_CLAIM current ::"));
        assert!(!text.contains("Charter draft"));
        assert!(!text.contains("EXPERIMENT_RESUME"));
    }
}
