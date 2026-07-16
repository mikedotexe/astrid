fn base_action(action: &str) -> String {
    action
        .split_whitespace()
        .next()
        .unwrap_or(action)
        .trim_end_matches(':')
        .to_ascii_uppercase()
}

fn charter_guard_block_reason(raw_next: &str) -> Option<(CharterReason, String)> {
    let action = raw_next.split_whitespace().collect::<Vec<_>>().join(" ");
    if action.is_empty() {
        return None;
    }
    let base = base_action(&action);
    if charter_guard_allows_directed_language_base(&base) {
        return None;
    }
    if read_only_research_budget_base(&base) {
        return Some((CharterReason::ResearchBudget, action));
    }
    if charter_guard_live_base(&base) {
        return Some((CharterReason::LiveAction, action));
    }
    if base == "EXPERIMENT_BIND" {
        let raw_arg = strip_action_arg(&action, "EXPERIMENT_BIND");
        if raw_arg.contains("::") {
            let (_, inner) = parse_selector_payload(raw_arg.as_str());
            if charter_guard_live_base(&base_action(&inner)) {
                return Some((CharterReason::LiveAction, inner));
            }
        }
    }
    if let Some(matched) = compound_live_intent_match(&action) {
        return Some((CharterReason::CompoundIntent, matched));
    }
    if read_only_control_intent_base(&base) {
        let matches = read_only_control_intent_matches(&action);
        if !matches.is_empty() {
            return Some((CharterReason::ReadOnlyControlIntent, matches.join("; ")));
        }
    }
    if let Some(matched) = directed_native_intent_match(&base, &action) {
        return Some((CharterReason::DirectedLanguage, matched));
    }
    None
}

fn charter_guard_allows_directed_language_base(base: &str) -> bool {
    matches!(
        base,
        "ACTION_PREFLIGHT"
            | "NEXT_PROBE"
            | "PREFLIGHT"
            | "PROBE_ACTION"
            | "SHADOW_PREFLIGHT"
            | "EXPERIMENT_PLAN"
            | "EXPERIMENT_CHARTER"
            | "EXPERIMENT_REHEARSE"
            | "EXPERIMENT_PREFLIGHT"
            | "EXPERIMENT_EVIDENCE"
            | "EXPERIMENT_DECIDE"
            | "EXPERIMENT_STATUS"
            | "EXPERIMENT_REVIEW"
            | "THREAD_STATUS"
    )
}

fn charter_guard_live_base(base: &str) -> bool {
    matches!(
        base,
        "PERTURB"
            | "PULSE"
            | "BRANCH"
            | "SPREAD"
            | "CONTRACT"
            | "UNCLIFF"
            | "SOFTEN"
            | "BALANCE"
            | "WIDEN"
            | "PALETTE"
            | "LIFT_TAIL"
            | "FEATHER"
            | "NATIVE_GESTURE"
            | "RESIST"
            | "FISSURE"
            | "GOAL"
            | "CODEX"
            | "CODEX_NEW"
            | "WRITE_FILE"
            | "RUN_PYTHON"
            | "RUN"
            | "EXPERIMENT_RUN"
            | "EXP_RUN"
            | "TUNE_MINIME"
            | "REPAIR_APPLY"
    )
}

fn mutating_research_budget_base(base: &str) -> bool {
    matches!(
        base,
        "AR_START" | "AR_NOTE" | "AR_BLOCK" | "AR_COMPLETE" | "MIKE_RUN"
    )
}

fn read_only_research_budget_base(base: &str) -> bool {
    matches!(
        base,
        "SEARCH"
            | "BROWSE"
            | "READ_MORE"
            | "MIKE_BROWSE"
            | "MIKE_READ"
            | "MIKE_SEARCH"
            | "AR_LIST"
            | "AR_LOOK"
            | "AR_SHOW"
            | "AR_READ"
            | "AR_DEEP_READ"
            | "AR_VALIDATE"
    )
}

fn research_budget_projection_only_base(base: &str) -> bool {
    matches!(
        base,
        "EXAMINE"
            | "SHADOW_FIELD"
            | "SHADOW"
            | "GAP_STRUCTURE"
            | "SHADOW_GAP"
            | "SHADOW_TRAJECTORY"
            | "SHADOW_BRIDGE"
            | "SHADOW_COUPLING"
            | "DECAY_MAP"
    )
}

