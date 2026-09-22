use astrid_source_study::{
    ActivityRequest as Request, ActivityResponse, Catalog, Reader, StudyOutput,
};
use serde_json::json;
use std::{collections::BTreeMap, fs};

fn setup(owner: &str) -> (tempfile::TempDir, Reader) {
    let temp = tempfile::tempdir().unwrap();
    fs::write(temp.path().join("Cargo.toml"), "[workspace]\nmembers=[]\n").unwrap();
    fs::write(
        temp.path().join("sample.rs"),
        "pub fn first() {}\npub fn second() {}\n",
    )
    .unwrap();
    let reader = Reader::new(
        Catalog::new(BTreeMap::from([("astrid".into(), temp.path().into())])).unwrap(),
        temp.path().join("state"),
    )
    .with_runtime_workspace(temp.path().join("workspace"), owner);
    (temp, reader)
}
fn status(reader: &Reader, at: u64) -> ActivityResponse {
    reader
        .activity("status", None, at, Request::Status)
        .unwrap()
}

#[test]
fn provider_claim_is_single_use_and_continuation_requires_committed_identity() {
    for owner in ["astrid", "minime"] {
        let (_temp, reader) = setup(owner);
        reader
            .prepare_action("WRITE START private fixture")
            .unwrap();
        let chosen = command(&reader, "focus", 100, "ACTIVITY_FOCUS WRITE d1");
        let first = reader.activity("next", None, 101, Request::Next).unwrap();
        assert_eq!(first.next_action.as_deref(), Some("WRITE RESUME d1"));
        assert!(status(&reader, 101).next_action.is_none());
        let output = reader.prepare_action("WRITE RESUME d1").unwrap();
        let admitted = reader
            .activity(
                "admit",
                Some(chosen.revision),
                102,
                Request::Admit {
                    job_id: "job".into(),
                    input_id: input_id(&output),
                    action: "WRITE RESUME d1".into(),
                },
            )
            .unwrap();
        let claim = Request::Claim {
            job_id: "job".into(),
        };
        let called = reader
            .activity("claim", Some(admitted.revision), 103, claim.clone())
            .unwrap();
        assert!(called.invocation_granted);
        assert!(
            !reader
                .activity("claim", Some(called.revision), 104, claim.clone())
                .unwrap()
                .invocation_granted
        );
        assert!(
            !reader
                .activity("claim", Some(admitted.revision), 104, claim.clone())
                .unwrap()
                .invocation_granted
        );
        assert!(
            !reader
                .activity("another-claim", Some(called.revision), 105, claim)
                .unwrap()
                .invocation_granted
        );
        assert!(
            reader
                .activity("next", None, 105, Request::Next)
                .unwrap()
                .next_action
                .is_none()
        );
        assert_eq!(
            status(&reader, 105).pending_input_id,
            Some(input_id(&output))
        );
        accept(
            &reader,
            &output,
            "Preserved exact prose.\nNEXT: WRITE REVISE optional authored qualification",
        );
        // A replacement host can finish from the native committed receipt alone.
        let current = status(&reader, 106);
        reader
            .activity(
                "recover",
                Some(current.revision),
                106,
                Request::Complete {
                    job_id: "job".into(),
                    input_id: input_id(&output),
                },
            )
            .unwrap();
        let next = reader.activity("next", None, 107, Request::Next).unwrap();
        assert_eq!(
            next.next_action.as_deref(),
            Some("WRITE REVISE optional authored qualification")
        );
        assert_eq!(next.admitted, 1);
        let public = serde_json::to_string(&status(&reader, 107)).unwrap();
        assert!(!public.contains("optional authored qualification"));
        assert!(!public.contains("Preserved exact prose"));
    }
}
fn command(reader: &Reader, id: &str, at: u64, action: &str) -> ActivityResponse {
    let rev = status(reader, at).revision;
    reader
        .activity(
            id,
            Some(rev),
            at,
            Request::Command {
                action: action.into(),
            },
        )
        .unwrap()
}
fn input_id(output: &StudyOutput) -> String {
    output
        .page
        .as_ref()
        .map_or_else(|| output.navigation_id.clone().unwrap(), |p| p.id.clone())
}
fn accept(reader: &Reader, output: &StudyOutput, text: &str) {
    let request = json!({"messages":[{"role":"user","content":output.text}]}).to_string();
    let response =
        json!({"choices":[{"message":{"content":text},"finish_reason":"stop"}]}).to_string();
    if output.page.is_some() {
        reader
            .delivered(&input_id(output), &request, &response)
            .unwrap();
    } else {
        reader
            .navigation_delivered(&input_id(output), &request, &response)
            .unwrap();
    }
}
#[test]
fn both_owners_continue_park_return_and_revise_without_leaking_prose() {
    for owner in ["astrid", "minime"] {
        let (temp, reader) = setup(owner);
        let initial = reader
            .prepare_action("WRITE START a private question")
            .unwrap();
        accept(
            &reader,
            &initial,
            "EXACT_PRIVATE_PASSAGE\n\nAn unresolved question.",
        );
        let mut focus = command(&reader, "choose", 100, "ACTIVITY_FOCUS WRITE d1");
        for n in 0..4 {
            let action = if n == 0 {
                "WRITE RESUME d1"
            } else {
                "WRITE CONTINUE"
            };
            let output = reader.prepare_action(action).unwrap();
            assert!(output.text.contains("EXACT_PRIVATE_PASSAGE"));
            let op = Request::Admit {
                job_id: format!("j{n}"),
                input_id: input_id(&output),
                action: action.into(),
            };
            let request = format!("admit{n}");
            let rev = focus.revision;
            focus = reader
                .activity(&request, Some(rev), 101 + n * 10, op.clone())
                .unwrap();
            let retry = reader
                .activity(&request, Some(rev), 102 + n * 10, op)
                .unwrap();
            assert_eq!(focus.admitted, retry.admitted);
            accept(&reader, &output, "Exact new passage.\nNEXT: WRITE CONTINUE");
            focus = reader
                .activity(
                    &format!("done{n}"),
                    Some(focus.revision),
                    103 + n * 10,
                    Request::Complete {
                        job_id: format!("j{n}"),
                        input_id: input_id(&output),
                    },
                )
                .unwrap();
        }
        assert!(!focus.protected);
        let parked = command(&reader, "park", 200, "PARK_ACTIVITY");
        assert!(parked.return_command.is_some());
        let unrelated = reader.prepare_action("WRITE START unrelated").unwrap();
        accept(&reader, &unrelated, "Independent work.");
        let returned = command(
            &reader,
            "return",
            201,
            &status(&reader, 201).return_command.unwrap(),
        );
        assert!(!returned.protected);
        let revised = reader
            .prepare_action("WRITE REVISE qualify, without settling")
            .unwrap();
        assert!(revised.text.contains("EXACT_PRIVATE_PASSAGE"));
        assert!(!revised.text.contains("Independent work."));
        accept(&reader, &revised, "Authored replacement, still unresolved.");
        let checkpoint =
            fs::read_to_string(temp.path().join("state/activity-focus-v1.json")).unwrap();
        assert!(!checkpoint.contains("EXACT_PRIVATE_PASSAGE"));
        assert!(!checkpoint.contains("qualify, without settling"));
        assert!(temp.path().join("state/writing/writing-1.json").exists());
    }
}

