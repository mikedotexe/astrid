//! Durable, explicitly admitted inbox delivery. Arrival is not an interruption.
//!
//! A receipt retires one immutable content version. Source pathnames are retained:
//! deleting a checked pathname could delete a concurrent publisher's replacement.
//! The queue, rather than directory presence, determines what remains unread.

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

use anyhow::{Result, anyhow, bail};
use serde::{Deserialize, Serialize};

#[path = "durable_inbox/storage.rs"]
mod storage;
#[cfg(test)]
#[path = "durable_inbox/tests.rs"]
mod tests;

pub(crate) const ORDINARY_LETTER_BYTES: usize = 6_000;
const MAX_RETAINED_LETTER_BYTES: usize = 64 * 1024 * 1024;
const MAX_QUEUE_ITEMS: usize = 100_000;
const RETRY_BASE_MS: u64 = 30_000;
const RETRY_MAX_MS: u64 = 15 * 60 * 1_000;

#[derive(Debug, Clone)]
pub(crate) struct DurableInbox {
    inbox_dir: PathBuf,
    queue_root: PathBuf,
}

/// A runtime-created identity, stable for one explicitly eligible receive window.
#[derive(Debug, Clone)]
pub(crate) struct ReceiveWindow {
    pub id: String,
    pub now_unix_ms: u64,
    pub eligible: bool,
    pub max_letter_bytes: usize,
}

