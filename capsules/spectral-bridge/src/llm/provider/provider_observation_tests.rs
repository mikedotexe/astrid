#[cfg(test)]
mod provider_observation_tests {
    use super::*;
    use std::sync::Arc;

    fn context(root: &std::path::Path) -> ProviderObservationContext {
        ProviderObservationContext {
            store: Some(Arc::new(ProviderObservationStore::new(
                root.join("evidence"),
            ))),
            generation_id: Some("fixture-generation".into()),
            logical_attempt_index: Some(1),
            attempts: Arc::default(),
        }
    }

    fn outcomes(ctx: &ProviderObservationContext) -> Vec<serde_json::Value> {
        std::fs::read_dir(ctx.store.as_ref().unwrap().root.join("events"))
            .unwrap()
            .map(Result::unwrap)
            .filter(|entry| {
                entry
                    .file_name()
                    .to_string_lossy()
                    .ends_with("-outcome.json")
            })
            .map(|entry| serde_json::from_slice(&std::fs::read(entry.path()).unwrap()).unwrap())
            .collect()
    }

    fn observe(ctx: &ProviderObservationContext, raw: &str) -> String {
        let mut observer =
            ProviderAttemptObserver::begin("fixture", "mlx", "configured", b"request", Some(ctx));
        let normalized = normalize_provider_output_v1(raw);
        ProviderAttemptObserver::normalized(&mut observer, raw, &normalized);
        ProviderAttemptObserver::returned(&mut observer, &normalized.text);
        normalized.text
    }

    #[test]
    fn raw_artifacts_are_exact_private_and_deduplicated_without_collapsing_attempts() {
        use std::os::unix::fs::PermissionsExt as _;
        let temp = tempfile::tempdir().unwrap();
        let ctx = context(temp.path());
        let raw = "  α🙂 <end_of_turn> marker token.\nNEXT: LISTEN  ";
        let expected = normalize_provider_output_v1(raw).text;
        assert_eq!(observe(&ctx, raw), expected);
        assert_eq!(observe(&ctx, raw), expected);
        let records = outcomes(&ctx);
        assert_eq!(records.len(), 2);
        assert_ne!(records[0]["attempt_id"], records[1]["attempt_id"]);
        let root = &ctx.store.as_ref().unwrap().root;
        assert_eq!(std::fs::read_dir(root.join("raw")).unwrap().count(), 1);
        for record in records {
            assert_eq!(record["raw_response_sha256"], generation_sha256_hex(raw));
            assert_eq!(
                record["normalized_output_sha256"],
                generation_sha256_hex(&expected)
            );
            assert_eq!(record["input_availability"], "retained_exact");
            let path = root.join(record["raw_artifact"].as_str().unwrap());
            assert_eq!(std::fs::read_to_string(&path).unwrap(), raw);
            assert_eq!(path.metadata().unwrap().permissions().mode() & 0o777, 0o600);
            assert_eq!(record["release_before"]["status"], "activation_unknown");
            assert_eq!(record["release_before"], record["release_after"]);
        }
        for dir in [root.clone(), root.join("raw"), root.join("events")] {
            assert_eq!(dir.metadata().unwrap().permissions().mode() & 0o777, 0o700);
        }
    }

    #[test]
    fn zero_markers_and_omitted_input_are_distinct() {
        let temp = tempfile::tempdir().unwrap();
        let ctx = context(temp.path());
        observe(&ctx, "No control markers here.");
        let large = format!("<end_of_turn>{}", "x".repeat(PROVIDER_RAW_MAX_BYTES));
        observe(&ctx, &large);
        let records = outcomes(&ctx);
        assert!(records.iter().any(|r| r["marker_observed_total"] == 0
            && r["input_availability"] == "observed_no_markers_raw_not_retained"));
        assert!(records.iter().any(|r| r["marker_observed_total"] == 1
            && r["input_availability"] == "input_not_retained_size_limit"));
        assert_eq!(
            std::fs::read_dir(ctx.store.as_ref().unwrap().root.join("raw"))
                .unwrap()
                .count(),
            0
        );
    }

