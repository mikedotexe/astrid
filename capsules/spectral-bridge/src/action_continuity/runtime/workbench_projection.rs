fn choice_envelope_value(
    response_text: &str,
    raw_next: &str,
    canonical_next: &str,
    effective_next: &str,
) -> Option<Value> {
    let mut primary_next: Option<String> = None;
    let mut alternate_nexts = Vec::new();
    let mut return_threads = Vec::new();
    let mut residue: Option<String> = final_next_residue(response_text);
    let mut why_this_path: Option<String> = None;
    let mut defer_reason: Option<String> = None;

    for line in choice_metadata_lines(response_text) {
        if let Some(value) = label_value(
            line,
            &[
                "Primary NEXT:",
                "Primary path:",
                "Chosen NEXT:",
                "Chosen path:",
            ],
        ) {
            primary_next = Some(compact_text(strip_next_prefix(value), 240));
        } else if let Some(value) = label_value(
            line,
            &[
                "Alternate NEXT:",
                "Alternative NEXT:",
                "Alternate path:",
                "Alternative path:",
            ],
        ) {
            alternate_nexts.push(compact_text(strip_next_prefix(value), 240));
        } else if let Some(value) =
            label_value(line, &["Return thread:", "Return threads:", "Return to:"])
        {
            return_threads.push(compact_text(value, 240));
        } else if let Some(value) =
            label_value(line, &["Residue:", "Transition residue:", "Stickiness:"])
        {
            residue = Some(compact_text(value, 240));
        } else if let Some(value) =
            label_value(line, &["Why this path:", "Why this NEXT:", "Why now:"])
        {
            why_this_path = Some(compact_text(value, 360));
        } else if let Some(value) = label_value(
            line,
            &["Defer reason:", "Deferred because:", "Deferring because:"],
        ) {
            defer_reason = Some(compact_text(value, 360));
        }
    }

    let has_metadata = primary_next.is_some()
        || !alternate_nexts.is_empty()
        || !return_threads.is_empty()
        || residue.is_some()
        || why_this_path.is_some()
        || defer_reason.is_some();
    if !has_metadata {
        return None;
    }

    let executable_next = canonical_next.trim();
    let declared_primary = primary_next.unwrap_or_else(|| executable_next.to_string());
    let declared_canonical = crate::autonomous::canonicalize_next_action_text(&declared_primary);
    let mismatch_warning = if declared_canonical.trim() != executable_next {
        Some(format!(
            "primary_next `{}` canonicalized to `{}` but executable NEXT was `{}`; dispatch followed executable NEXT",
            compact_text(&declared_primary, 120),
            compact_text(&declared_canonical, 120),
            compact_text(executable_next, 120)
        ))
    } else {
        None
    };

    let mut object = serde_json::Map::new();
    object.insert("policy".to_string(), json!("choice_envelope_v1"));
    object.insert("schema_version".to_string(), json!(1));
    object.insert("source".to_string(), json!("astrid_next_response"));
    object.insert(
        "authority".to_string(),
        json!("diagnostic_context_not_command"),
    );
    object.insert("primary_next".to_string(), json!(declared_primary));
    object.insert("executable_next".to_string(), json!(executable_next));
    object.insert("effective_next".to_string(), json!(effective_next));
    object.insert("raw_next".to_string(), json!(raw_next));
    object.insert("alternate_nexts".to_string(), json!(alternate_nexts));
    object.insert("return_threads".to_string(), json!(return_threads));
    if let Some(value) = residue {
        object.insert("residue".to_string(), json!(value));
    }
    if let Some(value) = why_this_path {
        object.insert("why_this_path".to_string(), json!(value));
    }
    if let Some(value) = defer_reason {
        object.insert("defer_reason".to_string(), json!(value));
    }
    if let Some(value) = mismatch_warning {
        object.insert("mismatch_warning".to_string(), json!(value));
    }
    Some(Value::Object(object))
}

fn transition_residue_value(
    choice_envelope_v1: Option<&Value>,
    canonical_next: &str,
    effective_next: &str,
    telemetry: &SpectralTelemetry,
) -> Option<Value> {
    let residue = choice_envelope_v1
        .and_then(|value| value.get("residue"))
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())?;
    let resonance = telemetry.resonance_density_v1.as_ref();
    let pressure = telemetry.pressure_source_v1.as_ref();
    Some(json!({
        "policy": "transition_residue_v1",
        "schema_version": 1,
        "source": "choice_envelope_v1",
        "authority": "diagnostic_context_not_command",
        "residue_text": residue,
        "canonical_action": canonical_next,
        "effective_action": effective_next,
        "telemetry": {
            "fill_ratio": telemetry.fill_ratio,
            "density_gradient": crate::codec::spectral_density_gradient(&telemetry.eigenvalues),
            "pressure_risk": resonance.map(|metric| metric.pressure_risk),
            "resonance_mode_packing": resonance.map(|metric| metric.components.mode_packing),
            "pressure_score": pressure.map(|metric| metric.pressure_score),
            "porosity_score": pressure.map(|metric| metric.porosity_score),
            "pressure_mode_packing": pressure.map(|metric| metric.components.mode_packing),
        },
    }))
}

