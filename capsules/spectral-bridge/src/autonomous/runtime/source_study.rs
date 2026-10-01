/// Workspace artifacts retain their owner-aware reader; all source uses one catalog.
fn uses_shared_source_study(conv: &ConversationState) -> bool {
    let Some(target) = conv.introspect_target.as_ref() else {
        return true;
    };
    if target.label.starts_with("SELF_STUDY") || next_action::study_navigation::private(target) {
        return true;
    }
    let sources = introspect::introspect_sources();
    let Ok(resolved) = introspect::resolve_introspect_target_result(&target.label, &sources) else {
        return true;
    };
    !resolved.path.starts_with(bridge_paths().bridge_workspace())
        && !resolved.path.starts_with(bridge_paths().minime_workspace())
}

fn shared_study_command(
    target: Option<state::IntrospectTargetV2>,
) -> anyhow::Result<astrid_source_study::Command> {
    use astrid_source_study::Command;
    match target {
        None => Ok(Command::Reflect),
        Some(target) if target.label == "INTROSPECT" => Ok(Command::Reflect),
        Some(target) if next_action::study_navigation::private(&target) => anyhow::bail!("private writing requires the writing preparation path"),
        Some(target) if target.label.starts_with("SELF_STUDY") => Command::parse(&target.label),
        Some(target) if target.offset == state::IntrospectOffsetV2::Auto => Ok(Command::Resume {
            source: target.label,
        }),
        Some(target) => Ok(Command::Open {
            source: target.label,
            line: match target.offset {
                state::IntrospectOffsetV2::Auto => 1,
                state::IntrospectOffsetV2::Exact(offset) => offset.saturating_add(1),
            },
        }),
    }
}

/// Private intent controls preparation before source recovery can echo a title.
/// This is a privacy boundary, not an alias or a broader writing grammar.
fn prepare_shared_study_target(
    reader: &astrid_source_study::Reader,
    target: Option<state::IntrospectTargetV2>,
) -> anyhow::Result<astrid_source_study::StudyOutput> {
    let operation = target.as_ref().and_then(|t| t.operation_id.clone());
    let mut explicit_action = None;
    if let Some(target) = target.as_ref() {
        if next_action::study_navigation::private(target) {
            if target.label.split_whitespace().next() != Some("WRITE") {
                anyhow::bail!("use an explicit WRITE command without a source prefix or colon; WRITE HELP lists writing choices");
            }
            explicit_action = Some(target.label.clone());
        }
        if target.label.starts_with("SELF_STUDY") {
            explicit_action = Some(target.label.clone());
        }
    }
    let action = match explicit_action {
        Some(action) => action,
        None => match shared_study_command(target)? {
            astrid_source_study::Command::Continue => "SELF_STUDY CONTINUE".into(),
            astrid_source_study::Command::Reflect => "INTROSPECT".into(),
            astrid_source_study::Command::Resume { source } => format!("SELF_STUDY RESUME {source}"),
            astrid_source_study::Command::Open { source, line } => format!("SELF_STUDY OPEN {source} {line}"),
            _ => anyhow::bail!("unsupported legacy study target"),
        },
    };
    let revision = reader.preparation_revision()?;
    // Authored requests use their durable dispatch event; protected presentation
    // uses its window/slot. Untargeted scheduler reads are checkpoint-scoped.
    let operation = operation.unwrap_or_else(|| format!("scheduled-reader-{revision}"));
    use sha2::{Digest as _, Sha256};
    reader.prepare_once(&format!("prepare-{:x}", Sha256::digest(operation.as_bytes())), &revision, &action)
}

fn source_study_prepare_notice(
    private_request: bool,
    error: &anyhow::Error,
) -> (&'static str, String, String) {
    (
        if private_request { "private_writing_notice" } else { "source_study_runtime_notice" },
        if private_request { format!("Private writing: {error:#}. WRITE HELP lists your choices; WRITE CONTINUE retries a pending draft turn.") } else { format!("Source study: {error:#}. Use SELF_STUDY MAP or SELF_STUDY FIND <text>.") },
        String::new(),
    )
}

fn retain_study_runtime_notice(directory: &std::path::Path, text: &str) -> std::io::Result<std::path::PathBuf> {
    std::fs::create_dir_all(directory)?;
    let document = format!("=== SOURCE STUDY RUNTIME NOTICE ===\nAccount: runtime-generated diagnostic, not Astrid's authored reflection.\nNo source delivery or understanding is established by this notice.\n\n{text}\n");
    write_collision_safe_journal_document(directory, "runtime_notice", &chrono_timestamp(), &document)
}

