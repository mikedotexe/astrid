use astrid_source_study::{
    ActivityRequest, ActivityResponse, Catalog, InputKind, Reader, StudyOutput,
};
use serde_json::{Value, json};
use std::{collections::BTreeMap, fs, path::Path};

const SOURCE: &str = "astrid/crates/example/src/first.rs";
const OTHER: &str = "astrid/crates/example/src/second.rs";

fn setup() -> tempfile::TempDir {
    let root = tempfile::tempdir().unwrap();
    for (path, name) in [(SOURCE, "first"), (OTHER, "second")] {
        let path = root.path().join(path);
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(path, format!("pub fn {name}() {{}}\n").repeat(900)).unwrap();
    }
    root
}

fn reader(root: &Path, owner: &str) -> Reader {
    Reader::new(
        Catalog::new(BTreeMap::from([("astrid".into(), root.join("astrid"))])).unwrap(),
        root.join(owner),
    )
    .with_runtime_workspace(root.join("workspace"), owner)
}

fn prepare(reader: &Reader, id: &str, action: &str) -> StudyOutput {
    reader
        .prepare_once(id, &reader.preparation_revision().unwrap(), action)
        .unwrap()
}

fn accept(reader: &Reader, output: &StudyOutput, text: &str) {
    let request = json!({"messages":[{"role":"user","content":output.text}]}).to_string();
    let response = json!({"message":{"content":text},"done":true}).to_string();
    if let Some(page) = &output.page {
        reader.delivered(&page.id, &request, &response).unwrap();
    } else {
        reader
            .navigation_delivered(
                output.navigation_id.as_deref().unwrap(),
                &request,
                &response,
            )
            .unwrap();
    }
}

fn checkpoint(root: &Path, owner: &str, name: &str) -> Value {
    serde_json::from_slice(&fs::read(root.join(owner).join(name)).unwrap()).unwrap()
}

fn review(reader: &Reader, id: &str, question: &str) -> (StudyOutput, Value) {
    let output = prepare(
        reader,
        id,
        &format!("SELF_STUDY QUESTION REVIEW {question}"),
    );
    assert_eq!(output.input_kind, InputKind::InquiryReview);
    let start = output.text.find('{').unwrap();
    let value = serde_json::Deserializer::from_str(&output.text[start..])
        .into_iter::<Value>()
        .next()
        .unwrap()
        .unwrap();
    (output, value)
}

fn status(reader: &Reader, at: u64) -> ActivityResponse {
    reader
        .activity("status", None, at, ActivityRequest::Status)
        .unwrap()
}

fn command(reader: &Reader, id: &str, at: u64, action: &str) -> ActivityResponse {
    reader
        .activity(
            id,
            Some(status(reader, at).revision),
            at,
            ActivityRequest::Command {
                action: action.into(),
            },
        )
        .unwrap()
}

