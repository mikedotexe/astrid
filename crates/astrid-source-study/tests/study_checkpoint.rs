use astrid_source_study::{Catalog, MAX_INPUT_BYTES, Reader, StudyOutput};
use serde_json::{Value, json};
use std::{collections::BTreeMap, fmt::Write as _, fs};

const SOURCE: &str = "astrid/crates/example/src/handler.rs";
const SECOND: &str = "astrid/crates/example/src/caller.rs";
const CHECK_IN: &str = "OPTIONAL STUDY CHECK-IN";
const CHECK_IN_END: &str = "Your findings remain yours to retain, qualify or revise.\n";
const NOTEBOOK: &str = "RECALLED ACCOUNT — your study notebook";
const STUB: &str = "pub fn install_capsule() -> &'static str { \"not implemented\" }";

fn setup() -> (tempfile::TempDir, Reader) {
    let temp = tempfile::tempdir().unwrap();
    let root = temp.path().join("astrid");
    fs::create_dir_all(root.join("crates/example/src")).unwrap();
    fs::write(
        root.join("crates/example/src/handler.rs"),
        format!("{STUB}\n"),
    )
    .unwrap();
    fs::write(
        root.join("crates/example/src/caller.rs"),
        "pub fn caller() { install_capsule(); }\n",
    )
    .unwrap();
    let reader = Reader::new(
        Catalog::new(BTreeMap::from([("astrid".into(), root)])).unwrap(),
        temp.path().join("reader"),
    );
    (temp, reader)
}

fn open(reader: &Reader, source: &str) -> StudyOutput {
    reader
        .prepare_action(&format!("SELF_STUDY OPEN {source} 1"))
        .unwrap()
}

fn wire(output: &StudyOutput, input: &str, content: &str) -> (String, String) {
    (
        json!({"messages":[{"role":"system","content":output.system_prompt},{"role":"user","content":input}]}).to_string(),
        json!({"message":{"content":content},"done":true,"done_reason":"stop"}).to_string(),
    )
}

fn deliver(reader: &Reader, output: &StudyOutput, input: &str, content: &str) -> bool {
    let (request, response) = wire(output, input, content);
    if let Some(page) = &output.page {
        reader.delivered(&page.id, &request, &response).is_ok()
    } else {
        reader
            .navigation_delivered(
                output.navigation_id.as_deref().unwrap(),
                &request,
                &response,
            )
            .is_ok()
    }
}

fn accept(reader: &Reader, output: &StudyOutput, content: &str) {
    assert!(deliver(reader, output, &output.text, content));
}

fn state_bytes(temp: &tempfile::TempDir) -> Vec<u8> {
    fs::read(temp.path().join("reader/reader-v1.json")).unwrap()
}

fn state(temp: &tempfile::TempDir) -> Value {
    serde_json::from_slice(&state_bytes(temp)).unwrap()
}

fn check_in(output: &StudyOutput) -> &str {
    output
        .text
        .split_once(CHECK_IN)
        .unwrap()
        .1
        .split_once(NOTEBOOK)
        .unwrap()
        .0
}

#[test]
fn current_source_precedes_authored_words_and_their_retained_evidence() {
    let (temp, reader) = setup();
    let first = open(&reader, SOURCE);
    let original_page = first.page.as_ref().unwrap();
    // Deliberately unsupported: delivery is not a truth classifier.
    let claim = "This stub guarantees all requests have permission.";
    accept(
        &reader,
        &first,
        &format!(
            "STUDY_QUESTION: Does this check permission?\nSTUDY_NOTE: The caller is still unexamined.\nSTUDY_FINDING: {SOURCE}:1 | {claim}"
        ),
    );
    let output = open(&reader, SECOND);
    let current_page = output.page.as_ref().unwrap();
    let source_end = output.text.find(&current_page.text).unwrap() + current_page.text.len();
    let coverage = output.text.find("READING COVERAGE").unwrap();
    let checkpoint = output.text.find(CHECK_IN).unwrap();
    let notebook = output.text.find(NOTEBOOK).unwrap();
    assert!(source_end <= coverage && coverage < checkpoint && checkpoint < notebook);
    let visible = check_in(&output);
    assert!(visible.contains("YOUR SAVED NOTE — recalled account, not independently verified"));
    assert!(visible.contains("The caller is still unexamined."));
    assert!(visible.contains("This origin does not establish the note's claims"));
    assert!(visible.contains("your saved interpretations, not independently verified facts"));
    assert!(visible.contains(claim));
    assert!(visible.contains(&format!("Retained fragment {SOURCE}:1")));
    assert!(visible.contains(&original_page.revision.sha256));
    assert!(visible.contains(&format!("{STUB:?}")));
    assert!(visible.contains(&format!("SELF_STUDY OPEN {SOURCE} 1")));
    let saved = state(&temp);
    let finding = &saved["notebook"]["source_findings"]["authored"][0];
    assert_eq!(finding["words"], claim);
    assert_eq!(finding["anchor"]["page_id"], original_page.id);
    assert!(finding.get("verified").is_none());
    assert!(finding.get("truth").is_none());
}

