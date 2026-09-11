use astrid_source_study::{
    Catalog, InputKind, Reader, StudyOutput,
    writing::{Profile, Writer, profile},
};
use serde_json::{Value, json};
use std::{collections::BTreeMap, fs};
fn wire(output: &StudyOutput, text: &str) -> (String, String) {
    (
        json!({"messages":[{"role":"user","content":output.text}]}).to_string(),
        json!({"choices":[{"message":{"content":text},"finish_reason":"stop"}]}).to_string(),
    )
}
fn accept(writer: &Writer, out: &StudyOutput, text: &str) {
    let (request, response) = wire(out, text);
    writer
        .delivered(out.navigation_id.as_deref().unwrap(), &request, &response)
        .unwrap();
}
#[test]
fn full_draft_survives_restart_retry_revision_branch_and_finish() {
    let temp = tempfile::tempdir().unwrap();
    let writer = Writer::new(temp.path().into());
    let start = writer.prepare("WRITE START caller versus branch").unwrap();
    assert_eq!(start.input_kind, InputKind::PrivateWriting);
    let long = format!("{}\nCONCLUSION_AT_END", "a sustained thought ".repeat(1000));
    accept(&writer, &start, &format!("{long}\nNEXT: WRITE CONTINUE"));
    accept(&writer, &start, &format!("{long}\nNEXT: WRITE CONTINUE"));
    let evidence = writer
        .prepare("WRITE EVIDENCE astrid/main.rs sha256:fixture; branch meaning remains unverified")
        .unwrap();
    accept(&writer, &evidence, "NEXT: WRITE CONTINUE");
    let question = writer.prepare("WRITE QUESTION Which branch?").unwrap();
    accept(&writer, &question, "NEXT: WRITE CONTINUE");
    let writer = Writer::new(temp.path().into());
    let next = writer.prepare("WRITE CONTINUE").unwrap();
    assert_eq!(next.text.matches("CONCLUSION_AT_END").count(), 1);
    assert!(next.text.contains(&long));
    assert!(next.text.contains("sha256:fixture"));
    assert_eq!(writer.prepare("WRITE CONTINUE").unwrap(), next);
    accept(&writer, &next, "I revise my earlier inference.");
    let revision = writer
        .prepare("WRITE REVISE reconcile the counterexample")
        .unwrap();
    assert!(revision.text.contains("CONCLUSION_AT_END"));
    accept(&writer, &revision, "A corrected whole account.");
    let branch = writer
        .prepare("WRITE BRANCH different interpretation")
        .unwrap();
    assert!(branch.text.contains("A corrected whole account."));
    assert!(
        branch
            .text
            .contains("sha256:fixture; branch meaning remains unverified")
    );
    assert!(branch.text.contains("Current question: Which branch?"));
    assert!(!branch.text.contains("CONCLUSION_AT_END"));
    accept(&writer, &branch, "Alternative account.");
    let finish = writer.prepare("WRITE FINISH").unwrap();
    accept(&writer, &finish, "NEXT: REST");
    assert!(writer.prepare("WRITE CONTINUE").is_err());
    let resume = writer.prepare("WRITE RESUME d1").unwrap();
    assert!(resume.text.contains("A corrected whole account."));
    assert!(
        resume
            .text
            .contains("sha256:fixture; branch meaning remains unverified")
    );
    assert!(resume.text.contains("Current question: Which branch?"));
    assert!(!resume.text.contains("Alternative account."));
    let state: Value =
        serde_json::from_slice(&fs::read(temp.path().join("drafts-v1.json")).unwrap()).unwrap();
    assert_eq!(state["drafts"]["d2"]["parent"], "d1");
    let earlier = fs::read_to_string(
        temp.path()
            .join(format!("{}.json", start.navigation_id.unwrap())),
    )
    .unwrap();
    assert!(earlier.contains("CONCLUSION_AT_END"));
}
#[test]
fn missing_truncated_stale_and_conflicting_deliveries_never_advance_draft() {
    let temp = tempfile::tempdir().unwrap();
    let writer = Writer::new(temp.path().into());
    let first = writer.prepare("WRITE START a question").unwrap();
    let (request, response) = wire(&first, "A passage.");
    let id = first.navigation_id.as_deref().unwrap();
    assert!(
        writer
            .delivered(id, "{\"messages\":[]}", &response)
            .is_err()
    );
    assert!(
        writer
            .delivered(id, &request, &response.replace("stop", "length"))
            .is_err()
    );
    assert_eq!(writer.prepare("WRITE CONTINUE").unwrap(), first);
    let second = writer.prepare("WRITE START another question").unwrap();
    assert!(writer.delivered(id, &request, &response).is_err());
    accept(&writer, &second, "NEXT: WRITE CONTINUE");
    let next = writer.prepare("WRITE CONTINUE").unwrap();
    assert!(next.text.contains("revision 0"));
    assert!(!next.text.contains("A passage."));
    let (req, res) = wire(&second, "different");
    assert!(
        writer
            .delivered(second.navigation_id.as_deref().unwrap(), &req, &res)
            .is_err()
    );
}
#[test]
fn explicit_preference_roundtrips_and_reader_routes_the_same_private_store() {
    let temp = tempfile::tempdir().unwrap();
    fs::write(temp.path().join("Cargo.toml"), "[workspace]\nmembers=[]").unwrap();
    let directory = temp.path().join("state");
    let reader = Reader::new(
        Catalog::new(BTreeMap::from([("astrid".into(), temp.path().into())])).unwrap(),
        directory.clone(),
    );
    assert_eq!(
        profile(&directory.join("writing")).unwrap(),
        Profile::Default
    );
    for (name, expected, tokens) in [
        ("EXTENDED", Profile::Extended, 8192),
        ("SHORT", Profile::Short, 512),
        ("DEFAULT", Profile::Default, 768),
    ] {
        let out = reader
            .prepare_action(&format!("WRITE PROFILE {name}"))
            .unwrap();
        let (request, response) = wire(&out, "Preference acknowledged.");
        reader
            .navigation_delivered(out.navigation_id.as_deref().unwrap(), &request, &response)
            .unwrap();
        assert_eq!(profile(&directory.join("writing")).unwrap(), expected);
        assert_eq!(expected.tokens(768), tokens);
    }
    let source = reader.prepare_action("SELF_STUDY MAP").unwrap();
    assert_eq!(source.input_kind, InputKind::Map);
    assert!(!source.text.contains("Preference acknowledged."));
}
#[test]
fn capacity_error_is_explicit_and_keeps_full_prior_draft() {
    let temp = tempfile::tempdir().unwrap();
    let writer = Writer::new(temp.path().into());
    let out = writer.prepare("WRITE START long").unwrap();
    accept(&writer, &out, &"x".repeat(48_000));
    let before = fs::read(temp.path().join("drafts-v1.json")).unwrap();
    assert!(
        writer
            .prepare("WRITE CONTINUE")
            .unwrap_err()
            .to_string()
            .contains("no text was shortened")
    );
    assert_eq!(
        fs::read(temp.path().join("drafts-v1.json")).unwrap(),
        before
    );
    let read = writer.prepare("WRITE READ d1 6").unwrap();
    assert!(read.text.contains("page 6/6"));
    assert!(read.text.contains(&"x".repeat(3000)));
    assert!(writer.prepare("WRITE START another").is_ok());
}

