use super::*;

fn store(label: &str) -> ActionContinuityStore {
    ActionContinuityStore::new(std::env::temp_dir().join(format!(
        "astrid_session_contract_{label}_{}_{}",
        std::process::id(),
        now_millis()
    )))
}

#[test]
fn session_missing_input_is_not_handled_or_persisted() {
    let store = store("missing");
    let thread = store.create_thread(None, "Missing input", None).unwrap();
    let result = NextActionOutcome::continuity_result(
        store.continuity_session_capture_command("latest"),
        "summary",
    );
    assert_eq!(result.status, "needs_input");
    assert!(!result.handled);
    store
        .continuity_session_start_command("current :: title: A question")
        .unwrap();
    let before = fs::read(store.continuity_sessions_path(&thread.thread_id)).unwrap();
    for payload in [
        "",
        "summary: ...",
        "next: INTROSPECT regulator 400",
        "source_refs: regulator.rs",
    ] {
        let result = NextActionOutcome::continuity_result(
            store.continuity_session_capture_command(&format!("latest :: {payload}")),
            "summary",
        );
        assert_eq!(result.status, "needs_input");
        assert!(!result.handled);
        assert_eq!(
            fs::read(store.continuity_sessions_path(&thread.thread_id)).unwrap(),
            before
        );
    }
    fs::remove_dir_all(store.root()).unwrap();
}

#[test]
fn session_bookmark_survives_parking_and_explicit_return() {
    let store = store("roundtrip");
    let thread = store
        .create_thread(None, "Returnable inquiry", None)
        .unwrap();
    store
        .continuity_session_start_command(
            "current :: title: Shift invariance; focus: compare formulas",
        )
        .unwrap();
    store.continuity_session_capture_command("latest :: summary: Read the next section; question: Which formula is invariant?; source_refs: regulator.rs@sha256:fixture; artifact_refs: replay.json; next: INTROSPECT regulator 400").unwrap();
    let source = store
        .resolve_continuity_session(&thread, Some("latest"))
        .unwrap()
        .unwrap();
    let id = source["session_id"].as_str().unwrap();
    store
        .continuity_session_summarize_command("latest :: summary: Compare formulas next")
        .unwrap();
    store
        .continuity_session_finalize_command(
            "latest :: outcome: park; return_cue: when I choose to return",
        )
        .unwrap();
    let parked = store
        .resolve_continuity_session(&thread, Some("latest"))
        .unwrap()
        .unwrap();
    for field in [
        "focus",
        "open_questions",
        "source_refs",
        "artifact_refs",
        "suggested_next",
    ] {
        assert_eq!(parked[field], source[field], "{field}");
    }
    assert_eq!(parked["automatic_return"], false);
    assert_eq!(parked["return_cue"], "when I choose to return");
    assert!(
        store
            .continuity_session_summary_v1(&thread, None, 8)
            .unwrap()["active_session"]
            .is_null()
    );
    assert!(store.continuity_session_line(&thread, None).is_empty());
    assert!(
        store
            .continuity_session_capture_command("latest :: summary: not an explicit resume")
            .is_err()
    );
    assert!(
        store
            .continuity_session_summarize_command("latest :: summary: not an explicit resume")
            .is_err()
    );
    let before = fs::read(store.continuity_sessions_path(&thread.thread_id)).unwrap();
    for _ in 0..10 {
        store.continuity_session_status_command("latest").unwrap();
    }
    assert_eq!(
        fs::read(store.continuity_sessions_path(&thread.thread_id)).unwrap(),
        before
    );
    let reopened = store.continuity_session_resume_command(id).unwrap();
    assert!(reopened.contains("INTROSPECT regulator 400"));
    let resumed = store
        .resolve_continuity_session(&thread, Some(id))
        .unwrap()
        .unwrap();
    assert_eq!(resumed["source_refs"], source["source_refs"]);
    assert_eq!(resumed["open_questions"], source["open_questions"]);
    assert_eq!(resumed["status"], "active");
    assert_eq!(resumed["authority_change"], false);
    assert_eq!(resumed["peer_mutation"], false);
    fs::remove_dir_all(store.root()).unwrap();
}

