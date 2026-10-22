//! Research handoff for the native saved-text worker, not legacy context listings.
use super::*;
use crate::action_continuity::{
    ActionContinuityStore, ReaderBookmarkVersion, ReaderPassage, ResearchReservation,
};
use crate::autonomous::activity_reading::{self, ActivityReadingOfferV1, ReaderActivityRefV1};
use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::cell::RefCell;
use std::io::Write;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
enum Phase {
    Preparing,
    Queued,
    HostStarted,
    Started,
    Closed,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct PassageRef {
    reader: ReaderActivityRefV1,
    version: ReaderBookmarkVersion,
    passage: ReaderPassage,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct ReadingJob {
    schema_version: u32,
    reservation: ResearchReservation,
    phase: Phase,
    selected_reader: Option<ReaderActivityRefV1>,
    selection_revision: Option<u64>,
    selection: Option<PassageRef>,
    // Only bounded delivery identities, never the retained output path or prose.
    delivery: Option<serde_json::Value>,
}

thread_local! {
    static DISPATCH: RefCell<Option<ResearchReservation>> = const { RefCell::new(None) };
}
struct DispatchGuard;
impl Drop for DispatchGuard {
    fn drop(&mut self) {
        DISPATCH.with(|slot| slot.borrow_mut().take());
    }
}

fn path(store: &ActionContinuityStore) -> std::path::PathBuf {
    store.root().join("research_saved_reading_handoffs.jsonl")
}

fn latest(store: &ActionContinuityStore) -> Result<Option<ReadingJob>> {
    let _lock = action_continuity::research_owner_transaction(store.root())?;
    let rows = action_continuity::read_native_authority_records(&path(store))?;
    let mut last: Option<ReadingJob> = None;
    let mut operations = std::collections::HashSet::new();
    for row in rows {
        let record: ReadingJob = serde_json::from_value(row)?;
        anyhow::ensure!(
            record.schema_version == 1,
            "unsupported saved research handoff"
        );
        validate_successor(last.as_ref(), &record)?;
        if last
            .as_ref()
            .is_none_or(|prior| prior.phase == Phase::Closed)
        {
            anyhow::ensure!(
                operations.insert(record.reservation.metadata()["operation_id"].to_string()),
                "duplicate saved research operation"
            );
            store.validate_research_reservation(&record.reservation)?;
        }
        last = Some(record);
    }
    Ok(last)
}

fn validate_successor(prior: Option<&ReadingJob>, record: &ReadingJob) -> Result<()> {
    anyhow::ensure!(
        record.schema_version == 1,
        "unsupported saved research handoff"
    );
    if let Some(reader) = &record.selected_reader {
        anyhow::ensure!(
            record.selection_revision.is_some(),
            "missing reading selection revision"
        );
        for id in [&reader.thread_id, &reader.session_id] {
            anyhow::ensure!(
                !id.is_empty()
                    && id.len() <= 512
                    && id
                        .bytes()
                        .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b'-')),
                "invalid saved research reader identity"
            );
        }
    }
    if matches!(record.phase, Phase::Queued | Phase::Started) {
        anyhow::ensure!(record.selection.as_ref().is_some_and(|selection| Some(&selection.reader) == record.selected_reader.as_ref()), "missing or conflicting saved research passage");
    }
    let Some(prior) = prior.filter(|prior| prior.phase != Phase::Closed) else {
        anyhow::ensure!(
            record.phase == Phase::Preparing && record.selection.is_none(),
            "missing saved research preparation"
        );
        return Ok(());
    };
    anyhow::ensure!(
        serde_json::to_value(&prior.reservation)? == serde_json::to_value(&record.reservation)?,
        "conflicting saved research handoff"
    );
    let valid = match prior.phase {
        Phase::Preparing => matches!(
            record.phase,
            Phase::Preparing | Phase::Queued | Phase::HostStarted | Phase::Closed
        ),
        Phase::Queued => matches!(record.phase, Phase::Started | Phase::Closed),
        Phase::Started | Phase::HostStarted => record.phase == Phase::Closed,
        Phase::Closed => false,
    };
    anyhow::ensure!(valid, "invalid saved research phase transition");
    if prior.selection.is_some() {
        anyhow::ensure!(
            serde_json::to_value(&prior.selection)? == serde_json::to_value(&record.selection)?
                && prior.selection_revision == record.selection_revision
                && prior.selected_reader == record.selected_reader,
            "saved research passage changed after queueing"
        );
    }
    Ok(())
}

