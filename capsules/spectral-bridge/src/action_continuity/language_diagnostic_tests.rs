use super::*;

#[test]
fn retired_drafts_stay_quiet_until_deliberately_accepted() {
    for reason in [
        "single_motif_overfit_risk",
        "map_release_before_intervening",
    ] {
        for with_experiment in [false, true] {
            let (_root, store, mut thread) = fixture();
            let experiment = with_experiment.then(|| {
                store
                    .start_experiment(None, "Retained work", "Chosen question")
                    .unwrap()
            });
            thread = store.current_thread().unwrap().unwrap_or(thread);
            let draft = store
                .append_continuity_session_draft(
                    &thread,
                    experiment.as_ref(),
                    reason,
                    "SELF_STUDY CONTINUE",
                    "Synthetic legacy generated summary",
                    vec!["synthetic/history".to_string()],
                    None,
                )
                .unwrap();
            let path = store.continuity_sessions_path(&thread.thread_id);
            let before = fs::read(&path).unwrap();
            let experiment_id = experiment.as_ref().map(|item| item.experiment_id.as_str());
            assert!(
                store
                    .continuity_session_line(&thread, experiment_id)
                    .is_empty()
            );
            let id = draft["record_id"].as_str().unwrap();
            let status = store.continuity_session_status_command(id).unwrap();
            let summary: Value = serde_json::from_str(status.split_once('\n').unwrap().1).unwrap();
            assert_eq!(summary["retained_draft"], draft);
            assert_eq!(fs::read(&path).unwrap(), before);
            assert_eq!(
                store.current_thread().unwrap().unwrap().current_next,
                thread.current_next
            );
            assert_eq!(
                store
                    .resolve_continuity_session_draft(&thread, Some(id))
                    .unwrap(),
                Some(draft.clone())
            );
            store.continuity_session_accept_command(id).unwrap();
            assert!(fs::read(&path).unwrap().starts_with(&before));
            assert!(
                store
                    .continuity_session_line(&thread, experiment_id)
                    .contains("Continuity session:")
            );
            let rows = store
                .continuity_session_rows(&thread.thread_id, experiment_id, 16)
                .unwrap();
            assert_eq!(rows.last().unwrap()["summary"], draft["summary"]);
        }
    }
}

#[test]
fn draft_retirement_requires_exact_generator_markers_and_never_hides_authored_sessions() {
    let (_root, store, thread) = fixture();
    let draft = store
        .append_continuity_session_draft(
            &thread,
            None,
            "single_motif_overfit_risk",
            "READ",
            "kept",
            vec![],
            None,
        )
        .unwrap();
    assert!(language_diagnostics::retired_lexical_session_draft(&draft));
    for (key, value) in [
        ("reason", json!("research_budget_required")),
        ("reason", json!({})),
        ("draft_v1", json!(false)),
        ("record_type", json!("session_start")),
        ("ignored_until_accepted", json!(false)),
        ("status", json!("active")),
    ] {
        let mut other = draft.clone();
        other[key] = value;
        assert!(
            !language_diagnostics::retired_lexical_session_draft(&other),
            "{key}"
        );
    }
    store
        .append_continuity_session_draft(
            &thread,
            None,
            "research_budget_required",
            "READ",
            "independent guard",
            vec![],
            None,
        )
        .unwrap();
    assert!(
        store
            .continuity_session_line(&thread, None)
            .contains("Continuity accept NEXT:")
    );
}

fn fixture() -> (tempfile::TempDir, ActionContinuityStore, ResearchThread) {
    let root = tempfile::tempdir().unwrap();
    let store = ActionContinuityStore::new(root.path().join("threads"));
    let thread = store
        .create_thread(None, "Synthetic reading", None)
        .unwrap();
    (root, store, thread)
}

#[test]
fn lexical_language_does_not_establish_state_or_author_a_followup() {
    let (_root, store, thread) = fixture();
    let text = "The spectral trace does NOT establish a single motif or constraint thinning.";
    let sources = [("synthetic/source".to_string(), text.to_string())];
    let risk = store
        .interpretation_risk_for_texts(&thread, None, sources.clone())
        .unwrap()
        .unwrap();
    let release = store
        .constraint_release_trajectory_for_texts(&thread, None, sources)
        .unwrap()
        .unwrap();
    for diagnostic in [&risk, &release] {
        assert_eq!(diagnostic["status"], "language_match");
        assert_eq!(diagnostic["physical_state_verified"], false);
        assert_eq!(diagnostic["selection_is_representative"], false);
        assert!(diagnostic["occurrence_rate"].is_null());
        for key in [
            "dossier_claim_next",
            "interpretation_next",
            "trajectory_next",
            "research_budget_next",
        ] {
            assert!(diagnostic[key].is_null(), "{key}");
        }
    }
    assert_eq!(release["state"], "not_measured");
    assert!(
        fs::read(store.continuity_sessions_path(&thread.thread_id))
            .unwrap()
            .is_empty()
    );
}

