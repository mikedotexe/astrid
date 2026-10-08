//! Explicit operator review releases a dispatch, never verifies missing delivery.
use super::*;
use std::io::Read as _;

const SCHEMA: &str = "study_uncertainty_review_v1";
const ACK: &str = "retire-exact-dispatch-unresolved-without-retry-or-next";
const MAX_REVIEW_BYTES: u64 = 16 * 1024;

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
struct Review {
    schema: String,
    operation_id: String,
    actor: String,
    acknowledgement: String,
    job_id: String,
    expected_selection_revision: u64,
    expected_intent_sha256: String,
    expected_output_sha256: String,
    evidence_sha256: String,
    provider_quiescent_observed: bool,
}

fn validate(review: &Review) -> Result<()> {
    let identifier = |s: &str| {
        !s.is_empty()
            && s.len() <= 128
            && s.bytes()
                .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'-' | b'_'))
    };
    ensure!(
        review.schema == SCHEMA && review.acknowledgement == ACK,
        "explicit unresolved-dispatch review acknowledgement required"
    );
    ensure!(
        identifier(&review.actor) && identifier(&review.operation_id),
        "invalid operator review identity"
    );
    ensure!(
        review
            .job_id
            .strip_prefix("study-job-")
            .is_some_and(valid_hash)
            && valid_hash(&review.expected_intent_sha256)
            && valid_hash(&review.expected_output_sha256)
            && valid_hash(&review.evidence_sha256),
        "invalid review references"
    );
    ensure!(
        review.provider_quiescent_observed,
        "operator must verify provider quiescence before staging review"
    );
    Ok(())
}

fn read_review(path: &Path) -> Result<Review> {
    let file = std::fs::OpenOptions::new()
        .read(true)
        .custom_flags(libc::O_NOFOLLOW | libc::O_NONBLOCK)
        .open(path)?;
    let metadata = file.metadata()?;
    ensure!(
        metadata.is_file() && metadata.len() <= MAX_REVIEW_BYTES,
        "invalid bounded review file"
    );
    let mut bytes = Vec::new();
    file.take(MAX_REVIEW_BYTES + 1).read_to_end(&mut bytes)?;
    ensure!(
        bytes.len() as u64 <= MAX_REVIEW_BYTES,
        "review exceeds bound"
    );
    // Do not return serde's diagnostic, which could contain supplied private text.
    let review: Review =
        serde_json::from_slice(&bytes).map_err(|_| anyhow::anyhow!("invalid review JSON"))?;
    validate(&review)?;
    Ok(review)
}

fn request_path(store: &ActionContinuityStore, id: &str, revision: u64) -> Result<PathBuf> {
    ensure!(
        id.strip_prefix("study-job-").is_some_and(valid_hash),
        "invalid review job ID"
    );
    Ok(root(store)?.join(format!("review-{id}-r{revision}.json")))
}

fn review_exists(path: &Path) -> Result<bool> {
    match fs::symlink_metadata(path) {
        Ok(_) => Ok(true),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(false),
        Err(error) => Err(error.into()),
    }
}

fn validate_binding(
    store: &ActionContinuityStore,
    selection: &activity_reading::ActivityRuntimeV1,
    review: &Review,
) -> Result<()> {
    validate(review)?;
    ensure!(
        selection.selection_revision == review.expected_selection_revision,
        "activity revision changed; a fresh review is required"
    );
    ensure!(
        selection.native_focus_window.is_none()
            && selection.study_handoff.active.as_deref() == Some(review.job_id.as_str()),
        "review does not own the current dispatch"
    );
    let job = selection
        .study_handoff
        .jobs
        .get(&review.job_id)
        .context("review job missing")?;
    ensure!(
        job.phase == Phase::Claimed
            && job.intent == review.expected_intent_sha256
            && job.output.as_deref() == Some(review.expected_output_sha256.as_str()),
        "review does not match the claimed dispatch"
    );
    intent(store, &review.job_id, job)?;
    let _: astrid_source_study::StudyOutput = read(store, &review.expected_output_sha256)?;
    Ok(())
}

