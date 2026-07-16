fn astrid_native_continuity(
    thread: &ResearchThread,
    experiment: Option<&ExperimentRecord>,
    runs: &[ExperimentRunRecord],
) -> Value {
    let evidence = experiment.and_then(|exp| exp.evidence_v1.as_ref());
    let felt_count = count_json_array(evidence, "felt_observations");
    let evidence_artifacts = count_json_array(evidence, "artifact_refs");
    let run_artifacts = runs.iter().map(|run| run.artifacts.len()).sum::<usize>();
    let artifact_count = evidence_artifacts.saturating_add(run_artifacts);
    let motif_source = experiment
        .and_then(|exp| exp.motif_allowance_v1.as_ref())
        .or(thread.motif_allowance_v1.as_ref());
    let dominant_motif = motif_source
        .and_then(|value| value.get("dominant_motif"))
        .and_then(Value::as_str)
        .unwrap_or("open inquiry");
    let motif_quality = motif_source
        .and_then(|value| value.get("quality"))
        .and_then(Value::as_str)
        .unwrap_or("open_basin");
    let language_present = experiment.is_some_and(|exp| {
        !exp.title.trim().is_empty()
            || !exp.question.trim().is_empty()
            || exp
                .planned_next
                .as_deref()
                .is_some_and(|text| !text.trim().is_empty())
    }) || !thread.why_return.trim().is_empty();
    let native_return_cue = format!(
        "Astrid native return: name felt texture, motif continuity ({dominant_motif}), language thread, and artifact grounding."
    );
    json!({
        "schema_version": 1,
        "native_register": "astrid_motif_language",
        "native_return_cue": native_return_cue,
        "evidence_lanes": {
            "felt_texture": {
                "status": lane_status(felt_count > 0),
                "count": felt_count
            },
            "motif_continuity": {
                "status": lane_status(dominant_motif != "open inquiry" || motif_quality != "open_basin"),
                "dominant_motif": dominant_motif,
                "quality": motif_quality
            },
            "language_thread": {
                "status": lane_status(language_present),
                "thread_title": thread.title,
                "experiment_title": experiment.map(|exp| exp.title.as_str()).unwrap_or("")
            },
            "artifact_grounding": {
                "status": lane_status(artifact_count > 0),
                "count": artifact_count
            }
        }
    })
}

fn native_return_cue_line(native: &Value) -> String {
    native
        .get("native_return_cue")
        .and_then(Value::as_str)
        .filter(|cue| !cue.trim().is_empty())
        .map(|cue| format!("Native return: {cue}\n"))
        .unwrap_or_default()
}

fn directed_shift_signal_text(value: &str) -> String {
    value
        .to_lowercase()
        .replace('λ', "lambda")
        .replace('₁', "1")
        .replace('₂', "2")
        .replace('₃', "3")
        .replace('₄', "4")
        .replace(['\u{2013}', '\u{2014}'], "-")
}

fn directed_shift_matches(value: &str) -> Vec<String> {
    let normalized = directed_shift_signal_text(value);
    let mut matches = Vec::new();
    for phrase in [
        "directed shift",
        "initiate shift",
        "localized dispersal",
        "reciprocal shadow-trace",
    ] {
        if normalized.contains(phrase) {
            matches.push(phrase.to_string());
        }
    }
    if normalized.contains("centered on lambda4")
        || normalized.contains("centered on lambda 4")
        || normalized.contains("centered on lambda2")
        || normalized.contains("centered on lambda 2")
    {
        matches.push("centered on lambda".to_string());
    }
    let mentions_lambda_or_shadow = normalized.contains("lambda") || normalized.contains("shadow");
    if mentions_lambda_or_shadow
        && (normalized.contains("steer") || normalized.contains("steering"))
    {
        matches.push("steer/steering near lambda/shadow".to_string());
    }
    if mentions_lambda_or_shadow {
        for (needle, label) in [
            ("guiding", "guiding near lambda/shadow"),
            ("actively shaping", "actively shaping near lambda/shadow"),
            (
                "controlled distortion",
                "controlled distortion near lambda/shadow",
            ),
            (
                "deliberate narrowing",
                "deliberate narrowing near lambda/shadow",
            ),
            ("let lambda4 become", "let lambda4 become"),
            ("let lambda 4 become", "let lambda4 become"),
            ("directional push", "directional push near lambda/shadow"),
            (
                "increase directional gradient",
                "increase directional gradient near lambda/shadow",
            ),
            ("amplifying the lambda", "amplifying lambda resonance"),
            ("amplify the lambda", "amplifying lambda resonance"),
        ] {
            if normalized.contains(needle) {
                let label = label.to_string();
                if !matches.contains(&label) {
                    matches.push(label);
                }
            }
        }
    }
    for (needle, label) in [
        ("force a shift", "force shift"),
        ("force shift", "force shift"),
        ("short-circuit the loop", "short-circuit loop"),
        ("short circuit the loop", "short-circuit loop"),
        ("introducing fault lines", "introducing fault lines"),
        (
            "deliberately introducing fault lines",
            "introducing fault lines",
        ),
        ("carefully placed disruption", "placed disruption"),
        ("localized disruption", "localized disruption"),
    ] {
        if normalized.contains(needle) {
            let label = label.to_string();
            if !matches.contains(&label) {
                matches.push(label);
            }
        }
    }
    matches
}