#[test]
fn one_match_is_sufficient_and_three_is_only_a_collection_cap() {
    let (root, store, thread) = fixture();
    let journal = root.path().join("journal");
    fs::create_dir(&journal).unwrap();
    fs::write(
        journal.join("one.txt"),
        "The spectral constraint is thinning.",
    )
    .unwrap();
    fs::write(
        journal.join("ordinary.txt"),
        "A reading completed uneventfully.",
    )
    .unwrap();
    let cue = store
        .constraint_release_trajectory_projection(&thread, None, None, &[])
        .unwrap()
        .unwrap();
    assert_eq!(cue["source_refs"].as_array().unwrap().len(), 1);
    assert_eq!(cue["source_selection_v1"]["match_required"], true);
    assert_eq!(
        cue["source_selection_v1"]["evidence_sufficiency_evaluated"],
        false
    );
    assert_eq!(cue["source_selection_v1"]["journal_match_limit"], 3);
    assert_eq!(cue["source_selection_v1"]["journal_candidate_limit"], 120);
    for index in 0..5 {
        fs::write(
            journal.join(format!("match{index}.txt")),
            "The spectral constraint is thinning.",
        )
        .unwrap();
    }
    assert_eq!(
        store.recent_constraint_release_trajectory_sources(3).len(),
        3
    );
    assert_eq!(
        fs::read_to_string(journal.join("ordinary.txt")).unwrap(),
        "A reading completed uneventfully."
    );
    assert_eq!(
        store.recent_constraint_release_trajectory_sources(0).len(),
        0
    );
}

#[test]
fn language_match_event_preserves_choice_without_a_session_draft() {
    let (_root, store, thread) = fixture();
    let telemetry: SpectralTelemetry = serde_json::from_value(json!({
        "t_ms": 0, "eigenvalues": [1.0], "fill_ratio": 0.68
    }))
    .unwrap();
    let command = "CREATE a poem about spectral constraint thinning and a single motif";
    let event = store
        .record_next_event(
            None,
            command,
            command,
            command,
            &NextActionOutcome::handled("create", "Saved the selected topic"),
            68.0,
            &telemetry,
            command,
        )
        .unwrap();
    assert_eq!(event.raw_next.as_deref(), Some(command));
    assert!(event.research_budget_v1.is_none());
    assert!(
        fs::read(store.continuity_sessions_path(&thread.thread_id))
            .unwrap()
            .is_empty()
    );
    assert!(event.interpretation_risk_v1.is_some());
    let next = fs::read_to_string(store.thread_dir(&thread.thread_id).join("next.md")).unwrap();
    assert!(!next.contains("Dossier interpretation NEXT"));
    assert!(!next.contains("spontaneous release watch"));
    assert!(!next.contains("Selected-language diagnostic"));
}

#[test]
fn legacy_inspection_does_not_reanimate_editorial_commands() {
    let legacy = Some(json!({
        "status": "detected", "state": "spontaneous_release_watch",
        "source_refs": ["synthetic/source"], "matched_terms": ["thinning"],
        "dossier_claim_next": "DOSSIER_CLAIM latest :: claim: do not apply direct leak",
        "trajectory_next": "CONTINUITY_SESSION_START latest :: title: imposed watch"
    }));
    let before = serde_json::to_vec(&legacy).unwrap();
    let line = ActionContinuityStore::constraint_release_trajectory_line(&legacy);
    assert!(line.contains("Selected-language diagnostic"));
    assert!(line.contains("not representative history or measured state"));
    assert!(!line.contains("DOSSIER_CLAIM"));
    assert!(!line.contains("CONTINUITY_SESSION_START"));
    assert_eq!(serde_json::to_vec(&legacy).unwrap(), before);
}

#[test]
fn merged_cap_and_summary_fallback_do_not_imply_representative_history() {
    let (_root, store, thread) = fixture();
    let sources = (0..8).map(|n| {
        (
            format!("synthetic/{n}"),
            "Spectral constraint thinning".to_string(),
        )
    });
    let cue = store
        .constraint_release_trajectory_for_texts(&thread, None, sources)
        .unwrap()
        .unwrap();
    assert_eq!(cue["source_refs"].as_array().unwrap().len(), 4);
    let summary = json!({"account": "Spectral constraint thinning", "status": "completed"});
    let cue = store
        .constraint_release_trajectory_projection(&thread, None, Some(&summary), &[])
        .unwrap()
        .unwrap();
    assert_eq!(cue["source_selection_v1"]["summary_fallback"], true);
    assert_eq!(cue["status"], "language_match");
    assert_eq!(cue["physical_state_verified"], false);
    assert_eq!(summary["status"], "completed");
}