#[test]
#[allow(clippy::too_many_lines)] // One ordered workflow checks selection, delivery and budget together.
fn both_owners_inspect_return_continue_park_and_decline_without_budget_renewal() {
    for owner in ["astrid", "minime"] {
        let temp = setup();
        let r = reader(temp.path(), owner);
        let first = prepare(&r, "start", "WRITE START a private unfinished thought");
        accept(&r, &first, "EXACT PRIVATE PROSE\nNEXT: WRITE CONTINUE");
        let note = prepare(
            &r,
            "note",
            "WRITE STOPPING_POINT PRIVATE NOTE; NEXT: WRITE FINISH",
        );
        assert!(!note.generation_requested);
        let initial = checkpoint(temp.path(), owner, "writing/drafts-v2.json");
        let inspect = prepare(&r, "inspect", "WRITE READ d1 1");
        assert!(inspect.generation_requested);
        assert!(inspect.text.contains("\"revision\": 1"));
        assert!(inspect.text.contains("\"stopping_point_available\": true"));
        assert!(inspect.text.contains("PRIVATE NOTE"));
        accept(&r, &inspect, "STUDY_NOTE: not a draft edit.\nNEXT: REST");
        assert_eq!(
            initial["drafts"],
            checkpoint(temp.path(), owner, "writing/drafts-v2.json")["drafts"]
        );

        let focused = command(&r, "focus", 100, "ACTIVITY_FOCUS WRITE d1 turns 2");
        assert!(focused.target_state.is_none());
        assert!(
            r.activity("next", None, 100, ActivityRequest::Next)
                .unwrap()
                .target_state
                .is_none()
        );
        let resumed = prepare(&r, "resume", "WRITE RESUME d1");
        assert!(resumed.text.contains("requests a new appended passage"));
        let admitted = r
            .activity(
                "admit",
                Some(focused.revision),
                101,
                ActivityRequest::Admit {
                    job_id: "job".into(),
                    input_id: resumed.navigation_id.clone().unwrap(),
                    action: "WRITE RESUME d1".into(),
                },
            )
            .unwrap();
        assert_eq!(admitted.admitted, 1);
        assert!(admitted.target_state.is_none());
        accept(&r, &resumed, "An appended passage.\nNEXT: WRITE CONTINUE");
        let completed = r
            .activity(
                "complete",
                Some(admitted.revision),
                102,
                ActivityRequest::Complete {
                    job_id: "job".into(),
                    input_id: resumed.navigation_id.clone().unwrap(),
                },
            )
            .unwrap();
        let parked = command(&r, "park", 103, "PARK_ACTIVITY");
        assert!(completed.target_state.is_none());
        assert!(parked.target_state.is_none());
        let unrelated = prepare(&r, "unrelated", "WRITE START independent interest");
        assert!(!unrelated.text.contains("PRIVATE NOTE"));
        assert!(!unrelated.text.contains("EXACT PRIVATE PROSE"));
        accept(&r, &unrelated, "Unrelated prose.");
        let before = checkpoint(temp.path(), owner, "writing/drafts-v2.json");
        let returned = reader(temp.path(), owner);
        let inspect = prepare(&returned, "inspect-parked", "WRITE READ d1 1");
        accept(&returned, &inspect, "No return selected.\nNEXT: REST");
        let after = checkpoint(temp.path(), owner, "writing/drafts-v2.json");
        assert_eq!(before["active"], after["active"]);
        assert_eq!(before["drafts"], after["drafts"]);
        let still_parked = status(&returned, 104);
        assert_eq!(parked.return_command, still_parked.return_command);
        assert_eq!(completed.admitted, still_parked.admitted);
        assert_eq!(completed.remaining, still_parked.remaining);
        assert_eq!(completed.deadline_ms, still_parked.deadline_ms);
        let public = serde_json::to_string(&still_parked).unwrap();
        assert!(!public.contains("PRIVATE NOTE"));
        assert!(!public.contains("EXACT PRIVATE PROSE"));
        assert!(!public.contains("private unfinished thought"));
        let return_action = still_parked.return_command.unwrap();
        let returned_state = command(&returned, "return", 105, &return_action);
        assert!(!returned_state.protected);
        assert_eq!(returned_state.admitted, 1);
        assert_eq!(returned_state.remaining, 1);
        assert_eq!(returned_state.deadline_ms, completed.deadline_ms);
        let returned_facts = returned_state.target_state.as_ref().unwrap();
        assert_eq!(returned_facts["draft_id"], "d1");
        assert_eq!(returned_facts["revision"], 2);
        assert_eq!(returned_facts["selected"], true);
        assert_eq!(returned_facts["stopping_point_available"], true);
        assert_eq!(returned_facts["pending_delivery_count"], 0);
        assert_eq!(
            returned_facts["authored_revision"],
            still_parked.target_state.as_ref().unwrap()["authored_revision"]
        );
        let focus = checkpoint(temp.path(), owner, "activity-focus-v1.json");
        assert_eq!(
            focus["focus"]["window"]["id"],
            completed.window_id.as_deref().unwrap()
        );
        assert_eq!(
            focus["focus"]["window"]["ended"],
            "explicit_return_without_focus"
        );
        let native = fs::read(temp.path().join(owner).join("writing/drafts-v2.json")).unwrap();
        let retry = returned
            .activity(
                "return",
                Some(still_parked.revision),
                106,
                ActivityRequest::Command {
                    action: return_action,
                },
            )
            .unwrap();
        assert_eq!(retry.revision, returned_state.revision);
        assert_eq!(
            fs::read(temp.path().join(owner).join("writing/drafts-v2.json")).unwrap(),
            native
        );
        assert_eq!(
            checkpoint(temp.path(), owner, "writing/drafts-v2.json")["drafts"],
            before["drafts"]
        );
        let continued = prepare(&returned, "continue", "WRITE CONTINUE");
        assert!(continued.text.contains("An appended passage."));
        assert!(!continued.text.contains("Unrelated prose."));
        accept(&returned, &continued, "A deliberately continued passage.");
        prepare(&returned, "finish", "WRITE FINISH");
        let finished = checkpoint(temp.path(), owner, "writing/drafts-v2.json");
        let inspect = prepare(&returned, "inspect-finished", "WRITE READ d1");
        accept(&returned, &inspect, "NEXT: REST");
        assert_eq!(
            finished["drafts"],
            checkpoint(temp.path(), owner, "writing/drafts-v2.json")["drafts"]
        );
        assert_eq!(status(&returned, 107).admitted, 1);
    }
}

