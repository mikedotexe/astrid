//! Host adapter for bounded native attention. Canonical prose stays in the reader.
use anyhow::{Context as _, Result, ensure};
use astrid_source_study::{
    ActivityRequest as Request, ActivityResponse as Response, Catalog, Reader,
};

use super::{activity_reading, state::ConversationState};
use crate::{action_continuity::ActionContinuityStore, paths::bridge_paths};

fn now() -> u64 {
    u64::try_from(
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis(),
    )
    .unwrap_or(u64::MAX)
}

pub(super) fn reader() -> Result<Reader> {
    let paths = bridge_paths();
    Ok(Reader::new(
        Catalog::installation(paths.astrid_root(), paths.minime_root())?,
        paths
            .bridge_workspace()
            .join("diagnostics/source_first_v3/shared_reader"),
    )
    .with_runtime_workspace(paths.bridge_workspace().to_path_buf(), "astrid"))
}

fn exists() -> bool {
    let root = bridge_paths()
        .bridge_workspace()
        .join("diagnostics/source_first_v3/shared_reader");
    ["activity-focus-v1.json", "activity-transition-v1.json"]
        .iter()
        .any(|p| root.join(p).exists())
}

pub(super) fn view(reader: &Reader) -> Result<Response> {
    reader.activity("status", None, now(), Request::Status)
}

fn change(reader: &Reader, request: Request) -> Result<Response> {
    let status = view(reader)?;
    let operation = serde_json::to_string(&(&status.window_id, &status.pending_job, &request))?;
    change_once(reader, request, &operation)
}

fn operation_key(operation: &str) -> String {
    use sha2::{Digest as _, Sha256};
    format!("bridge-{:x}", Sha256::digest(operation.as_bytes()))
}

fn change_once(reader: &Reader, request: Request, operation: &str) -> Result<Response> {
    ensure!(
        !operation.is_empty(),
        "durable activity operation identity required"
    );
    let status = view(reader)?;
    reader.activity(
        &operation_key(operation),
        Some(status.revision),
        now(),
        request,
    )
}

/// Fail quietly on uncertain state; never consume ordinary correspondence.
pub(super) fn quiet() -> bool {
    exists()
        && reader()
            .and_then(|r| view(&r))
            .map_or(true, |v| v.protected || v.pending_job.is_some())
}

fn save_window(conv: &mut ConversationState, window: Option<String>) -> Result<()> {
    save_window_in(&ActionContinuityStore::for_astrid_workspace(), conv, window)
}

fn save_window_in(
    store: &ActionContinuityStore,
    conv: &mut ConversationState,
    window: Option<String>,
) -> Result<()> {
    if conv.activity.native_focus_window == window {
        return Ok(());
    }
    let mut selection = conv.activity.clone();
    selection.native_focus_window = window;
    conv.activity = activity_reading::persist_activity(store, &selection)?;
    Ok(())
}

fn start_selection(
    store: &ActionContinuityStore,
    r: &Reader,
    conv: &mut ConversationState,
    action: &str,
    operation: &str,
) -> Result<Response> {
    let _lock = store.reader_owner_transaction()?;
    ensure!(
        !activity_reading::load_activity(store)?
            .study_handoff
            .pending(),
        "pending ordinary study must complete before protected focus can begin"
    );
    // Reject invalid or overlapping focus before touching the existing selector.
    // If a later write fails, prepare_exchange refuses the unmatched window;
    // it never starts a second foreground generator.
    let result = change_once(
        r,
        Request::Command {
            action: action.into(),
        },
        operation,
    )?;
    ensure!(
        result.window_id.as_deref() == Some(operation_key(operation).as_str()),
        "focus retry no longer names the current window; newer selection preserved"
    );
    if !result.protected {
        return Ok(result);
    }
    activity_reading::handle_action_in(store, conv, "PARK_ACTIVITY", "PARK_ACTIVITY")
        .context("saved activity park unavailable")??;
    let mut selection = conv.activity.clone();
    selection.mailbox_window = None;
    selection.native_focus_window = result.window_id.clone();
    conv.activity = activity_reading::persist_activity(store, &selection)?;
    Ok(result)
}