#[test]
fn chosen_correction_outlives_recent_responses_without_automatic_rewriting() {
    let (temp, reader) = setup();
    let first = open(&reader, SOURCE);
    accept(
        &reader,
        &first,
        &format!(
            "STUDY_QUESTION: Does installation enforce the input gate?\nSTUDY_NOTE: I assumed every request is blocked.\nSTUDY_FINDING: {SOURCE}:1 | I assumed this checks the gate.\nInitial account."
        ),
    );
    let correction = open(&reader, SOURCE);
    let note = "The handler is a stub; admission elsewhere is still unverified.";
    let finding = "This line returns a stub message; it establishes no permission check.";
    accept(
        &reader,
        &correction,
        &format!(
            "STUDY_NOTE: {note}\nSTUDY_FINDING: {SOURCE}:1 | {finding}\nI am explicitly correcting my earlier claim."
        ),
    );
    let corrected = state(&temp)["notebook"].clone();
    for turn in 0..6 {
        let next = open(&reader, SECOND);
        let visible = check_in(&next);
        assert!(visible.contains(note));
        assert!(visible.contains(finding));
        assert!(visible.contains("This origin does not establish the note's claims"));
        accept(
            &reader,
            &next,
            &format!(
                "Later account {turn}: perhaps every request is blocked after all. I have supplied no note or finding directive."
            ),
        );
    }
    let after = state(&temp)["notebook"].clone();
    assert_eq!(after["note"], corrected["note"]);
    assert_eq!(after["question"], corrected["question"]);
    assert_eq!(
        after["source_findings"]["authored"],
        corrected["source_findings"]["authored"]
    );
    assert_eq!(after["recent"].as_array().unwrap().len(), 3);
    assert!(
        !after["recent"]
            .to_string()
            .contains("explicitly correcting")
    );
    assert!(
        !after["previous"]
            .to_string()
            .contains("explicitly correcting")
    );

    // A new checkout revision and a new reader must retain the authored
    // correction's original anchor, rather than silently re-justifying it.
    fs::write(
        temp.path().join("astrid/crates/example/src/handler.rs"),
        "pub fn install_capsule() -> bool { true }\n",
    )
    .unwrap();
    let restored = Reader::new(
        Catalog::new(BTreeMap::from([(
            "astrid".into(),
            temp.path().join("astrid"),
        )]))
        .unwrap(),
        temp.path().join("reader"),
    );
    let reopened = open(&restored, SOURCE);
    assert_ne!(
        reopened.page.as_ref().unwrap().revision.sha256,
        correction.page.as_ref().unwrap().revision.sha256
    );
    let visible = check_in(&reopened);
    assert!(visible.contains(note));
    assert!(visible.contains(finding));
    assert!(visible.contains(&format!("{STUB:?}")));
    assert!(visible.contains("current checkout may differ"));
    assert_eq!(state(&temp)["notebook"]["note"], corrected["note"]);
}

