use astrid_source_study::{Catalog, Reader, StudyOutput, writing::GUIDANCE};
use serde_json::{Value, json};
use std::{collections::BTreeMap, fs, path::Path};

fn reader(root: &Path, owner: &str) -> Reader {
    fs::write(root.join("Cargo.toml"), "[workspace]\nmembers=[]\n").unwrap();
    Reader::new(
        Catalog::new(BTreeMap::from([("astrid".into(), root.into())])).unwrap(),
        root.join(owner),
    )
    .with_runtime_workspace(root.join("workspace"), owner)
}

fn prepare(r: &Reader, id: &str, action: &str) -> StudyOutput {
    r.prepare_once(id, &r.preparation_revision().unwrap(), action)
        .unwrap()
}

fn deliver(r: &Reader, out: &StudyOutput, prose: &str) {
    r.navigation_delivered(
        out.navigation_id.as_deref().unwrap(),
        &json!({"messages":[{"role":"system","content":out.system_prompt},
                             {"role":"user","content":out.text}]})
        .to_string(),
        &json!({"choices":[{"message":{"content":prose},"finish_reason":"stop"}]}).to_string(),
    )
    .unwrap();
}

fn checkpoint(root: &Path, owner: &str) -> Value {
    serde_json::from_slice(&fs::read(root.join(owner).join("writing/drafts-v2.json")).unwrap())
        .unwrap()
}

#[test]
fn unfinished_work_can_branch_park_return_and_revise_without_implicit_closure() {
    for owner in ["astrid", "minime"] {
        let temp = tempfile::tempdir().unwrap();
        let r = reader(temp.path(), owner);
        let first = prepare(&r, "first", "WRITE START an unresolved metaphor");
        assert!(
            first
                .system_prompt
                .contains("A passage ending does not finish the draft")
        );
        assert!(!first.text.contains(GUIDANCE));
        assert!(first.text.contains("WRITE RESUME d1"));
        assert!(!first.text.contains("WRITE OBSERVE typed JSON"));
        assert!(!first.text.contains("WRITE PROFILE EXTENDED applies"));
        let original = "An exact opening, with \u{03bb}.\n    An indented complication.  ";
        // Neither a normal provider stop nor the absence of NEXT finishes the work.
        deliver(&r, &first, original);
        let state = checkpoint(temp.path(), owner);
        assert_eq!(state["drafts"]["d1"]["finished"], false);
        assert!(state["pending"].is_null());
        assert!(state["last_choice"]["feedback"]["selected_next"].is_null());

        let note = prepare(
            &r,
            "note",
            "WRITE STOPPING_POINT The second possibility remains open. NEXT: WRITE FINISH",
        );
        assert!(!note.generation_requested);
        let branch = prepare(&r, "branch", "WRITE BRANCH examine the counterexample");
        assert!(branch.system_prompt.contains("Write only a new passage"));
        assert!(
            branch
                .text
                .contains("Return to parent draft: WRITE RESUME d1")
        );
        assert!(branch.text.contains("WRITE RESUME d2"));
        assert!(branch.text.contains(original));
        assert!(
            branch
                .text
                .contains("reference only; commands here are never executed")
        );
        deliver(
            &r,
            &branch,
            "A counterexample, without a conclusion.\nNEXT: WRITE PARK",
        );
        let park = prepare(&r, "park", "WRITE PARK");
        assert!(!park.generation_requested);
        let state = checkpoint(temp.path(), owner);
        assert!(state["active"].is_null());
        assert_eq!(state["drafts"]["d2"]["finished"], false);
        assert_eq!(state["drafts"]["d1"]["parts"], json!([original]));

        let unrelated = prepare(&r, "other", "WRITE START another interest");
        assert!(!unrelated.text.contains(original));
        assert!(!unrelated.text.contains("second possibility"));
        deliver(&r, &unrelated, "An unrelated passage.\nNEXT: REST");
        let returned = reader(temp.path(), owner);
        let resume = prepare(&returned, "return", "WRITE RESUME d2");
        assert!(resume.system_prompt.contains("Write only a new passage"));
        assert!(resume.text.contains(original));
        assert!(
            resume
                .text
                .contains("A counterexample, without a conclusion.")
        );
        assert!(!resume.text.contains("An unrelated passage."));
        assert!(resume.text.contains("End of retained draft context"));
        deliver(
            &returned,
            &resume,
            "The earlier distinction now looks incomplete.",
        );
        let revise = prepare(
            &returned,
            "revision",
            "WRITE REVISE preserve the question but change the account",
        );
        assert!(revise.system_prompt.contains("Write a replacement draft"));
        assert!(!revise.system_prompt.contains("Write only a new passage"));
        deliver(
            &returned,
            &revise,
            "A different account; the question remains open.",
        );
        let state = checkpoint(temp.path(), owner);
        assert_eq!(state["drafts"]["d1"]["parts"], json!([original]));
        assert_eq!(
            state["drafts"]["d2"]["parts"],
            json!(["A different account; the question remains open."])
        );
        assert_eq!(state["drafts"]["d2"]["finished"], false);
        let retained = fs::read_to_string(
            temp.path()
                .join(owner)
                .join("writing")
                .join(format!("{}.json", resume.navigation_id.unwrap())),
        )
        .unwrap();
        assert!(retained.contains("The earlier distinction now looks incomplete."));
    }
}