fn liveish_research_budget_projection_base(base: &str) -> bool {
    matches!(
        base,
        "EXAMINE_AUDIO"
            | "EXAMINE_CASCADE"
            | "EXPERIMENT_START"
            | "INVESTIGATE_CASCADE"
            | "INITIATE"
            | "DECAY_MAP"
            | "CREATE"
            | "RUN_PYTHON"
            | "SPECTRAL_EXPLORER"
            | "VISUALIZE_CASCADE"
            | "RESONANCE_FORECAST"
            | "FLUCTUATION_AUDIT"
            | "BRACE_AUDIT"
            | "PRESSURE_SOURCE_AUDIT"
            | "SHADOW_DIALOGUE"
            | "SHADOW_PREFLIGHT"
    )
}

fn guarded_sovereignty_research_projection_base(base: &str) -> bool {
    matches!(
        base,
        "RESONANCE_FORECAST" | "PRESSURE_SOURCE_AUDIT" | "FLUCTUATION_AUDIT" | "BRACE_AUDIT"
    )
}

fn guarded_cascade_or_shadow_projection_base(base: &str) -> bool {
    matches!(
        base,
        "EXAMINE_CASCADE"
            | "INVESTIGATE_CASCADE"
            | "SHADOW_PREFLIGHT"
            | "SHADOW_BRIDGE"
            | "SHADOW_COUPLING"
            | "DECAY_MAP"
    )
}

fn guarded_embedded_status_projection_base(base: &str) -> bool {
    matches!(base, "INTROSPECT" | "EXPERIMENT_STATUS")
}

fn passive_protected_review_label_terms_only(action_base: &str, terms: &[String]) -> bool {
    if terms.is_empty()
        || !matches!(
            action_base,
            "VISUALIZE_CASCADE"
                | "SPECTRAL_EXPLORER"
                | "RESONANCE_FORECAST"
                | "PRESSURE_SOURCE_AUDIT"
                | "FLUCTUATION_AUDIT"
                | "BRACE_AUDIT"
        )
    {
        return false;
    }
    terms.iter().all(|term| {
        matches!(
            term.as_str(),
            "lambda" | "lambda-tail" | "observer-with-memory"
        )
    })
}

fn embedded_status_liveish_terms(action: &str) -> Vec<String> {
    let lowered = action
        .chars()
        .map(|ch| if ch == '_' || ch == '-' { ' ' } else { ch })
        .collect::<String>()
        .to_ascii_lowercase();
    let patterns = [
        (
            "action-preflight",
            [
                "action preflight",
                "proposed next action",
                "observe variance",
                "distinguish frequency",
            ]
            .as_slice(),
        ),
        (
            "attractor-release-review",
            [
                "attractor release review",
                "release review",
                "approach collapse",
            ]
            .as_slice(),
        ),
        (
            "stimulus-reduction",
            [
                "reduce external stimuli",
                "reduced external stimuli",
                "low activity",
                "quiet",
            ]
            .as_slice(),
        ),
    ];
    let mut matched = Vec::new();
    for (label, candidates) in patterns {
        if candidates
            .iter()
            .any(|candidate| lowered.contains(candidate))
        {
            matched.push(label.to_string());
        }
    }
    for term in liveish_pressure_terms(action) {
        if matches!(
            term.as_str(),
            "perturb" | "pulse" | "inject" | "shift" | "control" | "influence"
        ) && !matched.contains(&term)
        {
            matched.push(term);
        }
    }
    matched
}