pub(super) fn observe_choice(conv: &mut ConversationState, action: &str) -> Result<()> {
    if !exists() {
        return Ok(());
    }
    let _lock = ActionContinuityStore::for_astrid_workspace().reader_owner_transaction()?;
    let r = reader()?;
    let v = r.activity("next", None, now(), Request::Next)?;
    if v.protected && v.next_action.as_deref() != Some(action) {
        change(
            &r,
            Request::Command {
                action: "END_ACTIVITY_FOCUS".into(),
            },
        )?;
        save_window(conv, None)?;
    } else if !v.protected {
        save_window(conv, None)?;
    }
    Ok(())
}

pub(super) fn handle_action(
    conv: &mut ConversationState,
    base: &str,
    action: &str,
    operation: Option<&str>,
) -> Option<Result<String>> {
    let native_return = action.starts_with("RETURN_ACTIVITY WRITE ")
        || action.starts_with("RETURN_ACTIVITY QUESTION ");
    let native = matches!(base, "ACTIVITY_FOCUS" | "END_ACTIVITY_FOCUS")
        || native_return
        || (exists()
            && (base == "ACTIVITY_STATUS"
                || (base == "PARK_ACTIVITY" && conv.activity.foreground_reader.is_none())));
    if (base == "CHECK_MAILBOX" || (base == "RETURN_ACTIVITY" && !native_return)) && exists() {
        if let Err(error) = reader().and_then(|r| {
            change_once(
                &r,
                Request::Command {
                    action: "END_ACTIVITY_FOCUS".into(),
                },
                operation.context("mailbox handoff requires a durable action identity")?,
            )
        }) {
            return Some(Err(error));
        }
        if let Err(error) = save_window(conv, None) {
            return Some(Err(error));
        }
        return None; // Existing saved reading/mailbox policy owns admission.
    }
    if !native {
        return None;
    }
    Some((|| {
        let _lock = ActionContinuityStore::for_astrid_workspace().reader_owner_transaction()?;
        let r = reader()?;
        let result = if base == "ACTIVITY_FOCUS" {
            start_selection(
                &ActionContinuityStore::for_astrid_workspace(),
                &r,
                conv,
                action,
                operation.context("focus requires a durable action identity")?,
            )?
        } else {
            change_once(
                &r,
                Request::Command {
                    action: action.into(),
                },
                if base == "ACTIVITY_STATUS" {
                    "status"
                } else {
                    operation.context("activity metadata requires a durable action identity")?
                },
            )?
        };
        save_window(
            conv,
            if result.protected {
                result.window_id.clone()
            } else {
                None
            },
        )?;
        if base == "ACTIVITY_FOCUS" {
            activity_reading::clear_superseded_reading_intents(conv);
            conv.introspect_target = None;
            conv.wants_introspect = false;
        }
        let mut text = serde_json::to_string_pretty(&result)?;
        if base == "ACTIVITY_STATUS" {
            text.push('\n');
            text.push_str(
                &activity_reading::handle_action(conv, base, action)
                    .context("saved activity status unavailable")??,
            );
        }
        Ok(text)
    })())
}

/// Restore only an authorized presentation or a verified eligible native NEXT.
pub(super) fn prepare_exchange(conv: &mut ConversationState) -> Result<bool> {
    if !exists() {
        ensure!(
            conv.activity.native_focus_window.is_none(),
            "selected native activity store missing"
        );
        return Ok(false);
    }
    prepare_exchange_in(
        &ActionContinuityStore::for_astrid_workspace(),
        &reader()?,
        conv,
    )
}

