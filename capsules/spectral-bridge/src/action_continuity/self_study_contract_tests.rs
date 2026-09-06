use super::*;
use sha2::{Digest, Sha256};

fn fixture() -> (
    tempfile::TempDir,
    ActionContinuityStore,
    ResearchThread,
    ExperimentRecord,
    String,
) {
    let dir = tempfile::tempdir().unwrap();
    let store = ActionContinuityStore::new(dir.path().to_path_buf());
    let thread = store
        .create_thread(None, "Synthetic retrieval inquiry", None)
        .unwrap();
    let experiment = store
        .start_experiment(None, "Reading position", "Can I return to the evidence?")
        .unwrap();
    store.dossier_claim_command(None, &format!("{} :: claim: The first window contains the answer; basis: Synthetic fixture hypothesis", experiment.experiment_id)).unwrap();
    let rows = store.strict_dossier_records(&thread.thread_id).unwrap();
    let claim = rows.last().unwrap()["record_id"]
        .as_str()
        .unwrap()
        .to_string();
    (dir, store, thread, experiment, claim)
}

fn prediction() -> SelfStudyPrediction {
    SelfStudyPrediction {
        recipe: SelfStudyRecipe::SyntheticRetrieval,
        assets_sha256: "a".repeat(64),
        expected_above: 0.5,
        authored_expectation: "The selected window contains the marker".into(),
    }
}

#[test]
fn synthetic_learning_workflow_preserves_counterevidence_and_reading_position() {
    let (dir, store, thread, experiment, claim) = fixture();
    let fixture_path = dir.path().join("synthetic_source.txt");
    fs::write(
        &fixture_path,
        "header\ncontext\nmore context\nANSWER: violet\nend\n",
    )
    .unwrap();
    let bytes = fs::read(&fixture_path).unwrap();
    let source_sha256 = format!("{:x}", Sha256::digest(&bytes));
    let spec = SelfStudyPrediction {
        assets_sha256: self_study_digest(
            &json!({"source_sha256":source_sha256, "offset":0, "limit":2, "marker":"ANSWER:"}),
        )
        .unwrap(),
        ..prediction()
    };
    let p1 = store
        .self_study_predict(&experiment.experiment_id, &claim, &spec)
        .unwrap();
    let before = fs::read(store.dossier_path(&thread.thread_id)).unwrap();
    let source = fs::read_to_string(&fixture_path).unwrap();
    let first_window = source.lines().take(2).collect::<Vec<_>>().join("\n");
    let score = if first_window.contains("ANSWER:") {
        1.
    } else {
        0.
    };
    let e1 = store
        .self_study_evaluate(
            &experiment.experiment_id,
            &p1,
            SelfStudyRecipe::SyntheticRetrieval,
            &spec.assets_sha256,
            Some(score),
            "synthetic_source.txt:lines=1..2",
        )
        .unwrap();
    let revision = store
        .self_study_revise(
            &experiment.experiment_id,
            &claim,
            &e1,
            "supersedes",
            "The answer requires a later reading window",
            "The tested first window did not contain the marker",
        )
        .unwrap();
    assert!(
        fs::read(store.dossier_path(&thread.thread_id))
            .unwrap()
            .starts_with(&before)
    );
    store
        .continuity_session_start_command(
            "current :: title: Resume source reading; focus: find the marker",
        )
        .unwrap();
    store.continuity_session_capture_command(&format!("latest :: summary: First window tested, no marker; source_refs: synthetic_source.txt@sha256:{source_sha256}; artifact_refs: {e1}; question: Does the later window contain it?; next: INTROSPECT synthetic_source 3")).unwrap();
    store
        .continuity_session_finalize_command(
            "latest :: outcome: park; return_cue: my explicit return",
        )
        .unwrap();
    assert!(store.continuity_session_line(&thread, None).is_empty());
    assert!(
        store
            .continuity_session_resume_command("latest")
            .unwrap()
            .contains("INTROSPECT synthetic_source 3")
    );
    let later_spec = SelfStudyPrediction {
        assets_sha256: self_study_digest(
            &json!({"source_sha256":source_sha256, "offset":3, "limit":2, "marker":"ANSWER:"}),
        )
        .unwrap(),
        ..spec.clone()
    };
    let p2 = store
        .self_study_predict(&experiment.experiment_id, &revision, &later_spec)
        .unwrap();
    let later_window = source
        .lines()
        .skip(3)
        .take(2)
        .collect::<Vec<_>>()
        .join("\n");
    let score = if later_window.contains("ANSWER:") {
        1.
    } else {
        0.
    };
    store
        .self_study_evaluate(
            &experiment.experiment_id,
            &p2,
            SelfStudyRecipe::SyntheticRetrieval,
            &later_spec.assets_sha256,
            Some(score),
            "synthetic_source.txt:lines=4..5",
        )
        .unwrap();
    let rows = store.strict_dossier_records(&thread.thread_id).unwrap();
    let outcomes: Vec<_> = rows
        .iter()
        .filter(|r| r["record_type"] == "study_evaluation")
        .map(|r| r["outcome"].as_str().unwrap())
        .collect();
    assert_eq!(outcomes, ["not_matched", "matched"]);
    assert_eq!(rows.last().unwrap()["authority_change"], false);
    assert_eq!(
        rows.iter().find(|r| r["record_id"] == claim).unwrap()["claim"],
        "The first window contains the answer"
    );
}