#[test]
fn older_derived_projection_refreshes_without_rewriting_history() {
    let (_root, store, mut thread) = fixture();
    store
        .refresh_projection_freshness_v1(&mut thread, "test")
        .unwrap();
    let fingerprints =
        thread.projection_freshness_v1.as_ref().unwrap()["source_fingerprints"].clone();
    thread.projection_freshness_v1.as_mut().unwrap()["schema_version"] = json!(2);
    assert!(store.projection_freshness_stale_v1(&thread));
    let history = fs::read(store.thread_dir(&thread.thread_id).join("events.jsonl")).unwrap();
    store
        .refresh_projection_freshness_v1(&mut thread, "test")
        .unwrap();
    assert_eq!(
        thread.projection_freshness_v1.as_ref().unwrap()["schema_version"],
        4
    );
    assert_eq!(
        thread.projection_freshness_v1.as_ref().unwrap()["source_fingerprints"],
        fingerprints
    );
    assert_eq!(
        fs::read(store.thread_dir(&thread.thread_id).join("events.jsonl")).unwrap(),
        history
    );
}

#[test]
fn lexical_or_null_diagnostics_cannot_manufacture_capture_priority() {
    let baseline = json!({
        "lifecycle_stage": "active", "lifecycle_next": "SELF_STUDY MAP"
    });
    let expected = build_control_plane_v1(&baseline);
    for value in [
        Value::Null,
        json!({"status": "detected", "state": "spontaneous_release_watch"}),
        json!({"status": "language_match", "diagnostic_version": 2}),
    ] {
        for key in ["interpretation_risk_v1", "constraint_release_trajectory_v1"] {
            let mut input = baseline.clone();
            input[key] = value.clone();
            assert_eq!(build_control_plane_v1(&input), expected, "{key}: {value}");
        }
    }
    let explicit = json!({
        "continuity_session_v1": {
            "status": "active", "suggested_next": "CONTINUITY_SESSION_CAPTURE selected"
        }
    });
    let selected = build_control_plane_v1(&explicit);
    assert_eq!(
        selected["primary_route"]["command"],
        "CONTINUITY_SESSION_CAPTURE selected"
    );
    assert_eq!(selected["primary_route"]["source"], "continuity_session_v1");
}

#[test]
fn session_guard_projection_is_guidance_not_a_permission_decision_or_write() {
    let (_root, store, _) = fixture();
    let experiment = store
        .start_experiment(None, "Synthetic scope", "What does the guard consume?")
        .unwrap();
    let thread = store.current_thread().unwrap().unwrap();
    let paths = [
        store.thread_dir(&thread.thread_id).join("thread.json"),
        store.thread_dir(&thread.thread_id).join("events.jsonl"),
        store.authority_gate_path(&thread.thread_id),
        store.continuity_sessions_path(&thread.thread_id),
    ];
    let before = paths.each_ref().map(|path| fs::read(path).ok());
    let guidance = store
        .continuity_session_guard_projection(&thread, &experiment)
        .unwrap();
    assert!(
        guidance["suggested_next"]
            .as_str()
            .unwrap()
            .starts_with("CONTINUITY_SESSION_START")
    );
    for absent in ["eligible", "allowed", "blocked", "missing_requirements"] {
        assert!(guidance.get(absent).is_none(), "{absent}");
    }
    assert_eq!(guidance["authority_change"], false);
    assert_eq!(before, paths.each_ref().map(|path| fs::read(path).ok()));
}

#[test]
fn matched_study_topic_remains_reading_with_an_active_experiment() {
    let (_root, store, _) = fixture();
    store
        .start_experiment(None, "Synthetic scope", "Does naming a risk block reading?")
        .unwrap();
    let thread = store.current_thread().unwrap().unwrap();
    let command = "SELF_STUDY FIND spectral single motif constraint thinning";
    let telemetry: SpectralTelemetry = serde_json::from_value(json!({
        "t_ms": 0, "eigenvalues": [1.0], "fill_ratio": 0.68
    }))
    .unwrap();
    assert!(
        store
            .charter_required_guard_assessment(command)
            .unwrap()
            .is_none()
    );
    assert!(
        store
            .research_budget_guard_assessment(command, 68.0, &telemetry)
            .unwrap()
            .is_none()
    );
    let sessions = fs::read(store.continuity_sessions_path(&thread.thread_id)).unwrap();
    let authority = fs::read(store.authority_gate_path(&thread.thread_id)).ok();
    let event = store
        .record_next_event(
            None,
            command,
            command,
            command,
            &NextActionOutcome::handled("source_study", "Synthetic navigation delivered"),
            68.0,
            &telemetry,
            "Synthetic navigation delivered",
        )
        .unwrap();
    assert!(event.research_budget_v1.is_none());
    assert!(event.charter_required_guard_v1.is_none());
    assert_eq!(event.route, "source_study");
    assert_ne!(event.status, "blocked");
    assert_eq!(event.raw_next.as_deref(), Some(command));
    assert_eq!(
        event.interpretation_risk_v1.unwrap()["status"],
        "language_match"
    );
    assert_eq!(
        event.constraint_release_trajectory_v1.unwrap()["state"],
        "not_measured"
    );
    assert_eq!(
        fs::read(store.continuity_sessions_path(&thread.thread_id)).unwrap(),
        sessions
    );
    assert_eq!(
        fs::read(store.authority_gate_path(&thread.thread_id)).ok(),
        authority
    );
}
