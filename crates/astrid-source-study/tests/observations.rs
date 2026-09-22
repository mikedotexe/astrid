use astrid_source_study::{Catalog, Reader, StudyOutput};
use serde_json::{Value, json};
use sha2::{Digest as _, Sha256};
use std::{collections::BTreeMap, fs, path::Path};
fn hash(s: impl AsRef<[u8]>) -> String {
    format!("{:x}", Sha256::digest(s.as_ref()))
}
struct Fixture {
    temp: tempfile::TempDir,
    reader: Reader,
    owner: String,
    op: u64,
}
impl Fixture {
    fn new(owner: &str) -> Self {
        let temp = tempfile::tempdir().unwrap();
        let root = temp.path();
        fs::create_dir_all(root.join("minime/workspace/runtime")).unwrap();
        fs::create_dir_all(root.join("astrid/crates/example/src")).unwrap();
        fs::write(
            root.join("astrid/crates/example/src/lib.rs"),
            "fn fixture() {}\n",
        )
        .unwrap();
        let reader = Reader::new(
            Catalog::new(BTreeMap::from([
                ("astrid".into(), root.join("astrid")),
                ("minime".into(), root.join("minime")),
            ]))
            .unwrap(),
            root.join("reader"),
        )
        .with_runtime_workspace(root.join("workspace"), owner);
        Self {
            temp,
            reader,
            owner: owner.into(),
            op: 0,
        }
    }
    fn root(&self) -> &Path {
        self.temp.path()
    }
    fn action(&mut self, action: &str) -> anyhow::Result<StudyOutput> {
        self.op = self.op.saturating_add(1);
        self.reader.prepare_once(
            &format!("host-{}", self.op),
            &self.reader.preparation_revision()?,
            action,
        )
    }
    fn state(&self) -> Value {
        serde_json::from_slice(
            &fs::read(self.root().join("reader/writing/drafts-v2.json")).unwrap(),
        )
        .unwrap()
    }
    fn history(&self) -> Value {
        self.state()["drafts"]["d1"]["observations"].clone()
    }
    fn head(&self) -> String {
        self.history()["records"]
            .as_array()
            .unwrap()
            .last()
            .map_or("empty", |r| r["id"].as_str().unwrap())
            .into()
    }
    fn revision(&self) -> String {
        let s = self.state();
        let d = &s["drafts"]["d1"];
        hash(
            serde_json::to_vec(&json!([
                d["topic"],
                d["question"],
                d["evidence"],
                d["parts"],
                d["stopping_point"],
                d["revision"]
            ]))
            .unwrap(),
        )
    }
    #[allow(clippy::needless_pass_by_value)] // Inline fixture payloads match the wire operation.
    fn request(&self, id: &str, operation: Value) -> String {
        format!(
            "WRITE OBSERVE {}",
            json!({"owner":self.owner,"draft":"d1","revision":self.revision(),"request_id":id,"expected_head":self.head(),"operation":operation})
        )
    }
    fn observe(&mut self, id: &str, operation: Value) -> anyhow::Result<StudyOutput> {
        self.action(&self.request(id, operation))
    }
    fn delivered(&self, output: &StudyOutput, text: &str) {
        self.reader
            .navigation_delivered(
                output.navigation_id.as_ref().unwrap(),
                &json!({"messages":[{"role":"user","content":output.text}]}).to_string(),
                &json!({"message":{"content":text},"done":true,"done_reason":"stop"}).to_string(),
            )
            .unwrap();
    }
    fn preview_delivery(&mut self, id: &str) {
        let output=self.action(&format!("WRITE OBSERVE {}",json!({"owner":self.owner,"draft":"d1","present":true,"operation":{"kind":"show","id":id}}))).unwrap();
        self.delivered(
            &output,
            "I have the selected evidence and exact disclosure preview.",
        );
    }
    fn draft(&mut self) {
        let output = self
            .action("WRITE START Private title must not travel")
            .unwrap();
        self.delivered(
            &output,
            "Unselected secret.\nThe pattern matters to me: café.\nUnselected ending.",
        );
        self.action("WRITE STOPPING_POINT Private stopping point")
            .unwrap();
    }
    fn trace(&self) {
        let now = u64::try_from(
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_millis(),
        )
        .unwrap();
        let frames:Vec<_>=(0_u64..80).map(|i|json!({"t_ms":i.saturating_mul(1000),"wall_clock_unix_ms":now.saturating_sub(79_u64.saturating_sub(i).saturating_mul(1000)),"activations":vec![if i%8<4{0.2}else{-0.2};128],"summary":{"finite_fraction":1.0}})).collect();
        fs::write(self.root().join("minime/workspace/runtime/esn_activation_trace_v1.json"),json!({"policy":"esn_activation_trace_v1","reservoir_dim":128,"sample_interval_ms":1000,"retained_secs":180,"updated_at_unix_ms":now,"frames":frames}).to_string()).unwrap();
    }
}
#[test]
fn private_capture_analysis_preview_confirm_revision_and_quiet_return_both_owners() {
    for owner in ["astrid", "minime"] {
        let mut f = Fixture::new(owner);
        f.draft();
        f.trace();
        let status = f.observe("", json!({"kind":"status"})).unwrap();
        assert!(!status.generation_requested);
        assert!(status.text.contains("start_byte") && status.text.contains("state-return-rms-v1"));
        let before = f.state()["drafts"]["d1"]["parts"].clone();
        let captured = f
            .observe("capture", json!({"kind":"capture","seconds":180}))
            .unwrap();
        assert!(!captured.generation_requested);
        let capture = f.head();
        f.observe(
            "note",
            json!({"kind":"annotate","target":capture,"text":"Private annotation"}),
        )
        .unwrap();
        let returned = f.action("WRITE CONTINUE").unwrap();
        assert!(returned.text.contains(&capture) && !returned.text.contains("Private annotation"));
        f.action("WRITE PARK").unwrap();
        f.action("WRITE START unrelated").unwrap();
        let restored = f.action("WRITE RESUME d1").unwrap();
        assert!(restored.text.contains("café") && !restored.text.contains("Private annotation"));
        assert_eq!(f.state()["drafts"]["d1"]["parts"], before);
        f.observe("analysis",json!({"kind":"analyze","analysis":{"recipe":"state-return-rms-v1","window":{"capture":capture,"start_ms":0,"end_ms":79000},"threshold":0.01}})).unwrap();
        let result = f.head();
        f.observe("preview",json!({"kind":"link_preview","captures":[capture],"destination":{"kind":"new","question":"Does this recur?"}})).unwrap();
        let preview = f.head();
        let reader_path = f.root().join("reader/reader-v1.json");
        assert!(
            !reader_path.exists(),
            "preview is private, not a new inquiry"
        );
        let action = f.request(
            "Private operation label must not travel",
            json!({"kind":"link_confirm","preview":preview}),
        );
        assert!(
            f.action(&action).is_err(),
            "cannot confirm unseen private preview"
        );
        f.preview_delivery(&preview);
        f.action(&action).unwrap();
        f.action(&action).unwrap();
        let reader: Value = serde_json::from_slice(&fs::read(&reader_path).unwrap()).unwrap();
        assert_eq!(reader["questions"]["entries"].as_object().unwrap().len(), 1);
        assert!(reader["questions"]["active"].is_null());
        let public = serde_json::to_string(&reader).unwrap();
        for private in [
            "Private annotation",
            "Private stopping point",
            "Private title",
            "Private operation label",
            "Unselected secret",
            "café",
        ] {
            assert!(!public.contains(private), "leaked {private}");
        }
        f.observe("counterevidence",json!({"kind":"annotate","target":result,"text":"This result does not establish the recurrence I meant."})).unwrap();
        assert_eq!(f.state()["drafts"]["d1"]["parts"], before);
        assert!(!f.root().join("workspace/journal").exists());
    }
}