#[test]
fn fresh_start_and_parentless_branch_do_not_import_study_accounts() {
    for action in ["WRITE START a poem", "WRITE BRANCH a new direction"] {
        let temp = tempfile::tempdir().unwrap();
        fs::write(temp.path().join("Cargo.toml"), "[workspace]\nmembers=[]").unwrap();
        let directory = temp.path().join("reader");
        let reader = Reader::new(
            Catalog::new(BTreeMap::from([("astrid".into(), temp.path().into())])).unwrap(),
            directory.clone(),
        );
        let study = reader.prepare_action("SELF_STUDY MAP").unwrap();
        let (request, response) = wire(
            &study,
            "An unresolved STUDY_ACCOUNT_ONLY claim.\nSTUDY_NOTE: STUDY_NOTE_ONLY hypothesis.\nSTUDY_QUESTION: STUDY_QUESTION_ONLY?",
        );
        reader
            .navigation_delivered(study.navigation_id.as_deref().unwrap(), &request, &response)
            .unwrap();
        let checkpoint = directory.join("reader-v1.json");
        let before = fs::read(&checkpoint).unwrap();
        let writer = Writer::new(directory.join("writing"));
        let start = reader.prepare_action(action).unwrap();
        for marker in [
            "STUDY_ACCOUNT_ONLY",
            "STUDY_NOTE_ONLY",
            "STUDY_QUESTION_ONLY",
            "RECALLED ACCOUNT",
        ] {
            assert!(!start.text.contains(marker));
        }
        accept(&writer, &start, "A short passage without a required NEXT.");
        assert_eq!(fs::read(&checkpoint).unwrap(), before);
        let writing_checkpoint = directory.join("writing/drafts-v1.json");
        let state: Value = serde_json::from_slice(&fs::read(&writing_checkpoint).unwrap()).unwrap();
        assert_eq!(state["drafts"]["d1"]["evidence"], "");
        assert_eq!(state["drafts"]["d1"]["question"], "");
        assert!(state["drafts"]["d1"]["parent"].is_null());

        let evidence = reader
            .prepare_action("WRITE EVIDENCE DELIBERATELY_ATTACHED reference, still uncertain")
            .unwrap();
        accept(&writer, &evidence, "NEXT: WRITE CONTINUE");
        let unrelated = reader.prepare_action("WRITE START another topic").unwrap();
        assert!(!unrelated.text.contains("DELIBERATELY_ATTACHED"));
        assert!(!unrelated.text.contains("A short passage"));
        let state: Value = serde_json::from_slice(&fs::read(&writing_checkpoint).unwrap()).unwrap();
        assert_eq!(
            state["drafts"]["d1"]["evidence"],
            "DELIBERATELY_ATTACHED reference, still uncertain"
        );
        assert_eq!(state["drafts"]["d2"]["evidence"], "");
        assert!(state["drafts"]["d2"]["parent"].is_null());
        assert_eq!(fs::read(checkpoint).unwrap(), before);
    }
}