fn prepare_exchange_in(
    store: &ActionContinuityStore,
    r: &Reader,
    conv: &mut ConversationState,
) -> Result<bool> {
    let _lock = store.reader_owner_transaction()?;
    let status = view(r)?;
    if let Some(job_id) = status.pending_job {
        // The bridge loop is serial and this boundary is outside provider work.
        // Recovery accepts committed receipts only; uncertainty never reruns a job.
        change(
            r,
            Request::Complete {
                job_id,
                input_id: status
                    .pending_input_id
                    .context("pending input identity missing")?,
            },
        )?;
    }
    let status = r.activity("next", None, now(), Request::Next)?;
    // The conversation snapshot can predate a native completion. Retire only
    // scheduler targets belonging to consumed slots in this exact window, even
    // when its budget/deadline ended. Never replay them as unprotected work.
    if let (Some(window), Some(operation)) = (
        status.window_id.as_deref(),
        conv.introspect_target
            .as_ref()
            .and_then(|t| t.operation_id.as_deref()),
    ) && (0..status.admitted).any(|slot| operation == format!("focus-{window}-{slot}"))
    {
        conv.introspect_target = None;
        conv.wants_introspect = false;
    }
    if !status.protected {
        save_window_in(store, conv, None)?;
        return Ok(false);
    }
    if conv.activity.native_focus_window != status.window_id {
        change(
            r,
            Request::Command {
                action: "END_ACTIVITY_FOCUS".into(),
            },
        )?;
        save_window_in(store, conv, None)?;
        anyhow::bail!(
            "interrupted native foreground selection released priority; explicitly reselect"
        );
    }
    ensure!(
        conv.activity.foreground_reader.is_none() && conv.activity.mailbox_window.is_none(),
        "competing activity selection; explicit reconciliation required"
    );
    let action = status
        .next_action
        .context("protected activity has no recoverable continuation")?;
    if let Some(target) = &conv.introspect_target {
        ensure!(
            target.label == action,
            "pending study differs from protected selection"
        );
    }
    let mut target = super::state::IntrospectTargetV2::auto(action);
    target.operation_id = Some(format!(
        "focus-{}-{}",
        status.window_id.context("focus window missing")?,
        status.admitted
    ));
    conv.introspect_target = Some(target);
    conv.wants_introspect = false; // This exchange owns the selected presentation.
    Ok(true)
}

pub(super) struct Admission {
    job: String,
    input: String,
}

pub(super) fn admit(
    r: &Reader,
    output: &astrid_source_study::StudyOutput,
    action: &str,
) -> Result<Option<Admission>> {
    let status = view(r)?;
    ensure!(
        status.pending_job.is_none(),
        "protected job already pending"
    );
    if !status.protected {
        return Ok(None);
    }
    let input = output
        .page
        .as_ref()
        .map(|p| p.id.clone())
        .or_else(|| output.navigation_id.clone())
        .context("protected input identity missing")?;
    let job = format!("bridge-{:032x}", rand::random::<u128>());
    let admitted = r.activity(
        &format!("admit-{job}"),
        Some(status.revision),
        now(),
        Request::Admit {
            job_id: job.clone(),
            input_id: input.clone(),
            action: action.into(),
        },
    )?;
    let claimed = r.activity(
        &format!("claim-{job}"),
        Some(admitted.revision),
        now(),
        Request::Claim {
            job_id: job.clone(),
        },
    )?;
    ensure!(
        claimed.invocation_granted,
        "provider invocation already claimed"
    );
    Ok(Some(Admission { job, input }))
}

