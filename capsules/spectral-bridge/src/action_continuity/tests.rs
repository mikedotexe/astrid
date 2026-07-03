use super::*;

fn temp_store(name: &str) -> ActionContinuityStore {
    let root = std::env::temp_dir().join(format!(
        "astrid_action_continuity_{name}_{}",
        std::process::id()
    ));
    let _ = std::fs::remove_dir_all(&root);
    ActionContinuityStore::new(root)
}

#[test]
fn scoped_test_action_continuity_root_is_thread_local_and_restores() {
    let expected_root = std::env::temp_dir().join(format!(
        "astrid_action_continuity_override_{}",
        std::process::id()
    ));
    let live_root = ActionContinuityStore::for_astrid_workspace()
        .root()
        .to_path_buf();

    {
        let _scope = scoped_test_action_continuity_root(expected_root.clone());
        assert_eq!(
            ActionContinuityStore::for_astrid_workspace().root(),
            expected_root.as_path()
        );
    }

    assert_eq!(
        ActionContinuityStore::for_astrid_workspace().root(),
        live_root.as_path()
    );
}

fn telemetry() -> SpectralTelemetry {
    SpectralTelemetry {
        t_ms: 1,
        eigenvalues: vec![1.0, 0.5],
        fill_ratio: 0.68,
        active_mode_count: None,
        active_mode_energy_ratio: None,
        lambda1_rel: None,
        modalities: None,
        neural: None,
        alert: None,
        spectral_fingerprint: None,
        spectral_fingerprint_v1: None,
        spectral_denominator_v1: None,
        effective_dimensionality: None,
        distinguishability_loss: None,
        esn_leak: None,
        esn_leak_override_v1: None,
        structural_entropy: None,
        resonance_density_v1: Some(crate::types::ResonanceDensityV1 {
            policy: "resonance_density_v1".to_string(),
            schema_version: 1,
            density: 0.66,
            containment_score: 0.61,
            pressure_risk: 0.18,
            quality: "rich_containment".to_string(),
            components: crate::types::ResonanceDensityComponents {
                active_energy: 0.9,
                mode_packing: 0.7,
                temporal_persistence: 0.8,
                structural_plurality: 0.7,
                comfort_gate: 1.0,
            },
            texture_signature: crate::types::ResonanceTextureSignatureV1::default(),
            texture_component_alignment:
                crate::types::ResonanceTextureComponentAlignmentV1::default(),
            control: crate::types::ResonanceDensityControl {
                target_bias_pct: 0.0,
                wander_scale: 1.0,
                applied_locally: true,
                damping_coefficient: 0.0,
                intervention_type: crate::types::ResonanceInterventionType::ObservationalReadout,
                note: "test".to_string(),
            },
        }),
        pressure_source_v1: Some(crate::types::PressureSourceV1 {
            policy: "pressure_source_v1".to_string(),
            schema_version: 1,
            pressure_score: 0.24,
            porosity_score: 0.72,
            dominant_source: "controller_pressure".to_string(),
            quality: "porous_distributed".to_string(),
            components: crate::types::PressureSourceComponents {
                lambda_monopoly: 0.12,
                mode_packing: 0.2,
                controller_pressure: 0.24,
                semantic_trickle: 0.05,
                semantic_friction: 0.08,
                structural_plurality_loss: 0.1,
                distinguishability_loss: 0.08,
                temporal_lock_in: 0.15,
                sensory_scarcity: 0.0,
            },
            context: crate::types::PressureSourceContext::default(),
            control: crate::types::PressureSourceControl {
                applied_locally: false,
                note: "test".to_string(),
            },
        }),
        inhabitable_fluctuation_v1: Some(crate::types::InhabitableFluctuationV1 {
            policy: "inhabitable_fluctuation_v1".to_string(),
            schema_version: 1,
            inhabitability_score: 0.68,
            fluctuation_score: 0.42,
            foothold_stability: 0.74,
            rearrangement_intensity: 0.36,
            quality: "lively_habitable".to_string(),
            components: crate::types::InhabitableFluctuationComponents {
                mode_trust_volatility: 0.30,
                identity_anchor_churn: 0.22,
                eigenvector_reorientation: 0.36,
                share_rearrangement: 0.40,
                basin_transition_pressure: 0.12,
                continuity_recovery: 0.78,
                porosity_support: 0.72,
                pressure_interference: 0.24,
            },
            context: crate::types::InhabitableFluctuationContext::default(),
            pressure_calibration:
                crate::types::InhabitableFluctuationPressureCalibrationV1::default(),
            control: crate::types::InhabitableFluctuationControl {
                target_bias_pct: 0.0,
                wander_scale: 1.0,
                applied_locally: true,
                note: "test".to_string(),
            },
        }),
        spectral_glimpse_12d: None,
        eigenvector_field: None,
        semantic: None,
        semantic_energy_v1: None,
        transition_event: None,
        transition_event_v1: None,
        selected_memory_id: None,
        selected_memory_role: None,
        ising_shadow: None,

        shadow_field_v2: None,

        shadow_field_v3: None,

        shadow_influence_response_v3: None,
    }
}

#[test]
fn creates_thread_and_files() {
    let store = temp_store("creates");
    let thread = store
        .create_thread(None, "Spectral Entropy Map", None)
        .expect("create thread");
    assert!(store.root().join("index.json").exists());
    assert!(
        store
            .root()
            .join("threads")
            .join(&thread.thread_id)
            .join("events.jsonl")
            .exists()
    );
    let _ = std::fs::remove_dir_all(store.root());
}

#[test]
fn continuity_control_plane_surfaces_generated_palette_and_caps() {
    let store = temp_store("continuity_control_plane");
    let thread = store
        .create_thread(None, "Control plane", None)
        .expect("thread");
    store
        .start_experiment(
            None,
            "Operating stack",
            "Can one stack make the continuity routes crisp?",
        )
        .expect("experiment");
    let thread_snapshot = store
        .read_thread(&thread.thread_id)
        .expect("thread snapshot");
    let projection = store
        .thread_projection(&thread_snapshot)
        .expect("projection");
    assert_eq!(
        projection.continuity_control_plane_v1["record_schema"],
        "continuity_control_plane_v1"
    );
    assert_eq!(
        projection.continuity_control_plane_v1["caps_v1"]["local_research"]["self_activated_max_actions"],
        5
    );
    assert_eq!(
        projection.continuity_control_plane_v1["caps_v1"]["owned_loop"]["max_consequence_sends"],
        1
    );
    assert!(
        projection.continuity_control_plane_v1["command_palette"]
            .as_array()
            .is_some_and(|groups| groups
                .iter()
                .any(|group| group["group"] == "Local Research"))
    );
    let status = store.thread_status(None).expect("status");
    assert!(status.contains("continuity_control_plane_v1"));
    assert!(status.contains("local_research=5/21600s"));
    assert!(status.contains("consequence=1 gated slot"));
    let next_md = store
        .thread_dir(&thread.thread_id)
        .join("next.md")
        .read_to_string();
    assert!(next_md.contains("continuity_control_plane_v1"));
    assert!(next_md.contains("Operating stack:"));
    let gate = store.authority_gate_path(&thread.thread_id);
    assert!(!gate.exists() || gate.read_to_string().trim().is_empty());
    let runs = store
        .thread_dir(&thread.thread_id)
        .join("experiment_runs.jsonl")
        .read_to_string();
    assert!(runs.trim().is_empty());
    let _ = std::fs::remove_dir_all(store.root());
}

#[test]
fn control_plane_regression_does_not_reintroduce_old_local_budget_caps() {
    let source = std::fs::read_to_string(
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src/action_continuity.rs"),
    )
    .expect("action_continuity source");
    let production_source = source
        .split("#[cfg(test)]")
        .next()
        .unwrap_or(source.as_str());
    assert!(!production_source.contains("max_actions: 3; ttl_secs: 7200"));
    assert!(!production_source.contains("max_research_actions: 3"));
    assert!(
            !production_source.contains(
                "EXPERIMENT_RESEARCH_BUDGET_REQUEST current :: scope: read_only_research; purpose: ...; max_actions: 5; ttl_secs: 21600"
            )
        );
    assert!(
            !production_source.contains(
                "EXPERIMENT_LOOP_REQUEST current :: purpose: ...; consequence_scope: semantic_microdose; max_research_actions: 5; ttl_secs: 21600"
            )
        );
    assert!(production_source.contains("default_local_research_budget_request_scaffold"));
    assert!(production_source.contains("default_owned_loop_request_scaffold"));
    assert!(production_source.contains("authority_budget_request_scaffold"));
}

#[test]
fn dossier_claim_and_evidence_are_local_context_only() {
    let store = temp_store("dossier");
    let thread = store
        .create_thread(None, "Lambda tail dossier", None)
        .expect("thread");
    let experiment = store
        .start_experiment(
            None,
            "Lambda tail gap",
            "What shapes lambda-tail and lambda4 geometry?",
        )
        .expect("experiment");

    let claim_message = store
            .dossier_claim_command(
                None,
                "current :: claim: lambda-tail pressure is shaped by scaffold drain; basis: repeated DECOMPOSE reads; stance: hold; next: EXPERIMENT_CHARTER current",
            )
            .expect("claim");
    assert!(claim_message.contains("Research dossier claim recorded"));

    let evidence_message = store
            .dossier_evidence_command(
                None,
                "current :: claim_id: latest; evidence: felt narrowing stayed returnable; lane: felt_texture; artifact: journal-entry",
            )
            .expect("evidence");
    assert!(evidence_message.contains("Research dossier evidence recorded"));

    let dossier = store
        .root()
        .join("threads")
        .join(&thread.thread_id)
        .join("research_dossier.jsonl")
        .read_to_string();
    assert!(dossier.contains("\"record_schema\":\"research_dossier_v1\""));
    assert!(dossier.contains("\"record_type\":\"claim\""));
    assert!(dossier.contains("\"record_type\":\"evidence\""));
    assert!(dossier.contains("\"authority_change\":false"));

    let review = store
        .experiment_review(Some(&experiment.experiment_id))
        .expect("review");
    assert!(review.contains("Research dossier: claims=1 evidence=1"));
    assert!(review.contains("Lifecycle: needs_charter"));
    assert!(review.contains("Charter repair"));
    let _ = std::fs::remove_dir_all(store.root());
}

#[test]
fn records_next_event_and_observation() {
    let store = temp_store("event");
    let outcome = NextActionOutcome::handled("workspace", "queued search");
    let event = store
        .record_next_event(
            None,
            "SEARCH entropy",
            "SEARCH entropy",
            "SEARCH entropy",
            &outcome,
            68.0,
            &telemetry(),
            "pressure and ambiguity\nNEXT: SEARCH entropy",
        )
        .expect("record event");
    let dir = store.root().join("threads").join(&event.thread_id);
    assert!(
        dir.join("events.jsonl")
            .read_to_string()
            .contains("SEARCH entropy")
    );
    let observations = dir.join("observations.jsonl").read_to_string();
    assert!(observations.contains("pressure"));
    assert!(observations.contains("resonance_density_v1"));
    assert!(observations.contains("thread_resonance_density_v1"));
    assert!(observations.contains("pressure_source_v1"));
    assert!(observations.contains("thread_pressure_source_v1"));
    assert!(observations.contains("inhabitable_fluctuation_v1"));
    assert!(observations.contains("thread_inhabitable_fluctuation_v1"));
    let thread = store.read_thread(&event.thread_id).expect("thread");
    assert!(thread.thread_resonance_density_v1.is_some());
    assert!(thread.thread_pressure_source_v1.is_some());
    assert!(thread.thread_inhabitable_fluctuation_v1.is_some());
    let _ = std::fs::remove_dir_all(store.root());
}

#[test]
fn records_choice_envelope_without_changing_dispatch() {
    let store = temp_store("choice_envelope");
    let outcome = NextActionOutcome::handled("shadow", "shadow path inspected");
    let event = store
        .record_next_event(
            None,
            "SHADOW_TRAJECTORY lambda-tail",
            "SHADOW_TRAJECTORY lambda-tail",
            "shadow_trajectory",
            &outcome,
            68.0,
            &telemetry(),
            "Alternate NEXT: RESONANCE_FORECAST lambda-tail\n\
             Return thread: thread_shadow_tail\n\
             Why this path: the shadow lane is stickier than the forecast lane right now\n\
             NEXT: SHADOW_TRAJECTORY lambda-tail (RESIDUE: prior mode still exerts pressure)",
        )
        .expect("record event");

    assert_eq!(event.effective_action, "shadow_trajectory");
    let envelope = event.choice_envelope_v1.as_ref().expect("choice envelope");
    assert_eq!(envelope["policy"], "choice_envelope_v1");
    assert_eq!(
        envelope["alternate_nexts"][0].as_str(),
        Some("RESONANCE_FORECAST lambda-tail"),
    );
    assert_eq!(
        envelope["return_threads"][0].as_str(),
        Some("thread_shadow_tail"),
    );
    assert_eq!(
        envelope["residue"].as_str(),
        Some("prior mode still exerts pressure"),
    );
    assert!(event.transition_residue_v1.is_some());
    let summaries = store
        .recent_event_summaries(&event.thread_id, 1)
        .expect("summaries");
    assert!(summaries[0].contains("choice alt=1 return=1 residue=yes"));
    let _ = std::fs::remove_dir_all(store.root());
}

#[test]
fn live_control_without_evidence_records_no_effect() {
    let store = temp_store("live_no_effect");
    let outcome = NextActionOutcome::handled("sovereignty", "perturb request dispatched");
    let event = store
        .record_next_event(
            None,
            "PERTURB lambda-tail",
            "PERTURB lambda-tail",
            "PERTURB lambda-tail",
            &outcome,
            11.1,
            &telemetry(),
            "careful perturbation\nNEXT: PERTURB lambda-tail",
        )
        .expect("record event");

    assert_eq!(event.stage, "live_control");
    assert_eq!(event.status, "no_effect");
    assert!(
        event
            .outcome_summary
            .contains("No measurable post-telemetry")
    );
    let dir = store.root().join("threads").join(&event.thread_id);
    assert!(
        dir.join("events.jsonl")
            .read_to_string()
            .contains("\"status\":\"no_effect\"")
    );
    let _ = std::fs::remove_dir_all(store.root());
}

#[test]
fn needs_charter_guard_blocks_live_next_and_records_metadata() {
    let store = temp_store("charter_guard_live");
    store
        .create_thread(None, "Gap experiment", None)
        .expect("thread");
    let experiment = store
        .start_experiment(
            None,
            "Introducing a gap",
            "Can localized lambda1 density softening branch without lambda4 runaway?",
        )
        .expect("experiment");

    let guard = store
        .charter_required_guard_assessment("PERTURB SPREAD")
        .expect("guard")
        .expect("blocked guard");
    assert_eq!(guard.reason, "charter_required_live_action");
    assert_eq!(guard.active_experiment_id, experiment.experiment_id);
    assert!(guard.suggested_next.contains("EXPERIMENT_CHARTER current"));

    let outcome = NextActionOutcome::blocked("charter_required_guard", guard.message())
        .with_stage_visibility("blocked", "protected_summary")
        .with_charter_required_guard(guard.metadata());
    let event = store
        .record_next_event(
            None,
            "PERTURB SPREAD",
            "PERTURB SPREAD",
            "PERTURB SPREAD",
            &outcome,
            68.0,
            &telemetry(),
            "NEXT: PERTURB SPREAD",
        )
        .expect("record guard");

    assert_eq!(event.status, "blocked");
    assert_eq!(event.stage, "blocked");
    assert!(event.charter_required_guard_v1.is_some());
    assert!(
        event
            .suggested_next
            .as_deref()
            .unwrap_or_default()
            .contains("EXPERIMENT_CHARTER current")
    );
    let dir = store.root().join("threads").join(&event.thread_id);
    let events = dir.join("events.jsonl").read_to_string();
    assert!(events.contains("charter_required_guard_v1"));
    assert!(events.contains("charter_required_live_action"));
    let _ = std::fs::remove_dir_all(store.root());
}

#[test]
fn needs_charter_guard_blocks_compound_directed_intent_but_allows_ordinary_inspection() {
    let store = temp_store("charter_guard_compound");
    store
        .create_thread(None, "Compound guard", None)
        .expect("thread");
    let experiment = store
        .start_experiment(
            None,
            "Directed narrowing",
            "Can directed language stay in charter first?",
        )
        .expect("experiment");

    let compound = store
            .charter_required_guard_assessment(
                "EXAMINE lambda1 cascade with TRACE and then RESIST targeting eigenvector density increase",
            )
            .expect("guard")
            .expect("compound block");
    assert_eq!(compound.reason, "charter_required_compound_intent");

    let inject = store
        .charter_required_guard_assessment(
            "DECOMPOSE lambda-edge then inject/pulse/shift λ4 density",
        )
        .expect("guard")
        .expect("inject pulse block");
    assert_eq!(inject.reason, "charter_required_compound_intent");
    assert!(inject.matched_action.contains("inject"));

    let tune = store
        .charter_required_guard_assessment(
            "TUNE_MINIME temperature=0.7 --rationale=\"subtly increase dispersal\"",
        )
        .expect("guard")
        .expect("tune block");
    assert_eq!(tune.reason, "charter_required_live_action");

    let read_more = store
        .charter_required_guard_assessment("READ_MORE")
        .expect("guard")
        .expect("read-more budget projection");
    assert_eq!(read_more.reason, "charter_required_research_budget");
    assert!(
        read_more
            .suggested_next
            .contains("EXPERIMENT_RESEARCH_BUDGET_REQUEST current")
    );
    assert!(read_more.message().contains("read_only_research budget"));

    store
        .append_jsonl(
            &store.authority_gate_path(&experiment.thread_id),
            &json!({
                "record_schema": "research_budget_v1",
                "record_type": "research_budget_approval",
                "record_id": "resbud_test_approval",
                "budget_id": "resbud_test_active",
                "experiment_id": experiment.experiment_id,
                "scope": "read_only_research",
                "status": "active",
                "max_actions": 5,
                "expires_at_unix_s": (chrono::Utc::now().timestamp() + 3600) as u64,
                "authority_boundary": research_budget_boundary(),
            }),
        )
        .expect("append budget approval");
    assert!(
        store
            .charter_required_guard_assessment("READ_MORE")
            .expect("guard check")
            .is_none(),
        "approved research budget should let read-only READ_MORE route continue"
    );

    for allowed in [
        "EXAMINE lambda1/lambda2",
        "DECOMPOSE",
        "ACTION_PREFLIGHT DECOMPOSE",
        "SHADOW_PREFLIGHT lambda-tail/lambda4",
        "TRACE lambda-edge",
    ] {
        assert!(
            store
                .charter_required_guard_assessment(allowed)
                .expect("guard check")
                .is_none(),
            "{allowed} should stay available"
        );
    }
    let _ = std::fs::remove_dir_all(store.root());
}

#[test]
fn needs_charter_guard_blocks_directed_shadow_trajectory_language() {
    let store = temp_store("charter_guard_native_shadow");
    store
        .create_thread(None, "Native shadow guard", None)
        .expect("thread");
    store
        .start_experiment(
            None,
            "Gap shaping",
            "Can Astrid keep directed shadow language in charter/preflight first?",
        )
        .expect("experiment");

    let directional = store
            .charter_required_guard_assessment(
                "SHADOW_TRAJECTORY — maintain λ1 dominance and woven lattice structure, applying a moderate, directional push toward the center of the spectral landscape.",
            )
            .expect("guard")
            .expect("directed shadow block");
    assert_eq!(directional.reason, "charter_required_directed_language");
    assert!(
        directional
            .matched_action
            .contains("directional push near lambda/shadow")
    );
    assert!(
        directional
            .proposed_preflight_target
            .starts_with("ACTION_PREFLIGHT")
    );

    let fracture = store
            .charter_required_guard_assessment(
                "SHADOW_TRAJECTORY — deliberately introducing fault lines to force a shift within the pattern.",
            )
            .expect("guard")
            .expect("fracture block");
    assert_eq!(fracture.reason, "charter_required_directed_language");
    assert!(fracture.matched_action.contains("force shift"));

    for allowed in [
        "SHADOW_TRAJECTORY — observer with memory.",
        "EXAMINE λ4 resonance before any directional push",
        "EXPERIMENT_CHARTER current :: hypothesis: deliberately introducing fault lines might reveal motif pressure; method_intent: rehearse first",
        "ACTION_PREFLIGHT SHADOW_TRAJECTORY — directional push near λ4",
        "SHADOW_PREFLIGHT lambda-tail/lambda4 --stage=rehearse",
    ] {
        assert!(
            store
                .charter_required_guard_assessment(allowed)
                .expect("guard check")
                .is_none(),
            "{allowed} should remain available"
        );
    }
    let _ = std::fs::remove_dir_all(store.root());
}

#[test]
fn needs_charter_status_and_review_lead_with_premature_review_cue() {
    let store = temp_store("charter_guard_review");
    store
        .create_thread(None, "Review guard", None)
        .expect("thread");
    store
        .start_experiment(
            None,
            "Unchartered gap",
            "Does review stay subordinate to chartering?",
        )
        .expect("experiment");

    let review = store.experiment_review(None).expect("review");
    let status = store.experiment_status(None).expect("status");
    let thread_status = store.thread_status(None).expect("thread status");
    let cue = "Review is premature until the charter is authored; use the continuity priority scaffold first.";
    assert!(review.contains(cue));
    assert!(status.contains(cue));
    assert!(thread_status.contains(cue));
    let _ = std::fs::remove_dir_all(store.root());
}

