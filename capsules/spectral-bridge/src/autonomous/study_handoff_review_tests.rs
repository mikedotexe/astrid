use super::*;

struct Fixture {
    temp: tempfile::TempDir,
    store: ActionContinuityStore,
    reader: astrid_source_study::Reader,
    conv: ConversationState,
    output: astrid_source_study::StudyOutput,
    review: Review,
}

impl Fixture {
    fn new() -> Self {
        let temp = tempfile::tempdir().unwrap();
        let root = temp.path().canonicalize().unwrap();
        fs::write(root.join("Cargo.toml"), "[workspace]\nmembers=[]\n").unwrap();
        let store = ActionContinuityStore::new(root.join("action_threads"));
        let reader = astrid_source_study::Reader::new(
            astrid_source_study::Catalog::new(BTreeMap::from([("astrid".into(), root.clone())]))
                .unwrap(),
            root.join("diagnostics/source_first_v3/shared_reader"),
        )
        .with_runtime_workspace(root, "astrid");
        let mut conv = ConversationState::new(Vec::new(), None);
        let mut target = Some(IntrospectTargetV2::auto(
            "WRITE START synthetic private title".into(),
        ));
        queue(&store, &mut conv, &mut target, "synthetic-choice", false).unwrap();
        let id = target.as_ref().unwrap().operation_id.clone().unwrap();
        conv.introspect_target = target.clone();
        let output = prepare(&store, &mut conv, &id, || {
            super::super::super::prepare_shared_study_target(&reader, target)
        })
        .unwrap();
        claim(&store, &mut conv, &id).unwrap();
        let selection = activity_reading::load_activity(&store).unwrap();
        let job = &selection.study_handoff.jobs[&id];
        let review = Review {
            schema: SCHEMA.into(),
            operation_id: "review-synthetic-1".into(),
            actor: "test-operator".into(),
            acknowledgement: ACK.into(),
            job_id: id,
            expected_selection_revision: selection.selection_revision,
            expected_intent_sha256: job.intent.clone(),
            expected_output_sha256: job.output.clone().unwrap(),
            evidence_sha256: digest(b"synthetic operator evidence"),
            provider_quiescent_observed: true,
        };
        Self {
            temp,
            store,
            reader,
            conv,
            output,
            review,
        }
    }
    fn deliveries(&self) -> PathBuf {
        self.temp.path().join("deliveries")
    }
    fn stage(&self) -> Result<serde_json::Value> {
        stage_review(&self.store, &self.deliveries(), &self.review)
    }
    fn review_path(&self) -> PathBuf {
        request_path(
            &self.store,
            &self.review.job_id,
            self.review.expected_selection_revision,
        )
        .unwrap()
    }
    fn reconcile(&mut self) -> Result<()> {
        let deliveries = self.deliveries();
        reconcile(&self.store, &mut self.conv, &self.reader, &deliveries)
    }
}

#[test]
fn review_is_permission_only_then_retires_without_delivery_retry_or_next() {
    let mut f = Fixture::new();
    let reader_state = f.reader.preparation_revision().unwrap();
    let old = fs::read(f.store.root().join("activity_runtime_v3.json")).unwrap();
    assert_eq!(f.stage().unwrap()["status"], "staged_not_applied");
    assert_eq!(
        old,
        fs::read(f.store.root().join("activity_runtime_v3.json")).unwrap()
    );
    f.stage().unwrap();
    f.reconcile().unwrap();
    let job = &f.conv.activity.study_handoff.jobs[&f.review.job_id];
    assert_eq!(job.phase, Phase::RetiredUnresolved);
    assert_eq!(job.intent, f.review.expected_intent_sha256);
    assert_eq!(job.output.as_ref(), Some(&f.review.expected_output_sha256));
    assert!(f.conv.activity.study_handoff.active.is_none());
    assert!(f.conv.introspect_target.is_none());
    assert!(!f.conv.wants_introspect);
    assert_eq!(reader_state, f.reader.preparation_revision().unwrap());
    assert!(!f.deliveries().exists());
    assert_eq!(f.stage().unwrap()["status"], "already_applied");
    let saved = fs::read(f.store.root().join("activity_runtime_v3.json")).unwrap();
    f.reconcile().unwrap();
    assert_eq!(
        saved,
        fs::read(f.store.root().join("activity_runtime_v3.json")).unwrap()
    );
}

#[test]
fn absent_review_keeps_unknown_work_blocked_and_conflicting_retry_is_rejected() {
    let mut f = Fixture::new();
    assert!(f.reconcile().is_err());
    f.stage().unwrap();
    let original = fs::read(f.review_path()).unwrap();
    f.review.operation_id = "different-review".into();
    assert!(f.stage().is_err());
    assert_eq!(original, fs::read(f.review_path()).unwrap());
}

