#[derive(Debug)]
struct RehearsalAssessment {
    stage: &'static str,
    status: &'static str,
    blocked: bool,
    gate_decision: Value,
    summary: &'static str,
    interpretation: &'static str,
}

#[derive(Debug)]
struct ExperimentDecision<'a> {
    outcome: &'a str,
    reason: String,
}

fn parse_experiment_charter(experiment: &ExperimentRecord, raw: &str) -> Value {
    let hypothesis = charter_field(raw, &["hypothesis"])
        .unwrap_or_else(|| experiment.hypothesis.clone().unwrap_or_default());
    let method_intent = charter_field(raw, &["method_intent", "method"]).unwrap_or_default();
    let proposed_next_action = charter_field(raw, &["proposed_next_action", "next"])
        .or_else(|| find_next_line(raw))
        .unwrap_or_default();
    let evidence_targets = charter_list_field(raw, &["evidence_targets", "evidence", "measures"]);
    let stop_criteria = charter_list_field(raw, &["stop_criteria", "stop"]);
    let consent_posture =
        charter_field(raw, &["consent_posture", "consent"]).unwrap_or_else(|| {
            "advisory; ordinary choices remain valid; refusal and counteroffer are valid outcomes"
                .to_string()
        });
    let authority_level =
        charter_field(raw, &["authority_level", "authority"]).unwrap_or_else(|| {
            "rehearsal_first_existing_gates_only; no new live-control authority".to_string()
        });
    let source_journal_refs = charter_list_field(raw, &["source_journal_refs", "source"]);
    json!({
        "schema_version": SCHEMA_VERSION,
        "authored_by": SYSTEM,
        "hypothesis": hypothesis,
        "method_intent": method_intent,
        "proposed_next_action": proposed_next_action,
        "evidence_targets": evidence_targets,
        "stop_criteria": stop_criteria,
        "consent_posture": consent_posture,
        "authority_level": authority_level,
        "source_journal_refs": source_journal_refs,
        "raw_text": raw.trim(),
        "recorded_at": iso_now(),
    })
}

fn charter_payload_has_meaning(raw: &str) -> bool {
    let raw = raw.trim();
    if raw.is_empty() || placeholder_payload(raw) {
        return false;
    }
    charter_field(raw, &["hypothesis"]).is_some()
        || charter_field(raw, &["method_intent", "method"]).is_some()
        || charter_field(raw, &["proposed_next_action", "next"]).is_some()
        || find_next_line(raw).is_some()
        || !charter_list_field(raw, &["evidence_targets", "evidence", "measures"]).is_empty()
        || !charter_list_field(raw, &["stop_criteria", "stop"]).is_empty()
}

fn valid_experiment_charter(charter: Option<&Value>) -> bool {
    let Some(charter) = charter else {
        return false;
    };
    meaningful_charter_text(charter.get("hypothesis"))
        || meaningful_charter_text(charter.get("method_intent"))
        || meaningful_charter_text(charter.get("proposed_next_action"))
        || meaningful_charter_list(charter.get("evidence_targets"))
        || meaningful_charter_list(charter.get("stop_criteria"))
}

fn lifecycle_valid_charter_value(charter: Option<&Value>) -> bool {
    let Some(charter) = charter else {
        return false;
    };
    meaningful_charter_text(charter.get("hypothesis"))
        && meaningful_charter_text(charter.get("proposed_next_action"))
        && meaningful_charter_list(charter.get("evidence_targets"))
}

fn meaningful_charter_text(value: Option<&Value>) -> bool {
    value
        .and_then(Value::as_str)
        .map(str::trim)
        .is_some_and(|text| !text.is_empty() && !placeholder_payload(text))
}

fn meaningful_charter_list(value: Option<&Value>) -> bool {
    value
        .and_then(Value::as_array)
        .is_some_and(|items| items.iter().any(|item| meaningful_charter_text(Some(item))))
}