#[test]
fn session_activity_uses_latest_state_for_each_identity() {
    let rows = vec![
        json!({"session_id":"one", "status":"active"}),
        json!({"session_id":"two", "status":"active"}),
        json!({"session_id":"two", "status":"parked"}),
    ];
    assert_eq!(latest_active_session(&rows).unwrap()["session_id"], "one");
    assert!(
        latest_active_session(&[
            json!({"session_id":"one", "status":"active"}),
            json!({"session_id":"one", "status":"held"})
        ])
        .is_none()
    );
}

#[test]
fn session_explicit_bookmark_outlives_recent_projection_window() {
    let store = store("old_bookmark");
    let thread = store.create_thread(None, "Older inquiry", None).unwrap();
    store
        .continuity_session_start_command("current :: title: Older question")
        .unwrap();
    store
        .continuity_session_capture_command(
            "latest :: summary: My stopping point; next: INTROSPECT regulator 800",
        )
        .unwrap();
    store
        .continuity_session_finalize_command("latest :: outcome: park")
        .unwrap();
    let bookmark = store
        .resolve_continuity_session(&thread, Some("latest"))
        .unwrap()
        .unwrap();
    let path = store.continuity_sessions_path(&thread.thread_id);
    let mut file = OpenOptions::new().append(true).open(&path).unwrap();
    for index in 0..260 {
        writeln!(file, "{}", json!({"record_schema":"continuity_session_v1", "record_type":"session_start", "session_id":format!("fixture_{index}"), "status":"complete"})).unwrap();
    }
    drop(file);
    let status = store
        .continuity_session_status_command(bookmark["session_id"].as_str().unwrap())
        .unwrap();
    let projected: Value =
        serde_json::from_str(status.strip_prefix("continuity_session_v1:\n").unwrap()).unwrap();
    assert_eq!(projected["latest_session"], bookmark);
    assert_eq!(projected["session_count"], 261);
    let unknown = store
        .continuity_session_summary_v1(&thread, Some("missing_bookmark"), 8)
        .unwrap();
    assert!(unknown["latest_session"].is_null());
    assert!(unknown["recent_records"].as_array().unwrap().is_empty());
    let reply = store
        .continuity_session_resume_command(bookmark["session_id"].as_str().unwrap())
        .unwrap();
    assert!(reply.contains("My stopping point"));
    assert!(reply.contains("INTROSPECT regulator 800"));
    fs::remove_dir_all(store.root()).unwrap();
}

#[test]
fn session_status_is_pure_even_without_a_store_or_with_stale_projections() {
    let store = store("pure_status");
    assert!(!store.root().exists());
    assert!(
        store
            .continuity_session_status_command("latest")
            .unwrap()
            .contains("nothing was created")
    );
    assert!(!store.root().exists());
    let thread = store.create_thread(None, "Pure status", None).unwrap();
    store
        .continuity_session_start_command("current :: title: Read quietly")
        .unwrap();
    let thread_path = store.thread_dir(&thread.thread_id).join("thread.json");
    let mut snapshot: Value = serde_json::from_slice(&fs::read(&thread_path).unwrap()).unwrap();
    snapshot["projection_freshness_v1"] = Value::Null;
    fs::write(&thread_path, serde_json::to_vec(&snapshot).unwrap()).unwrap();
    let before = fs::read(&thread_path).unwrap();
    let index_before = fs::read(store.index_path()).unwrap();
    let log_before = fs::read(store.continuity_sessions_path(&thread.thread_id)).unwrap();
    store.continuity_session_status_command("latest").unwrap();
    assert_eq!(fs::read(&thread_path).unwrap(), before);
    assert_eq!(fs::read(store.index_path()).unwrap(), index_before);
    assert_eq!(
        fs::read(store.continuity_sessions_path(&thread.thread_id)).unwrap(),
        log_before
    );
    fs::remove_dir_all(store.root()).unwrap();
}