#[test]
fn existing_destination_drift_failed_preview_delivery_and_complete_public_export() {
    let mut f = Fixture::new("astrid");
    f.draft();
    f.trace();
    f.action("SELF_STUDY QUESTION NEW Existing authored question?")
        .unwrap();
    let opened = f
        .action("SELF_STUDY OPEN astrid/crates/example/src/lib.rs 1")
        .unwrap();
    f.reader.delivered(&opened.page.as_ref().unwrap().id,
        &json!({"messages":[{"role":"user","content":opened.text}]}).to_string(),
        &json!({"message":{"content":"STUDY_NOTE: This source remains unresolved."},"done":true,"done_reason":"stop"}).to_string()).unwrap();
    f.action("SELF_STUDY GEOMETRY {\"question\":\"q1\",\"request_id\":\"legacy\",\"expected_head\":\"empty\",\"operation\":{\"kind\":\"capture\",\"seconds\":30,\"note\":\"Legacy geometry remains readable.\"}}").unwrap();
    f.observe("capture", json!({"kind":"capture","seconds":180}))
        .unwrap();
    let capture = f.head();
    let status_action = "SELF_STUDY OBSERVE {\"owner\":\"astrid\",\"question\":\"q1\",\"operation\":{\"kind\":\"status\"}}";
    let status = f.action(status_action).unwrap();
    let revision = status
        .text
        .lines()
        .find_map(|l| l.strip_prefix("Revision "))
        .unwrap();
    f.observe("preview-stale", json!({"kind":"link_preview","captures":[capture],"destination":{"kind":"existing","question":"q1","revision":revision}})).unwrap();
    let stale_preview = f.head();
    f.preview_delivery(&stale_preview);
    let revised = f.action("SELF_STUDY QUESTION q1").unwrap();
    f.delivered(
        &revised,
        "STUDY_NOTE: New counterevidence, not a resolution.",
    );
    assert!(
        f.observe(
            "stale-confirm",
            json!({"kind":"link_confirm","preview":stale_preview})
        )
        .is_err()
    );
    let status = f.action(status_action).unwrap();
    let revision = status
        .text
        .lines()
        .find_map(|l| l.strip_prefix("Revision "))
        .unwrap();
    f.observe("preview-current", json!({"kind":"link_preview","captures":[capture],"destination":{"kind":"existing","question":"q1","revision":revision}})).unwrap();
    let preview = f.head();
    let output=f.action(&format!("WRITE OBSERVE {}",json!({"owner":"astrid","draft":"d1","present":true,"operation":{"kind":"show","id":preview}}))).unwrap();
    assert!(f.reader.navigation_delivered(output.navigation_id.as_ref().unwrap(),
        &json!({"messages":[{"role":"user","content":"Incomplete input"}]}).to_string(),
        &json!({"message":{"content":"Not the exact preview"},"done":true,"done_reason":"stop"}).to_string()).is_err());
    assert!(
        f.observe(
            "not-delivered",
            json!({"kind":"link_confirm","preview":preview})
        )
        .is_err()
    );
    f.preview_delivery(&preview);
    let path = f.root().join("reader/reader-v1.json");
    let before: Value = serde_json::from_slice(&fs::read(&path).unwrap()).unwrap();
    let receipt = f
        .observe(
            "confirm-existing",
            json!({"kind":"link_confirm","preview":preview}),
        )
        .unwrap();
    assert!(receipt.text.contains("Inquiry q1"));
    let after: Value = serde_json::from_slice(&fs::read(&path).unwrap()).unwrap();
    assert_eq!(before["questions"]["active"], after["questions"]["active"]);
    assert_eq!(
        before["questions"]["entries"]["q1"]["notebook"],
        after["questions"]["entries"]["q1"]["notebook"]
    );
    assert_eq!(
        before["questions"]["entries"]["q1"]["geometry"],
        after["questions"]["entries"]["q1"]["geometry"]
    );
    f.action("SELF_STUDY GEOMETRY {\"question\":\"q1\",\"operation\":{\"kind\":\"export\"}}")
        .unwrap();
    let export = fs::read_dir(f.root().join("reader/geometry-exports"))
        .unwrap()
        .next()
        .unwrap()
        .unwrap()
        .path();
    let raw = fs::read_to_string(export).unwrap();
    let envelope: Value = serde_json::from_str(&raw).unwrap();
    assert_eq!(envelope["format"], "inquiry-observations-v2");
    let body: Value = serde_json::from_str(envelope["body_json"].as_str().unwrap()).unwrap();
    assert_eq!(body["geometry_v1"]["records"].as_array().unwrap().len(), 1);
    assert_eq!(body["observations"]["records"].as_array().unwrap().len(), 1);
    assert!(!raw.contains("Private title") && !raw.contains("café") && !raw.contains("drafts-v2"));
}
#[test]
fn exact_utf8_disclosure_stale_source_and_conflicting_retries_fail_without_loss() {
    let mut f = Fixture::new("minime");
    f.draft();
    f.trace();
    f.observe("capture", json!({"kind":"capture","seconds":180}))
        .unwrap();
    let capture = f.head();
    let prose = f.state()["drafts"]["d1"]["parts"][0]
        .as_str()
        .unwrap()
        .to_owned();
    let start = prose.find("café").unwrap();
    let end = start.saturating_add("café".len());
    let op = json!({"kind":"link_preview","captures":[capture],"destination":{"kind":"new","question":"A chosen question"},"passage":{"start_byte":start,"end_byte":end,"source_sha256":hash(&prose)}});
    let mut invalid = op.clone();
    invalid["passage"]["end_byte"] = json!(end.saturating_sub(1));
    let head = f.head();
    assert!(f.observe("bad", invalid).is_err());
    assert_eq!(f.head(), head);
    f.observe("preview", op).unwrap();
    let preview = f.head();
    f.preview_delivery(&preview);
    f.observe("confirm", json!({"kind":"link_confirm","preview":preview}))
        .unwrap();
    let raw = fs::read_to_string(f.root().join("reader/reader-v1.json")).unwrap();
    assert!(raw.contains("café") && !raw.contains("Unselected secret"));
    assert!(
        f.observe(
            "confirm",
            json!({"kind":"annotate","target":capture,"text":"conflict"})
        )
        .is_err()
    );
    f.observe("preview-2",json!({"kind":"link_preview","captures":[capture],"destination":{"kind":"new","question":"Another chosen question"}})).unwrap();
    let preview = f.head();
    f.preview_delivery(&preview);
    let revision = f.action("WRITE REVISE new words").unwrap();
    f.delivered(&revision, "New exact words.");
    let head = f.head();
    assert!(
        f.observe("stale", json!({"kind":"link_confirm","preview":preview}))
            .is_err()
    );
    assert_eq!(f.head(), head);
}

