/// Workspace artifacts retain their owner-aware reader; all source uses one catalog.
fn uses_shared_source_study(conv: &ConversationState) -> bool {
    let Some(target) = conv.introspect_target.as_ref() else {
        return true;
    };
    if target.label.starts_with("SELF_STUDY") {
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

async fn run_shared_source_study(
    conv: &mut ConversationState,
    state: &Arc<RwLock<BridgeState>>,
    fill_pct: f32,
) -> (&'static str, String, String) {
    let _attempt = next_action::introspection_cadence::begin_attempt(conv);
    let requested = conv.introspect_target.take();
    let prepared = (|| -> anyhow::Result<_> {
        let paths = bridge_paths();
        let catalog =
            astrid_source_study::Catalog::installation(paths.astrid_root(), paths.minime_root())?;
        let reader = astrid_source_study::Reader::new(
            catalog.clone(),
            paths
                .bridge_workspace()
                .join("diagnostics/source_first_v3/shared_reader"),
        );
        let output = if let Some(target) = requested
            .as_ref()
            .filter(|target| target.label.starts_with("SELF_STUDY"))
        {
            reader.prepare_action(&target.label)?
        } else {
            reader.prepare(shared_study_command(requested)?)?
        };
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
            return (
                "introspect_notice",
                format!("Source study: {error:#}. Use SELF_STUDY MAP or SELF_STUDY FIND <text>."),
                String::new(),
            );
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
    let source = output
        .page
        .as_ref()
        .map_or_else(|| "source catalog".into(), |p| p.source.clone());
    let Some(text) = completion.text else {
        next_action::introspection_cadence::mark_failed(
            conv,
            "source_study_generation_unavailable",
            None,
        );
        return ("introspect_notice", "Source-study generation was unavailable. The page remains pending; SELF_STUDY CONTINUE retries it.".into(), source);
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
    let directory = bridge_paths().introspections_dir();
    let artifact_kind = if delivery.is_ok() {
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
        || "navigation only".into(),
        |page| {
            format!(
                "sha256:{}; bytes {}..{}",
                page.revision.sha256, page.start.byte, page.end.byte
            )
        },
    );
    let visibility = if delivery.is_ok() {
        "summary"
    } else {
        "protected"
    };
    let artifact = format!(
        "=== ASTRID INTROSPECTION ===\nSource: {source}\nSource revision: {revision}\nSource scope: local checkout; deployed behavior not established\nInput evidence: {}\nAccount: Astrid’s response to this input, not independently verified code facts.\nTimestamp: {timestamp}\nArtifact kind: {artifact_kind}\nVisibility: {visibility}\nLived-state witness: {}\nDelivery: {delivery_status}\n\n{text}",
        output.evidence_scope, authorship.witness_id()
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
    if mode == "self_study" {
        next_action::introspection_cadence::mark_admitted(conv, &artifact_path, witness);
        if let Some(page) = &output.page {
            finish_source_study_invitation(&catalog, &page.source);
        }
        (mode, text, source)
    } else {
        next_action::introspection_cadence::mark_failed(
            conv,
            delivery_status.clone(),
            written.is_ok().then_some(artifact_path.as_path()),
        );
        (
            mode,
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
