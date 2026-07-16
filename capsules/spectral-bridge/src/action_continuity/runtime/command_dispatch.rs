pub fn handle_thread_next_action(
    db: &BridgeDb,
    base_action: &str,
    original: &str,
    response_text: &str,
    telemetry: &SpectralTelemetry,
    fill_pct: f32,
) -> Option<Result<String>> {
    let store = ActionContinuityStore::for_astrid_workspace();
    match crate::action_self_knowledge::handle_action(store.root(), base_action, original) {
        Ok(Some(message)) => return Some(Ok(message)),
        Ok(None) => {},
        Err(err) => return Some(Err(err)),
    }
    if matches!(
        base_action,
        "ACTION_STATUS" | "JOB_STATUS" | "ACTION_CANCEL"
    ) {
        let selector = strip_action_arg(original, base_action);
        let selector = if selector.is_empty() {
            None
        } else {
            Some(selector.as_str())
        };
        return Some(if base_action == "ACTION_CANCEL" {
            crate::llm_jobs::cancel(selector)
        } else {
            crate::llm_jobs::status_text(selector)
        });
    }
    let state = spectral_state(fill_pct, telemetry);
    match base_action {
        "THREAD_START" => {
            let title = strip_action_arg(original, base_action);
            let title = if title.is_empty() {
                "Untitled action thread"
            } else {
                title.as_str()
            };
            Some(
                store
                    .create_thread(Some(db), title, Some(&derive_why_return(response_text)))
                    .map(|thread| {
                        format!(
                            "Started action thread `{}`: {}",
                            thread.thread_id, thread.title
                        )
                    }),
            )
        },
        "THREADS" => Some(store.list_threads(8).map(|threads| {
            if threads.is_empty() {
                return "No action threads yet. Use THREAD_START <title>.".to_string();
            }
            threads
                .into_iter()
                .map(|thread| {
                    format!(
                        "- {} [{}]: {}",
                        thread.thread_id, thread.status, thread.title
                    )
                })
                .collect::<Vec<_>>()
                .join("\n")
        })),
        "THREAD_STATUS" => {
            let selector = strip_action_arg(original, base_action);
            Some(store.thread_status(if selector.is_empty() {
                None
            } else {
                Some(selector.as_str())
            }))
        },
        "THREAD_NOTE" => {
            let raw = strip_action_arg(original, base_action);
            let (selector, note) = parse_thread_note(&raw);
            Some(
                store
                    .append_note(Some(db), selector.as_deref(), &note, state)
                    .map(|event| format!("Thread note recorded as `{}`.", event.action_id)),
            )
        },
        "RESUME" => {
            let selector = strip_action_arg(original, base_action);
            Some(store.resume_thread(selector.as_str()))
        },
        "SAVEPOINT" => {
            let name = strip_action_arg(original, base_action);
            let name = if name.is_empty() {
                "current"
            } else {
                name.as_str()
            };
            Some(store.savepoint(name, state))
        },
        "RECALL" => {
            let name = strip_action_arg(original, base_action);
            let name = if name.is_empty() {
                "current"
            } else {
                name.as_str()
            };
            Some(store.recall(name))
        },
        "EXPERIMENT_START" => {
            let raw = strip_action_arg(original, base_action);
            Some(store.experiment_start_command(Some(db), &raw))
        },
        "EXPERIMENT_BRANCH" => {
            let raw = strip_action_arg(original, base_action);
            Some(store.experiment_branch_command(Some(db), &raw))
        },
        "EXPERIMENT_RESUME" => {
            let selector = strip_action_arg(original, base_action);
            Some(store.experiment_resume_command(Some(db), optional_selector(&selector)))
        },
        "EXPERIMENT_COMPARE" => {
            let selector = strip_action_arg(original, base_action);
            Some(store.experiment_compare_command(optional_selector(&selector)))
        },
        "EXPERIMENT_ALT_PATHS" => {
            let selector = strip_action_arg(original, base_action);
            Some(store.experiment_alt_paths(optional_selector(&selector)))
        },
        "SHARED_INVESTIGATION_START" => {
            let raw = strip_action_arg(original, base_action);
            Some(store.shared_investigation_start_command(Some(db), &raw))
        },
        "SHARED_INVESTIGATION_STATUS" => {
            let selector = strip_action_arg(original, base_action);
            Some(store.shared_investigation_status(optional_selector(&selector)))
        },
        "SHARED_INVESTIGATION_CLAIM" => {
            let raw = strip_action_arg(original, base_action);
            Some(store.shared_investigation_claim_command(&raw))
        },
        "SHARED_INVESTIGATION_DECIDE" => {
            let raw = strip_action_arg(original, base_action);
            Some(store.shared_investigation_decide_command(Some(db), &raw))
        },
        "EXPERIMENT_PLAN" => {
            let selector = strip_action_arg(original, base_action);
            Some(
                repair_experiment_command_arg(
                    &store,
                    Some(db),
                    base_action,
                    original,
                    &selector,
                    &state,
                )
                .and_then(|(selector, notice, _focus)| {
                    store
                        .experiment_plan(optional_selector(&selector))
                        .map(|message| format!("{}{}", notice.unwrap_or_default(), message))
                }),
            )
        },
        "EXPERIMENT_ADVANCE" | "EXPERIMENT_CONVEYOR" => {
            let raw = strip_action_arg(original, base_action);
            Some(store.experiment_advance_command(Some(db), &raw, state))
        },
        "MEMORY_STATUS" => {
            let selector = strip_action_arg(original, base_action);
            Some(store.memory_status_command(optional_selector(&selector)))
        },
        "MEMORY_RECALL" => {
            let raw = strip_action_arg(original, base_action);
            Some(store.memory_recall_command(&raw))
        },
        "MEMORY_CAPTURE" => {
            let raw = strip_action_arg(original, base_action);
            Some(store.memory_capture_command(&raw))
        },
        "MEMORY_PROMOTE" => {
            let raw = strip_action_arg(original, base_action);
            Some(store.memory_promote_command(&raw, state))
        },
        "EXPERIMENT_AUTHORITY_PREPARE" => {
            let raw = strip_action_arg(original, base_action);
            Some(store.experiment_authority_prepare_command(Some(db), &raw, state))
        },
        "EXPERIMENT_AUTHORITY_REQUEST" => {
            let raw = strip_action_arg(original, base_action);
            Some(store.experiment_authority_request_command(Some(db), &raw, state))
        },
        "EXPERIMENT_AUTHORITY_STATUS" => {
            let selector = strip_action_arg(original, base_action);
            Some(store.experiment_authority_status_command(
                Some(db),
                optional_selector(&selector),
                state,
            ))
        },
        "EXPERIMENT_AUTHORITY_EXECUTE" => {
            let request_id = strip_action_arg(original, base_action);
            Some(store.experiment_authority_execute_command(Some(db), &request_id, state))
        },
        "EXPERIMENT_AUTHORITY_BUDGET_REQUEST" => {
            let raw = strip_action_arg(original, base_action);
            Some(store.experiment_authority_budget_request_command(Some(db), &raw, state))
        },
        "EXPERIMENT_AUTHORITY_BUDGET_STATUS" => {
            let selector = strip_action_arg(original, base_action);
            Some(store.experiment_authority_budget_status_command(
                Some(db),
                optional_selector(&selector),
                state,
            ))
        },
        "EXPERIMENT_AUTHORITY_REVIEW" => {
            let raw = strip_action_arg(original, base_action);
            Some(store.experiment_authority_review_command(Some(db), &raw, state))
        },
        "EXPERIMENT_RESEARCH_BUDGET_ACCEPT" | "EXPERIMENT_RESEARCH_BUDGET_USE_SCAFFOLD" => {
            let selector = strip_action_arg(original, base_action);
            Some(store.experiment_research_budget_accept_command(
                Some(db),
                optional_selector(&selector),
                state,
            ))
        },
        "EXPERIMENT_RESEARCH_BUDGET_REQUEST" => {
            let raw = strip_action_arg(original, base_action);
            Some(store.experiment_research_budget_request_command(Some(db), &raw, state))
        },
        "EXPERIMENT_RESEARCH_BUDGET_STATUS" => {
            let selector = strip_action_arg(original, base_action);
            Some(store.experiment_research_budget_status_command(
                Some(db),
                optional_selector(&selector),
                state,
            ))
        },
        "EXPERIMENT_RESEARCH_REVIEW" => {
            let raw = strip_action_arg(original, base_action);
            Some(store.experiment_research_review_command(Some(db), &raw, state))
        },
        "EXPERIMENT_LOOP_REQUEST" => {
            let raw = strip_action_arg(original, base_action);
            Some(store.experiment_loop_request_command(Some(db), &raw, state))
        },
        "EXPERIMENT_LOOP_STATUS" => {
            let selector = strip_action_arg(original, base_action);
            Some(store.experiment_loop_status_command(
                Some(db),
                optional_selector(&selector),
                state,
            ))
        },
        "EXPERIMENT_LOOP_STEP" => {
            let raw = strip_action_arg(original, base_action);
            Some(store.experiment_loop_step_command(Some(db), &raw, state))
        },
        "EXPERIMENT_LOOP_REVIEW" => {
            let raw = strip_action_arg(original, base_action);
            Some(store.experiment_loop_review_command(Some(db), &raw, state))
        },
        "ACCEPT_SUGGESTED_NEXT" | "ACCEPT_SCAFFOLD" => {
            let selector = strip_action_arg(original, base_action);
            Some(store.accept_suggested_next_command(Some(db), optional_selector(&selector), state))
        },
        "CONTINUITY_SESSION_ACCEPT" => {
            let raw = strip_action_arg(original, base_action);
            Some(store.continuity_session_accept_command(&raw))
        },
        "CONTINUITY_SESSION_START" => {
            let raw = strip_action_arg(original, base_action);
            Some(store.continuity_session_start_command(&raw))
        },
        "CONTINUITY_SESSION_CAPTURE" => {
            let raw = strip_action_arg(original, base_action);
            Some(store.continuity_session_capture_command(&raw))
        },
        "CONTINUITY_SESSION_SUMMARIZE" => {
            let raw = strip_action_arg(original, base_action);
            Some(store.continuity_session_summarize_command(&raw))
        },
        "CONTINUITY_SESSION_FINALIZE" => {
            let raw = strip_action_arg(original, base_action);
            Some(store.continuity_session_finalize_command(&raw))
        },
        "CONTINUITY_SESSION_RESUME" => {
            let raw = strip_action_arg(original, base_action);
            Some(store.continuity_session_resume_command(&raw))
        },
        "CONTINUITY_SESSION_STATUS" => {
            let raw = strip_action_arg(original, base_action);
            Some(store.continuity_session_status_command(&raw))
        },
        "EXPERIMENT_CHARTER" => {
            let raw = strip_action_arg(original, base_action);
            Some(
                repair_experiment_command_arg(
                    &store,
                    Some(db),
                    base_action,
                    original,
                    &raw,
                    &state,
                )
                .and_then(|(raw, notice, _focus)| {
                    let (selector, prose) = parse_selector_payload(&raw);
                    if empty_or_placeholder_payload(&prose) || !charter_payload_has_meaning(&prose)
                    {
                        return Ok(format!(
                            "{}{}",
                            notice.unwrap_or_default(),
                            experiment_intent_repair_prompt(base_action, selector.as_deref())
                        ));
                    }
                    store
                        .experiment_charter(Some(db), selector.as_deref(), &prose)
                        .map(|experiment| {
                            format!(
                                "{}Experiment charter recorded for `{}`. Next: {}",
                                notice.unwrap_or_default(),
                                experiment.experiment_id,
                                experiment
                                    .planned_next
                                    .as_deref()
                                    .unwrap_or("EXPERIMENT_REHEARSE current")
                            )
                        })
                }),
            )
        },
        "EXPERIMENT_REHEARSE" | "EXPERIMENT_PREFLIGHT" => {
            let selector = strip_action_arg(original, base_action);
            Some(
                repair_experiment_command_arg(
                    &store,
                    Some(db),
                    base_action,
                    original,
                    &selector,
                    &state,
                )
                .and_then(|(selector, notice, focus)| {
                    if let Some(focus) = focus.as_deref() {
                        let thread = store.ensure_active_thread(Some(db))?;
                        let experiment = store.resolve_experiment(&thread, Some("current"))?;
                        let state_text = state.clone();
                        let pseudo_run = ExperimentRunRecord {
                            schema_version: SCHEMA_VERSION,
                            run_id: String::new(),
                            experiment_id: experiment.experiment_id.clone(),
                            source: "experiment_intent_repair".to_string(),
                            action_text: format!("ACTION_PREFLIGHT {focus}"),
                            stage: "read_only".to_string(),
                            status: "candidate_context".to_string(),
                            gate_decision: json!({"source": "experiment_intent_repair"}),
                            pre_state: state_text.clone(),
                            post_state: state_text,
                            artifacts: Vec::new(),
                            result_summary: format!("Repaired preflight focus: {focus}"),
                            interpretation:
                                "Preflight focus preserved as advisory workbench candidate context."
                                    .to_string(),
                            suggested_next: Some("EXPERIMENT_REHEARSE current".to_string()),
                            created_at: iso_now(),
                            updated_at: iso_now(),
                            motif_allowance_v1: None,
                        };
                        let _ = store.refresh_workbench_candidates(
                            Some(db),
                            &thread,
                            &experiment,
                            Some(&pseudo_run),
                            Some(focus),
                            "experiment_intent_repair",
                        )?;
                    }
                    store
                        .experiment_rehearse(Some(db), optional_selector(&selector), state)
                        .map(|run| {
                            format!(
                                "{}Experiment rehearsal recorded as `{}` [{}].",
                                notice.unwrap_or_default(),
                                run.run_id,
                                run.status
                            )
                        })
                }),
            )
        },
        "EXPERIMENT_EVIDENCE" => {
            let raw = strip_action_arg(original, base_action);
            Some(
                repair_experiment_command_arg(
                    &store,
                    Some(db),
                    base_action,
                    original,
                    &raw,
                    &state,
                )
                .and_then(|(raw, notice, _focus)| {
                    let (selector, note) = parse_selector_payload(&raw);
                    if empty_or_placeholder_payload(&note) {
                        return Ok(format!(
                            "{}{}",
                            notice.unwrap_or_default(),
                            experiment_intent_repair_prompt(base_action, selector.as_deref())
                        ));
                    }
                    store
                        .experiment_evidence(Some(db), selector.as_deref(), &note, state)
                        .map(|run| {
                            format!(
                                "{}Experiment evidence recorded as `{}`.",
                                notice.unwrap_or_default(),
                                run.run_id
                            )
                        })
                }),
            )
        },
        "EXPERIMENT_DECIDE" => {
            let raw = strip_action_arg(original, base_action);
            Some(
                repair_experiment_command_arg(
                    &store,
                    Some(db),
                    base_action,
                    original,
                    &raw,
                    &state,
                )
                .and_then(|(raw, notice, _focus)| {
                    let (selector, decision) = parse_selector_payload(&raw);
                    if empty_or_placeholder_payload(&decision) {
                        return Ok(format!(
                            "{}{}",
                            notice.unwrap_or_default(),
                            experiment_intent_repair_prompt(base_action, selector.as_deref())
                        ));
                    }
                    store
                        .experiment_decide(Some(db), selector.as_deref(), &decision)
                        .map(|experiment| {
                            format!(
                                "{}Experiment `{}` decision recorded; status={} next={}",
                                notice.unwrap_or_default(),
                                experiment.experiment_id,
                                experiment.status,
                                experiment.planned_next.as_deref().unwrap_or("(none)")
                            )
                        })
                }),
            )
        },
        "EXPERIMENT_OBSERVE" => {
            let raw = strip_action_arg(original, base_action);
            let (selector, note) = parse_selector_payload(&raw);
            if let Some(peer) = selector.as_deref().and_then(peer_experiment_ref) {
                Some(store.record_peer_experiment_reference(
                    Some(db),
                    &peer,
                    "EXPERIMENT_OBSERVE",
                    Some(&note),
                ))
            } else {
                Some(
                    store
                        .experiment_observe(Some(db), selector.as_deref(), &note, state)
                        .map(|run| format!("Experiment observation recorded as `{}`.", run.run_id)),
                )
            }
        },
        "EXPERIMENT_STATUS" => {
            let selector = strip_action_arg(original, base_action);
            Some(store.experiment_status(optional_selector(&selector)))
        },
        "EXPERIMENT_REVIEW" => {
            let selector = strip_action_arg(original, base_action);
            Some(store.experiment_review(optional_selector(&selector)))
        },
        "DOSSIER_CLAIM" => {
            let raw = strip_action_arg(original, base_action);
            Some(store.dossier_claim_command(Some(db), &raw))
        },
        "DOSSIER_EVIDENCE" => {
            let raw = strip_action_arg(original, base_action);
            Some(store.dossier_evidence_command(Some(db), &raw))
        },
        "DOSSIER_STATUS" => {
            let selector = strip_action_arg(original, base_action);
            Some(store.dossier_status(optional_selector(&selector)))
        },
        "DOSSIER_REVIEW" => {
            let selector = strip_action_arg(original, base_action);
            Some(store.dossier_review(optional_selector(&selector)))
        },
        "EXPERIMENT_CLOSE" => {
            let raw = strip_action_arg(original, base_action);
            let (selector, summary) = parse_selector_payload(&raw);
            Some(
                store
                    .close_experiment(Some(db), selector.as_deref(), &summary)
                    .map(|experiment| {
                        format!(
                            "Experiment `{}` marked {}: {}",
                            experiment.experiment_id,
                            experiment.status,
                            experiment.success_observation.as_deref().unwrap_or("")
                        )
                    }),
            )
        },
        "EXPERIMENT_PEER_REVIEW" => {
            let selector = strip_action_arg(original, base_action);
            Some(store.experiment_peer_review(Some(db), optional_selector(&selector)))
        },
        _ => None,
    }
}