#[test]
fn escaped_passage_must_fit_one_complete_confirmation_preview() {
    for owner in ["astrid", "minime"] {
        let mut f = Fixture::new(owner);
        f.draft();
        let prose = format!("Keep this exactly: {}", "\u{1}".repeat(6000));
        let revised = f.action("WRITE REVISE exact synthetic passage").unwrap();
        f.delivered(&revised, &prose);
        f.trace();
        f.observe("capture", json!({"kind":"capture","seconds":180}))
            .unwrap();
        let capture = f.head();
        let before = f.state();
        let error = f
            .observe(
                "oversized-preview",
                json!({
                    "kind":"link_preview", "captures":[capture],
                    "destination":{"kind":"new","question":"An optional question?"},
                    "passage":{"start_byte":0,"end_byte":prose.len(),"source_sha256":hash(&prose)}
                }),
            )
            .unwrap_err();
        assert!(error.to_string().contains("complete preview"));
        assert_eq!(f.state(), before, "no partial preview or prose loss");
        assert!(!f.root().join("reader/reader-v1.json").exists());
    }
}

#[test]
fn numerical_expectation_is_historical_optional_and_not_a_resolution() {
    let mut f = Fixture::new("astrid");
    f.draft();
    f.trace();
    f.observe("capture", json!({"kind":"capture","seconds":180}))
        .unwrap();
    let capture = f.head();
    let analysis = json!({"recipe":"state-return-rms-v1","window":{"capture":capture,"start_ms":0,"end_ms":79000},"threshold":0.01});
    f.observe("prediction",json!({"kind":"annotate","target":capture,"text":"I expect fewer close pairs.","expectation":{"analysis":analysis,"comparison":"at_most","value":0.01}})).unwrap();
    let expectation = f.head();
    f.observe(
        "result",
        json!({"kind":"analyze","analysis":analysis,"expectation":expectation}),
    )
    .unwrap();
    let records = f.history();
    let entry: Value = serde_json::from_str(
        records["records"].as_array().unwrap().last().unwrap()["body_json"]
            .as_str()
            .unwrap(),
    )
    .unwrap();
    assert_eq!(entry["evaluation"]["evidence_predates_expectation"], true);
    assert_eq!(entry["evaluation"]["numerical_expectation_matched"], false);
    assert!(!f.root().join("reader/reader-v1.json").exists());
    let mut wrong = analysis;
    wrong["threshold"] = json!(0.7);
    assert!(
        f.observe(
            "wrong",
            json!({"kind":"analyze","analysis":wrong,"expectation":expectation})
        )
        .is_err()
    );
}

