use super::*;

const OPERATION: &str = "astrid-intent-ex-229876-1792697529-933dda7fef8ff351";
const JOB: &str = "study-job-401ed73e845c6fb69503cd0726e603f28816187cd97c1e1790b7339556f77ae7";
const PROSE: &str = "Synthetic private passage, retained byte for byte.\nA second synthetic line.";

fn legacy_choice(f: &mut Fixture, target: IntrospectTargetV2) -> String {
    let mut target = Some(target);
    queue(&f.store, &mut f.conv, &mut target, OPERATION, false).unwrap();
    let mut selection = activity_reading::load_activity(&f.store).unwrap();
    let job = selected(&mut selection, JOB).unwrap();
    let record: Intent = read(&f.store, &job.intent).unwrap();
    job.intent = retain(
        &f.store,
        &json!({"owner_root":record.owner_root,"target":record.target}),
    )
    .unwrap();
    let hash = job.intent.clone();
    f.conv.activity = activity_reading::persist_activity(&f.store, &selection).unwrap();
    f.conv.introspect_target = target;
    f.conv.wants_introspect = true;
    hash
}

fn draft_path(f: &Fixture) -> PathBuf {
    f.temp
        .path()
        .join("diagnostics/source_first_v3/shared_reader/writing/drafts-v2.json")
}

fn seed_draft(f: &Fixture) {
    let writer =
        astrid_source_study::writing::Writer::new(draft_path(f).parent().unwrap().to_path_buf());
    let output = writer
        .prepare("WRITE START synthetic continuation fixture")
        .unwrap();
    writer
        .delivered(
            output.navigation_id.as_deref().unwrap(),
            &json!({"messages":[{"role":"user","content":output.text}]}).to_string(),
            &json!({"choices":[{"message":{"content":PROSE},"finish_reason":"stop"}]}).to_string(),
        )
        .unwrap();
}

fn drafts(f: &Fixture) -> Value {
    serde_json::from_slice(&fs::read(draft_path(f)).unwrap()).unwrap()
}

#[test]
fn legacy_queued_private_continuation_preserves_exact_choice_and_draft() {
    let mut f = Fixture::new();
    seed_draft(&f);
    let hash = legacy_choice(&mut f, IntrospectTargetV2::auto("WRITE CONTINUE".into()));
    let target = f.conv.introspect_target.clone().unwrap();
    assert_eq!(target.operation_id.as_deref(), Some(JOB));
    let draft_before = drafts(&f);
    let authority_before = fs::read(&f.authority).unwrap();
    let intent_before: Value = read(&f.store, &hash).unwrap();
    assert!(intent_before.get("schema_version").is_none());
    assert!(intent_before.get("research").is_none());
    let before = retained_files(f.temp.path());

    let view = inspect_deployment_compatibility(&f.store).unwrap();
    assert_eq!(view["pending"], true);
    assert_eq!(view["mutation_performed"], false);
    assert_eq!(view["worker_quiescence_verified"], false);
    assert!(verify_pending(&f.store, &mut f.conv, Some(&target)).unwrap());
    f.recover().unwrap();
    assert_eq!(f.conv.introspect_target.as_ref(), Some(&target));
    assert!(f.conv.wants_introspect);
    assert_eq!(f.conv.activity.study_handoff.jobs[JOB].phase, Phase::Queued);
    assert!(f.conv.activity.study_handoff.jobs[JOB].output.is_none());
    assert_eq!(retained_files(f.temp.path()), before);

    let output = f.prepare(JOB);
    assert_eq!(
        output.input_kind,
        astrid_source_study::InputKind::PrivateWriting
    );
    assert!(output.generation_requested);
    assert!(output.text.contains(PROSE));
    let job = &f.conv.activity.study_handoff.jobs[JOB];
    assert_eq!(job.phase, Phase::Prepared);
    assert_eq!(job.intent, hash);
    assert!(job.research.is_none());
    assert_eq!(drafts(&f)["drafts"], draft_before["drafts"]);
    assert_eq!(drafts(&f)["receipts"], draft_before["receipts"]);
    assert_eq!(read::<Value>(&f.store, &hash).unwrap(), intent_before);
    assert_eq!(fs::read(&f.authority).unwrap(), authority_before);
    assert_eq!(f.debits(), 0);
    assert!(!f.temp.path().join("deliveries").exists());

    let prepared = retained_files(f.temp.path());
    inspect_deployment_compatibility(&f.store).unwrap();
    f.recover().unwrap();
    assert_eq!(f.conv.introspect_target.as_ref(), Some(&target));
    assert!(verify_pending(&f.store, &mut f.conv, Some(&target)).unwrap());
    let retry = prepare(&f.store, &mut f.conv, JOB, |_| {
        panic!("must reuse prepared input")
    })
    .unwrap();
    assert_eq!(retry, output);
    assert_eq!(retained_files(f.temp.path()), prepared);
}