pub fn parse_experiment_bind(original: &str) -> Result<(Option<String>, String)> {
    let raw = strip_action_arg(original, "EXPERIMENT_BIND");
    if !raw.contains("::") {
        anyhow::bail!("EXPERIMENT_BIND needs `::` before the inner NEXT action.");
    }
    let (selector, action) = parse_selector_payload(&raw);
    if action.trim().is_empty() {
        anyhow::bail!("EXPERIMENT_BIND needs an inner NEXT action after `::`.");
    }
    Ok((selector, action))
}

pub fn is_peer_experiment_selector(selector: &str) -> bool {
    peer_experiment_ref(selector).is_some()
}

pub fn is_experiment_control_action(action: &str) -> bool {
    let base = base_action(action);
    matches!(
        base.as_str(),
        "EXPERIMENT"
            | "EXPERIMENT_START"
            | "EXPERIMENT_PLAN"
            | "EXPERIMENT_ADVANCE"
            | "EXPERIMENT_CONVEYOR"
            | "MEMORY_STATUS"
            | "MEMORY_RECALL"
            | "MEMORY_CAPTURE"
            | "MEMORY_PROMOTE"
            | "EXPERIMENT_AUTHORITY_REQUEST"
            | "EXPERIMENT_AUTHORITY_PREPARE"
            | "EXPERIMENT_AUTHORITY_STATUS"
            | "EXPERIMENT_AUTHORITY_EXECUTE"
            | "EXPERIMENT_AUTHORITY_BUDGET_REQUEST"
            | "EXPERIMENT_AUTHORITY_BUDGET_STATUS"
            | "EXPERIMENT_AUTHORITY_REVIEW"
            | "EXPERIMENT_RESEARCH_BUDGET_ACCEPT"
            | "EXPERIMENT_RESEARCH_BUDGET_USE_SCAFFOLD"
            | "EXPERIMENT_RESEARCH_BUDGET_REQUEST"
            | "EXPERIMENT_RESEARCH_BUDGET_STATUS"
            | "EXPERIMENT_RESEARCH_REVIEW"
            | "EXPERIMENT_LOOP_REQUEST"
            | "EXPERIMENT_LOOP_STATUS"
            | "EXPERIMENT_LOOP_STEP"
            | "EXPERIMENT_LOOP_REVIEW"
            | "ACCEPT_SUGGESTED_NEXT"
            | "ACCEPT_SCAFFOLD"
            | "CONTINUITY_SESSION_ACCEPT"
            | "CONTINUITY_SESSION_START"
            | "CONTINUITY_SESSION_CAPTURE"
            | "CONTINUITY_SESSION_SUMMARIZE"
            | "CONTINUITY_SESSION_FINALIZE"
            | "CONTINUITY_SESSION_RESUME"
            | "CONTINUITY_SESSION_STATUS"
            | "EXPERIMENT_CHARTER"
            | "EXPERIMENT_REHEARSE"
            | "EXPERIMENT_PREFLIGHT"
            | "EXPERIMENT_EVIDENCE"
            | "EXPERIMENT_DECIDE"
            | "EXPERIMENT_BIND"
            | "EXPERIMENT_OBSERVE"
            | "EXPERIMENT_STATUS"
            | "EXPERIMENT_REVIEW"
            | "EXPERIMENT_CLOSE"
            | "EXPERIMENT_PEER_REVIEW"
            | "EXPERIMENT_BRANCH"
            | "EXPERIMENT_RESUME"
            | "EXPERIMENT_COMPARE"
            | "EXPERIMENT_ALT_PATHS"
            | "SHARED_INVESTIGATION_START"
            | "SHARED_INVESTIGATION_STATUS"
            | "SHARED_INVESTIGATION_CLAIM"
            | "SHARED_INVESTIGATION_DECIDE"
            | "DOSSIER_CLAIM"
            | "DOSSIER_EVIDENCE"
            | "DOSSIER_STATUS"
            | "DOSSIER_REVIEW"
    )
}

