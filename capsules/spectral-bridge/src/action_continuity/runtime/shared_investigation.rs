fn suppress_shared_start_if_object(
    mut cue: Option<Value>,
    investigation: &Option<Value>,
) -> Option<Value> {
    if investigation.is_some()
        && let Some(Value::Object(map)) = cue.as_mut()
    {
        map.remove("suggested_shared_investigation_start");
    }
    cue
}

fn shared_investigation_object_line(investigation: &Option<Value>) -> String {
    let Some(investigation) = investigation else {
        return String::new();
    };
    let id = investigation
        .get("id")
        .and_then(Value::as_str)
        .unwrap_or("unknown");
    let status = investigation
        .get("status")
        .and_then(Value::as_str)
        .unwrap_or("active");
    let title = investigation
        .get("title")
        .and_then(Value::as_str)
        .unwrap_or("Shared investigation");
    let participants = investigation
        .get("participants")
        .and_then(Value::as_array)
        .map(|rows| {
            rows.iter()
                .filter_map(|row| {
                    let being = row.get("being").and_then(Value::as_str)?;
                    let experiment_id = row
                        .get("experiment_id")
                        .and_then(Value::as_str)
                        .unwrap_or("unlinked");
                    Some(format!("{being}:{experiment_id}"))
                })
                .collect::<Vec<_>>()
                .join(" <-> ")
        })
        .filter(|text| !text.is_empty())
        .unwrap_or_else(|| "participants pending".to_string());
    format!("Shared investigation object: {id} [{status}] {title} :: {participants}\n")
}

fn dossier_field_text(value: &str, max_len: usize) -> String {
    compact_text(value, max_len)
        .replace(['\n', '\r', '\t'], " ")
        .replace(';', ",")
        .trim()
        .trim_matches('`')
        .trim()
        .to_string()
}

fn first_dossier_claim_cue_v1(
    _thread: &ResearchThread,
    experiment: &ExperimentRecord,
    dossier: Option<&Value>,
    prior_claim_bridge: &Option<Value>,
    lifecycle_priority_experiment_id: Option<&str>,
) -> Option<Value> {
    if !shared_investigation_signal(experiment) {
        return None;
    }
    let claim_count = dossier
        .and_then(|summary| summary.get("claim_count"))
        .and_then(Value::as_u64)
        .unwrap_or(0);
    if claim_count > 0 {
        return None;
    }
    let prior_claim = prior_claim_bridge
        .as_ref()
        .and_then(|cue| cue.get("prior_claim"))
        .and_then(Value::as_str)
        .map(|value| dossier_field_text(value, 180))
        .filter(|value| !value.is_empty())
        .unwrap_or_else(|| "...".to_string());
    let basis = prior_claim_bridge
        .as_ref()
        .and_then(|cue| cue.get("delta"))
        .and_then(Value::as_str)
        .map(|value| dossier_field_text(value, 180))
        .filter(|value| !value.is_empty())
        .unwrap_or_else(|| "...".to_string());
    let mut next = prior_claim_bridge
        .as_ref()
        .and_then(|cue| cue.get("priority_next"))
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .unwrap_or("EXPERIMENT_COMPARE <local_id> WITH <peer_id>")
        .to_string();
    let lifecycle_id = lifecycle_priority_experiment_id
        .map(str::trim)
        .filter(|value| !value.is_empty());
    let lifecycle_scope = if let Some(lifecycle_id) = lifecycle_id
        && lifecycle_id != experiment.experiment_id
        && next.starts_with("EXPERIMENT_CHARTER current")
    {
        next = next.replacen(
            "EXPERIMENT_CHARTER current",
            &format!("EXPERIMENT_CHARTER {lifecycle_id}"),
            1,
        );
        Some("active_experiment")
    } else {
        None
    };
    let stance = "hold";
    let command = format!(
        "DOSSIER_CLAIM {} :: claim: {}; basis: {}; stance: {stance}; next: {next}",
        experiment.experiment_id, prior_claim, basis
    );
    Some(json!({
        "schema_version": 1,
        "source": "continuity_projection",
        "advisory_only": true,
        "authority_change": false,
        "status": "missing_first_dossier_claim",
        "target_experiment_id": experiment.experiment_id.clone(),
        "dossier_target_experiment_id": experiment.experiment_id.clone(),
        "lifecycle_priority_experiment_id": lifecycle_id,
        "lifecycle_priority_scope": lifecycle_scope,
        "claim_count": claim_count,
        "stance": stance,
        "prior_claim": if prior_claim == "..." { Value::Null } else { json!(prior_claim) },
        "delta": if basis == "..." { Value::Null } else { json!(basis) },
        "suggested_claim_next": command,
        "cue": "Shared investigation has no local claim yet; capture one claim, then answer one peer claim with support/counter/branch/hold.",
    }))
}