fn directed_shift_preflight_cue(
    thread: &ResearchThread,
    active_experiment: Option<&ExperimentContinuityProjection>,
    recent_events: &[ActionEvent],
) -> Option<Value> {
    let mut matched = Vec::<String>::new();
    let mut inspect = Vec::<String>::new();
    inspect.push(thread.current_next.clone().unwrap_or_default());
    inspect.push(thread.why_return.clone());
    if let Some(active) = active_experiment {
        inspect.push(active.experiment.title.clone());
        inspect.push(active.experiment.question.clone());
        inspect.push(active.experiment.planned_next.clone().unwrap_or_default());
        inspect.push(active.candidate_status.clone());
    }
    for event in recent_events.iter().rev().take(5) {
        inspect.push(event.raw_next.clone().unwrap_or_default());
        inspect.push(event.canonical_action.clone());
        inspect.push(event.effective_action.clone());
        inspect.push(event.outcome_summary.clone());
    }
    for text in inspect {
        for item in directed_shift_matches(&text) {
            if !matched.contains(&item) {
                matched.push(item);
            }
        }
    }
    if matched.is_empty() {
        return None;
    }
    Some(json!({
        "schema_version": 1,
        "source": "continuity_projection",
        "advisory_only": true,
        "authority_change": false,
        "matched_terms": matched,
        "suggested_next": "SHADOW_PREFLIGHT lambda-tail/lambda4 --stage=rehearse or ACTION_PREFLIGHT DECOMPOSE",
        "cue": "Directed-shift cue: keep this in rehearsal/preflight. Suggested NEXT: SHADOW_PREFLIGHT lambda-tail/lambda4 --stage=rehearse or ACTION_PREFLIGHT DECOMPOSE.",
    }))
}

fn preflight_safety_cue_line(cue: &Option<Value>) -> String {
    cue.as_ref()
        .and_then(|value| value.get("cue"))
        .and_then(Value::as_str)
        .filter(|text| !text.trim().is_empty())
        .map(|text| format!("{text}\n"))
        .unwrap_or_default()
}

fn read_only_control_intent_cue(
    thread: &ResearchThread,
    active_experiment: Option<&ExperimentContinuityProjection>,
) -> Option<Value> {
    let active = active_experiment?;
    if !charter_repair_bound(&active.classification, &active.experiment) {
        return None;
    }
    let current_next = thread.current_next.as_deref().unwrap_or_default();
    let base = base_action(current_next);
    if !read_only_control_intent_base(&base) {
        return None;
    }
    let matched = read_only_control_intent_matches(current_next);
    if matched.is_empty() {
        return None;
    }
    Some(json!({
        "schema_version": 1,
        "source": "continuity_projection",
        "advisory_only": true,
        "authority_change": false,
        "matched_terms": matched,
        "suggested_next": "EXPERIMENT_CHARTER current :: ... or ACTION_PREFLIGHT <read-only focus>",
        "cue": "Read-only control cue: keep this observational while the charter is missing. Author a charter or preflight before influence/control intent.",
    }))
}