fn project_study_response(conv: &mut ConversationState, mode: &str, text: &str, notice_directory: &std::path::Path) -> (String, bool) {
    if mode != "source_study_runtime_notice" {
        return project_mailbox_response(text);
    }
    if let Err(error) = retain_study_runtime_notice(notice_directory, text) {
        warn!(%error, "source-study runtime notice could not be retained");
    }
    conv.push_receipt("SOURCE_STUDY_RUNTIME", vec![text.into()]);
    // No runtime error is authored prose, a peer/sensory signal or an executable NEXT.
    (String::new(), false)
}

async fn run_shared_source_study(
    conv: &mut ConversationState,
    state: &Arc<RwLock<BridgeState>>,
    fill_pct: f32,
) -> (&'static str, String, String) {
    let _attempt = next_action::introspection_cadence::begin_attempt(conv);
    let store = crate::action_continuity::ActionContinuityStore::for_astrid_workspace();
    let private_request = conv.introspect_target.as_ref().is_some_and(next_action::study_navigation::private);
    let job = match study_handoff::ensure_run_job(conv) {
        Ok(job) => job,
        Err(error) => return source_study_prepare_notice(private_request, &error),
    };
    let requested = conv.introspect_target.take();
    let requested_action = requested.as_ref().map_or("INTROSPECT", |t| t.label.as_str()).to_owned();
    let private_request = requested.as_ref().is_some_and(next_action::study_navigation::private);
    let prepared = (|| -> anyhow::Result<_> {
        let paths = bridge_paths();
        let catalog =
            astrid_source_study::Catalog::installation(paths.astrid_root(), paths.minime_root())?;
        let reader = astrid_source_study::Reader::new(
            catalog.clone(),
            paths
                .bridge_workspace()
                .join("diagnostics/source_first_v3/shared_reader"),
        )
        .with_runtime_workspace(paths.bridge_workspace().to_path_buf(), "astrid");
        let output = if let Some(id) = job.as_deref() {
            study_handoff::prepare(&store, conv, id, || prepare_shared_study_target(&reader, requested))?
        } else { prepare_shared_study_target(&reader, requested)? };
        study_handoff::validate_sources(&output, &catalog)?;
        Ok((reader, output, catalog))
    })();
    let (reader, output, catalog) = match prepared {
        Ok(value) => value,
        Err(error) => {
            if let Some(id) = job.as_deref()
                && let Err(failure) = study_handoff::failed(&store, conv, id, false) {
                    warn!(%failure, "study preparation failure retains handoff debt");
            }
            next_action::introspection_cadence::mark_failed(
                conv,
                format!("source_study_prepare:{error}"),
                None,
            );
            return source_study_prepare_notice(private_request, &error);
        },
    };
    let admission = match activity_focus::admit(&reader, &output, &requested_action) {
        Ok(admission) => admission,
        Err(error) => return source_study_prepare_notice(private_request, &error),
    };
    let source_snapshot = output.page.as_ref().and_then(|page| {
        if page.revision.lines == 0 {
            return None;
        }
        let source = catalog.resolve(&page.source).ok()?;
        let content = std::fs::read_to_string(&source.path).ok()?;
        let digest = format!(
            "{:x}",
            <sha2::Sha256 as sha2::Digest>::digest(content.as_bytes())
        );
        if digest != page.revision.sha256 {
            return None;
        }
        Some(crate::lived_state_witness::source_snapshot_v1(
            &source.path,
            &content,
            &page.text,
            page.start.line,
            page.end
                .line
                .saturating_sub(usize::from(
                    content.as_bytes().get(page.end.byte.saturating_sub(1)) == Some(&b'\n'),
                ))
                .min(page.revision.lines),
            page.revision.lines,
            crate::lived_state_witness::clock_sample_v1(),
        ))
    });
    let started = crate::lived_state_witness::clock_sample_v1().unix_ms;
    let shadow = conv
        .astrid_shadow
        .lived_state_scalar_observation_v1(started);
    if let Some(id) = job.as_deref()
        && let Err(error) = study_handoff::claim(&store, conv, id) {
            return source_study_prepare_notice(private_request, &error);
    }
    let completion = if job.is_some() {
        crate::llm::generate_source_study_for_job(&output, job.as_deref()).await
    } else { crate::llm::generate_source_study(&output).await };
    let completed = crate::lived_state_witness::clock_sample_v1().unix_ms;
    let source = output.page.as_ref().map_or_else(
        || {
            if output.session_pages.is_empty() {
                if output.is_continuation_decision() { "study continuation choice".into() } else if output.input_kind == astrid_source_study::InputKind::PrivateWriting { "private draft".into() } else if output.input_kind == astrid_source_study::InputKind::Reflection { "open reflection".into() } else if output.input_kind == astrid_source_study::InputKind::Geometry { "chosen geometry evidence".into() } else { "source catalog".into() }
            } else {
                format!(
                    "study session ({} source pages)",
                    output.session_pages.len()
                )
            }
        },
        |p| p.source.clone(),
    );
    let Some(text) = completion.text else {
        if let Some(id) = job.as_deref()
            && let Err(error) = study_handoff::failed(&store, conv, id, true) {
                warn!(%error, "study provider failure retains handoff debt");
        }
        if let Err(error) = activity_focus::finish(&reader, admission, false) {
            warn!(%error, "protected provider failure retains recovery debt");
        }
        next_action::introspection_cadence::mark_failed(
            conv,
            "source_study_generation_unavailable",
            None,
        );
        return (if private_request { "private_writing_notice" } else { "source_study_runtime_notice" }, if private_request { "Writing generation was unavailable; WRITE CONTINUE retries the pending draft turn.".into() } else { "Source-study generation was unavailable; the offered input remains retained. Retry the exact requested action to prepare another attempt.".into() }, source);
    };
    let delivery = completion
        .accepted_delivery
        .as_ref()
        .ok_or_else(|| anyhow::anyhow!("complete source delivery was not retained"))
        .and_then(|receipt| {
            if let Some(id) = job.as_deref() {
                return study_handoff::delivered(&store, conv, id, &reader, receipt);
            }
            crate::llm::verify_delivery_receipt(receipt)?;
            if let Some(page) = &output.page {
                reader.delivered_artifact(
                    &page.id,
                    std::path::Path::new(&receipt.retained_artifact_path),
                )?;
            } else if let Some(navigation_id) = &output.navigation_id {
                reader.navigation_delivered_artifact(
                    navigation_id,
                    std::path::Path::new(&receipt.retained_artifact_path),
                )?;
            }
            Ok(())
        });
    if let Err(error) = activity_focus::finish(&reader, admission, delivery.is_ok()) {
        warn!(%error, "protected delivery retains recovery debt");
    }
    let timestamp = chrono_timestamp();
    let directory = if output.input_kind == astrid_source_study::InputKind::PrivateWriting {
        bridge_paths().bridge_workspace().join("private_writing/artifacts")
    } else { bridge_paths().introspections_dir() };
    let artifact_kind = if output.input_kind == astrid_source_study::InputKind::PrivateWriting { "private_writing" } else if delivery.is_ok() && output.is_continuation_decision() { "study_decision" } else if delivery.is_ok() {
        "introspection"
    } else {
        "source_study_notice"
    };
    let artifact_path = directory.join(format!(
        "{artifact_kind}_{}_{}.txt",
        introspect::safe_artifact_label(&source),
        timestamp
    ));
    let delivery_status = delivery.as_ref().map_or_else(
        |error| format!("unverified: {error:#}"),
        |()| "verified input delivery; response claims and understanding not verified".into(),
    );
    let routes = completion
        .accepted_delivery
        .as_ref()
        .map(|receipt| {
            crate::lived_state_witness::model_route_v1(
                None,
                None,
                Some(receipt.request_sha256.clone()),
                &receipt.provider_route,
                &receipt.provider_model,
                started,
                completed,
                None,
                None,
                None,
                &text,
            )
        })
        .into_iter()
        .collect::<Vec<_>>();
    let authorship = crate::lived_state_witness::begin_authorship_v1(
        source_snapshot.as_ref(),
        &routes,
        artifact_kind,
    );
    let revision = output.page.as_ref().map_or_else(
        || {
            if output.session_pages.is_empty() {
                if output.input_kind == astrid_source_study::InputKind::Reflection { "not applicable; no source or measurements supplied".into() } else if output.input_kind == astrid_source_study::InputKind::Geometry { "frozen observation hashes in supplied evidence; no new source page".into() } else { "navigation only".into() }
            } else {
                output
                    .session_pages
                    .iter()
                    .map(|p| {
                        format!(
                            "{} sha256:{}; bytes {}..{}",
                            p.source, p.revision.sha256, p.start.byte, p.end.byte
                        )
                    })
                    .collect::<Vec<_>>()
                    .join("; ")
            }
        },
        |page| {
            format!(
                "sha256:{}; bytes {}..{}",
                page.revision.sha256, page.start.byte, page.end.byte
            )
        },
    );
    let visibility = if output.input_kind == astrid_source_study::InputKind::PrivateWriting { "protected" } else if delivery.is_ok() {
        "summary"
    } else {
        "protected"
    };
    let source_scope = if output.is_continuation_decision() {
        "continuation choice; no new source analysis supplied"
    } else if output.input_kind == astrid_source_study::InputKind::Reflection {
        "open reflection; no source inspection supplied"
    } else { "local checkout; deployed behavior not established" };
    let artifact = format!(
        "=== {} ===\nSource: {source}\nSource revision: {revision}\nSource scope: {source_scope}\nInput evidence: {}\nAccount: Astrid’s response to this input, not independently verified code facts.\nTimestamp: {timestamp}\nArtifact kind: {artifact_kind}\nVisibility: {visibility}\nLived-state witness: {}\nDelivery: {delivery_status}\n\n{text}",
        if output.is_continuation_decision() { "ASTRID STUDY DECISION" } else if output.input_kind == astrid_source_study::InputKind::RevisionRecovery { "ASTRID STUDY NAVIGATION RESPONSE" } else { "ASTRID INTROSPECTION" },
        output.evidence_scope,
        authorship.witness_id()
    );
    let written = std::fs::create_dir_all(&directory)
        .and_then(|()| std::fs::write(&artifact_path, artifact.as_bytes()));
    let witness = if written.is_ok() {
        let guard = state.read().await;
        let runtime = crate::lived_state_witness::runtime_context_v1(&guard, fill_pct, shadow);
        match crate::lived_state_witness::finalize_and_submit_v1(
            &authorship,
            artifact_kind,
            artifact_path
                .file_name()
                .and_then(|name| name.to_str())
                .unwrap_or("self_study.txt"),
            artifact.as_bytes(),
            source_snapshot,
            routes,
            runtime,
        ) {
            crate::lived_state_witness::WitnessSubmitResultV1::Accepted => "accepted",
            crate::lived_state_witness::WitnessSubmitResultV1::QueueFull => "queue_full",
            crate::lived_state_witness::WitnessSubmitResultV1::Disconnected => "disconnected",
        }
    } else {
        "not_attempted"
    };
    if artifact_kind == "introspection" && written.is_ok() {
        // Reflective sidecar hook for the shared reader (2026-09-23). The shared
        // reader replaced the legacy introspection writer on 2026-09-08 without
        // this hook, so controller reports stopped that night. Same context shape
        // as the legacy hook in orchestration.rs; `query_sidecar` owns the
        // operator switch (default off), the cooldown and the script check, so
        // this path costs nothing while the switch is off.
        let telemetry = state.read().await.latest_telemetry.clone();
        let spectral = telemetry
            .as_ref()
            .map_or_else(|| "No live telemetry.".to_string(), interpret_spectral);
        let sidecar_context = format!(
            "Fill {fill_pct:.1}%. {spectral}\n\nAstrid's authored account:\n{}",
            semantic_truncate_str(&text, 500)
        );
        let controller_path = directory.join(format!(
            "controller_{}_{timestamp}.json",
            introspect::safe_artifact_label(&source)
        ));
        let source_label = source.clone();
        crate::lifecycle::spawn_background(async move {
            if let Some(report) = crate::reflective::query_sidecar(&sidecar_context).await
                && !report.as_context_block().is_empty()
                && let Ok(json) = serde_json::to_string_pretty(&report.storage_snapshot())
            {
                let _ = std::fs::write(&controller_path, json);
                info!(source = %source_label, "reflective controller report saved");
            }
        });
    }
    let mode = source_study_completion_mode(delivery.is_ok(), written.is_ok());
    let private = output.input_kind == astrid_source_study::InputKind::PrivateWriting;
    if mode == "self_study" {
        next_action::introspection_cadence::mark_admitted(conv, &artifact_path, witness);
        if let Some(page) = &output.page {
            finish_source_study_invitation(&catalog, &page.source);
        }
        (source_study_authored_mode(&output), text, source)
    } else {
        next_action::introspection_cadence::mark_failed(
            conv,
            delivery_status.clone(),
            written.is_ok().then_some(artifact_path.as_path()),
        );
        (
            if private { "private_writing_notice" } else { mode },
            format!("{text}\n\n[Source-study delivery: {delivery_status}.]"),
            source,
        )
    }
}

