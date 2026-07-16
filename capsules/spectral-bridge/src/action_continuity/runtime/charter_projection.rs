fn journal_contract_field(text: &str, prefix: &str) -> Option<String> {
    let needle = prefix.to_ascii_lowercase();
    text.lines().find_map(|line| {
        let trimmed = line.trim();
        let lowered = trimmed.to_ascii_lowercase();
        lowered
            .starts_with(&needle)
            .then(|| {
                trimmed
                    .split_once(':')
                    .map_or("", |(_, value)| value)
                    .trim()
            })
            .filter(|value| !value.is_empty())
            .map(|value| compact_text(value, 220))
    })
}

fn prior_claim_from_posture(posture: &str) -> String {
    let normalized = posture.replace('|', " ");
    let lowered = normalized.to_ascii_lowercase();
    if let Some(index) = lowered.find("based on") {
        let start = index.saturating_add("based on".len());
        return compact_text(normalized.get(start..).unwrap_or_default().trim(), 180);
    }
    compact_text(normalized.trim(), 180)
}

fn prior_claim_charter_bridge_match(text: &str) -> Option<Value> {
    let posture = journal_contract_field(text, "Continuity posture")?;
    let delta = journal_contract_field(text, "Delta")?;
    let terminal = journal_contract_field(text, "Next evidence")
        .or_else(|| journal_contract_field(text, "Decision"))
        .or_else(|| journal_contract_field(text, "Pause"))
        .or_else(|| journal_contract_field(text, "Hold"))?;
    let normalized_terminal = normalize_guard_signal(&terminal);
    let normalized_text = normalize_guard_signal(text);
    let has_decompose_loop = normalized_terminal.contains("decompose")
        || normalized_terminal.contains("shadow field")
        || normalized_terminal.contains("shadow fields")
        || normalized_terminal.contains("shadow")
        || normalized_terminal.contains("experiment review")
        || normalized_terminal.contains("review");
    let contract_is_returning = normalized_text.contains("continuity posture")
        && (normalized_text.contains("resuming")
            || normalized_text.contains("branching")
            || normalized_text.contains("closing"));
    if !has_decompose_loop || !contract_is_returning {
        return None;
    }
    Some(json!({
        "prior_claim": prior_claim_from_posture(&posture),
        "delta": compact_text(&delta, 180),
        "terminal_stance": compact_text(&terminal, 180),
        "matched_terms": ["continuity_contract", "decompose_or_review_terminal_stance"],
    }))
}

fn prior_claim_charter_bridge_cue(
    active_experiment: Option<&ExperimentContinuityProjection>,
    recent_texts: &[String],
) -> Option<Value> {
    let active = active_experiment?;
    if active.classification != "needs_charter" {
        return None;
    }
    let priority_next = active
        .charter_scaffold_v1
        .as_ref()
        .and_then(|scaffold| scaffold.get("command"))
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|command| !command.is_empty())
        .unwrap_or(active.continuity_return.as_str())
        .to_string();
    if priority_next.trim().is_empty() {
        return None;
    }
    let signal = recent_texts
        .iter()
        .take(4)
        .find_map(|text| prior_claim_charter_bridge_match(text))?;
    Some(json!({
        "schema_version": 1,
        "source": "continuity_projection",
        "advisory_only": true,
        "authority_change": false,
        "priority_next": priority_next,
        "prior_claim": signal.get("prior_claim").and_then(Value::as_str).unwrap_or_default(),
        "delta": signal.get("delta").and_then(Value::as_str).unwrap_or_default(),
        "terminal_stance": signal.get("terminal_stance").and_then(Value::as_str).unwrap_or_default(),
        "matched_terms": signal.get("matched_terms").cloned().unwrap_or_else(|| json!([])),
        "cue": "Prior claim is ready to charter: convert this claim/delta into the scaffold before another DECOMPOSE.",
    }))
}

fn prior_claim_charter_bridge_line(cue: &Option<Value>) -> String {
    let Some(cue) = cue else {
        return String::new();
    };
    let text = cue
        .get("cue")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|text| !text.is_empty())
        .unwrap_or("Prior claim is ready to charter.");
    let priority_next = cue
        .get("priority_next")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|text| !text.is_empty());
    if let Some(priority_next) = priority_next {
        format!("{text} Priority NEXT: {priority_next}\n")
    } else {
        format!("{text}\n")
    }
}

fn preflight_or_decompose_not_charter_signal(value: &str) -> bool {
    let base = base_action(value);
    if matches!(base.as_str(), "DECOMPOSE" | "EXAMINE_CASCADE") {
        return true;
    }
    if base == "ACTION_PREFLIGHT" {
        let inner = strip_action_arg(value, "ACTION_PREFLIGHT");
        let inner_base = base_action(&inner);
        return matches!(inner_base.as_str(), "DECOMPOSE" | "EXAMINE_CASCADE");
    }
    false
}