#[test]
fn capacity_branch_and_corruption_preserve_every_existing_record() {
    let mut f = Fixture::new("minime");
    f.draft();
    f.trace();
    for i in 0..4 {
        f.observe(
            &format!("capture-{i}"),
            json!({"kind":"capture","seconds":180}),
        )
        .unwrap();
    }
    let head = f.head();
    assert!(
        f.observe("fifth", json!({"kind":"capture","seconds":180}))
            .is_err()
    );
    assert_eq!(head, f.head());
    f.action("WRITE RESUME d1").unwrap();
    f.action("WRITE BRANCH independent interpretation").unwrap();
    let inherited = f.state()["drafts"]["d2"]["observations"].clone();
    for i in 4..64 {
        f.observe(
            &format!("note-{i}"),
            json!({"kind":"annotate","target":head,"text":"A retained optional annotation."}),
        )
        .unwrap();
    }
    assert_eq!(f.state()["drafts"]["d2"]["observations"], inherited);
    let before = fs::read(f.root().join("reader/writing/drafts-v2.json")).unwrap();
    assert!(
        f.observe(
            "overflow",
            json!({"kind":"annotate","target":head,"text":"No room."})
        )
        .is_err()
    );
    assert_eq!(
        before,
        fs::read(f.root().join("reader/writing/drafts-v2.json")).unwrap()
    );
    let path = f.root().join("reader/writing/drafts-v2.json");
    let mut corrupt = f.state();
    corrupt["drafts"]["d1"]["observations"]["records"][0]["body_json"] = json!("{broken");
    let bytes = serde_json::to_vec(&corrupt).unwrap();
    fs::write(&path, &bytes).unwrap();
    assert!(f.action("WRITE LIST").is_err());
    assert_eq!(fs::read(path).unwrap(), bytes);
}