    #[test]
    fn graceful_drop_and_crash_pending_receipt_never_claim_success() {
        let temp = tempfile::tempdir().unwrap();
        let ctx = context(temp.path());
        let pending =
            ProviderAttemptObserver::begin("fixture", "mlx", "model", b"request", Some(&ctx));
        assert_eq!(outcomes(&ctx).len(), 0);
        let entries = std::fs::read_dir(ctx.store.as_ref().unwrap().root.join("events"))
            .unwrap()
            .count();
        assert_eq!(entries, 1); // Dispatch exists before a response or Drop.
        drop(pending);
        assert_eq!(outcomes(&ctx)[0]["outcome"], "cancelled_or_abandoned");
        assert!(outcomes(&ctx)[0]["marker_observed_total"].is_null());
    }

    #[test]
    fn raw_quota_still_records_the_outcome_and_output() {
        let temp = tempfile::tempdir().unwrap();
        let mut ctx = context(temp.path());
        Arc::get_mut(ctx.store.as_mut().unwrap())
            .unwrap()
            .max_raw_bytes = 1;
        let raw = "<end_of_turn> Words continue.";
        assert_eq!(observe(&ctx, raw), normalize_provider_output_v1(raw).text);
        let records = outcomes(&ctx);
        assert_eq!(records[0]["input_availability"], "input_not_retained_quota");
        assert_eq!(records[0]["outcome"], "provider_returned");
    }

    #[test]
    fn recording_failure_survives_in_generation_join_without_changing_text() {
        let temp = tempfile::tempdir().unwrap();
        let ctx = context(temp.path());
        std::fs::write(temp.path().join("evidence"), "block directory creation").unwrap();
        assert_eq!(observe(&ctx, "plain text"), "plain text");
        let decision = ctx.finish_dialogue(Some("plain text"));
        assert_eq!(decision.attempts.len(), 1);
        assert_eq!(decision.attempts[0].recording_status, "recording_failed");
        assert_eq!(decision.decision_recording_status, "recording_failed");
        assert!(
            ctx.store
                .as_ref()
                .unwrap()
                .failures
                .load(std::sync::atomic::Ordering::Relaxed)
                >= 3
        );
    }

    #[test]
    fn cleanup_receipts_remain_bounded_and_keep_utf8_offsets() {
        let temp = tempfile::tempdir().unwrap();
        let ctx = context(temp.path());
        let raw = format!("🙂 {}", "<end_of_turn> ".repeat(40));
        observe(&ctx, &raw);
        let report = outcomes(&ctx).remove(0)["cleanup_report"].clone();
        let receipts = report["context_receipts"].as_array().unwrap();
        assert_eq!(receipts.len(), 32);
        assert_eq!(report["context_receipts_omitted"], 8);
        assert_eq!(receipts[0]["start_byte"], 5);
    }

    #[test]
    fn second_writer_and_symlink_artifact_are_refused() {
        let temp = tempfile::tempdir().unwrap();
        let ctx = context(temp.path());
        observe(&ctx, "first");
        let other = context(temp.path());
        observe(&other, "second");
        assert_eq!(
            other.finish_dialogue(None).attempts[0].recording_status,
            "recording_failed"
        );
        let raw = "<end_of_turn> private";
        let root = &ctx.store.as_ref().unwrap().root;
        let external = temp.path().join("external");
        std::fs::write(&external, "unchanged").unwrap();
        std::os::unix::fs::symlink(
            &external,
            root.join("raw")
                .join(format!("{}.txt", generation_sha256_hex(raw))),
        )
        .unwrap();
        assert_eq!(
            ctx.store.as_ref().unwrap().write_raw(raw),
            Err(ProviderObservationWriteError::Io)
        );
        assert_eq!(std::fs::read_to_string(external).unwrap(), "unchanged");
    }

    #[test]
    fn disabled_observer_creates_no_store_or_attempts() {
        let ctx = ProviderObservationContext::configured(Some("disabled"), Some(0));
        assert!(
            ProviderAttemptObserver::begin("fixture", "mlx", "model", b"request", Some(&ctx))
                .is_none()
        );
        let result = ctx.finish_dialogue(None);
        assert_eq!(result.status, "disabled");
        assert!(result.attempts.is_empty());
    }