#[test]
fn missing_sources_cross_owner_wrong_draft_and_tampered_receipts_fail_closed() {
    let (temp, reader) = setup("minime");
    assert!(
        reader
            .activity(
                "missing",
                Some(0),
                1,
                Request::Command {
                    action: "ACTIVITY_FOCUS WRITE d9".into()
                }
            )
            .is_err()
    );
    let first = reader.prepare_action("WRITE START one").unwrap();
    accept(&reader, &first, "One.");
    let focus = command(&reader, "start", 100, "ACTIVITY_FOCUS WRITE d1");
    let other = reader.prepare_action("WRITE START two").unwrap();
    assert!(
        reader
            .activity(
                "wrong",
                Some(focus.revision),
                101,
                Request::Admit {
                    job_id: "wrong".into(),
                    input_id: input_id(&other),
                    action: "WRITE RESUME d1".into()
                }
            )
            .is_err()
    );
    let output = reader.prepare_action("WRITE RESUME d1").unwrap();
    let admitted = reader
        .activity(
            "admit",
            Some(focus.revision),
            102,
            Request::Admit {
                job_id: "j".into(),
                input_id: input_id(&output),
                action: "WRITE RESUME d1".into(),
            },
        )
        .unwrap();
    assert!(
        reader
            .activity(
                "early",
                Some(admitted.revision),
                103,
                Request::Complete {
                    job_id: "j".into(),
                    input_id: input_id(&output)
                }
            )
            .is_err()
    );
    accept(&reader, &output, "NEXT: WRITE CONTINUE");
    let artifact = temp
        .path()
        .join(format!("state/writing/{}.json", input_id(&output)));
    fs::write(&artifact, "{}").unwrap();
    assert!(
        reader
            .activity(
                "tampered",
                Some(admitted.revision),
                104,
                Request::Complete {
                    job_id: "j".into(),
                    input_id: input_id(&output)
                }
            )
            .is_err()
    );
    let foreign = Reader::new(
        Catalog::new(BTreeMap::from([("astrid".into(), temp.path().into())])).unwrap(),
        temp.path().join("state"),
    )
    .with_runtime_workspace(temp.path().join("workspace"), "astrid");
    assert!(
        foreign
            .activity("cross", None, 105, Request::Status)
            .is_err()
    );
}