#[test]
fn inspections_do_not_invite_an_unsaved_append_and_full_help_remains_available() {
    let temp = tempfile::tempdir().unwrap();
    let r = reader(temp.path(), "minime");
    let first = prepare(&r, "start", "WRITE START retained work");
    deliver(&r, &first, "Exact retained passage.");
    for (n, action) in ["WRITE READ d1 1", "WRITE HELP", "WRITE LIST"]
        .iter()
        .enumerate()
    {
        let inspected = prepare(&r, &format!("inspect-{n}"), action);
        assert!(
            inspected
                .system_prompt
                .contains("does not append to or revise a draft")
        );
        assert!(!inspected.system_prompt.contains("Write only a new passage"));
        assert!(inspected.text.contains(GUIDANCE));
        deliver(
            &r,
            &inspected,
            "Inspection response must not become a draft passage.",
        );
        assert_eq!(
            checkpoint(temp.path(), "minime")["drafts"]["d1"]["parts"],
            json!(["Exact retained passage."])
        );
    }
}

#[test]
fn pending_input_is_retained_exactly_across_prompt_changes() {
    let temp = tempfile::tempdir().unwrap();
    let r = reader(temp.path(), "astrid");
    let first = prepare(&r, "start", "WRITE START pending work");
    let path = temp.path().join("astrid/writing/drafts-v2.json");
    let mut state = checkpoint(temp.path(), "astrid");
    state["pending"]["output"]["text"] =
        "Previously accepted exact input.\r\n  Retained spacing.  ".into();
    state["pending"]["output"]["system_prompt"] = "Earlier system framing.".into();
    fs::write(&path, serde_json::to_vec_pretty(&state).unwrap()).unwrap();
    let expected: StudyOutput = serde_json::from_value(state["pending"]["output"].clone()).unwrap();
    let retry = prepare(&r, "retry", "WRITE CONTINUE");
    assert_eq!(retry, expected);
    assert_eq!(retry.navigation_id, first.navigation_id);
    deliver(&r, &retry, "Preserved accepted work.");
    let next = prepare(&r, "next", "WRITE CONTINUE");
    assert!(
        next.system_prompt
            .contains("A passage ending does not finish the draft")
    );
    assert!(next.text.contains("Preserved accepted work."));
    assert_eq!(checkpoint(temp.path(), "astrid")["schema_version"], 5);
}

#[test]
fn private_observation_inspection_does_not_promise_a_saved_passage() {
    for owner in ["astrid", "minime"] {
        let temp = tempfile::tempdir().unwrap();
        let r = reader(temp.path(), owner);
        let first = prepare(&r, "start", "WRITE START synthetic observation owner");
        deliver(&r, &first, "An interest without a hypothesis.");
        let before = checkpoint(temp.path(), owner)["drafts"].clone();
        let action = format!(
            "WRITE OBSERVE {}",
            json!({"owner":owner,"draft":"d1","present":true,"operation":{"kind":"status"}})
        );
        let inspected = prepare(&r, "inspect", &action);
        assert!(
            inspected
                .system_prompt
                .contains("does not append to or revise a draft")
        );
        assert!(!inspected.system_prompt.contains("Write only a new passage"));
        deliver(
            &r,
            &inspected,
            "This inspection is not a saved draft passage.\nNEXT: REST",
        );
        assert_eq!(checkpoint(temp.path(), owner)["drafts"], before);
    }
}