#[test]
fn pending_writing_inputs_survive_inspection_duplicate_delivery_and_restart() {
    for owner in ["astrid", "minime"] {
        let temp = setup();
        let r = reader(temp.path(), owner);
        let pending = prepare(&r, "start", "WRITE START retained pending work");
        let initial = checkpoint(temp.path(), owner, "writing/drafts-v2.json");
        command(&r, "focus", 10, "ACTIVITY_FOCUS WRITE d1");
        let facts = status(&r, 11).target_state.unwrap();
        assert_eq!(facts["pending_delivery_count"], 1);
        assert_eq!(
            facts["pending_deliveries"][0]["input_id"],
            pending.navigation_id.as_deref().unwrap()
        );
        assert_eq!(facts["stopping_point_available"], false);
        let revision = r.preparation_revision().unwrap();
        let inspect = r
            .prepare_once("inspect", &revision, "WRITE READ d1")
            .unwrap();
        let r = reader(temp.path(), owner);
        assert_eq!(
            inspect,
            r.prepare_once("inspect", &revision, "WRITE READ d1")
                .unwrap()
        );
        let native = checkpoint(temp.path(), owner, "writing/drafts-v2.json");
        assert_eq!(
            native["retained_pending"][pending.navigation_id.as_ref().unwrap()],
            initial["pending"]
        );
        accept(&r, &inspect, "Decline continuation.\nNEXT: REST");
        accept(
            &r,
            &pending,
            "Accepted original pending passage.\nNEXT: WRITE CONTINUE",
        );
        accept(
            &r,
            &pending,
            "Accepted original pending passage.\nNEXT: WRITE CONTINUE",
        );
        let native = checkpoint(temp.path(), owner, "writing/drafts-v2.json");
        assert_eq!(
            native["drafts"]["d1"]["parts"],
            json!(["Accepted original pending passage."])
        );
        assert_eq!(native["drafts"]["d1"]["revision"], 1);
        assert_eq!(status(&r, 12).admitted, 0);
        assert_eq!(native["schema_version"], initial["schema_version"]);
    }
}