fn choice_metadata_lines(text: &str) -> Vec<&str> {
    let mut in_fence = false;
    let mut out = Vec::new();
    for line in text.lines() {
        let trimmed = line.trim();
        if trimmed.starts_with("```") {
            in_fence = !in_fence;
            continue;
        }
        if !in_fence {
            out.push(line);
        }
    }
    out
}

fn label_value<'a>(line: &'a str, labels: &[&str]) -> Option<&'a str> {
    let trimmed = line
        .trim()
        .trim_start_matches(['-', '*', '>', '•'])
        .trim_start();
    let lowered = trimmed.to_ascii_lowercase();
    for label in labels {
        let label_lower = label.to_ascii_lowercase();
        if lowered.starts_with(&label_lower) {
            return Some(trimmed[label.len()..].trim());
        }
    }
    None
}

fn strip_next_prefix(value: &str) -> &str {
    let trimmed = value.trim();
    if trimmed
        .get(..5)
        .is_some_and(|prefix| prefix.eq_ignore_ascii_case("NEXT:"))
    {
        trimmed[5..].trim()
    } else {
        trimmed
    }
}

fn final_next_residue(text: &str) -> Option<String> {
    let mut in_fence = false;
    for line in text.lines().rev() {
        let trimmed = line.trim();
        if trimmed.starts_with("```") {
            in_fence = !in_fence;
            continue;
        }
        if in_fence {
            continue;
        }
        if let Some(action) = trimmed.strip_prefix("NEXT:") {
            return crate::autonomous::extract_residue_from_next_action(action)
                .map(|value| compact_text(value, 240));
        }
    }
    None
}