#[test]
fn prediction_contract_rejects_wrong_owner_assets_recipe_and_duplicate_outcome() {
    let (_dir, store, thread, experiment, claim) = fixture();
    let spec = prediction();
    assert!(
        store
            .self_study_predict(&experiment.experiment_id, "unknown", &spec)
            .is_err()
    );
    let p = store
        .self_study_predict(&experiment.experiment_id, &claim, &spec)
        .unwrap();
    let before = fs::read(store.dossier_path(&thread.thread_id)).unwrap();
    for (recipe, digest, value) in [
        (
            SelfStudyRecipe::SyntheticRetrieval,
            "b".repeat(64),
            Some(1.),
        ),
        (
            SelfStudyRecipe::SyntheticReservoirHistory,
            spec.assets_sha256.clone(),
            Some(1.),
        ),
        (
            SelfStudyRecipe::SyntheticRetrieval,
            spec.assets_sha256.clone(),
            Some(f64::NAN),
        ),
    ] {
        assert!(
            store
                .self_study_evaluate(
                    &experiment.experiment_id,
                    &p,
                    recipe,
                    &digest,
                    value,
                    "fixture"
                )
                .is_err()
        );
    }
    assert_eq!(
        fs::read(store.dossier_path(&thread.thread_id)).unwrap(),
        before
    );
    let e = store
        .self_study_evaluate(
            &experiment.experiment_id,
            &p,
            spec.recipe.clone(),
            &spec.assets_sha256,
            None,
            "missing fixture",
        )
        .unwrap();
    assert_eq!(
        store
            .self_study_target(&experiment.experiment_id, &e)
            .unwrap()
            .1["outcome"],
        "insufficient"
    );
    assert!(
        store
            .self_study_evaluate(
                &experiment.experiment_id,
                &p,
                spec.recipe.clone(),
                &spec.assets_sha256,
                Some(1.),
                "fixture"
            )
            .is_err()
    );
    let other = store.start_experiment(None, "Other", "Unrelated").unwrap();
    assert!(
        store
            .self_study_predict(&other.experiment_id, &claim, &spec)
            .is_err()
    );
    assert!(
        store
            .self_study_revise(
                &experiment.experiment_id,
                &claim,
                &p,
                "supersedes",
                "new",
                "reason"
            )
            .is_err()
    );
}

#[test]
fn contract_does_not_invent_an_inquiry_or_bypass_a_writer() {
    let dir = tempfile::tempdir().unwrap();
    let store = ActionContinuityStore::new(dir.path().to_path_buf());
    assert!(
        store
            .self_study_predict("unknown", "unknown", &prediction())
            .is_err()
    );
    assert!(store.current_thread().unwrap().is_none());
    let (_dir, store, _thread, experiment, claim) = fixture();
    let lock = store.lock_self_study().unwrap();
    assert!(
        store
            .self_study_predict(&experiment.experiment_id, &claim, &prediction())
            .is_err()
    );
    drop(lock);
    assert!(
        store
            .self_study_predict("current", &claim, &prediction())
            .is_err()
    );
    assert!(
        store
            .self_study_predict(&experiment.experiment_id, &claim, &prediction())
            .is_ok()
    );
}

#[test]
fn corrupted_history_is_not_skipped_to_record_a_success() {
    let (_dir, store, thread, experiment, claim) = fixture();
    let p = store
        .self_study_predict(&experiment.experiment_id, &claim, &prediction())
        .unwrap();
    let path = store.dossier_path(&thread.thread_id);
    writeln!(
        OpenOptions::new().append(true).open(&path).unwrap(),
        "broken json"
    )
    .unwrap();
    let before = fs::read(&path).unwrap();
    assert!(
        store
            .self_study_evaluate(
                &experiment.experiment_id,
                &p,
                SelfStudyRecipe::SyntheticRetrieval,
                &prediction().assets_sha256,
                Some(1.),
                "fixture"
            )
            .is_err()
    );
    assert_eq!(fs::read(path).unwrap(), before);
}