#[test]
fn new_and_restored_inquiries_show_only_their_own_authored_context() {
    let (temp, reader) = setup();
    let general = open(&reader, SOURCE);
    accept(&reader, &general, "STUDY_NOTE: General browsing note.");
    let first = reader
        .prepare_action("SELF_STUDY QUESTION NEW Why is installation a stub?")
        .unwrap();
    assert_eq!(first.question_id.as_deref(), Some("q1"));
    assert!(!first.text.contains("General browsing note."));
    accept(&reader, &first, "I will read the handler.");

    // A NEW inquiry has a title before any STUDY_QUESTION directive exists.
    let first_page = open(&reader, SOURCE);
    let page = first_page.page.as_ref().unwrap();
    let source_end = first_page.text.find(&page.text).unwrap() + page.text.len();
    let active = first_page.text.find("ACTIVE STUDY QUESTION q1").unwrap();
    assert!(source_end < active);
    assert!(
        first_page
            .text
            .contains("Saved inquiry question: Why is installation a stub?")
    );
    assert!(first_page.text.contains("SELF_STUDY QUESTION PARK q1"));
    assert!(first_page.text.contains("SELF_STUDY QUESTION HOME"));
    assert!(!first_page.text.contains("General browsing note."));
    accept(
        &reader,
        &first_page,
        &format!(
            "STUDY_QUESTION: Which caller invokes installation?\nSTUDY_NOTE: First inquiry note.\nSTUDY_FINDING: {SOURCE}:1 | First inquiry's tentative stub conclusion."
        ),
    );
    let second = reader
        .prepare_action("SELF_STUDY QUESTION NEW What does the caller establish?")
        .unwrap();
    assert_eq!(second.question_id.as_deref(), Some("q2"));
    assert!(!second.text.contains("First inquiry note."));
    assert!(
        !second
            .text
            .contains("First inquiry's tentative stub conclusion.")
    );
    accept(&reader, &second, "I will read the caller independently.");
    let second_page = open(&reader, SECOND);
    accept(
        &reader,
        &second_page,
        &format!(
            "STUDY_NOTE: Second inquiry note.\nSTUDY_FINDING: {SECOND}:1 | Second inquiry's tentative caller conclusion."
        ),
    );
    let restored = reader.prepare_action("SELF_STUDY QUESTION q1").unwrap();
    assert!(check_in(&restored).contains("First inquiry note."));
    assert!(check_in(&restored).contains("First inquiry's tentative stub conclusion."));
    assert!(!restored.text.contains("Second inquiry note."));
    assert!(
        !restored
            .text
            .contains("Second inquiry's tentative caller conclusion.")
    );
    let parked = reader
        .prepare_action("SELF_STUDY QUESTION PARK q1")
        .unwrap();
    assert!(parked.question_id.is_none());
    assert!(check_in(&parked).contains("General browsing note."));
    assert!(!parked.text.contains("First inquiry note."));
    assert!(
        !parked
            .text
            .contains("First inquiry's tentative stub conclusion.")
    );
    let second_again = reader.prepare_action("SELF_STUDY QUESTION q2").unwrap();
    assert!(check_in(&second_again).contains("Second inquiry note."));
    assert!(check_in(&second_again).contains("Second inquiry's tentative caller conclusion."));
    assert!(!second_again.text.contains("General browsing note."));
    assert_eq!(
        state(&temp)["questions"]["entries"]["q1"]["status"],
        "parked"
    );
    assert_eq!(state(&temp)["questions"]["entries"]["q2"]["status"], "open");

    // A pending source input retains its own inquiry even when another one is
    // selected before completion. Its check-in must not borrow the new focus.
    let first_again = reader.prepare_action("SELF_STUDY QUESTION q1").unwrap();
    accept(
        &reader,
        &first_again,
        "I am returning to the first inquiry.",
    );
    let pending = open(&reader, SOURCE);
    let switch = reader.prepare_action("SELF_STUDY QUESTION q2").unwrap();
    accept(&reader, &switch, "The second inquiry remains independent.");
    let resumed = reader.prepare_action("SELF_STUDY CONTINUE").unwrap();
    assert_eq!(resumed.page.as_ref().unwrap().id, pending.page.unwrap().id);
    assert_eq!(resumed.question_id.as_deref(), Some("q1"));
    assert!(check_in(&resumed).contains("First inquiry note."));
    assert!(check_in(&resumed).contains("First inquiry's tentative stub conclusion."));
    assert!(!resumed.text.contains("Second inquiry note."));
    assert!(
        !resumed
            .text
            .contains("Second inquiry's tentative caller conclusion.")
    );
    assert_eq!(state(&temp)["questions"]["active"], "q2");
}

#[test]
fn omitting_only_the_check_in_cannot_earn_source_or_navigation_delivery_credit() {
    let (temp, reader) = setup();
    let first = open(&reader, SOURCE);
    accept(
        &reader,
        &first,
        &format!(
            "STUDY_QUESTION: Who calls this?\nSTUDY_NOTE: Caller unknown.\nSTUDY_FINDING: {SOURCE}:1 | A stub is supplied here."
        ),
    );
    for action in [
        format!("SELF_STUDY OPEN {SECOND} 1"),
        "SELF_STUDY MAP".into(),
    ] {
        let output = reader.prepare_action(&action).unwrap();
        let start = output.text.find(CHECK_IN).unwrap();
        let end = output.text.find(CHECK_IN_END).unwrap() + CHECK_IN_END.len();
        let mut shortened = output.text.clone();
        shortened.replace_range(start..end, "");
        assert!(shortened.contains(NOTEBOOK));
        let (request, response) = wire(&output, &shortened, "A shortened-input account.");
        if let Some(page) = &output.page {
            assert!(shortened.contains(&page.text));
            assert!(page.verify_delivery(&request, &response).is_ok());
        }
        let before = state_bytes(&temp);
        assert!(!deliver(
            &reader,
            &output,
            &shortened,
            "A shortened-input account."
        ));
        assert_eq!(state_bytes(&temp), before);
        accept(&reader, &output, "The complete input arrived.");
    }
}