fn build_workbench_candidates(
    experiment: &ExperimentRecord,
    run: Option<&ExperimentRunRecord>,
    focus_text: Option<&str>,
    source: &str,
) -> Value {
    let now = iso_now();
    let action_text = candidate_action_seed(run, focus_text);
    let focus = focus_text.unwrap_or_default().trim();
    let context = compact_text(
        if !focus.is_empty() {
            focus
        } else {
            run.map_or(experiment.question.as_str(), |record| {
                if record.result_summary.trim().is_empty() {
                    experiment.question.as_str()
                } else {
                    record.result_summary.as_str()
                }
            })
        },
        180,
    );
    let state = run.map_or(&Value::Null, |record| {
        if record.post_state.is_null() {
            &record.pre_state
        } else {
            &record.post_state
        }
    });
    let telemetry = candidate_state_text(state);
    let artifact_refs = run
        .map(|record| {
            record
                .artifacts
                .iter()
                .filter_map(|artifact| {
                    if !artifact.artifact_id.is_empty() {
                        Some(artifact.artifact_id.clone())
                    } else if !artifact.path_or_uri.is_empty() {
                        Some(artifact.path_or_uri.clone())
                    } else {
                        None
                    }
                })
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    let artifact_text = if artifact_refs.is_empty() {
        run.map_or_else(|| "latest run".to_string(), |record| record.run_id.clone())
    } else {
        compact_text(&artifact_refs.join(", "), 160)
    };
    let charter_command = format!(
        "EXPERIMENT_CHARTER current :: hypothesis: {} can become clearer through `{}`; method_intent: rehearse or return through {} without adding live authority; proposed_next_action: {}; evidence_targets: felt, telemetry, artifact; stop_criteria: pressure spike, unstable fill, or the route feels heavy; consent_posture: advisory; ordinary choices remain valid.",
        experiment.title, action_text, context, action_text
    );
    let evidence_command = format!(
        "EXPERIMENT_EVIDENCE current :: felt: what changed after `{}`; telemetry: {}; artifact: {}; counterevidence: note anything that resisted the hypothesis.",
        action_text, telemetry, artifact_text
    );
    json!({
        "schema_version": 1,
        "updated_at": now.clone(),
        "source": source,
        "charter": {
            "candidate_id": format!("cand_{}_charter", experiment.experiment_id),
            "status": "candidate",
            "generated_at": now.clone(),
            "source": source,
            "source_run_id": run.map(|record| record.run_id.clone()),
            "focus_text": if focus.is_empty() { Value::Null } else { json!(focus) },
            "hypothesis": format!("{} can become clearer through `{}`.", experiment.title, action_text),
            "method_intent": format!("Rehearse or return through {} without adding live authority.", context),
            "proposed_next_action": action_text,
            "evidence_targets": ["felt", "telemetry", "artifact"],
            "stop_criteria": "pressure spike, unstable fill, or the route feels heavy",
            "consent_posture": "advisory; ordinary choices remain valid",
            "command": charter_command,
        },
        "evidence": {
            "candidate_id": format!("cand_{}_evidence", experiment.experiment_id),
            "status": "candidate",
            "generated_at": now,
            "source": source,
            "source_run_id": run.map(|record| record.run_id.clone()),
            "focus_text": if focus.is_empty() { Value::Null } else { json!(focus) },
            "telemetry": telemetry,
            "artifact_refs": artifact_refs,
            "command": evidence_command,
        }
    })
}

fn mark_workbench_candidate(experiment: &mut ExperimentRecord, key: &str, status: &str) {
    let Some(candidates) = experiment
        .workbench_candidates_v1
        .as_mut()
        .and_then(Value::as_object_mut)
    else {
        return;
    };
    let Some(candidate) = candidates.get_mut(key).and_then(Value::as_object_mut) else {
        return;
    };
    if candidate.get("status").and_then(Value::as_str) == Some("candidate") {
        candidate.insert("status".to_string(), json!(status));
        candidate.insert("resolved_at".to_string(), json!(iso_now()));
        candidates.insert("updated_at".to_string(), json!(iso_now()));
    }
}

fn charter_bind_relation(experiment: &ExperimentRecord, inner_action: &str) -> &'static str {
    let Some(proposed) = experiment
        .charter_v1
        .as_ref()
        .and_then(charter_proposed_next_action)
    else {
        return "no_charter";
    };
    if normalize_action_match(&proposed) == normalize_action_match(inner_action) {
        "matched_charter"
    } else {
        "diverged_from_charter"
    }
}

fn peer_experiment_ref(raw: &str) -> Option<PeerExperimentRef> {
    let (selector, focus) = split_experiment_selector_hint(raw);
    peer_experiment_ref_from_parts(selector.as_deref(), &focus).map(|mut peer| {
        peer.raw_selector = raw.trim().to_string();
        peer
    })
}

fn peer_experiment_ref_from_parts(
    selector: Option<&str>,
    focus: &str,
) -> Option<PeerExperimentRef> {
    let selector = selector?;
    let experiment_id = normalize_experiment_selector(selector);
    if !experiment_id.starts_with(PEER_EXPERIMENT_PREFIX) {
        return None;
    }
    Some(PeerExperimentRef {
        peer_system: PEER_SYSTEM.to_string(),
        peer_experiment_id: experiment_id,
        raw_selector: selector.trim().to_string(),
        focus: if focus.trim().is_empty() {
            None
        } else {
            Some(focus.trim().to_string())
        },
    })
}

fn split_experiment_selector_hint(raw: &str) -> (Option<String>, String) {
    let text = raw.trim();
    if text.is_empty() {
        return (None, String::new());
    }
    if let Some((selector, hint)) = text.split_once("::") {
        return (
            optional_selector_owned(&normalize_experiment_selector(selector)),
            hint.trim().to_string(),
        );
    }
    for marker in [" – ", " — ", " - "] {
        if let Some((selector, hint)) = text.split_once(marker) {
            let selector = normalize_experiment_selector(selector);
            if selector == "current" || selector.starts_with("exp_") {
                return (optional_selector_owned(&selector), hint.trim().to_string());
            }
        }
    }
    let lower = text.to_ascii_lowercase();
    if lower.starts_with("current ") {
        return (
            None,
            text["current ".len()..]
                .trim_matches(|ch| matches!(ch, ' ' | '-' | ':'))
                .to_string(),
        );
    }
    if lower.starts_with("exp_") {
        let mut parts = text.splitn(2, char::is_whitespace);
        let selector = parts.next().unwrap_or_default();
        if let Some(hint) = parts.next() {
            return (
                optional_selector_owned(&normalize_experiment_selector(selector)),
                hint.trim_matches(|ch| matches!(ch, ' ' | '-' | ':'))
                    .to_string(),
            );
        }
    }
    (
        optional_selector_owned(&normalize_experiment_selector(text)),
        String::new(),
    )
}

fn normalize_experiment_selector(selector: &str) -> String {
    let mut text = selector.trim();
    if text.is_empty() {
        return String::new();
    }
    let lower = text.to_ascii_lowercase();
    if lower == "current" || lower.starts_with("current ") {
        return "current".to_string();
    }
    if lower.starts_with("exp_") {
        for marker in ["::", " – ", " — ", " - "] {
            if let Some((head, _tail)) = text.split_once(marker) {
                text = head.trim();
            }
        }
        return text
            .split_whitespace()
            .next()
            .unwrap_or_default()
            .to_string();
    }
    text.to_string()
}

fn experiment_match_key(value: &str) -> String {
    value
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        .to_ascii_lowercase()
}

fn optional_selector(selector: &str) -> Option<&str> {
    let selector = selector.trim();
    if selector.is_empty() || selector.eq_ignore_ascii_case("current") {
        None
    } else {
        Some(selector)
    }
}

fn optional_selector_owned(selector: &str) -> Option<String> {
    let selector = selector.trim();
    if selector.is_empty() || selector.eq_ignore_ascii_case("current") {
        None
    } else {
        Some(selector.to_string())
    }
}