#[test]
fn inquiry_review_uses_its_own_cursor_and_stays_quiet_after_declining_return() {
    for owner in ["astrid", "minime"] {
        let temp = setup();
        let r = reader(temp.path(), owner);
        prepare(&r, "question", "SELF_STUDY QUESTION NEW First inquiry?");
        let page = prepare(&r, "open", &format!("SELF_STUDY OPEN {SOURCE} 1"));
        accept(
            &r,
            &page,
            "STUDY_NOTE: First retained note.\nNEXT: SELF_STUDY CONTINUE",
        );
        command(&r, "focus", 10, "ACTIVITY_FOCUS QUESTION q1");
        let (_, inspected) = review(&r, "inspect", "q1");
        let facts = &inspected["return_support"];
        assert_eq!(
            facts["saved_position"]["page_id"],
            page.page.as_ref().unwrap().id
        );
        assert_eq!(
            facts["saved_position"]["next_byte_position"],
            json!(page.page.as_ref().unwrap().end)
        );
        assert_eq!(
            facts["saved_position"]["source_status"],
            "verified_matching"
        );
        assert_eq!(facts["continue_after_selection"], "SELF_STUDY CONTINUE");
        let continued = prepare(&r, "continue", "SELF_STUDY CONTINUE");
        assert_eq!(
            continued.page.as_ref().unwrap().start,
            page.page.as_ref().unwrap().end
        );
        accept(&r, &continued, "STUDY_NOTE: Still uncertain.");
        let parked = command(&r, "park", 11, "PARK_ACTIVITY");
        prepare(
            &r,
            "other-question",
            "SELF_STUDY QUESTION NEW Second inquiry?",
        );
        let other = prepare(&r, "other-open", &format!("SELF_STUDY OPEN {OTHER} 400"));
        accept(&r, &other, "STUDY_NOTE: Independent inquiry note.");
        let before = checkpoint(temp.path(), owner, "reader-v1.json");
        let r = reader(temp.path(), owner);
        let (inspection, inspected) = review(&r, "inspect-parked", "q1");
        assert_eq!(
            inspected["return_support"]["saved_position"]["page_id"],
            continued.page.as_ref().unwrap().id
        );
        assert!(!inspection.text.contains(OTHER));
        assert!(!inspection.text.contains("Independent inquiry note"));
        accept(
            &r,
            &inspection,
            "STUDY_NOTE: must not revise either inquiry.\nNEXT: REST",
        );
        let after = checkpoint(temp.path(), owner, "reader-v1.json");
        assert_eq!(before["questions"], after["questions"]);
        assert_eq!(before["notebook"], after["notebook"]);
        assert_eq!(before["bookmarks"], after["bookmarks"]);
        assert_eq!(
            before["reading_contexts"]["q1"],
            after["reading_contexts"]["q1"]
        );
        let inspected_status = status(&r, 12);
        assert_eq!(parked.return_command, inspected_status.return_command);
        assert_eq!(parked.remaining, inspected_status.remaining);
        assert_eq!(parked.deadline_ms, inspected_status.deadline_ms);
        let returned = command(&r, "return", 13, parked.return_command.as_deref().unwrap());
        assert!(!returned.protected);
        let returned_facts = returned.target_state.as_ref().unwrap();
        assert_eq!(returned_facts["question_id"], "q1");
        assert_eq!(returned_facts["selected"], true);
        assert_eq!(
            returned_facts["saved_position"]["page_id"],
            continued.page.as_ref().unwrap().id
        );
        assert_eq!(
            returned_facts["authored_revision"],
            inspected_status.target_state.as_ref().unwrap()["authored_revision"]
        );
        let next = prepare(&r, "continue-returned", "SELF_STUDY CONTINUE");
        assert_eq!(
            next.page.as_ref().unwrap().start,
            continued.page.as_ref().unwrap().end
        );
        accept(&r, &next, "STUDY_NOTE: A deliberate continuation.");
        prepare(
            &r,
            "resolve",
            "SELF_STUDY QUESTION RESOLVE q1 Uncertain finding.",
        );
        let before = checkpoint(temp.path(), owner, "reader-v1.json");
        let (inspection, _) = review(&r, "inspect-resolved", "q1");
        accept(&r, &inspection, "NEXT: REST");
        assert_eq!(
            before["questions"],
            checkpoint(temp.path(), owner, "reader-v1.json")["questions"]
        );
    }
}