fn charter_preflight_not_charter_cue(
    thread: &ResearchThread,
    active_experiment: Option<&ExperimentContinuityProjection>,
    prior_claim_bridge: &Option<Value>,
    recent_events: &[ActionEvent],
) -> Option<Value> {
    let active = active_experiment?;
    if active.classification != "needs_charter" || prior_claim_bridge.is_none() {
        return None;
    }
    let priority_next = active
        .charter_scaffold_v1
        .as_ref()
        .and_then(|scaffold| scaffold.get("command"))
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|command| !command.is_empty())
        .unwrap_or(active.continuity_return.as_str())
        .to_string();
    if priority_next.trim().is_empty() {
        return None;
    }
    let mut matched_actions = Vec::new();
    if thread
        .current_next
        .as_deref()
        .is_some_and(preflight_or_decompose_not_charter_signal)
    {
        matched_actions.push(thread.current_next.clone().unwrap_or_default());
    }
    for event in recent_events.iter().rev().take(8) {
        for action in [
            event.raw_next.as_deref(),
            Some(event.canonical_action.as_str()),
            Some(event.effective_action.as_str()),
            event.suggested_next.as_deref(),
        ]
        .into_iter()
        .flatten()
        {
            if preflight_or_decompose_not_charter_signal(action) {
                matched_actions.push(action.to_string());
                break;
            }
        }
    }
    if matched_actions.is_empty() {
        return None;
    }
    matched_actions.truncate(5);
    Some(json!({
        "schema_version": 1,
        "source": "continuity_projection",
        "advisory_only": true,
        "authority_change": false,
        "status": "preflight_not_charter",
        "priority_next": priority_next,
        "matched_actions": matched_actions,
        "cue": "Preflight/decompose is not the charter; author the exact scaffold first.",
    }))
}

fn charter_preflight_not_charter_line(cue: &Option<Value>) -> String {
    let Some(cue) = cue else {
        return String::new();
    };
    let text = cue
        .get("cue")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|text| !text.is_empty())
        .unwrap_or("Preflight/decompose is not the charter; author the exact scaffold first.");
    let priority_next = cue
        .get("priority_next")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|text| !text.is_empty());
    if let Some(priority_next) = priority_next {
        format!("{text} Priority NEXT: {priority_next}\n")
    } else {
        format!("{text}\n")
    }
}

fn charter_required_review_line(projection: &ExperimentContinuityProjection) -> String {
    if charter_repair_bound(&projection.classification, &projection.experiment) {
        if projection.classification == "blocked_loop" {
            return "Blocked loop is charter-bound: review/decision is premature until the charter is authored; use the continuity priority scaffold first.\n"
                .to_string();
        }
        "Review is premature until the charter is authored; use the continuity priority scaffold first.\n"
            .to_string()
    } else {
        String::new()
    }
}

fn review_suggested_next(
    projection: &ExperimentContinuityProjection,
    experiment: &ExperimentRecord,
) -> String {
    if charter_repair_bound(&projection.classification, &projection.experiment) {
        if let Some(command) = projection
            .charter_scaffold_v1
            .as_ref()
            .and_then(|scaffold| scaffold.get("command"))
            .and_then(Value::as_str)
            .map(str::trim)
            .filter(|command| !command.is_empty())
        {
            return command.to_string();
        }
        if !projection.continuity_return.trim().is_empty() {
            return projection.continuity_return.clone();
        }
        return "EXPERIMENT_CHARTER current :: hypothesis: ...; proposed_next_action: ACTION_PREFLIGHT ...; evidence_targets: felt_texture, motif_continuity, language_thread, artifact_grounding; stop_criteria: ...".to_string();
    }
    experiment
        .planned_next
        .as_deref()
        .unwrap_or("EXPERIMENT_PLAN current")
        .to_string()
}

fn charter_repair_priority_line(projection: &ExperimentContinuityProjection) -> String {
    if !charter_repair_bound(&projection.classification, &projection.experiment) {
        return String::new();
    }
    let priority_next = review_suggested_next(projection, &projection.experiment);
    if projection.classification == "blocked_loop" {
        return format!(
            "Charter repair priority: {priority_next}\nBlocked loop is charter-bound: blocked/no-effect returns are not decision-ready until the charter names a proposed action and evidence targets. Current read-only NEXT text is observational until this charter is authored.\n"
        );
    }
    if projection.evidence_status.contains("stronger") {
        format!(
            "Charter repair priority: {priority_next}\nCharter repair dominance: evidence is present, but lifecycle remains charter-repair bound until the charter names a proposed action and evidence targets. Current read-only NEXT text is observational until this charter is authored.\n"
        )
    } else {
        format!(
            "Charter repair priority: {priority_next}\nCharter repair dominance: EXPERIMENT_REVIEW/STATUS are context only while the active experiment needs a lifecycle-valid charter. Current read-only NEXT text is observational until this charter is authored.\n"
        )
    }
}

