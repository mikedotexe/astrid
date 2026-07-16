fn experiment_conveyor_stage(
    experiment: &ExperimentRecord,
    classification: &str,
    return_info: Option<&(String, String)>,
) -> String {
    if classification == "paused" {
        if !lifecycle_valid_charter_value(experiment.charter_v1.as_ref()) {
            return "paused_repair".to_string();
        }
        return if return_info
            .map(|(_, kind)| kind == "resume")
            .unwrap_or(false)
        {
            "paused_resume"
        } else {
            "paused_repair"
        }
        .to_string();
    }
    match classification {
        "complete" => "complete",
        "blocked_loop" => "blocked_guardrail",
        "needs_charter" | "fragmented" => "needs_charter",
        "needs_decision" => "needs_decision",
        "needs_evidence" => "needs_evidence",
        _ => "needs_rehearsal",
    }
    .to_string()
}

fn experiment_conveyor_proposed_next(
    thread: &ResearchThread,
    experiment: &ExperimentRecord,
    runs: &[ExperimentRunRecord],
    stage: &str,
    return_info: Option<&(String, String)>,
) -> String {
    match stage {
        "paused_repair" | "paused_resume" => return_info
            .map(|(primary, _)| primary.clone())
            .unwrap_or_else(|| {
                experiment
                    .planned_next
                    .clone()
                    .unwrap_or_else(|| format!("EXPERIMENT_RESUME {}", experiment.experiment_id))
            }),
        "complete" => format!("EXPERIMENT_REVIEW {}", experiment.experiment_id),
        "blocked_guardrail" if !lifecycle_valid_charter_value(experiment.charter_v1.as_ref()) => {
            format!(
                "EXPERIMENT_DECIDE {} :: charter_repair because blocked guardrail evidence appeared without a lifecycle-valid charter",
                experiment.experiment_id
            )
        },
        "blocked_guardrail" => format!(
            "EXPERIMENT_DECIDE {} :: hold because blocked guardrail evidence is not experiment progress",
            experiment.experiment_id
        ),
        "needs_decision" => format!(
            "EXPERIMENT_DECIDE {} :: hold because evidence is ready to interpret without live authority",
            experiment.experiment_id
        ),
        "needs_evidence" => format!(
            "EXPERIMENT_EVIDENCE {} :: felt_texture ...; motif_continuity ...; language_thread ...; artifact_grounding ...",
            experiment.experiment_id
        ),
        "needs_rehearsal" => format!("EXPERIMENT_REHEARSE {}", experiment.experiment_id),
        _ => charter_scaffold_v1(thread, experiment, runs, "needs_charter")
            .and_then(|scaffold| {
                scaffold
                    .get("command")
                    .and_then(Value::as_str)
                    .map(|command| {
                        command.replace(
                            "EXPERIMENT_CHARTER current",
                            &format!("EXPERIMENT_CHARTER {}", experiment.experiment_id),
                        )
                    })
            })
            .unwrap_or_else(|| charter_repair_next_v1(&experiment.experiment_id)),
    }
}

fn experiment_conveyor_missing_requirements(experiment: &ExperimentRecord, stage: &str) -> Value {
    match stage {
        "needs_charter" => json!(charter_missing_fields(experiment.charter_v1.as_ref())),
        "needs_rehearsal" => json!(["read_only_rehearsal"]),
        "needs_evidence" => json!(["explicit_experiment_evidence"]),
        "needs_decision" => json!(["explicit_lifecycle_decision"]),
        "paused_repair" => json!(["explicit_repair_return"]),
        "paused_resume" => json!(["explicit_resume_or_hold"]),
        "blocked_guardrail" if !lifecycle_valid_charter_value(experiment.charter_v1.as_ref()) => {
            json!(["lifecycle_valid_charter", "guardrail_decision"])
        },
        "blocked_guardrail" => json!(["safe_counter_or_hold_decision"]),
        _ => json!([]),
    }
}