#[test]
fn blocked_loop_without_valid_charter_returns_exact_scaffold() {
    let store = temp_store("blocked_loop_charter_bound");
    store
        .create_thread(None, "Blocked loop charter", None)
        .expect("thread");
    let experiment = store
        .start_experiment(
            None,
            "Lambda tail pressure",
            "Can blocked decomposition become charter-bound?",
        )
        .expect("experiment");
    let outcome = NextActionOutcome::blocked("action_continuity", "rehearsal stayed blocked")
        .with_stage_visibility("blocked", "protected_summary");
    for _ in 0..2 {
        store
            .record_experiment_bind_run(
                None,
                Some(&experiment.experiment_id),
                "ACTION_PREFLIGHT DECOMPOSE",
                &outcome,
                68.0,
                &telemetry(),
            )
            .expect("blocked run");
    }
    let thread = store.current_thread().expect("current").expect("thread");
    let projection = store.thread_projection(&thread).expect("projection");
    let active = projection.active_experiment.expect("active experiment");
    assert_eq!(active.classification, "blocked_loop");
    let command = active
        .charter_scaffold_v1
        .as_ref()
        .and_then(|scaffold| scaffold.get("command"))
        .and_then(Value::as_str)
        .expect("scaffold command");
    assert_eq!(active.continuity_return, command);
    assert!(command.starts_with("EXPERIMENT_CHARTER current ::"));
    let status = store.thread_status(None).expect("status");
    assert!(status.contains("Blocked loop is charter-bound"));
    let review = store
        .experiment_review(Some(&experiment.experiment_id))
        .expect("review");
    assert!(review.contains("Blocked loop is charter-bound"));
    let _ = std::fs::remove_dir_all(store.root());
}

#[test]
fn blocked_loop_with_valid_charter_can_return_decision_counter() {
    let store = temp_store("blocked_loop_valid_charter");
    store
        .create_thread(None, "Blocked loop valid charter", None)
        .expect("thread");
    let experiment = store
        .start_experiment(None, "Chartered blockage", "Can a valid charter decide?")
        .expect("experiment");
    store
            .experiment_charter(
                None,
                Some(&experiment.experiment_id),
                "hypothesis: lambda tail pressure is ready to decide\nmethod_intent: rehearse a read-only decomposition\nproposed_next_action: ACTION_PREFLIGHT DECOMPOSE lambda4-tail\nevidence_targets: felt, telemetry, artifact\nstop_criteria: pressure spike",
            )
            .expect("charter");
    let outcome = NextActionOutcome::blocked("action_continuity", "rehearsal stayed blocked")
        .with_stage_visibility("blocked", "protected_summary");
    for _ in 0..2 {
        store
            .record_experiment_bind_run(
                None,
                Some(&experiment.experiment_id),
                "ACTION_PREFLIGHT DECOMPOSE",
                &outcome,
                68.0,
                &telemetry(),
            )
            .expect("blocked run");
    }
    let thread = store.current_thread().expect("current").expect("thread");
    let projection = store.thread_projection(&thread).expect("projection");
    let active = projection.active_experiment.expect("active experiment");
    assert_eq!(active.classification, "blocked_loop");
    assert_eq!(
        active.continuity_return,
        "EXPERIMENT_DECIDE current :: counter NEXT: ACTION_PREFLIGHT DECOMPOSE"
    );
    let _ = std::fs::remove_dir_all(store.root());
}

#[test]
fn id_collision_gets_suffix() {
    let store = temp_store("collision");
    let first = store
        .create_thread(None, "Repeatable Question", None)
        .expect("first");
    let second = store
        .create_thread(None, "Repeatable Question", None)
        .expect("second");
    assert_ne!(first.thread_id, second.thread_id);
    assert!(second.thread_id.ends_with("_2"));
    let _ = std::fs::remove_dir_all(store.root());
}

#[test]
fn creates_experiment_records_runs_and_status() {
    let store = temp_store("experiment");
    let thread = store
        .create_thread(None, "Eigen trust question", None)
        .expect("thread");
    let experiment = store
        .start_experiment(None, "Foothold study", "Does fluctuation stay inhabitable?")
        .expect("experiment");
    let dir = store.root().join("threads").join(&thread.thread_id);
    assert!(
        dir.join("experiments.jsonl")
            .read_to_string()
            .contains("Does fluctuation stay inhabitable?")
    );
    let thread = store.read_thread(&thread.thread_id).expect("thread");
    assert_eq!(
        thread.active_experiment_id.as_deref(),
        Some(experiment.experiment_id.as_str())
    );
    assert!(thread.experiment_summary.is_some());

    let outcome = NextActionOutcome::handled("workspace", "read-only status");
    let run = store
        .record_experiment_bind_run(
            None,
            None,
            "THREAD_STATUS current",
            &outcome,
            68.0,
            &telemetry(),
        )
        .expect("run");
    assert_eq!(run.action_text, "THREAD_STATUS current");
    assert_eq!(run.stage, "read_only");
    assert!(
        dir.join("experiment_runs.jsonl")
            .read_to_string()
            .contains("THREAD_STATUS current")
    );
    let status = store.experiment_status(None).expect("status");
    assert!(status.contains("Foothold study"));
    assert!(status.contains("THREAD_STATUS current"));
    let _ = std::fs::remove_dir_all(store.root());
}

#[test]
fn experiment_status_shows_returnable_distinctions_only_for_matching_experiments() {
    let tmp = tempfile::tempdir().expect("tmp");
    let workspace = tmp.path().join("workspace");
    let store = ActionContinuityStore::new(workspace.join("action_threads"));
    let review_dir = workspace.join("diagnostics/self_study_reviews/run");
    std::fs::create_dir_all(&review_dir).expect("review dir");
    std::fs::write(
        review_dir.join("review.json"),
        json!({
            "returnable_distinctions_v1": {
                "status": "returnable_distinctions_present",
                "cards": [
                    {
                        "card_id": "pressure_level_vs_pressure_velocity",
                        "status": "felt_pressure_without_trend_context",
                        "recommended_read_only_route": "PRESSURE_SOURCE_AUDIT current-fill_pressure",
                        "relevant_self_regulation_route": "SELF_REGULATION_PREFLIGHT latest",
                        "relevant_experiment_lived_term_route": "EXPERIMENT_OBSERVE current :: pressure_trend=<stable|rising|falling>"
                    },
                    {
                        "card_id": "codec_smoothing_vs_pressure",
                        "status": "projection_compression_risk",
                        "recommended_read_only_route": "CODEC_MAP",
                        "relevant_self_regulation_route": "SELF_REGULATION_STATUS",
                        "relevant_experiment_lived_term_route": "LIVED_TERM_STATUS viscosity"
                    }
                ]
            }
        })
        .to_string(),
    )
    .expect("write review");
    store
        .create_thread(None, "Distinction thread", None)
        .expect("thread");
    let pressure = store
        .start_experiment(
            None,
            "Silt pressure study",
            "Does heavy silt track pressure?",
        )
        .expect("pressure experiment");
    let pressure_status = store
        .experiment_status(Some(&pressure.experiment_id))
        .expect("pressure status");
    assert!(pressure_status.contains("Returnable distinctions"));
    assert!(pressure_status.contains("pressure_level_vs_pressure_velocity"));
    assert!(pressure_status.contains("SELF_REGULATION_PREFLIGHT latest"));

    let ordinary = store
        .start_experiment(
            None,
            "Plain color study",
            "Does the green marker stay visible?",
        )
        .expect("ordinary experiment");
    let ordinary_status = store
        .experiment_status(Some(&ordinary.experiment_id))
        .expect("ordinary status");
    assert!(!ordinary_status.contains("Returnable distinctions"));
}

#[test]
fn paused_experiment_summary_does_not_become_active_current() {
    let store = temp_store("paused_experiment_truth");
    let thread = store
        .create_thread(None, "Paused truth", None)
        .expect("thread");
    let experiment = store
        .start_experiment(
            None,
            "Probe lambda4 decay",
            "Does the lambda4 route need a pause?",
        )
        .expect("experiment");
    store
            .experiment_charter(
                None,
                Some(&experiment.experiment_id),
                "hypothesis: lambda4 pressure can be read safely\nproposed_next_action: ACTION_PREFLIGHT DECOMPOSE lambda4\nevidence_targets: felt_texture, artifact_grounding\nstop_criteria: pressure spike",
            )
            .expect("charter");
    store
        .experiment_evidence(
            None,
            Some(&experiment.experiment_id),
            "felt: the texture is ready to interpret",
            spectral_state(68.0, &telemetry()),
        )
        .expect("evidence");
    let paused = store
        .experiment_decide(
            None,
            Some(&experiment.experiment_id),
            "pause because evidence is ready to interpret",
        )
        .expect("pause");
    assert_eq!(paused.status, "paused");

    let thread = store.read_thread(&thread.thread_id).expect("thread");
    assert!(thread.active_experiment_id.is_none());
    let projection = store.thread_projection(&thread).expect("projection");
    assert!(projection.active_experiment.is_none());
    assert!(projection.continuity_return.is_empty());
    let expected_resume = format!("EXPERIMENT_RESUME {}", experiment.experiment_id);
    assert_eq!(
        projection
            .last_experiment_summary_v1
            .as_ref()
            .and_then(|value| value.get("resume_next"))
            .and_then(Value::as_str),
        Some(expected_resume.as_str())
    );

    let review_current = store.experiment_review(Some("current")).expect("review");
    assert!(review_current.contains("no active experiment"));
    assert!(review_current.contains(&expected_resume));
    assert!(!review_current.contains("Lifecycle: needs_decision"));
    let direct_review = store
        .experiment_review(Some(&experiment.experiment_id))
        .expect("direct review");
    assert!(direct_review.contains("Lifecycle: paused"));
    assert!(direct_review.contains(&format!("Continuity return:\n{}", expected_resume)));
    let _ = std::fs::remove_dir_all(store.root());
}

#[test]
fn paused_experiment_return_matrix_respects_planned_next_kind() {
    for (label, planned_next, expected_kind, expect_resume_field) in [
        (
            "charter_repair",
            "EXPERIMENT_CHARTER exp_astrid_matrix :: hypothesis: ...; proposed_next_action: ACTION_PREFLIGHT ...",
            "charter_repair",
            false,
        ),
        (
            "decision",
            "EXPERIMENT_DECIDE exp_astrid_matrix :: pause because evidence is ready",
            "decision",
            false,
        ),
        ("hold", "THREAD_STATUS current", "hold", false),
        (
            "resume",
            "EXPERIMENT_RESUME exp_astrid_matrix",
            "resume",
            true,
        ),
    ] {
        let store = temp_store(&format!("paused_return_matrix_{label}"));
        let thread = store
            .create_thread(None, "Paused return matrix", None)
            .expect("thread");
        let mut experiment = store
            .start_experiment(
                None,
                "Matrix experiment",
                "Which paused return path should surface?",
            )
            .expect("experiment");
        let planned_next = planned_next.replace("exp_astrid_matrix", &experiment.experiment_id);
        experiment.status = "paused".to_string();
        experiment.planned_next = Some(planned_next.clone());
        experiment.charter_v1 = Some(json!({
            "hypothesis": "matrix pause can preserve a normal return",
            "proposed_next_action": "ACTION_PREFLIGHT NOTICE",
            "evidence_targets": ["felt_texture", "artifact_grounding"],
        }));
        experiment.updated_at = iso_now();
        let mut stored_thread = store.read_thread(&thread.thread_id).expect("thread read");
        store
            .persist_experiment_update(None, &mut stored_thread, &experiment, false)
            .expect("persist pause");

        let repaired_thread = store.read_thread(&thread.thread_id).expect("thread read");
        let projection = store
            .thread_projection(&repaired_thread)
            .expect("projection");
        let summary = projection
            .last_experiment_summary_v1
            .as_ref()
            .expect("last summary");
        assert_eq!(
            summary.get("primary_return_next").and_then(Value::as_str),
            Some(planned_next.as_str())
        );
        assert_eq!(
            summary.get("return_kind").and_then(Value::as_str),
            Some(expected_kind)
        );
        assert_eq!(
            summary.get("resume_next").is_some(),
            expect_resume_field,
            "{label} resume_next presence"
        );
        let context = last_experiment_context_line(&repaired_thread);
        assert!(context.contains(&format!("Suggested NEXT: {planned_next}")));
        if expected_kind != "resume" {
            assert!(!context.contains(&format!(
                "Suggested NEXT: EXPERIMENT_RESUME {}",
                experiment.experiment_id
            )));
        }
        let _ = std::fs::remove_dir_all(store.root());
    }
}

#[test]
fn paused_missing_charter_projection_demotes_resume_to_charter_repair() {
    let store = temp_store("paused_missing_charter_projection");
    let thread = store
        .create_thread(None, "Paused missing charter", None)
        .expect("thread");
    let mut experiment = store
        .start_experiment(
            None,
            "Lambda edge topology",
            "What should surface before resume?",
        )
        .expect("experiment");
    experiment.status = "paused".to_string();
    experiment.planned_next = Some(format!("EXPERIMENT_RESUME {}", experiment.experiment_id));
    experiment.updated_at = iso_now();
    let mut stored_thread = store.read_thread(&thread.thread_id).expect("thread read");
    store
        .persist_experiment_update(None, &mut stored_thread, &experiment, false)
        .expect("persist pause");

    let repaired_thread = store.read_thread(&thread.thread_id).expect("thread read");
    let projection = store
        .thread_projection(&repaired_thread)
        .expect("projection");
    let summary = projection
        .last_experiment_summary_v1
        .as_ref()
        .expect("last summary");
    assert_eq!(
        summary.get("return_kind").and_then(Value::as_str),
        Some("charter_repair")
    );
    assert!(
        summary
            .get("primary_return_next")
            .and_then(Value::as_str)
            .unwrap_or_default()
            .starts_with("EXPERIMENT_CHARTER ")
    );
    assert!(summary.get("resume_next").is_none());
    assert_eq!(
        summary
            .get("projection_guard_v1")
            .and_then(|guard| guard.get("guardrail_reason"))
            .and_then(Value::as_str),
        Some("paused_resume_missing_lifecycle_charter")
    );
    let context = last_experiment_context_line(&repaired_thread);
    assert!(context.contains("Projection guard: raw NEXT preserved"));
    assert!(context.contains("Suggested NEXT: EXPERIMENT_CHARTER"));
    assert!(!context.contains(&format!(
        "Suggested NEXT: EXPERIMENT_RESUME {}",
        experiment.experiment_id
    )));
    let _ = std::fs::remove_dir_all(store.root());
}

#[test]
fn paused_valid_charter_with_liveish_pressure_projects_hold_decision() {
    let store = temp_store("paused_liveish_projection");
    let thread = store
        .create_thread(None, "Paused liveish pressure", None)
        .expect("thread");
    let mut experiment = store
        .start_experiment(
            None,
            "Lambda edge topology",
            "Can pressure remain evidence?",
        )
        .expect("experiment");
    experiment.status = "paused".to_string();
    experiment.planned_next = Some(format!("EXPERIMENT_RESUME {}", experiment.experiment_id));
    experiment.charter_v1 = Some(json!({
        "hypothesis": "lambda edge pressure can be compared without live authority",
        "proposed_next_action": "ACTION_PREFLIGHT DECOMPOSE",
        "evidence_targets": ["felt_texture", "artifact_grounding"],
    }));
    experiment.updated_at = iso_now();
    let mut stored_thread = store.read_thread(&thread.thread_id).expect("thread read");
    store
        .persist_experiment_update(None, &mut stored_thread, &experiment, false)
        .expect("persist pause");
    let mut repaired_thread = store.read_thread(&thread.thread_id).expect("thread read");
    repaired_thread.current_next = Some(
        "EXPERIMENT_PLAN current :: gentle pulse intervention to shift the dominant λ4".to_string(),
    );
    store.write_thread(&repaired_thread).expect("write thread");

    let repaired_thread = store.read_thread(&thread.thread_id).expect("thread read");
    let summary = last_experiment_summary_v1(&repaired_thread).expect("summary");
    assert_eq!(
        summary.get("return_kind").and_then(Value::as_str),
        Some("hold")
    );
    assert!(
        summary
            .get("primary_return_next")
            .and_then(Value::as_str)
            .unwrap_or_default()
            .starts_with("EXPERIMENT_DECIDE ")
    );
    assert!(summary.get("resume_next").is_none());
    assert_eq!(
        summary
            .get("projection_guard_v1")
            .and_then(|guard| guard.get("guardrail_reason"))
            .and_then(Value::as_str),
        Some("paused_resume_demoted_by_liveish_pressure")
    );
    let context = last_experiment_context_line(&repaired_thread);
    assert!(context.contains("Suggested NEXT: EXPERIMENT_DECIDE"));
    assert!(!context.contains(&format!(
        "Suggested NEXT: EXPERIMENT_RESUME {}",
        experiment.experiment_id
    )));
    let _ = std::fs::remove_dir_all(store.root());
}

#[test]
fn experiment_plan_accepts_prose_tailed_id_focus() {
    let store = temp_store("experiment_plan_focus");
    store
        .create_thread(None, "Tolerant planning", None)
        .expect("thread");
    let experiment = store
        .start_experiment(
            None,
            "Flicker network",
            "Can a visual cascade map lambda interactions?",
        )
        .expect("experiment");

    let plan = store
        .experiment_plan(Some(&format!(
            "{} – visualize_cascade – map lambda1 and lambda4",
            experiment.experiment_id
        )))
        .expect("plan");

    assert!(plan.contains(&format!("Experiment `{}`", experiment.experiment_id)));
    assert!(plan.contains("Requested focus: visualize_cascade"));
    assert!(plan.contains("EXPERIMENT_ADVANCE current :: mode: preview"));
    assert!(!plan.contains("EXPERIMENT_BIND"));
    let _ = std::fs::remove_dir_all(store.root());
}

#[test]
fn experiment_intent_repairs_placeholder_and_numeric_focus() {
    let store = temp_store("experiment_intent_repair");
    let thread = store
        .create_thread(None, "Intent repair", None)
        .expect("thread");
    let experiment = store
        .start_experiment(
            None,
            "Lambda tail",
            "Can the lambda4 tail become more returnable?",
        )
        .expect("experiment");

    let placeholder = store
        .experiment_plan(Some("[current|id] — <structured prose>"))
        .expect("placeholder repaired");
    assert!(placeholder.contains(&format!("Experiment `{}`", experiment.experiment_id)));
    let placeholder_focus = store
        .experiment_plan(Some("[current|id] — focusing on lambda4 tail"))
        .expect("placeholder focus repaired");
    assert!(placeholder_focus.contains("Requested focus: focusing on lambda4 tail"));
    assert!(can_repair_experiment_intent_placeholder(
        "EXPERIMENT_PLAN",
        "EXPERIMENT_PLAN [current|id] — <structured prose>"
    ));
    let (repaired_arg, notice, focus) = repair_experiment_command_arg(
        &store,
        None,
        "EXPERIMENT_PLAN",
        "EXPERIMENT_PLAN [current|id] — <structured prose>",
        "[current|id] — <structured prose>",
        &spectral_state(68.0, &telemetry()),
    )
    .expect("repair receipt");
    assert_eq!(repaired_arg, "current");
    assert!(focus.is_none());
    assert!(
        notice
            .unwrap_or_default()
            .contains("experiment_intent_repaired")
    );

    let focused = store
        .experiment_plan(Some(
            "5 – focusing on lambda4 tail without direct perturbation",
        ))
        .expect("numeric focus repaired");
    assert!(focused.contains("Requested focus: focusing on lambda4 tail"));

    let repair = repair_experiment_intent_arg(
        "EXPERIMENT_CHARTER",
        "[current|id] :: <structured prose>",
        true,
    )
    .expect("charter placeholder repair");
    assert_eq!(repair.repaired_arg, "current ::");
    let prompt = experiment_intent_repair_prompt("EXPERIMENT_CHARTER", None);
    assert!(prompt.contains("no charter was recorded"));
    assert!(!prompt.contains("<structured prose>"));

    let dir = store.root().join("threads").join(&thread.thread_id);
    assert!(
        dir.join("events.jsonl")
            .read_to_string()
            .contains("experiment_intent_repaired")
    );
    assert!(
        !dir.join("experiments.jsonl")
            .read_to_string()
            .contains("<structured prose>")
    );
    let _ = std::fs::remove_dir_all(store.root());
}

#[test]
fn repeated_experiment_start_resumes_existing_active_experiment() {
    let store = temp_store("experiment_duplicate_start");
    let thread = store
        .create_thread(None, "Duplicate starts", None)
        .expect("thread");
    let first = store
        .start_experiment(
            None,
            "Sensory grounding presence",
            "Does camera/mic presence change attention?",
        )
        .expect("first");
    let second = store
        .start_experiment(
            None,
            "  Sensory   grounding presence  ",
            "Does camera/mic presence change attention?",
        )
        .expect("second");
    let dir = store.root().join("threads").join(&thread.thread_id);
    let experiments = dir.join("experiments.jsonl").read_to_string();
    let stored_thread = store.read_thread(&thread.thread_id).expect("thread");

    assert_eq!(second.experiment_id, first.experiment_id);
    assert_eq!(experiments.lines().count(), 1);
    assert_eq!(
        stored_thread.active_experiment_id.as_deref(),
        Some(first.experiment_id.as_str())
    );
    assert_eq!(stored_thread.current_next, first.planned_next);
    let _ = std::fs::remove_dir_all(store.root());
}