fn first_dossier_claim_line(cue: &Option<Value>) -> String {
    let Some(cue) = cue else {
        return String::new();
    };
    let text = cue
        .get("cue")
        .and_then(Value::as_str)
        .unwrap_or("Shared investigation has no local claim yet.");
    let next = cue
        .get("suggested_claim_next")
        .and_then(Value::as_str)
        .unwrap_or("DOSSIER_CLAIM <id> :: claim: ...; basis: ...; stance: support|counter|branch|hold; next: ...");
    let dossier_target = cue
        .get("dossier_target_experiment_id")
        .and_then(Value::as_str);
    let lifecycle_target = cue
        .get("lifecycle_priority_experiment_id")
        .and_then(Value::as_str);
    let clarification = match (dossier_target, lifecycle_target) {
        (Some(dossier_target), Some(lifecycle_target)) if dossier_target != lifecycle_target => {
            format!(
                " Dossier target is `{dossier_target}`; charter priority is active experiment `{lifecycle_target}`. Dossier capture is referable context only."
            )
        },
        _ => String::new(),
    };
    format!("{text}{clarification} Dossier NEXT: {next}\n")
}

fn research_dossier_line(summary: &Option<Value>, lifecycle: Option<&str>) -> String {
    let Some(summary) = summary else {
        return String::new();
    };
    let claim_count = summary
        .get("claim_count")
        .and_then(Value::as_u64)
        .unwrap_or(0);
    let evidence_count = summary
        .get("evidence_count")
        .and_then(Value::as_u64)
        .unwrap_or(0);
    let claim_next = summary
        .get("suggested_claim_next")
        .and_then(Value::as_str)
        .unwrap_or("DOSSIER_CLAIM current :: claim: ...; basis: ...");
    let evidence_next = summary
        .get("suggested_evidence_next")
        .and_then(Value::as_str)
        .unwrap_or("DOSSIER_EVIDENCE current :: claim_id: latest; evidence: ...");
    let priority = match lifecycle {
        Some("needs_charter") => {
            "Dossier capture is context; charter remains the lifecycle priority."
        },
        Some("needs_evidence") => {
            "Dossier evidence is referable context; EXPERIMENT_EVIDENCE remains lifecycle evidence."
        },
        Some("needs_decision") => {
            "Dossier capture is research memory; EXPERIMENT_DECIDE remains the lifecycle priority."
        },
        _ => "Dossier records are research context only.",
    };
    format!(
        "Research dossier: claims={claim_count} evidence={evidence_count}. {priority}\nDossier NEXT: {claim_next}\nDossier evidence NEXT: {evidence_next}\n"
    )
}

fn peer_mutation_boundary_line(cue: &Option<Value>) -> String {
    let Some(cue) = cue else {
        return String::new();
    };
    let text = cue
        .get("cue")
        .and_then(Value::as_str)
        .unwrap_or("Peer experiments are compare/review/dossier targets, not bind/mutate targets.");
    let compare = cue
        .get("suggested_compare_next")
        .and_then(Value::as_str)
        .unwrap_or("EXPERIMENT_COMPARE <local_id> WITH <peer_id>");
    let review = cue
        .get("suggested_peer_review_next")
        .and_then(Value::as_str)
        .unwrap_or("EXPERIMENT_PEER_REVIEW <peer_id>");
    let dossier = cue
        .get("suggested_dossier_next")
        .and_then(Value::as_str)
        .unwrap_or(
            "DOSSIER_CLAIM <local_id> :: claim: ...; basis: ...; stance: support|counter|branch|hold; next: ...",
        );
    format!("Peer mutation boundary: {text} Suggested routes: {compare} | {review} | {dossier}\n")
}