fn source_study_authored_mode(output: &astrid_source_study::StudyOutput) -> &'static str {
    if output.input_kind == astrid_source_study::InputKind::PrivateWriting {
        "private_writing"
    } else if output.is_continuation_decision() {
        "study_decision"
    } else if output.input_kind == astrid_source_study::InputKind::Reflection {
        "introspect"
    } else {
        "self_study"
    }
}

/// A presentation distinction must not change existing study authorization,
/// NEXT handling, signal encoding or regulation policy.
fn study_execution_mode(mode: &str) -> &str {
    if mode == "study_decision" { "self_study" } else { mode }
}

pub(super) fn source_study_completion_mode(
    delivery_verified: bool,
    artifact_written: bool,
) -> &'static str {
    if delivery_verified && artifact_written {
        "self_study"
    } else {
        "self_study_carriage_notice"
    }
}

/// A source invitation is attended only for the exact canonical source identity.
/// This clears the invitation, not a claim that a code change or diagnosis is done.
fn finish_source_study_invitation(catalog: &astrid_source_study::Catalog, source: &str) {
    let path = bridge_paths().open_steward_query_path();
    let slot = std::fs::read(&path)
        .ok()
        .and_then(|bytes| serde_json::from_slice::<Value>(&bytes).ok());
    let matches = slot
        .as_ref()
        .and_then(|slot| slot.get("review_target"))
        .and_then(Value::as_str)
        .and_then(|target| catalog.resolve(review_target_match_basis(target)).ok())
        .is_some_and(|target| target.id == source);
    if matches && let Err(error) = std::fs::remove_file(path) {
        warn!(%error, "source-study invitation could not be cleared after delivery");
    }
}