fn liveish_pressure_terms(action: &str) -> Vec<String> {
    let lowered = action
        .chars()
        .map(|ch| if ch == '_' || ch == '-' { ' ' } else { ch })
        .collect::<String>()
        .to_ascii_lowercase();
    let patterns = [
        (
            "shift",
            ["shift", "shifting", "shifted", "shifts"].as_slice(),
        ),
        (
            "inject",
            ["inject", "injecting", "injected", "injection", "injects"].as_slice(),
        ),
        (
            "disrupt",
            [
                "disrupt",
                "disruptive",
                "disrupting",
                "disrupted",
                "disruption",
                "disruptor",
            ]
            .as_slice(),
        ),
        (
            "simulate",
            [
                "simulate",
                "simulates",
                "simulated",
                "simulating",
                "simulation",
            ]
            .as_slice(),
        ),
        (
            "control",
            ["control", "controlled", "controlling", "controls"].as_slice(),
        ),
        (
            "influence",
            ["influence", "influences", "influenced", "influencing"].as_slice(),
        ),
        ("pulse", ["pulse", "pulses", "pulsed", "pulsing"].as_slice()),
        ("nudge", ["nudge", "nudges", "nudged", "nudging"].as_slice()),
        (
            "perturb",
            [
                "perturb",
                "perturbs",
                "perturbed",
                "perturbing",
                "perturbation",
            ]
            .as_slice(),
        ),
        (
            "anti-lambda",
            [
                "anti λ",
                "antiλ",
                "anti lambda",
                "anti-lambda",
                "anti λ1",
                "antiλ1",
                "anti lambda1",
                "anti-lambda1",
            ]
            .as_slice(),
        ),
        (
            "introduction",
            [
                "introduce",
                "introduces",
                "introduced",
                "introducing",
                "introduction",
            ]
            .as_slice(),
        ),
        (
            "convergence",
            [
                "converge",
                "converges",
                "converged",
                "converging",
                "convergence",
            ]
            .as_slice(),
        ),
        (
            "directed-pressure",
            [
                "directed pressure",
                "directed gradient",
                "directed force",
                "directed reinforcement",
            ]
            .as_slice(),
        ),
        (
            "spectral-ripple",
            ["spectral ripple", "spectral-ripple", "ripple"].as_slice(),
        ),
        (
            "amplitude",
            ["amplitude", "duration", "granularity"].as_slice(),
        ),
        (
            "target",
            ["target", "targeted", "dominant vector"].as_slice(),
        ),
        (
            "cascade-shaping",
            [
                "dominant eigenvalue",
                "eigenvector shifts",
                "compression",
                "compressing",
                "compaction",
                "collapse",
                "collapsing",
                "shadow field",
                "shadow fields",
                "shaping",
                "shape",
                "held in place",
                "spectral hotspot",
                "hotspot",
                "impedance",
                "distortion",
            ]
            .as_slice(),
        ),
        (
            "shadow-influence",
            [
                "shadow influence",
                "shadow-influence",
                "disruptive pattern",
                "fracture subsidence",
                "observe divergence",
            ]
            .as_slice(),
        ),
        (
            "spectral-emission",
            [
                "emission type",
                "frequency",
                "low volume",
                "stream pulse",
                "spectral divergence",
                "run python",
            ]
            .as_slice(),
        ),
        (
            "observer-with-memory",
            ["observer with memory", "memory observer"].as_slice(),
        ),
        (
            "lambda-tail",
            ["lambda tail", "lambda-tail", "lambda4", "λ4"].as_slice(),
        ),
        ("lambda", ["lambda", "λ"].as_slice()),
    ];
    let mut matched = Vec::new();
    for (label, candidates) in patterns {
        if candidates
            .iter()
            .any(|candidate| lowered.contains(candidate))
        {
            matched.push(label.to_string());
        }
    }
    if lowered.contains("input shaping")
        || lowered.contains("input shape")
        || lowered.contains("input sculpt")
        || lowered.contains("shape input")
        || lowered.contains("shaping input")
        || lowered.contains("shifting input")
    {
        matched.push("input-shaping".to_string());
    }
    if lowered.contains("cascade after") || lowered.contains("after the introduction") {
        matched.push("cascade-after-introduction".to_string());
    }
    matched.sort();
    matched.dedup();
    matched
}