#[test]
fn cross_owner_and_future_schema_fail_closed_and_next_json_remains_exact() {
    let mut f = Fixture::new("astrid");
    f.draft();
    f.trace();
    let request = f.request("capture", json!({"kind":"capture","seconds":180}));
    assert!(
        f.action(&request.replace("\"owner\":\"astrid\"", "\"owner\":\"minime\""))
            .is_err()
    );
    let action = "WRITE OBSERVE {\"text\":\"<think> AND REST RESIDUE: NEXT: REST </s>\"}  ";
    assert_eq!(
        astrid_source_study::response_choice::final_explicit_next(&format!("NEXT: {action}")),
        Some(action)
    );
    let path = f.root().join("reader/writing/drafts-v2.json");
    let mut value = f.state();
    value["schema_version"] = json!(5);
    let bytes = serde_json::to_vec(&value).unwrap();
    fs::write(&path, &bytes).unwrap();
    assert!(f.action("WRITE LIST").is_err());
    assert_eq!(fs::read(path).unwrap(), bytes);
}

#[test]
fn concurrent_first_admissions_cannot_append_over_the_same_head() {
    let mut f = Fixture::new("astrid");
    f.draft();
    f.trace();
    let requests = [
        f.request("a", json!({"kind":"capture","seconds":180})),
        f.request("b", json!({"kind":"capture","seconds":180})),
    ];
    let barrier = std::sync::Arc::new(std::sync::Barrier::new(2));
    let handles: Vec<_> = requests
        .into_iter()
        .map(|action| {
            let root = f.root().to_path_buf();
            let barrier = barrier.clone();
            std::thread::spawn(move || {
                let r = Reader::new(
                    Catalog::new(BTreeMap::from([
                        ("astrid".into(), root.join("astrid")),
                        ("minime".into(), root.join("minime")),
                    ]))
                    .unwrap(),
                    root.join("reader"),
                )
                .with_runtime_workspace(root.join("workspace"), "astrid");
                let revision = r.preparation_revision().unwrap();
                barrier.wait();
                r.prepare_once(&hash(&action), &revision, &action).is_ok()
            })
        })
        .collect();
    let passed = handles
        .into_iter()
        .map(|h| h.join().unwrap())
        .filter(|ok| *ok)
        .count();
    assert_eq!(passed, 1);
    assert_eq!(f.history()["records"].as_array().unwrap().len(), 1);
}

