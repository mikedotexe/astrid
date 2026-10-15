use astrid_source_study::{Catalog, Reader, StudyOutput, WritingRecovery, writing_storage_only};
use serde_json::{Value, json};
use std::{collections::BTreeMap, fs};

fn reader(root: &std::path::Path) -> Reader {
    fs::write(root.join("Cargo.toml"), "[workspace]\nmembers=[]\n").unwrap();
    Reader::new(
        Catalog::new(BTreeMap::from([("astrid".into(), root.into())])).unwrap(),
        root.join("state"),
    )
    .with_runtime_workspace(root.join("workspace"), "minime")
}
fn prepare(r: &Reader, id: &str, action: &str) -> StudyOutput {
    r.prepare_once(id, &r.preparation_revision().unwrap(), action)
        .unwrap()
}
fn deliver(r: &Reader, output: &StudyOutput, prose: &str) {
    r.navigation_delivered(
        output.navigation_id.as_deref().unwrap(),
        &json!({"messages":[{"role":"user","content":output.text}]}).to_string(),
        &json!({"choices":[{"message":{"content":prose},"finish_reason":"stop"}]}).to_string(),
    )
    .unwrap();
}
#[test]
fn storage_receipts_are_idempotent_and_never_provider_deliveries() {
    let temp = tempfile::tempdir().unwrap();
    let r = reader(temp.path());
    let start = prepare(&r, "start", "WRITE START exact preservation");
    deliver(&r, &start, "Exact passage.\nNEXT: WRITE FINISH");
    for (n, action) in [
        "WRITE QUESTION Why?",
        "WRITE EVIDENCE fixture",
        "WRITE STOPPING_POINT return here",
        "WRITE PROFILE SHORT",
        "WRITE FINISH",
    ]
    .iter()
    .enumerate()
    {
        let id = format!("metadata-{n}");
        let before = r.preparation_revision().unwrap();
        let output = r.prepare_once(&id, &before, action).unwrap();
        assert!(!output.generation_requested);
        assert!(output.system_prompt.is_empty());
        assert_eq!(r.prepare_once(&id, &before, action).unwrap(), output);
        assert!(r.prepare_once(&id, &before, "WRITE PARK").is_err());
        assert!(
            r.navigation_delivered(output.navigation_id.as_deref().unwrap(), "{}", "{}")
                .is_err()
        );
    }
    let error = r.prepare_action("WRITE CONTINUE").unwrap_err();
    let recovery = error.downcast_ref::<WritingRecovery>().unwrap();
    assert_eq!(recovery.code, "draft_finished");
    assert_eq!(recovery.commands[0], "WRITE RESUME d1");
    let resumed = prepare(&r, "resume", "WRITE RESUME d1");
    assert!(resumed.text.contains("Exact passage."));
    deliver(&r, &resumed, "Another passage.");
    let park = prepare(&r, "park", "WRITE PARK");
    assert!(!park.generation_requested);
    assert!(r.prepare_action("WRITE CONTINUE").is_err());
    let state: Value = serde_json::from_slice(
        &fs::read(temp.path().join("state/writing/drafts-v2.json")).unwrap(),
    )
    .unwrap();
    assert_eq!(
        state["drafts"]["d1"]["parts"],
        json!(["Exact passage.", "Another passage."])
    );
    assert_eq!(state["operations"].as_object().unwrap().len(), 6);
    assert_eq!(state["pending"], Value::Null);
    assert_eq!(state["schema_version"], 5);
}
#[test]
fn oversized_twelve_passages_remain_intact_with_actionable_capacity_details() {
    let temp = tempfile::tempdir().unwrap();
    let r = reader(temp.path());
    let mut parts = Vec::new();
    for n in 0..12 {
        let output = prepare(
            &r,
            &format!("passage-{n}"),
            if n == 0 {
                "WRITE START capacity fixture"
            } else {
                "WRITE CONTINUE"
            },
        );
        // The final accepted output crosses the next-input limit. It must be retained whole.
        let part = format!(
            "Passage {n}: {}",
            "x".repeat(if n == 11 { 27_000 } else { 3_000 })
        );
        deliver(&r, &output, &part);
        parts.push(part);
    }
    let prose = parts.join("\n\n");
    let path = temp.path().join("state/writing/drafts-v2.json");
    let before = fs::read(&path).unwrap();
    let error = r
        .prepare_once(
            "oversize",
            &r.preparation_revision().unwrap(),
            "WRITE CONTINUE",
        )
        .unwrap_err();
    let recovery = error.downcast_ref::<WritingRecovery>().unwrap();
    assert_eq!(recovery.code, "complete_input_too_large");
    let capacity = recovery.capacity.as_ref().unwrap();
    assert_eq!(capacity.allowance_bytes, 48_000);
    assert_eq!(capacity.prose_bytes, prose.len());
    assert_eq!(
        capacity.complete_bytes,
        capacity.prose_bytes + capacity.reference_bytes + capacity.framing_bytes
    );
    assert!(recovery.commands.contains(&"WRITE READ d1 1".into()));
    assert!(!recovery.commands.contains(&"WRITE CONTINUE".into()));
    assert!(!recovery.commands.contains(&"WRITE EVIDENCE".into()));
    assert_eq!(fs::read(&path).unwrap(), before);
    let stored: Value = serde_json::from_slice(&before).unwrap();
    assert_eq!(stored["drafts"]["d1"]["parts"], json!(parts));
    let page = prepare(&r, "read", "WRITE READ d1 1");
    assert!(page.text.contains("labelled excerpt"));
}
#[test]
fn pending_input_is_not_retried_after_finish_and_late_output_is_preserved() {
    let temp = tempfile::tempdir().unwrap();
    let r = reader(temp.path());
    let start = prepare(&r, "start", "WRITE START pending fixture");
    let id = start.navigation_id.as_deref().unwrap();
    assert_eq!(
        r.writing_recovery(id).unwrap().code,
        "pending_generation_available"
    );
    prepare(&r, "finish", "WRITE FINISH");
    assert_eq!(r.writing_recovery(id).unwrap().code, "draft_finished");
    assert!(r.prepare_action("WRITE CONTINUE").is_err());
    deliver(&r, &start, "Already accepted work retained.");
    assert!(r.prepare_action("WRITE CONTINUE").is_err());
    let resumed = prepare(&r, "resume", "WRITE RESUME d1");
    assert!(resumed.text.contains("Already accepted work retained."));
}