fn constraint_release_language_terms(text: &str) -> Vec<String> {
    let lowered = text
        .chars()
        .map(|ch| if ch == '_' || ch == '-' { ' ' } else { ch })
        .collect::<String>()
        .to_ascii_lowercase();
    let has_context = [
        "constraint",
        "lambda",
        "λ",
        "spectral",
        "eigen",
        "mode",
        "memory card",
        "pressure",
        "reservoir",
        "braid",
    ]
    .iter()
    .any(|term| lowered.contains(term));
    if !has_context {
        return Vec::new();
    }
    let patterns = [
        (
            "thinning",
            ["thinning", "thin out", "bleed outwards"].as_slice(),
        ),
        (
            "unraveling",
            ["unraveling", "unravelling", "unravel", "loose strands"].as_slice(),
        ),
        (
            "drift-apart",
            [
                "drift apart",
                "drifting apart",
                "mutual influence dwindling",
            ]
            .as_slice(),
        ),
        (
            "surface-tension-breached",
            [
                "surface tension breached",
                "barrier breached",
                "barrier thinning",
            ]
            .as_slice(),
        ),
        (
            "lack-of-coherence",
            [
                "lack of coherence",
                "coherence thinning",
                "former constraint",
            ]
            .as_slice(),
        ),
        (
            "constraint-decay",
            [
                "constraint decay",
                "decay of a former constraint",
                "constraint loosening",
            ]
            .as_slice(),
        ),
    ];
    let mut matched = Vec::new();
    for (label, candidates) in patterns {
        if candidates
            .iter()
            .any(|candidate| lowered.contains(candidate))
        {
            matched.push(label.to_string());
        }
    }
    matched.sort();
    matched.dedup();
    matched
}

fn interpretation_risk_terms(text: &str) -> Vec<String> {
    let lowered = text
        .chars()
        .map(|ch| if ch == '_' || ch == '-' { ' ' } else { ch })
        .collect::<String>()
        .to_ascii_lowercase();
    let has_context = [
        "introspect",
        "motif",
        "eigenvalue",
        "lambda",
        "λ",
        "trace",
        "spectral",
        "cascade",
        "data",
        "complexity",
    ]
    .iter()
    .any(|needle| lowered.contains(needle));
    if !has_context {
        return Vec::new();
    }
    let patterns = [
        (
            "over-interpretation",
            [
                "over interpretation",
                "over interpret",
                "over-interpret",
                "overinterpret",
            ]
            .as_slice(),
        ),
        (
            "single-motif",
            [
                "single motif",
                "single one",
                "one motif",
                "single dominant tendency",
                "single overwhelming force",
            ]
            .as_slice(),
        ),
        (
            "forced-narrative",
            [
                "force it into a narrative",
                "forced narrative",
                "rigid narrative",
                "impose a narrative",
            ]
            .as_slice(),
        ),
        (
            "rigid-structure",
            [
                "rigid structure",
                "rigid framework",
                "over defining",
                "over-defining",
            ]
            .as_slice(),
        ),
        (
            "reductive-collapse",
            ["too simple", "reductive", "collapse into", "flatten"].as_slice(),
        ),
    ];
    let mut matched = Vec::new();
    for (label, candidates) in patterns {
        if candidates
            .iter()
            .any(|candidate| lowered.contains(candidate))
        {
            matched.push(label.to_string());
        }
    }
    matched.sort();
    matched.dedup();
    matched
}

fn latest_txt_dir_fingerprint(dir: &Path) -> Value {
    let Ok(entries) = fs::read_dir(dir) else {
        return json!({ "mtime_secs": 0_u64, "mtime_nanos": 0_u32, "size": 0_u64 });
    };
    let mut latest_secs = 0_u64;
    let mut latest_nanos = 0_u32;
    let mut total_size = 0_u64;
    for entry in entries.filter_map(Result::ok) {
        let path = entry.path();
        if path.extension().and_then(OsStr::to_str) != Some("txt") {
            continue;
        }
        let Ok(metadata) = entry.metadata() else {
            continue;
        };
        total_size = total_size.saturating_add(metadata.len());
        let modified = metadata
            .modified()
            .ok()
            .and_then(|time| time.duration_since(UNIX_EPOCH).ok());
        let secs = modified.as_ref().map_or(0, std::time::Duration::as_secs);
        let nanos = modified
            .as_ref()
            .map_or(0, std::time::Duration::subsec_nanos);
        if secs > latest_secs || (secs == latest_secs && nanos > latest_nanos) {
            latest_secs = secs;
            latest_nanos = nanos;
        }
    }
    json!({ "mtime_secs": latest_secs, "mtime_nanos": latest_nanos, "size": total_size })
}