pub(super) fn finish(r: &Reader, admission: Option<Admission>, verified: bool) -> Result<()> {
    if let Some(a) = admission {
        change(
            r,
            if verified {
                Request::Complete {
                    job_id: a.job,
                    input_id: a.input,
                }
            } else {
                Request::Failed { job_id: a.job }
            },
        )?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeMap;

    #[test]
    fn ordinary_queued_study_blocks_focus_before_native_selection_changes() {
        let temp = tempfile::tempdir().unwrap();
        let workspace = temp.path().canonicalize().unwrap();
        let store = ActionContinuityStore::new(workspace.join("action_threads"));
        let r = Reader::new(
            Catalog::new(BTreeMap::from([("astrid".into(), workspace.clone())])).unwrap(),
            workspace.join("diagnostics/source_first_v3/shared_reader"),
        )
        .with_runtime_workspace(workspace.clone(), "astrid");
        r.prepare_action("WRITE START synthetic draft").unwrap();
        let mut conv = ConversationState::new(Vec::new(), None);
        let mut target = Some(super::super::state::IntrospectTargetV2::auto(
            "SELF_STUDY MAP".into(),
        ));
        super::super::study_handoff::queue(&store, &mut conv, &mut target, "ordinary", false)
            .unwrap();
        let before = activity_reading::load_activity(&store).unwrap();
        assert!(
            start_selection(&store, &r, &mut conv, "ACTIVITY_FOCUS WRITE d1", "focus").is_err()
        );
        assert_eq!(activity_reading::load_activity(&store).unwrap(), before);
        assert!(
            !workspace
                .join("diagnostics/source_first_v3/shared_reader/activity-focus-v1.json")
                .exists()
        );
    }

    #[test]
    fn rejected_focus_preserves_existing_native_and_saved_selection() {
        let temp = tempfile::tempdir().unwrap();
        let store = ActionContinuityStore::new(temp.path().join("action_threads"));
        let r = Reader::new(
            Catalog::new(BTreeMap::from([("astrid".into(), temp.path().to_owned())])).unwrap(),
            temp.path()
                .join("diagnostics/source_first_v3/shared_reader"),
        )
        .with_runtime_workspace(temp.path().to_owned(), "astrid");
        r.prepare_action("WRITE START synthetic private work")
            .unwrap();
        let mut conv = ConversationState::new(Vec::new(), None);
        assert!(
            start_selection(
                &store,
                &r,
                &mut conv,
                "ACTIVITY_FOCUS WRITE d999",
                "bad-event"
            )
            .is_err()
        );
        assert!(!store.root().join("activity_runtime_v2.json").exists());
        let first = start_selection(
            &store,
            &r,
            &mut conv,
            "ACTIVITY_FOCUS WRITE d1",
            "first-event",
        )
        .unwrap();
        let path = store.root().join("activity_runtime_v3.json");
        let bytes = std::fs::read(&path).unwrap();
        assert!(
            start_selection(
                &store,
                &r,
                &mut conv,
                "ACTIVITY_FOCUS WRITE d1",
                "second-event"
            )
            .is_err()
        );
        assert_eq!(std::fs::read(path).unwrap(), bytes);
        assert_eq!(conv.activity.native_focus_window, first.window_id);
        assert_eq!(view(&r).unwrap().window_id, first.window_id);
        let retry = start_selection(
            &store,
            &r,
            &mut conv,
            "ACTIVITY_FOCUS WRITE d1",
            "first-event",
        )
        .unwrap();
        assert_eq!(retry.window_id, first.window_id);
        assert_eq!(retry.admitted, 0);
        assert!(
            start_selection(
                &store,
                &r,
                &mut conv,
                "ACTIVITY_FOCUS WRITE d1 turns 1",
                "first-event"
            )
            .is_err()
        );
    }

    #[test]
    fn bridge_adapter_admits_once_and_recovers_only_native_delivery() {
        let temp = tempfile::tempdir().unwrap();
        let r = Reader::new(
            Catalog::new(BTreeMap::from([("astrid".into(), temp.path().to_owned())])).unwrap(),
            temp.path().join("state"),
        )
        .with_runtime_workspace(temp.path().join("workspace"), "astrid");
        r.prepare_action("WRITE START synthetic private work")
            .unwrap();
        change(
            &r,
            Request::Command {
                action: "ACTIVITY_FOCUS WRITE d1 turns 1".into(),
            },
        )
        .unwrap();
        let output = r.prepare_action("WRITE RESUME d1").unwrap();
        let admission = admit(&r, &output, "WRITE RESUME d1").unwrap().unwrap();
        assert!(admit(&r, &output, "WRITE RESUME d1").is_err());
        let status = view(&r).unwrap();
        let operation = Request::Complete {
            job_id: admission.job.clone(),
            input_id: admission.input.clone(),
        };
        assert!(change(&r, operation.clone()).is_err());
        assert_eq!(view(&r).unwrap().pending_job, status.pending_job);
        r.navigation_delivered(&admission.input,
            &serde_json::json!({"messages":[{"role":"user","content":output.text}]}).to_string(),
            &serde_json::json!({"choices":[{"message":{"content":"Exact private result.\nNEXT: WRITE CONTINUE"},"finish_reason":"stop"}]}).to_string(),
        ).unwrap();
        finish(&r, Some(admission), true).unwrap();
        let final_status = view(&r).unwrap();
        assert_eq!(final_status.admitted, 1);
        assert!(!final_status.protected);
        assert!(final_status.pending_job.is_none());
        assert!(
            !serde_json::to_string(&final_status)
                .unwrap()
                .contains("Exact private result")
        );
    }
}

#[cfg(test)]
#[path = "activity_focus_interruption_tests.rs"]
mod interruption_tests;