/// Counts only: discovery never publishes body text or opens a steward question.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(crate) struct InboxScanSummary {
    pub pending: usize,
    pub needs_explicit_reading_window: usize,
    pub unreadable_sources: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub(crate) struct InboxPendingLetter {
    pub message_id: String,
    pub thread_id: String,
    pub version_id: String,
    pub source_path: PathBuf,
    pub content_sha256: Option<String>,
    pub byte_len: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub(crate) struct InboxReservation {
    pub reservation_id: String,
    pub window_id: String,
    pub letter: InboxPendingLetter,
    pub content_sha256: String,
    pub text: String,
}

/// An unresolved attempt reference, without copying its potentially large body.
#[derive(Debug, Clone)]
pub(crate) struct InboxPendingReservation {
    pub reservation_id: String,
    pub window_id: String,
    pub letter: InboxPendingLetter,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum InboxAdmission {
    Reserved(InboxReservation),
    Empty,
    Ineligible,
    AlreadyAttempted,
    NeedsExplicitReadingWindow(InboxPendingLetter),
}

/// Execution evidence must come from the final provider adapter after completion
/// retention. Caller-supplied hashes alone are not proof that a request ran.
#[derive(Debug, Clone)]
pub(crate) struct InboxDeliveryEvidence {
    pub accepted_attempt_id: String,
    pub submitted_content_sha256: String,
    pub retained_completion_sha256: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub(crate) struct InboxDeliveryReceipt {
    pub reservation_id: String,
    pub version_id: String,
    pub message_id: String,
    pub thread_id: String,
    pub source_path: PathBuf,
    pub archived_path: PathBuf,
    pub content_sha256: String,
    pub accepted_attempt_id: String,
    pub retained_completion_sha256: String,
    pub source_path_retained: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct QueueLetter {
    identity: InboxPendingLetter,
    arrival_sequence: u64,
    attempts: u32,
    retry_after_unix_ms: u64,
    receipt: Option<InboxDeliveryReceipt>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct Attempt {
    version_id: String,
    window_id: String,
    failure: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
struct QueueState {
    schema: String,
    next_sequence: u64,
    letters: BTreeMap<String, QueueLetter>,
    attempts: BTreeMap<String, Attempt>,
    windows: BTreeSet<String>,
    thread_last_attempt: BTreeMap<String, u64>,
    source_observations: BTreeMap<String, String>,
}

impl Default for QueueState {
    fn default() -> Self {
        Self {
            schema: "durable_inbox_v1".to_string(),
            next_sequence: 1,
            letters: BTreeMap::new(),
            attempts: BTreeMap::new(),
            windows: BTreeSet::new(),
            thread_last_attempt: BTreeMap::new(),
            source_observations: BTreeMap::new(),
        }
    }
}

impl DurableInbox {
    /// The return surface follows this configured inbox, including isolated tests.
    pub(crate) fn outbox_dir(&self) -> Result<PathBuf> {
        Ok(self
            .inbox_dir
            .parent()
            .ok_or_else(|| anyhow!("inbox has no containing workspace"))?
            .join("outbox"))
    }

    pub(crate) fn new(inbox_dir: &Path, queue_root: &Path) -> Self {
        Self {
            inbox_dir: inbox_dir.to_path_buf(),
            queue_root: queue_root.to_path_buf(),
        }
    }

    pub(crate) fn scan(&self) -> Result<InboxScanSummary> {
        let mut locked = storage::LockedQueue::open(self)?;
        let unreadable_sources = storage::discover(self, &mut locked.state)?;
        let mut summary = InboxScanSummary {
            unreadable_sources,
            ..InboxScanSummary::default()
        };
        for letter in locked
            .state
            .letters
            .values()
            .filter(|letter| letter.receipt.is_none())
        {
            summary.pending = summary.pending.saturating_add(1);
            if letter.identity.byte_len > ORDINARY_LETTER_BYTES as u64 {
                summary.needs_explicit_reading_window =
                    summary.needs_explicit_reading_window.saturating_add(1);
            }
        }
        locked.save()?;
        Ok(summary)
    }

    /// Reserve at most one intact letter per window, with FIFO within each thread
    /// and least-recently-served ordering between eligible thread heads.
    pub(crate) fn reserve(&self, window: &ReceiveWindow) -> Result<InboxAdmission> {
        if !window.eligible {
            return Ok(InboxAdmission::Ineligible);
        }
        validate_id(&window.id)?;
        if window.max_letter_bytes == 0 || window.max_letter_bytes > MAX_RETAINED_LETTER_BYTES {
            bail!("receive window byte allowance is outside the supported bounds");
        }
        let mut locked = storage::LockedQueue::open(self)?;
        if locked.state.windows.contains(&window.id) {
            return Ok(InboxAdmission::AlreadyAttempted);
        }
        storage::discover(self, &mut locked.state)?;
        if locked.state.windows.len() >= MAX_QUEUE_ITEMS
            || locked.state.attempts.len() >= MAX_QUEUE_ITEMS
        {
            bail!("inbox delivery history reached its reviewed capacity; nothing was discarded");
        }
        let chosen = choose_letter(&locked.state, window);
        // An empty/backed-off/oversized window is consumed too: a loop cannot
        // reread this boundary until another message happens to arrive.
        locked.state.windows.insert(window.id.clone());
        let Some(version_id) = chosen else {
            locked.save()?;
            return Ok(InboxAdmission::Empty);
        };
        let selected = locked
            .state
            .letters
            .get(&version_id)
            .ok_or_else(|| anyhow!("selected inbox version disappeared"))?;
        if selected.identity.byte_len > window.max_letter_bytes as u64
            || selected.identity.content_sha256.is_none()
        {
            let pending = selected.identity.clone();
            // Oversized heads participate in fairness without admitting a body.
            let thread_id = pending.thread_id.clone();
            let sequence = advance_sequence(&mut locked.state)?;
            locked.state.thread_last_attempt.insert(thread_id, sequence);
            locked.save()?;
            return Ok(InboxAdmission::NeedsExplicitReadingWindow(pending));
        }
        let text = storage::retained_text(self, &selected.identity)?;
        let reservation_id = format!(
            "inbox_attempt_{}",
            storage::hash(format!("{}\0{version_id}", window.id).as_bytes())
        );
        let content_sha256 = selected
            .identity
            .content_sha256
            .clone()
            .ok_or_else(|| anyhow!("retained letter has no digest"))?;
        let reservation = InboxReservation {
            reservation_id: reservation_id.clone(),
            window_id: window.id.clone(),
            letter: selected.identity.clone(),
            content_sha256,
            text,
        };
        let sequence = advance_sequence(&mut locked.state)?;
        locked
            .state
            .thread_last_attempt
            .insert(reservation.letter.thread_id.clone(), sequence);
        let letter = locked
            .state
            .letters
            .get_mut(&version_id)
            .ok_or_else(|| anyhow!("selected inbox version disappeared"))?;
        letter.attempts = letter.attempts.saturating_add(1);
        letter.retry_after_unix_ms = window
            .now_unix_ms
            .saturating_add(retry_delay(letter.attempts));
        locked.state.attempts.insert(
            reservation_id,
            Attempt {
                version_id,
                window_id: window.id.clone(),
                failure: None,
            },
        );
        locked.save()?;
        Ok(InboxAdmission::Reserved(reservation))
    }

    /// Recover unresolved attempt identities without reserving another window.
    pub(crate) fn pending_reservations(&self) -> Result<Vec<InboxPendingReservation>> {
        let locked = storage::LockedQueue::open(self)?;
        Ok(locked
            .state
            .attempts
            .iter()
            .filter_map(|(reservation_id, attempt)| {
                let letter = locked.state.letters.get(&attempt.version_id)?;
                if letter.receipt.is_some() {
                    return None;
                }
                Some(InboxPendingReservation {
                    reservation_id: reservation_id.clone(),
                    window_id: attempt.window_id.clone(),
                    letter: letter.identity.clone(),
                })
            })
            .collect())
    }

    /// Restore exact offered bytes for receipt reconciliation, never generation.
    pub(crate) fn recover_reservation(
        &self,
        reservation_id: &str,
    ) -> Result<Option<InboxReservation>> {
        let locked = storage::LockedQueue::open(self)?;
        let Some(attempt) = locked.state.attempts.get(reservation_id) else {
            return Ok(None);
        };
        let letter = locked
            .state
            .letters
            .get(&attempt.version_id)
            .ok_or_else(|| anyhow!("unknown inbox attempt version"))?;
        let text = storage::retained_text(self, &letter.identity)?;
        Ok(Some(InboxReservation {
            reservation_id: reservation_id.to_string(),
            window_id: attempt.window_id.clone(),
            letter: letter.identity.clone(),
            content_sha256: letter
                .identity
                .content_sha256
                .clone()
                .ok_or_else(|| anyhow!("attempt has no retained source"))?,
            text,
        }))
    }

    /// A failure leaves the exact retained letter pending. Time is an admission
    /// backoff, never a timer that creates an unsolicited receive window.
    pub(crate) fn fail(
        &self,
        reservation: &InboxReservation,
        now_unix_ms: u64,
        reason: &str,
    ) -> Result<()> {
        let mut locked = storage::LockedQueue::open(self)?;
        validate_reservation(self, &locked.state, reservation)?;
        let attempt = locked
            .state
            .attempts
            .get_mut(&reservation.reservation_id)
            .ok_or_else(|| anyhow!("unknown inbox reservation"))?;
        if attempt.failure.is_some() {
            return Ok(());
        }
        attempt.failure = Some(reason.chars().take(160).collect());
        let letter = locked
            .state
            .letters
            .get_mut(&reservation.letter.version_id)
            .ok_or_else(|| anyhow!("unknown inbox version"))?;
        if letter.receipt.is_none() {
            letter.retry_after_unix_ms = letter
                .retry_after_unix_ms
                .max(now_unix_ms.saturating_add(retry_delay(letter.attempts)));
        }
        locked.save()
    }

    /// Acknowledge only an exact reservation after verified intact submission and
    /// durable completion retention. This never sweeps other filenames/versions.
    pub(crate) fn acknowledge(
        &self,
        reservation: &InboxReservation,
        evidence: &InboxDeliveryEvidence,
    ) -> Result<InboxDeliveryReceipt> {
        self.acknowledge_inner(reservation, evidence, None::<fn() -> Result<()>>)
    }

    /// Run recoverable local publication after reservation validation and before
    /// acknowledgement, under the same lock. The callback must be idempotent and
    /// must not call back into the queue or run model/actions/peer forwarding.
    pub(crate) fn acknowledge_with_precommit(
        &self,
        reservation: &InboxReservation,
        evidence: &InboxDeliveryEvidence,
        before_acknowledge: impl FnOnce() -> Result<()>,
    ) -> Result<InboxDeliveryReceipt> {
        self.acknowledge_inner(reservation, evidence, Some(before_acknowledge))
    }

    fn acknowledge_inner(
        &self,
        reservation: &InboxReservation,
        evidence: &InboxDeliveryEvidence,
        before_acknowledge: Option<impl FnOnce() -> Result<()>>,
    ) -> Result<InboxDeliveryReceipt> {
        validate_id(&evidence.accepted_attempt_id)?;
        if !storage::valid_hash(&evidence.retained_completion_sha256)
            || evidence.submitted_content_sha256 != reservation.content_sha256
        {
            bail!("inbox completion does not attest the intact reserved content");
        }
        let mut locked = storage::LockedQueue::open(self)?;
        validate_reservation(self, &locked.state, reservation)?;
        let letter = locked
            .state
            .letters
            .get(&reservation.letter.version_id)
            .ok_or_else(|| anyhow!("unknown inbox version"))?;
        if let Some(before_acknowledge) = before_acknowledge {
            if let Some(receipt) = &letter.receipt
                && (receipt.accepted_attempt_id != evidence.accepted_attempt_id
                    || receipt.retained_completion_sha256 != evidence.retained_completion_sha256)
            {
                bail!("a different completion already acknowledged this source version");
            }
            before_acknowledge()?;
        }
        if let Some(receipt) = &letter.receipt {
            return Ok(receipt.clone());
        }
        let archived_path = storage::archive(self, reservation)?;
        let receipt = InboxDeliveryReceipt {
            reservation_id: reservation.reservation_id.clone(),
            version_id: reservation.letter.version_id.clone(),
            message_id: reservation.letter.message_id.clone(),
            thread_id: reservation.letter.thread_id.clone(),
            source_path: reservation.letter.source_path.clone(),
            archived_path,
            content_sha256: reservation.content_sha256.clone(),
            accepted_attempt_id: evidence.accepted_attempt_id.clone(),
            retained_completion_sha256: evidence.retained_completion_sha256.clone(),
            source_path_retained: true,
        };
        locked
            .state
            .letters
            .get_mut(&reservation.letter.version_id)
            .ok_or_else(|| anyhow!("unknown inbox version"))?
            .receipt = Some(receipt.clone());
        locked.save()?;
        Ok(receipt)
    }
}

fn validate_id(value: &str) -> Result<()> {
    if value.trim().is_empty() || value.len() > 512 || value.chars().any(char::is_control) {
        bail!("invalid inbox window/attempt identity");
    }
    Ok(())
}

fn advance_sequence(state: &mut QueueState) -> Result<u64> {
    let sequence = state.next_sequence;
    state.next_sequence = sequence
        .checked_add(1)
        .ok_or_else(|| anyhow!("inbox sequence exhausted"))?;
    Ok(sequence)
}

fn retry_delay(attempts: u32) -> u64 {
    RETRY_BASE_MS
        .saturating_mul(
            1_u64
                .checked_shl(attempts.saturating_sub(1).min(8))
                .unwrap_or(u64::MAX),
        )
        .min(RETRY_MAX_MS)
}

fn choose_letter(state: &QueueState, window: &ReceiveWindow) -> Option<String> {
    let mut heads: BTreeMap<&str, &QueueLetter> = BTreeMap::new();
    for letter in state
        .letters
        .values()
        .filter(|letter| letter.receipt.is_none())
    {
        heads
            .entry(&letter.identity.thread_id)
            .and_modify(|head| {
                if letter.arrival_sequence < head.arrival_sequence {
                    *head = letter;
                }
            })
            .or_insert(letter);
    }
    heads
        .values()
        .filter(|letter| letter.retry_after_unix_ms <= window.now_unix_ms)
        .min_by_key(|letter| {
            (
                state
                    .thread_last_attempt
                    .get(&letter.identity.thread_id)
                    .copied()
                    .unwrap_or(0),
                letter.arrival_sequence,
                &letter.identity.version_id,
            )
        })
        .map(|letter| letter.identity.version_id.clone())
}

fn validate_reservation(
    queue: &DurableInbox,
    state: &QueueState,
    reservation: &InboxReservation,
) -> Result<()> {
    let attempt = state
        .attempts
        .get(&reservation.reservation_id)
        .ok_or_else(|| anyhow!("unknown inbox reservation"))?;
    let letter = state
        .letters
        .get(&attempt.version_id)
        .ok_or_else(|| anyhow!("unknown inbox version"))?;
    if attempt.window_id != reservation.window_id
        || letter.identity != reservation.letter
        || reservation.letter.content_sha256.as_deref() != Some(reservation.content_sha256.as_str())
        || storage::hash(reservation.text.as_bytes()) != reservation.content_sha256
        || storage::retained_text(queue, &letter.identity)? != reservation.text
    {
        bail!("inbox reservation identity or retained bytes changed");
    }
    Ok(())
}
