//! Exact experiment authority across the ordinary source reader's deferred worker.
use super::*;
use crate::action_continuity::{self, NextActionOutcome, ResearchReservation};
use crate::autonomous::state::IntrospectOffsetV2;
use std::cell::RefCell;

thread_local! {
    static DISPATCH: RefCell<Option<ResearchReservation>> = const { RefCell::new(None) };
}
struct DispatchGuard;
impl Drop for DispatchGuard {
    fn drop(&mut self) {
        DISPATCH.with(|slot| slot.borrow_mut().take());
    }
}

pub(super) fn dispatch_reservation() -> Option<ResearchReservation> {
    DISPATCH.with(|slot| slot.borrow().clone())
}

pub(super) fn validate_binding(id: &str, job: &Job, record: &Intent) -> Result<()> {
    let Some(ticket) = &record.research else {
        ensure!(
            job.research.is_none(),
            "study reservation reference missing"
        );
        return Ok(());
    };
    ensure!(record.schema_version == 2, "unsupported bound study intent");
    ensure!(
        job.research.as_deref() == Some(digest(serde_json::to_vec(ticket)?).as_str()),
        "study reservation hash mismatch"
    );
    let metadata = ticket.metadata();
    let operation = metadata["operation_id"]
        .as_str()
        .context("research operation missing")?;
    ensure!(
        id == format!("study-job-{}", digest(operation)),
        "research operation differs from study job"
    );
    let route = astrid_source_study::research_source_route(ticket.action()?)
        .context("reservation is not a verified source research route")?;
    ensure!(
        record.target.offset == IntrospectOffsetV2::Auto
            && astrid_source_study::research_source_route(&record.target.label).as_ref()
                == Some(&route),
        "study target differs from exact research reservation"
    );
    Ok(())
}

pub(super) fn reservation(
    store: &ActionContinuityStore,
    id: &str,
    job: &Job,
) -> Result<Option<ResearchReservation>> {
    intent(store, id, job)?;
    let record: Intent = read(store, &job.intent)?;
    if let Some(ticket) = &record.research {
        store.validate_research_reservation(ticket)?;
    }
    Ok(record.research)
}

fn lifecycle(store: &ActionContinuityStore, ticket: &ResearchReservation) -> Result<String> {
    store.validate_research_reservation(ticket)?;
    let value = serde_json::to_value(ticket)?;
    let record = &value["record"];
    let thread = record["thread_id"]
        .as_str()
        .context("research thread missing")?;
    let rows = action_continuity::read_native_authority_records(
        &store
            .root()
            .join("threads")
            .join(thread)
            .join("authority_gate.jsonl"),
    )?;
    rows.iter()
        .rev()
        .find(|row| row["operation_id"] == record["operation_id"])
        .and_then(|row| row["lifecycle"].as_str())
        .map(str::to_owned)
        .context("research lifecycle missing")
}

pub(super) fn check_phase(store: &ActionContinuityStore, id: &str, job: &Job) -> Result<()> {
    let Some(ticket) = reservation(store, id, job)? else {
        return Ok(());
    };
    let current = lifecycle(store, &ticket)?;
    let valid = match job.phase {
        Phase::Queued | Phase::Prepared => current == "queued",
        Phase::Claimed => matches!(current.as_str(), "execution_started" | "completed"),
        Phase::Delivered => current == "completed",
        Phase::Failed => matches!(current.as_str(), "released_unstarted" | "failed"),
        Phase::Superseded => current == "released_unstarted",
        Phase::RetiredUnresolved => current == "execution_started",
        Phase::Stored => false,
    };
    ensure!(
        valid,
        "study research state is uncertain or terminal; no automatic replay"
    );
    Ok(())
}

pub(super) fn transition(
    store: &ActionContinuityStore,
    id: &str,
    job: &Job,
    next: &str,
) -> Result<()> {
    if let Some(ticket) = reservation(store, id, job)? {
        store.transition_research(&ticket, next)?;
    }
    Ok(())
}