#[test]
fn clearing_evidence_preserves_branch_prose_question_and_parent() {
    let temp = tempfile::tempdir().unwrap();
    let writer = Writer::new(temp.path().into());
    let start = writer.prepare("WRITE START a question").unwrap();
    accept(&writer, &start, "The original passage.");
    let evidence = writer
        .prepare("WRITE EVIDENCE CHOSEN_REFERENCE with an unresolved inference")
        .unwrap();
    accept(&writer, &evidence, "NEXT: WRITE CONTINUE");
    let question = writer
        .prepare("WRITE QUESTION What remains unknown?")
        .unwrap();
    accept(&writer, &question, "NEXT: WRITE CONTINUE");
    let branch = writer.prepare("WRITE BRANCH an alternative").unwrap();
    assert!(
        branch
            .text
            .contains("CHOSEN_REFERENCE with an unresolved inference")
    );
    accept(&writer, &branch, "An alternative passage.");
    let checkpoint = temp.path().join("drafts-v1.json");
    let before: Value = serde_json::from_slice(&fs::read(&checkpoint).unwrap()).unwrap();
    let clear = writer.prepare("WRITE EVIDENCE").unwrap();
    accept(&writer, &clear, "NEXT: WRITE CONTINUE");
    let after: Value = serde_json::from_slice(&fs::read(checkpoint).unwrap()).unwrap();
    assert_eq!(after["drafts"]["d1"], before["drafts"]["d1"]);
    assert_eq!(after["drafts"]["d2"]["evidence"], "");
    for field in ["parts", "question", "revision", "parent"] {
        assert_eq!(after["drafts"]["d2"][field], before["drafts"]["d2"][field]);
    }
    let next = Writer::new(temp.path().into())
        .prepare("WRITE CONTINUE")
        .unwrap();
    assert!(!next.text.contains("CHOSEN_REFERENCE"));
    assert!(next.text.contains("The original passage."));
    assert!(next.text.contains("An alternative passage."));
    assert!(next.text.contains("What remains unknown?"));
}