fn experiment_evidence_is_meaningful(evidence: Option<&Value>) -> bool {
    let Some(evidence) = evidence else {
        return false;
    };
    let meaningful_felt = evidence
        .get("felt_observations")
        .and_then(Value::as_array)
        .is_some_and(|items| {
            items.iter().any(|item| {
                meaningful_charter_text(item.get("note"))
                    || meaningful_charter_text(item.get("felt"))
                    || meaningful_charter_text(item.get("summary"))
            })
        });
    let meaningful_telemetry = evidence
        .get("telemetry_snapshots")
        .and_then(Value::as_array)
        .is_some_and(|items| !items.is_empty());
    let meaningful_artifact = evidence
        .get("artifact_refs")
        .and_then(Value::as_array)
        .is_some_and(|items| {
            items.iter().any(|item| {
                item.as_object().is_some_and(|object| !object.is_empty())
                    || meaningful_charter_text(Some(item))
            })
        });
    meaningful_felt || meaningful_telemetry || meaningful_artifact
}

fn charter_field(raw: &str, labels: &[&str]) -> Option<String> {
    for line in raw.lines() {
        let trimmed = line.trim().trim_start_matches(['-', '*', ' ']).trim();
        let lower = trimmed.to_ascii_lowercase();
        for label in labels {
            let label_lower = label.to_ascii_lowercase();
            for marker in [format!("{label_lower}:"), format!("{label_lower} =")] {
                if lower.starts_with(&marker) {
                    let value = trimmed[marker.len()..].trim();
                    if !value.is_empty() {
                        return Some(value.to_string());
                    }
                }
            }
        }
    }
    None
}

fn charter_list_field(raw: &str, labels: &[&str]) -> Vec<String> {
    charter_field(raw, labels)
        .map(|value| {
            value
                .split([',', ';'])
                .map(str::trim)
                .filter(|item| !item.is_empty())
                .map(str::to_string)
                .collect::<Vec<_>>()
        })
        .unwrap_or_default()
}

fn dossier_field(raw: &str, labels: &[&str]) -> Option<String> {
    for segment in raw.split([';', '\n']) {
        let trimmed = segment.trim().trim_start_matches(['-', '*', ' ']).trim();
        let lower = trimmed.to_ascii_lowercase();
        for label in labels {
            let label_lower = label.to_ascii_lowercase();
            for marker in [format!("{label_lower}:"), format!("{label_lower} =")] {
                if lower.starts_with(&marker) {
                    let value = trimmed[marker.len()..].trim();
                    if !value.is_empty() && !placeholder_payload(value) {
                        return Some(value.to_string());
                    }
                }
            }
        }
    }
    None
}

fn dossier_list_field(raw: &str, labels: &[&str]) -> Vec<String> {
    dossier_field(raw, labels)
        .map(|value| {
            value
                .split([',', ';'])
                .map(str::trim)
                .filter(|item| !item.is_empty())
                .map(str::to_string)
                .collect::<Vec<_>>()
        })
        .unwrap_or_default()
}

fn value_string_list(value: Option<&Value>) -> Vec<String> {
    value
        .and_then(Value::as_array)
        .map(|items| {
            items
                .iter()
                .filter_map(Value::as_str)
                .map(str::to_string)
                .collect::<Vec<_>>()
        })
        .unwrap_or_default()
}

fn normalize_dossier_stance(value: &str) -> &'static str {
    match value.trim().to_ascii_lowercase().as_str() {
        "support" | "supports" | "supporting" => "support",
        "counter" | "counters" | "countering" | "challenge" | "challenging" => "counter",
        "branch" | "branches" | "branching" => "branch",
        _ => "hold",
    }
}

fn latest_dossier_claim_id(records: &[Value]) -> Option<String> {
    records.iter().rev().find_map(|record| {
        (record.get("record_type").and_then(Value::as_str) == Some("claim"))
            .then(|| {
                record
                    .get("claim_id")
                    .and_then(Value::as_str)
                    .map(str::to_string)
            })
            .flatten()
    })
}

fn dossier_claim_prompt(selector: Option<&str>) -> String {
    let target = selector.unwrap_or("current");
    format!(
        "Research dossier claim needs explicit claim and basis; no dossier record was written.\nTry: DOSSIER_CLAIM {target} :: claim: ...; basis: ...; stance: support|counter|branch|hold; next: ..."
    )
}