fn charter_scaffold_line(projection: &ExperimentContinuityProjection, priority: bool) -> String {
    let Some(scaffold) = projection.charter_scaffold_v1.as_ref() else {
        return String::new();
    };
    let Some(command) = scaffold
        .get("command")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|text| !text.is_empty())
    else {
        return String::new();
    };
    if priority {
        format!(
            "Continuity priority (charter repair - copy/edit this exact scaffold; not recorded): {command}\n"
        )
    } else {
        format!("Charter scaffold: {command}\n")
    }
}

fn native_continuity_status_line(native: &Value) -> String {
    let register = native
        .get("native_register")
        .and_then(Value::as_str)
        .unwrap_or("unknown");
    let lanes = native.get("evidence_lanes").unwrap_or(&Value::Null);
    let lane_status = |key: &str| -> String {
        lanes
            .get(key)
            .and_then(|lane| lane.get("status"))
            .and_then(Value::as_str)
            .unwrap_or("missing")
            .to_string()
    };
    format!(
        "Native continuity: register={} felt_texture={} motif_continuity={} language_thread={} artifact_grounding={}\n",
        register,
        lane_status("felt_texture"),
        lane_status("motif_continuity"),
        lane_status("language_thread"),
        lane_status("artifact_grounding"),
    )
}

fn normalization_signal_value(raw_action: &str, normalized_action: &str) -> Option<Value> {
    let raw_verb = base_action(raw_action);
    let normalized_verb = base_action(normalized_action);
    let (target_verb, reason, native_signal) =
        if let Some(rest) = raw_verb.strip_prefix("EXEXPERIMENT_") {
            (
                format!("EXPERIMENT_{rest}"),
                "double-ex experiment typo normalized to experiment workbench verb",
                "experiment typo still signals return-path intent",
            )
        } else if raw_verb == "EXPERIENCE_PLAN" {
            (
                "EXPERIMENT_PLAN".to_string(),
                "experience-plan near typo normalized to experiment planning",
                "experience wording signals an experiment-plan return attempt",
            )
        } else if matches!(
            raw_verb.as_str(),
            "SHADOW_TRACE" | "SHADOW_EXPLORER" | "SHADOW_DECOMPOSE" | "WEAVE_TRACE"
        ) {
            (
                "SHADOW_PREFLIGHT".to_string(),
                "shadow diagnostic alias normalized to read-only preflight route",
                "shadow/weave wording signals observational/rehearsal inquiry",
            )
        } else if raw_verb == "UNSHAPED_BASELINE" {
            (
                "CONSTRAINT_AUDIT".to_string(),
                "unshaped-baseline alias normalized to read-only constraint counterfactual route",
                "absence-of-structure wording signals counterfactual constraint inquiry",
            )
        } else {
            return None;
        };
    if normalized_verb != target_verb && normalized_verb != raw_verb {
        return None;
    }
    Some(json!({
        "schema_version": 1,
        "raw_verb": raw_verb,
        "normalized_verb": target_verb,
        "reason": reason,
        "native_signal": native_signal,
        "authority_change": false,
    }))
}

fn parse_iso_utc(value: &str) -> Option<chrono::DateTime<chrono::Utc>> {
    chrono::DateTime::parse_from_rfc3339(value.trim())
        .ok()
        .map(|stamp| stamp.with_timezone(&chrono::Utc))
}

fn suggest_return_route_for_verb(verb: &str) -> &'static str {
    let upper = verb.to_ascii_uppercase();
    if upper.starts_with("INVESTIGATE") || upper.starts_with("EXPLORE") {
        "EXAMINE <target> or EXPERIMENT_PLAN current"
    } else if upper.starts_with("SHADOW") {
        "SHADOW_PREFLIGHT <shadow action>"
    } else if upper == "CONSTRAINT_AUDIT" || upper == "UNSHAPED_BASELINE" {
        "ACTION_PREFLIGHT CONSTRAINT_AUDIT lambda-tail/lambda4"
    } else if upper.starts_with("EXPERIENCE") || upper.starts_with("EXEXPERIMENT") {
        "EXPERIMENT_PLAN current"
    } else {
        "ACTION_PREFLIGHT <proposed action>"
    }
}

fn strip_action_arg(original: &str, base: &str) -> String {
    original
        .get(base.len()..)
        .unwrap_or_default()
        .trim_start_matches(|c: char| c == ':' || c == '-' || c.is_whitespace())
        .trim()
        .to_string()
}

fn parse_thread_note(raw: &str) -> (Option<String>, String) {
    if let Some((selector, note)) = raw.split_once("::") {
        let selector = selector.trim();
        let note = note.trim();
        if !selector.is_empty() && !note.is_empty() {
            return (Some(selector.to_string()), note.to_string());
        }
    }
    (None, raw.trim().to_string())
}