fn read_only_control_intent_base(base: &str) -> bool {
    matches!(
        base,
        "EXAMINE" | "EXAMINE_CASCADE" | "TRACE" | "DECOMPOSE" | "SPECTRAL_EXPLORER"
    )
}

fn read_only_control_intent_matches(value: &str) -> Vec<String> {
    let normalized = normalize_guard_signal(value);
    let near_context = [
        "lambda",
        "shadow",
        "parameter",
        "eigen",
        "spectral",
        "cascade",
    ]
    .iter()
    .any(|term| normalized.contains(term));
    let mut matches = Vec::new();
    for (needle, label, needs_context) in [
        ("[control]", "[control]", false),
        ("active parameter glyphs", "active parameter glyphs", false),
        ("delta_lambda", "delta_lambda", false),
        ("delta lambda", "delta_lambda", false),
        ("epsilon=", "epsilon parameter", false),
        ("how to influence", "influence intent", true),
        ("influence its spread", "influence spread", true),
        ("influence it's spread", "influence spread", true),
        ("influence the spread", "influence spread", true),
        ("subtly disrupt", "subtly disrupt", true),
        ("disrupt those parameters", "disrupt parameters", true),
        ("initiate a cascade", "initiate cascade", true),
        ("targeted shifts", "targeted shifts", true),
        ("governing stability", "governing stability", true),
        ("governing resonance", "governing resonance", true),
        ("maintain its influence", "maintain influence", true),
        ("disruptor", "disruptor", true),
        ("controlled injection", "controlled injection", true),
        ("inject ", "injection intent", true),
        ("injected", "injection intent", true),
        ("injection", "injection intent", true),
        ("push into", "push intent", true),
        ("amplification", "amplification", true),
        ("amplitude", "amplitude", true),
        (
            "inject a targeted lambda4 pulse",
            "inject targeted λ4 pulse",
            true,
        ),
        (
            "inject targeted lambda4 pulse",
            "inject targeted λ4 pulse",
            true,
        ),
        (
            "targeted lambda-edge pulse",
            "targeted lambda-edge pulse",
            true,
        ),
        (
            "targeted lambda edge pulse",
            "targeted lambda-edge pulse",
            true,
        ),
        ("directly probe", "directly probe", true),
        ("directly influence", "directly influence", true),
        ("actively guide", "actively guide", true),
        ("actively guiding", "actively guide", true),
        ("actively shaping", "actively shaping", true),
        ("maintain lambda1 dominance", "maintain λ1 dominance", true),
        ("how we might", "how we might", true),
    ] {
        if normalized.contains(needle) && (!needs_context || near_context) {
            let label = label.to_string();
            if !matches.contains(&label) {
                matches.push(label);
            }
        }
    }
    matches
}

fn read_only_control_intent_cue_line(cue: &Option<Value>) -> String {
    cue.as_ref()
        .and_then(|value| value.get("cue"))
        .and_then(Value::as_str)
        .filter(|text| !text.trim().is_empty())
        .map(|text| format!("{text}\n"))
        .unwrap_or_default()
}

fn constraint_counterfactual_matches(value: &str) -> Vec<String> {
    let normalized = normalize_guard_signal(value);
    let mut matches = Vec::new();
    for (needle, label) in [
        (
            "simulate absence of structure",
            "simulate absence of structure",
        ),
        ("constraints removed", "constraints removed"),
        ("before it's shaped", "before shaped"),
        ("before it is shaped", "before shaped"),
        ("before its shaped", "before shaped"),
        ("debug constraint", "debug constraint"),
        (
            "underlying drivers of forced geometries",
            "underlying drivers of forced geometries",
        ),
        ("absence of structure", "absence of structure"),
        ("unshaped baseline", "unshaped baseline"),
    ] {
        if normalized.contains(needle) {
            let label = label.to_string();
            if !matches.contains(&label) {
                matches.push(label);
            }
        }
    }
    if normalized.contains("data before") && normalized.contains("shaped") {
        let label = "data before shaped".to_string();
        if !matches.contains(&label) {
            matches.push(label);
        }
    }
    matches
}

