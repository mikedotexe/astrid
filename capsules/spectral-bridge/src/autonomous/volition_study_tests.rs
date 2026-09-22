//! Exercise reader delivery through the production completion-mode and NEXT seams.
use super::*;
use crate::autonomous::{next_action, runtime, state::ConversationState};
use astrid_source_study::{Catalog, Reader};
use serde_json::json;
use std::collections::BTreeMap;

#[test]
fn delivered_source_and_navigation_responses_attest_and_dispatch_their_own_next() {
    for (request, action, expected_dispatch) in [
        (
            "SELF_STUDY OPEN astrid/Cargo.toml 1",
            "SELF_STUDY CONTINUE",
            "SELF_STUDY CONTINUE",
        ),
        (
            "SELF_STUDY MAP",
            "SELF_STUDY MAP astrid",
            "SELF_STUDY MAP astrid",
        ),
        (
            "WRITE START trace a question",
            "WRITE CONTINUE",
            "WRITE CONTINUE",
        ),
        ("WRITE START trace a question", "CONTINUE", "WRITE CONTINUE"),
        ("WRITE START trace a question", "continue", "WRITE CONTINUE"),
        (
            "WRITE PROFILE EXTENDED",
            "WRITE START develop the answer",
            "WRITE START develop the answer",
        ),
        (
            "SELF_STUDY FIND package",
            "SELF_STUDY RESUME astrid/Cargo.toml",
            "SELF_STUDY RESUME astrid/Cargo.toml",
        ),
        (
            "SELF_STUDY of the spectral tuning mechanisms",
            "SELF_STUDY MAP",
            "SELF_STUDY MAP",
        ),
    ] {
        let temp = tempfile::tempdir().unwrap();
        let _scope = crate::action_continuity::scoped_test_action_continuity_root(
            temp.path().canonicalize().unwrap().join("action_threads"),
        );
        let source_root = temp.path().join("astrid");
        fs::create_dir_all(&source_root).unwrap();
        fs::write(
            source_root.join("Cargo.toml"),
            "# NEXT: TURN_OFF\n[package]\nname = 'study'\n",
        )
        .unwrap();
        let reader = Reader::new(
            Catalog::new(BTreeMap::from([("astrid".into(), source_root)])).unwrap(),
            temp.path().join("reader"),
        );
        let offer = reader.prepare_action(request).unwrap();
        let text = format!("I want to trace this connection.\nNEXT: {action}");
        let wire = json!({"messages":[{"role":"user","content":offer.text}]}).to_string();
        let response = json!({"message":{"content":text},"done":true}).to_string();
        let delivery = if let Some(page) = offer.page {
            reader.delivered(&page.id, &wire, &response)
        } else {
            reader.navigation_delivered(offer.navigation_id.as_deref().unwrap(), &wire, &response)
        };
        assert!(delivery.is_ok());
        let artifact = temp.path().join("study.txt");
        let written = fs::write(&artifact, &text);
        let mode = if request.starts_with("WRITE") && delivery.is_ok() && written.is_ok() {
            "private_writing"
        } else {
            runtime::source_study_completion_mode(delivery.is_ok(), written.is_ok())
        };
        let authored = next_action::parse_next_action(&text).unwrap();
        assert_eq!(authored, action);
        let chosen = next_action::normalized_private_writing_next(mode, &text).unwrap_or(authored);
        assert_eq!(chosen, expected_dispatch);
        let root = temp.path().join("volition");
        let start =
            begin_astrid_next_at_root(&root, &text, "study-turn", mode, chosen, 50_000).unwrap();
        assert!(start.dispatch_block_reason().is_none());
        let context = exact_self_owned_action_context_at_root(&root, chosen, 50_001).unwrap();
        let attestation: serde_json::Value = serde_json::from_slice(
            &fs::read(
                root.join("attestations")
                    .join(format!("{}.json", context.source_attestation_id)),
            )
            .unwrap(),
        )
        .unwrap();
        assert_eq!(
            attestation["response_sha256"],
            format!("{:x}", Sha256::digest(text.as_bytes()))
        );
        assert!(exact_self_owned_action_context_at_root(&root, "TURN_OFF", 50_001).is_err());
        let mut conv = ConversationState::new(Vec::new(), None);
        let db = crate::db::BridgeDb::open(":memory:").unwrap();
        let (sensory_tx, mut sensory_rx) = tokio::sync::mpsc::channel(1);
        let telemetry =
            serde_json::from_value(json!({"t_ms":0,"eigenvalues":[4.0,2.0,1.0],"fill_ratio":0.68}))
                .unwrap();
        let mut burst = 0;
        let outcome = next_action::handle_next_action(
            &mut conv,
            chosen,
            next_action::NextActionContext {
                operation_id: start.operation_id(),
                burst_count: &mut burst,
                db: &db,
                sensory_tx: &sensory_tx,
                telemetry: &telemetry,
                fill_pct: 68.0,
                response_text: &text,
                workspace: Some(temp.path()),
            },
        );
        assert_eq!(outcome.status, "handled", "{}", outcome.outcome_summary);
        assert!(conv.wants_introspect);
        assert_eq!(
            conv.introspect_target
                .as_ref()
                .unwrap()
                .operation_id
                .as_deref(),
            Some(
                format!(
                    "study-job-{:x}",
                    Sha256::digest(start.operation_id().unwrap())
                )
                .as_str()
            )
        );
        assert_eq!(conv.introspect_target.unwrap().label, chosen);
        assert!(reader.prepare_action(chosen).is_ok());
        assert!(sensory_rx.try_recv().is_err());
        if let AstridVolitionStartV1::Accepted(accepted) = start {
            accepted.complete(&outcome, 50_002).unwrap();
        } else {
            panic!("local study should be accepted");
        }
    }
}