#[test]
fn review_preserves_exact_pending_source_input_and_does_not_borrow_another_cursor() {
    for owner in ["astrid", "minime"] {
        let temp = setup();
        let r = reader(temp.path(), owner);
        prepare(&r, "question", "SELF_STUDY QUESTION NEW Pending inquiry?");
        let pending = prepare(&r, "open", &format!("SELF_STUDY OPEN {SOURCE} 1"));
        let before = checkpoint(temp.path(), owner, "reader-v1.json");
        let revision = r.preparation_revision().unwrap();
        let (inspection, inspected) = review(&r, "review-pending", "q1");
        let facts = &inspected["return_support"];
        assert!(facts["saved_position"].is_null());
        assert_eq!(
            facts["pending_source_delivery"][0]["input_id"],
            pending.page.as_ref().unwrap().id
        );
        assert_eq!(facts["continuation"], "pending_source_delivery");
        accept(&r, &inspection, "STUDY_NOTE: never saved.\nNEXT: REST");
        let r = reader(temp.path(), owner);
        assert_eq!(
            inspection,
            r.prepare_once("review-pending", &revision, "SELF_STUDY QUESTION REVIEW q1")
                .unwrap()
        );
        let retry = prepare(&r, "continue", "SELF_STUDY CONTINUE");
        assert_eq!(pending, retry);
        let after = checkpoint(temp.path(), owner, "reader-v1.json");
        assert_eq!(before["pending_page_output"], after["pending_page_output"]);
        assert_eq!(before["questions"], after["questions"]);
        accept(&r, &pending, "Retained source delivery.");
        accept(&r, &pending, "Retained source delivery.");
        prepare(
            &r,
            "empty-question",
            "SELF_STUDY QUESTION NEW No position yet?",
        );
        prepare(&r, "select-first", "SELF_STUDY QUESTION q1");
        let (_, empty) = review(&r, "inspect-empty", "q2");
        assert!(empty["return_support"]["saved_position"].is_null());
        assert_eq!(
            empty["return_support"]["pending_source_delivery"],
            json!([])
        );
        assert_eq!(
            empty["return_support"]["continuation"],
            "source_selection_required"
        );
        assert!(!serde_json::to_string(&empty).unwrap().contains(SOURCE));
    }
}

#[test]
fn changed_missing_and_unreadable_sources_keep_history_and_offer_existing_reselection() {
    for owner in ["astrid", "minime"] {
        for condition in ["changed", "missing", "unreadable"] {
            let temp = setup();
            let r = reader(temp.path(), owner);
            prepare(&r, "question", "SELF_STUDY QUESTION NEW Historical source?");
            let page = prepare(&r, "open", &format!("SELF_STUDY OPEN {SOURCE} 1"));
            accept(&r, &page, "STUDY_NOTE: Saved before drift.");
            command(&r, "focus", 10, "ACTIVITY_FOCUS QUESTION q1");
            command(&r, "park", 11, "PARK_ACTIVITY");
            let before = checkpoint(temp.path(), owner, "reader-v1.json");
            match condition {
                "changed" => fs::write(temp.path().join(SOURCE), "pub fn changed() {}\n").unwrap(),
                "missing" => fs::remove_file(temp.path().join(SOURCE)).unwrap(),
                _ => fs::write(temp.path().join(SOURCE), [0xff, 0xfe]).unwrap(),
            }
            let (_, inspected) = review(&r, "inspect-drift", "q1");
            let facts = &inspected["return_support"];
            let saved = &facts["saved_position"];
            assert_eq!(
                saved["historical_source_revision"],
                page.page.as_ref().unwrap().revision.sha256
            );
            assert_eq!(facts["continuation"], "source_reselection_required");
            assert!(facts["continue_after_selection"].is_null());
            assert_eq!(facts["select_command"], "SELF_STUDY QUESTION q1");
            assert_eq!(facts["browse_command"], "SELF_STUDY MAP");
            if condition == "changed" {
                assert_eq!(saved["source_status"], "verified_changed");
                assert_ne!(
                    saved["currently_verified_revision"],
                    saved["historical_source_revision"]
                );
                assert_eq!(
                    facts["reselect_after_selection"],
                    json!([format!("SELF_STUDY OPEN {SOURCE} 1")])
                );
            } else {
                assert_eq!(saved["source_status"], "unavailable");
                assert!(saved["currently_verified_revision"].is_null());
                assert_eq!(facts["reselect_after_selection"], json!([]));
            }
            assert!(status(&r, 12).return_command.is_none());
            let after = checkpoint(temp.path(), owner, "reader-v1.json");
            assert_eq!(before["questions"], after["questions"]);
            assert_eq!(
                before["reading_contexts"]["q1"],
                after["reading_contexts"]["q1"]
            );
            prepare(&r, "select", facts["select_command"].as_str().unwrap());
            let source = if condition == "changed" {
                SOURCE
            } else {
                OTHER
            };
            let reselected = prepare(&r, "reselect", &format!("SELF_STUDY OPEN {source} 1"));
            assert_eq!(
                reselected.page.as_ref().unwrap().question_id.as_deref(),
                Some("q1")
            );
            assert_eq!(reselected.page.as_ref().unwrap().source, source);
        }
    }
}