#[test]
fn forged_numerical_receipt_is_recomputed_even_with_a_rehashed_tail() {
    let mut f = Fixture::new("astrid");
    f.draft();
    f.trace();
    f.observe("capture", json!({"kind":"capture","seconds":180}))
        .unwrap();
    let capture = f.head();
    f.observe("analysis",json!({"kind":"analyze","analysis":{"recipe":"state-return-rms-v1","window":{"capture":capture,"start_ms":0,"end_ms":79000},"threshold":0.01}})).unwrap();
    let mut state = f.state();
    let record = state["drafts"]["d1"]["observations"]["records"]
        .as_array_mut()
        .unwrap()
        .last_mut()
        .unwrap();
    let mut body: Value = serde_json::from_str(record["body_json"].as_str().unwrap()).unwrap();
    body["result"]["near_pairs"] = json!(0);
    record["body_json"] = serde_json::to_string(&body).unwrap().into();
    record["id"] = hash(format!(
        "{}\n{}\n{}\n{}",
        record["previous"].as_str().unwrap(),
        hash(record["request_id"].as_str().unwrap()),
        record["request_sha256"].as_str().unwrap(),
        hash(record["body_json"].as_str().unwrap())
    ))
    .into();
    let path = f.root().join("reader/writing/drafts-v2.json");
    let bytes = serde_json::to_vec(&state).unwrap();
    fs::write(&path, &bytes).unwrap();
    assert!(f.action("WRITE LIST").is_err());
    assert_eq!(fs::read(path).unwrap(), bytes);
}

#[test]
fn sanitized_sources_are_rejected_and_metadata_does_not_spend_or_refill_focus() {
    use astrid_source_study::ActivityRequest as Request;
    let mut f = Fixture::new("minime");
    f.draft();
    f.trace();
    let at = u64::try_from(
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_millis(),
    )
    .unwrap();
    let status = f
        .reader
        .activity("status", None, at, Request::Status)
        .unwrap();
    let focused = f
        .reader
        .activity(
            "focus",
            Some(status.revision),
            at,
            Request::Command {
                action: "ACTIVITY_FOCUS WRITE d1 turns 3".into(),
            },
        )
        .unwrap();
    assert!(focused.protected);
    f.observe("capture", json!({"kind":"capture","seconds":180}))
        .unwrap();
    let after = f
        .reader
        .activity("status", None, at, Request::Status)
        .unwrap();
    assert_eq!(after.admitted, focused.admitted);
    assert!(!after.protected);
    let path = f
        .root()
        .join("minime/workspace/runtime/esn_activation_trace_v1.json");
    let mut trace: Value = serde_json::from_slice(&fs::read(&path).unwrap()).unwrap();
    trace["frames"][0]["summary"]["finite_fraction"] = json!(0.99);
    fs::write(path, trace.to_string()).unwrap();
    let head = f.head();
    assert!(
        f.observe("sanitized", json!({"kind":"capture","seconds":180}))
            .is_err()
    );
    assert_eq!(f.head(), head);
}