fn append(store: &ActionContinuityStore, job: &ReadingJob) -> Result<()> {
    let _lock = action_continuity::research_owner_transaction(store.root())?;
    let prior = latest(store)?;
    validate_successor(prior.as_ref(), job)?;
    if prior
        .as_ref()
        .is_none_or(|prior| prior.phase == Phase::Closed)
    {
        let operation = job.reservation.metadata()["operation_id"].clone();
        anyhow::ensure!(
            !action_continuity::read_native_authority_records(&path(store))?
                .iter()
                .any(|row| row["reservation"]["record"]["operation_id"] == operation),
            "saved research operation already used"
        );
    }
    store.validate_research_reservation(&job.reservation)?;
    // Also reject dangling links before OpenOptions can create their targets.
    anyhow::ensure!(
        !path(store).is_symlink(),
        "saved research handoff is a symlink"
    );
    let mut options = std::fs::OpenOptions::new();
    options.create(true).append(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    let mut file = options.open(path(store))?;
    let mut bytes = serde_json::to_vec(job)?;
    bytes.push(b'\n');
    file.write_all(&bytes)?;
    file.sync_all()?;
    std::fs::File::open(store.root())?.sync_all()?;
    Ok(())
}

pub(in crate::autonomous) fn pending() -> Result<bool> {
    Ok(latest(&ActionContinuityStore::for_astrid_workspace())?
        .is_some_and(|job| job.phase != Phase::Closed))
}

/// This scope is synchronous; neither its thread-local identity nor any lock
/// survives into provider execution. A durable preparation precedes selection.
pub(super) fn dispatch(
    conv: &mut ConversationState,
    reservation: ResearchReservation,
    run: impl FnOnce(&mut ConversationState) -> NextActionOutcome,
) -> Result<NextActionOutcome> {
    let store = ActionContinuityStore::for_astrid_workspace();
    {
        let _lock = action_continuity::research_owner_transaction(store.root())?;
        anyhow::ensure!(
            !latest(&store)?.is_some_and(|job| job.phase != Phase::Closed),
            "saved research reading is pending"
        );
        anyhow::ensure!(
            DISPATCH.with(|slot| slot.borrow().is_none()),
            "nested research reading dispatch"
        );
        append(
            &store,
            &ReadingJob {
                schema_version: 1,
                reservation: reservation.clone(),
                phase: Phase::Preparing,
                selected_reader: None,
                selection_revision: None,
                selection: None,
                delivery: None,
            },
        )?;
    }
    DISPATCH.with(|slot| *slot.borrow_mut() = Some(reservation.clone()));
    let _guard = DispatchGuard;
    let mut outcome = run(conv);
    let mut job = latest(&store)?.context("missing saved research preparation")?;
    match job.phase {
        Phase::Queued => {
            outcome.status = "queued".into();
            outcome.outcome_summary =
                "Bound saved passage queued; generation and delivery have not occurred.".into();
            let mut metadata = reservation.metadata();
            metadata["execution_scope"] = serde_json::json!("saved_text_generation_worker");
            outcome.research_budget_v1 = Some(metadata);
            return Ok(outcome);
        },
        Phase::HostStarted => {
            store.transition_research(
                &reservation,
                if outcome.status == "handled" {
                    "completed"
                } else {
                    "failed"
                },
            )?;
        },
        Phase::Preparing => {
            store.transition_research(&reservation, "released_unstarted")?;
            park_selected(&job, conv)?;
            outcome = NextActionOutcome::blocked(
                "research_admission",
                "No saved passage or host operation was started. Inspect the reading notice; no research allowance was debited.",
            );
        },
        _ => anyhow::bail!("unexpected research dispatch state"),
    }
    job.phase = Phase::Closed;
    append(&store, &job)?;
    Ok(outcome.with_research_budget(reservation.metadata()))
}

/// Called immediately before the existing synchronous PDF/listing/page branch.
pub(super) fn start_host(conv: &mut ConversationState) -> bool {
    let reservation = DISPATCH.with(|slot| slot.borrow().clone());
    let Some(reservation) = reservation else {
        return true;
    };
    let result = (|| -> Result<()> {
        let store = ActionContinuityStore::for_astrid_workspace();
        let _lock = action_continuity::research_owner_transaction(store.root())?;
        let mut job = latest(&store)?.context("missing saved research preparation")?;
        anyhow::ensure!(
            job.phase == Phase::Preparing,
            "host dispatch already claimed"
        );
        // A failed append after the debit is uncertain, never a free retry.
        store.claim_research_start(&reservation)?;
        job.phase = Phase::HostStarted;
        append(&store, &job)
    })();
    if let Err(error) = result {
        conv.pending_file_listing = Some(format!(
            "Bound reading host work did not start: {error:#}. Inspect the operation before retrying."
        ));
        return false;
    }
    true
}

/// Freeze only the actual native offer selected by the ordinary route handler.
pub(super) fn queue_selected(conv: &mut ConversationState) -> Result<()> {
    let reservation = DISPATCH.with(|slot| slot.borrow().clone());
    let Some(reservation) = reservation else {
        return Ok(());
    };
    let store = ActionContinuityStore::for_astrid_workspace();
    {
        let _lock = action_continuity::research_owner_transaction(store.root())?;
        let mut preparing = latest(&store)?.context("missing saved research preparation")?;
        anyhow::ensure!(
            preparing.phase == Phase::Preparing,
            "saved reading already selected"
        );
        preparing.selected_reader = conv.activity.foreground_reader.clone();
        preparing.selection_revision = Some(conv.activity.selection_revision);
        append(&store, &preparing)?;
    }
    let offer =
        activity_reading::offer_requested_reading(conv)?.context("no saved passage available")?;
    let _lock = action_continuity::research_owner_transaction(store.root())?;
    let mut job = latest(&store)?.context("missing saved research preparation")?;
    anyhow::ensure!(
        job.phase == Phase::Preparing,
        "saved reading already selected"
    );
    job.selection = Some(PassageRef {
        reader: offer.reader,
        version: offer.version,
        passage: offer.passage,
    });
    store.queue_saved_reading_research(&reservation)?;
    job.phase = Phase::Queued;
    append(&store, &job)
}

impl ReadingJob {
    fn committed(&self, store: &ActionContinuityStore) -> Result<Option<serde_json::Value>> {
        let selection = self
            .selection
            .as_ref()
            .context("missing saved research passage")?;
        let preview = store
            .reader_bookmark_preview(&selection.reader.thread_id, &selection.reader.session_id)?
            .context("missing saved reader")?;
        let Some(committed) = preview
            .bookmark
            .and_then(|bookmark| bookmark.last_committed_passage)
        else {
            return Ok(None);
        };
        let passage = &committed.passage;
        if passage.offer_id != selection.passage.offer_id {
            return Ok(None);
        }
        anyhow::ensure!(
            passage.source_sha256 == selection.passage.source_sha256
                && passage.start_byte == selection.passage.start_byte
                && passage.end_byte > passage.start_byte
                && passage.end_byte <= selection.passage.end_byte,
            "saved research delivery differs from selected passage"
        );
        let delivery = committed.delivery;
        Ok(Some(
            serde_json::json!({ "scope":"verified_native_reader_commit", "offer_id":passage.offer_id,
            "source_sha256":passage.source_sha256,"start_byte":passage.start_byte,"end_byte":passage.end_byte,
            "final_request_sha256":delivery.final_request_sha256,"output_sha256":delivery.retained_output_sha256,
            "model_input_delivery":"unknown" }),
        ))
    }
}

/// Run before reader preparation/recovery. Interrupted preparation is kept
/// quiet; a previously started job is never automatically claimed again.
pub(in crate::autonomous) fn ready(conv: &mut ConversationState) -> Result<bool> {
    let store = ActionContinuityStore::for_astrid_workspace();
    let Some(mut job) = latest(&store)?.filter(|job| job.phase != Phase::Closed) else {
        return Ok(true);
    };
    if job.phase == Phase::Queued {
        return Ok(true);
    }
    if job.phase == Phase::Started
        && let Some(delivery) = job.committed(&store)?
    {
        store.transition_research(&job.reservation, "completed")?;
        job.delivery = Some(delivery);
        park_selected(&job, conv)?;
        job.phase = Phase::Closed;
        append(&store, &job)?;
        // Recovery must not implicitly begin the following unbound passage.
        return Ok(false);
    }
    // Allow the existing retained-delivery recovery to inspect an accepted
    // completion, but begin() below will still refuse a second worker start.
    if job.phase == Phase::Started {
        return Ok(true);
    }
    conv.pending_file_listing = Some("Bound saved-reading handoff is incomplete or execution is uncertain. Its reservation and passage are retained; no automatic generation retry.".into());
    Ok(false)
}

pub(in crate::autonomous) fn begin(
    conv: &mut ConversationState,
    offer: Option<&ActivityReadingOfferV1>,
) -> Result<Option<ReadingJob>> {
    let store = ActionContinuityStore::for_astrid_workspace();
    let Some(mut job) = latest(&store)?.filter(|job| job.phase != Phase::Closed) else {
        return Ok(None);
    };
    if job.phase == Phase::Started && job.committed(&store)?.is_some() {
        finish(conv, Some(job))?;
        anyhow::bail!("recovered the prior research delivery; no implicit next passage");
    }
    anyhow::ensure!(
        job.phase == Phase::Queued,
        "saved research is not queued; no replay"
    );
    let selected = job
        .selection
        .as_ref()
        .context("missing saved research passage")?;
    let matches = job.selection_revision == Some(conv.activity.selection_revision)
        && offer.is_some_and(|offer| {
            offer.reader == selected.reader
                && offer.version == selected.version
                && offer.passage == selected.passage
        });
    let started = if matches {
        store.claim_research_start(&job.reservation)
    } else {
        Err(anyhow::anyhow!("selected research passage changed"))
    };
    if let Err(error) = started {
        // This only succeeds while the canonical operation is provably unstarted.
        store.transition_research(&job.reservation, "released_unstarted")?;
        park_selected(&job, conv)?;
        job.phase = Phase::Closed;
        append(&store, &job)?;
        return Err(error);
    }
    job.phase = Phase::Started;
    append(&store, &job)?;
    Ok(Some(job))
}

fn park_selected(job: &ReadingJob, conv: &mut ConversationState) -> Result<()> {
    if job.selected_reader.is_some()
        && conv.activity.foreground_reader == job.selected_reader
        && job.selection_revision == Some(conv.activity.selection_revision)
    {
        let store = ActionContinuityStore::for_astrid_workspace();
        let current = activity_reading::load_activity(&store)?;
        if current.selection_revision != conv.activity.selection_revision {
            return Ok(());
        }
        activity_reading::observe_chosen_action(conv, "REST")?;
    }
    Ok(())
}

/// Called after the reader's verified delivery commit, never from response text.
pub(in crate::autonomous) fn finish(
    conv: &mut ConversationState,
    job: Option<ReadingJob>,
) -> Result<()> {
    let Some(mut job) = job else {
        return Ok(());
    };
    let store = ActionContinuityStore::for_astrid_workspace();
    job.delivery = job.committed(&store)?;
    store.transition_research(
        &job.reservation,
        if job.delivery.is_some() {
            "completed"
        } else {
            "failed"
        },
    )?;
    park_selected(&job, conv)?;
    job.phase = Phase::Closed;
    append(&store, &job)
}

#[cfg(test)]
#[path = "research_reading_tests.rs"]
mod tests;