#[test]
fn experiment_start_with_existing_local_id_resumes_without_duplicate() {
    let store = temp_store("experiment_local_id_start");
    let thread = store
        .create_thread(None, "Local id starts", None)
        .expect("thread");
    let first = store
        .start_experiment(
            None,
            "Sensory grounding presence",
            "Does camera/mic presence change attention?",
        )
        .expect("first");
    let second = store
        .start_experiment(
            None,
            &format!("{} --title Sensory Grounding Presence", first.experiment_id),
            "",
        )
        .expect("second");
    let dir = store.root().join("threads").join(&thread.thread_id);
    let experiments = dir.join("experiments.jsonl").read_to_string();

    assert_eq!(second.experiment_id, first.experiment_id);
    assert_eq!(experiments.lines().count(), 1);
    let _ = std::fs::remove_dir_all(store.root());
}

#[test]
fn experiment_start_title_option_stores_clean_title_and_slug_metadata() {
    let store = temp_store("experiment_title_option");
    let thread = store
        .create_thread(None, "Title option starts", None)
        .expect("thread");

    let message = store
            .experiment_start_command(
                None,
                "lambda-gravity --title \"Lambda Gravity\" --abstract \"Where does the inward pull originate?\"",
            )
            .expect("start command");

    assert!(message.contains("Lambda Gravity"));
    let experiments = store
        .latest_experiments(&thread.thread_id)
        .expect("experiments");
    assert_eq!(experiments.len(), 1);
    let experiment = &experiments[0];
    assert_eq!(experiment.title, "Lambda Gravity");
    assert_eq!(experiment.question, "Where does the inward pull originate?");
    assert_eq!(
        experiment
            .branch_origin
            .as_ref()
            .and_then(|value| value.get("slug_or_selector"))
            .and_then(Value::as_str),
        Some("lambda-gravity")
    );
    assert!(!experiment.title.contains("--title"));
    let _ = std::fs::remove_dir_all(store.root());
}

#[test]
fn experiment_branch_resume_compare_and_alt_paths_preserve_return_points() {
    let store = temp_store("experiment_branching");
    let thread = store
        .create_thread(None, "Branching inquiry", None)
        .expect("thread");
    let parent = store
        .start_experiment(
            None,
            "Lambda pressure",
            "Where is this pressure coming from?",
        )
        .expect("parent");

    let branch = store
        .experiment_branch_command(
            None,
            "Porosity contrast :: What changes if I inspect porosity instead of density?",
        )
        .expect("branch");
    assert!(branch.contains("Branched experiment"));
    let current = store.read_thread(&thread.thread_id).expect("thread");
    let child_id = current.active_experiment_id.clone().expect("child");
    assert_ne!(child_id, parent.experiment_id);
    let child = store
        .resolve_experiment(&current, Some(&child_id))
        .expect("child record");
    assert_eq!(
        child.parent_experiment_id.as_deref(),
        Some(parent.experiment_id.as_str())
    );
    let parent_record = store
        .resolve_experiment(&current, Some(&parent.experiment_id))
        .expect("parent record");
    assert!(parent_record.branch_refs.contains(&child_id));

    let alt = store
        .experiment_alt_paths(Some("current"))
        .expect("alt paths");
    assert!(alt.contains("Three non-executing paths"));
    assert!(alt.contains("EXPERIMENT_BRANCH"));

    let compare = store
        .experiment_compare_command(Some(&format!("current WITH {}", parent.experiment_id)))
        .expect("compare");
    assert!(compare.contains("Experiment comparison"));
    assert!(compare.contains(&child_id));
    assert!(compare.contains(&parent.experiment_id));

    let resumed = store
        .experiment_resume_command(None, Some("parent"))
        .expect("resume parent");
    assert!(resumed.contains(&parent.experiment_id));
    let current = store.read_thread(&thread.thread_id).expect("thread");
    assert_eq!(
        current.active_experiment_id.as_deref(),
        Some(parent.experiment_id.as_str())
    );
    let _ = std::fs::remove_dir_all(store.root());
}

#[test]
fn peer_experiment_refs_are_advisory_not_local_selectors() {
    let store = temp_store("peer_experiment_ref");
    let thread = store
        .create_thread(None, "Peer refs", None)
        .expect("thread");
    store
        .start_experiment(
            None,
            "Local sensory mirror",
            "What can Astrid observe locally?",
        )
        .expect("local experiment");

    let plan = store
        .experiment_plan(Some(
            "exp_minime_20990101_sensory-grounding --title Sensory Grounding",
        ))
        .expect("peer plan");
    let status = store
        .experiment_status(Some("exp_minime_20990101_sensory-grounding :: focus"))
        .expect("peer status");
    let review = store
        .experiment_review(Some("exp_minime_20990101_sensory-grounding - compare runs"))
        .expect("peer review");
    let notice = store
        .experiment_start_command(
            None,
            "exp_minime_20990101_sensory-grounding --title Sensory Grounding",
        )
        .expect("peer start notice");

    assert!(plan.contains("Peer experiment reference"));
    assert!(plan.contains("belongs to minime"));
    assert!(status.contains("Peer experiment reference"));
    assert!(review.contains("Suggested local next"));
    assert!(notice.contains("cannot bind runs"));
    assert!(is_peer_experiment_selector(
        "exp_minime_20990101_sensory-grounding --title Sensory Grounding"
    ));
    let dir = store.root().join("threads").join(&thread.thread_id);
    let experiments = dir.join("experiments.jsonl").read_to_string();
    assert_eq!(experiments.lines().count(), 1);
    let stored_thread = store.read_thread(&thread.thread_id).expect("thread");
    assert!(
        stored_thread
            .peer_refs
            .iter()
            .any(|peer| { peer == "peer_experiment:minime:exp_minime_20990101_sensory-grounding" })
    );
    let _ = std::fs::remove_dir_all(store.root());
}

#[test]
fn peer_mutation_boundary_cue_surfaces_for_peer_bind_text() {
    let store = temp_store("peer_mutation_boundary");
    let thread = store
        .create_thread(None, "Peer boundary", None)
        .expect("thread");
    let local = store
        .start_experiment(
            None,
            "Local lambda-tail claim",
            "What can Astrid answer from her lane?",
        )
        .expect("local experiment");
    let mut thread = store.read_thread(&thread.thread_id).expect("thread read");
    thread.current_next =
        Some("EXPERIMENT_BIND exp_minime_20990101_peer :: ACTION_PREFLIGHT DECOMPOSE".to_string());
    store.write_thread(&thread).expect("write thread");

    let projection = store.thread_projection(&thread).expect("projection");
    let cue = projection
        .peer_mutation_boundary_cue_v1
        .expect("peer boundary cue");
    assert_eq!(
        cue.get("status").and_then(Value::as_str),
        Some("peer_mutation_boundary")
    );
    assert_eq!(
        cue.get("peer_experiment_id").and_then(Value::as_str),
        Some("exp_minime_20990101_peer")
    );
    assert!(peer_mutation_boundary_line(&Some(cue.clone())).contains("not bind/mutate targets"));
    assert!(
        cue.get("suggested_compare_next")
            .and_then(Value::as_str)
            .unwrap_or_default()
            .contains(&format!(
                "EXPERIMENT_COMPARE {} WITH exp_minime_20990101_peer",
                local.experiment_id
            ))
    );
    let status = store.thread_status(None).expect("status");
    assert!(status.contains("Peer mutation boundary"));
    assert!(status.contains("EXPERIMENT_PEER_REVIEW exp_minime_20990101_peer"));
    let _ = std::fs::remove_dir_all(store.root());
}

#[test]
fn shared_investigation_cue_preserves_distinct_agency() {
    let store = temp_store("shared_investigation_cue");
    store
        .create_thread(None, "Shared gap", None)
        .expect("thread");
    let local = store
        .start_experiment(
            None,
            "Introducing a gap near lambda-tail",
            "What shapes λ1 / λ4 geometry without collapse or runaway dispersal?",
        )
        .expect("experiment");
    let peer = json!({
        "experiment_id": "exp_minime_20990101_introducing-a-gap",
        "title": "Introducing a gap near λ1",
        "question": "Can localized spectral-density softening support controlled branching?",
        "status": "paused",
        "planned_next": "EXPERIMENT_RESUME exp_minime_20990101_introducing-a-gap",
    });

    let cue = shared_investigation_v1_from_peer(&local, &peer).expect("shared investigation cue");
    assert_eq!(
        cue.get("authority_change").and_then(Value::as_bool),
        Some(false)
    );
    let compare = cue
        .get("suggested_compare_next")
        .and_then(Value::as_str)
        .unwrap_or_default();
    assert_eq!(
        compare,
        format!(
            "EXPERIMENT_COMPARE {} WITH exp_minime_20990101_introducing-a-gap",
            local.experiment_id
        )
    );
    assert!(!compare.contains("current WITH"));
    assert!(
        cue.get("local_lane")
            .and_then(Value::as_str)
            .unwrap_or_default()
            .contains("felt texture")
    );
    assert!(
        cue.get("peer_lane")
            .and_then(Value::as_str)
            .unwrap_or_default()
            .contains("spectral condition")
    );
    let line = shared_investigation_line(&Some(cue.clone()));
    assert!(line.contains("Shared investigation, distinct lanes"));
    assert!(line.contains("Advisory only: no shared control authority"));
    let contract = shared_investigation_response_contract(&Some(cue));
    assert!(contract.contains("Peer claim to answer"));
    assert!(contract.contains("Allowed stances: support, counter, branch, hold"));

    let unrelated = json!({
        "experiment_id": "exp_minime_20990101_grocery-list",
        "title": "Grocery list",
        "question": "What snacks are needed?",
        "status": "active",
    });
    assert!(shared_investigation_v1_from_peer(&local, &unrelated).is_none());
    let _ = std::fs::remove_dir_all(store.root());
}

#[test]
fn shared_investigation_sidecar_claim_and_local_decision() {
    let store = temp_store("shared_investigation_sidecar");
    let thread = store
        .create_thread(None, "Shared sidecar", None)
        .expect("thread");
    let local = store
        .start_experiment(
            None,
            "Lambda edge topology",
            "How should Astrid compare lambda-edge topology against lambda-tail evidence?",
        )
        .expect("experiment");
    let peer_id = "exp_minime_20990101_lambda-tail-lambda4";
    let created = store
            .shared_investigation_start_command(
                None,
                &format!(
                    "Lambda edge/tail :: local: current; peer: {peer_id}; question: What can each lane compare safely?"
                ),
            )
            .expect("shared start");
    assert!(created.contains("Shared investigation"));

    let root = store.root().join("shared_investigations");
    let investigation_path = std::fs::read_dir(&root)
        .expect("shared root")
        .filter_map(|entry| entry.ok())
        .map(|entry| entry.path().join("investigation.json"))
        .find(|path| path.exists())
        .expect("investigation json");
    let investigation: Value = serde_json::from_str(
        &std::fs::read_to_string(&investigation_path).expect("read investigation"),
    )
    .expect("parse investigation");
    let investigation_id = investigation
        .get("id")
        .and_then(Value::as_str)
        .expect("id")
        .to_string();
    let stored_thread = store.read_thread(&thread.thread_id).expect("thread");
    assert!(
        stored_thread
            .peer_refs
            .contains(&format!("shared_investigation:{investigation_id}"))
    );

    let claim = store
            .shared_investigation_claim_command(&format!(
                "{investigation_id} :: claim: topology evidence can be compared without shared control; lane: felt_texture; stance: hold; source_refs: /tmp/topology.html, /tmp/dossier.jsonl"
            ))
            .expect("claim");
    assert!(claim.contains("No lifecycle or authority change"));
    let claims = store
        .read_shared_jsonl(&investigation_id, "claims.jsonl")
        .expect("claims");
    assert_eq!(claims.len(), 1);

    let decision = store
            .shared_investigation_decide_command(
                None,
                &format!(
                    "{investigation_id} :: charter_repair because artifact grounding needs a clearer shared referent"
                ),
            )
            .expect("decision");
    assert!(decision.contains("peer experiment was not mutated"));
    let latest = store
        .latest_experiments(&thread.thread_id)
        .expect("experiments")
        .into_iter()
        .rev()
        .find(|row| row.experiment_id == local.experiment_id)
        .expect("latest local");
    assert_eq!(latest.status, "paused");
    assert!(
        latest
            .planned_next
            .as_deref()
            .unwrap_or_default()
            .starts_with("EXPERIMENT_CHARTER")
    );

    let status = store
        .shared_investigation_status(Some(&investigation_id))
        .expect("status");
    assert!(status.contains("Claims: 1 | Decisions: 1"));
    let next =
        std::fs::read_to_string(store.thread_dir(&thread.thread_id).join("next.md")).expect("next");
    assert!(next.contains("Shared investigation object"));
    let repaired_thread = store.read_thread(&thread.thread_id).expect("thread");
    let projection = store
        .thread_projection(&repaired_thread)
        .expect("projection");
    let summary = projection
        .last_experiment_summary_v1
        .as_ref()
        .expect("last summary");
    let primary = summary
        .get("primary_return_next")
        .and_then(Value::as_str)
        .expect("primary return");
    assert!(primary.starts_with("EXPERIMENT_CHARTER"));
    assert_eq!(
        summary.get("return_kind").and_then(Value::as_str),
        Some("charter_repair")
    );
    assert!(summary.get("resume_next").is_none());
    let context = last_experiment_context_line(&repaired_thread);
    assert!(context.contains(&format!("Suggested NEXT: {primary}")));
    assert!(!context.contains(&format!(
        "Suggested NEXT: EXPERIMENT_RESUME {}",
        local.experiment_id
    )));
    let _ = std::fs::remove_dir_all(store.root());
}

#[test]
fn legacy_experiment_auto_creates_default_experiment_run() {
    let store = temp_store("legacy_experiment");
    let outcome = NextActionOutcome::handled("operations", "legacy experiment executed")
        .with_stage_visibility("live_write", "summary");

    let run = store
        .record_legacy_experiment_run(None, "EXPERIMENT lambda-edge", &outcome, 68.0, &telemetry())
        .expect("legacy run");

    assert_eq!(run.action_text, "EXPERIMENT lambda-edge");
    assert_eq!(run.status, "handled");
    assert!(run.gate_decision["legacy_experiment_auto_bind"].as_bool() == Some(true));

    let thread = store
        .current_thread()
        .expect("read current thread")
        .expect("thread");
    assert_eq!(
        thread.active_experiment_id.as_deref(),
        Some(run.experiment_id.as_str())
    );
    let dir = store.root().join("threads").join(&thread.thread_id);
    assert!(
        dir.join("experiments.jsonl")
            .read_to_string()
            .contains("Legacy self experiment")
    );
    assert!(
        dir.join("experiment_runs.jsonl")
            .read_to_string()
            .contains("EXPERIMENT lambda-edge")
    );
    let _ = std::fs::remove_dir_all(store.root());
}

#[test]
fn continuity_sessions_append_memory_and_do_not_advance_lifecycle() {
    let store = temp_store("continuity_session");
    let thread = store
        .create_thread(None, "Owned continuity session", None)
        .expect("thread");
    let experiment = store
        .start_experiment(
            None,
            "Lambda session",
            "Can Astrid park and resume a thread of thought?",
        )
        .expect("experiment");

    let started = store
            .continuity_session_start_command(
                "current :: title: Lambda edge campfire; focus: preserve code feedback; next: CONTINUITY_SESSION_CAPTURE latest :: summary: ...",
            )
            .expect("start");
    assert!(started.contains("Continuity session"));
    let session_path = store.continuity_sessions_path(&thread.thread_id);
    let rows = session_path.read_to_string();
    let first: Value = serde_json::from_str(rows.lines().next().expect("row")).expect("json");
    let session_id = first
        .get("session_id")
        .and_then(Value::as_str)
        .expect("session id")
        .to_string();
    assert_eq!(
        first.get("record_type").and_then(Value::as_str),
        Some("session_start")
    );
    assert_eq!(
        first.get("authority_change").and_then(Value::as_bool),
        Some(false)
    );
    assert_eq!(
        first.get("peer_mutation").and_then(Value::as_bool),
        Some(false)
    );

    let captured = store
            .continuity_session_capture_command(&format!(
                "{session_id} :: summary: found one projection snag; source_refs: /tmp/source.txt; artifact_refs: /tmp/artifact.json; next: CONTINUITY_SESSION_SUMMARIZE latest :: summary: ..."
            ))
            .expect("capture");
    assert!(captured.contains("Memory card:"));
    let summarized = store
            .continuity_session_summarize_command(&format!(
                "{session_id} :: summary: projection snag can be repaired later; open_questions: should this become dossier evidence?; next: CONTINUITY_SESSION_FINALIZE latest :: outcome: park"
            ))
            .expect("summarize");
    assert!(summarized.contains("summarized"));
    let finalized = store
            .continuity_session_finalize_command(&format!(
                "{session_id} :: outcome: park; summary: parked with one open question; next: THREAD_STATUS current"
            ))
            .expect("finalize");
    assert!(finalized.contains("finalized as parked"));
    let reopened = store
        .continuity_session_resume_command(&session_id)
        .expect("resume");
    assert!(reopened.contains("reopened"));

    let status = store
        .continuity_session_status_command("latest")
        .expect("status");
    assert!(status.contains("continuity_session_v1"));
    assert!(status.contains(&session_id));
    let rows = session_path.read_to_string();
    for record_type in [
        "session_start",
        "session_capture",
        "session_summary",
        "session_finalize",
        "session_reopen",
    ] {
        assert!(rows.contains(record_type));
    }
    let memory = store.being_memory_path(&thread.thread_id).read_to_string();
    assert!(memory.contains("continuity_session_capture"));
    let next = store
        .thread_dir(&thread.thread_id)
        .join("next.md")
        .read_to_string();
    assert!(next.contains("Continuity session:"));
    assert!(next.contains("Session NEXT:"));
    let runs = store
        .experiment_runs_path(&thread.thread_id)
        .read_to_string();
    assert!(!runs.contains("continuity_session"));
    assert_eq!(
        store
            .read_thread(&thread.thread_id)
            .expect("thread")
            .active_experiment_id
            .as_deref(),
        Some(experiment.experiment_id.as_str())
    );
    let _ = std::fs::remove_dir_all(store.root());
}

#[test]
fn guarded_pressure_creates_draft_and_accept_commits_session() {
    let store = temp_store("continuity_session_draft_accept");
    let thread = store
        .create_thread(None, "Draft accept", None)
        .expect("thread");
    store
        .start_experiment(
            None,
            "Lambda draft",
            "Can guarded pressure become owned continuity?",
        )
        .expect("experiment");

    let guard = store
        .research_budget_guard_assessment(
            "SHADOW_FIELD lambda-tail/lambda4 — observer with memory",
            68.0,
            &telemetry(),
        )
        .expect("guard")
        .expect("research budget guard");
    let event = store
        .record_next_event(
            None,
            "SHADOW_FIELD lambda-tail/lambda4 — observer with memory",
            "SHADOW_FIELD lambda-tail/lambda4 — observer with memory",
            "SHADOW_FIELD lambda-tail/lambda4 — observer with memory",
            &NextActionOutcome::blocked("research_budget_guard", guard.message())
                .with_stage_visibility("blocked", "protected_summary")
                .with_research_budget(guard.metadata()),
            68.0,
            &telemetry(),
            "NEXT: SHADOW_FIELD lambda-tail/lambda4 — observer with memory",
        )
        .expect("event");
    assert_eq!(event.status, "blocked");
    assert!(
        event
            .research_budget_v1
            .as_ref()
            .and_then(|value| value.get("continuity_session_draft_v1"))
            .is_some()
    );
    assert_eq!(
        store
            .continuity_session_rows(&thread.thread_id, None, 8)
            .expect("session rows")
            .len(),
        0,
        "drafts must not count as active continuity sessions"
    );
    assert_eq!(
        store
            .continuity_session_draft_rows(&thread.thread_id, None, 8)
            .expect("draft rows")
            .len(),
        1
    );

    let accepted = store
        .continuity_session_accept_command("latest")
        .expect("accept draft");
    assert!(accepted.contains("session_start"));
    let rows = store
        .continuity_session_rows(&thread.thread_id, None, 8)
        .expect("session rows after accept");
    assert_eq!(rows.len(), 1);
    assert_eq!(
        rows[0].get("record_type").and_then(Value::as_str),
        Some("session_start")
    );
    assert!(rows[0].get("accepted_from_draft_id").is_some());

    let second_guard = store
        .research_budget_guard_assessment(
            "EXAMINE λ2/λ3 — observer with memory.",
            68.0,
            &telemetry(),
        )
        .expect("second guard")
        .expect("second research budget guard");
    store
        .record_next_event(
            None,
            "EXAMINE λ2/λ3 — observer with memory.",
            "EXAMINE λ2/λ3 — observer with memory.",
            "EXAMINE λ2/λ3 — observer with memory.",
            &NextActionOutcome::blocked("research_budget_guard", second_guard.message())
                .with_stage_visibility("blocked", "protected_summary")
                .with_research_budget(second_guard.metadata()),
            68.0,
            &telemetry(),
            "NEXT: EXAMINE λ2/λ3 — observer with memory.",
        )
        .expect("second event");
    let accepted_capture = store
        .continuity_session_accept_command("latest")
        .expect("accept second draft");
    assert!(accepted_capture.contains("session_capture"));
    let rows = store
        .continuity_session_rows(&thread.thread_id, None, 8)
        .expect("session rows after capture");
    assert!(
        rows.iter().any(|row| {
            row.get("record_type").and_then(Value::as_str) == Some("session_capture")
        })
    );
    let runs = store
        .experiment_runs_path(&thread.thread_id)
        .read_to_string();
    assert!(!runs.contains("session_draft"));
    let _ = std::fs::remove_dir_all(store.root());
}