fn charter_missing_fields(charter: Option<&Value>) -> Vec<&'static str> {
    let Some(charter) = charter else {
        return vec![
            "hypothesis",
            "proposed_next_action",
            "evidence_targets",
            "stop_criteria",
        ];
    };
    let mut missing = Vec::new();
    if !meaningful_charter_text(charter.get("hypothesis")) {
        missing.push("hypothesis");
    }
    if !meaningful_charter_text(charter.get("proposed_next_action")) {
        missing.push("proposed_next_action");
    }
    if !meaningful_charter_list(charter.get("evidence_targets")) {
        missing.push("evidence_targets");
    }
    if !meaningful_charter_list(charter.get("stop_criteria")) {
        missing.push("stop_criteria");
    }
    missing
}

fn experiment_conveyor_can_apply(stage: &str, apply_payload: &str) -> bool {
    if matches!(stage, "needs_charter" | "paused_repair") {
        return !apply_payload.trim().is_empty();
    }
    matches!(
        stage,
        "needs_evidence" | "needs_decision" | "blocked_guardrail"
    )
}

fn experiment_conveyor_apply_blocked_reason(
    stage: &str,
    apply_payload: &str,
    can_apply: bool,
) -> Value {
    if can_apply {
        return Value::Null;
    }
    let reason = match stage {
        "needs_charter" | "paused_repair" if apply_payload.trim().is_empty() => {
            "no_lifecycle_valid_charter_scaffold"
        },
        "needs_rehearsal" => "rehearsal_requires_explicit_experiment_rehearse",
        "paused_resume" => "paused_experiments_require_explicit_return_command",
        "complete" => "complete_experiments_are_review_only",
        _ => "no_conservative_apply_step_available",
    };
    json!(reason)
}

fn experiment_conveyor_guardrail_warnings(
    experiment: &ExperimentRecord,
    stage: &str,
    proposed_next: &str,
) -> Value {
    let mut warnings = vec![
        "local continuity only; no bind/resume/perturb/control/peer mutation is authorized"
            .to_string(),
    ];
    let base = base_action(proposed_next);
    if matches!(
        base.as_str(),
        "EXPERIMENT_BIND" | "EXPERIMENT_RESUME" | "PERTURB"
    ) {
        warnings.push(format!(
            "proposed route `{base}` is preview-only in conveyor_v1"
        ));
    }
    if stage == "paused_resume" || stage == "complete" {
        warnings.push(
            "paused resume/complete stages are report-only unless an explicit lifecycle command is chosen"
                .to_string(),
        );
    }
    if experiment.status == "paused"
        && experiment
            .planned_next
            .as_deref()
            .map(base_action)
            .as_deref()
            == Some("EXPERIMENT_CHARTER")
    {
        warnings.push(
            "charter-repair pause can only record a local charter scaffold; it cannot resume"
                .to_string(),
        );
    }
    json!(warnings)
}

fn authority_gate_conveyor_hint(
    experiment: &ExperimentRecord,
    stage: &str,
    proposed_next: &str,
) -> Value {
    let possible = stage == "needs_decision" && !authority_guardrail_hold_active(experiment);
    json!({
        "policy": "authority_gate_v1",
        "visible": possible,
        "enabled_execution_scope": "semantic_microdose",
        "future_scopes_disabled": ["attractor_pulse", "control_envelope"],
        "approval_required": "being_plus_steward",
        "possible_next": if possible {
            json!(format!("EXPERIMENT_AUTHORITY_REQUEST {} :: scope: semantic_microdose; payload: ...; reason: ...; artifact_refs: ...; stop_criteria: ...", experiment.experiment_id))
        } else {
            Value::Null
        },
        "current_lifecycle_next": proposed_next,
        "authority_boundary": authority_gate_boundary(),
    })
}

