fn contains_any(text: &str, needles: &[&str]) -> bool {
    needles.iter().any(|needle| text.contains(needle))
}

fn card_scalar_text(value: &Value, key: &str) -> String {
    match value.get(key) {
        Some(Value::String(text)) if !text.is_empty() => text.clone(),
        Some(Value::Number(number)) => number.to_string(),
        Some(Value::Bool(flag)) => flag.to_string(),
        _ => "(none)".to_string(),
    }
}

pub fn prompt_summary() -> Option<String> {
    let store = ActionContinuityStore::for_astrid_workspace();
    let thread = store.current_thread().ok().flatten()?;
    let projection = store.thread_projection(&thread).ok()?;
    let recent = projection
        .recent_event_summaries
        .iter()
        .take(3)
        .map(|summary| format!("  - {summary}"))
        .collect::<Vec<_>>()
        .join("\n");
    let resonance = thread
        .thread_resonance_density_v1
        .as_ref()
        .map(|value| {
            format!(
                "Thread resonance: {} aggregate={} density_ema={} pressure_ema={}\n",
                value
                    .get("quality")
                    .and_then(Value::as_str)
                    .unwrap_or("open_experiment"),
                value
                    .get("aggregate")
                    .map_or_else(|| "n/a".to_string(), Value::to_string),
                value
                    .get("density_ema")
                    .map_or_else(|| "n/a".to_string(), Value::to_string),
                value
                    .get("pressure_ema")
                    .map_or_else(|| "n/a".to_string(), Value::to_string),
            )
        })
        .unwrap_or_else(|| {
            format!(
                "Active experiment: none\n{}",
                last_experiment_context_line(&thread)
            )
        });
    let pressure = thread
        .thread_pressure_source_v1
        .as_ref()
        .map(|value| {
            format!(
                "Thread pressure source: {} aggregate={} dominant={} porosity_ema={}\n",
                value
                    .get("quality")
                    .and_then(Value::as_str)
                    .unwrap_or("mixed_thread_pressure"),
                value
                    .get("aggregate")
                    .map_or_else(|| "n/a".to_string(), Value::to_string),
                value
                    .get("dominant_source")
                    .and_then(Value::as_str)
                    .unwrap_or("unknown"),
                value
                    .get("porosity_ema")
                    .map_or_else(|| "n/a".to_string(), Value::to_string),
            )
        })
        .unwrap_or_default();
    let fluctuation = thread
        .thread_inhabitable_fluctuation_v1
        .as_ref()
        .map(|value| {
            format!(
                "Thread fluctuation: {} inhabitability_ema={} fluctuation_ema={} foothold_ema={}\n",
                value
                    .get("quality")
                    .and_then(Value::as_str)
                    .unwrap_or("open_experiment"),
                value
                    .get("inhabitability_ema")
                    .map_or_else(|| "n/a".to_string(), Value::to_string),
                value
                    .get("fluctuation_ema")
                    .map_or_else(|| "n/a".to_string(), Value::to_string),
                value
                    .get("foothold_ema")
                    .map_or_else(|| "n/a".to_string(), Value::to_string),
            )
        })
        .unwrap_or_default();
    let experiment = projection
        .active_experiment
        .as_ref()
        .map(|active| {
            format!(
                "Active experiment: {} ({}) question={} planned_next={}\nLifecycle: {}\n{}\n{}\n{}{}{}Workbench reminder: author a charter, rehearse before live, record felt plus telemetry/artifact evidence, then accept/refuse/counter/pause/complete. Ordinary choices remain valid.\n",
                active.experiment.title,
                active.experiment.experiment_id,
                active.experiment.question,
                active
                    .experiment
                    .planned_next
                    .as_deref()
                    .unwrap_or("(none)"),
                active.classification,
                active.charter_status,
                active.evidence_status,
                if active.candidate_status.trim().is_empty() {
                    String::new()
                } else {
                    format!("{}\n", active.candidate_status)
                },
                first_dossier_claim_line(&active.first_dossier_claim_cue_v1),
                research_dossier_line(&active.research_dossier_v1, Some(&active.classification)),
            )
        })
        .unwrap_or_default();
    let allowance = thread
        .motif_allowance_v1
        .as_ref()
        .map(|value| {
            format!(
                "Motif allowance: {} dominant={} action_concentration={} returnability={}\nAllowance culture: deepen, branch, compare, release, rest, or hold space are all valid; branching preserves the original return point.\n",
                value
                    .get("quality")
                    .and_then(Value::as_str)
                    .unwrap_or("open_basin"),
                value
                    .get("dominant_motif")
                    .and_then(Value::as_str)
                    .unwrap_or("open inquiry"),
                value
                    .get("action_base_concentration")
                    .map_or_else(|| "n/a".to_string(), Value::to_string),
                value
                    .get("returnability")
                    .map_or_else(|| "n/a".to_string(), Value::to_string),
            )
        })
        .unwrap_or_default();
    let preflight = preflight_recommendation_line(&thread);
    let continuity_return = projection.continuity_return_line.clone();
    let native_return = native_return_cue_line(&projection.native_continuity_v1);
    let safety_cue = preflight_safety_cue_line(&projection.preflight_safety_cue_v1);
    let read_only_control_cue =
        read_only_control_intent_cue_line(&projection.read_only_control_intent_cue_v1);
    let constraint_counterfactual_cue =
        constraint_counterfactual_cue_line(&projection.constraint_counterfactual_cue_v1);
    let charter_now_bridge = charter_now_bridge_line(&projection.charter_now_bridge_v1);
    let prior_claim_bridge =
        prior_claim_charter_bridge_line(&projection.prior_claim_charter_bridge_v1);
    let charter_preflight_not_charter =
        charter_preflight_not_charter_line(&projection.charter_preflight_not_charter_cue_v1);
    let peer_boundary = peer_mutation_boundary_line(&projection.peer_mutation_boundary_cue_v1);
    let first_dossier_claim = first_dossier_claim_line(&projection.first_dossier_claim_cue_v1);
    let shared_investigation = shared_investigation_line(&projection.shared_investigation_v1);
    let shared_investigation_object =
        shared_investigation_object_line(&projection.shared_investigation_object_v1);
    let voice_health = voice_health_line();
    let research_budget_priority = store.research_budget_priority_line(&thread, &projection);
    let sovereign_loop = ActionContinuityStore::sovereign_loop_line(&projection.sovereign_loop_v1);
    let control_plane = control_plane_text(&projection.continuity_control_plane_v1);
    let research_dossier = research_dossier_line(
        &projection.research_dossier_v1,
        projection
            .active_experiment
            .as_ref()
            .map(|active| active.classification.as_str()),
    );
    let stale_notice = store.stale_projection_line(&projection);
    let proposal_diagnostics = if projection.top_actionable_proposals.is_empty() {
        String::new()
    } else {
        format!(
            "Proposal diagnostics: {}\n",
            projection
                .top_actionable_proposals
                .iter()
                .take(3)
                .map(|diag| format!("{} x{} -> {}", diag.verb, diag.count, diag.suggested_route))
                .collect::<Vec<_>>()
                .join("; ")
        )
    };
    Some(format!(
        "Current action thread: {} ({})\nWhy return: {}\n{}{}{}{}{}{}{}{}{}{}{}Current NEXT: {}\n{}{}{}{}{}{}{}{}{}{}{}{}{}{}Recent thread events:\n{}\nThread actions available: THREAD_START, THREADS, THREAD_STATUS, THREAD_NOTE, RESUME, SAVEPOINT, RECALL.\n{}\nRead-only research actions auto-link when an experiment is active; dossier/shared/memory/session actions preserve referable claims without changing lifecycle or granting peer authority.",
        thread.title,
        thread.thread_id,
        thread.why_return,
        charter_now_bridge,
        prior_claim_bridge,
        charter_preflight_not_charter,
        peer_boundary,
        first_dossier_claim,
        shared_investigation,
        shared_investigation_object,
        voice_health,
        research_budget_priority,
        sovereign_loop,
        research_dossier,
        projection_current_next_display(&projection, thread.current_next.as_deref()),
        control_plane,
        resonance,
        pressure,
        fluctuation,
        experiment,
        allowance,
        continuity_return,
        native_return,
        safety_cue,
        read_only_control_cue,
        constraint_counterfactual_cue,
        stale_notice,
        proposal_diagnostics,
        preflight,
        if recent.is_empty() {
            "  - none yet"
        } else {
            recent.as_str()
        },
        control_plane_command_palette_text()
    ))
}