#[cfg(test)]
mod source_study_tests {
    use super::*;
    #[test]
    fn new_namespace_recovery_and_help_use_real_bridge_preparation_without_selecting() {
        let temp = tempfile::tempdir().unwrap();
        let reader = astrid_source_study::Reader::new(
            astrid_source_study::Catalog::new(std::collections::BTreeMap::from([("astrid".into(), temp.path().into())])).unwrap(),
            temp.path().join("reader"),
        ).with_runtime_workspace(temp.path().join("workspace"), "astrid");
        for (index, action) in ["SELF_STUDY NEW Does the Kernel implement the blocked check?", "SELF_STUDY HELP notebook"].iter().enumerate() {
            let mut conv = ConversationState::new(Vec::new(), None);
            next_action::study_navigation::handle_request(&mut conv, "SELF_STUDY", action).unwrap();
            conv.introspect_target.as_mut().unwrap().operation_id = Some(format!("reference-{index}"));
            let out = prepare_shared_study_target(&reader, conv.introspect_target).unwrap();
            assert_eq!(source_study_authored_mode(&out), "study_decision");
            assert!(out.text.contains("SELF_STUDY QUESTION NEW"));
            assert!(out.page.is_none() && out.question_id.is_none());
            let before: Value = serde_json::from_slice(&std::fs::read(temp.path().join("reader/reader-v1.json")).unwrap()).unwrap();
            let request = serde_json::json!({"messages":[{"role":"user","content":out.text}]}).to_string();
            let response = serde_json::json!({"message":{"content":"NEXT: REST"},"done":true}).to_string();
            reader.navigation_delivered(out.navigation_id.as_ref().unwrap(), &request, &response).unwrap();
            let after: Value = serde_json::from_slice(&std::fs::read(temp.path().join("reader/reader-v1.json")).unwrap()).unwrap();
            assert_eq!(before["questions"], after["questions"]);
            assert_eq!(after["questions"]["entries"], serde_json::json!({}));
        }
    }
    #[test]
    fn quiet_notebook_and_decisions_pass_through_real_bridge_preparation() {
        let temp = tempfile::tempdir().unwrap();
        let reader = astrid_source_study::Reader::new(
            astrid_source_study::Catalog::new(std::collections::BTreeMap::from([("astrid".into(), temp.path().into())])).unwrap(),
            temp.path().join("reader"),
        ).with_runtime_workspace(temp.path().join("workspace"), "astrid");
        let prepare = |action: &str, operation: &str| {
            let mut conv = ConversationState::new(Vec::new(), None);
            next_action::study_navigation::handle_request(&mut conv, "SELF_STUDY", action).unwrap();
            conv.introspect_target.as_mut().unwrap().operation_id = Some(operation.into());
            prepare_shared_study_target(&reader, conv.introspect_target).unwrap()
        };
        let first = prepare("SELF_STUDY MAP", "first");
        let request = serde_json::json!({"messages":[{"role":"user","content":first.text}]}).to_string();
        let response = serde_json::json!({"message":{"content":"STUDY_QUESTION: Retained synthetic question?\nNEXT: REST"},"done":true}).to_string();
        reader.navigation_delivered(first.navigation_id.as_ref().unwrap(), &request, &response).unwrap();
        for (action, id) in [
            ("SELF_STUDY QUESTION NEW Separate inquiry?", "new"),
            ("SELF_STUDY QUESTION RESOLVE q1 Still uncertain.", "resolve"),
            ("SELF_STUDY QUESTION PARK NOTEBOOK", "park"),
        ] {
            let output = prepare(action, id);
            assert_eq!(source_study_authored_mode(&output), "study_decision");
            assert!(!output.text.contains("Retained synthetic question?"));
            assert_eq!(output, prepare(action, id));
        }
        let inspected = prepare("SELF_STUDY QUESTION NOTEBOOK", "inspect");
        assert!(inspected.text.contains("Retained synthetic question?"));
        assert!(!prepare("SELF_STUDY MAP", "quiet").text.contains("Retained synthetic question?"));
        let returned = prepare("SELF_STUDY QUESTION RETURN NOTEBOOK", "return");
        assert!(returned.text.contains("Retained synthetic question?"));
        assert_eq!(source_study_authored_mode(&returned), "study_decision");
        let execution = study_execution_mode(source_study_authored_mode(&returned));
        assert_eq!(execution, "self_study");
        assert!(volition::mode_supports_being_attestation(execution));
        assert_eq!(next_action::normalized_study_continue_next(execution, "NEXT: CONTINUE"), Some("SELF_STUDY CONTINUE"));
        assert_eq!(next_action::normalized_study_continue_next(execution, "NEXT: REST"), None);
        assert_eq!(study_execution_mode("source_study_runtime_notice"), "source_study_runtime_notice");
        let mut conv = ConversationState::new(Vec::new(), None);
        let (text, _) = project_study_response(&mut conv, "study_decision", "NEXT: REST", temp.path());
        assert_eq!(text, "NEXT: REST");
        assert!(parse_next_action(&text).is_some());
    }
    #[test]
    fn changed_source_reaches_the_real_bridge_preparation_and_requires_reselection() {
        let temp = tempfile::tempdir().unwrap();
        let root = temp.path().join("astrid");
        let source = root.join("crates/example/src/lib.rs");
        std::fs::create_dir_all(source.parent().unwrap()).unwrap();
        std::fs::write(&source, format!("pub fn before() {{}}\n{}", "// old source\n".repeat(1500))).unwrap();
        let directory = temp.path().join("reader");
        let reader = astrid_source_study::Reader::new(
            astrid_source_study::Catalog::new(std::collections::BTreeMap::from([("astrid".into(), root)])).unwrap(),
            directory.clone(),
        ).with_runtime_workspace(temp.path().join("workspace"), "astrid");
        let prepare = |action: &str, operation: &str| {
            let mut conv = ConversationState::new(Vec::new(), None);
            next_action::study_navigation::handle_request(&mut conv, "SELF_STUDY", action).unwrap();
            conv.introspect_target.as_mut().unwrap().operation_id = Some(operation.into());
            prepare_shared_study_target(&reader, conv.introspect_target).unwrap()
        };
        let deliver = |output: &astrid_source_study::StudyOutput, text: &str| {
            let request = serde_json::json!({"messages":[{"role":"user","content":output.text}]}).to_string();
            let response = serde_json::json!({"message":{"content":text},"done":true}).to_string();
            if let Some(page) = &output.page {
                reader.delivered(&page.id, &request, &response).unwrap();
            } else {
                reader.navigation_delivered(output.navigation_id.as_ref().unwrap(), &request, &response).unwrap();
            }
        };
        let open = "SELF_STUDY OPEN astrid/crates/example/src/lib.rs 1";
        let old = prepare(open, "old");
        deliver(&old, "STUDY_NOTE: Original account.\nNEXT: SELF_STUDY CONTINUE");
        let before: Value = serde_json::from_slice(&std::fs::read(directory.join("reader-v1.json")).unwrap()).unwrap();
        std::fs::write(&source, "// changed\n".repeat(1500)).unwrap();
        let recovery = prepare("SELF_STUDY CONTINUE", "changed");
        assert_eq!(recovery.input_kind, astrid_source_study::InputKind::RevisionRecovery);
        assert_eq!(recovery, prepare("SELF_STUDY CONTINUE", "changed"));
        assert!(recovery.page.is_none() && recovery.text.contains(open));
        deliver(&recovery, &format!("STUDY_NOTE: Not an implicit revision.\nNEXT: {open}"));
        let after: Value = serde_json::from_slice(&std::fs::read(directory.join("reader-v1.json")).unwrap()).unwrap();
        for key in ["bookmarks", "progress", "notebook", "questions", "last_input"] {
            assert_eq!(before[key], after[key]);
        }
        let selected = prepare(open, "selected");
        assert_ne!(selected.page.as_ref().unwrap().revision, old.page.unwrap().revision);
        deliver(&selected, "NEXT: SELF_STUDY CONTINUE");
        let resumed = prepare("SELF_STUDY CONTINUE", "resumed");
        assert_eq!(resumed.page.unwrap().start, selected.page.unwrap().end);
    }