#[test]
fn review_reports_each_pending_session_source_without_regenerating_the_session() {
    let temp = setup();
    let r = reader(temp.path(), "minime");
    prepare(&r, "question", "SELF_STUDY QUESTION NEW Two exact sources?");
    let session = prepare(
        &r,
        "session",
        &format!("SELF_STUDY SESSION OPEN {SOURCE} 1 | OPEN {OTHER} 20"),
    );
    let (_, inspected) = review(&r, "inspect", "q1");
    let facts = inspected["return_support"]["pending_source_delivery"]
        .as_array()
        .unwrap();
    assert_eq!(facts.len(), 2);
    for (page, facts) in session.session_pages.iter().zip(facts) {
        assert_eq!(facts["page_id"], page.id);
        assert_eq!(facts["input_id"], session.navigation_id.as_deref().unwrap());
        assert_eq!(facts["historical_source_revision"], page.revision.sha256);
    }
    assert_eq!(session, prepare(&r, "continue", "SELF_STUDY CONTINUE"));
}

#[test]
fn oversized_stopping_notes_remain_exact_and_inspection_stays_bounded() {
    let temp = setup();
    let r = reader(temp.path(), "astrid");
    let draft = prepare(&r, "start", "WRITE START bounded inspection");
    accept(&r, &draft, "Exact draft prose.");
    let note = "Retained stopping note. ".repeat(3_000);
    prepare(&r, "note", &format!("WRITE STOPPING_POINT {note}"));
    let before = checkpoint(temp.path(), "astrid", "writing/drafts-v2.json");
    let inspection = prepare(&r, "inspect", "WRITE READ d1");
    assert!(
        inspection
            .text
            .contains("labelled excerpt; complete note remains stored")
    );
    assert!(inspection.text.len() < astrid_source_study::MAX_INPUT_BYTES);
    accept(&r, &inspection, "NEXT: REST");
    assert_eq!(
        before["drafts"],
        checkpoint(temp.path(), "astrid", "writing/drafts-v2.json")["drafts"]
    );
}

#[test]
fn late_delivery_invalidates_old_return_identity_without_changing_budget_or_selection() {
    for owner in ["astrid", "minime"] {
        let temp = setup();
        let r = reader(temp.path(), owner);
        let pending = prepare(&r, "start", "WRITE START exact pending revision");
        command(&r, "focus", 10, "ACTIVITY_FOCUS WRITE d1 turns 2");
        let parked = command(&r, "park", 11, "PARK_ACTIVITY");
        accept(
            &r,
            &pending,
            "A late accepted passage.\nNEXT: WRITE CONTINUE",
        );
        let before = checkpoint(temp.path(), owner, "writing/drafts-v2.json");
        let current = status(&r, 12);
        assert_ne!(parked.return_command, current.return_command);
        assert_eq!(parked.remaining, current.remaining);
        assert_eq!(parked.deadline_ms, current.deadline_ms);
        assert!(
            r.activity(
                "stale-return",
                Some(current.revision),
                13,
                ActivityRequest::Command {
                    action: parked.return_command.unwrap()
                }
            )
            .is_err()
        );
        assert_eq!(
            before,
            checkpoint(temp.path(), owner, "writing/drafts-v2.json")
        );
        let r = reader(temp.path(), owner);
        let returned = command(
            &r,
            "fresh-return",
            14,
            current.return_command.as_deref().unwrap(),
        );
        assert!(!returned.protected);
        assert_eq!(returned.admitted, parked.admitted);
        assert_eq!(returned.remaining, parked.remaining);
        assert_eq!(returned.target_state.unwrap()["pending_delivery_count"], 0);
    }
}