#[test]
fn final_provider_capacity_recovery_preserves_pending_bytes_and_valid_choices() {
    use astrid_source_study::FinalWritingCapacity;
    let temp = tempfile::tempdir().unwrap();
    let r = reader(temp.path());
    let start = prepare(&r, "start", "WRITE START synthetic boundary");
    deliver(&r, &start, &"x".repeat(45_000));
    let output = prepare(&r, "continue", "WRITE CONTINUE");
    assert!(output.text.len() + output.system_prompt.len() + 32 <= 48_000);
    let id = output.navigation_id.as_deref().unwrap();
    let path = temp.path().join("state/writing/drafts-v2.json");
    let before = fs::read(&path).unwrap();
    let capacity = FinalWritingCapacity {
        prepared_input_bytes: output.text.len(),
        system_bytes: 3_000,
        wrapper_bytes: 500,
        allowance_bytes: 48_000,
    };
    let recovery = r.writing_capacity_recovery(id, capacity.clone()).unwrap();
    assert_eq!(recovery.code, "final_request_too_large");
    assert_eq!(recovery.draft.as_deref(), Some("d1"));
    assert_eq!(recovery.final_capacity, Some(capacity.clone()));
    assert!(recovery.commands.contains(&"WRITE READ d1 1".into()));
    assert!(recovery.commands.contains(&"WRITE PARK".into()));
    assert!(!recovery.commands.contains(&"WRITE CONTINUE".into()));
    assert!(
        !recovery
            .commands
            .iter()
            .any(|c| c.starts_with("WRITE EVIDENCE"))
    );
    assert_eq!(fs::read(&path).unwrap(), before);
    assert_eq!(r.prepare_action("WRITE CONTINUE").unwrap(), output);
    let mut wrong = capacity.clone();
    wrong.prepared_input_bytes += 1;
    assert!(r.writing_capacity_recovery(id, wrong).is_err());
    let mut fits = capacity.clone();
    fits.allowance_bytes = 100_000;
    assert!(r.writing_capacity_recovery(id, fits).is_err());
    let mut overflow = capacity.clone();
    overflow.system_bytes = usize::MAX;
    assert!(r.writing_capacity_recovery(id, overflow).is_err());
    prepare(&r, "finish", "WRITE FINISH");
    assert_eq!(
        r.writing_capacity_recovery(id, capacity.clone())
            .unwrap()
            .code,
        "draft_finished"
    );
    deliver(
        &r,
        &output,
        "Late already-accepted output remains retained.",
    );
    prepare(&r, "other", "WRITE START unrelated synthetic work");
    let stale = r.writing_capacity_recovery(id, capacity).unwrap();
    assert_eq!(stale.code, "pending_input_unavailable");
    assert!(!stale.commands.contains(&"WRITE PARK".into()));
}