#[test]
fn stale_revision_missing_intent_and_invalid_approval_do_not_release() {
    for failure in [
        "revision",
        "intent",
        "ack",
        "quiescence",
        "output",
        "cross-owner",
    ] {
        let f = Fixture::new();
        let mut r = f.review.clone();
        match failure {
            "revision" => r.expected_selection_revision += 1,
            "intent" => {
                fs::remove_file(
                    root(&f.store)
                        .unwrap()
                        .join(format!("{}.json", r.expected_intent_sha256)),
                )
                .unwrap();
            },
            "ack" => r.acknowledgement = "retry".into(),
            "quiescence" => r.provider_quiescent_observed = false,
            "output" => r.expected_output_sha256 = digest(b"wrong"),
            _ => r.job_id = format!("study-job-{}", digest(b"another-owner")),
        }
        assert!(
            stage_review(&f.store, &f.deliveries(), &r).is_err(),
            "{failure}"
        );
        assert_eq!(
            activity_reading::load_activity(&f.store)
                .unwrap()
                .study_handoff
                .active
                .as_ref(),
            Some(&f.review.job_id)
        );
    }
}

#[test]
fn review_revalidates_on_consumption_and_corrupt_history_fails_closed() {
    let mut f = Fixture::new();
    f.stage().unwrap();
    let original_path = f.review_path();
    let original = fs::read(&original_path).unwrap();
    let state = activity_reading::load_activity(&f.store).unwrap();
    let new_state = activity_reading::persist_activity(&f.store, &state).unwrap();
    assert!(f.reconcile().is_err());
    assert!(
        activity_reading::load_activity(&f.store)
            .unwrap()
            .study_handoff
            .active
            .is_some()
    );
    assert!(f.stage().is_err());
    f.review.expected_selection_revision = new_state.selection_revision;
    f.review.operation_id = "fresh-revision-review".into();
    f.stage().unwrap();
    assert_eq!(fs::read(original_path).unwrap(), original);
    fs::write(f.review_path(), b"{partial").unwrap();
    assert!(f.reconcile().is_err());
}

#[test]
fn verified_receipt_takes_precedence_and_review_never_overrides_corruption() {
    for corrupt in [false, true] {
        let mut f = Fixture::new();
        f.stage().unwrap();
        let receipt = crate::llm::retain_source_study_fixture(
            &f.output,
            &f.review.job_id,
            &f.deliveries(),
            "Exact synthetic output.\nNEXT: REST",
        );
        if corrupt {
            fs::write(receipt.retained_artifact_path, b"corrupt").unwrap();
            assert!(f.reconcile().is_err());
            assert!(f.conv.activity.study_handoff.active.is_some());
        } else {
            assert!(f.stage().is_err());
            f.reconcile().unwrap();
            assert_eq!(
                f.conv.activity.study_handoff.jobs[&f.review.job_id].phase,
                Phase::Delivered
            );
        }
    }
}

#[test]
fn interrupted_retirement_reuses_review_and_missing_retired_receipt_blocks_restart() {
    let mut f = Fixture::new();
    f.stage().unwrap();
    let hash = retain(&f.store, &f.review).unwrap();
    f.reconcile().unwrap();
    assert_eq!(
        f.conv.activity.study_handoff.jobs[&f.review.job_id]
            .uncertainty_review
            .as_ref(),
        Some(&hash)
    );
    fs::remove_file(root(&f.store).unwrap().join(format!("{hash}.json"))).unwrap();
    f.conv.introspect_target = Some(IntrospectTargetV2::auto("WRITE CONTINUE".into()));
    f.conv.introspect_target.as_mut().unwrap().operation_id = Some(f.review.job_id.clone());
    assert!(f.reconcile().is_err());
}

#[test]
fn symlink_and_oversized_review_inputs_fail_closed() {
    let f = Fixture::new();
    let path = f.temp.path().join("request.json");
    fs::write(&path, serde_json::to_vec(&f.review).unwrap()).unwrap();
    assert_eq!(read_review(&path).unwrap(), f.review);
    let link = f.temp.path().join("link");
    std::os::unix::fs::symlink(&path, &link).unwrap();
    assert!(read_review(&link).is_err());
    fs::write(&path, vec![b' '; MAX_REVIEW_BYTES as usize + 1]).unwrap();
    assert!(read_review(&path).is_err());
    let destination = f.review_path();
    std::os::unix::fs::symlink(f.temp.path().join("missing"), &destination).unwrap();
    assert!(f.stage().is_err());
    assert!(destination.is_symlink());
}

#[test]
fn concurrent_review_staging_is_idempotent_and_new_format_refuses_legacy_decoding() {
    let mut f = Fixture::new();
    #[derive(Deserialize)]
    #[serde(deny_unknown_fields)]
    struct LegacyJob {
        #[serde(rename = "intent")]
        _intent: String,
        #[serde(rename = "output")]
        _output: Option<String>,
        #[serde(rename = "phase")]
        _phase: String,
    }
    let old = serde_json::to_value(&f.conv.activity.study_handoff.jobs[&f.review.job_id]).unwrap();
    assert!(serde_json::from_value::<LegacyJob>(old).is_ok());
    let threads: Vec<_> = (0..4)
        .map(|_| {
            let store = f.store.clone();
            let deliveries = f.deliveries();
            let review = f.review.clone();
            std::thread::spawn(move || stage_review(&store, &deliveries, &review).unwrap())
        })
        .collect();
    for t in threads {
        assert_eq!(t.join().unwrap()["status"], "staged_not_applied");
    }
    f.reconcile().unwrap();
    let job = serde_json::to_value(&f.conv.activity.study_handoff.jobs[&f.review.job_id]).unwrap();
    assert!(serde_json::from_value::<LegacyJob>(job).is_err());
}