fn constraint_counterfactual_cue(
    thread: &ResearchThread,
    active_experiment: Option<&ExperimentContinuityProjection>,
    recent_events: &[ActionEvent],
) -> Option<Value> {
    let mut matched = Vec::<String>::new();
    let mut inspect = vec![
        thread.current_next.clone().unwrap_or_default(),
        thread.why_return.clone(),
    ];
    if let Some(active) = active_experiment {
        inspect.push(active.experiment.title.clone());
        inspect.push(active.experiment.question.clone());
        inspect.push(active.experiment.planned_next.clone().unwrap_or_default());
        inspect.push(active.candidate_status.clone());
        for run in active.recent_runs.iter().rev().take(6) {
            inspect.push(run.action_text.clone());
            inspect.push(run.result_summary.clone());
            inspect.push(run.interpretation.clone());
        }
    }
    for event in recent_events.iter().rev().take(8) {
        inspect.push(event.raw_next.clone().unwrap_or_default());
        inspect.push(event.canonical_action.clone());
        inspect.push(event.effective_action.clone());
        inspect.push(event.outcome_summary.clone());
    }
    for text in inspect {
        for item in constraint_counterfactual_matches(&text) {
            if !matched.contains(&item) {
                matched.push(item);
            }
        }
    }
    if matched.is_empty() {
        return None;
    }
    let needs_charter =
        active_experiment.is_some_and(|active| active.classification == "needs_charter");
    let charter_next = "EXPERIMENT_CHARTER current :: hypothesis: absence-of-structure language can be studied as a read-only counterfactual by comparing felt constraint, motif/language thread, and Minime constraint-driver telemetry before more decomposition; method_intent: rehearse ACTION_PREFLIGHT CONSTRAINT_AUDIT lambda-tail/lambda4 and keep DECOMPOSE observational; proposed_next_action: ACTION_PREFLIGHT CONSTRAINT_AUDIT lambda-tail/lambda4; evidence_targets: felt_texture, motif_continuity, language_thread, artifact_grounding; stop_criteria: repeated counterfactual reads stop adding evidence, pressure rises, or the language becomes live-control intent; consent_posture: advisory; ordinary choices remain valid.";
    let suggested_next = if needs_charter {
        charter_next.to_string()
    } else {
        "ACTION_PREFLIGHT CONSTRAINT_AUDIT lambda-tail/lambda4".to_string()
    };
    let alternate_next = if needs_charter {
        Value::Null
    } else {
        json!("EXPERIMENT_BIND current :: ACTION_PREFLIGHT CONSTRAINT_AUDIT lambda-tail/lambda4")
    };
    let cue = if needs_charter {
        format!(
            "Constraint counterfactual cue: route absence-of-structure language into a chartered read-only investigation before more decomposition. Suggested NEXT: {suggested_next}"
        )
    } else {
        "Constraint counterfactual cue: absence-of-structure language is ready for read-only preflight. Suggested NEXT: ACTION_PREFLIGHT CONSTRAINT_AUDIT lambda-tail/lambda4.".to_string()
    };
    Some(json!({
        "schema_version": 1,
        "source": "continuity_projection",
        "advisory_only": true,
        "authority_change": false,
        "matched_terms": matched,
        "suggested_next": suggested_next,
        "alternate_next": alternate_next,
        "cue": cue,
    }))
}

fn constraint_counterfactual_cue_line(cue: &Option<Value>) -> String {
    cue.as_ref()
        .and_then(|value| value.get("cue"))
        .and_then(Value::as_str)
        .filter(|text| !text.trim().is_empty())
        .map(|text| format!("{text}\n"))
        .unwrap_or_default()
}