#[test]
fn stale_session_update_and_incomplete_log_fail_without_appending() {
    let store = store("stale_append");
    let thread = store
        .create_thread(None, "Concurrent update", None)
        .unwrap();
    store
        .continuity_session_start_command("current :: title: Original")
        .unwrap();
    let old = store
        .resolve_continuity_session(&thread, None)
        .unwrap()
        .unwrap();
    store
        .continuity_session_finalize_command("latest :: outcome: park")
        .unwrap();
    let path = store.continuity_sessions_path(&thread.thread_id);
    let before = fs::read(&path).unwrap();
    let mut stale = old.clone();
    stale["record_type"] = json!("session_capture");
    stale["expected_session_record_id"] = old["record_id"].clone();
    stale["record_id"] = json!("stale-capture");
    assert!(store.append_jsonl(&path, &stale).is_err());
    assert_eq!(fs::read(&path).unwrap(), before);
    OpenOptions::new()
        .append(true)
        .open(&path)
        .unwrap()
        .write_all(b"{\"incomplete\"")
        .unwrap();
    let damaged = fs::read(&path).unwrap();
    assert!(store.continuity_session_status_command("latest").is_err());
    assert!(store.append_jsonl(&path, &old).is_err());
    assert_eq!(fs::read(&path).unwrap(), damaged);
    fs::remove_dir_all(store.root()).unwrap();
}

#[test]
fn session_quiet_state_is_not_a_control_plane_route() {
    for session in [
        json!({"status":"parked", "suggested_next":"INTROSPECT regulator 400"}),
        json!({"active_session":null, "latest_session":{"status":"parked"}}),
    ] {
        let control = crate::continuity_control_plane::build_control_plane_v1(
            &json!({"continuity_session_v1":session}),
        );
        assert!(
            !control["route_stack"]
                .as_array()
                .unwrap()
                .iter()
                .any(|route| route["source"] == "continuity_session_v1")
        );
    }
}

#[test]
fn session_historical_record_cannot_bypass_parking() {
    let store = store("historical");
    let thread = store
        .create_thread(None, "Versioned bookmark", None)
        .unwrap();
    store
        .continuity_session_start_command("current :: title: Versioned bookmark")
        .unwrap();
    let old = store
        .resolve_continuity_session(&thread, Some("latest"))
        .unwrap()
        .unwrap();
    let old_id = old["record_id"].as_str().unwrap();
    store
        .continuity_session_capture_command(
            "latest :: summary: Later stopping point; next: INTROSPECT regulator 900",
        )
        .unwrap();
    store
        .continuity_session_finalize_command("latest :: outcome: park")
        .unwrap();
    let before = fs::read(store.continuity_sessions_path(&thread.thread_id)).unwrap();
    assert!(
        store
            .continuity_session_capture_command(&format!("{old_id} :: summary: stale reference"))
            .is_err()
    );
    assert_eq!(
        fs::read(store.continuity_sessions_path(&thread.thread_id)).unwrap(),
        before
    );
    let resumed = store.continuity_session_resume_command(old_id).unwrap();
    assert!(resumed.contains("Later stopping point"));
    assert!(resumed.contains("INTROSPECT regulator 900"));
    fs::remove_dir_all(store.root()).unwrap();
}

#[test]
fn session_invalid_finalize_preserves_active_state() {
    let store = store("invalid");
    let thread = store.create_thread(None, "Active inquiry", None).unwrap();
    store
        .continuity_session_start_command("current :: title: Active")
        .unwrap();
    let before = fs::read(store.continuity_sessions_path(&thread.thread_id)).unwrap();
    let result = NextActionOutcome::continuity_result(
        store.continuity_session_finalize_command("latest :: outcome: parkk"),
        "summary",
    );
    assert_eq!(result.status, "needs_input");
    assert_eq!(
        fs::read(store.continuity_sessions_path(&thread.thread_id)).unwrap(),
        before
    );
    fs::remove_dir_all(store.root()).unwrap();
}