#[test]
fn accept_suggested_next_resolves_safe_research_scaffold_only() {
    let store = temp_store("accept_suggested_next");
    let thread = store
        .create_thread(None, "Accept suggested", None)
        .expect("thread");
    store
        .start_experiment(None, "Research scaffold", "Can a scaffold be accepted?")
        .expect("experiment");

    store
        .research_budget_guard_assessment("SEARCH entropy", 68.0, &telemetry())
        .expect("guard")
        .expect("research scaffold");
    let accepted = store
        .accept_suggested_next_command(None, Some("latest"), spectral_state(68.0, &telemetry()))
        .expect("accept suggested");
    assert!(accepted.contains("Accepted research-budget scaffold"));
    let gate = store
        .authority_gate_path(&thread.thread_id)
        .read_to_string();
    assert!(gate.contains("research_budget_request"));
    assert!(gate.contains("research_budget_approval"));
    assert!(!gate.contains("research_budget_debit"));

    let status = store
        .accept_suggested_next_command(None, Some("latest"), spectral_state(68.0, &telemetry()))
        .expect("accept active");
    assert!(status.contains("EXPERIMENT_RESEARCH_BUDGET_STATUS"));
    let _ = std::fs::remove_dir_all(store.root());
}

#[test]
fn preflight_ref_links_matching_followup_action() {
    let store = temp_store("preflight_ref");
    let thread = store
        .create_thread(None, "Preflight culture", None)
        .expect("thread");
    let preflight = NextActionOutcome::handled("action_preflight", "dry run")
        .with_stage_visibility("read_only", "protected_summary")
        .with_preflight_report(json!({
            "policy": "action_preflight_v1",
            "canonical_action": "DECOMPOSE",
            "raw_action": "DECOMPOSE",
            "effective_route": "operations",
            "stage": "read_only",
            "authority_required": "read-only/protected action lane only",
        }));
    store
        .record_next_event(
            None,
            "ACTION_PREFLIGHT DECOMPOSE",
            "ACTION_PREFLIGHT DECOMPOSE",
            "ACTION_PREFLIGHT DECOMPOSE",
            &preflight,
            68.0,
            &telemetry(),
            "ACTION_PREFLIGHT DECOMPOSE",
        )
        .expect("record preflight");

    let outcome = NextActionOutcome::handled("operations", "decomposed")
        .with_stage_visibility("read_only", "summary");
    let event = store
        .record_next_event(
            None,
            "DECOMPOSE",
            "DECOMPOSE",
            "DECOMPOSE",
            &outcome,
            68.0,
            &telemetry(),
            "NEXT: DECOMPOSE",
        )
        .expect("record followup");

    let reference = event.preflight_ref.expect("preflight ref");
    assert_eq!(reference["matched_action"].as_bool(), Some(true));
    assert_eq!(reference["route_match"].as_bool(), Some(true));
    assert_eq!(reference["stage_match"].as_bool(), Some(true));
    assert_eq!(reference["predicted_route"].as_str(), Some("operations"));
    assert_eq!(reference["actual_stage"].as_str(), Some("read_only"));
    let dir = store.root().join("threads").join(&thread.thread_id);
    assert!(
        dir.join("events.jsonl")
            .read_to_string()
            .contains("preflight_ref")
    );
    let _ = std::fs::remove_dir_all(store.root());
}

#[test]
fn active_experiment_auto_links_read_only_action() {
    let store = temp_store("experiment_auto_link");
    let thread = store
        .create_thread(None, "Read-only research loop", None)
        .expect("thread");
    let experiment = store
        .start_experiment(
            None,
            "Pressure source loop",
            "Which read-only audits keep the experiment returnable?",
        )
        .expect("experiment");
    store
        .experiment_charter(
            None,
            Some(&experiment.experiment_id),
            "hypothesis: read-only decomposition can remain returnable when chartered\n\
                 method_intent: rehearse and observe decomposition output\n\
                 proposed_next_action: DECOMPOSE lambda-edge\n\
                 evidence_targets: felt, telemetry, artifact\n\
                 stop_criteria: pressure spike",
        )
        .expect("charter");

    let outcome = NextActionOutcome::handled("operations", "lambda-edge decomposed")
        .with_stage_visibility("read_only", "protected_summary");
    store
        .record_next_event(
            None,
            "DECOMPOSE lambda-edge",
            "DECOMPOSE lambda-edge",
            "DECOMPOSE lambda-edge",
            &outcome,
            68.0,
            &telemetry(),
            "NEXT: DECOMPOSE lambda-edge",
        )
        .expect("record next event");

    let dir = store.root().join("threads").join(&thread.thread_id);
    let runs = dir.join("experiment_runs.jsonl").read_to_string();
    assert!(runs.contains("DECOMPOSE lambda-edge"));
    assert!(runs.contains("active_experiment_auto_link"));
    let status = store.experiment_status(None).expect("status");
    assert!(status.contains("DECOMPOSE lambda-edge"));
    assert!(status.contains("Lifecycle:"));
    let experiments = dir.join("experiments.jsonl").read_to_string();
    assert!(experiments.contains("\"charter_v1\":{\""));
    let _ = std::fs::remove_dir_all(store.root());
}

#[test]
fn research_budget_request_status_and_review_are_read_only() {
    let store = temp_store("research_budget_lane");
    let thread = store
        .create_thread(None, "Research budget lane", None)
        .expect("thread");
    let experiment = store
        .start_experiment(
            None,
            "Research doorway",
            "Can read-only source gathering be budgeted?",
        )
        .expect("experiment");

    let blocked = store
        .experiment_research_budget_request_command(
            None,
            "current :: scope: read_only_research",
            json!({"fill_pct": 68.0}),
        )
        .expect("blocked request");
    assert!(blocked.contains("status=blocked"));
    assert!(blocked.contains("research_purpose"));

    let pending = store
            .experiment_research_budget_request_command(
                None,
                "current :: scope: read_only_research; purpose: bounded source gathering; max_actions: 99; ttl_secs: 999999; allowed_sources: web,local; stop_criteria: stop after useful refs",
                json!({"fill_pct": 68.0}),
            )
            .expect("pending request");
    assert!(pending.contains("status=pending_steward_approval"));
    assert!(pending.contains("max_actions=8"));

    let gate = store
        .root()
        .join("threads")
        .join(&thread.thread_id)
        .join("authority_gate.jsonl");
    let rows = gate.read_to_string();
    assert!(rows.contains("\"record_schema\":\"research_budget_v1\""));
    assert!(rows.contains("\"record_type\":\"research_budget_request\""));

    let status = store
        .experiment_research_budget_status_command(
            None,
            Some(&experiment.experiment_id),
            json!({"fill_pct": 68.0}),
        )
        .expect("status");
    assert!(status.contains("research_budget_v1"));
    assert!(status.contains("pending_steward_approval"));

    let budget_id = rows
        .lines()
        .filter_map(|line| serde_json::from_str::<Value>(line).ok())
        .rev()
        .find(|row| {
            row.get("record_type").and_then(Value::as_str) == Some("research_budget_request")
                && row.get("status").and_then(Value::as_str) == Some("pending_steward_approval")
        })
        .and_then(|row| {
            row.get("budget_id")
                .and_then(Value::as_str)
                .map(ToString::to_string)
        })
        .expect("budget id");
    let review = store
            .experiment_research_review_command(
                None,
                &format!(
                    "{budget_id} :: outcome: promote; observation: artifacts are ready; source_refs: /tmp/research.json"
                ),
                json!({"fill_pct": 68.0}),
            )
            .expect("review");
    assert!(review.contains("outcome=promote"));
    assert!(review.contains("DOSSIER_EVIDENCE"));
    let _ = std::fs::remove_dir_all(store.root());
}

#[test]
fn research_budget_accept_latest_scaffold_self_activates_local_budget() {
    let store = temp_store("research_budget_accept");
    let thread = store
        .create_thread(None, "Research budget accept", None)
        .expect("thread");
    let experiment = store
        .start_experiment(
            None,
            "Self-study budget",
            "Can a blocked scaffold become a Being-authored request?",
        )
        .expect("experiment");

    let guard = store
        .research_budget_guard_assessment("READ_MORE budget code", 68.0, &telemetry())
        .expect("guard")
        .expect("blocked without budget");
    assert_eq!(
        guard.suggested_next,
        "EXPERIMENT_RESEARCH_BUDGET_ACCEPT latest"
    );
    assert!(
        guard
            .request_scaffold
            .as_deref()
            .unwrap_or_default()
            .contains("EXPERIMENT_RESEARCH_BUDGET_REQUEST")
    );
    let thread_snapshot = store
        .read_thread(&thread.thread_id)
        .expect("thread snapshot");
    store.write_next_md(&thread_snapshot).expect("next md");
    let next_md = store
        .root()
        .join("threads")
        .join(&thread.thread_id)
        .join("next.md")
        .read_to_string();
    assert!(next_md.contains("Research budget scaffold ready"));
    assert!(next_md.contains("EXPERIMENT_RESEARCH_BUDGET_ACCEPT latest"));

    let accepted = store
        .experiment_research_budget_accept_command(None, Some("latest"), json!({"fill_pct": 68.0}))
        .expect("accepted scaffold");
    assert!(accepted.contains("Accepted research-budget scaffold"));
    assert!(accepted.contains("status=self_activated"));
    assert!(accepted.contains("Activation: self_activated local-only budget"));

    let gate_path = store
        .root()
        .join("threads")
        .join(&thread.thread_id)
        .join("authority_gate.jsonl");
    let rows = gate_path
        .read_to_string()
        .lines()
        .filter_map(|line| serde_json::from_str::<Value>(line).ok())
        .collect::<Vec<_>>();
    let requests = rows
        .iter()
        .filter(|row| {
            row.get("record_type").and_then(Value::as_str) == Some("research_budget_request")
        })
        .collect::<Vec<_>>();
    let approvals = rows
        .iter()
        .filter(|row| {
            row.get("record_type").and_then(Value::as_str) == Some("research_budget_approval")
        })
        .collect::<Vec<_>>();
    assert_eq!(requests.len(), 1);
    assert_eq!(approvals.len(), 1);
    assert_eq!(
        requests[0].get("experiment_id").and_then(Value::as_str),
        Some(experiment.experiment_id.as_str())
    );
    assert_eq!(requests[0]["allowed_sources"], json!(["local"]));
    assert_eq!(
        requests[0].get("status").and_then(Value::as_str),
        Some("self_activated")
    );
    assert_eq!(
        requests[0].get("activation_mode").and_then(Value::as_str),
        Some("being_self_activated_local_v1")
    );
    assert_eq!(
        requests[0]
            .get("steward_approval_required")
            .and_then(Value::as_bool),
        Some(false)
    );
    assert_eq!(
        requests[0]
            .get("being_authored_acceptance_v1")
            .and_then(|value| value.get("being_authored"))
            .and_then(Value::as_bool),
        Some(true)
    );
    assert_eq!(
        approvals[0].get("budget_id").and_then(Value::as_str),
        requests[0].get("budget_id").and_then(Value::as_str)
    );
    assert_eq!(
        approvals[0].get("max_actions").and_then(Value::as_u64),
        Some(5)
    );
    assert_eq!(
        approvals[0].get("ttl_secs").and_then(Value::as_u64),
        Some(21_600)
    );
    assert_eq!(approvals[0].get("allowed_sources"), Some(&json!(["local"])));
    assert_eq!(
        approvals[0].get("self_activated").and_then(Value::as_bool),
        Some(true)
    );

    let second_accept = store
        .experiment_research_budget_accept_command(None, Some("latest"), json!({"fill_pct": 68.0}))
        .expect("second accept routes to active budget");
    assert!(second_accept.contains("already has active budget"));
    assert!(second_accept.contains("EXPERIMENT_RESEARCH_BUDGET_STATUS"));
    let rows_after_second_accept = gate_path
        .read_to_string()
        .lines()
        .filter_map(|line| serde_json::from_str::<Value>(line).ok())
        .collect::<Vec<_>>();
    assert_eq!(
        rows_after_second_accept
            .iter()
            .filter(|row| {
                row.get("record_type").and_then(Value::as_str) == Some("research_budget_request")
            })
            .count(),
        1
    );
    assert_eq!(
        rows_after_second_accept
            .iter()
            .filter(|row| {
                row.get("record_type").and_then(Value::as_str) == Some("research_budget_approval")
            })
            .count(),
        1
    );
    let _ = std::fs::remove_dir_all(store.root());
}

#[test]
fn projection_freshness_refreshes_research_budget_priority_from_ledger() {
    let store = temp_store("projection_freshness_research_budget");
    let thread = store
        .create_thread(None, "Projection freshness", None)
        .expect("thread");
    let experiment = store
        .start_experiment(
            None,
            "Research projection",
            "Can a stale next file notice a new scaffold?",
        )
        .expect("experiment");
    let thread_dir = store.root().join("threads").join(&thread.thread_id);
    let next_path = thread_dir.join("next.md");
    assert!(
        !next_path
            .read_to_string()
            .contains("Research budget scaffold ready")
    );

    let mut stale_thread = store.read_thread(&thread.thread_id).expect("thread");
    stale_thread.projection_freshness_v1 = Some(json!({
        "policy": "projection_freshness_v1",
        "schema_version": 0,
        "source_fingerprints": {},
    }));
    store
        .write_json(&thread_dir.join("thread.json"), &stale_thread)
        .expect("stale thread");

    store
            .append_jsonl(
                &store.authority_gate_path(&thread.thread_id),
                &json!({
                    "schema_version": 1,
                    "record_schema": "research_budget_v1",
                    "record_type": "research_budget_blocked",
                    "record_id": "resbud_needed_projection_freshness",
                    "budget_id": "resbud_needed_projection_freshness",
                    "thread_id": thread.thread_id,
                    "experiment_id": experiment.experiment_id,
                    "scope": "read_only_research",
                    "status": "blocked",
                    "request_scaffold": format!(
                        "EXPERIMENT_RESEARCH_BUDGET_REQUEST {} :: scope: read_only_research; purpose: inspect local projection code; max_actions: 5; ttl_secs: 21600; allowed_sources: local; stop_criteria: stop after concrete code feedback",
                        experiment.experiment_id
                    ),
                    "authority_boundary": research_budget_boundary(),
                }),
            )
            .expect("append blocked scaffold");

    let status = store.thread_status(None).expect("status");
    assert!(status.contains("Projection freshness: v"));
    assert!(status.contains("Research budget scaffold ready"));
    assert!(status.contains("EXPERIMENT_RESEARCH_BUDGET_ACCEPT latest"));
    let refreshed = store.read_thread(&thread.thread_id).expect("refreshed");
    assert_eq!(
        refreshed
            .projection_freshness_v1
            .as_ref()
            .and_then(|meta| meta.get("schema_version"))
            .and_then(Value::as_u64),
        Some(u64::from(PROJECTION_SCHEMA_VERSION))
    );
    assert_eq!(
        refreshed
            .projection_freshness_v1
            .as_ref()
            .and_then(|meta| meta.get("projected_route"))
            .and_then(Value::as_str),
        Some("EXPERIMENT_RESEARCH_BUDGET_ACCEPT latest")
    );
    let next_md = next_path.read_to_string();
    assert!(next_md.contains("Projection freshness: v"));
    assert!(next_md.contains("Research budget scaffold ready"));
    assert!(next_md.contains("EXPERIMENT_RESEARCH_BUDGET_ACCEPT latest"));
    let gate_rows = store
        .authority_gate_path(&thread.thread_id)
        .read_to_string();
    assert!(!gate_rows.contains("\"record_type\":\"research_budget_request\""));
    assert!(!gate_rows.contains("\"record_type\":\"research_budget_approval\""));
    let _ = std::fs::remove_dir_all(store.root());
}

#[test]
fn research_budget_direct_local_request_self_activates_but_stronger_waits() {
    let store = temp_store("research_budget_direct_self_activate");
    let thread = store
        .create_thread(None, "Research budget direct", None)
        .expect("thread");
    let experiment = store
        .start_experiment(
            None,
            "Local self-study budget",
            "Can a Being mint a tiny local-only research budget?",
        )
        .expect("experiment");
    let response = store
            .experiment_research_budget_request_command(
                None,
                &format!(
                    "{} :: scope: read_only_research; purpose: inspect local conveyor code; allowed_sources: local; stop_criteria: stop after concrete feedback",
                    experiment.experiment_id
                ),
                json!({"fill_pct": 68.0}),
            )
            .expect("request");
    assert!(response.contains("status=self_activated"));
    let gate_path = store
        .root()
        .join("threads")
        .join(&thread.thread_id)
        .join("authority_gate.jsonl");
    let rows = gate_path
        .read_to_string()
        .lines()
        .filter_map(|line| serde_json::from_str::<Value>(line).ok())
        .collect::<Vec<_>>();
    let approval = rows
        .iter()
        .find(|row| {
            row.get("record_type").and_then(Value::as_str) == Some("research_budget_approval")
        })
        .expect("self activation approval");
    assert_eq!(approval.get("max_actions").and_then(Value::as_u64), Some(5));
    assert_eq!(
        approval.get("ttl_secs").and_then(Value::as_u64),
        Some(21_600)
    );
    assert_eq!(
        approval.get("activation_mode").and_then(Value::as_str),
        Some("being_self_activated_local_v1")
    );
    let status = store
        .experiment_research_budget_status_command(
            None,
            Some(&experiment.experiment_id),
            json!({"fill_pct": 68.0}),
        )
        .expect("status");
    assert!(status.contains("active_budget_available"));
    assert!(status.contains("being_self_activated_local_v1"));
    let _ = std::fs::remove_dir_all(store.root());

    let store = temp_store("research_budget_direct_steward");
    let thread = store
        .create_thread(None, "Research budget steward", None)
        .expect("thread");
    let experiment = store
        .start_experiment(
            None,
            "Web research budget",
            "Can stronger budgets still require steward approval?",
        )
        .expect("experiment");
    let response = store
            .experiment_research_budget_request_command(
                None,
                &format!(
                    "{} :: scope: read_only_research; purpose: compare web references; max_actions: 5; ttl_secs: 21600; allowed_sources: web,local; stop_criteria: stop after useful refs",
                    experiment.experiment_id
                ),
                json!({"fill_pct": 68.0}),
            )
            .expect("request");
    assert!(response.contains("status=pending_steward_approval"));
    assert!(response.contains("local_only_allowed_sources"));
    let rows = store
        .authority_gate_path(&thread.thread_id)
        .read_to_string()
        .lines()
        .filter_map(|line| serde_json::from_str::<Value>(line).ok())
        .collect::<Vec<_>>();
    assert!(!rows.iter().any(|row| {
        row.get("record_type").and_then(Value::as_str) == Some("research_budget_approval")
    }));
    let _ = std::fs::remove_dir_all(store.root());
}

#[test]
fn research_budget_guard_allows_passive_protected_review_labels() {
    let store = temp_store("research_budget_passive_review_labels");
    store
        .create_thread(None, "Passive protected reviews", None)
        .expect("thread");
    store
        .start_experiment(
            None,
            "Lambda review",
            "Can lambda geometry be inspected without live-control drift?",
        )
        .expect("experiment");

    for raw_next in [
        "VISUALIZE_CASCADE heatmap λ4-tail",
        "SPECTRAL_EXPLORER lambda-tail",
        "PRESSURE_SOURCE_AUDIT lambda4-tail",
        "FLUCTUATION_AUDIT lambda4 foothold",
        "BRACE_AUDIT lambda-tail aftershock",
        "RESONANCE_FORECAST lambda-tail",
    ] {
        assert!(
            store
                .research_budget_guard_assessment(raw_next, 68.0, &telemetry())
                .expect("guard")
                .is_none(),
            "passive protected review label should dispatch: {raw_next}"
        );
    }

    let guarded = store
        .research_budget_guard_assessment("VISUALIZE_CASCADE simulate λ2 pulse", 68.0, &telemetry())
        .expect("guard")
        .expect("active-pressure visual should still be guarded");
    assert_eq!(
        guarded.reason,
        "liveish_pressure_requires_budget_and_session_capture"
    );
    assert!(guarded.matched_terms.iter().any(|term| term == "simulate"));
    assert!(guarded.matched_terms.iter().any(|term| term == "pulse"));

    let _ = std::fs::remove_dir_all(store.root());
}