    #[test]
    fn runtime_failure_notice_has_no_authored_or_telemetry_framing() {
        let temp = tempfile::tempdir().unwrap();
        let (mode, text, _) = source_study_prepare_notice(false, &anyhow::anyhow!("unreadable checkpoint; preserved"));
        assert_eq!(mode, "source_study_runtime_notice");
        let path = retain_study_runtime_notice(temp.path(), &text).unwrap();
        let saved = std::fs::read_to_string(path).unwrap();
        assert!(saved.contains("runtime-generated diagnostic, not Astrid's authored reflection"));
        assert!(!saved.contains("Fill:") && !saved.contains("ASTRID JOURNAL"));
        assert!(!text.contains("CONTINUE retries"));
        let mut conv = ConversationState::new(Vec::new(), None);
        let (ordinary, shared) = project_study_response(&mut conv, mode, "unavailable\nNEXT: REST", temp.path());
        assert!(ordinary.is_empty() && !shared);
        assert!(parse_next_action(&ordinary).is_none());
        let (authored, _) = project_study_response(&mut conv, "self_study", "Chosen reply.\nNEXT: REST", temp.path());
        assert_eq!(authored, "Chosen reply.\nNEXT: REST");
    }
    #[test]
    fn explicit_inquiry_review_does_not_select_or_revise_the_reviewed_question() {
        let temp = tempfile::tempdir().unwrap();
        let reader = astrid_source_study::Reader::new(
            astrid_source_study::Catalog::new(std::collections::BTreeMap::from([("astrid".into(), temp.path().into())])).unwrap(),
            temp.path().join("reader"),
        ).with_runtime_workspace(temp.path().join("workspace"), "astrid");
        reader.prepare_action("SELF_STUDY QUESTION NEW Retained question?").unwrap();
        reader.prepare_action("SELF_STUDY QUESTION PARK q1").unwrap();
        reader.prepare_action("SELF_STUDY QUESTION NEW Unrelated active question?").unwrap();
        let before: serde_json::Value = serde_json::from_slice(&std::fs::read(temp.path().join("reader/reader-v1.json")).unwrap()).unwrap();
        let mut conv = ConversationState::new(Vec::new(), None);
        let action = "SELF_STUDY QUESTION REVIEW q1";
        next_action::study_navigation::handle_request(&mut conv, "SELF_STUDY", action).unwrap();
        let output = prepare_shared_study_target(&reader, conv.introspect_target).unwrap();
        assert_eq!(output.input_kind, astrid_source_study::InputKind::InquiryReview);
        assert!(output.question_id.is_none());
        assert!(output.text.contains("Retained question?"));
        assert!(!output.text.contains("Unrelated active question?"));
        let request = serde_json::json!({"messages":[{"role":"system","content":output.system_prompt},{"role":"user","content":output.text}]}).to_string();
        let response = serde_json::json!({"message":{"content":"STUDY_NOTE: Not a saved revision.\nNEXT: REST"},"done":true}).to_string();
        reader.navigation_delivered(output.navigation_id.as_ref().unwrap(), &request, &response).unwrap();
        let after: serde_json::Value = serde_json::from_slice(&std::fs::read(temp.path().join("reader/reader-v1.json")).unwrap()).unwrap();
        assert_eq!(before["questions"], after["questions"]);
    }
    #[test]
    fn explicit_introspection_prepares_reflection_not_a_source_page() {
        let temp = tempfile::tempdir().unwrap();
        let reader = astrid_source_study::Reader::new(
            astrid_source_study::Catalog::new(std::collections::BTreeMap::from([("astrid".into(), temp.path().into())])).unwrap(),
            temp.path().join("reader"),
        ).with_runtime_workspace(temp.path().join("workspace"), "astrid");
        let mut conv = ConversationState::new(Vec::new(), None);
        next_action::study_navigation::handle_request(&mut conv, "INTROSPECT", "INTROSPECT").unwrap();
        assert_eq!(conv.introspect_target.as_ref().unwrap().label, "INTROSPECT");
        let output = prepare_shared_study_target(&reader, conv.introspect_target).unwrap();
        assert_eq!(output.input_kind, astrid_source_study::InputKind::Reflection);
        assert!(output.page.is_none());
        assert!(!output.text.contains("RECALLED ACCOUNT"));
        assert_eq!(shared_study_command(Some(state::IntrospectTargetV2::auto("SELF_STUDY CONTINUE".into()))).unwrap(), astrid_source_study::Command::Continue);
    }
    #[test]
    fn authored_study_preparation_replays_by_dispatch_identity() {
        let temp = tempfile::tempdir().unwrap();
        let reader = astrid_source_study::Reader::new(
            astrid_source_study::Catalog::new(std::collections::BTreeMap::from([("astrid".into(), temp.path().into())])).unwrap(),
            temp.path().join("reader"),
        ).with_runtime_workspace(temp.path().join("workspace"), "astrid");
        let mut target = state::IntrospectTargetV2::auto("SELF_STUDY QUESTION NEW Synthetic retry?".into());
        target.operation_id = Some("durable-dispatch-1".into());
        let first = prepare_shared_study_target(&reader, Some(target.clone())).unwrap();
        let restored: state::IntrospectTargetV2 = serde_json::from_str(&serde_json::to_string(&target).unwrap()).unwrap();
        let retry = prepare_shared_study_target(&reader, Some(restored)).unwrap();
        assert_eq!(serde_json::to_value(first).unwrap(), serde_json::to_value(retry).unwrap());
        target.label = "SELF_STUDY MAP".into();
        assert!(prepare_shared_study_target(&reader, Some(target)).is_err());
    }