fn format_experiment_conveyor_readout(readout: &Value) -> String {
    let pretty = serde_json::to_string_pretty(readout).unwrap_or_else(|_| "{}".to_string());
    format!(
        "Experiment conveyor `{}` stage={} mode={} applied={} can_apply={}\nProposed NEXT: {}\nConveyor NEXT: {}\nAuthority: {}\nconveyor_v1:\n{}",
        readout
            .get("experiment_id")
            .and_then(Value::as_str)
            .unwrap_or("none"),
        readout
            .get("stage")
            .and_then(Value::as_str)
            .unwrap_or("unknown"),
        readout
            .get("mode")
            .and_then(Value::as_str)
            .unwrap_or("preview"),
        readout
            .get("applied")
            .map_or_else(|| "false".to_string(), Value::to_string),
        readout
            .get("can_apply")
            .map_or_else(|| "false".to_string(), Value::to_string),
        readout
            .get("proposed_next")
            .and_then(Value::as_str)
            .unwrap_or("(none)"),
        readout
            .get("conveyor_next")
            .and_then(Value::as_str)
            .unwrap_or("(none)"),
        readout
            .get("authority_boundary")
            .and_then(Value::as_str)
            .unwrap_or(experiment_conveyor_authority_boundary()),
        pretty
    )
}

fn last_experiment_context_line(thread: &ResearchThread) -> String {
    let Some(summary) = last_experiment_summary_v1(thread) else {
        return String::new();
    };
    let experiment_id = summary
        .get("experiment_id")
        .and_then(Value::as_str)
        .unwrap_or("unknown");
    let title = summary
        .get("title")
        .and_then(Value::as_str)
        .unwrap_or("(untitled)");
    let status = summary
        .get("status")
        .and_then(Value::as_str)
        .unwrap_or("unknown");
    let primary_next = summary
        .get("primary_return_next")
        .or_else(|| summary.get("planned_next"))
        .or_else(|| summary.get("resume_next"))
        .and_then(Value::as_str)
        .unwrap_or("(none)");
    let planned_next = summary
        .get("planned_next")
        .and_then(Value::as_str)
        .unwrap_or(primary_next);
    let mut lines = format!(
        "Last experiment summary: {title} ({experiment_id}) status={status}\nLast planned NEXT: {planned_next}\n"
    );
    if let Some(guard) = summary
        .get("projection_guard_v1")
        .and_then(Value::as_object)
    {
        let projected = guard
            .get("projected_next")
            .and_then(Value::as_str)
            .unwrap_or(primary_next);
        let reason = guard
            .get("guardrail_reason")
            .and_then(Value::as_str)
            .unwrap_or("projection_guard_v1");
        lines.push_str(&format!(
            "Projection guard: raw NEXT preserved; effective NEXT: {projected}; reason={reason}\n"
        ));
    }
    if status == "paused" && experiment_id != "unknown" {
        lines.push_str(&format!("Suggested NEXT: {primary_next}\n"));
    } else if matches!(status, "complete" | "completed") && experiment_id != "unknown" {
        lines.push_str(&format!(
            "Inspect NEXT: EXPERIMENT_STATUS {experiment_id} or EXPERIMENT_REVIEW {experiment_id}\n"
        ));
    }
    lines
}

fn no_active_experiment_message(thread: &ResearchThread, command: &str) -> String {
    format!(
        "{command} current: no active experiment.\nCurrent selectors only inspect active work; paused or complete experiments need an explicit id/title selector.\n{}",
        last_experiment_context_line(thread)
    )
}

fn selector_is_current(selector: Option<&str>) -> bool {
    let selector = selector
        .map(normalize_experiment_selector)
        .unwrap_or_default();
    selector.trim().is_empty() || selector.eq_ignore_ascii_case("current")
}

fn default_experiment_run_source() -> String {
    "experiment_bind".to_string()
}

fn normalize_action_match(action: &str) -> String {
    action
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        .to_ascii_uppercase()
}