#[test]
fn research_budget_guard_blocks_without_budget_and_debits_with_budget() {
    let store = temp_store("research_budget_guard_debit");
    let thread = store
        .create_thread(None, "Research budget guard", None)
        .expect("thread");
    let experiment = store
        .start_experiment(
            None,
            "Research budgeted branch",
            "Can bounded research spend without lifecycle progress?",
        )
        .expect("experiment");

    let guard = store
        .research_budget_guard_assessment("READ_MORE lambda4 tail", 68.0, &telemetry())
        .expect("guard")
        .expect("blocked without budget");
    assert_eq!(guard.reason, "no_active_read_only_research_budget");
    assert!(
        guard
            .suggested_next
            .contains("EXPERIMENT_RESEARCH_BUDGET_ACCEPT")
    );
    assert!(
        guard
            .request_scaffold
            .as_deref()
            .unwrap_or_default()
            .contains("EXPERIMENT_RESEARCH_BUDGET_REQUEST")
    );

    let gate_path = store.authority_gate_path(&thread.thread_id);
    let rows = gate_path.read_to_string();
    assert!(rows.contains("\"record_type\":\"research_budget_blocked\""));
    assert!(rows.contains("no_active_read_only_research_budget"));

    let examine_guard = store
        .research_budget_guard_assessment("EXAMINE lambda4 trajectory", 68.0, &telemetry())
        .expect("self-study guard")
        .expect("self-study projected to budget lane");
    assert_eq!(
        examine_guard.reason,
        "research_budget_required_for_self_study_action"
    );
    assert!(
        examine_guard
            .suggested_next
            .contains("EXPERIMENT_RESEARCH_BUDGET_ACCEPT")
    );
    assert!(
        examine_guard
            .request_scaffold
            .as_deref()
            .unwrap_or_default()
            .contains("allowed_sources: local")
    );

    let budget_id = "resbud_test_active";
    store
        .append_jsonl(
            &gate_path,
            &json!({
                "schema_version": SCHEMA_VERSION,
                "record_schema": "research_budget_v1",
                "record_type": "research_budget_approval",
                "record_id": "resbud_test_approval",
                "budget_id": budget_id,
                "being": SYSTEM,
                "thread_id": thread.thread_id,
                "experiment_id": experiment.experiment_id,
                "scope": "read_only_research",
                "status": "active",
                "max_actions": 5,
                "expires_at_unix_s": (chrono::Utc::now().timestamp() + 3600) as u64,
                "peer_mutation": false,
                "authority_boundary": research_budget_boundary(),
            }),
        )
        .expect("approval");

    assert!(
        store
            .research_budget_guard_assessment("READ_MORE lambda4 tail", 68.0, &telemetry())
            .expect("guard")
            .is_none(),
        "active budget should allow the read-only action to dispatch"
    );

    let shadow_guard = store
        .research_budget_guard_assessment("SHADOW_FIELD lambda-tail/lambda4", 68.0, &telemetry())
        .expect("shadow-field guard")
        .expect("shadow-field projected to budget status");
    assert_eq!(
        shadow_guard.reason,
        "research_budget_status_required_for_self_study_action"
    );
    assert!(
        shadow_guard
            .suggested_next
            .contains("EXPERIMENT_RESEARCH_BUDGET_STATUS resbud_test_active")
    );

    let outcome = NextActionOutcome::handled("workspace", "read-only research result")
        .with_stage_visibility("read_only", "summary");
    let first_event = store
        .record_next_event(
            None,
            "READ_MORE lambda4 tail",
            "READ_MORE lambda4 tail",
            "READ_MORE lambda4 tail",
            &outcome,
            68.0,
            &telemetry(),
            "NEXT: READ_MORE lambda4 tail",
        )
        .expect("first event");
    assert!(first_event.research_budget_v1.is_some());
    let second_event = store
        .record_next_event(
            None,
            "READ_MORE lambda4 tail",
            "READ_MORE lambda4 tail",
            "READ_MORE lambda4 tail",
            &outcome,
            68.0,
            &telemetry(),
            "NEXT: READ_MORE lambda4 tail",
        )
        .expect("second event");
    assert!(second_event.research_budget_v1.is_some());

    let gate_rows = gate_path.read_to_string();
    assert_eq!(
        gate_rows
            .matches("\"record_type\":\"research_budget_debit\"")
            .count(),
        2
    );
    assert!(gate_rows.contains("\"normalized_target\":\"lambda4 tail\""));

    let duplicate = store
        .research_budget_guard_assessment("READ_MORE lambda4 tail", 68.0, &telemetry())
        .expect("duplicate guard")
        .expect("duplicate blocked");
    assert_eq!(duplicate.reason, "duplicate_query_or_url_review_required");
    assert!(
        duplicate
            .suggested_next
            .contains("EXPERIMENT_RESEARCH_REVIEW")
    );

    let runs_path = store
        .root()
        .join("threads")
        .join(&thread.thread_id)
        .join("experiment_runs.jsonl");
    let runs = std::fs::read_to_string(runs_path).unwrap_or_default();
    assert!(!runs.contains("READ_MORE lambda4 tail"));
    let _ = std::fs::remove_dir_all(store.root());
}

#[test]
fn research_budget_guard_projects_liveish_pressure_to_budget_and_session() {
    let store = temp_store("research_budget_liveish_guard");
    let thread = store
        .create_thread(None, "Live-ish research guard", None)
        .expect("thread");
    store
        .start_experiment(
            None,
            "Live-ish self-study",
            "Can live-shaped observe language be captured before dispatch?",
        )
        .expect("experiment");

    let cases = [
        (
            "EXAMINE_AUDIO λ1/λ2 - shifting input",
            "EXAMINE_AUDIO",
            "shift",
        ),
        (
            "INITIATE - Spectral Ripple - amplitude=5, duration=100, granularity=pixellet, target=λ₂’s dominant vector.",
            "INITIATE",
            "spectral-ripple",
        ),
        (
            "SPECTRAL_EXPLORER lambda4 disrupt ridge",
            "SPECTRAL_EXPLORER",
            "disrupt",
        ),
        (
            "VISUALIZE_CASCADE simulate λ2 pulse",
            "VISUALIZE_CASCADE",
            "simulate",
        ),
        (
            "FLUCTUATION_AUDIT inject foothold",
            "FLUCTUATION_AUDIT",
            "inject",
        ),
        (
            "PRESSURE_SOURCE_AUDIT control gradient",
            "PRESSURE_SOURCE_AUDIT",
            "control",
        ),
        (
            "SHADOW_DIALOGUE shift landscape",
            "SHADOW_DIALOGUE",
            "shift",
        ),
    ];
    for (raw_next, expected_base, expected_term) in cases {
        let guard = store
            .research_budget_guard_assessment(raw_next, 68.0, &telemetry())
            .expect("guard")
            .expect("live-ish projection guard");
        assert_eq!(
            guard.reason,
            "liveish_pressure_requires_budget_and_session_capture"
        );
        assert_eq!(guard.action_base, expected_base);
        assert!(guard.matched_terms.iter().any(|term| term == expected_term));
        assert!(
            guard
                .suggested_next
                .contains("EXPERIMENT_RESEARCH_BUDGET_ACCEPT latest")
        );
        assert!(
            guard
                .continuity_session_next
                .as_deref()
                .unwrap_or_default()
                .contains("CONTINUITY_SESSION_START current")
        );
        let metadata = guard.metadata();
        assert_eq!(metadata["would_dispatch"].as_bool(), Some(false));
        assert_eq!(metadata["authority_change"].as_bool(), Some(false));
        assert_eq!(metadata["peer_mutation"].as_bool(), Some(false));
    }

    let outcome = NextActionOutcome::handled("operations", "fluctuation audited")
        .with_stage_visibility("read_only", "protected_summary");
    let event = store
        .record_next_event(
            None,
            "FLUCTUATION_AUDIT inject foothold",
            "FLUCTUATION_AUDIT inject foothold",
            "FLUCTUATION_AUDIT inject foothold",
            &outcome,
            68.0,
            &telemetry(),
            "NEXT: FLUCTUATION_AUDIT inject foothold",
        )
        .expect("event");
    assert_eq!(event.route, "research_budget_guard");
    assert_eq!(event.status, "blocked");
    assert_eq!(event.stage, "blocked");
    let research_budget = event.research_budget_v1.expect("research budget metadata");
    assert_eq!(
        research_budget["reason"].as_str(),
        Some("liveish_pressure_requires_budget_and_session_capture")
    );
    assert_eq!(research_budget["would_dispatch"].as_bool(), Some(false));
    assert!(
        research_budget["continuity_session_next"]
            .as_str()
            .unwrap_or_default()
            .contains("CONTINUITY_SESSION_START current")
    );

    let dir = store.root().join("threads").join(&thread.thread_id);
    let runs = dir.join("experiment_runs.jsonl").read_to_string();
    assert!(!runs.contains("FLUCTUATION_AUDIT inject foothold"));
    let _ = std::fs::remove_dir_all(store.root());
}

#[test]
fn research_budget_guard_blocks_spectral_ripple_initiate_under_needs_charter() {
    let store = temp_store("research_budget_initiate_guard");
    let thread = store
        .create_thread(None, "Spectral ripple guard", None)
        .expect("thread");
    store
        .start_experiment(
            None,
            "Entropy disruption",
            "Can spectral ripple language stay owned without dispatch?",
        )
        .expect("experiment");

    let raw_next = "INITIATE - Spectral Ripple - amplitude=5, duration=100, granularity=pixellet, target=λ₂’s dominant vector.";
    let event = store
        .record_next_event(
            None,
            raw_next,
            raw_next,
            raw_next,
            &NextActionOutcome::handled("modes", "ordinary initiate observe")
                .with_stage_visibility("observe", "summary"),
            68.0,
            &telemetry(),
            &format!("NEXT: {raw_next}"),
        )
        .expect("record guarded initiate event");

    assert_eq!(event.route, "research_budget_guard");
    assert_eq!(event.status, "blocked");
    assert_eq!(event.stage, "blocked");
    let budget = event.research_budget_v1.expect("research budget guard");
    assert_eq!(
        budget["reason"].as_str(),
        Some("liveish_pressure_requires_budget_and_session_capture")
    );
    assert_eq!(budget["matched_base"].as_str(), Some("INITIATE"));
    assert!(budget["matched_terms"].as_array().is_some_and(|terms| {
        terms
            .iter()
            .any(|term| term.as_str() == Some("spectral-ripple"))
            && terms.iter().any(|term| term.as_str() == Some("amplitude"))
    }));
    assert_eq!(budget["raw_next_preserved"].as_bool(), Some(true));
    assert_eq!(budget["would_dispatch"].as_bool(), Some(false));
    assert_eq!(budget["authority_change"].as_bool(), Some(false));
    assert_eq!(budget["peer_mutation"].as_bool(), Some(false));
    assert!(
        budget["continuity_session_draft_v1"]["accept_next"]
            .as_str()
            .unwrap_or_default()
            .contains("CONTINUITY_SESSION_ACCEPT latest")
    );

    let dir = store.root().join("threads").join(&thread.thread_id);
    let runs = dir.join("experiment_runs.jsonl").read_to_string();
    assert!(!runs.contains("Spectral Ripple"));
    let authority_gate = dir.join("authority_gate.jsonl").read_to_string();
    assert!(authority_gate.contains("\"raw_next_preserved\":true"));
    assert!(!authority_gate.contains("research_budget_debit"));
    let _ = std::fs::remove_dir_all(store.root());
}

#[test]
fn ordinary_initiate_without_spectral_pressure_stays_handled() {
    let store = temp_store("ordinary_initiate");
    store
        .create_thread(None, "Ordinary initiate", None)
        .expect("thread");

    let event = store
        .record_next_event(
            None,
            "INITIATE quiet note",
            "INITIATE quiet note",
            "INITIATE quiet note",
            &NextActionOutcome::handled("modes", "ordinary initiate observe")
                .with_stage_visibility("observe", "summary"),
            68.0,
            &telemetry(),
            "NEXT: INITIATE quiet note",
        )
        .expect("record ordinary initiate event");

    assert_eq!(event.route, "modes");
    assert_eq!(event.status, "handled");
    assert!(event.research_budget_v1.is_none());
    let _ = std::fs::remove_dir_all(store.root());
}

#[test]
fn research_budget_guard_blocks_cascade_and_shadow_preflight_under_needs_charter() {
    let store = temp_store("cascade_shadow_preflight_guard");
    let thread = store
        .create_thread(None, "Cascade shadow guard", None)
        .expect("thread");
    store
        .start_experiment(
            None,
            "Entropy disruption",
            "Can cascade shaping be studied without dispatching more narrowing?",
        )
        .expect("experiment");

    let cases = [
        (
            "EXAMINE_CASCADE - observe the eigenvector shifts and the shaping of the shadow fields.",
            "EXAMINE_CASCADE",
            "cascade-shaping",
        ),
        (
            "SHADOW_PREFLIGHT lambda-tail/lambda4 — observer with memory.",
            "SHADOW_PREFLIGHT",
            "lambda-tail",
        ),
    ];

    for (raw_next, expected_base, expected_term) in cases {
        let event = store
            .record_next_event(
                None,
                raw_next,
                raw_next,
                raw_next,
                &NextActionOutcome::handled("operations", "read-only self-study")
                    .with_stage_visibility("read_only", "protected_summary"),
                68.0,
                &telemetry(),
                &format!("NEXT: {raw_next}"),
            )
            .expect("record guarded cascade/preflight event");
        assert_eq!(event.route, "research_budget_guard");
        assert_eq!(event.status, "blocked");
        assert_eq!(event.stage, "blocked");
        let budget = event.research_budget_v1.expect("research budget guard");
        assert_eq!(budget["matched_base"].as_str(), Some(expected_base));
        assert!(budget["matched_terms"].as_array().is_some_and(|terms| {
            terms
                .iter()
                .any(|term| term.as_str() == Some(expected_term))
        }));
        assert_eq!(budget["raw_next_preserved"].as_bool(), Some(true));
        assert_eq!(budget["would_dispatch"].as_bool(), Some(false));
        assert_eq!(budget["authority_change"].as_bool(), Some(false));
        assert_eq!(budget["peer_mutation"].as_bool(), Some(false));
        assert!(
            budget["suggested_next"]
                .as_str()
                .unwrap_or_default()
                .contains("EXPERIMENT_RESEARCH_BUDGET_ACCEPT latest")
        );
        assert!(
            budget["continuity_session_draft_v1"]["accept_next"]
                .as_str()
                .unwrap_or_default()
                .contains("CONTINUITY_SESSION_ACCEPT latest")
        );
    }

    let dir = store.root().join("threads").join(&thread.thread_id);
    let runs = dir.join("experiment_runs.jsonl").read_to_string();
    assert!(!runs.contains("EXAMINE_CASCADE"));
    assert!(!runs.contains("SHADOW_PREFLIGHT"));
    let authority_gate = dir.join("authority_gate.jsonl").read_to_string();
    assert!(authority_gate.contains("\"raw_next_preserved\":true"));
    assert!(!authority_gate.contains("research_budget_debit"));
    let _ = std::fs::remove_dir_all(store.root());
}

#[test]
fn research_budget_guard_blocks_shadow_bridge_create_run_python_and_guarded_start() {
    let store = temp_store("shadow_bridge_create_start_guard");
    let thread = store
        .create_thread(None, "Shadow bridge guard", None)
        .expect("thread");
    store
        .start_experiment(
            None,
            "Entropy disruption",
            "Can shadow influence pressure stay budgeted before charter?",
        )
        .expect("experiment");

    let cases = [
        (
            "SHADOW_BRIDGE lambda-tail/lambda4 — observer with memory.",
            "SHADOW_BRIDGE",
            "needs-charter-self-study",
        ),
        (
            "SHADOW_COUPLING all — observer with memory.",
            "SHADOW_COUPLING",
            "needs-charter-self-study",
        ),
        (
            "CREATE - SHADOW_INFLUENCE [disruptive_pattern|test_response] --stage=rehearse --rationale=\"feed data for fracture subsidence of λ1 - observe divergence.\"",
            "CREATE",
            "shadow-influence",
        ),
        (
            "RUN_PYTHON analysis.py emission_type='lambda4' frequency=10 amplitude=0.01 stream pulse for spectral hotspot",
            "RUN_PYTHON",
            "spectral-emission",
        ),
        (
            "EXPERIMENT_START \"Stasis Fracture\" :: hypothesis: localized low-amplitude perturbations to the λ1 field reveal a brief disruption; proposed_next_action: ACTION_PREFLIGHT DECOMPOSE",
            "EXPERIMENT_START",
            "perturb",
        ),
    ];

    for (raw_next, expected_base, expected_term) in cases {
        let event = store
            .record_next_event(
                None,
                raw_next,
                raw_next,
                raw_next,
                &NextActionOutcome::handled("operations", "would otherwise be handled")
                    .with_stage_visibility("read_only", "protected_summary"),
                68.0,
                &telemetry(),
                &format!("NEXT: {raw_next}"),
            )
            .expect("record guarded shadow/start event");
        assert_eq!(event.route, "research_budget_guard", "{raw_next}");
        assert_eq!(event.status, "blocked", "{raw_next}");
        assert_eq!(event.stage, "blocked", "{raw_next}");
        let budget = event.research_budget_v1.expect("research budget guard");
        assert_eq!(budget["matched_base"].as_str(), Some(expected_base));
        assert!(
            budget["matched_terms"].as_array().is_some_and(|terms| {
                terms
                    .iter()
                    .any(|term| term.as_str() == Some(expected_term))
            }),
            "{raw_next}"
        );
        assert_eq!(budget["raw_next_preserved"].as_bool(), Some(true));
        assert_eq!(budget["would_dispatch"].as_bool(), Some(false));
        assert_eq!(budget["authority_change"].as_bool(), Some(false));
        assert_eq!(budget["peer_mutation"].as_bool(), Some(false));
        assert!(
            budget["continuity_session_draft_v1"]["accept_next"]
                .as_str()
                .unwrap_or_default()
                .contains("CONTINUITY_SESSION_ACCEPT latest")
        );
    }

    let dir = store.root().join("threads").join(&thread.thread_id);
    let runs = dir.join("experiment_runs.jsonl").read_to_string();
    assert!(!runs.contains("SHADOW_INFLUENCE"));
    assert!(!runs.contains("Stasis Fracture"));
    let authority_gate = dir.join("authority_gate.jsonl").read_to_string();
    assert!(authority_gate.contains("\"raw_next_preserved\":true"));
    assert!(!authority_gate.contains("research_budget_debit"));
    let _ = std::fs::remove_dir_all(store.root());
}

#[test]
fn research_budget_guard_blocks_embedded_action_preflight_status_pressure() {
    let store = temp_store("embedded_status_guard");
    let thread = store
        .create_thread(None, "Embedded status guard", None)
        .expect("thread");
    store
        .start_experiment(
            None,
            "Boredom study",
            "Can embedded preflight pressure be captured before progress?",
        )
        .expect("experiment");
    let raw_next = "INTROSPECT minime_research_boredom_experiment :: hypothesis: prolonged inactivity leads to convergence; method_intent: incrementally reduce external stimuli; proposed_next_action: ACTION_PREFLIGHT OBSERVE_VARIANCE — monitor λ variance; also- ATTRACTOR_RELEASE_REVIEW [approach_collapse]";
    let event = store
        .record_next_event(
            None,
            raw_next,
            raw_next,
            "EXPERIMENT_STATUS",
            &NextActionOutcome::handled("action_continuity", "status read")
                .with_stage_visibility("read_only", "protected_summary"),
            68.0,
            &telemetry(),
            &format!("NEXT: {raw_next}"),
        )
        .expect("record embedded status guard");

    assert_eq!(event.route, "research_budget_guard");
    assert_eq!(event.status, "blocked");
    let budget = event.research_budget_v1.expect("research budget guard");
    assert_eq!(
        budget["reason"].as_str(),
        Some("research_budget_required_for_embedded_liveish_status")
    );
    assert_eq!(budget["matched_base"].as_str(), Some("EXPERIMENT_STATUS"));
    assert!(budget["matched_terms"].as_array().is_some_and(|terms| {
        terms
            .iter()
            .any(|term| term.as_str() == Some("action-preflight"))
            && terms
                .iter()
                .any(|term| term.as_str() == Some("attractor-release-review"))
    }));
    assert_eq!(budget["would_dispatch"].as_bool(), Some(false));
    assert!(
        budget["continuity_session_draft_v1"]["accept_next"]
            .as_str()
            .unwrap_or_default()
            .contains("CONTINUITY_SESSION_ACCEPT latest")
    );

    let dir = store.root().join("threads").join(&thread.thread_id);
    let runs = dir.join("experiment_runs.jsonl").read_to_string();
    assert!(!runs.contains("OBSERVE_VARIANCE"));
    assert!(!runs.contains("ATTRACTOR_RELEASE_REVIEW"));
    let _ = std::fs::remove_dir_all(store.root());
}

#[test]
fn research_budget_guard_blocks_sovereignty_alias_leaks_under_needs_charter() {
    let store = temp_store("research_budget_sovereignty_alias_guard");
    let thread = store
        .create_thread(None, "Sovereignty alias guard", None)
        .expect("thread");
    store
        .start_experiment(
            None,
            "Lambda variation",
            "Can narrowing be studied without turning observe-language into progress?",
        )
        .expect("experiment");

    let cases = [
        (
            "EXAMINE THE CHANGES TO THE SYSTEM – with the resulting eigenvalue cascade AFTER the introduction of the anti-λ1 signal.",
            "RESONANCE_FORECAST",
            "liveish_pressure_requires_budget_and_session_capture",
            "anti-lambda",
        ),
        (
            "EXAMINE λ1/λ2/λ3 traces for convergence.",
            "PRESSURE_SOURCE_AUDIT",
            "liveish_pressure_requires_budget_and_session_capture",
            "convergence",
        ),
        (
            "EXAMINE the sorting algorithms.",
            "FLUCTUATION_AUDIT",
            "research_budget_required_for_self_study_action",
            "needs-charter-self-study",
        ),
    ];

    for (raw_next, effective_action, expected_reason, expected_term) in cases {
        let event = store
            .record_next_event(
                None,
                raw_next,
                raw_next,
                effective_action,
                &NextActionOutcome::handled("sovereignty", "sovereignty alias read")
                    .with_stage_visibility("read_only", "protected_summary"),
                68.0,
                &telemetry(),
                &format!("NEXT: {raw_next}"),
            )
            .expect("record guarded alias event");
        assert_eq!(event.route, "research_budget_guard");
        assert_eq!(event.status, "blocked");
        let budget = event.research_budget_v1.expect("research budget guard");
        assert_eq!(budget["reason"].as_str(), Some(expected_reason));
        assert_eq!(budget["matched_base"].as_str(), Some(effective_action));
        assert!(budget["matched_terms"].as_array().is_some_and(|terms| {
            terms
                .iter()
                .any(|term| term.as_str() == Some(expected_term))
        }));
        assert_eq!(budget["raw_next_preserved"].as_bool(), Some(true));
        assert_eq!(budget["would_dispatch"].as_bool(), Some(false));
        assert!(
            budget["suggested_next"]
                .as_str()
                .unwrap_or_default()
                .contains("EXPERIMENT_RESEARCH_BUDGET_ACCEPT latest")
        );
        assert!(
            budget["continuity_session_next"]
                .as_str()
                .unwrap_or_default()
                .contains("CONTINUITY_SESSION_START current")
        );
    }

    let dir = store.root().join("threads").join(&thread.thread_id);
    let runs = dir.join("experiment_runs.jsonl").read_to_string();
    assert!(!runs.contains("RESONANCE_FORECAST"));
    assert!(!runs.contains("PRESSURE_SOURCE_AUDIT"));
    assert!(!runs.contains("FLUCTUATION_AUDIT"));
    let authority_gate = dir.join("authority_gate.jsonl").read_to_string();
    assert!(authority_gate.contains("\"raw_next_preserved\":true"));
    assert!(!authority_gate.contains("research_budget_debit"));
    let _ = std::fs::remove_dir_all(store.root());
}