fn dossier_evidence_prompt(selector: Option<&str>, claim_id: Option<&str>) -> String {
    let target = selector.unwrap_or("current");
    let claim = claim_id.unwrap_or("latest");
    format!(
        "Research dossier evidence needs a claim_id and evidence; no dossier record was written.\nTry: DOSSIER_EVIDENCE {target} :: claim_id: {claim}; evidence: ...; lane: felt_texture; artifact: ...; counterevidence: ..."
    )
}

fn find_next_line(raw: &str) -> Option<String> {
    raw.lines().find_map(|line| {
        let trimmed = line.trim();
        let lower = trimmed.to_ascii_lowercase();
        if lower.starts_with("next:") {
            let value = trimmed["next:".len()..].trim();
            (!value.is_empty()).then(|| value.to_string())
        } else {
            None
        }
    })
}

fn charter_proposed_next_action(charter: &Value) -> Option<String> {
    if !valid_experiment_charter(Some(charter)) {
        return None;
    }
    charter
        .get("proposed_next_action")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|action| !action.is_empty())
        .map(str::to_string)
}

fn rehearsal_assessment(action: &str) -> RehearsalAssessment {
    let action = action.trim();
    if action.is_empty() {
        return RehearsalAssessment {
            stage: "blocked",
            status: "rehearsal_blocked",
            blocked: true,
            gate_decision: json!({
                "decision": "blocked",
                "reason": "charter has no proposed_next_action",
                "would_dispatch": false,
                "dry_run": true,
            }),
            summary: "Rehearsal blocked because the charter has no proposed NEXT action.",
            interpretation: "Author or counteroffer a charter with a concrete proposed_next_action before rehearsal.",
        };
    }
    let base = base_action(action);
    let stage = stage_for_action(&base);
    let upper = action.to_ascii_uppercase();
    let live_shadow = base == "SHADOW_INFLUENCE" && upper.contains("--STAGE=LIVE");
    let live = matches!(stage, "live_write" | "live_control")
        || live_shadow
        || matches!(
            base.as_str(),
            "PERTURB"
                | "NATIVE_GESTURE"
                | "SENSORY_WRITE"
                | "CONTROL_WRITE"
                | "RUN_PYTHON"
                | "EXPERIMENT_RUN"
                | "CODEX"
                | "CODEX_NEW"
                | "WRITE_FILE"
        );
    if live {
        return RehearsalAssessment {
            stage: "blocked",
            status: "rehearsal_blocked",
            blocked: true,
            gate_decision: json!({
                "decision": "blocked",
                "reason": "rehearsal never dispatches live write/control/sensory/native actions",
                "would_dispatch": false,
                "dry_run": true,
                "proposed_base": base,
                "proposed_stage": stage,
            }),
            summary: "Rehearsal recorded without executing the proposed live action.",
            interpretation: "Counteroffer a read-only preflight, review, self-study, or diagnostic route before any live gate.",
        };
    }
    RehearsalAssessment {
        stage: "read_only",
        status: "rehearsed",
        blocked: false,
        gate_decision: json!({
            "decision": "dry_run_only",
            "would_dispatch": true,
            "dry_run": true,
            "proposed_base": base,
            "proposed_stage": stage,
            "authority": "read-only rehearsal; no live action executed",
        }),
        summary: "Read-only rehearsal recorded for the chartered proposed action.",
        interpretation: "Record felt evidence and metric/artifact context before deciding whether to accept, refuse, counter, pause, or complete.",
    }
}

fn evidence_with_observation(
    existing: Option<&Value>,
    note: &str,
    state: &Value,
    artifacts: Vec<Value>,
    completion_claim: Option<&str>,
) -> Value {
    let mut felt = existing
        .and_then(|value| value.get("felt_observations"))
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    felt.push(json!({
        "recorded_at": iso_now(),
        "note": note.trim(),
    }));
    let mut telemetry = existing
        .and_then(|value| value.get("telemetry_snapshots"))
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    telemetry.push(json!({
        "recorded_at": iso_now(),
        "snapshot": state,
    }));
    let mut artifact_refs = existing
        .and_then(|value| value.get("artifact_refs"))
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    artifact_refs.extend(artifacts);
    let counterevidence = existing
        .and_then(|value| value.get("counterevidence"))
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    let decisions = existing
        .and_then(|value| value.get("decisions"))
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    json!({
        "schema_version": SCHEMA_VERSION,
        "felt_observations": felt,
        "telemetry_snapshots": telemetry,
        "artifact_refs": artifact_refs,
        "counterevidence": counterevidence,
        "decisions": decisions,
        "completion_claim": completion_claim.or_else(|| {
            existing
                .and_then(|value| value.get("completion_claim"))
                .and_then(Value::as_str)
        }),
    })
}