#[test]
fn legacy_draft_context_and_pending_input_survive_without_migration() {
    let temp = tempfile::tempdir().unwrap();
    let legacy: StudyOutput = serde_json::from_value(json!({
        "input_kind": "private_writing",
        "evidence_scope": "Legacy private writing scope.",
        "system_prompt": "Legacy private writing prompt.",
        "text": "Legacy exact pending input.\nRECALLED ACCOUNT: LEGACY_EVIDENCE, not verified.\nOriginal prose.",
        "page": null,
        "navigation_id": "writing-7"
    }))
    .unwrap();
    let checkpoint = temp.path().join("drafts-v1.json");
    fs::write(
        &checkpoint,
        serde_json::to_vec_pretty(&json!({
            "sequence": 7,
            "active": "d1",
            "drafts": {"d1": {
                "topic": "Earlier draft",
                "question": "An earlier unresolved question?",
                "evidence": "RECALLED ACCOUNT: LEGACY_EVIDENCE, not verified.",
                "parent": null,
                "finished": false,
                "revision": 3,
                "parts": ["Original prose."]
            }},
            "pending": {"output": legacy, "draft": "d1", "revision": 3, "replace": false},
            "receipts": {}
        }))
        .unwrap(),
    )
    .unwrap();
    let before = fs::read(&checkpoint).unwrap();
    let writer = Writer::new(temp.path().into());
    assert_eq!(writer.prepare("WRITE CONTINUE").unwrap(), legacy);
    assert_eq!(fs::read(&checkpoint).unwrap(), before);
    accept(&writer, &legacy, "A later delivered passage.");
    let delivery: Value =
        serde_json::from_slice(&fs::read(temp.path().join("writing-7.json")).unwrap()).unwrap();
    assert_eq!(delivery["output"], serde_json::to_value(&legacy).unwrap());
    let resume = writer.prepare("WRITE RESUME d1").unwrap();
    assert!(
        resume
            .text
            .contains("RECALLED ACCOUNT: LEGACY_EVIDENCE, not verified.")
    );
    assert!(resume.text.contains("Original prose."));
    assert!(resume.text.contains("A later delivered passage."));
    assert!(resume.text.contains("An earlier unresolved question?"));
    accept(&writer, &resume, "NEXT: WRITE CONTINUE");
    let branch = writer.prepare("WRITE BRANCH a different reading").unwrap();
    assert!(branch.text.contains("LEGACY_EVIDENCE"));
    assert!(branch.text.contains("An earlier unresolved question?"));
    let fresh = writer.prepare("WRITE START unrelated topic").unwrap();
    assert!(!fresh.text.contains("LEGACY_EVIDENCE"));
    assert!(!fresh.text.contains("Original prose."));
    let state: Value = serde_json::from_slice(&fs::read(checkpoint).unwrap()).unwrap();
    assert_eq!(
        state["drafts"]["d1"]["evidence"],
        state["drafts"]["d2"]["evidence"]
    );
    assert_eq!(state["drafts"]["d3"]["evidence"], "");
}

#[test]
fn private_writing_does_not_read_or_repair_an_unreadable_study_checkpoint() {
    let temp = tempfile::tempdir().unwrap();
    fs::write(temp.path().join("Cargo.toml"), "[workspace]\nmembers=[]").unwrap();
    let directory = temp.path().join("reader");
    fs::create_dir(&directory).unwrap();
    let checkpoint = directory.join("reader-v1.json");
    fs::write(&checkpoint, "not a readable study checkpoint").unwrap();
    let reader = Reader::new(
        Catalog::new(BTreeMap::from([("astrid".into(), temp.path().into())])).unwrap(),
        directory,
    );
    let out = reader
        .prepare_action("WRITE START freely chosen topic")
        .unwrap();
    assert_eq!(out.input_kind, InputKind::PrivateWriting);
    assert_eq!(
        fs::read_to_string(checkpoint).unwrap(),
        "not a readable study checkpoint"
    );
}

#[test]
fn draft_preserves_quoted_fenced_and_indented_next_examples() {
    for (response, expected) in [
        (
            "    NEXT: WRITE FINISH\n\tNEXT: REST\nNEXT: WRITE CONTINUE",
            "    NEXT: WRITE FINISH\n\tNEXT: REST",
        ),
        (
            "> NEXT: WRITE FINISH\n\"NEXT: REST\"\nNEXT: WRITE CONTINUE",
            "> NEXT: WRITE FINISH\n\"NEXT: REST\"",
        ),
        (
            "~~~~text\nNEXT: WRITE FINISH\n~~~\nNEXT: REST\n~~~~\nnext: WRITE CONTINUE",
            "~~~~text\nNEXT: WRITE FINISH\n~~~\nNEXT: REST\n~~~~",
        ),
        (
            "```text\nNEXT: WRITE FINISH\n~~~\nNEXT: REST\n```\nNEXT: WRITE CONTINUE",
            "```text\nNEXT: WRITE FINISH\n~~~\nNEXT: REST\n```",
        ),
        ("```text\nNEXT: WRITE FINISH", "```text\nNEXT: WRITE FINISH"),
    ] {
        let temp = tempfile::tempdir().unwrap();
        let writer = Writer::new(temp.path().into());
        let start = writer.prepare("WRITE START command examples").unwrap();
        accept(&writer, &start, response);
        let state: Value =
            serde_json::from_slice(&fs::read(temp.path().join("drafts-v1.json")).unwrap()).unwrap();
        assert_eq!(state["drafts"]["d1"]["parts"], json!([expected]));
        assert_eq!(state["drafts"]["d1"]["finished"], false);
    }
}