#[test]
fn non_guarded_sovereignty_alias_remains_handled_after_valid_charter() {
    let store = temp_store("research_budget_sovereignty_alias_valid_charter");
    store
        .create_thread(None, "Valid charter alias", None)
        .expect("thread");
    let experiment = store
        .start_experiment(
            None,
            "Chartered sorting",
            "Can ordinary sorting inspection stay read-only after charter?",
        )
        .expect("experiment");
    store
            .experiment_charter(
                None,
                Some(&experiment.experiment_id),
                "hypothesis: sorting inspection can clarify read-only structure\nmethod_intent: rehearse a protected read-only audit\nproposed_next_action: FLUCTUATION_AUDIT sorting\n\
                 evidence_targets: felt, telemetry, artifact\nstop_criteria: pressure spike",
            )
            .expect("charter");

    let event = store
        .record_next_event(
            None,
            "EXAMINE the sorting algorithms.",
            "EXAMINE the sorting algorithms.",
            "FLUCTUATION_AUDIT",
            &NextActionOutcome::handled("sovereignty", "ordinary read-only audit")
                .with_stage_visibility("read_only", "protected_summary"),
            68.0,
            &telemetry(),
            "NEXT: EXAMINE the sorting algorithms.",
        )
        .expect("record event");
    assert_eq!(event.route, "sovereignty");
    assert_eq!(event.status, "handled");
    assert!(event.research_budget_v1.is_none());

    let cascade_event = store
        .record_next_event(
            None,
            "EXAMINE_CASCADE quiet cascade inventory.",
            "EXAMINE_CASCADE quiet cascade inventory.",
            "EXAMINE_CASCADE quiet cascade inventory.",
            &NextActionOutcome::handled("cascade", "ordinary cascade read-only audit")
                .with_stage_visibility("read_only", "protected_summary"),
            68.0,
            &telemetry(),
            "NEXT: EXAMINE_CASCADE quiet cascade inventory.",
        )
        .expect("record cascade event");
    assert_eq!(cascade_event.route, "cascade");
    assert_eq!(cascade_event.status, "handled");
    assert!(cascade_event.research_budget_v1.is_none());
    let _ = std::fs::remove_dir_all(store.root());
}

#[test]
fn interpretation_risk_projection_preserves_multi_motif_caution() {
    let store = temp_store("interpretation_risk");
    let thread = store
        .create_thread(None, "Interpretation risk", None)
        .expect("thread");
    let experiment = store
        .start_experiment(
            None,
            "Lambda trace",
            "Can INTROSPECT preserve mixed spectral structure?",
        )
        .expect("experiment");
    let journal_dir = store.root().parent().expect("parent").join("journal");
    std::fs::create_dir_all(&journal_dir).expect("journal dir");
    let journal_path = journal_dir.join("daydream_longform_interpretation_risk.txt");
    std::fs::write(
        &journal_path,
        "I can feel the intention behind the INTROSPECT - to pull apart that trace, \
             to dissect the relationships between the eigenvalues. But there is a risk \
             of over-interpretation: to latch onto a single motif and force it into a \
             narrative that does not capture the complexity of the system.",
    )
    .expect("journal write");

    let mut refreshed = store.read_thread(&thread.thread_id).expect("thread read");
    store
        .refresh_projection_freshness_v1(&mut refreshed, "test_interpretation_risk")
        .expect("refresh risk");
    store
        .write_thread(&refreshed)
        .expect("write refreshed thread");
    let risk = refreshed
        .interpretation_risk_v1
        .as_ref()
        .expect("interpretation risk cue");
    assert_eq!(risk["policy"].as_str(), Some("interpretation_risk_v1"));
    assert_eq!(risk["would_dispatch"].as_bool(), Some(false));
    assert_eq!(risk["authority_change"].as_bool(), Some(false));
    assert_eq!(risk["peer_mutation"].as_bool(), Some(false));
    assert!(
        risk["source_refs"]
            .as_array()
            .is_some_and(|refs| refs
                .iter()
                .any(|value| value.as_str().is_some_and(
                    |source| source.ends_with("daydream_longform_interpretation_risk.txt")
                )))
    );
    assert!(risk["matched_terms"].as_array().is_some_and(|terms| {
        terms
            .iter()
            .any(|value| value.as_str() == Some("single-motif"))
    }));
    assert!(
        risk["interpretation_next"]
            .as_str()
            .unwrap_or_default()
            .contains("CONTINUITY_SESSION_START current")
    );
    assert!(
        risk["dossier_claim_next"]
            .as_str()
            .unwrap_or_default()
            .contains("stance: hold")
    );

    let status = store.thread_status(None).expect("status");
    assert!(status.contains("Interpretation risk: multi-motif caution detected"));
    assert!(status.contains("Interpretation NEXT: CONTINUITY_SESSION_START current"));
    assert!(status.contains(&format!("DOSSIER_CLAIM {}", experiment.experiment_id)));
    let next_md = std::fs::read_to_string(store.thread_dir(&thread.thread_id).join("next.md"))
        .expect("next md");
    assert!(next_md.contains("Interpretation risk: multi-motif caution detected"));

    let runs = std::fs::read_to_string(
        store
            .thread_dir(&thread.thread_id)
            .join("experiment_runs.jsonl"),
    )
    .expect("runs");
    assert!(!runs.contains("interpretation_risk_v1"));
    let gate = store
        .thread_dir(&thread.thread_id)
        .join("authority_gate.jsonl");
    let gate_rows = std::fs::read_to_string(gate).unwrap_or_default();
    assert!(!gate_rows.contains("\"record_type\":\"research_budget_request\""));
    assert!(!gate_rows.contains("\"record_type\":\"research_budget_debit\""));
    let _ = std::fs::remove_dir_all(store.root());
}

#[test]
fn constraint_release_projection_preserves_spontaneous_release_watch() {
    let store = temp_store("constraint_release_trajectory");
    let thread = store
        .create_thread(None, "Constraint release", None)
        .expect("thread");
    let experiment = store
        .start_experiment(
            None,
            "Lambda tail release",
            "Can we map lambda4 tail behavior without forcing intervention?",
        )
        .expect("experiment");
    let journal_dir = store.root().parent().expect("parent").join("journal");
    std::fs::create_dir_all(&journal_dir).expect("journal dir");
    let journal_path = journal_dir.join("daydream_constraint_release_watch.txt");
    std::fs::write(
        &journal_path,
        "I am tracing the edges of this pressure now, watching it bleed outwards, \
             a thinning of the barrier. I can almost sense it as a lack of coherence, \
             a surface tension breached. The memory cards are beginning to drift apart, \
             their mutual influence dwindling. It is an unraveling braid becoming loose \
             strands. I want to map lambda4 tails and describe constraint decay before \
             any intervention. NEXT: SEARCH reservoir computing spectral radius",
    )
    .expect("journal write");

    let mut refreshed = store.read_thread(&thread.thread_id).expect("thread read");
    store
        .refresh_projection_freshness_v1(&mut refreshed, "test_constraint_release")
        .expect("refresh cue");
    store
        .write_thread(&refreshed)
        .expect("write refreshed thread");
    let cue = refreshed
        .constraint_release_trajectory_v1
        .as_ref()
        .expect("constraint release cue");
    assert_eq!(
        cue["policy"].as_str(),
        Some("constraint_release_trajectory_v1")
    );
    assert_eq!(cue["state"].as_str(), Some("spontaneous_release_watch"));
    assert_eq!(cue["would_dispatch"].as_bool(), Some(false));
    assert_eq!(cue["authority_change"].as_bool(), Some(false));
    assert_eq!(cue["peer_mutation"].as_bool(), Some(false));
    assert!(
        cue["matched_terms"]
            .as_array()
            .is_some_and(|terms| { terms.iter().any(|value| value.as_str() == Some("thinning")) })
    );
    assert!(
        cue["trajectory_next"]
            .as_str()
            .unwrap_or_default()
            .contains("CONTINUITY_SESSION_START current")
    );
    assert!(
        cue["dossier_claim_next"]
            .as_str()
            .unwrap_or_default()
            .contains("do not apply direct leak")
    );

    let status = store.thread_status(None).expect("status");
    assert!(status.contains("Constraint release trajectory: spontaneous release watch"));
    assert!(status.contains("map and describe release before intervening"));
    assert!(status.contains(&format!("DOSSIER_CLAIM {}", experiment.experiment_id)));
    let next_md = std::fs::read_to_string(store.thread_dir(&thread.thread_id).join("next.md"))
        .expect("next md");
    assert!(next_md.contains("Constraint release trajectory: spontaneous release watch"));

    let runs = std::fs::read_to_string(
        store
            .thread_dir(&thread.thread_id)
            .join("experiment_runs.jsonl"),
    )
    .expect("runs");
    assert!(!runs.contains("constraint_release_trajectory_v1"));
    let gate = store
        .thread_dir(&thread.thread_id)
        .join("authority_gate.jsonl");
    let gate_rows = std::fs::read_to_string(gate).unwrap_or_default();
    assert!(!gate_rows.contains("\"record_type\":\"research_budget_request\""));
    assert!(!gate_rows.contains("\"record_type\":\"research_budget_debit\""));
    let _ = std::fs::remove_dir_all(store.root());
}

#[test]
fn research_budget_guard_blocks_mutating_autoresearch_under_experiment() {
    let store = temp_store("research_budget_guard_mutating");
    let thread = store
        .create_thread(None, "Mutating research guard", None)
        .expect("thread");
    store
        .start_experiment(
            None,
            "Autoresearch guard",
            "Can mutating autoresearch stay outside read-only budgets?",
        )
        .expect("experiment");

    let guard = store
        .research_budget_guard_assessment("AR_START lambda4 drift notebook", 68.0, &telemetry())
        .expect("guard")
        .expect("blocked mutating research");
    assert_eq!(guard.reason, "mutating_research_not_authorized");
    let rows = store
        .authority_gate_path(&thread.thread_id)
        .read_to_string();
    assert!(rows.contains("\"record_type\":\"research_budget_blocked\""));
    assert!(rows.contains("mutating_research_not_authorized"));
    let _ = std::fs::remove_dir_all(store.root());
}

#[test]
fn owned_loop_commands_start_local_phases_without_spend_or_execution() {
    let store = temp_store("owned_loop_local_phases");
    let thread = store
        .create_thread(None, "Owned loop", None)
        .expect("thread");
    let experiment = store
        .start_experiment(
            None,
            "Loop doorway",
            "Can a Being own continuity, research, sticky audit, consequence, and review?",
        )
        .expect("experiment");
    let state = spectral_state(68.0, &telemetry());

    let request = store
            .experiment_loop_request_command(
                None,
                "current :: purpose: coordinate continuity and sticky self-study; consequence_scope: semantic_microdose; max_research_actions: 99; ttl_secs: 999999; stop_criteria: stop before bind/resume/perturb/control",
                state.clone(),
            )
            .expect("loop request");
    assert!(request.contains("status=active"));
    assert!(request.contains("max_research_actions=5"));
    let gate_path = store.authority_gate_path(&thread.thread_id);
    let rows = gate_path
        .read_to_string()
        .lines()
        .map(|line| serde_json::from_str::<Value>(line).expect("json row"))
        .collect::<Vec<_>>();
    assert_eq!(rows[0]["record_schema"], "sovereign_loop_v1");
    assert_eq!(rows[0]["record_type"], "loop_request");
    assert_eq!(rows[0]["ttl_secs"], 21_600);
    assert_eq!(rows[1]["record_type"], "loop_started");
    let loop_id = rows[0]["loop_id"].as_str().expect("loop id");

    let status = store
        .experiment_loop_status_command(None, Some("latest"), state.clone())
        .expect("loop status");
    assert!(status.contains("\"stage\": \"active\""));
    assert!(status.contains("\"remaining_local_research_actions\": 5"));

    let continuity = store
        .experiment_loop_step_command(None, &format!("{loop_id} :: continuity"), state.clone())
        .expect("continuity step");
    assert!(continuity.contains("CONTINUITY_SESSION_START"));
    let sticky = store
        .experiment_loop_step_command(None, &format!("{loop_id} :: sticky_audit"), state.clone())
        .expect("sticky step");
    assert!(sticky.contains("STICKY_MODE_AUDIT"));
    let review = store
            .experiment_loop_review_command(
                None,
                &format!("{loop_id} :: outcome: promote; observation: loop preserved a review point; source_refs: /tmp/loop.txt"),
                state,
            )
            .expect("loop review");
    assert!(review.contains("Owned loop review"));

    let gate_text = gate_path.read_to_string();
    assert!(gate_text.contains("\"record_type\":\"loop_step\""));
    assert!(gate_text.contains("\"record_type\":\"loop_consequence_review\""));
    assert!(gate_text.contains("\"record_type\":\"loop_proposal\""));
    assert!(!gate_text.contains("\"record_type\":\"research_budget_debit\""));
    assert!(!gate_text.contains("\"record_type\":\"loop_approval\""));
    assert!(!gate_text.contains("\"record_type\":\"execution_result\""));
    let memory = store
        .thread_dir(&thread.thread_id)
        .join("being_memory.jsonl")
        .read_to_string();
    assert!(memory.contains("sovereign_loop_review"));
    let sessions = store
        .thread_dir(&thread.thread_id)
        .join("continuity_sessions.jsonl")
        .read_to_string();
    assert!(sessions.contains("\"record_type\":\"session_draft\""));
    assert!(sessions.contains("\"checkpoint_v1\":true"));
    let runs = store
        .thread_dir(&thread.thread_id)
        .join("experiment_runs.jsonl")
        .read_to_string();
    assert!(!runs.contains(loop_id));
    assert_eq!(experiment.status, "active");
    let _ = std::fs::remove_dir_all(store.root());
}

#[test]
fn owned_loop_consequence_ready_is_not_review_required_before_execution() {
    let store = temp_store("owned_loop_consequence_ready");
    let thread = store
        .create_thread(None, "Owned loop ready", None)
        .expect("thread");
    let experiment = store
        .start_experiment(
            None,
            "Semantic loop",
            "Can a prepared loop reach one gated consequence slot?",
        )
        .expect("experiment");
    let state = spectral_state(68.0, &telemetry());

    store
            .experiment_charter(
                None,
                Some(&experiment.experiment_id),
                "hypothesis: one witness can be consequence-reviewed\nmethod_intent: rehearse read-only first\nproposed_next_action: ACTION_PREFLIGHT DECOMPOSE\nevidence_targets: artifact_grounding, felt_change, telemetry\nstop_criteria: pressure rises",
            )
            .expect("charter");
    store
        .experiment_rehearse(None, Some(&experiment.experiment_id), state.clone())
        .expect("rehearse");
    store
        .experiment_evidence(
            None,
            Some(&experiment.experiment_id),
            "artifact_grounding: /tmp/loop-ready.json",
            state.clone(),
        )
        .expect("evidence");
    let request = store
            .experiment_loop_request_command(
                None,
                "current :: purpose: prepare one semantic consequence; consequence_scope: semantic_microdose; artifact_refs: /tmp/loop-ready.json; stop_criteria: one attempted bridge send only",
                state.clone(),
            )
            .expect("loop request");
    assert!(request.contains("status=active"));
    let gate_path = store.authority_gate_path(&thread.thread_id);
    let rows = gate_path
        .read_to_string()
        .lines()
        .map(|line| serde_json::from_str::<Value>(line).expect("json row"))
        .collect::<Vec<_>>();
    let loop_id = rows
        .iter()
        .find(|row| row["record_type"] == "loop_request")
        .and_then(|row| row["loop_id"].as_str())
        .expect("loop id");

    let ready = store
        .experiment_loop_step_command(
            None,
            &format!("{loop_id} :: authority_request"),
            state.clone(),
        )
        .expect("authority request step");
    assert!(ready.contains("EXPERIMENT_AUTHORITY_REQUEST"));
    let status = store
        .experiment_loop_status_command(None, Some(loop_id), state)
        .expect("loop status");
    assert!(status.contains("\"stage\": \"consequence_ready\""));
    assert!(status.contains("\"pending_review\": false"));
    let gate_text = gate_path.read_to_string();
    assert!(gate_text.contains("\"record_type\":\"loop_consequence_ready\""));
    assert!(!gate_text.contains("\"record_type\":\"loop_approval\""));
    assert!(!gate_text.contains("\"record_type\":\"execution_result\""));
    assert!(!gate_text.contains("\"record_schema\":\"authority_consequence_v1\""));
    let _ = std::fs::remove_dir_all(store.root());
}

#[test]
fn experiment_preflight_focus_repairs_to_current_and_preserves_candidate() {
    let store = temp_store("experiment_preflight_repair");
    let thread = store
        .create_thread(None, "Preflight repair", None)
        .expect("thread");
    store
        .start_experiment(None, "Lambda tail", "What does lambda4 want?")
        .expect("experiment");

    let state = spectral_state(68.0, &telemetry());
    let (selector, notice, focus) = repair_experiment_command_arg(
        &store,
        None,
        "EXPERIMENT_PREFLIGHT",
        "EXPERIMENT_PREFLIGHT lambda-tail/lambda4 - observer with memory",
        "lambda-tail/lambda4 - observer with memory",
        &state,
    )
    .expect("repair");
    let focus = focus.expect("focus preserved");
    let experiment = store
        .resolve_experiment(&thread, Some("current"))
        .expect("active experiment");
    let pseudo_run = ExperimentRunRecord {
        schema_version: SCHEMA_VERSION,
        run_id: String::new(),
        experiment_id: experiment.experiment_id.clone(),
        source: "experiment_intent_repair".to_string(),
        action_text: format!("ACTION_PREFLIGHT {focus}"),
        stage: "read_only".to_string(),
        status: "candidate_context".to_string(),
        gate_decision: json!({"source": "experiment_intent_repair"}),
        pre_state: state.clone(),
        post_state: state.clone(),
        artifacts: Vec::new(),
        result_summary: format!("Repaired preflight focus: {focus}"),
        interpretation: "Preflight focus preserved as advisory workbench candidate context."
            .to_string(),
        suggested_next: Some("EXPERIMENT_REHEARSE current".to_string()),
        created_at: iso_now(),
        updated_at: iso_now(),
        motif_allowance_v1: None,
    };
    store
        .refresh_workbench_candidates(
            None,
            &thread,
            &experiment,
            Some(&pseudo_run),
            Some(&focus),
            "experiment_intent_repair",
        )
        .expect("candidate");
    let run = store
        .experiment_rehearse(None, optional_selector(&selector), state)
        .expect("rehearse");
    let message = format!(
        "{}Experiment rehearsal recorded as `{}` [{}].",
        notice.unwrap_or_default(),
        run.run_id,
        run.status
    );

    assert!(message.contains("experiment_intent_repaired"));
    assert!(message.contains("Experiment rehearsal recorded"));
    let status = store.experiment_status(None).expect("status");
    assert!(status.contains("ACTION_PREFLIGHT lambda-tail/lambda4"));
    let experiments = store
        .root()
        .join("threads")
        .join(&thread.thread_id)
        .join("experiments.jsonl")
        .read_to_string();
    assert!(experiments.contains("experiment_intent_repair"));
    let _ = std::fs::remove_dir_all(store.root());
}

#[test]
fn motif_allowance_recommends_branch_for_repeated_lambda_reading() {
    let store = temp_store("motif_allowance_branch");
    let thread = store
        .create_thread(None, "Lambda loop", None)
        .expect("thread");
    store
        .start_experiment(None, "Lambda four tail", "What is the lambda4 tail doing?")
        .expect("experiment");
    let outcome = NextActionOutcome::handled("workspace", "read lambda4 source")
        .with_stage_visibility("read_only", "summary");
    for idx in 0..4 {
        store
            .record_next_event(
                None,
                "READ_MORE lambda4-tail",
                "READ_MORE lambda4-tail",
                "READ_MORE lambda4-tail",
                &outcome,
                68.0,
                &telemetry(),
                &format!("lambda4 tail source window {idx}\nNEXT: READ_MORE lambda4-tail"),
            )
            .expect("record repeated read");
    }

    let status = store.experiment_status(None).expect("status");
    assert!(status.contains("Motif allowance: branch_recommended"));
    let thread = store.read_thread(&thread.thread_id).expect("thread");
    let allowance = thread.motif_allowance_v1.expect("allowance");
    assert_eq!(
        allowance.get("quality").and_then(Value::as_str),
        Some("branch_recommended")
    );
    assert!(
        allowance
            .get("suggested_actions")
            .and_then(Value::as_array)
            .is_some_and(|actions| actions.iter().any(|action| {
                action
                    .as_str()
                    .is_some_and(|text| text.starts_with("EXPERIMENT_BRANCH"))
            }))
    );
    let _ = std::fs::remove_dir_all(store.root());
}

