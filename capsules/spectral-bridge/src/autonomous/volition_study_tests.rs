//! Exercise reader delivery through the production completion-mode and NEXT seams.
use super::*;
use crate::autonomous::{next_action, runtime, state::ConversationState};
use astrid_source_study::{Catalog, Reader};
use serde_json::json;
use std::collections::BTreeMap;

#[test]
fn delivered_source_and_navigation_responses_attest_and_dispatch_their_own_next() {
    for (request, action) in [
        ("SELF_STUDY OPEN astrid/Cargo.toml 1", "SELF_STUDY CONTINUE"),
        ("SELF_STUDY MAP", "SELF_STUDY MAP astrid"),
        ("WRITE START trace a question", "WRITE CONTINUE"),
        ("WRITE PROFILE EXTENDED", "WRITE START develop the answer"),
        (
            "SELF_STUDY FIND package",
            "SELF_STUDY RESUME astrid/Cargo.toml",
        ),
        (
            "SELF_STUDY of the spectral tuning mechanisms",
            "SELF_STUDY MAP",
        ),
    ] {
        let temp = tempfile::tempdir().unwrap();
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
        let mode = if request.starts_with("WRITE") && delivery.is_ok() && written.is_ok() { "private_writing" } else { runtime::source_study_completion_mode(delivery.is_ok(), written.is_ok()) };
        let chosen = next_action::parse_next_action(&text).unwrap();
        assert_eq!(chosen, action);
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
fn failed_carriage_is_ineligible_and_study_authorship_does_not_grant_elevated_authority() {
    let temp = tempfile::tempdir().unwrap();
    for (delivered, written) in [(false, false), (false, true), (true, false)] {
        let mode = runtime::source_study_completion_mode(delivered, written);
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