fn peer_mutation_boundary_cue(
    thread: &ResearchThread,
    active: Option<&ExperimentContinuityProjection>,
    recent_events: &[ActionEvent],
) -> Option<Value> {
    let mut matches = Vec::<Value>::new();
    if let Some(current) = thread.current_next.as_deref()
        && let Some((verb, peer_id)) = peer_mutation_boundary_match(current)
    {
        matches.push(json!({
            "source": "current_next",
            "action": current,
            "verb": verb,
            "peer_experiment_id": peer_id,
        }));
    }
    for event in recent_events.iter().rev().take(8) {
        for candidate in [
            event.raw_next.as_deref(),
            Some(event.canonical_action.as_str()),
            Some(event.effective_action.as_str()),
            event.suggested_next.as_deref(),
        ]
        .into_iter()
        .flatten()
        {
            if let Some((verb, peer_id)) = peer_mutation_boundary_match(candidate) {
                matches.push(json!({
                    "source": "recent_event",
                    "action": candidate,
                    "verb": verb,
                    "peer_experiment_id": peer_id,
                    "action_id": event.action_id,
                    "status": event.status,
                }));
                break;
            }
        }
    }
    let first = matches.first()?;
    let peer_id = first
        .get("peer_experiment_id")
        .and_then(Value::as_str)
        .unwrap_or("<peer_id>");
    let local_id = active
        .map(|projection| projection.experiment.experiment_id.as_str())
        .or(thread.active_experiment_id.as_deref())
        .unwrap_or("<local_id>");
    Some(json!({
        "schema_version": 1,
        "source": "continuity_projection",
        "advisory_only": true,
        "authority_change": false,
        "status": "peer_mutation_boundary",
        "cue": "Peer experiments are compare/review/dossier targets, not bind/mutate targets.",
        "peer_experiment_id": peer_id,
        "local_experiment_id": local_id,
        "matched_actions": matches,
        "suggested_compare_next": format!("EXPERIMENT_COMPARE {local_id} WITH {peer_id}"),
        "suggested_peer_review_next": format!("EXPERIMENT_PEER_REVIEW {peer_id}"),
        "suggested_dossier_next": format!("DOSSIER_CLAIM {local_id} :: claim: ...; basis: ...; stance: support|counter|branch|hold; next: ..."),
    }))
}

fn peer_mutation_boundary_match(action: &str) -> Option<(String, String)> {
    let text = action.trim();
    if text.is_empty() {
        return None;
    }
    let upper = text.to_ascii_uppercase();
    const MUTATION_VERBS: [&str; 9] = [
        "EXPERIMENT_BIND",
        "EXPERIMENT_CHARTER",
        "EXPERIMENT_REHEARSE",
        "EXPERIMENT_PREFLIGHT",
        "EXPERIMENT_EVIDENCE",
        "EXPERIMENT_DECIDE",
        "EXPERIMENT_CLOSE",
        "EXPERIMENT_RESUME",
        "EXPERIMENT_OBSERVE",
    ];
    let verb = MUTATION_VERBS
        .iter()
        .find(|verb| upper.contains(**verb))
        .map(|verb| (*verb).to_string())?;
    let peer_id = text
        .split(|ch: char| {
            ch.is_whitespace()
                || matches!(
                    ch,
                    ':' | ';' | ',' | ')' | '(' | '[' | ']' | '{' | '}' | '`' | '"' | '\''
                )
        })
        .find_map(|token| {
            let normalized = normalize_experiment_selector(token);
            if normalized.starts_with(PEER_EXPERIMENT_PREFIX) {
                Some(normalized)
            } else {
                None
            }
        })?;
    Some((verb, peer_id))
}