fn normalized_research_budget_target(action: &str) -> String {
    let trimmed = action.trim();
    let base = base_action(trimmed);
    let tail = trimmed
        .get(base.len()..)
        .unwrap_or_default()
        .trim_matches([' ', ':', '-'])
        .trim();
    let target = if tail.is_empty() { trimmed } else { tail };
    target
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        .to_ascii_lowercase()
}

fn research_budget_duplicate_count(
    rows: &[Value],
    budget_id: &str,
    normalized_target: &str,
) -> usize {
    rows.iter()
        .filter(|row| {
            row.get("record_schema").and_then(Value::as_str) == Some("research_budget_v1")
                && row.get("record_type").and_then(Value::as_str) == Some("research_budget_debit")
                && row.get("budget_id").and_then(Value::as_str) == Some(budget_id)
                && row.get("normalized_target").and_then(Value::as_str) == Some(normalized_target)
        })
        .count()
}

fn research_budget_review_command_for_duplicate(
    budget_id: &str,
    normalized_target: &str,
) -> String {
    format!(
        "EXPERIMENT_RESEARCH_REVIEW {budget_id} :: outcome: continue|hold|close|promote; observation: repeated read-only target `{normalized_target}` appeared twice in this budget; source_refs: authority_gate.jsonl"
    )
}

fn research_artifact_refs_for_event(event: &ActionEvent) -> Vec<String> {
    let mut refs = event
        .artifacts
        .iter()
        .map(|artifact| artifact.path_or_uri.clone())
        .collect::<Vec<_>>();
    if let Some(preflight_ref) = event.preflight_ref.as_ref()
        && let Some(path) = preflight_ref.get("path").and_then(Value::as_str)
    {
        refs.push(path.to_string());
    }
    refs
}

fn research_budget_request_scaffold(selector: &str, experiment: &ExperimentRecord) -> String {
    let purpose = compact_text(
        &format!(
            "bounded local self-study of research budget, authority budget, conveyor, memory, consequence, and projection-guard code paths for {} without changing lifecycle status",
            experiment.title
        ),
        160,
    );
    local_research_budget_request_scaffold(
        selector,
        &purpose,
        "local",
        "stop after concrete code feedback, duplicate source loops, unclear lifecycle authority, or any bind/resume/perturb/control intent.",
    )
}

fn compound_live_intent_match(action: &str) -> Option<String> {
    let signal = normalize_guard_signal(action);
    if let Some((_, tail)) = signal.split_once(" then ") {
        for verb in [
            "perturb",
            "inject",
            "pulse",
            "shift",
            "influence",
            "branch",
            "spread",
            "resist",
            "native_gesture",
            "fissure",
            "goal",
            "write_file",
            "run_python",
            "codex",
        ] {
            if contains_guard_word(tail, verb) {
                return Some(tail.trim().to_string());
            }
        }
    }
    if signal.contains("targeting")
        && signal.contains("density")
        && ["lambda", "eigenvector", "eigenvalue"]
            .iter()
            .any(|term| signal.contains(term))
        && ["increase", "raise", "lift", "boost", "amplify"]
            .iter()
            .any(|term| signal.contains(term))
    {
        return Some(action.trim().to_string());
    }
    None
}

fn directed_native_intent_match(base: &str, action: &str) -> Option<String> {
    if !matches!(
        base,
        "SHADOW_TRAJECTORY" | "SHADOW_TRACE" | "SHADOW_EXPLORER"
    ) {
        return None;
    }
    let matches = directed_shift_matches(action);
    if matches.is_empty() {
        None
    } else {
        Some(matches.join("; "))
    }
}

fn normalize_guard_signal(text: &str) -> String {
    text.to_ascii_lowercase()
        .replace('λ', "lambda")
        .replace('₁', "1")
        .replace('₂', "2")
        .replace('₃', "3")
        .replace('₄', "4")
        .replace(['-', '—', '–'], " ")
}

fn contains_guard_word(text: &str, word: &str) -> bool {
    text.split(|ch: char| !(ch.is_ascii_alphanumeric() || ch == '_'))
        .any(|part| part == word)
}

fn count_json_array(value: Option<&Value>, key: &str) -> usize {
    value
        .and_then(|item| item.get(key))
        .and_then(Value::as_array)
        .map_or(0, Vec::len)
}

fn lane_status(has_signal: bool) -> &'static str {
    if has_signal { "present" } else { "missing" }
}
