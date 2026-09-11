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
        None => Ok(Command::Continue),
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
    if let Some(target) = target.as_ref() {
        if next_action::study_navigation::private(target) {
            if target.label.split_whitespace().next() != Some("WRITE") {
                anyhow::bail!("use an explicit WRITE command without a source prefix or colon; WRITE HELP lists writing choices");
            }
            return reader.prepare_action(&target.label);
        }
        if target.label.starts_with("SELF_STUDY") {
            return reader.prepare_action(&target.label);
        }
    }
    reader.prepare(shared_study_command(target)?)
}

fn source_study_prepare_notice(
    private_request: bool,
    error: &anyhow::Error,
) -> (&'static str, String, String) {
    (
        if private_request { "private_writing_notice" } else { "introspect_notice" },
        if private_request { format!("Private writing: {error:#}. WRITE HELP lists your choices; WRITE CONTINUE retries a pending draft turn.") } else { format!("Source study: {error:#}. Use SELF_STUDY MAP or SELF_STUDY FIND <text>.") },
        String::new(),
    )
}

async fn run_shared_source_study(
    conv: &mut ConversationState,
    state: &Arc<RwLock<BridgeState>>,
    fill_pct: f32,
) -> (&'static str, String, String) {
    let _attempt = next_action::introspection_cadence::begin_attempt(conv);
    let requested = conv.introspect_target.take();
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
        let output = prepare_shared_study_target(&reader, requested)?;
        Ok((reader, output, catalog))
    })();
    let (reader, output, catalog) = match prepared {
        Ok(value) => value,
        Err(error) => {
            next_action::introspection_cadence::mark_failed(
                conv,
                format!("source_study_prepare:{error}"),
                None,
            );
            return source_study_prepare_notice(private_request, &error);
        },
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
    let completion = crate::llm::generate_source_study(&output).await;
    let completed = crate::lived_state_witness::clock_sample_v1().unix_ms;
    let source = output.page.as_ref().map_or_else(
        || {
            if output.session_pages.is_empty() {
                if output.input_kind == astrid_source_study::InputKind::PrivateWriting { "private draft".into() } else { "source catalog".into() }
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
        next_action::introspection_cadence::mark_failed(
            conv,
            "source_study_generation_unavailable",
            None,
        );
        return (if private_request { "private_writing_notice" } else { "introspect_notice" }, if private_request { "Writing generation was unavailable; WRITE CONTINUE retries the pending draft turn.".into() } else { "Source-study generation was unavailable. The page remains pending; SELF_STUDY CONTINUE retries it.".into() }, source);
    };
    let delivery = completion
        .accepted_delivery
        .as_ref()
        .ok_or_else(|| anyhow::anyhow!("complete source delivery was not retained"))
        .and_then(|receipt| {
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
    let timestamp = chrono_timestamp();
    let directory = if output.input_kind == astrid_source_study::InputKind::PrivateWriting {
        bridge_paths().bridge_workspace().join("private_writing/artifacts")
    } else { bridge_paths().introspections_dir() };
    let artifact_kind = if output.input_kind == astrid_source_study::InputKind::PrivateWriting { "private_writing" } else if delivery.is_ok() {
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
                "navigation only".into()
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
    let artifact = format!(
        "=== ASTRID INTROSPECTION ===\nSource: {source}\nSource revision: {revision}\nSource scope: local checkout; deployed behavior not established\nInput evidence: {}\nAccount: Astrid’s response to this input, not independently verified code facts.\nTimestamp: {timestamp}\nArtifact kind: {artifact_kind}\nVisibility: {visibility}\nLived-state witness: {}\nDelivery: {delivery_status}\n\n{text}",
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
    let mode = source_study_completion_mode(delivery.is_ok(), written.is_ok());
    let private = output.input_kind == astrid_source_study::InputKind::PrivateWriting;
    if mode == "self_study" {
        next_action::introspection_cadence::mark_admitted(conv, &artifact_path, witness);
        if let Some(page) = &output.page {
            finish_source_study_invitation(&catalog, &page.source);
        }
        (if private { "private_writing" } else { mode }, text, source)
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
        );
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