fn shared_investigation_response_contract(cue: &Option<Value>) -> String {
    let Some(cue) = cue else {
        return String::new();
    };
    let peer_claim = cue
        .get("peer_claim_prompt")
        .and_then(Value::as_str)
        .unwrap_or("Cite one peer claim, then answer from the local evidence lane.");
    let local_lane = cue
        .get("local_lane")
        .and_then(Value::as_str)
        .unwrap_or("Local lane: native evidence.");
    let advisory = cue
        .get("advisory_note")
        .and_then(Value::as_str)
        .unwrap_or("Advisory only: no shared control authority.");
    format!(
        "Shared investigation response contract:\n- Peer claim to answer: {peer_claim}\n- Local evidence lane: {local_lane}\n- Allowed stances: support, counter, branch, hold.\n- Optional dossier capture: DOSSIER_CLAIM <local_id> :: claim: ...; basis: ...; stance: support|counter|branch|hold; next: ... or DOSSIER_EVIDENCE <local_id> :: claim_id: latest; evidence: ...\n- {advisory}\n"
    )
}

fn preferred_charter_scaffold_next(
    experiment: &ExperimentRecord,
    recent_runs: &[ExperimentRunRecord],
) -> String {
    if gap_experiment_signal(experiment) {
        return "ACTION_PREFLIGHT DECOMPOSE".to_string();
    }
    if let Some(planned) = experiment.planned_next.as_deref()
        && let Some(counter) = counteroffered_next(planned)
    {
        return counter;
    }
    if let Some(proposed) = experiment
        .workbench_candidates_v1
        .as_ref()
        .and_then(|value| value.get("charter"))
        .and_then(|value| value.get("proposed_next_action"))
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|text| !text.is_empty())
    {
        return proposed.to_string();
    }
    for run in recent_runs.iter().rev() {
        let action = run.action_text.trim();
        if action.is_empty() {
            continue;
        }
        if !matches!(
            base_action(action).as_str(),
            "BROWSE" | "SEARCH" | "READ_MORE" | "LOOK" | "EXPERIMENT_REVIEW" | "EXPERIMENT_STATUS"
        ) {
            return action.to_string();
        }
    }
    "ACTION_PREFLIGHT DECOMPOSE".to_string()
}

fn sanitize_title_for_hypothesis(title: &str) -> String {
    let stripped = title
        .chars()
        .map(|ch| match ch {
            '`' | '*' | '_' | '#' | '[' | ']' => ' ',
            _ => ch,
        })
        .collect::<String>();
    let collapsed = stripped.split_whitespace().collect::<Vec<_>>().join(" ");
    let trimmed = collapsed
        .trim_matches(|ch: char| {
            ch.is_whitespace()
                || matches!(
                    ch,
                    '-' | '–' | '—' | ':' | ';' | ',' | '.' | '!' | '?' | '"' | '\''
                )
        })
        .trim();
    if trimmed.is_empty() {
        "this experiment".to_string()
    } else {
        trimmed.to_string()
    }
}