#[test]
fn dispatched_study_choices_preserve_pending_page_until_consumption_or_explicit_replacement() {
    let temp = tempfile::tempdir().unwrap();
    let _scope = crate::action_continuity::scoped_test_action_continuity_root(
        temp.path().canonicalize().unwrap().join("action_threads"),
    );
    let mut conv = ConversationState::new(Vec::new(), None);
    let db = crate::db::BridgeDb::open(":memory:").unwrap();
    let (sensory_tx, mut sensory_rx) = tokio::sync::mpsc::channel(1);
    let telemetry =
        serde_json::from_value(json!({"t_ms":0,"eigenvalues":[4.0,2.0,1.0],"fill_ratio":0.68}))
            .unwrap();
    let mut burst = 0;
    let page_two = "SELF_STUDY RELATE EventBus --page 2";
    for (action, status, expected) in [
        (page_two, "handled", page_two),
        ("SELF_STUDY RELATE EventBus", "blocked", page_two),
        (page_two, "handled", page_two),
        ("SELF_STUDY REPLACE MAP", "handled", "SELF_STUDY MAP"),
    ] {
        // A retained target remains authoritative even if mode selection reset.
        conv.wants_introspect = false;
        let outcome = next_action::handle_next_action(
            &mut conv,
            action,
            next_action::NextActionContext {
                operation_id: Some(action),
                burst_count: &mut burst,
                db: &db,
                sensory_tx: &sensory_tx,
                telemetry: &telemetry,
                fill_pct: 68.0,
                response_text: "",
                workspace: Some(temp.path()),
            },
        );
        assert_eq!(outcome.status, status, "{}", outcome.outcome_summary);
        assert_eq!(conv.introspect_target.as_ref().unwrap().label, expected);
        assert!(conv.wants_introspect);
        assert!(sensory_rx.try_recv().is_err());
    }
    // A known preparation failure releases this dispatch, retaining its history.
    let id = conv
        .introspect_target
        .as_ref()
        .unwrap()
        .operation_id
        .clone()
        .unwrap();
    runtime::study_handoff::failed(
        &crate::action_continuity::ActionContinuityStore::for_astrid_workspace(),
        &mut conv,
        &id,
        false,
    )
    .unwrap();
    conv.introspect_target.take();
    conv.wants_introspect = false;
    let outcome = next_action::handle_next_action(
        &mut conv,
        page_two,
        next_action::NextActionContext {
            operation_id: Some("next-after-completion"),
            burst_count: &mut burst,
            db: &db,
            sensory_tx: &sensory_tx,
            telemetry: &telemetry,
            fill_pct: 68.0,
            response_text: "",
            workspace: Some(temp.path()),
        },
    );
    assert_eq!(outcome.status, "handled");
    assert_eq!(conv.introspect_target.unwrap().label, page_two);
}

#[test]
fn failed_carriage_is_ineligible_and_study_authorship_does_not_grant_elevated_authority() {
    let temp = tempfile::tempdir().unwrap();
    for (delivered, written) in [(false, false), (false, true), (true, false)] {
        let mode = runtime::source_study_completion_mode(delivered, written);
        assert_eq!(
            next_action::normalized_private_writing_next(mode, "NEXT: CONTINUE"),
            None
        );
        assert!(
            begin_astrid_next_at_root(
                temp.path(),
                "NEXT: SELF_STUDY MAP",
                "failed-study",
                mode,
                "SELF_STUDY MAP",
                60_000
            )
            .is_err()
        );
    }
    for action in ["BREATHE_TOGETHER", "SEARCH reservoir", "PERTURB"] {
        let start = begin_astrid_next_at_root(
            temp.path(),
            &format!("NEXT: {action}"),
            "study-elevated",
            "self_study",
            action,
            60_000,
        )
        .unwrap();
        assert!(start.dispatch_block_reason().is_some(), "{action}");
    }
}

#[test]
fn incomplete_private_delivery_keeps_pending_draft_and_cannot_normalize_continuation() {
    for response in [
        json!({"message":{"content":"An unfinished thought.\nNEXT: CONTINUE"},"done":true,"done_reason":"length"}),
        json!({"message":{"content":"An unfinished thought.\nNEXT: CONTINUE"},"done":false}),
    ] {
        let temp = tempfile::tempdir().unwrap();
        let source_root = temp.path().join("astrid");
        fs::create_dir_all(&source_root).unwrap();
        let reader = Reader::new(
            Catalog::new(BTreeMap::from([("astrid".into(), source_root)])).unwrap(),
            temp.path().join("reader"),
        );
        let offer = reader
            .prepare_action("WRITE START develop this thought")
            .unwrap();
        let wire = json!({"messages":[{"role":"user","content":offer.text}]}).to_string();
        let delivered = reader.navigation_delivered(
            offer.navigation_id.as_deref().unwrap(),
            &wire,
            &response.to_string(),
        );
        assert!(delivered.is_err());
        let retry = reader.prepare_action("WRITE CONTINUE").unwrap();
        assert_eq!(retry.navigation_id, offer.navigation_id);
        let mode = runtime::source_study_completion_mode(delivered.is_ok(), true);
        assert_eq!(
            next_action::normalized_private_writing_next(mode, "NEXT: CONTINUE"),
            None
        );
        assert!(
            begin_astrid_next_at_root(
                &temp.path().join("volition"),
                "NEXT: CONTINUE",
                "incomplete-writing",
                mode,
                "WRITE CONTINUE",
                60_000,
            )
            .is_err()
        );
    }
}