/// Repair only a known terminal authority write interrupted before activity commit.
/// Started-without-claim remains uncertain and charged, never converted to failure.
pub(super) fn recover_known_failure(
    store: &ActionContinuityStore,
    conv: &mut ConversationState,
    selection: &activity_reading::ActivityRuntimeV1,
    id: &str,
) -> Result<bool> {
    let job = &selection.study_handoff.jobs[id];
    let Some(ticket) = reservation(store, id, job)? else {
        return Ok(false);
    };
    let current = lifecycle(store, &ticket)?;
    let known = match current.as_str() {
        "released_unstarted" => matches!(job.phase, Phase::Queued | Phase::Prepared),
        "failed" => job.phase == Phase::Claimed,
        _ => false,
    };
    if !known {
        return Ok(false);
    }
    let mut recovered = selection.clone();
    selected(&mut recovered, id)?.phase = Phase::Failed;
    recovered.study_handoff.active = None;
    conv.activity = activity_reading::persist_activity(store, &recovered)?;
    conv.introspect_target = None;
    conv.wants_introspect = false;
    Ok(true)
}

pub(in crate::autonomous) fn dispatch(
    conv: &mut ConversationState,
    ticket: ResearchReservation,
    run: impl FnOnce(&mut ConversationState) -> NextActionOutcome,
) -> Result<NextActionOutcome> {
    dispatch_in(
        &ActionContinuityStore::for_astrid_workspace(),
        conv,
        ticket,
        run,
    )
}

fn dispatch_in(
    store: &ActionContinuityStore,
    conv: &mut ConversationState,
    ticket: ResearchReservation,
    run: impl FnOnce(&mut ConversationState) -> NextActionOutcome,
) -> Result<NextActionOutcome> {
    // All handoff paths take authority before reader, including synchronous
    // nested queueing. Neither lock nor the thread-local scope crosses await.
    let _research_lock = action_continuity::research_owner_transaction(store.root())?;
    let _reader_lock = store.reader_owner_transaction()?;
    store.validate_research_reservation(&ticket)?;
    ensure!(
        DISPATCH.with(|slot| slot.borrow().is_none()),
        "nested source research dispatch"
    );
    let metadata = ticket.metadata();
    let operation = metadata["operation_id"]
        .as_str()
        .context("research operation missing")?;
    let id = format!("study-job-{}", digest(operation));
    let selection = activity_reading::load_activity(store)?;
    if selection.native_focus_window.is_some()
        || selection
            .study_handoff
            .active
            .as_ref()
            .is_some_and(|active| active != &id)
        || (conv.wants_introspect
            && conv
                .introspect_target
                .as_ref()
                .and_then(|target| target.operation_id.as_deref())
                != Some(id.as_str()))
    {
        store.transition_research(&ticket, "released_unstarted")?;
        return Ok(NextActionOutcome::blocked(
            "research_handoff",
            "Existing source work or protected focus retained; bound source work was not queued.",
        ));
    }
    ensure!(
        matches!(lifecycle(store, &ticket)?.as_str(), "admitted" | "queued"),
        "source research already started or terminal; no replay"
    );
    DISPATCH.with(|slot| *slot.borrow_mut() = Some(ticket.clone()));
    let _guard = DispatchGuard;
    let mut outcome = run(conv);
    let selection = activity_reading::load_activity(store)?;
    if let Some(job) = selection.study_handoff.jobs.get(&id) {
        let retained =
            reservation(store, &id, job)?.context("bound source ticket was not retained")?;
        ensure!(
            serde_json::to_value(retained)? == serde_json::to_value(&ticket)?
                && selection.study_handoff.active.as_deref() == Some(&id),
            "source dispatch differs from retained research job"
        );
        check_phase(store, &id, job)?;
        ensure!(
            matches!(job.phase, Phase::Queued | Phase::Prepared),
            "source dispatch already claimed"
        );
        outcome.status = "queued".into();
        outcome.outcome_summary =
            "Bound source study queued; worker execution and delivery have not occurred.".into();
        let mut metadata = metadata;
        metadata["execution_scope"] = serde_json::json!("source_study_generation_worker");
        return Ok(outcome.with_research_budget(metadata));
    }
    store.transition_research(&ticket, "released_unstarted")?;
    Ok(NextActionOutcome::blocked(
        "research_handoff",
        "No durable source study was queued; no research allowance was debited.",
    )
    .with_research_budget(metadata))
}

#[cfg(test)]
#[path = "study_research_tests.rs"]
mod tests;