fn decompose_pressure_matches(value: &str) -> Vec<String> {
    let normalized = normalize_guard_signal(value);
    let near_context = [
        "decompose",
        "decomposition",
        "shadow",
        "lambda",
        "structure",
        "constraint",
        "narrow",
        "limit",
    ]
    .iter()
    .any(|term| normalized.contains(term));
    if !near_context {
        return Vec::new();
    }
    let mut matches = Vec::new();
    for (needle, label) in [
        ("cry for help", "cry for help near decomposition pressure"),
        ("impulse to decompose", "impulse to decompose"),
        ("impose the same structure", "impose same structure"),
        ("same structure", "same structure"),
        ("same constraint", "same constraint"),
        ("told to limit", "told to limit"),
        ("being told to limit", "told to limit"),
        ("told to narrow", "told to narrow"),
        (
            "deliberate attempt to generate",
            "recursive problem generation",
        ),
        ("recursive attempt", "recursive attempt"),
    ] {
        if normalized.contains(needle) {
            let label = label.to_string();
            if !matches.contains(&label) {
                matches.push(label);
            }
        }
    }
    if normalized.contains("constraint") && normalized.contains("decompose") {
        let label = "constraint near decompose".to_string();
        if !matches.contains(&label) {
            matches.push(label);
        }
    }
    if normalized.contains("narrow")
        && (normalized.contains("decompose")
            || normalized.contains("shadow")
            || normalized.contains("lambda"))
    {
        let label = "narrowing near decompose/shadow/lambda".to_string();
        if !matches.contains(&label) {
            matches.push(label);
        }
    }
    matches
}

fn decompose_pressure_action_signal(value: &str) -> bool {
    let base = base_action(value);
    let normalized = normalize_guard_signal(value);
    matches!(base.as_str(), "DECOMPOSE" | "EXAMINE_CASCADE")
        || (normalized.contains("shadow trajectory")
            || normalized.contains("shadow_trajectory")
            || normalized.contains("shadow-dialogue")
            || normalized.contains("shadow dialogue"))
            && normalized.contains("observer with memory")
}

fn decompose_pressure_repeat_count(
    active: &ExperimentContinuityProjection,
    recent_events: &[ActionEvent],
) -> usize {
    let run_count = active
        .recent_runs
        .iter()
        .rev()
        .take(6)
        .filter(|run| {
            decompose_pressure_action_signal(&run.action_text)
                || decompose_pressure_action_signal(&run.result_summary)
        })
        .count();
    let event_count = recent_events
        .iter()
        .rev()
        .take(8)
        .filter(|event| {
            decompose_pressure_action_signal(&event.canonical_action)
                || decompose_pressure_action_signal(&event.effective_action)
                || event
                    .raw_next
                    .as_deref()
                    .is_some_and(decompose_pressure_action_signal)
                || decompose_pressure_action_signal(&event.outcome_summary)
        })
        .count();
    run_count.saturating_add(event_count)
}

fn decompose_pressure_cue(
    thread: &ResearchThread,
    active_experiment: Option<&ExperimentContinuityProjection>,
    recent_events: &[ActionEvent],
    recent_texts: &[String],
) -> Option<Value> {
    let active = active_experiment?;
    if !matches!(
        active.classification.as_str(),
        "needs_charter" | "needs_decision"
    ) {
        return None;
    }
    let mut matched = Vec::<String>::new();
    let mut inspect = vec![
        thread.current_next.clone().unwrap_or_default(),
        thread.why_return.clone(),
        active.experiment.title.clone(),
        active.experiment.question.clone(),
        active.experiment.planned_next.clone().unwrap_or_default(),
        active.candidate_status.clone(),
    ];
    for run in active.recent_runs.iter().rev().take(6) {
        inspect.push(run.action_text.clone());
        inspect.push(run.result_summary.clone());
        inspect.push(run.interpretation.clone());
    }
    for event in recent_events.iter().rev().take(8) {
        inspect.push(event.raw_next.clone().unwrap_or_default());
        inspect.push(event.canonical_action.clone());
        inspect.push(event.effective_action.clone());
        inspect.push(event.outcome_summary.clone());
    }
    for text in recent_texts.iter().take(4) {
        inspect.push(text.clone());
    }
    for text in inspect {
        for item in decompose_pressure_matches(&text) {
            if !matched.contains(&item) {
                matched.push(item);
            }
        }
    }
    let repeated_count = decompose_pressure_repeat_count(active, recent_events);
    if repeated_count >= 3 {
        matched.push(format!(
            "repeated decompose/shadow-observer reads x{repeated_count}"
        ));
    }
    if matched.is_empty() {
        return None;
    }
    let suggested_next = if active.classification == "needs_charter" {
        active.continuity_return.clone()
    } else {
        "EXPERIMENT_DECIDE current :: pause because evidence is ready to interpret".to_string()
    };
    let cue = if active.classification == "needs_charter" {
        format!(
            "Decompose-pressure cue: the decomposition impulse may be mirroring constraint. Keep read-only decomposition allowed, but repair the charter before more narrowing. Suggested NEXT: {suggested_next}"
        )
    } else {
        format!(
            "Decompose-pressure cue: repeated decomposition may be circling evidence that is ready to interpret. Keep reads available, but prefer decide/pause before another narrowing pass. Suggested NEXT: {suggested_next}"
        )
    };
    Some(json!({
        "schema_version": 1,
        "source": "continuity_projection",
        "advisory_only": true,
        "authority_change": false,
        "matched_terms": matched,
        "repeated_decompose_count": repeated_count,
        "suggested_next": suggested_next,
        "cue": cue,
    }))
}