#[test]
fn compact_writing_guidance_keeps_balanced_choices_and_exact_return() {
    let temp = tempfile::tempdir().unwrap();
    let r = reader(temp.path());
    let output = prepare(&r, "start", "WRITE START synthetic guidance");
    let choices = output
        .text
        .split("Optional choices (never automatic):")
        .nth(1)
        .unwrap()
        .split("\nTo choose another action")
        .next()
        .unwrap();
    assert!(choices.len() < 450);
    for command in [
        "WRITE CONTINUE",
        "WRITE PARK",
        "WRITE FINISH",
        "WRITE RESUME d1",
        "WRITE READ d1 1",
        "WRITE HELP",
        "ACTIVITY_STATUS",
    ] {
        assert!(choices.contains(command));
    }
    assert!(!choices.contains("ACTIVITY_FOCUS WRITE"));
    assert!(
        choices
            .lines()
            .any(|line| line == "Return: WRITE RESUME d1")
    );
    assert!(
        choices
            .lines()
            .any(|line| line == "Inspect: WRITE READ d1 1")
    );
    assert!(output.system_prompt.contains("Missing NEXT or REST stops"));
    let help = prepare(&r, "help", "WRITE HELP");
    for command in [
        "WRITE BRANCH",
        "WRITE REVISE",
        "WRITE STOPPING_POINT",
        "ACTIVITY_FOCUS",
    ] {
        assert!(help.text.contains(command), "{command}");
    }
}
#[test]
fn classification_does_not_treat_arbitrary_writing_as_metadata() {
    for action in [
        "WRITE FINISH",
        "WRITE PARK",
        "WRITE PROFILE SHORT",
        "WRITE EVIDENCE",
    ] {
        assert!(writing_storage_only(action));
    }
    for action in [
        "WRITE CONTINUE",
        "WRITE READ d1",
        "WRITE START FINISH",
        "SELF_STUDY WRITE PARK",
    ] {
        assert!(!writing_storage_only(action));
    }
}

#[test]
fn metadata_never_spends_or_renews_protected_attention() {
    use astrid_source_study::ActivityRequest;
    for action in [
        "WRITE QUESTION Why?",
        "WRITE EVIDENCE fixture",
        "WRITE STOPPING_POINT here",
        "WRITE PROFILE SHORT",
        "WRITE FINISH",
        "WRITE PARK",
    ] {
        let temp = tempfile::tempdir().unwrap();
        let r = reader(temp.path());
        let start = prepare(&r, "start", "WRITE START synthetic focus");
        deliver(&r, &start, "Exact retained prose.");
        let at = u64::try_from(
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_millis(),
        )
        .unwrap();
        let status = r
            .activity("status", None, at, ActivityRequest::Status)
            .unwrap();
        let focused = r
            .activity(
                "focus",
                Some(status.revision),
                at,
                ActivityRequest::Command {
                    action: "ACTIVITY_FOCUS WRITE d1 turns 2".into(),
                },
            )
            .unwrap();
        let saved = prepare(&r, "save", action);
        assert!(!saved.generation_requested);
        let after = r
            .activity("after", None, at + 1000, ActivityRequest::Status)
            .unwrap();
        assert_eq!(after.admitted, focused.admitted);
        assert_eq!(after.remaining, focused.remaining);
        assert_eq!(after.window_id, focused.window_id);
        assert!(!after.protected);
        assert!(after.pending_job.is_none());
        assert!(after.next_action.is_none());
    }
}