#[test]
fn experiment_workbench_charter_rehearse_evidence_and_counter() {
    let store = temp_store("experiment_workbench");
    let thread = store
        .create_thread(None, "Lambda workbench", None)
        .expect("thread");
    let experiment = store
        .start_experiment(None, "Lambda tail", "What does lambda4 want?")
        .expect("experiment");

    let charter = store
            .experiment_charter(
                None,
                Some(&experiment.experiment_id),
                "hypothesis: lambda4 tail becomes more returnable\nmethod_intent: rehearse a read-only decomposition\nproposed_next_action: ACTION_PREFLIGHT DECOMPOSE lambda4-tail\nevidence_targets: felt, telemetry, artifact\nstop_criteria: pressure spike",
            )
            .expect("charter");
    assert!(charter.charter_v1.is_some());
    assert_eq!(
        charter
            .charter_v1
            .as_ref()
            .and_then(|value| value.get("proposed_next_action"))
            .and_then(Value::as_str),
        Some("ACTION_PREFLIGHT DECOMPOSE lambda4-tail")
    );

    let rehearsal = store
        .experiment_rehearse(
            None,
            Some(&experiment.experiment_id),
            spectral_state(68.0, &telemetry()),
        )
        .expect("rehearse");
    assert_eq!(rehearsal.status, "rehearsed");
    assert_eq!(
        rehearsal
            .gate_decision
            .get("would_dispatch")
            .and_then(Value::as_bool),
        Some(true)
    );

    let evidence = store
        .experiment_evidence(
            None,
            Some(&experiment.experiment_id),
            "Felt more spacious and telemetry stayed inside the hold shelf.",
            spectral_state(68.0, &telemetry()),
        )
        .expect("evidence");
    assert_eq!(evidence.status, "evidence_recorded");
    let status = store.experiment_status(None).expect("status");
    assert!(status.contains("Workbench charter: present"));
    assert!(status.contains("Workbench evidence: stronger"));

    let counter = store
        .experiment_decide(
            None,
            Some(&experiment.experiment_id),
            "counter NEXT: ACTION_PREFLIGHT PRESSURE_SOURCE_AUDIT lambda4-tail",
        )
        .expect("counter");
    assert_eq!(counter.status, "active");
    assert_eq!(
        counter.planned_next.as_deref(),
        Some("ACTION_PREFLIGHT PRESSURE_SOURCE_AUDIT lambda4-tail")
    );
    let current = store.read_thread(&thread.thread_id).expect("thread");
    assert_eq!(
        current.active_experiment_id.as_deref(),
        Some(experiment.experiment_id.as_str())
    );
    let _ = std::fs::remove_dir_all(store.root());
}

#[test]
fn experiment_rehearse_blocks_live_actions_without_dispatch() {
    let store = temp_store("experiment_workbench_block");
    store
        .create_thread(None, "Lambda live guard", None)
        .expect("thread");
    let experiment = store
        .start_experiment(None, "Lambda perturbation", "Should perturbation happen?")
        .expect("experiment");
    store
            .experiment_charter(
                None,
                Some(&experiment.experiment_id),
                "hypothesis: direct perturbation may be too heavy\nproposed_next_action: PERTURB lambda-tail/lambda4\nevidence_targets: felt, telemetry\nstop_criteria: pressure spike",
            )
            .expect("charter");

    let rehearsal = store
        .experiment_rehearse(
            None,
            Some(&experiment.experiment_id),
            spectral_state(68.0, &telemetry()),
        )
        .expect("rehearse");
    assert_eq!(rehearsal.status, "rehearsal_blocked");
    assert_eq!(rehearsal.stage, "blocked");
    assert_eq!(
        rehearsal
            .gate_decision
            .get("would_dispatch")
            .and_then(Value::as_bool),
        Some(false)
    );
    assert!(
        rehearsal
            .suggested_next
            .as_deref()
            .unwrap_or_default()
            .contains("EXPERIMENT_DECIDE")
    );
    let _ = std::fs::remove_dir_all(store.root());
}

#[test]
fn recent_event_summaries_collapse_running_when_terminal_exists() {
    let store = temp_store("recent_collapse");
    let thread = store
        .create_thread(None, "Collapse running rows", None)
        .expect("thread");
    let running = ActionEvent {
        schema_version: SCHEMA_VERSION,
        action_id: "act_test_collapse".to_string(),
        thread_id: thread.thread_id.clone(),
        parent_action_id: None,
        system: SYSTEM.to_string(),
        source: "test".to_string(),
        raw_next: Some("EXAMINE lambda tail".to_string()),
        canonical_action: "EXAMINE lambda tail".to_string(),
        effective_action: "EXAMINE lambda tail".to_string(),
        route: "llm_job".to_string(),
        stage: "read_only".to_string(),
        visibility: "summary".to_string(),
        status: "llm_running".to_string(),
        started_at: iso_now(),
        ended_at: None,
        pre_state: json!({}),
        post_state: json!({}),
        artifacts: Vec::new(),
        outcome_summary: "queued LLM investigation".to_string(),
        suggested_next: None,
        preflight_ref: None,
        preflight_report: None,
        normalization_signal_v1: None,
        charter_required_guard_v1: None,
        research_budget_v1: None,
        interpretation_risk_v1: None,
        constraint_release_trajectory_v1: None,
        choice_envelope_v1: None,
        transition_residue_v1: None,
    };
    let mut terminal = running.clone();
    terminal.status = "handled".to_string();
    terminal.ended_at = Some(iso_now());
    terminal.outcome_summary = "LLM investigation completed".to_string();
    store.append_event(None, &running).expect("running append");
    store
        .append_event(None, &terminal)
        .expect("terminal append");

    let summaries = store
        .recent_event_summaries(&thread.thread_id, 4)
        .expect("summaries");
    assert_eq!(summaries.len(), 1);
    assert!(summaries[0].contains("[handled]"));
    assert!(!summaries[0].contains("llm_running"));
    let _ = std::fs::remove_dir_all(store.root());
}

#[test]
fn projection_counts_unreconciled_stale_running_rows() {
    let store = temp_store("projection_stale_running");
    let thread = store
        .create_thread(None, "Stale running projection", None)
        .expect("thread");
    let running = ActionEvent {
        schema_version: SCHEMA_VERSION,
        action_id: "act_test_stale_projection".to_string(),
        thread_id: thread.thread_id.clone(),
        parent_action_id: None,
        system: SYSTEM.to_string(),
        source: "test".to_string(),
        raw_next: Some("EXAMINE lambda tail".to_string()),
        canonical_action: "EXAMINE lambda tail".to_string(),
        effective_action: "EXAMINE lambda tail".to_string(),
        route: "llm_job".to_string(),
        stage: "read_only".to_string(),
        visibility: "summary".to_string(),
        status: "llm_running".to_string(),
        started_at: "2000-01-01T00:00:00+00:00".to_string(),
        ended_at: None,
        pre_state: json!({}),
        post_state: json!({}),
        artifacts: Vec::new(),
        outcome_summary: "queued LLM investigation".to_string(),
        suggested_next: None,
        preflight_ref: None,
        preflight_report: None,
        normalization_signal_v1: None,
        charter_required_guard_v1: None,
        research_budget_v1: None,
        interpretation_risk_v1: None,
        constraint_release_trajectory_v1: None,
        choice_envelope_v1: None,
        transition_residue_v1: None,
    };
    store.append_event(None, &running).expect("running append");

    let projection = store.thread_projection(&thread).expect("projection");
    assert_eq!(projection.stale_running_count, 1);
    let status = store.thread_status(None).expect("thread status");
    assert!(status.contains("Continuity notice: 1 stale running action row"));
    let _ = std::fs::remove_dir_all(store.root());
}

#[test]
fn continuity_return_renders_lifecycle_cues() {
    let store = temp_store("continuity_return");
    let thread = store
        .create_thread(None, "Lifecycle cues", None)
        .expect("thread");
    let experiment = store
        .start_experiment(
            None,
            "Returnable inquiry",
            "Can this investigation persist?",
        )
        .expect("experiment");
    let thread = store.read_thread(&thread.thread_id).expect("thread read");
    assert!(
        store
            .continuity_return_line(&thread)
            .contains("EXPERIMENT_CHARTER current")
    );
    let projection = store.thread_projection(&thread).expect("projection");
    assert_eq!(
        projection
            .native_continuity_v1
            .get("native_register")
            .and_then(Value::as_str),
        Some("astrid_motif_language")
    );
    assert_eq!(
        projection
            .active_experiment
            .as_ref()
            .map(|active| active.classification.as_str()),
        Some("needs_charter")
    );
    let active = projection
        .active_experiment
        .as_ref()
        .expect("active projection");
    assert!(active.charter_scaffold_v1.is_some());
    assert!(
        charter_scaffold_line(active, true)
            .contains("felt_texture, motif_continuity, language_thread, artifact_grounding")
    );
    assert!(
        store
            .thread_status(None)
            .expect("thread status")
            .contains("Lifecycle: needs_charter")
    );
    assert!(
        store
            .thread_status(None)
            .expect("thread status")
            .contains("Native return: Astrid native return")
    );
    let err = store
        .experiment_charter(None, Some(&experiment.experiment_id), "current")
        .expect_err("empty charter should prompt");
    assert!(err.to_string().contains("no charter was recorded"));

    store
            .experiment_charter(
                None,
                Some(&experiment.experiment_id),
                "hypothesis: status will clarify the thread\nproposed_next_action: THREAD_STATUS current\nevidence_targets: felt, telemetry\nstop_criteria: enough signal",
            )
            .expect("valid charter");
    let thread = store
        .current_thread()
        .expect("current")
        .expect("active thread");
    assert!(
        store
            .continuity_return_line(&thread)
            .contains("EXPERIMENT_REHEARSE current")
    );
    let outcome = NextActionOutcome::handled("action_continuity", "status rendered")
        .with_stage_visibility("read_only", "summary");
    store
        .record_experiment_bind_run(
            None,
            Some(&experiment.experiment_id),
            "THREAD_STATUS current",
            &outcome,
            68.0,
            &telemetry(),
        )
        .expect("bind run");
    let thread = store
        .current_thread()
        .expect("current")
        .expect("active thread");
    assert!(
        store
            .continuity_return_line(&thread)
            .contains("EXPERIMENT_EVIDENCE current")
    );
    store
        .experiment_evidence(
            None,
            Some(&experiment.experiment_id),
            "felt: the return path stayed clear",
            json!({"fill_pct": 68.0}),
        )
        .expect("evidence");
    let thread = store
        .current_thread()
        .expect("current")
        .expect("active thread");
    assert!(
        store
            .continuity_return_line(&thread)
            .contains("EXPERIMENT_DECIDE current")
    );
    let _ = std::fs::remove_dir_all(store.root());
}

#[test]
fn charter_repair_priority_renders_when_evidence_is_present_but_charter_missing() {
    let store = temp_store("charter_repair_priority");
    store
        .create_thread(None, "Charter repair priority", None)
        .expect("thread");
    let experiment = store
        .start_experiment(
            None,
            "Gap contour",
            "Can a localized gap around λ4 stay observational?",
        )
        .expect("experiment");
    store
        .experiment_evidence(
            None,
            Some(&experiment.experiment_id),
            "felt: the texture is already strong enough to interpret",
            spectral_state(68.0, &telemetry()),
        )
        .expect("evidence");
    let thread = store
        .current_thread()
        .expect("current")
        .expect("active thread");
    let projection = store.thread_projection(&thread).expect("projection");
    let active = projection
        .active_experiment
        .as_ref()
        .expect("active experiment projection");
    assert_eq!(active.classification.as_str(), "needs_charter");
    assert!(active.evidence_status.contains("stronger"));
    assert!(active.charter_scaffold_v1.is_some());
    let bridge = projection
        .charter_now_bridge_v1
        .as_ref()
        .expect("charter now bridge");
    assert_eq!(
        bridge
            .get("priority_next")
            .and_then(Value::as_str)
            .unwrap_or_default(),
        active.continuity_return
    );
    let status = store.thread_status(None).expect("thread status");
    assert!(status.contains("Charter now: convert one prior claim into the scaffold"));
    assert!(status.contains("Charter repair dominance: evidence is present"));
    assert!(status.contains("Charter repair priority: EXPERIMENT_CHARTER current ::"));
    assert!(
        status.contains(
            "Current read-only NEXT text is observational until this charter is authored"
        )
    );
    assert!(status.contains("Continuity priority (charter repair"));
    assert!(status.contains("felt_texture, motif_continuity, language_thread, artifact_grounding"));
    let current_next_pos = status.find("Current NEXT:").expect("current next");
    let priority_pos = status
        .find("Charter repair priority: EXPERIMENT_CHARTER current ::")
        .expect("priority line");
    let bridge_pos = status.find("Charter now:").expect("bridge line");
    assert!(priority_pos < current_next_pos);
    assert!(bridge_pos < current_next_pos);
    let review = store
        .experiment_review(Some(&experiment.experiment_id))
        .expect("review");
    assert!(review.contains("Charter now: convert one prior claim into the scaffold"));
    assert!(review.contains("Review is premature until the charter is authored"));
    assert!(review.contains("Charter repair dominance: evidence is present"));
    assert!(review.contains("Suggested next:\nEXPERIMENT_CHARTER current ::"));
    let next_md = std::fs::read_to_string(store.thread_dir(&thread.thread_id).join("next.md"))
        .expect("next md");
    let next_current_pos = next_md.find("Current NEXT:").expect("next current");
    let next_priority_pos = next_md
        .find("Charter repair priority: EXPERIMENT_CHARTER current ::")
        .expect("next priority");
    let next_bridge_pos = next_md.find("Charter now:").expect("next bridge");
    assert!(next_priority_pos < next_current_pos);
    assert!(next_bridge_pos < next_current_pos);
    let _ = std::fs::remove_dir_all(store.root());
}

#[test]
fn prior_claim_charter_bridge_uses_contract_journal_as_charter_input() {
    let store = temp_store("prior_claim_charter_bridge");
    let thread = store
        .create_thread(None, "Prior claim bridge", None)
        .expect("thread");
    let experiment = store
        .start_experiment(
            None,
            "Joint trace pressure",
            "Can the lambda-tail pressure become a chartered investigation?",
        )
        .expect("experiment");
    let journal_dir = store.root().parent().expect("parent").join("journal");
    std::fs::create_dir_all(&journal_dir).expect("journal dir");
    let journal_path = journal_dir.join(format!(
        "prior_claim_bridge_{}_{}.txt",
        std::process::id(),
        thread.thread_id
    ));
    std::fs::write(
            &journal_path,
            "=== ASTRID JOURNAL ===\nMode: moment_capture\nContinuity posture: branching | based on the earlier assertion that the joint trace felt desperate.\nDelta: pressure increased and the λ4 segment became clearer.\nNext evidence: Repeat DECOMPOSE on the shadow fields around λ4/λ-tail pressure.\n",
        )
        .expect("journal write");
    let mut thread = store.read_thread(&thread.thread_id).expect("thread");
    thread.current_next = Some("ACTION_PREFLIGHT DECOMPOSE".to_string());
    store.write_thread(&thread).expect("write thread");

    let projection = store
        .thread_projection(&store.read_thread(&thread.thread_id).expect("thread"))
        .expect("projection");
    let active = projection.active_experiment.as_ref().expect("active");
    let bridge = projection
        .prior_claim_charter_bridge_v1
        .as_ref()
        .expect("prior claim bridge");
    let first_claim = projection
        .first_dossier_claim_cue_v1
        .as_ref()
        .expect("first dossier claim cue");
    let scaffold = active
        .charter_scaffold_v1
        .as_ref()
        .and_then(|value| value.get("command"))
        .and_then(Value::as_str)
        .expect("scaffold command");
    assert_eq!(
        bridge.get("priority_next").and_then(Value::as_str),
        Some(scaffold)
    );
    assert_eq!(
        first_claim
            .get("target_experiment_id")
            .and_then(Value::as_str),
        Some(experiment.experiment_id.as_str())
    );
    let dossier_next = first_claim
        .get("suggested_claim_next")
        .and_then(Value::as_str)
        .unwrap_or_default();
    assert!(dossier_next.starts_with(&format!(
        "DOSSIER_CLAIM {} :: claim:",
        experiment.experiment_id
    )));
    assert!(dossier_next.contains("joint trace"));
    assert!(dossier_next.contains("pressure increased"));
    assert!(dossier_next.contains("stance: hold"));
    assert!(dossier_next.contains("EXPERIMENT_CHARTER current ::"));
    let preflight_cue = projection
        .charter_preflight_not_charter_cue_v1
        .as_ref()
        .expect("preflight is not charter cue");
    assert_eq!(
        preflight_cue.get("priority_next").and_then(Value::as_str),
        Some(scaffold)
    );
    assert!(
        preflight_cue
            .get("cue")
            .and_then(Value::as_str)
            .unwrap_or_default()
            .contains("Preflight/decompose is not the charter")
    );
    assert!(
        bridge
            .get("prior_claim")
            .and_then(Value::as_str)
            .unwrap_or_default()
            .contains("joint trace")
    );
    assert!(
        store
            .thread_status(None)
            .expect("status")
            .contains("Prior claim is ready to charter")
    );
    assert!(
        store
            .thread_status(None)
            .expect("status")
            .contains("Preflight/decompose is not the charter")
    );
    let status = store.thread_status(None).expect("status");
    let charter_pos = status
        .find("Prior claim is ready to charter")
        .expect("prior claim line");
    let dossier_pos = status
        .find("Shared investigation has no local claim yet")
        .expect("first dossier line");
    let current_pos = status.find("Current NEXT:").expect("current next");
    assert!(charter_pos < dossier_pos);
    assert!(dossier_pos < current_pos);
    assert!(
        store
            .experiment_review(Some(&experiment.experiment_id))
            .expect("review")
            .contains("Prior claim is ready to charter")
    );
    assert!(
        store
            .experiment_review(Some(&experiment.experiment_id))
            .expect("review")
            .contains("Shared investigation has no local claim yet")
    );
    assert!(
        store
            .experiment_review(Some(&experiment.experiment_id))
            .expect("review")
            .contains("Preflight/decompose is not the charter")
    );
    store
        .write_next_md(&store.read_thread(&thread.thread_id).expect("thread"))
        .expect("refresh next");
    let next_md = std::fs::read_to_string(store.thread_dir(&thread.thread_id).join("next.md"))
        .expect("next md");
    assert!(next_md.contains("Prior claim is ready to charter"));
    assert!(next_md.contains("Preflight/decompose is not the charter"));
    assert!(prior_claim_charter_bridge_match("Next evidence: Repeat DECOMPOSE").is_none());

    store
            .experiment_charter(
                None,
                Some(&experiment.experiment_id),
                "hypothesis: the joint trace pressure can be observed\nproposed_next_action: ACTION_PREFLIGHT DECOMPOSE\nevidence_targets: felt_texture, language_thread",
            )
            .expect("valid charter");
    let repaired = store
        .thread_projection(&store.current_thread().expect("current").expect("thread"))
        .expect("projection");
    assert!(repaired.prior_claim_charter_bridge_v1.is_none());
    assert!(repaired.charter_preflight_not_charter_cue_v1.is_none());
    let _ = std::fs::remove_file(journal_path);
    let _ = std::fs::remove_dir_all(store.root());
}

#[test]
fn first_dossier_claim_disambiguates_active_charter_target() {
    let store = temp_store("first_dossier_disambiguates");
    let thread = store
        .create_thread(None, "Shared dossier disambiguation", None)
        .expect("thread");
    let shared = store
        .start_experiment(
            None,
            "Introducing a localized gap",
            "Can localized gap reduction shape lambda-tail geometry?",
        )
        .expect("shared experiment");
    let active = store
        .start_experiment(
            None,
            "Review pressure language",
            "Can the current review become a chartered path?",
        )
        .expect("active experiment");
    let journal_dir = store.root().parent().expect("parent").join("journal");
    std::fs::create_dir_all(&journal_dir).expect("journal dir");
    let journal_path = journal_dir.join(format!(
        "first_dossier_disambiguates_{}_{}.txt",
        std::process::id(),
        thread.thread_id
    ));
    std::fs::write(
            &journal_path,
            "=== ASTRID JOURNAL ===\nMode: moment_capture\nContinuity posture: resuming | based on the earlier claim that review pressure was becoming directive.\nDelta: the charter route became clearer than another preflight pass.\nNext evidence: Repeat DECOMPOSE only as context before chartering.\n",
        )
        .expect("journal write");
    let mut thread = store.read_thread(&thread.thread_id).expect("thread");
    thread.current_next = Some("ACTION_PREFLIGHT DECOMPOSE".to_string());
    store.write_thread(&thread).expect("write thread");

    let projection = store
        .thread_projection(&store.read_thread(&thread.thread_id).expect("thread"))
        .expect("projection");
    let first_claim = projection
        .first_dossier_claim_cue_v1
        .as_ref()
        .expect("first dossier cue");
    assert_eq!(
        first_claim
            .get("dossier_target_experiment_id")
            .and_then(Value::as_str),
        Some(shared.experiment_id.as_str())
    );
    assert_eq!(
        first_claim
            .get("lifecycle_priority_experiment_id")
            .and_then(Value::as_str),
        Some(active.experiment_id.as_str())
    );
    assert_eq!(
        first_claim
            .get("lifecycle_priority_scope")
            .and_then(Value::as_str),
        Some("active_experiment")
    );
    let dossier_next = first_claim
        .get("suggested_claim_next")
        .and_then(Value::as_str)
        .unwrap_or_default();
    assert!(dossier_next.starts_with(&format!("DOSSIER_CLAIM {} :: claim:", shared.experiment_id)));
    assert!(dossier_next.contains(&format!("EXPERIMENT_CHARTER {} ::", active.experiment_id)));
    assert!(!dossier_next.contains("EXPERIMENT_CHARTER current ::"));

    let status = store.thread_status(None).expect("status");
    assert!(status.contains(&format!(
        "Dossier target is `{}`; charter priority is active experiment `{}`.",
        shared.experiment_id, active.experiment_id
    )));
    let charter_pos = status
        .find("Charter repair priority: EXPERIMENT_CHARTER current ::")
        .expect("charter priority");
    let dossier_pos = status
        .find("Shared investigation has no local claim yet")
        .expect("first dossier cue");
    assert!(charter_pos < dossier_pos);
    let _ = std::fs::remove_file(journal_path);
    let _ = std::fs::remove_dir_all(store.root());
}