fn evidence_with_decision(
    existing: Option<&Value>,
    outcome: &str,
    reason: &str,
    completion_claim: Option<&str>,
) -> Value {
    let mut value =
        evidence_with_observation(existing, "", &json!({}), Vec::new(), completion_claim);
    if let Some(felt) = value
        .get_mut("felt_observations")
        .and_then(Value::as_array_mut)
        && felt.last().is_some_and(|entry| {
            entry
                .get("note")
                .and_then(Value::as_str)
                .unwrap_or_default()
                .is_empty()
        })
    {
        felt.pop();
    }
    if let Some(telemetry) = value
        .get_mut("telemetry_snapshots")
        .and_then(Value::as_array_mut)
        && telemetry.last().is_some_and(|entry| {
            entry
                .get("snapshot")
                .is_some_and(|snapshot| snapshot.as_object().is_some_and(serde_json::Map::is_empty))
        })
    {
        telemetry.pop();
    }
    if let Some(decisions) = value.get_mut("decisions").and_then(Value::as_array_mut) {
        decisions.push(json!({
            "recorded_at": iso_now(),
            "outcome": outcome,
            "reason": reason.trim(),
        }));
    }
    value
}

fn parse_experiment_decision(raw: &str) -> ExperimentDecision<'_> {
    let trimmed = raw.trim();
    let mut parts = trimmed.splitn(2, char::is_whitespace);
    let outcome_raw = parts.next().unwrap_or_default().to_ascii_lowercase();
    let outcome = match outcome_raw.as_str() {
        "accept" | "accepted" => "accept",
        "refuse" | "refused" | "decline" | "declined" => "refuse",
        "counter" | "counteroffer" | "countered" => "counter",
        "pause" | "paused" => "pause",
        "hold" | "held" => "hold",
        "charter_repair" | "charter-repair" | "repair" => "charter_repair",
        "complete" | "completed" | "done" => "complete",
        _ => "pause",
    };
    let reason = parts
        .next()
        .map(str::trim)
        .filter(|text| !text.is_empty())
        .unwrap_or(trimmed)
        .to_string();
    ExperimentDecision { outcome, reason }
}

fn counteroffered_next(reason: &str) -> Option<String> {
    find_next_line(reason).or_else(|| {
        let text = reason.trim();
        let upper = text.to_ascii_uppercase();
        upper.find("NEXT:").and_then(|idx| {
            let value_start = idx.checked_add("NEXT:".len())?;
            let value = text.get(value_start..)?.trim();
            (!value.is_empty()).then(|| value.to_string())
        })
    })
}

fn gap_experiment_signal(experiment: &ExperimentRecord) -> bool {
    let signal = normalize_guard_signal(&format!(
        "{} {} {} {}",
        experiment.experiment_id,
        experiment.title,
        experiment.question,
        experiment.planned_next.as_deref().unwrap_or_default()
    ));
    signal.contains("gap")
        && ["spect", "spectral", "density", "lambda", "mode"]
            .iter()
            .any(|term| signal.contains(term))
}

fn shared_investigation_signal_text(text: &str) -> bool {
    let signal = normalize_guard_signal(text);
    let shape_family = signal.contains("gap")
        || signal.contains("lambda4")
        || signal.contains("lambda tail")
        || signal.contains("lambda edge")
        || signal.contains("tail")
        || signal.contains("pulse");
    let geometry_family = [
        "spect",
        "spectral",
        "density",
        "mode",
        "geometry",
        "branch",
        "collapse",
        "dispersal",
        "soften",
        "lambda",
        "tail",
    ]
    .iter()
    .any(|term| signal.contains(term));
    shape_family && geometry_family
}