    #[test]
    fn malformed_private_targets_cannot_prepare_public_source_recovery() {
        let temp = tempfile::tempdir().unwrap();
        let root = temp.path().join("astrid");
        std::fs::create_dir_all(&root).unwrap();
        std::fs::write(root.join("Cargo.toml"), "[workspace]\nmembers=[]\n").unwrap();
        let directory = temp.path().join("reader");
        let reader = astrid_source_study::Reader::new(
            astrid_source_study::Catalog::new(std::collections::BTreeMap::from([
                ("astrid".into(), root),
            ])).unwrap(),
            directory.clone(),
        ).with_runtime_workspace(temp.path().join("workspace"), "astrid");
        for label in [
            "write START PRIVATE_TITLE_MARKER",
            "WRITE: START PRIVATE_TITLE_MARKER",
            "write: START PRIVATE_TITLE_MARKER",
            "SELF_STUDY write START PRIVATE_TITLE_MARKER",
            "SELF_STUDY REPLACE WRITE START PRIVATE_TITLE_MARKER",
            "SELF_STUDY replace write START PRIVATE_TITLE_MARKER",
            "WRITE NOT_A_WRITING_VERB PRIVATE_TITLE_MARKER",
        ] {
            for target in [state::IntrospectTargetV2::auto(label.into()), state::IntrospectTargetV2::exact(label.into(), 9)] {
                let mut conv = ConversationState::new(Vec::new(), None);
                conv.introspect_target = Some(target.clone());
                assert!(uses_shared_source_study(&conv));
                assert!(next_action::study_navigation::private(&target));
                assert!(shared_study_command(Some(target.clone())).is_err());
                let error = prepare_shared_study_target(&reader, Some(target)).unwrap_err();
                let (mode, notice, source) = source_study_prepare_notice(true, &error);
                assert_eq!(mode, "private_writing_notice");
                assert!(notice.contains("WRITE HELP"));
                assert!(!notice.contains("PRIVATE_TITLE_MARKER"));
                assert!(source.is_empty());
                assert!(!directory.join("reader-v1.json").exists());
            }
        }
        let writing = prepare_shared_study_target(&reader, Some(state::IntrospectTargetV2::auto(
            "WRITE START a private topic".into(),
        ))).unwrap();
        assert_eq!(writing.input_kind, astrid_source_study::InputKind::PrivateWriting);
        assert!(!writing.text.contains("recovery map"));
        assert!(!directory.join("reader-v1.json").exists());
        let source = prepare_shared_study_target(&reader, Some(state::IntrospectTargetV2::auto(
            "SELF_STUDY OPEN astrid/Cargo.toml 1".into(),
        ))).unwrap();
        assert!(source.page.is_some());
    }

    #[test]
    fn source_action_keeps_exact_identity_and_one_based_line() {
        let action = shared_study_command(Some(state::IntrospectTargetV2::auto(
            "SELF_STUDY OPEN astrid/crates/astrid-kernel/src/lib.rs 51".into(),
        )))
        .unwrap();
        assert_eq!(
            action,
            astrid_source_study::Command::Open {
                source: "astrid/crates/astrid-kernel/src/lib.rs".into(),
                line: 51
            }
        );
        assert_eq!(
            shared_study_command(Some(state::IntrospectTargetV2::exact(
                "minime:esn".into(),
                400
            )))
            .unwrap(),
            astrid_source_study::Command::Open {
                source: "minime:esn".into(),
                line: 401
            }
        );
    }
}