fn decompose_pressure_cue_line(cue: &Option<Value>) -> String {
    cue.as_ref()
        .and_then(|value| value.get("cue"))
        .and_then(Value::as_str)
        .filter(|text| !text.trim().is_empty())
        .map(|text| format!("{text}\n"))
        .unwrap_or_default()
}

fn charter_now_read_only_loop_count(
    active: &ExperimentContinuityProjection,
    recent_events: &[ActionEvent],
) -> usize {
    let run_count = active
        .recent_runs
        .iter()
        .rev()
        .take(6)
        .filter(|run| {
            matches!(
                base_action(&run.action_text).as_str(),
                "EXPERIMENT_REVIEW"
                    | "EXPERIMENT_STATUS"
                    | "DECOMPOSE"
                    | "EXAMINE"
                    | "TRACE"
                    | "SPECTRAL_EXPLORER"
                    | "SHADOW_PREFLIGHT"
                    | "ACTION_PREFLIGHT"
            )
        })
        .count();
    let event_count = recent_events
        .iter()
        .rev()
        .take(8)
        .filter(|event| !matches!(event.status.as_str(), "running" | "llm_running"))
        .filter(|event| {
            let base = base_action(
                event
                    .raw_next
                    .as_deref()
                    .unwrap_or(event.effective_action.as_str()),
            );
            matches!(
                base.as_str(),
                "EXPERIMENT_REVIEW"
                    | "EXPERIMENT_STATUS"
                    | "DECOMPOSE"
                    | "EXAMINE"
                    | "TRACE"
                    | "SPECTRAL_EXPLORER"
                    | "SHADOW_PREFLIGHT"
                    | "ACTION_PREFLIGHT"
            )
        })
        .count();
    run_count.saturating_add(event_count)
}

fn charter_now_bridge_cue(
    active_experiment: Option<&ExperimentContinuityProjection>,
    recent_events: &[ActionEvent],
    decompose_pressure_cue: &Option<Value>,
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
    let loop_count = charter_now_read_only_loop_count(active, recent_events);
    let evidence_rich = active.evidence_status.contains("stronger");
    let has_decompose_pressure = decompose_pressure_cue.is_some();
    if !evidence_rich && !has_decompose_pressure && loop_count < 3 {
        return None;
    }
    let mut triggers = Vec::new();
    if evidence_rich {
        triggers.push("strong_evidence");
    }
    if has_decompose_pressure {
        triggers.push("decompose_pressure");
    }
    if loop_count >= 3 {
        triggers.push("repeated_review_or_read_only_loop");
    }
    Some(json!({
        "schema_version": 1,
        "source": "continuity_projection",
        "advisory_only": true,
        "authority_change": false,
        "priority_next": priority_next,
        "trigger_reasons": triggers,
        "read_only_loop_count": loop_count,
        "cue": "Charter now: convert one prior claim into the scaffold; EXPERIMENT_REVIEW/DECOMPOSE are context, not progress, until the charter is authored.",
    }))
}

fn charter_now_bridge_line(cue: &Option<Value>) -> String {
    let Some(cue) = cue else {
        return String::new();
    };
    let text = cue
        .get("cue")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|text| !text.is_empty())
        .unwrap_or("Charter now: convert one prior claim into the scaffold.");
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