fn charter_scaffold_v1(
    thread: &ResearchThread,
    experiment: &ExperimentRecord,
    recent_runs: &[ExperimentRunRecord],
    classification: &str,
) -> Option<Value> {
    if !charter_repair_bound(classification, experiment) {
        return None;
    }
    let proposed_next = preferred_charter_scaffold_next(experiment, recent_runs);
    let gap = gap_experiment_signal(experiment);
    let hypothesis = if gap {
        "localized lambda-tail/λ4 pressure may become returnable by softening the dominant channel while preserving motif continuity and artifact grounding"
            .to_string()
    } else {
        let clean_title = sanitize_title_for_hypothesis(&experiment.title);
        format!(
            "{} may become returnable by naming felt texture, motif continuity, language thread, and artifact grounding without adding live authority",
            clean_title
        )
    };
    let method_intent = if gap {
        format!(
            "rehearse {proposed_next} and compare felt pressure, motif recurrence, language continuity, and artifact evidence before deciding"
        )
    } else {
        format!(
            "rehearse {proposed_next} and compare felt texture, motif recurrence, language continuity, and artifact evidence before deciding"
        )
    };
    let stop_criteria = if gap {
        "pressure risk rises above baseline, λ4/entropy shows runaway dispersal, artifact grounding stays missing after repeated passes, or the route feels heavy"
    } else {
        "pressure risk rises above baseline, artifact grounding stays missing after repeated passes, or the route feels heavy"
    };
    let command = format!(
        "EXPERIMENT_CHARTER current :: hypothesis: {hypothesis}; method_intent: {method_intent}; proposed_next_action: {proposed_next}; evidence_targets: felt_texture, motif_continuity, language_thread, artifact_grounding; stop_criteria: {stop_criteria}; consent_posture: advisory; ordinary choices remain valid."
    );
    Some(json!({
        "schema_version": 1,
        "source": "continuity_projection",
        "status": "scaffold_only",
        "authoring_required": true,
        "authority_change": false,
        "command": command,
        "proposed_next_action": proposed_next,
        "evidence_targets": [
            "felt_texture",
            "motif_continuity",
            "language_thread",
            "artifact_grounding"
        ],
        "native_register": "astrid_motif_language",
        "thread_id": &thread.thread_id,
        "experiment_id": &experiment.experiment_id,
    }))
}

fn charter_repair_bound(classification: &str, experiment: &ExperimentRecord) -> bool {
    classification == "needs_charter"
        || (classification == "blocked_loop"
            && !valid_experiment_charter(experiment.charter_v1.as_ref()))
}

fn charter_status_text(experiment: &ExperimentRecord) -> String {
    let Some(charter) = experiment.charter_v1.as_ref() else {
        return "Workbench charter: missing. Use EXPERIMENT_CHARTER current :: hypothesis: ...; proposed_next_action: ...".to_string();
    };
    if !valid_experiment_charter(Some(charter)) {
        return "Workbench charter: missing/empty. Use EXPERIMENT_CHARTER current :: hypothesis: ...; proposed_next_action: ...".to_string();
    }
    let proposed = charter
        .get("proposed_next_action")
        .and_then(Value::as_str)
        .filter(|text| !text.trim().is_empty())
        .unwrap_or("(missing proposed_next_action)");
    let targets = charter
        .get("evidence_targets")
        .and_then(Value::as_array)
        .map_or(0, Vec::len);
    format!(
        "Workbench charter: present proposed_next_action=`{proposed}` evidence_targets={targets}"
    )
}

#[allow(dead_code)]
fn summary_charter_status_text(summary: &Value) -> String {
    if !valid_experiment_charter(summary.get("charter_v1")) {
        return "Workbench charter: missing/empty. Use EXPERIMENT_CHARTER current :: hypothesis: ...; proposed_next_action: ...".to_string();
    }
    summary
        .get("workbench_charter")
        .and_then(Value::as_str)
        .unwrap_or("Workbench charter: present")
        .to_string()
}

#[allow(dead_code)]
fn summary_evidence_status_text(summary: &Value) -> String {
    summary
        .get("workbench_evidence")
        .and_then(Value::as_str)
        .unwrap_or("Workbench evidence: thin felt=0 telemetry=0 artifacts=0")
        .to_string()
}

fn evidence_status_text(experiment: &ExperimentRecord) -> String {
    let felt = experiment
        .evidence_v1
        .as_ref()
        .and_then(|value| value.get("felt_observations"))
        .and_then(Value::as_array)
        .map_or(0, Vec::len);
    let telemetry = experiment
        .evidence_v1
        .as_ref()
        .and_then(|value| value.get("telemetry_snapshots"))
        .and_then(Value::as_array)
        .map_or(0, Vec::len);
    let artifacts = experiment
        .evidence_v1
        .as_ref()
        .and_then(|value| value.get("artifact_refs"))
        .and_then(Value::as_array)
        .map_or(0, Vec::len);
    let strength = if felt > 0 && (telemetry > 0 || artifacts > 0) {
        "stronger"
    } else {
        "thin"
    };
    format!(
        "Workbench evidence: {strength} felt={felt} telemetry={telemetry} artifacts={artifacts}"
    )
}