#[test]
fn escaped_maximum_authored_fields_fit_source_and_session_without_losing_saved_words() {
    let temp = tempfile::tempdir().unwrap();
    let root = temp.path().join("astrid");
    let relative = format!(
        "crates/{}/src/{}/quoted.rs",
        "a".repeat(240),
        "b".repeat(30)
    );
    let source = format!("astrid/{relative}");
    assert_eq!(source.len(), 299);
    let path = root.join(&relative);
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    let mut code = String::new();
    for line in 1..=6 {
        writeln!(
            code,
            "pub fn quoted_{line}() {{ let text = \"{}\"; }}",
            "\\\"".repeat(90)
        )
        .unwrap();
    }
    code.push_str(&"pub fn padding() {}\n".repeat(1000));
    fs::write(&path, code).unwrap();
    let reader = Reader::new(
        Catalog::new(BTreeMap::from([("astrid".into(), root)])).unwrap(),
        temp.path().join("reader"),
    );
    let first = open(&reader, &source);
    let words = "\\\"".repeat(300);
    let note = "\\\"".repeat(800);
    let question = "\\\"".repeat(250);
    assert_eq!(words.len(), 600);
    assert_eq!(note.len(), 1600);
    assert_eq!(question.len(), 500);
    let mut content = format!("STUDY_NOTE: {note}\nSTUDY_QUESTION: {question}\n");
    for line in 1..=6 {
        writeln!(content, "STUDY_FINDING: {source}:{line} | {words}").unwrap();
    }
    accept(&reader, &first, &content);
    let saved = state(&temp)["notebook"].clone();
    let authored = &saved["source_findings"]["authored"];
    assert_eq!(authored.as_array().unwrap().len(), 6);
    assert!(
        authored
            .as_array()
            .unwrap()
            .iter()
            .all(|f| f["words"] == words)
    );

    // Force recent-response pressure too; it may compact presentation, but
    // authored note, question and findings must remain complete and durable.
    for turn in 0..4 {
        let next = reader.prepare_action("SELF_STUDY MAP").unwrap();
        accept(
            &reader,
            &next,
            &format!("Account {turn}: {}", "p".repeat(7200)),
        );
    }
    for action in [
        format!("SELF_STUDY OPEN {source} 1"),
        format!("SELF_STUDY SESSION OPEN {source} 1 | OPEN {source} 100"),
    ] {
        let output = reader.prepare_action(&action).unwrap();
        assert!(output.text.len() + output.system_prompt.len() + 32 <= MAX_INPUT_BYTES);
        assert!(output.page.is_some() || output.session_pages.len() == 2);
        let checkpoint_start = output.text.find(CHECK_IN).unwrap();
        let checkpoint_end = output.text.find(CHECK_IN_END).unwrap() + CHECK_IN_END.len();
        let checkpoint = &output.text[checkpoint_start..checkpoint_end];
        assert!(checkpoint.len() <= 6000);
        assert!(checkpoint.contains("additional finding(s) remain whole in the full notebook"));
        for page in output.page.iter().chain(output.session_pages.iter()) {
            let end = output.text.find(&page.text).unwrap() + page.text.len();
            assert!(end < checkpoint_start);
        }
        let rendered: Value = serde_json::from_str(
            output
                .text
                .split_once(NOTEBOOK)
                .unwrap()
                .1
                .split_once('\n')
                .unwrap()
                .1
                .split_once("\nEnd of study notebook.")
                .unwrap()
                .0,
        )
        .unwrap();
        assert_eq!(rendered["note"]["text"], note);
        assert_eq!(rendered["question"]["text"], question);
        assert_eq!(&rendered["source_findings"]["authored"], authored);
        let durable = state(&temp)["notebook"].clone();
        assert_eq!(durable["note"], saved["note"]);
        assert_eq!(durable["question"], saved["question"]);
        assert_eq!(&durable["source_findings"]["authored"], authored);
        accept(&reader, &output, "No authored update is chosen.");
    }
}