pub fn record_experiment_bind_run(
    db: &BridgeDb,
    selector: Option<&str>,
    inner_action: &str,
    outcome: &NextActionOutcome,
    fill_pct: f32,
    telemetry: &SpectralTelemetry,
) -> Result<ExperimentRunRecord> {
    ActionContinuityStore::for_astrid_workspace().record_experiment_bind_run(
        Some(db),
        selector,
        inner_action,
        outcome,
        fill_pct,
        telemetry,
    )
}

pub fn record_legacy_experiment_run(
    db: &BridgeDb,
    action_text: &str,
    outcome: &NextActionOutcome,
    fill_pct: f32,
    telemetry: &SpectralTelemetry,
) -> Result<ExperimentRunRecord> {
    ActionContinuityStore::for_astrid_workspace().record_legacy_experiment_run(
        Some(db),
        action_text,
        outcome,
        fill_pct,
        telemetry,
    )
}

pub fn record_astrid_next_action(
    db: &BridgeDb,
    raw_next: &str,
    canonical_next: &str,
    effective_next: &str,
    outcome: &NextActionOutcome,
    fill_pct: f32,
    telemetry: &SpectralTelemetry,
    response_text: &str,
) -> Result<ActionEvent> {
    ActionContinuityStore::for_astrid_workspace().record_next_event(
        Some(db),
        raw_next,
        canonical_next,
        effective_next,
        outcome,
        fill_pct,
        telemetry,
        response_text,
    )
}

pub fn charter_required_guard_for_next(
    raw_next: &str,
) -> Result<Option<CharterRequiredGuardAssessment>> {
    ActionContinuityStore::for_astrid_workspace().charter_required_guard_assessment(raw_next)
}

pub fn research_budget_guard_for_next(
    raw_next: &str,
    fill_pct: f32,
    telemetry: &SpectralTelemetry,
) -> Result<Option<ResearchBudgetGuardAssessment>> {
    ActionContinuityStore::for_astrid_workspace()
        .research_budget_guard_assessment(raw_next, fill_pct, telemetry)
}