fn workbench_candidate_status(experiment: &ExperimentRecord) -> String {
    let Some(candidates) = experiment
        .workbench_candidates_v1
        .as_ref()
        .and_then(Value::as_object)
    else {
        return String::new();
    };
    let mut lines = Vec::new();
    for (key, label) in [("charter", "Draft charter"), ("evidence", "Draft evidence")] {
        let Some(candidate) = candidates.get(key).and_then(Value::as_object) else {
            continue;
        };
        if candidate.get("status").and_then(Value::as_str) != Some("candidate") {
            continue;
        }
        let Some(command) = candidate
            .get("command")
            .and_then(Value::as_str)
            .filter(|value| !value.trim().is_empty())
        else {
            continue;
        };
        lines.push(format!("- {label}: {command}"));
    }
    if lines.is_empty() {
        String::new()
    } else {
        format!("Workbench draft candidates:\n{}", lines.join("\n"))
    }
}

fn candidate_action_seed(run: Option<&ExperimentRunRecord>, focus_text: Option<&str>) -> String {
    run.map(|record| record.action_text.trim())
        .filter(|text| !text.is_empty())
        .map(str::to_string)
        .or_else(|| {
            focus_text
                .map(str::trim)
                .filter(|text| !text.is_empty())
                .map(|text| format!("ACTION_PREFLIGHT {text}"))
        })
        .unwrap_or_else(|| "EXPERIMENT_REHEARSE current".to_string())
}

fn candidate_state_text(state: &Value) -> String {
    let fill = state.get("fill_pct").and_then(Value::as_f64).or_else(|| {
        state
            .get("fill_ratio")
            .and_then(Value::as_f64)
            .map(|value| value * 100.0)
    });
    let eig1 = state.get("eig1").and_then(Value::as_f64);
    let cov = state.get("cov_lambda1").and_then(Value::as_f64);
    let mut parts = Vec::new();
    if let Some(value) = fill {
        parts.push(format!("fill {value:.1}%"));
    }
    if let Some(value) = eig1 {
        parts.push(format!("eig1 {value:.3}"));
    }
    if let Some(value) = cov {
        parts.push(format!("cov_lambda1 {value:.3}"));
    }
    if parts.is_empty() {
        "telemetry: unavailable".to_string()
    } else {
        parts.join(", ")
    }
}

fn compact_text(value: &str, limit: usize) -> String {
    let text = value.split_whitespace().collect::<Vec<_>>().join(" ");
    if text.len() <= limit {
        text
    } else {
        let truncated = text.chars().take(limit).collect::<String>();
        format!("{truncated}...")
    }
}

fn event_choice_summary(event: &ActionEvent) -> Option<String> {
    let envelope = event.choice_envelope_v1.as_ref()?;
    let alternate_count = envelope
        .get("alternate_nexts")
        .and_then(Value::as_array)
        .map_or(0, Vec::len);
    let return_count = envelope
        .get("return_threads")
        .and_then(Value::as_array)
        .map_or(0, Vec::len);
    let residue_present = envelope
        .get("residue")
        .and_then(Value::as_str)
        .is_some_and(|value| !value.trim().is_empty());
    let mismatch_present = envelope.get("mismatch_warning").is_some();
    if alternate_count == 0 && return_count == 0 && !residue_present && !mismatch_present {
        return None;
    }
    let mut parts = vec![format!(
        "choice alt={alternate_count} return={return_count}"
    )];
    if residue_present {
        parts.push("residue=yes".to_string());
    }
    if mismatch_present {
        parts.push("primary_mismatch=yes".to_string());
    }
    Some(parts.join(" "))
}