    #[test]
    fn spool_budget_survives_restart_and_never_deletes_evidence() {
        let temp = tempfile::tempdir().unwrap();
        let root = temp.path().join("evidence");
        {
            let store = ProviderObservationStore::new(root.clone());
            store.write_raw("kept exactly").unwrap();
        }
        let mut store = ProviderObservationStore::new(root.clone());
        store.max_bytes = 1;
        assert_eq!(
            store.write_raw("new bytes"),
            Err(ProviderObservationWriteError::Quota)
        );
        assert!(store.write_raw("kept exactly").is_ok());
        assert_eq!(std::fs::read_dir(root.join("raw")).unwrap().count(), 1);
    }

    #[test]
    fn oversized_model_identity_is_hash_only_not_a_false_exact_name() {
        let temp = tempfile::tempdir().unwrap();
        let ctx = context(temp.path());
        let mut observer =
            ProviderAttemptObserver::begin("fixture", "mlx", "configured", b"request", Some(&ctx));
        let model = "m".repeat(257);
        ProviderAttemptObserver::model(&mut observer, Some(&model));
        drop(observer);
        let record = outcomes(&ctx).remove(0);
        assert!(record["reported_model"].is_null());
        assert_eq!(
            record["reported_model_sha256"],
            generation_sha256_hex(&model)
        );
        assert_eq!(record["reported_model_status"], "too_long_hash_only");
    }

    // Invoked by scripts/qualify_provider_observation.py in a fresh process,
    // against its loopback mock server and private temporary workspace. No
    // environment mutation or live model connection occurs inside this test.
    #[tokio::test]
    async fn transport_worker() {
        let Ok(case_path) = std::env::var("ASTRID_OBSERVATION_TEST_CASE") else {
            return;
        };
        let case: serde_json::Value =
            serde_json::from_slice(&std::fs::read(&case_path).unwrap()).unwrap();
        let output = std::path::PathBuf::from(case["output"].as_str().unwrap());
        let mut ctx = context(output.parent().unwrap());
        if case["observer"] == "disabled" {
            ctx.store = None;
        }
        if case["observer"] == "fault" {
            std::fs::write(output.parent().unwrap().join("evidence"), "fault").unwrap();
        }
        if let Some(manifest) = case["manifest"].as_str() {
            crate::deployment::configure(Some(std::path::Path::new(manifest))).unwrap();
        }
        let protected = (case["protected"] == true).then(|| ProtectedDialogueInputV1 {
            content_id: "synthetic-protected-source".into(),
            kind: ProtectedDialogueKindV1::Reading,
            source_text: "Synthetic qualification passage.".into(),
            source_start_byte: 0,
            reply_message_id: None,
        });
        let messages = vec![Message {
            role: "user".into(),
            content: "Synthetic qualification request.".into(),
        }];
        let started = std::time::Instant::now();
        let (provider_text, accepted, model) = if case["provider"] == "mlx" {
            let response = mlx_chat_with_runtime_feedback(
                "observation_fixture",
                messages,
                0.7,
                64,
                1,
                MlxFailureLogMode::FallbackEligible,
                protected.as_ref(),
                &[],
                None,
                Some(&ctx),
            )
            .await;
            let text = response.as_ref().map(|r| r.text.clone());
            let accepted = response
                .and_then(|r| accept_primary_dialogue_with_feedback(r, configured_mlx_profile()))
                .map(|r| r.text);
            (text, accepted, None)
        } else {
            let response = ollama_chat_with_runtime_feedback(
                "observation_fixture",
                messages,
                0.7,
                64,
                1,
                None,
                protected.as_ref(),
                &[],
                None,
                Some(&ctx),
            )
            .await;
            let text = response.as_ref().map(|r| r.text.clone());
            let model = response.as_ref().map(|r| r.model.clone());
            let accepted = response
                .and_then(|r| accept_ollama_dialogue_with_feedback(r, configured_mlx_profile()))
                .map(|r| r.text);
            (text, accepted, model)
        };
        let observation = ctx.finish_dialogue(accepted.as_deref());
        std::fs::write(
            output,
            serde_json::to_vec_pretty(&serde_json::json!({
                "provider_text": provider_text, "accepted": accepted, "model": model,
                "observation": observation, "elapsed_ms": started.elapsed().as_secs_f64() * 1000.0,
            }))
            .unwrap(),
        )
        .unwrap();
    }
}
