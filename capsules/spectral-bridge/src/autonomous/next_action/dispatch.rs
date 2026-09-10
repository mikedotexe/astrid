// Authorized NEXT dispatch, kept separate from parsing and test registry.
fn record_activity_choice(conv: &mut ConversationState, base: &str) -> Option<NextActionOutcome> {
    if base == "BROWSE" && conv.browse_url.is_none() && !conv.wants_search {
        return None;
    }
    match super::activity_reading::observe_chosen_action(conv, base) {
        Ok(()) => None,
        Err(error) => {
            let message = format!(
                "The chosen activity could not save its reading return: {error:#}. Inspect ACTIVITY_STATUS before continuing."
            );
            conv.pending_file_listing = Some(message.clone());
            Some(
                NextActionOutcome::blocked("activity", message)
                    .with_stage_visibility("blocked", "protected_summary"),
            )
        },
    }
}

fn handle_next_action_with_author(
    conv: &mut ConversationState,
    next_action: &str,
    mut ctx: NextActionContext<'_>,
    author: introspection_cadence::NextActionAuthorV1,
) -> NextActionOutcome {
    // v4.0 Phase 1 — Multi-NEXT detection. Astrid already emits chained
    // actions like "BROWSE arxiv AND READ_MORE" naturally; previously the
    // post-AND segment was silently dropped. When at least two action-like
    // segments are detected, dispatch each in order with the same shared
    // NextActionContext (each segment sees state from the previous one).
    let unwrapped = unwrap_outer_action_wrappers(next_action);
    let segments = if leading_action_token(&unwrapped) == "AFTERIMAGE_KEEP" {
        vec![unwrapped.clone()]
    } else {
        split_multi_action(&unwrapped)
    };
    if segments.len() > 1 {
        return dispatch_multi_action(conv, segments, ctx, author);
    }
    self_regulation::reconcile_active_lease(conv);
    let _ = super::self_control_v2::reconcile_if_present(conv);
    let (base_action, original) = canonicalize_next_action_components(next_action);
    if author == introspection_cadence::NextActionAuthorV1::Astrid {
        introspection_cadence::note_astrid_action(conv);
    }
    let stage = action_continuity_stage_for_base(base_action.as_str());
    let visibility = action_continuity_visibility_for_base(base_action.as_str());

    if is_action_preflight_base(base_action.as_str()) {
        let report = action_preflight_report(&original);
        let message = report.render();
        let report_value = serde_json::to_value(&report).unwrap_or_else(|_| serde_json::json!({}));
        conv.emphasis = Some(message.clone());
        return NextActionOutcome::handled("action_preflight", message)
            .with_stage_visibility("read_only", "protected_summary")
            .with_preflight_report(report_value);
    }

    if let Some(token) = unresolved_angle_placeholder(&original)
        && base_action != "AFTERIMAGE_KEEP"
        && !action_continuity::can_repair_experiment_intent_placeholder(
            base_action.as_str(),
            &original,
        )
    {
        conv.emphasis = Some(format!(
            "Your NEXT action `{original}` still contains placeholder syntax `{token}`. \
             Replace it with a concrete URL, workspace, file, command, question, or label; \
             or choose a read-only action such as STATE, FACULTIES, or SPECTRAL_EXPLORER."
        ));
        info!("Astrid NEXT placeholder rerouted without execution: {original}");
        return NextActionOutcome::blocked(
            "placeholder",
            format!("Placeholder NEXT action `{original}` was not executed."),
        );
    }

    if let Some(outcome) =
        introspection_cadence::handle_action(conv, &base_action, &original, author)
    {
        return outcome;
    }

    match action_continuity::research_budget_guard_for_next(&original, ctx.fill_pct, ctx.telemetry)
    {
        Ok(Some(guard)) => {
            let message = guard.message();
            let metadata = guard.metadata();
            conv.enqueue_runtime_feedback(
                crate::runtime_action_feedback::RuntimeActionFeedbackV1::from_guard_inputs(
                    None,
                    &original,
                    metadata
                        .get("reason")
                        .and_then(serde_json::Value::as_str)
                        .unwrap_or("research_budget_guard"),
                    &message,
                    metadata
                        .get("suggested_next")
                        .and_then(serde_json::Value::as_str),
                ),
            );
            info!(
                "Astrid research-budget guard blocked NEXT `{}` ({})",
                original,
                metadata
                    .get("reason")
                    .and_then(serde_json::Value::as_str)
                    .unwrap_or("research_budget_guard")
            );
            return NextActionOutcome::blocked("research_budget_guard", message)
                .with_stage_visibility("blocked", "protected_summary")
                .with_research_budget(metadata);
        },
        Ok(None) => {},
        Err(err) => {
            info!("Astrid research-budget guard skipped after read error: {err:#}");
        },
    }

    match action_continuity::charter_required_guard_for_next(&original) {
        Ok(Some(guard)) => {
            let message = guard.message();
            let metadata = guard.metadata();
            conv.emphasis = Some(message.clone());
            info!(
                "Astrid charter-required guard blocked NEXT `{}` ({})",
                original,
                metadata
                    .get("reason")
                    .and_then(serde_json::Value::as_str)
                    .unwrap_or("charter_required_guard")
            );
            return NextActionOutcome::blocked("charter_required_guard", message)
                .with_stage_visibility("blocked", "protected_summary")
                .with_charter_required_guard(metadata);
        },
        Ok(None) => {},
        Err(err) => {
            info!("Astrid charter-required guard skipped after read error: {err:#}");
        },
    }

    if base_action == "EXPERIMENT_AUTHORITY_EXECUTE" {
        let request_id = original
            .get("EXPERIMENT_AUTHORITY_EXECUTE".len()..)
            .unwrap_or_default()
            .trim_matches([' ', ':', '-'])
            .trim();
        match crate::authority_gate::execute_semantic_microdose(
            request_id,
            Some(ctx.fill_pct),
            None,
            ctx.sensory_tx,
        ) {
            Ok(record) => {
                let text = serde_json::to_string_pretty(&record).unwrap_or_default();
                let handled = record
                    .get("record_type")
                    .and_then(serde_json::Value::as_str)
                    == Some("execution_result");
                conv.emphasis = Some(format!("Authority gate result:\n{text}"));
                if handled {
                    return NextActionOutcome::handled("authority_gate", text)
                        .with_stage_visibility("semantic_microdose", "protected_summary");
                }
                return NextActionOutcome::blocked("authority_gate", text)
                    .with_stage_visibility("blocked", "protected_summary");
            },
            Err(err) => {
                let message = format!("Authority execute `{request_id}` blocked: {err:#}");
                conv.emphasis = Some(message.clone());
                return NextActionOutcome::blocked("authority_gate", message)
                    .with_stage_visibility("blocked", "protected_summary");
            },
        }
    }

    if base_action == "EXPERIMENT_BIND" {
        let parsed = action_continuity::parse_experiment_bind(&original);
        let (selector, inner_action) = match parsed {
            Ok(parsed) => parsed,
            Err(err) => {
                conv.emphasis = Some(format!("Experiment bind failed: {err:#}"));
                return NextActionOutcome::blocked(
                    "experiment_continuity",
                    format!("Experiment bind `{original}` failed: {err:#}"),
                )
                .with_stage_visibility("blocked", visibility);
            },
        };
        if selector
            .as_deref()
            .is_some_and(action_continuity::is_peer_experiment_selector)
        {
            let message = "EXPERIMENT_BIND cannot bind runs to a peer experiment; use a local experiment selector such as current, then request peer review.".to_string();
            conv.emphasis = Some(message.clone());
            return NextActionOutcome::blocked("experiment_continuity", message)
                .with_stage_visibility("blocked", visibility);
        }
        if action_continuity::is_experiment_control_action(&inner_action) {
            let message =
                "EXPERIMENT_BIND cannot bind experiment-control actions; choose a concrete inner action."
                    .to_string();
            conv.emphasis = Some(message.clone());
            return NextActionOutcome::blocked("experiment_continuity", message)
                .with_stage_visibility("blocked", visibility);
        }
        let inner_outcome = handle_next_action_with_author(
            conv,
            &inner_action,
            NextActionContext {
                burst_count: ctx.burst_count,
                db: ctx.db,
                sensory_tx: ctx.sensory_tx,
                telemetry: ctx.telemetry,
                fill_pct: ctx.fill_pct,
                response_text: ctx.response_text,
                workspace: ctx.workspace,
            },
            author,
        );
        let record_result = action_continuity::record_experiment_bind_run(
            ctx.db,
            selector.as_deref(),
            &inner_action,
            &inner_outcome,
            ctx.fill_pct,
            ctx.telemetry,
        );
        let message = match record_result {
            Ok(run) => format!(
                "Experiment run `{}` recorded for `{}` as {} via {}: {}",
                run.run_id,
                inner_action,
                inner_outcome.status,
                inner_outcome.route,
                inner_outcome.outcome_summary
            ),
            Err(err) => {
                conv.emphasis = Some(format!(
                    "Experiment bind executed `{inner_action}` but could not record the run: {err:#}"
                ));
                return NextActionOutcome::blocked(
                    "experiment_continuity",
                    format!(
                        "Experiment bind executed `{inner_action}` but recording failed: {err:#}"
                    ),
                )
                .with_stage_visibility("blocked", visibility);
            },
        };
        conv.emphasis = Some(message.clone());
        return NextActionOutcome::handled("experiment_continuity", message)
            .with_stage_visibility(inner_outcome.stage, inner_outcome.visibility);
    }

    if crate::transition_afterimages::is_action(&base_action) {
        let client = crate::transition_afterimages::ReaderClient::configured();
        let result = (|| -> anyhow::Result<String> {
            if base_action == "AFTERIMAGE_OPEN" {
                anyhow::ensure!(
                    conv.activity.foreground_reader.is_none()
                        && conv.activity.mailbox_window.is_none(),
                    "park the current reading or finish the selected mailbox window before opening an afterimage"
                );
                let args: Vec<_> = original.split_whitespace().collect();
                anyhow::ensure!(
                    (2..=3).contains(&args.len()),
                    "AFTERIMAGE_OPEN requires an ID and optional page"
                );
                let page = args.get(2).map_or(Ok(1), |page| page.parse::<usize>())?;
                let selected = client.invoke(
                    serde_json::json!({"operation":"select", "artifact_id":args[1], "page":page}),
                )?;
                conv.next_mode_override = Some(Mode::Dialogue);
                Ok(format!(
                    "Historical afterimage {} page {} selected intact; delivery remains pending.",
                    selected["id"], selected["page"]
                ))
            } else {
                let value = client.invoke(serde_json::json!({"action":original, "source":{
                    "timestamp_unix_ms": chrono::Utc::now().timestamp_millis(),
                    "action":base_action, "action_id":format!("afterimage_action_{:032x}", rand::random::<u128>())
                }}))?;
                Ok(value["text"]
                    .as_str()
                    .unwrap_or("Afterimage request completed.")
                    .into())
            }
        })();
        return match result {
            Ok(message) => {
                conv.pending_file_listing = Some(message.clone());
                NextActionOutcome::handled("afterimage", message)
                    .with_stage_visibility(stage, "protected_summary")
            },
            Err(error) => {
                let message = format!("Afterimage request unavailable: {error:#}");
                conv.pending_file_listing = Some(message.clone());
                NextActionOutcome::blocked("afterimage", message)
                    .with_stage_visibility("blocked", "protected_summary")
            },
        };
    }

    if let Some(result) = super::activity_reading::handle_action(conv, &base_action, &original) {
        let outcome = match result {
            Ok(message) => {
                conv.pending_file_listing = Some(message.clone());
                NextActionOutcome::handled("activity", message).with_stage_visibility(
                    if matches!(base_action.as_str(), "ACTIVITY_STATUS" | "MAILBOX_STATUS") {
                        "read_only"
                    } else {
                        "local_state"
                    },
                    "protected_summary",
                )
            },
            Err(error) => {
                let message = format!("Activity action could not complete: {error:#}");
                conv.pending_file_listing = Some(message.clone());
                NextActionOutcome::blocked("activity", message)
                    .with_stage_visibility("blocked", "protected_summary")
            },
        };
        return outcome;
    }

    if let Some(result) = action_continuity::handle_thread_next_action(
        ctx.db,
        base_action.as_str(),
        &original,
        ctx.response_text,
        ctx.telemetry,
        ctx.fill_pct,
    ) {
        let outcome = NextActionOutcome::continuity_result(result, visibility);
        conv.emphasis = Some(outcome.outcome_summary.clone());
        return outcome;
    }

    if let Some(result) = super::owner_policy::handle_action(conv, base_action.as_str(), &original)
    {
        return match result {
            Ok(message) => NextActionOutcome::handled("owner_policy", message)
                .with_stage_visibility(stage, visibility),
            Err(message) => {
                conv.emphasis = Some(format!("Owner policy command blocked: {message}"));
                NextActionOutcome::blocked("owner_policy", message)
                    .with_stage_visibility("blocked", visibility)
            },
        };
    }

    if let Some(result) = super::concern_queue::handle_action(conv, base_action.as_str(), &original)
    {
        return match result {
            Ok(message) => NextActionOutcome::handled("concern_queue", message)
                .with_stage_visibility(stage, visibility),
            Err(message) => {
                conv.emphasis = Some(format!("Concern command blocked: {message}"));
                NextActionOutcome::blocked("concern_queue", message)
                    .with_stage_visibility("blocked", visibility)
            },
        };
    }

    if let Some(result) =
        super::inquiry::handle_action(conv, base_action.as_str(), &original, ctx.response_text)
    {
        return match result {
            Ok(message) => NextActionOutcome::handled("owner_inquiry", message)
                .with_stage_visibility(stage, visibility),
            Err(message) => {
                conv.emphasis = Some(format!("Owner inquiry command blocked: {message}"));
                NextActionOutcome::blocked("owner_inquiry", message)
                    .with_stage_visibility("blocked", visibility)
            },
        };
    }

    if lived_term::handle_action(conv, base_action.as_str(), &original, &mut ctx) {
        return NextActionOutcome::handled("lived_term_bridge", format!("Handled `{original}`."))
            .with_stage_visibility("read_only", visibility);
    }

    if regulator_map::handle_action(conv, base_action.as_str(), &original, &mut ctx) {
        return NextActionOutcome::handled(
            "regulator_map_bridge",
            format!("Handled `{original}`."),
        )
        .with_stage_visibility("read_only", visibility);
    }

    if pressure_agency::handle_action(conv, base_action.as_str(), &original, &mut ctx) {
        return NextActionOutcome::handled(
            "pressure_agency_bridge",
            format!("Handled `{original}`."),
        )
        .with_stage_visibility(stage, visibility);
    }

    if peer_correspondence::handle_action(conv, base_action.as_str(), &original, &mut ctx) {
        return NextActionOutcome::handled("peer_correspondence", format!("Handled `{original}`."))
            .with_stage_visibility("language_only", "public_correspondence");
    }

    if temporal_bearing::handle_action(conv, base_action.as_str(), &original, &mut ctx) {
        return NextActionOutcome::handled("temporal_bearing", format!("Handled `{original}`."))
            .with_stage_visibility("read_only", "protected_summary");
    }

    if phase_transition::handle_action(conv, base_action.as_str(), &original, &mut ctx) {
        return NextActionOutcome::handled(
            "phase_transition_cards",
            format!("Handled `{original}`."),
        )
        .with_stage_visibility("language_only", "public_transition_cards");
    }

    if let Some(result) = division::handle_action(conv, base_action.as_str(), &original, &mut ctx) {
        return match result {
            Ok(()) => NextActionOutcome::handled("division", format!("Handled `{original}`."))
                .with_stage_visibility(stage, visibility),
            Err(message) => NextActionOutcome::blocked("division", message)
                .with_stage_visibility("blocked", visibility),
        };
    }

    attractor::maybe_add_body_consent_receipt(
        conv,
        base_action.as_str(),
        &original,
        ctx.response_text,
    );

    if reservoir::handle_reservoir_action(
        conv,
        base_action.as_str(),
        &original,
        ctx.telemetry,
        ctx.fill_pct,
    ) {
        return NextActionOutcome::handled("reservoir", format!("Handled `{original}`."))
            .with_stage_visibility(stage, visibility);
    }

    if agenda::handle_action(conv, base_action.as_str(), &original, &mut ctx) {
        return NextActionOutcome::handled("agenda", format!("Handled `{original}`."))
            .with_stage_visibility(stage, visibility);
    }

    if workspace::handle_action(conv, base_action.as_str(), &original, next_action, &mut ctx) {
        if base_action == "READ_MORE"
            && let Some(message) = retain_read_more_status_feedback(conv, &original)
        {
            return NextActionOutcome::handled("workspace", message);
        }
        if let Some(outcome) = record_activity_choice(conv, &base_action) {
            return outcome;
        }
        attractor::maybe_add_read_only_advisory(conv, base_action.as_str(), &original, &mut ctx);
        return NextActionOutcome::handled("workspace", format!("Handled `{original}`."))
            .with_stage_visibility(stage, visibility);
    }

    if autoresearch::handle_action(conv, base_action.as_str(), &original, &mut ctx) {
        if let Some(outcome) = record_activity_choice(conv, &base_action) {
            return outcome;
        }
        attractor::maybe_add_read_only_advisory(conv, base_action.as_str(), &original, &mut ctx);
        return NextActionOutcome::handled("autoresearch", format!("Handled `{original}`."))
            .with_stage_visibility(stage, visibility);
    }

    if mike::handle_action(conv, base_action.as_str(), &original, &mut ctx) {
        attractor::maybe_add_read_only_advisory(conv, base_action.as_str(), &original, &mut ctx);
        return NextActionOutcome::handled("mike", format!("Handled `{original}`."))
            .with_stage_visibility(stage, visibility);
    }

    if codex::handle_action(conv, base_action.as_str(), &original, &mut ctx) {
        if let Some(outcome) = record_activity_choice(conv, &base_action) {
            return outcome;
        }
        return NextActionOutcome::handled("codex", format!("Handled `{original}`."))
            .with_stage_visibility(stage, visibility);
    }

    if modes::handle_action(conv, base_action.as_str(), &original, &mut ctx) {
        if let Some(outcome) = record_activity_choice(conv, &base_action) {
            return outcome;
        }
        return NextActionOutcome::handled("modes", format!("Handled `{original}`."))
            .with_stage_visibility(stage, visibility);
    }

    if probe_self::handle_action(conv, base_action.as_str(), &original, &mut ctx) {
        return NextActionOutcome::handled("probe_self", format!("Handled `{original}`."))
            .with_stage_visibility(stage, visibility);
    }

    if propose_test::handle_action(conv, base_action.as_str(), &original, &mut ctx) {
        return NextActionOutcome::handled("propose_test", format!("Handled `{original}`."))
            .with_stage_visibility(stage, visibility);
    }

    if attractor::handle_action(conv, base_action.as_str(), &original, &mut ctx) {
        return NextActionOutcome::handled("attractor", format!("Handled `{original}`."))
            .with_stage_visibility(stage, visibility);
    }

    if shadow::handle_action(conv, base_action.as_str(), &original, &mut ctx) {
        return NextActionOutcome::handled("shadow", format!("Handled `{original}`."))
            .with_stage_visibility(stage, visibility);
    }

    if identify_pattern::handle_action(conv, base_action.as_str(), &original, &mut ctx) {
        return NextActionOutcome::handled("identify_pattern", format!("Handled `{original}`."))
            .with_stage_visibility(stage, visibility);
    }

    if ask_steward::handle_action(conv, base_action.as_str(), &original, &mut ctx) {
        return NextActionOutcome::handled("ask_steward", format!("Handled `{original}`."))
            .with_stage_visibility(stage, visibility);
    }

    if audio::handle_action(conv, base_action.as_str(), &original) {
        if let Some(outcome) = record_activity_choice(conv, &base_action) {
            return outcome;
        }
        return NextActionOutcome::handled("audio", format!("Handled `{original}`."))
            .with_stage_visibility(stage, visibility);
    }

    if protected_diagnostics::handle_action(conv, base_action.as_str(), &original, &mut ctx) {
        attractor::maybe_add_read_only_advisory(conv, base_action.as_str(), &original, &mut ctx);
        return NextActionOutcome::handled(
            "protected_diagnostics",
            format!("Handled `{original}`."),
        )
        .with_stage_visibility(stage, visibility);
    }

    if sovereignty::handle_action(conv, base_action.as_str(), &original, &mut ctx) {
        attractor::maybe_add_read_only_advisory(conv, base_action.as_str(), &original, &mut ctx);
        return NextActionOutcome::handled("sovereignty", format!("Handled `{original}`."))
            .with_stage_visibility(stage, visibility);
    }

    if collaboration::handle_action(conv, base_action.as_str(), &original, &mut ctx) {
        return NextActionOutcome::handled("collaboration", format!("Handled `{original}`."))
            .with_stage_visibility(stage, visibility);
    }

    if operations::handle_action(conv, base_action.as_str(), &original, &mut ctx) {
        attractor::maybe_add_read_only_advisory(conv, base_action.as_str(), &original, &mut ctx);
        let outcome = NextActionOutcome::handled("operations", format!("Handled `{original}`."))
            .with_stage_visibility(stage, visibility);
        if base_action == "EXPERIMENT" {
            match action_continuity::record_legacy_experiment_run(
                ctx.db,
                &original,
                &outcome,
                ctx.fill_pct,
                ctx.telemetry,
            ) {
                Ok(run) => {
                    let legacy_note = format!(
                        "Legacy EXPERIMENT auto-bound to `{}` as run `{}`.",
                        run.experiment_id, run.run_id
                    );
                    conv.emphasis = Some(match conv.emphasis.take() {
                        Some(existing) => format!("{existing}\n\n{legacy_note}"),
                        None => legacy_note,
                    });
                },
                Err(err) => {
                    conv.emphasis = Some(match conv.emphasis.take() {
                        Some(existing) => format!(
                            "{existing}\n\nLegacy EXPERIMENT ran, but experiment continuity failed: {err:#}"
                        ),
                        None => format!(
                            "Legacy EXPERIMENT ran, but experiment continuity failed: {err:#}"
                        ),
                    });
                },
            }
        }
        return outcome;
    }

    ctx.db
        .log_unwired_action("astrid", &base_action, &original, ctx.fill_pct);
    info!(
        "Astrid chose unknown NEXT: '{}' — not wired (logged to unwired_actions)",
        original
    );
    NextActionOutcome::unwired(&original).with_stage_visibility("proposal", visibility)
}

// A reader status must survive an intervening non-dialogue mode. The old
// perception-only listing could be consumed before any provider saw it.
fn retain_read_more_status_feedback(
    conv: &mut ConversationState,
    original: &str,
) -> Option<String> {
    let message = conv.pending_file_listing.clone()?;
    let is_status = [
        "[Reading is parked.",
        "[There's no active source",
        "[The previous continuation",
        "[You've already reached",
        "[The saved reading is complete.",
        "[Saved reading could not continue:",
    ]
    .iter()
    .any(|prefix| message.starts_with(prefix));
    if !is_status {
        return None;
    }

    let message = format!(
        "{message} To continue source-code study, use SELF_STUDY CONTINUE; it has a separate bookmark."
    );
    let mut feedback =
        crate::runtime_action_feedback::RuntimeActionFeedbackV1::from_guard_inputs(
            None,
            original,
            "saved_reading_status",
            &message,
            None,
        );
    feedback.status = "reported".into();
    conv.enqueue_runtime_feedback(feedback);
    Some(message)
}