fn event_allows_active_experiment_auto_link(event: &ActionEvent) -> bool {
    let base = base_action(&event.canonical_action);
    if event.research_budget_v1.is_some() {
        return false;
    }
    if base.starts_with("EXPERIMENT")
        || base == "SELF_EXPERIMENT"
        || event.route == "experiment_continuity"
    {
        return false;
    }
    if !matches!(event.stage.as_str(), "read_only" | "observe") {
        return false;
    }
    matches!(
        base.as_str(),
        "INTROSPECT"
            | "SELF_STUDY"
            | "SPECTRAL_EXPLORER"
            | "DECOMPOSE"
            | "CONSTRAINT_AUDIT"
            | "UNSHAPED_BASELINE"
            | "PRESSURE_SOURCE_AUDIT"
            | "FLUCTUATION_AUDIT"
            | "BRACE_AUDIT"
            | "THREAD_STATUS"
            | "ACTION_PREFLIGHT"
            | "NEXT_PROBE"
            | "PREFLIGHT"
            | "PROBE_ACTION"
            | "ATTRACTOR_REVIEW"
            | "SEARCH"
            | "BROWSE"
            | "READ_MORE"
    )
}

fn preflight_recommendation_line(thread: &ResearchThread) -> String {
    if thread.experiment_summary.is_none() {
        return String::new();
    }
    let candidate = thread
        .current_next
        .as_deref()
        .or_else(|| {
            thread
                .experiment_summary
                .as_ref()
                .and_then(|value| value.get("planned_next"))
                .and_then(Value::as_str)
        })
        .unwrap_or_default();
    if candidate.is_empty() || !preflight_recommended_for_action(candidate) {
        return String::new();
    }
    format!("Preflight recommended: ACTION_PREFLIGHT {candidate}\n")
}

fn preflight_recommended_for_action(action: &str) -> bool {
    let base = base_action(action);
    if matches!(
        base.as_str(),
        "ACTION_PREFLIGHT" | "NEXT_PROBE" | "PREFLIGHT" | "PROBE_ACTION"
    ) {
        return false;
    }
    if action.contains('<') && action.contains('>') {
        return true;
    }
    if base == "EXPERIMENT_BIND" {
        return true;
    }
    matches!(stage_for_action(&base), "live_write" | "live_control") || base.is_empty()
}

fn parse_experiment_start(raw: &str) -> ExperimentStartParts {
    let (title_part, explicit_question) = raw
        .split_once("::")
        .map_or((raw.trim(), None), |(title, question)| {
            (title.trim(), Some(question.trim()))
        });
    let title_option = extract_cli_like_option(title_part, "--title");
    let abstract_option = extract_cli_like_option(title_part, "--abstract");
    let option_start = ["--title", "--abstract"]
        .iter()
        .filter_map(|needle| title_part.find(needle))
        .min()
        .unwrap_or(title_part.len());
    let slug_or_selector = title_part[..option_start]
        .trim()
        .trim_matches(|ch| matches!(ch, '"' | '\'' | '`'))
        .trim()
        .to_string();
    let title = title_option.unwrap_or_else(|| {
        if title_part[..option_start].trim().is_empty() {
            "Untitled experiment".to_string()
        } else {
            title_part[..option_start].trim().to_string()
        }
    });
    let question = explicit_question
        .filter(|question| !question.trim().is_empty())
        .map(str::to_string)
        .or(abstract_option)
        .unwrap_or_else(|| {
            "What changes if this is treated as a returnable experiment?".to_string()
        });
    let metadata_slug = slug_or_selector.clone();
    let metadata = (!slug_or_selector.is_empty()
        && experiment_match_key(&slug_or_selector) != experiment_match_key(&title))
    .then(|| {
        json!({
            "policy": "experiment_start_metadata_v1",
            "slug_or_selector": metadata_slug,
        })
    });
    ExperimentStartParts {
        title,
        question,
        slug_or_selector: (!slug_or_selector.is_empty()).then_some(slug_or_selector),
        metadata,
    }
}