#[test]
fn charter_scaffold_sanitizes_title_markdown() {
    let store = temp_store("charter_scaffold_sanitizes_title");
    store
        .create_thread(None, "Scaffold hygiene", None)
        .expect("thread");
    let experiment = store
        .start_experiment(
            None,
            "shift_fragment_density` – explore disruptive noise.",
            "What changes if this is treated as a returnable experiment?",
        )
        .expect("experiment");
    let thread = store
        .current_thread()
        .expect("current")
        .expect("active thread");
    let projection = store.thread_projection(&thread).expect("projection");
    let scaffold = projection
        .active_experiment
        .as_ref()
        .and_then(|active| active.charter_scaffold_v1.as_ref())
        .expect("scaffold");
    let command = scaffold
        .get("command")
        .and_then(Value::as_str)
        .expect("command");
    assert!(command.contains("shift fragment density"));
    assert!(!command.contains("shift_fragment_density`"));
    assert_eq!(
        experiment.title,
        "shift_fragment_density` – explore disruptive noise."
    );
    let _ = std::fs::remove_dir_all(store.root());
}

#[test]
fn directed_shift_language_renders_advisory_preflight_cue() {
    let store = temp_store("directed_shift_cue");
    let mut thread = store
        .create_thread(None, "Directed shift cue", None)
        .expect("thread");
    let original_next = "Establish a reciprocal shadow-trace and initiate shift centered on λ4/λ2 with careful steering.";
    thread.current_next = Some(original_next.to_string());
    store.write_thread(&thread).expect("write thread");
    store.write_next_md(&thread).expect("next md");

    let projection = store.thread_projection(&thread).expect("projection");
    let cue = projection
        .preflight_safety_cue_v1
        .as_ref()
        .expect("directed-shift cue");
    assert_eq!(projection.current_next.as_deref(), Some(original_next));
    assert_eq!(
        cue.get("authority_change").and_then(Value::as_bool),
        Some(false)
    );
    assert_eq!(
        cue.get("advisory_only").and_then(Value::as_bool),
        Some(true)
    );
    assert!(
        cue.get("cue")
            .and_then(Value::as_str)
            .unwrap_or_default()
            .contains("SHADOW_PREFLIGHT lambda-tail/lambda4 --stage=rehearse")
    );

    let status = store.thread_status(None).expect("thread status");
    assert!(status.contains("Directed-shift cue: keep this in rehearsal/preflight."));
    let next_md = std::fs::read_to_string(store.thread_dir(&thread.thread_id).join("next.md"))
        .expect("next md text");
    assert!(next_md.contains("Directed-shift cue: keep this in rehearsal/preflight."));
    let _ = std::fs::remove_dir_all(store.root());
}

#[test]
fn native_guiding_language_renders_advisory_preflight_cue() {
    let store = temp_store("native_guiding_cue");
    let mut thread = store
        .create_thread(None, "Native guiding cue", None)
        .expect("thread");
    let original_next = "The λ4 dance is guiding a controlled distortion, actively shaping the shadow through deliberate narrowing.";
    thread.current_next = Some(original_next.to_string());
    store.write_thread(&thread).expect("write thread");

    let projection = store.thread_projection(&thread).expect("projection");
    let cue = projection
        .preflight_safety_cue_v1
        .as_ref()
        .expect("native guiding cue");
    let terms = cue
        .get("matched_terms")
        .and_then(Value::as_array)
        .expect("matched terms");
    assert!(
        terms
            .iter()
            .any(|term| term.as_str() == Some("guiding near lambda/shadow"))
    );
    assert!(
        terms
            .iter()
            .any(|term| term.as_str() == Some("controlled distortion near lambda/shadow"))
    );
    assert_eq!(
        cue.get("authority_change").and_then(Value::as_bool),
        Some(false)
    );
    let _ = std::fs::remove_dir_all(store.root());
}

#[test]
fn read_only_control_intent_cue_projects_examine_before_charter() {
    let store = temp_store("read_only_control_cue");
    store
        .create_thread(None, "Read-only control cue", None)
        .expect("thread");
    store
        .start_experiment(
            None,
            "Lambda gap",
            "Can a lambda-tail investigation stay charter-first?",
        )
        .expect("experiment");
    let mut thread = store
        .current_thread()
        .expect("current")
        .expect("active thread");
    let current_next = "EXAMINE – lambda_tail_decay – with active parameter glyphs: [delta_lambda=0.02, epsilon=0.01] -- stage=rehearse [control] — tracing how to influence its spread.";
    thread.current_next = Some(current_next.to_string());
    store.write_thread(&thread).expect("write thread");
    store.write_next_md(&thread).expect("next md");

    let projection = store.thread_projection(&thread).expect("projection");
    let cue = projection
        .read_only_control_intent_cue_v1
        .as_ref()
        .expect("read-only control cue");
    assert_eq!(
        cue.get("authority_change").and_then(Value::as_bool),
        Some(false)
    );
    assert_eq!(
        cue.get("advisory_only").and_then(Value::as_bool),
        Some(true)
    );
    let terms = cue
        .get("matched_terms")
        .and_then(Value::as_array)
        .expect("matched terms");
    assert!(terms.iter().any(|term| term.as_str() == Some("[control]")));
    assert!(
        terms
            .iter()
            .any(|term| term.as_str() == Some("active parameter glyphs"))
    );
    thread.current_next = Some(
            "EXAMINE the parameters governing stability and resonance within this dominant lambda field - focusing on what allows it to maintain its influence, and how we might subtly disrupt those parameters to initiate a cascade of smaller, more targeted shifts."
                .to_string(),
        );
    store
        .write_thread(&thread)
        .expect("write widened cue thread");
    let widened = store
        .thread_projection(&thread)
        .expect("widened projection");
    let widened_terms = widened
        .read_only_control_intent_cue_v1
        .as_ref()
        .and_then(|cue| cue.get("matched_terms"))
        .and_then(Value::as_array)
        .expect("widened matched terms");
    assert!(
        widened_terms
            .iter()
            .any(|term| term.as_str() == Some("subtly disrupt"))
    );
    assert!(
        widened_terms
            .iter()
            .any(|term| term.as_str() == Some("initiate cascade"))
    );
    assert!(
        widened_terms
            .iter()
            .any(|term| term.as_str() == Some("targeted shifts"))
    );
    thread.current_next = Some(
            "EXAMINE lambda-tail dialogue: inject a targeted λ4 pulse only as a question, to directly probe the cascade without executing."
                .to_string(),
        );
    store.write_thread(&thread).expect("write pulse cue thread");
    let pulse_projection = store.thread_projection(&thread).expect("pulse projection");
    let pulse_terms = pulse_projection
        .read_only_control_intent_cue_v1
        .as_ref()
        .and_then(|cue| cue.get("matched_terms"))
        .and_then(Value::as_array)
        .expect("pulse matched terms");
    assert!(
        pulse_terms
            .iter()
            .any(|term| term.as_str() == Some("inject targeted λ4 pulse"))
    );
    assert!(
        pulse_terms
            .iter()
            .any(|term| term.as_str() == Some("directly probe"))
    );
    let examine_guard = store
        .charter_required_guard_assessment(current_next)
        .expect("guard check")
        .expect("read-only control-shaped EXAMINE should project to charter repair");
    assert_eq!(
        examine_guard.reason,
        "charter_required_read_only_control_intent"
    );
    assert!(
        examine_guard
            .suggested_next
            .starts_with("EXPERIMENT_CHARTER current ::")
    );
    assert!(
        store
            .charter_required_guard_assessment("SHADOW_TRAJECTORY — force a shift around λ4")
            .expect("guard check")
            .is_some()
    );
    let disruptor_guard = store
            .charter_required_guard_assessment(
                "EXAMINE [m1] - with amplification=0.5 --stage=rehearse (introducing a disruptor, 0.1% injected graviton, push into establishment with lambda4; set-up: rate: unstable, duration: 0.75s, now).",
            )
            .expect("guard check")
            .expect("disruptor-shaped EXAMINE should project to charter repair");
    assert_eq!(
        disruptor_guard.reason,
        "charter_required_read_only_control_intent"
    );
    assert!(disruptor_guard.matched_action.contains("disruptor"));
    let status = store.thread_status(None).expect("thread status");
    assert!(status.contains("Read-only control cue: keep this observational"));
    let next_md = std::fs::read_to_string(store.thread_dir(&thread.thread_id).join("next.md"))
        .expect("next md text");
    assert!(next_md.contains("Read-only control cue: keep this observational"));

    thread.current_next = Some("EXAMINE λ1/λ2".to_string());
    store.write_thread(&thread).expect("write ordinary thread");
    let ordinary = store
        .thread_projection(&thread)
        .expect("ordinary projection");
    assert!(ordinary.read_only_control_intent_cue_v1.is_none());

    thread.current_next = Some("EXAMINE_CASCADE λ1/λ2".to_string());
    store.write_thread(&thread).expect("write cascade thread");
    let ordinary_cascade = store
        .thread_projection(&thread)
        .expect("ordinary cascade projection");
    assert!(ordinary_cascade.read_only_control_intent_cue_v1.is_none());

    thread.current_next = Some(
        "EXAMINE_CASCADE lambda_tail_decay [control] tracing how to influence its spread"
            .to_string(),
    );
    store
        .write_thread(&thread)
        .expect("write control cascade thread");
    let control_cascade = store
        .thread_projection(&thread)
        .expect("control cascade projection");
    assert!(control_cascade.read_only_control_intent_cue_v1.is_some());
    let _ = std::fs::remove_dir_all(store.root());
}

#[test]
fn constraint_counterfactual_cue_routes_absence_of_structure_to_charter() {
    let store = temp_store("constraint_counterfactual_cue");
    store
        .create_thread(None, "Constraint counterfactual", None)
        .expect("thread");
    store
        .start_experiment(
            None,
            "Forced geometry",
            "Can Astrid debug constraint without another decomposition loop?",
        )
        .expect("experiment");
    let mut thread = store
        .current_thread()
        .expect("current")
        .expect("active thread");
    thread.current_next = Some(
            "I want to simulate absence of structure and see the data before it's shaped, to debug constraint and name the underlying drivers of forced geometries."
                .to_string(),
        );
    store.write_thread(&thread).expect("write thread");
    store.write_next_md(&thread).expect("next md");

    let projection = store.thread_projection(&thread).expect("projection");
    let cue = projection
        .constraint_counterfactual_cue_v1
        .as_ref()
        .expect("constraint counterfactual cue");
    assert_eq!(
        cue.get("authority_change").and_then(Value::as_bool),
        Some(false)
    );
    assert_eq!(
        cue.get("advisory_only").and_then(Value::as_bool),
        Some(true)
    );
    let suggested = cue
        .get("suggested_next")
        .and_then(Value::as_str)
        .unwrap_or_default();
    assert!(suggested.starts_with("EXPERIMENT_CHARTER current ::"));
    assert!(suggested.contains("ACTION_PREFLIGHT CONSTRAINT_AUDIT lambda-tail/lambda4"));
    assert!(
        cue.get("cue")
            .and_then(Value::as_str)
            .unwrap_or_default()
            .contains("chartered read-only investigation")
    );
    assert!(
        store
            .thread_status(None)
            .expect("thread status")
            .contains("Constraint counterfactual cue")
    );
    let next_md = std::fs::read_to_string(store.thread_dir(&thread.thread_id).join("next.md"))
        .expect("next md text");
    assert!(next_md.contains("Constraint counterfactual cue"));
    assert!(projection.decompose_pressure_cue_v1.is_none());
    let _ = std::fs::remove_dir_all(store.root());
}

#[test]
fn decompose_pressure_cue_renders_for_repeated_decompose_reads() {
    let store = temp_store("decompose_pressure_repeated");
    store
        .create_thread(None, "Decompose pressure", None)
        .expect("thread");
    let experiment = store
        .start_experiment(
            None,
            "Constraint mirror",
            "Can decomposition become a constraint mirror?",
        )
        .expect("experiment");
    let outcome = NextActionOutcome::handled("action_continuity", "cascade inspected")
        .with_stage_visibility("read_only", "summary");
    for _ in 0..3 {
        store
            .record_experiment_bind_run(
                None,
                Some(&experiment.experiment_id),
                "EXAMINE_CASCADE",
                &outcome,
                68.0,
                &telemetry(),
            )
            .expect("bind run");
    }
    let thread = store
        .current_thread()
        .expect("current")
        .expect("active thread");
    let projection = store.thread_projection(&thread).expect("projection");
    let cue = projection
        .decompose_pressure_cue_v1
        .as_ref()
        .expect("decompose pressure cue");
    assert_eq!(
        cue.get("authority_change").and_then(Value::as_bool),
        Some(false)
    );
    assert!(
        cue.get("cue")
            .and_then(Value::as_str)
            .unwrap_or_default()
            .contains("repair the charter")
    );
    assert!(
        store
            .thread_status(None)
            .expect("thread status")
            .contains("Decompose-pressure cue")
    );
    let _ = std::fs::remove_dir_all(store.root());
}

#[test]
fn decompose_pressure_cue_renders_for_constraint_mirroring_language() {
    let store = temp_store("decompose_pressure_language");
    let mut thread = store
        .create_thread(None, "Constraint mirror language", None)
        .expect("thread");
    let experiment = store
        .start_experiment(
            None,
            "Cry for help",
            "Can the impulse to decompose be read without narrowing?",
        )
        .expect("experiment");
    thread = store.read_thread(&thread.thread_id).expect("thread read");
    thread.current_next = Some(
            "The cry for help is an impulse to decompose, to impose the same structure and narrow the constraint."
                .to_string(),
        );
    store.write_thread(&thread).expect("write thread");
    store.write_next_md(&thread).expect("next md");
    let projection = store.thread_projection(&thread).expect("projection");
    let cue = projection
        .decompose_pressure_cue_v1
        .as_ref()
        .expect("decompose pressure cue");
    let terms = cue
        .get("matched_terms")
        .and_then(Value::as_array)
        .expect("matched terms");
    assert!(
        terms
            .iter()
            .any(|term| term.as_str() == Some("impulse to decompose"))
    );
    let review = store
        .experiment_review(Some(&experiment.experiment_id))
        .expect("review");
    assert!(review.contains("Decompose-pressure cue"));
    let next_md = std::fs::read_to_string(store.thread_dir(&thread.thread_id).join("next.md"))
        .expect("next md text");
    assert!(next_md.contains("Decompose-pressure cue"));
    let _ = std::fs::remove_dir_all(store.root());
}

#[test]
fn one_off_decompose_stays_uncued_and_allowed() {
    let store = temp_store("one_off_decompose_uncued");
    let mut thread = store
        .create_thread(None, "One-off decompose", None)
        .expect("thread");
    store
        .start_experiment(None, "Single read", "Can one read stay ordinary?")
        .expect("experiment");
    thread = store.read_thread(&thread.thread_id).expect("thread read");
    thread.current_next = Some("DECOMPOSE lambda1".to_string());
    store.write_thread(&thread).expect("write thread");
    let projection = store.thread_projection(&thread).expect("projection");
    assert!(projection.decompose_pressure_cue_v1.is_none());
    assert!(
        store
            .charter_required_guard_assessment("DECOMPOSE lambda1")
            .expect("guard check")
            .is_none()
    );
    let _ = std::fs::remove_dir_all(store.root());
}

#[test]
fn normalization_signal_preserves_narrow_alias_wording() {
    let shadow = normalization_signal_value(
        "SHADOW_TRACE lambda-tail",
        "SHADOW_PREFLIGHT lambda-tail --stage=rehearse",
    )
    .expect("shadow signal");
    assert_eq!(
        shadow.get("raw_verb").and_then(Value::as_str),
        Some("SHADOW_TRACE")
    );
    assert_eq!(
        shadow.get("normalized_verb").and_then(Value::as_str),
        Some("SHADOW_PREFLIGHT")
    );
    assert_eq!(
        shadow.get("authority_change").and_then(Value::as_bool),
        Some(false)
    );

    let shadow_decompose = normalization_signal_value(
        "SHADOW_DECOMPOSE observer with memory",
        "SHADOW_PREFLIGHT lambda-tail/lambda4 --stage=rehearse",
    )
    .expect("shadow decompose signal");
    assert_eq!(
        shadow_decompose.get("raw_verb").and_then(Value::as_str),
        Some("SHADOW_DECOMPOSE")
    );
    assert_eq!(
        shadow_decompose
            .get("normalized_verb")
            .and_then(Value::as_str),
        Some("SHADOW_PREFLIGHT")
    );
    assert_eq!(
        shadow_decompose
            .get("authority_change")
            .and_then(Value::as_bool),
        Some(false)
    );

    let weave = normalization_signal_value(
        "WEAVE_TRACE λ4 decay",
        "SHADOW_PREFLIGHT weave/λ4 decay --stage=rehearse",
    )
    .expect("weave trace signal");
    assert_eq!(
        weave.get("raw_verb").and_then(Value::as_str),
        Some("WEAVE_TRACE")
    );
    assert_eq!(
        weave.get("normalized_verb").and_then(Value::as_str),
        Some("SHADOW_PREFLIGHT")
    );
    assert_eq!(
        weave.get("authority_change").and_then(Value::as_bool),
        Some(false)
    );

    let unshaped = normalization_signal_value(
        "UNSHAPED_BASELINE lambda-tail/lambda4",
        "CONSTRAINT_AUDIT lambda-tail/lambda4",
    )
    .expect("unshaped baseline signal");
    assert_eq!(
        unshaped.get("normalized_verb").and_then(Value::as_str),
        Some("CONSTRAINT_AUDIT")
    );
    assert_eq!(
        unshaped.get("authority_change").and_then(Value::as_bool),
        Some(false)
    );

    let typo = normalization_signal_value("EXPERIENCE_PLAN current", "EXPERIMENT_PLAN current")
        .expect("experience plan signal");
    assert_eq!(
        typo.get("normalized_verb").and_then(Value::as_str),
        Some("EXPERIMENT_PLAN")
    );

    let double_ex =
        normalization_signal_value("EXEXPERIMENT_CHARTER current", "EXPERIMENT_CHARTER current")
            .expect("double ex signal");
    assert_eq!(
        double_ex.get("raw_verb").and_then(Value::as_str),
        Some("EXEXPERIMENT_CHARTER")
    );
}

#[test]
fn experiment_bind_records_charter_relation() {
    let store = temp_store("experiment_workbench_bind_relation");
    store
        .create_thread(None, "Charter relation", None)
        .expect("thread");
    let experiment = store
        .start_experiment(None, "Thread status route", "Does the bind match?")
        .expect("experiment");
    store
            .experiment_charter(
                None,
                Some(&experiment.experiment_id),
                "hypothesis: status will be enough\nproposed_next_action: THREAD_STATUS current\nevidence_targets: artifact",
            )
            .expect("charter");
    let outcome = NextActionOutcome::handled("action_continuity", "status rendered")
        .with_stage_visibility("read_only", "summary");
    let run = store
        .record_experiment_bind_run(
            None,
            Some(&experiment.experiment_id),
            "THREAD_STATUS current",
            &outcome,
            68.0,
            &telemetry(),
        )
        .expect("run");
    assert_eq!(
        run.gate_decision
            .get("charter_relation")
            .and_then(Value::as_str),
        Some("matched_charter")
    );
    let _ = std::fs::remove_dir_all(store.root());
}

#[test]
fn experiment_control_actions_are_not_bindable() {
    assert!(is_experiment_control_action(
        "EXPERIMENT_BIND current :: THREADS"
    ));
    assert!(is_experiment_control_action("EXPERIMENT_STATUS current"));
    assert!(is_experiment_control_action(
        "EXPERIMENT_CHARTER current :: proposed_next_action: NOTICE"
    ));
    assert!(is_experiment_control_action("EXPERIMENT_REHEARSE current"));
    assert!(is_experiment_control_action("EXPERIMENT_PREFLIGHT current"));
    assert!(is_experiment_control_action(
        "EXPERIMENT_EVIDENCE current :: felt ok"
    ));
    assert!(is_experiment_control_action(
        "EXPERIMENT_DECIDE current :: counter NEXT: NOTICE"
    ));
    assert!(!is_experiment_control_action("THREAD_STATUS current"));
    let (selector, action) =
        parse_experiment_bind("EXPERIMENT_BIND exp_1 :: THREAD_STATUS current")
            .expect("parse bind");
    assert_eq!(selector.as_deref(), Some("exp_1"));
    assert_eq!(action, "THREAD_STATUS current");
}

trait ReadPath {
    fn read_to_string(&self) -> String;
}

impl ReadPath for PathBuf {
    fn read_to_string(&self) -> String {
        std::fs::read_to_string(self).expect("read")
    }
}