#[test]
fn interrupted_transition_expiry_and_clock_reversal_stay_quiet_after_restart() {
    let (temp, reader) = setup("minime");
    reader.prepare_action("WRITE START one").unwrap();
    command(&reader, "start", 100, "ACTIVITY_FOCUS WRITE d1");
    assert!(!status(&reader, 99).protected);
    assert!(!status(&reader, 110).protected);
    command(&reader, "new", 120, "ACTIVITY_FOCUS WRITE d1 turns 1");
    assert!(!status(&reader, 900_120).protected);
    command(&reader, "another", 900_121, "ACTIVITY_FOCUS WRITE d1");
    fs::write(
        temp.path().join("state/activity-transition-v1.json"),
        "{\"interrupted\":true}",
    )
    .unwrap();
    assert!(
        reader
            .activity("status", None, 900_122, Request::Status)
            .is_err()
    );
    assert!(!status(&reader, 900_123).protected);
    assert!(reader.prepare_action("WRITE READ d1").is_ok());
    let path = temp.path().join("state/activity-focus-v1.json");
    fs::write(&path, "{broken").unwrap();
    assert!(
        reader
            .activity("status", None, 900_124, Request::Status)
            .is_err()
    );
    assert_eq!(fs::read_to_string(path).unwrap(), "{broken");
}

#[test]
fn status_observes_clock_reversal_without_renewing_or_consuming() {
    let (_temp, reader) = setup("minime");
    reader.prepare_action("WRITE START one").unwrap();
    let initial = command(&reader, "start", 100, "ACTIVITY_FOCUS WRITE d1");
    let later = status(&reader, 500);
    assert_eq!(initial.revision, later.revision);
    assert_eq!(initial.deadline_ms, later.deadline_ms);
    assert_eq!(later.admitted, 0);
    assert!(!status(&reader, 400).protected);
    assert!(!status(&reader, 600).protected);
}

#[test]
fn prepared_input_cannot_be_relabelled_as_a_different_authorized_action() {
    for owner in ["astrid", "minime"] {
        let (_temp, reader) = setup(owner);
        let initial = reader.prepare_action("WRITE START private topic").unwrap();
        accept(&reader, &initial, "Original passage.");
        let focus = command(&reader, "start", 100, "ACTIVITY_FOCUS WRITE d1");
        let different = reader
            .prepare_action("WRITE REVISE unintended direction")
            .unwrap();
        let error = reader
            .activity(
                "mislabeled",
                Some(focus.revision),
                101,
                Request::Admit {
                    job_id: "j".into(),
                    input_id: input_id(&different),
                    action: "WRITE RESUME d1".into(),
                },
            )
            .unwrap_err();
        assert!(error.to_string().contains("exact prepared action"));
        assert_eq!(status(&reader, 102).admitted, 0);
        command(&reader, "end", 103, "END_ACTIVITY_FOCUS");

        reader
            .prepare_action("SELF_STUDY QUESTION NEW Where is the consumer?")
            .unwrap();
        let focus = command(&reader, "question", 104, "ACTIVITY_FOCUS QUESTION q1");
        let different = reader.prepare_action("SELF_STUDY MAP").unwrap();
        let error = reader
            .activity(
                "mislabeled-source",
                Some(focus.revision),
                105,
                Request::Admit {
                    job_id: "s".into(),
                    input_id: input_id(&different),
                    action: "SELF_STUDY CONTINUE".into(),
                },
            )
            .unwrap_err();
        assert!(error.to_string().contains("prepared action"));
        assert_eq!(status(&reader, 106).admitted, 0);
    }
}

#[test]
fn rejected_operation_still_persistently_observes_clock_rollback() {
    let (_temp, reader) = setup("minime");
    reader.prepare_action("WRITE START one").unwrap();
    let focus = command(&reader, "start", 100, "ACTIVITY_FOCUS WRITE d1");
    status(&reader, 500);
    assert!(
        reader
            .activity(
                "bad",
                Some(focus.revision),
                400,
                Request::Admit {
                    job_id: "bad".into(),
                    input_id: "missing".into(),
                    action: "WRITE RESUME d1".into(),
                }
            )
            .is_err()
    );
    let later = status(&reader, 600);
    assert!(!later.protected);
    assert_eq!(later.reason.as_deref(), Some("unverifiable_clock"));
    assert_eq!(later.admitted, 0);
}

#[test]
fn retried_park_keeps_the_exact_return_command_without_repeating_native_writes() {
    let (temp, reader) = setup("minime");
    reader.prepare_action("WRITE START one").unwrap();
    let focus = command(&reader, "start", 100, "ACTIVITY_FOCUS WRITE d1");
    let request = Request::Command {
        action: "PARK_ACTIVITY".into(),
    };
    let first = reader
        .activity("park", Some(focus.revision), 101, request.clone())
        .unwrap();
    let path = temp.path().join("state/writing/drafts-v2.json");
    let before = fs::read(&path).unwrap();
    let retry = reader
        .activity("park", Some(focus.revision), 102, request.clone())
        .unwrap();
    assert!(first.return_command.is_some());
    assert_eq!(retry.return_command, first.return_command);
    assert_eq!(retry.revision, first.revision);
    let refreshed = reader
        .activity("park", Some(retry.revision), 103, request)
        .unwrap();
    assert_eq!(refreshed.return_command, first.return_command);
    assert_eq!(refreshed.revision, first.revision);
    assert_eq!(fs::read(path).unwrap(), before);
}