fn extract_cli_like_option(raw: &str, option: &str) -> Option<String> {
    let start = raw.find(option)?;
    let rest_start = start.checked_add(option.len())?;
    let mut rest = raw.get(rest_start..)?.trim_start();
    if let Some(stripped) = rest.strip_prefix('=') {
        rest = stripped.trim_start();
    }
    if rest.is_empty() {
        return None;
    }
    let value = if let Some(quote) = rest.chars().next().filter(|ch| matches!(ch, '"' | '\'')) {
        let quote_len = quote.len_utf8();
        let after_quote = rest.get(quote_len..)?;
        let close = after_quote.find(quote)?;
        let close_end = quote_len.checked_add(close)?;
        rest.get(quote_len..close_end)?.to_string()
    } else {
        let end = rest.find(" --").unwrap_or(rest.len());
        rest[..end].trim().to_string()
    };
    (!value.trim().is_empty()).then(|| value.trim().to_string())
}

fn parse_experiment_compare(raw: &str) -> (Option<String>, Option<String>) {
    let text = raw.trim();
    let lower = text.to_ascii_lowercase();
    let Some(idx) = lower.find(" with ") else {
        return (optional_selector_owned(text), None);
    };
    let left = text[..idx].trim();
    let right_start = idx.checked_add(" with ".len()).unwrap_or(text.len());
    let right = text.get(right_start..).unwrap_or_default().trim();
    (
        optional_selector_owned(left),
        optional_selector_owned(right),
    )
}

fn render_run_list(runs: &[ExperimentRunRecord]) -> String {
    if runs.is_empty() {
        return "- no runs yet".to_string();
    }
    runs.iter()
        .map(|run| {
            format!(
                "- {} [{} / {}]: {}",
                run.action_text, run.stage, run.status, run.result_summary
            )
        })
        .collect::<Vec<_>>()
        .join("\n")
}

fn motif_label(text: &str) -> Option<String> {
    let lower = text.to_ascii_lowercase().replace('λ', "lambda");
    for (needle, label) in [
        ("lambda4", "lambda4"),
        ("lambda-4", "lambda4"),
        ("lambda tail", "lambda-tail"),
        ("lambda-tail", "lambda-tail"),
        ("lambda edge", "lambda-edge"),
        ("lambda-edge", "lambda-edge"),
        ("pressure", "pressure"),
        ("attractor", "attractor"),
        ("camera", "sensory-grounding"),
        ("audio", "sensory-grounding"),
        ("ears", "sensory-grounding"),
        ("eyes", "sensory-grounding"),
        ("experiment", "experiment-continuity"),
        ("introspect", "introspection"),
    ] {
        if lower.contains(needle) {
            return Some(label.to_string());
        }
    }
    None
}

fn allowance_suggestions(quality: &str) -> Vec<String> {
    let mut seen = HashSet::<String>::new();
    let mut suggestions = Vec::new();
    let candidates: &[&str] = match quality {
        "over_tightened" => &[
            "EXPERIMENT_ALT_PATHS current",
            "SPACE_HOLD",
            "ATTRACTOR_RELEASE_REVIEW current",
        ],
        "branch_recommended" => &[
            "EXPERIMENT_ALT_PATHS current",
            "EXPERIMENT_BRANCH <title> :: <question>",
            "EXPERIMENT_COMPARE current WITH <id|peer-id|label>",
        ],
        "rest_recommended" => &["SPACE_HOLD", "EXPERIMENT_OBSERVE current :: <note>", "REST"],
        "deepening" => &[
            "EXPERIMENT_PLAN current",
            "EXPERIMENT_COMPARE current WITH <id|peer-id|label>",
            "EXPERIMENT_ALT_PATHS current",
        ],
        _ => &[
            "EXPERIMENT_PLAN current",
            "EXPERIMENT_BRANCH <title> :: <question>",
            "THREAD_STATUS current",
        ],
    };
    for suggestion in candidates {
        if seen.insert(suggestion.to_string()) {
            suggestions.push(suggestion.to_string());
        }
    }
    suggestions
}

