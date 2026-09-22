use astrid_source_study::focus::{FocusState, Operation, Owner, Target, WINDOW_MS, parse_start};

fn started() -> FocusState {
    let mut state = FocusState::new(Owner::Minime);
    state
        .apply(
            &Owner::Minime,
            "start",
            100,
            Operation::Start {
                target: Target::Write("d1".into()),
                target_revision: "r0".into(),
                turns: 4,
            },
        )
        .unwrap();
    state
}
fn apply(state: &mut FocusState, id: &str, at: u64, op: Operation) {
    state.apply(&Owner::Minime, id, at, op).unwrap();
}
#[test]
fn four_explicit_generations_with_idempotent_admission_and_restart() {
    let mut state = started();
    for index in 0..4 {
        let action = if index == 0 {
            "WRITE RESUME d1"
        } else {
            "WRITE CONTINUE"
        };
        let op = Operation::Admit {
            job_id: format!("job{index}"),
            action: action.into(),
            target_revision: format!("r{index}"),
        };
        let time = 110 + u64::from(index) * 20;
        apply(&mut state, &format!("admit{index}"), time, op.clone());
        apply(&mut state, &format!("admit{index}"), time + 1, op.clone());
        apply(&mut state, &format!("retry{index}"), time + 2, op);
        assert_eq!(state.window.as_ref().unwrap().admitted, index + 1);
        state = serde_json::from_slice(&serde_json::to_vec(&state).unwrap()).unwrap();
        apply(
            &mut state,
            &format!("done{index}"),
            time + 10,
            Operation::Complete {
                job_id: format!("job{index}"),
                next: Some("WRITE CONTINUE".into()),
                target_revision: format!("r{}", index + 1),
            },
        );
    }
    assert!(!state.protected(190));
    assert_eq!(
        state.window.as_ref().unwrap().ended.as_deref(),
        Some("job_budget")
    );
    // Expiry affects priority, not the independently retained authored action.
    assert!(state.window.as_ref().unwrap().next.is_some());
    assert!(
        !serde_json::to_string(&state)
            .unwrap()
            .contains("WRITE CONTINUE")
    );
}
#[test]
fn no_next_other_work_or_rest_ends_without_synthesizing_a_choice() {
    for next in [
        None,
        Some("REST"),
        Some("SEARCH history"),
        Some("WRITE START other"),
        Some("FOCUS"),
    ] {
        let mut state = started();
        apply(
            &mut state,
            "admit",
            110,
            Operation::Admit {
                job_id: "j".into(),
                action: "WRITE RESUME d1".into(),
                target_revision: "r0".into(),
            },
        );
        apply(
            &mut state,
            "done",
            120,
            Operation::Complete {
                job_id: "j".into(),
                next: next.map(str::to_owned),
                target_revision: "r1".into(),
            },
        );
        assert!(!state.protected(121));
    }
}
#[test]
fn deadline_clock_reversal_and_accepted_work_do_not_renew_or_cancel() {
    let mut state = started();
    apply(
        &mut state,
        "admit",
        110,
        Operation::Admit {
            job_id: "j".into(),
            action: "WRITE RESUME d1".into(),
            target_revision: "r0".into(),
        },
    );
    assert!(!state.protected(109));
    assert!(!state.protected(100 + WINDOW_MS));
    assert_eq!(
        state.window.as_ref().unwrap().running_job.as_deref(),
        Some("j")
    );
    apply(
        &mut state,
        "done",
        100 + WINDOW_MS,
        Operation::Complete {
            job_id: "j".into(),
            next: Some("WRITE CONTINUE".into()),
            target_revision: "r1".into(),
        },
    );
    assert_eq!(
        state.window.as_ref().unwrap().ended.as_deref(),
        Some("deadline")
    );
    assert_eq!(state.window.as_ref().unwrap().admitted, 1);
}
#[test]
fn park_return_keeps_revision_and_never_reopens_a_budget() {
    let mut state = started();
    apply(&mut state, "park", 110, Operation::Park);
    assert!(!state.protected(111));
    let revision = state.revision;
    let before = state.clone();
    assert!(
        state
            .apply(
                &Owner::Minime,
                "bad-return",
                120,
                Operation::Return {
                    expected_revision: 0
                }
            )
            .is_err()
    );
    assert_eq!(state, before);
    apply(
        &mut state,
        "return",
        120,
        Operation::Return {
            expected_revision: revision,
        },
    );
    assert!(!state.protected(121));
    assert_eq!(state.parked, Some(Target::Write("d1".into())));
}
#[test]
fn invalid_operations_fail_without_partial_mutation() {
    let mut state = started();
    let before = state.clone();
    for op in [
        Operation::Start {
            target: Target::Write("../d1".into()),
            target_revision: "r".into(),
            turns: 4,
        },
        Operation::Admit {
            job_id: "j".into(),
            action: "WRITE CONTINUE".into(),
            target_revision: "r0".into(),
        },
        Operation::Admit {
            job_id: "j".into(),
            action: "WRITE RESUME d1".into(),
            target_revision: "stale".into(),
        },
        Operation::Complete {
            job_id: "missing".into(),
            next: None,
            target_revision: "r".into(),
        },
    ] {
        assert!(state.apply(&Owner::Minime, "invalid", 110, op).is_err());
        assert_eq!(state, before);
    }
    assert!(
        state
            .apply(&Owner::Astrid, "cross-owner", 120, Operation::End)
            .is_err()
    );
    assert!(
        state
            .apply(&Owner::Minime, "start", 120, Operation::End)
            .is_err()
    );
    assert_eq!(state, before);
    state.schema_version = 99;
    assert!(state.validate(&Owner::Minime).is_err());
}
#[test]
fn syntax_never_reinterprets_metabolic_focus_or_invalid_limits() {
    assert_eq!(
        parse_start("ACTIVITY_FOCUS QUESTION q2 turns 2").unwrap(),
        (Target::Question("q2".into()), 2)
    );
    for action in [
        "FOCUS",
        "ACTIVITY_FOCUS WRITE d1 turns 0",
        "ACTIVITY_FOCUS WRITE d1 turns 5",
        "ACTIVITY_FOCUS WRITE d0",
        "ACTIVITY_FOCUS WRITE d01",
        "ACTIVITY_FOCUS WRITE d1 extra",
    ] {
        assert!(parse_start(action).is_err(), "{action}");
    }
}

#[test]
fn reusing_a_job_identity_in_a_new_window_cannot_bypass_admission_budget() {
    let mut state = started();
    let op = Operation::Admit {
        job_id: "j".into(),
        action: "WRITE RESUME d1".into(),
        target_revision: "r0".into(),
    };
    apply(&mut state, "admit", 110, op.clone());
    apply(
        &mut state,
        "done",
        120,
        Operation::Complete {
            job_id: "j".into(),
            next: None,
            target_revision: "r0".into(),
        },
    );
    apply(
        &mut state,
        "new-window",
        130,
        Operation::Start {
            target: Target::Write("d1".into()),
            target_revision: "r0".into(),
            turns: 4,
        },
    );
    assert!(state.apply(&Owner::Minime, "reused-job", 140, op).is_err());
    assert_eq!(state.window.unwrap().admitted, 0);
}