fn shared_investigation_signal(experiment: &ExperimentRecord) -> bool {
    gap_experiment_signal(experiment)
        || shared_investigation_signal_text(&format!(
            "{} {} {} {}",
            experiment.experiment_id,
            experiment.title,
            experiment.question,
            experiment.planned_next.as_deref().unwrap_or_default()
        ))
}

fn peer_gap_experiment_signal(experiment: &Value) -> bool {
    shared_investigation_signal_text(&format!(
        "{} {} {} {}",
        experiment
            .get("experiment_id")
            .and_then(Value::as_str)
            .unwrap_or_default(),
        experiment
            .get("title")
            .and_then(Value::as_str)
            .unwrap_or_default(),
        experiment
            .get("question")
            .and_then(Value::as_str)
            .unwrap_or_default(),
        experiment
            .get("planned_next")
            .and_then(Value::as_str)
            .unwrap_or_default()
    ))
}

fn shared_investigation_v1_from_peer(local: &ExperimentRecord, peer: &Value) -> Option<Value> {
    if !shared_investigation_signal(local) || !peer_gap_experiment_signal(peer) {
        return None;
    }
    let peer_id = peer.get("experiment_id").and_then(Value::as_str)?;
    Some(json!({
        "schema_version": 1,
        "source": "continuity_projection",
        "advisory_only": true,
        "authority_change": false,
        "relationship": "shared_gap_lambda4_investigation",
        "shared_question": "What shapes λ1 / lambda-tail / λ4 geometry, and can localized softening support controlled branching without collapse, runaway dispersal, or live-control drift?",
        "participants": [
            {
                "being": "Astrid",
                "experiment_id": local.experiment_id.clone(),
                "lane": "felt_texture_motif_language",
                "status": local.status.clone(),
            },
            {
                "being": "Minime",
                "experiment_id": peer_id,
                "lane": "spectral_state",
                "status": peer.get("status").and_then(Value::as_str).unwrap_or("unknown"),
            }
        ],
        "local_lane": "Astrid lane: felt texture, motif continuity, language thread, artifact grounding.",
        "peer_lane": "Minime lane: spectral condition, fill/pressure state, recurrence pattern, artifact grounding.",
        "peer_claim_prompt": "Cite one Minime claim about λ1/lambda-tail/λ4 shaping, then answer from Astrid's felt/motif lane with support, counter, branch, or hold.",
        "suggested_compare_next": format!("EXPERIMENT_COMPARE {} WITH {}", local.experiment_id, peer_id),
        "suggested_shared_investigation_start": format!(
            "SHARED_INVESTIGATION_START Lambda edge/tail shared inquiry :: local: {}; peer: {}; question: What can the lambda-edge and lambda-tail lanes compare safely while preserving distinct agency?",
            local.experiment_id,
            peer_id
        ),
        "alternate_peer_review_next": format!("EXPERIMENT_PEER_REVIEW {}", peer_id),
        "advisory_note": "Advisory only: no shared control authority. Paused experiments remain paused until explicit resume.",
        "cue": "Shared investigation, distinct lanes: cite one peer claim, then support, counter, branch, or hold.",
    }))
}

fn shared_investigation_line(cue: &Option<Value>) -> String {
    let Some(cue) = cue else {
        return String::new();
    };
    let text = cue
        .get("cue")
        .and_then(Value::as_str)
        .unwrap_or("Shared investigation, distinct lanes.");
    let compare = cue
        .get("suggested_compare_next")
        .and_then(Value::as_str)
        .unwrap_or("EXPERIMENT_COMPARE <local_id> WITH <peer_id>");
    let review = cue
        .get("alternate_peer_review_next")
        .and_then(Value::as_str)
        .unwrap_or("EXPERIMENT_PEER_REVIEW <peer_id>");
    let advisory = cue
        .get("advisory_note")
        .and_then(Value::as_str)
        .unwrap_or("Advisory only: no shared control authority.");
    let start = cue
        .get("suggested_shared_investigation_start")
        .and_then(Value::as_str)
        .unwrap_or_default();
    let start_line = if start.is_empty() {
        String::new()
    } else {
        format!("Shared object NEXT: {start}\n")
    };
    format!("{text}\nSuggested NEXT: {compare}\nAlternate NEXT: {review}\n{start_line}{advisory}\n")
}