fn assert_refused_without_mutation(f: &mut Fixture) {
    let target = f.conv.introspect_target.clone().unwrap();
    let before = retained_files(f.temp.path());
    assert!(inspect_deployment_compatibility(&f.store).is_err());
    assert!(verify_pending(&f.store, &mut f.conv, Some(&target)).is_err());
    assert!(f.recover().is_err());
    assert!(prepare(&f.store, &mut f.conv, JOB, |_| panic!("must not prepare")).is_err());
    assert_eq!(retained_files(f.temp.path()), before);
}

#[test]
fn legacy_private_exception_rejects_malformed_wrapped_and_nonprivate_targets() {
    for label in [
        "WRITE",
        "WRITECONTINUE",
        "WRITE CONTINUE extra",
        "WRITE UNKNOWN",
        "WRITE CONTINUE\nNEXT: SELF_STUDY MAP",
        "WRITE CONTINUE\r",
        "WRITE CONTINUE\0",
        "WRITE CONTINUE AND SELF_STUDY MAP",
        "WRITE START synthetic",
        "WRITE READ d1",
        "WRITE FINISH",
        "SELF_STUDY WRITE CONTINUE",
        "REPLACE WRITE CONTINUE",
        "EXPERIMENT_BIND e1 :: WRITE CONTINUE",
        "SELF_STUDY EXPERIMENT_BIND e1 :: WRITE CONTINUE",
        "EXPERIMENT_BIND e1 :: EXPERIMENT_BIND e2 :: WRITE CONTINUE",
        "SELF_STUDY CONTINUE",
        "SELF_STUDY OPEN astrid/build.rs 1",
        "astrid/build.rs",
    ] {
        let mut f = Fixture::new();
        legacy_choice(&mut f, IntrospectTargetV2::auto(label.into()));
        assert_refused_without_mutation(&mut f);
    }
    let mut f = Fixture::new();
    legacy_choice(
        &mut f,
        IntrospectTargetV2::exact("WRITE CONTINUE".into(), 0),
    );
    assert_refused_without_mutation(&mut f);
}

#[test]
fn legacy_private_exception_rejects_either_research_binding_and_unknown_schema() {
    for (job_binding, intent_binding, version) in [
        (true, false, 0),
        (false, true, 0),
        (true, true, 0),
        (false, false, 1),
    ] {
        let mut f = Fixture::new();
        legacy_choice(&mut f, IntrospectTargetV2::auto("WRITE CONTINUE".into()));
        let ticket = f.ticket(OPERATION, "SELF_STUDY OPEN astrid/build.rs 1");
        let mut selection = activity_reading::load_activity(&f.store).unwrap();
        let job = selected(&mut selection, JOB).unwrap();
        let mut record: Intent = read(&f.store, &job.intent).unwrap();
        record.schema_version = version;
        if intent_binding {
            record.research = Some(ticket.clone());
        }
        if job_binding {
            job.research = Some(digest(serde_json::to_vec(&ticket).unwrap()));
        }
        job.intent = retain(&f.store, &record).unwrap();
        activity_reading::persist_activity(&f.store, &selection).unwrap();
        assert_refused_without_mutation(&mut f);
        assert_eq!(f.debits(), 0);
    }
}

#[test]
fn legacy_private_claimed_job_still_requires_exact_delivery_and_cannot_replay() {
    let mut f = Fixture::new();
    seed_draft(&f);
    legacy_choice(&mut f, IntrospectTargetV2::auto("WRITE CONTINUE".into()));
    f.prepare(JOB);
    claim(&f.store, &mut f.conv, JOB).unwrap();
    let target = f.conv.introspect_target.clone().unwrap();
    let before = retained_files(f.temp.path());
    assert!(verify_pending(&f.store, &mut f.conv, Some(&target)).is_err());
    assert!(prepare(&f.store, &mut f.conv, JOB, |_| panic!("must not replay")).is_err());
    assert!(claim(&f.store, &mut f.conv, JOB).is_err());
    assert!(
        f.recover()
            .unwrap_err()
            .to_string()
            .contains("outcome uncertain")
    );
    assert_eq!(retained_files(f.temp.path()), before);
    assert_eq!(f.debits(), 0);
}