#[test]
fn review_cannot_hide_genuinely_stale_context_even_when_an_old_helper_omits_the_flag() {
    for owner in ["astrid", "minime"] {
        for legacy in [false, true] {
            let temp = setup();
            let r = reader(temp.path(), owner);
            prepare(&r, "question", "SELF_STUDY QUESTION NEW Original inquiry?");
            let original = prepare(&r, "open", &format!("SELF_STUDY OPEN {SOURCE} 1"));
            let map = prepare(&r, "map", "SELF_STUDY MAP");
            accept(
                &r,
                &map,
                "STUDY_QUESTION: Revised inquiry after navigation?\nSTUDY_NOTE: New authored context.\nNEXT: SELF_STUDY CONTINUE",
            );
            let mut state = checkpoint(temp.path(), owner, "reader-v1.json");
            assert_eq!(state["pending_page_context_stale"], true);
            if legacy {
                state
                    .as_object_mut()
                    .unwrap()
                    .remove("pending_page_context_stale");
                fs::write(
                    temp.path().join(owner).join("reader-v1.json"),
                    serde_json::to_vec(&state).unwrap(),
                )
                .unwrap();
            }
            let (inspection, _) = review(&r, "inspect", "q1");
            accept(
                &r,
                &inspection,
                "STUDY_QUESTION: Must not replace it.\nNEXT: REST",
            );
            let after_review = checkpoint(temp.path(), owner, "reader-v1.json");
            assert_eq!(after_review["pending_page_context_stale"], true);
            assert_eq!(state["questions"], after_review["questions"]);
            let continued = prepare(&r, "continue", "SELF_STUDY CONTINUE");
            assert_eq!(continued.page, original.page);
            assert!(continued.text.contains("Revised inquiry after navigation?"));
            assert!(!continued.text.contains("Must not replace it."));
            // Preserve the complete-input guard: an old wire cannot claim the
            // refreshed context. The newly prepared input is accepted once.
            assert!(
                r.delivered(
                    &original.page.as_ref().unwrap().id,
                    &json!({"messages":[{"role":"user","content":original.text}]}).to_string(),
                    &json!({"message":{"content":"Old incomplete context."},"done":true})
                        .to_string()
                )
                .is_err()
            );
            accept(
                &r,
                &continued,
                "Current exact source delivery.\nNEXT: SELF_STUDY CONTINUE",
            );
            accept(
                &r,
                &continued,
                "Current exact source delivery.\nNEXT: SELF_STUDY CONTINUE",
            );
            let saved = checkpoint(temp.path(), owner, "reader-v1.json");
            assert!(saved["pending"].is_null());
            assert_eq!(saved["receipts"].as_object().unwrap().len(), 1);
        }
    }
}

#[test]
fn late_source_completion_after_review_keeps_newer_choice_and_other_inquiry() {
    for owner in ["astrid", "minime"] {
        let temp = setup();
        let r = reader(temp.path(), owner);
        prepare(
            &r,
            "first",
            "SELF_STUDY QUESTION NEW First pending inquiry?",
        );
        let pending = prepare(&r, "open", &format!("SELF_STUDY OPEN {SOURCE} 1"));
        let (inspection, _) = review(&r, "inspect", "q1");
        accept(&r, &inspection, "NEXT: REST");
        prepare(&r, "second", "SELF_STUDY QUESTION NEW Other inquiry?");
        let other = prepare(&r, "other", &format!("SELF_STUDY OPEN {OTHER} 1"));
        accept(
            &r,
            &other,
            "STUDY_NOTE: Other retained note.\nNEXT: SELF_STUDY MAP",
        );
        let before = checkpoint(temp.path(), owner, "reader-v1.json");
        let r = reader(temp.path(), owner);
        accept(
            &r,
            &pending,
            "STUDY_NOTE: Late first note.\nNEXT: SELF_STUDY CONTINUE",
        );
        accept(
            &r,
            &pending,
            "STUDY_NOTE: Late first note.\nNEXT: SELF_STUDY CONTINUE",
        );
        let after = checkpoint(temp.path(), owner, "reader-v1.json");
        assert_eq!(before["last_choice"], after["last_choice"]);
        assert_eq!(
            before["questions"]["entries"]["q2"],
            after["questions"]["entries"]["q2"]
        );
        assert_eq!(after["questions"]["active"], "q2");
        assert_eq!(after["current"], OTHER);
        let (_, inspected) = review(&r, "inspect-first-again", "q1");
        assert_eq!(
            inspected["return_support"]["saved_position"]["page_id"],
            pending.page.as_ref().unwrap().id
        );
        assert_eq!(
            inspected["return_support"]["pending_source_delivery"],
            json!([])
        );
        assert_eq!(inspected["return_support"]["selected"], false);
    }
}