fn stage_review(
    store: &ActionContinuityStore,
    deliveries: &Path,
    review: &Review,
) -> Result<serde_json::Value> {
    let _lock = store.reader_owner_transaction()?;
    validate(review)?;
    let selection = activity_reading::load_activity(store)?;
    let path = request_path(store, &review.job_id, review.expected_selection_revision)?;
    let existing = if review_exists(&path)? {
        Some(read_review(&path)?)
    } else {
        None
    };
    if let Some(existing) = &existing {
        ensure!(
            existing == review,
            "conflicting review retry; original review preserved"
        );
        if let Some(job) = selection.study_handoff.jobs.get(&review.job_id)
            && job.phase == Phase::RetiredUnresolved
        {
            let saved: Review = read(
                store,
                job.uncertainty_review
                    .as_deref()
                    .context("review receipt missing")?,
            )?;
            ensure!(saved == *review, "retired review receipt differs");
            return Ok(receipt(review, "already_applied"));
        }
    }
    validate_binding(store, &selection, review)?;
    ensure!(
        crate::llm::recover_retained_delivery_at(deliveries, &content_id(&review.job_id), 0)?
            .is_none(),
        "verified delivery exists; use normal delivery reconciliation"
    );
    if existing.is_none() {
        // This commits permission only. Runtime reconciliation owns the phase
        // transition, outside inference and under the same owner transaction.
        let bytes = serde_json::to_vec(review)?;
        crate::lifecycle::atomic_private_write(&path, &bytes)?;
    }
    Ok(receipt(review, "staged_not_applied"))
}

fn receipt(review: &Review, status: &str) -> serde_json::Value {
    serde_json::json!({"schema":SCHEMA,"status":status,"job_id":review.job_id,
        "operation_id":review.operation_id,"review_sha256":digest(serde_json::to_vec(review).expect("review serializes")),
        "delivery_verified":false,"generation_retried":false,"next_dispatched":false})
}

pub(crate) fn stage_configured_review(path: &Path) -> Result<serde_json::Value> {
    let review = read_review(path)?;
    stage_review(
        &ActionContinuityStore::for_astrid_workspace(),
        &crate::paths::bridge_paths()
            .bridge_workspace()
            .join("diagnostics/accepted_deliveries"),
        &review,
    )
}

pub(super) fn verify_retired(store: &ActionContinuityStore, id: &str, job: &Job) -> Result<()> {
    if job.phase == Phase::RetiredUnresolved {
        let review: Review = read(
            store,
            job.uncertainty_review
                .as_deref()
                .context("review receipt missing")?,
        )?;
        validate(&review)?;
        ensure!(
            review.job_id == id
                && review.expected_intent_sha256 == job.intent
                && Some(review.expected_output_sha256.as_str()) == job.output.as_deref(),
            "retired review differs from retained dispatch"
        );
    }
    Ok(())
}

pub(super) fn apply_staged_review(
    store: &ActionContinuityStore,
    conv: &mut ConversationState,
    selection: &activity_reading::ActivityRuntimeV1,
    id: &str,
) -> Result<bool> {
    let path = request_path(store, id, selection.selection_revision)?;
    if !review_exists(&path)? {
        return Ok(false);
    }
    let review = read_review(&path)?;
    validate_binding(store, selection, &review)?;
    let mut next = selection.clone();
    let job = selected(&mut next, id)?;
    // Retain/fsync the immutable review before the atomic activity publication.
    // A crash between these writes leaves a retryable review, not a partial release.
    job.uncertainty_review = Some(retain(store, &review)?);
    job.phase = Phase::RetiredUnresolved;
    next.study_handoff.active = None;
    conv.activity = activity_reading::persist_activity(store, &next)?;
    conv.introspect_target = None;
    conv.wants_introspect = false;
    conv.pending_file_listing = Some(format!(
        "Operator recovery retired dispatch {id} as unresolved. No delivery or completion was verified, and neither generation nor NEXT was replayed. Existing authored records and pending reader input remain unchanged. A future continuation requires an explicit new choice."
    ));
    Ok(true)
}

#[cfg(test)]
#[path = "study_handoff_review_tests.rs"]
mod tests;