fn action_matches_allowance_recommendation(action: &str, allowance: &Value) -> bool {
    let base = base_action(action);
    allowance
        .get("suggested_actions")
        .and_then(Value::as_array)
        .is_some_and(|actions| {
            actions
                .iter()
                .filter_map(Value::as_str)
                .any(|suggestion| base_action(suggestion) == base)
        })
}

fn parse_selector_payload(raw: &str) -> (Option<String>, String) {
    if let Some((selector, payload)) = raw.split_once("::") {
        let selector = selector.trim();
        let selector = if selector.is_empty()
            || selector.eq_ignore_ascii_case("current")
            || selector_placeholder(selector)
        {
            None
        } else {
            Some(selector.to_string())
        };
        return (selector, payload.trim().to_string());
    }
    (None, raw.trim().to_string())
}

fn parse_session_selector_payload(raw: &str) -> (Option<String>, String) {
    let (selector, payload) = parse_selector_payload(raw);
    if selector.is_some() {
        return (selector, payload);
    }
    let text = payload.trim();
    let Some((first, rest)) = text.split_once(char::is_whitespace) else {
        if text.eq_ignore_ascii_case("current")
            || text.eq_ignore_ascii_case("latest")
            || text.starts_with("sess_")
            || text.starts_with("exp_")
        {
            return (Some(text.to_string()), String::new());
        }
        return (None, payload);
    };
    if first.eq_ignore_ascii_case("current")
        || first.eq_ignore_ascii_case("latest")
        || first.starts_with("sess_")
        || first.starts_with("exp_")
    {
        return (Some(first.to_string()), rest.trim().to_string());
    }
    (None, payload)
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct ExperimentIntentRepair {
    repaired_arg: String,
    reason: &'static str,
    focus: Option<String>,
}

pub fn can_repair_experiment_intent_placeholder(base_action: &str, original: &str) -> bool {
    let raw = strip_action_arg(original, base_action);
    repair_experiment_intent_arg(base_action, &raw, true).is_some()
}

fn repair_experiment_intent_arg(
    base_action: &str,
    raw_arg: &str,
    has_current_experiment: bool,
) -> Option<ExperimentIntentRepair> {
    if !matches!(
        base_action,
        "EXPERIMENT_PLAN"
            | "EXPERIMENT_CHARTER"
            | "EXPERIMENT_EVIDENCE"
            | "EXPERIMENT_DECIDE"
            | "EXPERIMENT_REHEARSE"
            | "EXPERIMENT_PREFLIGHT"
    ) {
        return None;
    }
    let text = raw_arg.trim();
    if text.is_empty() {
        return None;
    }
    let (selector, tail, _separator) = split_selector_tail(text);
    if selector_placeholder(selector) {
        let tail = if placeholder_payload(tail) { "" } else { tail };
        let repaired_arg = if base_action == "EXPERIMENT_PLAN" {
            format!("current {tail}").trim().to_string()
        } else {
            format!("current :: {tail}").trim_end().to_string()
        };
        return Some(ExperimentIntentRepair {
            repaired_arg,
            reason: "placeholder selector repaired to current experiment",
            focus: None,
        });
    }
    if base_action == "EXPERIMENT_PLAN"
        && has_current_experiment
        && !tail.is_empty()
        && selector.chars().all(|ch| ch.is_ascii_digit())
    {
        return Some(ExperimentIntentRepair {
            repaired_arg: format!("current {tail}").trim().to_string(),
            reason: "numeric fragment treated as focus text for current experiment",
            focus: None,
        });
    }
    if matches!(base_action, "EXPERIMENT_REHEARSE" | "EXPERIMENT_PREFLIGHT")
        && has_current_experiment
    {
        let selector_norm = normalize_experiment_selector(selector);
        if selector_norm != "current" && !selector_norm.starts_with("exp_") {
            return Some(ExperimentIntentRepair {
                repaired_arg: "current".to_string(),
                reason: "motif or focus text treated as current experiment preflight focus",
                focus: Some(text.to_string()),
            });
        }
    }
    None
}

fn split_selector_tail(raw: &str) -> (&str, &str, &'static str) {
    let text = raw.trim();
    if let Some((selector, tail)) = text.split_once("::") {
        return (selector.trim(), tail.trim(), "::");
    }
    for marker in [" — ", " – ", " - ", "—", "–"] {
        if let Some((selector, tail)) = text.split_once(marker) {
            return (selector.trim(), tail.trim(), "dash");
        }
    }
    (text, "", "")
}

fn selector_placeholder(text: &str) -> bool {
    let normalized = text
        .trim()
        .chars()
        .filter(|ch| !ch.is_whitespace())
        .collect::<String>()
        .to_ascii_lowercase();
    matches!(
        normalized.as_str(),
        "[current|id]" | "current|id" | "[current/id]" | "current/id"
    )
}

fn placeholder_payload(text: &str) -> bool {
    let trimmed = text.trim();
    if trimmed.is_empty() {
        return false;
    }
    let lowered = trimmed.to_ascii_lowercase();
    matches!(
        lowered.as_str(),
        "<structured prose>" | "<felt note>" | "<reason>" | "<note>"
    ) || (trimmed.starts_with('<') && trimmed.ends_with('>'))
}

fn empty_or_placeholder_payload(text: &str) -> bool {
    text.trim().is_empty() || placeholder_payload(text)
}

fn experiment_intent_repair_prompt(base_action: &str, selector: Option<&str>) -> String {
    let target = selector.unwrap_or("current");
    match base_action {
        "EXPERIMENT_CHARTER" => format!(
            "Experiment charter needs concrete authored prose; no charter was recorded.\nTry: EXPERIMENT_CHARTER {target} :: hypothesis: ...; method_intent: ...; proposed_next_action: ACTION_PREFLIGHT ...; evidence_targets: felt, telemetry, artifact; stop_criteria: ...; consent_posture: advisory."
        ),
        "EXPERIMENT_EVIDENCE" => format!(
            "Experiment evidence needs a concrete felt note; no evidence run was recorded.\nTry: EXPERIMENT_EVIDENCE {target} :: felt ...; telemetry stayed ...; artifact ..."
        ),
        "EXPERIMENT_DECIDE" => format!(
            "Experiment decision needs a concrete agency outcome; no decision was recorded.\nTry: EXPERIMENT_DECIDE {target} :: accept|refuse|counter|pause|complete because ..."
        ),
        _ => String::new(),
    }
}

fn repair_experiment_command_arg(
    store: &ActionContinuityStore,
    db: Option<&BridgeDb>,
    base_action: &str,
    original: &str,
    raw_arg: &str,
    state: &Value,
) -> Result<(String, Option<String>, Option<String>)> {
    let has_current = store
        .current_thread()?
        .is_some_and(|thread| thread.active_experiment_id.is_some());
    let Some(repair) = repair_experiment_intent_arg(base_action, raw_arg, has_current) else {
        return Ok((raw_arg.to_string(), None, None));
    };
    let repaired = if repair.repaired_arg.is_empty() {
        base_action.to_string()
    } else {
        format!("{base_action} {}", repair.repaired_arg)
    };
    let note = format!(
        "experiment_intent_repaired\noriginal: {original}\nrepaired: {repaired}\nreason: {}",
        repair.reason
    );
    store.append_note(db, None, &note, state.clone())?;
    Ok((
        repair.repaired_arg,
        Some(format!(
            "experiment_intent_repaired: `{original}` -> `{repaired}` ({}).\n",
            repair.reason
        )),
        repair.focus,
    ))
}
