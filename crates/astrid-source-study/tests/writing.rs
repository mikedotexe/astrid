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
    let start = writer
        .prepare(
            "WRITE START caller versus branch",
            "Evidence: astrid/main.rs sha256:fixture; current question: which branch?",
        )
        .unwrap();
    assert_eq!(start.input_kind, InputKind::PrivateWriting);
    let long = format!("{}\nCONCLUSION_AT_END", "a sustained thought ".repeat(1000));
    accept(&writer, &start, &format!("{long}\nNEXT: WRITE CONTINUE"));
    accept(&writer, &start, &format!("{long}\nNEXT: WRITE CONTINUE"));
    let writer = Writer::new(temp.path().into());
    let next = writer.prepare("WRITE CONTINUE", "").unwrap();
    assert_eq!(next.text.matches("CONCLUSION_AT_END").count(), 1);
    assert!(next.text.contains(&long));
    assert!(next.text.contains("sha256:fixture"));
    assert_eq!(writer.prepare("WRITE CONTINUE", "").unwrap(), next);
    accept(&writer, &next, "I revise my earlier inference.");
    let revision = writer
        .prepare("WRITE REVISE reconcile the counterexample", "")
        .unwrap();
    assert!(revision.text.contains("CONCLUSION_AT_END"));
    accept(&writer, &revision, "A corrected whole account.");
    let branch = writer
        .prepare("WRITE BRANCH different interpretation", "")
        .unwrap();
    assert!(branch.text.contains("A corrected whole account."));
    assert!(!branch.text.contains("CONCLUSION_AT_END"));
    accept(&writer, &branch, "Alternative account.");
    let finish = writer.prepare("WRITE FINISH", "").unwrap();
    accept(&writer, &finish, "NEXT: REST");
    assert!(writer.prepare("WRITE CONTINUE", "").is_err());
    let resume = writer.prepare("WRITE RESUME d1", "").unwrap();
    assert!(resume.text.contains("A corrected whole account."));
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
    let first = writer.prepare("WRITE START a question", "").unwrap();
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
    assert_eq!(writer.prepare("WRITE CONTINUE", "").unwrap(), first);
    let second = writer.prepare("WRITE START another question", "").unwrap();
    assert!(writer.delivered(id, &request, &response).is_err());
    accept(&writer, &second, "NEXT: WRITE CONTINUE");
    let next = writer.prepare("WRITE CONTINUE", "").unwrap();
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
    let out = writer.prepare("WRITE START long", "").unwrap();
    accept(&writer, &out, &"x".repeat(48_000));
    let before = fs::read(temp.path().join("drafts-v1.json")).unwrap();
    assert!(
        writer
            .prepare("WRITE CONTINUE", "")
            .unwrap_err()
            .to_string()
            .contains("no text was shortened")
    );
    assert_eq!(
        fs::read(temp.path().join("drafts-v1.json")).unwrap(),
        before
    );
    let read = writer.prepare("WRITE READ d1 6", "").unwrap();
    assert!(read.text.contains("page 6/6"));
    assert!(read.text.contains(&"x".repeat(3000)));
    assert!(writer.prepare("WRITE START another", "").is_ok());
}
