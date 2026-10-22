#[test]
fn retained_account_inspection_through_bridge_preserves_parked_and_pending_work() {
    let temp = tempfile::tempdir().unwrap();
    std::fs::write(temp.path().join("Cargo.toml"), "[workspace]\n").unwrap();
    let reader = astrid_source_study::Reader::new(
        astrid_source_study::Catalog::new(std::collections::BTreeMap::from([(
            "astrid".into(), temp.path().into(),
        )])).unwrap(),
        temp.path().join("reader"),
    ).with_runtime_workspace(temp.path().join("workspace"), "astrid");
    let prepare = |action: &str| {
        let mut conv = ConversationState::new(Vec::new(), None);
        next_action::study_navigation::handle_request(&mut conv, "SELF_STUDY", action).unwrap();
        prepare_shared_study_target(&reader, conv.introspect_target).unwrap()
    };
    let deliver = |out: &astrid_source_study::StudyOutput, text: &str| {
        let wire = serde_json::json!({"messages":[
            {"role":"system","content":out.system_prompt},{"role":"user","content":out.text}
        ]}).to_string();
        let response = serde_json::json!({"message":{"content":text},"done":true}).to_string();
        if let Some(page) = &out.page {
            reader.delivered(&page.id, &wire, &response).unwrap();
        } else {
            reader.navigation_delivered(out.navigation_id.as_ref().unwrap(), &wire, &response).unwrap();
        }
    };
    let saved = || -> serde_json::Value {
        serde_json::from_slice(&std::fs::read(temp.path().join("reader/reader-v1.json")).unwrap()).unwrap()
    };
    prepare("SELF_STUDY QUESTION NEW Original question?");
    let page = prepare("SELF_STUDY OPEN astrid/Cargo.toml");
    deliver(&page, "Exact contrary account.\nNEXT: REST");
    prepare("SELF_STUDY QUESTION PARK q1");
    prepare("SELF_STUDY QUESTION NEW Unrelated question?");
    let pending = prepare("SELF_STUDY OPEN astrid/Cargo.toml");
    let before = saved();
    let list = prepare("SELF_STUDY QUESTION ACCOUNTS q1");
    assert_eq!(list.input_kind, astrid_source_study::InputKind::InquiryReview);
    assert!(!list.text.contains("Exact contrary account."));
    let value = serde_json::Deserializer::from_str(list.text.split_once("\n\n").unwrap().1)
        .into_iter::<serde_json::Value>().next().unwrap().unwrap();
    let command = value["newest_first"][0]["inspect_command"].as_str().unwrap();
    let account = prepare(command);
    assert!(account.text.contains("Exact contrary account."));
    assert!(account.page.is_none() && account.question_id.is_none());
    deliver(&account, "STUDY_NOTE: Not a saved revision.\nNEXT: REST");
    let after = saved();
    for key in ["questions", "notebook", "bookmarks", "progress", "last_input", "cursor_owner"] {
        assert!(before[key] == after[key], "changed {key}");
    }
    assert_eq!(prepare("SELF_STUDY CONTINUE").page, pending.page);
}
