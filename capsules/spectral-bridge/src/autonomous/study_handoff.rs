//! Durable ordinary-study handoff. Activity state holds references, not prose.
use anyhow::{Context as _, Result, ensure};
use serde::{Deserialize, Serialize};
use sha2::{Digest as _, Sha256};
use std::{
    collections::BTreeMap,
    fs,
    io::Write as _,
    os::unix::fs::{DirBuilderExt as _, OpenOptionsExt as _},
    path::{Path, PathBuf},
};

use super::{
    activity_reading,
    state::{ConversationState, IntrospectTargetV2},
};
use crate::{action_continuity::ActionContinuityStore, llm::PromptDeliveryReceiptV1};

const MAX_JOBS: usize = 65_536;
const MAX_ARTIFACT: u64 = 1024 * 1024;

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct HandoffState {
    active: Option<String>,
    jobs: BTreeMap<String, Job>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Job {
    intent: String,
    output: Option<String>,
    phase: Phase,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
enum Phase {
    Queued,
    Prepared,
    Claimed,
    Delivered,
    Failed,
    Superseded,
}

impl Phase {
    fn terminal(self) -> bool {
        matches!(self, Self::Delivered | Self::Failed | Self::Superseded)
    }
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Intent {
    owner_root: PathBuf,
    target: IntrospectTargetV2,
}

fn digest(bytes: impl AsRef<[u8]>) -> String {
    format!("{:x}", Sha256::digest(bytes))
}
pub(super) fn shared_target(target: &IntrospectTargetV2) -> bool {
    if target.label.starts_with("SELF_STUDY")
        || super::next_action::study_navigation::private(target)
    {
        return true;
    }
    let sources = super::introspect::introspect_sources();
    let Ok(resolved) = super::introspect::resolve_introspect_target_result(&target.label, &sources)
    else {
        return true;
    };
    let paths = crate::paths::bridge_paths();
    !resolved.path.starts_with(paths.bridge_workspace())
        && !resolved.path.starts_with(paths.minime_workspace())
}
fn valid_hash(value: &str) -> bool {
    value.len() == 64
        && value
            .bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
}

impl HandoffState {
    pub(super) fn pending(&self) -> bool {
        self.active.is_some()
    }
    pub(super) fn status(&self) -> String {
        let active = self
            .active
            .as_ref()
            .and_then(|id| {
                self.jobs
                    .get(id)
                    .map(|job| format!("{id}: {:?}", job.phase))
            })
            .unwrap_or_else(|| "none".into());
        format!(
            "Study handoff: {active}; {} retained operation identities. Claimed work without a verified delivery requires recovery review, not automatic retry. Native study/draft records retain authored responses and NEXT; status does not dispatch them.",
            self.jobs.len()
        )
    }
    pub(super) fn validate(&self) -> Result<()> {
        ensure!(
            self.jobs.len() <= MAX_JOBS,
            "study handoff history full; retained unchanged"
        );
        for (id, job) in &self.jobs {
            ensure!(
                id.strip_prefix("study-job-").is_some_and(valid_hash)
                    && valid_hash(&job.intent)
                    && job.output.as_deref().is_none_or(valid_hash),
                "invalid study handoff identity"
            );
            ensure!(
                !matches!(
                    job.phase,
                    Phase::Prepared | Phase::Claimed | Phase::Delivered
                ) || job.output.is_some(),
                "study handoff input missing"
            );
            ensure!(
                job.phase.terminal() || self.active.as_ref() == Some(id),
                "unselected unfinished study job"
            );
        }
        if let Some(id) = &self.active {
            ensure!(
                self.jobs.get(id).is_some_and(|j| !j.phase.terminal()),
                "invalid active study job"
            );
        }
        Ok(())
    }
}

fn root(store: &ActionContinuityStore) -> Result<PathBuf> {
    let path = store
        .root()
        .parent()
        .context("study owner workspace missing")?
        .join("diagnostics/source_first_v3/shared_reader/study-handoff");
    for ancestor in path.ancestors() {
        if let Ok(meta) = fs::symlink_metadata(ancestor) {
            ensure!(
                !meta.file_type().is_symlink(),
                "symlink study handoff path refused"
            );
        }
    }
    Ok(path)
}

fn retain<T: Serialize>(store: &ActionContinuityStore, value: &T) -> Result<String> {
    let bytes = serde_json::to_vec(value)?;
    ensure!(
        u64::try_from(bytes.len())? <= MAX_ARTIFACT,
        "study handoff artifact too large"
    );
    let hash = digest(&bytes);
    let directory = root(store)?;
    fs::DirBuilder::new()
        .recursive(true)
        .mode(0o700)
        .create(&directory)?;
    let path = directory.join(format!("{hash}.json"));
    if fs::symlink_metadata(&path).is_ok() {
        ensure!(
            !path.is_symlink() && fs::read(&path)? == bytes,
            "study handoff artifact changed"
        );
        fs::File::open(&path)?.sync_all()?;
        fs::File::open(&directory)?.sync_all()?;
        return Ok(hash);
    }
    let temporary = directory.join(format!(".{:032x}.tmp", rand::random::<u128>()));
    let mut file = fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(0o600)
        .open(&temporary)?;
    file.write_all(&bytes)?;
    file.sync_all()?;
    fs::rename(&temporary, &path)?;
    fs::File::open(&directory)?.sync_all()?;
    Ok(hash)
}

fn read<T: serde::de::DeserializeOwned>(store: &ActionContinuityStore, hash: &str) -> Result<T> {
    ensure!(valid_hash(hash), "invalid study artifact reference");
    let path = root(store)?.join(format!("{hash}.json"));
    let meta = fs::symlink_metadata(&path)?;
    ensure!(
        meta.is_file() && !meta.file_type().is_symlink() && meta.len() <= MAX_ARTIFACT,
        "invalid retained study artifact"
    );
    let bytes = fs::read(path)?;
    ensure!(
        digest(&bytes) == hash,
        "retained study artifact hash mismatch"
    );
    Ok(serde_json::from_slice(&bytes)?)
}

fn intent(store: &ActionContinuityStore, id: &str, job: &Job) -> Result<IntrospectTargetV2> {
    let record: Intent = read(store, &job.intent)?;
    ensure!(
        record.owner_root == store.root() && record.target.operation_id.as_deref() == Some(id),
        "study handoff owner or operation mismatch"
    );
    Ok(record.target)
}

/// Called after action authorization, before the host reports successful queueing.
/// An exact retry of a completed operation never makes it current again.
pub(super) fn queue(
    store: &ActionContinuityStore,
    conv: &mut ConversationState,
    target: &mut Option<IntrospectTargetV2>,
    operation: &str,
    replace: bool,
) -> Result<bool> {
    ensure!(
        !operation.is_empty() && operation.len() <= 512,
        "durable study operation identity required"
    );
    let _lock = store.reader_owner_transaction()?;
    let mut selection = activity_reading::load_activity(store)?;
    ensure!(
        selection.native_focus_window.is_none(),
        "protected focus owns study admission"
    );
    let id = format!("study-job-{}", digest(operation));
    let mut requested = target
        .clone()
        .unwrap_or_else(|| IntrospectTargetV2::auto("SELF_STUDY CONTINUE".into()));
    ensure!(
        requested.label.len() <= 32 * 1024,
        "study request exceeds bound; not queued"
    );
    requested.operation_id = Some(id.clone());
    let record = Intent {
        owner_root: store.root().into(),
        target: requested.clone(),
    };
    let hash = digest(serde_json::to_vec(&record)?);
    let handoff = &mut selection.study_handoff;
    if let Some(job) = handoff.jobs.get(&id) {
        ensure!(job.intent == hash, "conflicting study operation retry");
        intent(store, &id, job)?;
        if job.phase.terminal() {
            return Ok(false);
        }
        ensure!(
            handoff.active.as_ref() == Some(&id),
            "study retry is not selected"
        );
        *target = Some(requested);
        conv.activity = selection;
        return Ok(true);
    }
    ensure!(
        handoff.jobs.len() < MAX_JOBS,
        "study handoff history full; retained unchanged"
    );
    if let Some(active) = &handoff.active {
        let previous = handoff
            .jobs
            .get_mut(active)
            .context("active study missing")?;
        ensure!(
            replace && matches!(previous.phase, Phase::Queued | Phase::Prepared),
            "pending or uncertain study preserved; later choice was not queued"
        );
        previous.phase = Phase::Superseded;
    }
    retain(store, &record)?;
    handoff.jobs.insert(
        id.clone(),
        Job {
            intent: hash,
            output: None,
            phase: Phase::Queued,
        },
    );
    handoff.active = Some(id);
    conv.activity = activity_reading::persist_activity(store, &selection)?;
    *target = Some(requested);
    Ok(true)
}

fn selected<'a>(
    selection: &'a mut activity_reading::ActivityRuntimeV1,
    id: &str,
) -> Result<&'a mut Job> {
    ensure!(
        selection.study_handoff.active.as_deref() == Some(id),
        "study job is not selected"
    );
    selection
        .study_handoff
        .jobs
        .get_mut(id)
        .context("study job missing")
}

pub(super) fn verify_pending(
    store: &ActionContinuityStore,
    conv: &mut ConversationState,
    target: Option<&IntrospectTargetV2>,
) -> Result<bool> {
    let _lock = store.reader_owner_transaction()?;
    let selection = activity_reading::load_activity(store)?;
    let target = target.context("legacy pending study requires explicit reselection")?;
    let id = target
        .operation_id
        .as_deref()
        .context("pending study has no durable identity")?;
    let job = selection
        .study_handoff
        .jobs
        .get(id)
        .context("pending study identity is not retained")?;
    ensure!(
        intent(store, id, job)? == *target,
        "pending study differs from its retained choice"
    );
    if job.phase.terminal() {
        return Ok(false);
    }
    ensure!(
        selection.study_handoff.active.as_deref() == Some(id)
            && matches!(job.phase, Phase::Queued | Phase::Prepared),
        "pending study is already claimed; recovery review required"
    );
    conv.activity = selection;
    Ok(true)
}

/// Freeze preparation before provider admission. Native prepare_once owns its own
/// redo transaction; retrying this boundary returns that exact prepared input.
pub(super) fn prepare(
    store: &ActionContinuityStore,
    conv: &mut ConversationState,
    id: &str,
    build: impl FnOnce() -> Result<astrid_source_study::StudyOutput>,
) -> Result<astrid_source_study::StudyOutput> {
    let _lock = store.reader_owner_transaction()?;
    let mut selection = activity_reading::load_activity(store)?;
    let job = selected(&mut selection, id)?;
    intent(store, id, job)?;
    ensure!(
        matches!(job.phase, Phase::Queued | Phase::Prepared),
        "study invocation already claimed; recovery required"
    );
    if let Some(hash) = &job.output {
        return read(store, hash);
    }
    let output = build()?;
    job.output = Some(retain(store, &output)?);
    job.phase = Phase::Prepared;
    conv.activity = activity_reading::persist_activity(store, &selection)?;
    Ok(output)
}

pub(super) fn claim(
    store: &ActionContinuityStore,
    conv: &mut ConversationState,
    id: &str,
) -> Result<()> {
    let _lock = store.reader_owner_transaction()?;
    let mut selection = activity_reading::load_activity(store)?;
    let job = selected(&mut selection, id)?;
    intent(store, id, job)?;
    ensure!(
        job.phase == Phase::Prepared,
        "study provider invocation already claimed or not prepared"
    );
    let _: astrid_source_study::StudyOutput = read(
        store,
        job.output.as_deref().context("prepared input missing")?,
    )?;
    job.phase = Phase::Claimed;
    conv.activity = activity_reading::persist_activity(store, &selection)?;
    Ok(())
}

/// A known failure terminates this dispatch, not its question or pending page.
pub(super) fn failed(
    store: &ActionContinuityStore,
    conv: &mut ConversationState,
    id: &str,
    provider_returned: bool,
) -> Result<()> {
    let _lock = store.reader_owner_transaction()?;
    let mut selection = activity_reading::load_activity(store)?;
    let job = selected(&mut selection, id)?;
    intent(store, id, job)?;
    ensure!(
        if provider_returned {
            job.phase == Phase::Claimed
        } else {
            matches!(job.phase, Phase::Queued | Phase::Prepared)
        },
        "failure does not match the admission phase; uncertain work retained"
    );
    job.phase = Phase::Failed;
    selection.study_handoff.active = None;
    conv.activity = activity_reading::persist_activity(store, &selection)?;
    Ok(())
}

pub(crate) fn content_id(id: &str) -> String {
    format!("source-job:{id}")
}

pub(super) fn ensure_run_job(conv: &mut ConversationState) -> Result<Option<String>> {
    if conv.activity.native_focus_window.is_some() {
        return Ok(None);
    }
    if let Some(target) = &conv.introspect_target {
        let id = target
            .operation_id
            .as_deref()
            .context("legacy study has no durable handoff; explicitly reselect it")?;
        ensure!(
            id.starts_with("study-job-"),
            "legacy study handoff is unverified; explicitly replace/reselect it"
        );
        return Ok(Some(id.into()));
    }
    // An untargeted scheduled study has not yet accepted a specific input.
    // Persist a unique admission now; restart reads that reference, not a new ID.
    let mut target = None;
    queue(
        &ActionContinuityStore::for_astrid_workspace(),
        conv,
        &mut target,
        &format!("scheduled-study-{:032x}", rand::random::<u128>()),
        false,
    )?;
    let id = target.as_ref().and_then(|t| t.operation_id.clone());
    conv.introspect_target = target;
    Ok(id)
}

pub(super) fn validate_sources(
    output: &astrid_source_study::StudyOutput,
    catalog: &astrid_source_study::Catalog,
) -> Result<()> {
    for page in output.page.iter().chain(&output.session_pages) {
        let source = catalog.resolve(&page.source)?;
        ensure!(
            digest(fs::read(source.path)?) == page.revision.sha256,
            "prepared study source changed; explicit reselection required"
        );
    }
    Ok(())
}

fn validate_receipt(
    output: &astrid_source_study::StudyOutput,
    id: &str,
    receipt: &PromptDeliveryReceiptV1,
) -> Result<()> {
    crate::llm::verify_delivery_receipt(receipt)?;
    ensure!(
        receipt.content_id == content_id(id),
        "study receipt belongs to another operation"
    );
    let artifact: serde_json::Value =
        serde_json::from_slice(&fs::read(&receipt.retained_artifact_path)?)?;
    output.verify_delivery(
        artifact["attempt"]["request_json"]
            .as_str()
            .context("retained request missing")?,
        artifact["attempt"]["response_json"]
            .as_str()
            .context("retained response missing")?,
    )?;
    Ok(())
}

pub(super) fn delivered(
    store: &ActionContinuityStore,
    conv: &mut ConversationState,
    id: &str,
    reader: &astrid_source_study::Reader,
    receipt: &PromptDeliveryReceiptV1,
) -> Result<()> {
    let _lock = store.reader_owner_transaction()?;
    let mut selection = activity_reading::load_activity(store)?;
    let job = selected(&mut selection, id)?;
    ensure!(
        job.phase == Phase::Claimed,
        "study delivery was not admitted"
    );
    intent(store, id, job)?;
    let output: astrid_source_study::StudyOutput = read(
        store,
        job.output.as_deref().context("prepared input missing")?,
    )?;
    validate_receipt(&output, id, receipt)?;
    if let Some(page) = &output.page {
        reader.delivered_artifact(&page.id, Path::new(&receipt.retained_artifact_path))?;
    } else {
        reader.navigation_delivered_artifact(
            output
                .navigation_id
                .as_deref()
                .context("study input ID missing")?,
            Path::new(&receipt.retained_artifact_path),
        )?;
    }
    job.phase = Phase::Delivered;
    selection.study_handoff.active = None;
    conv.activity = activity_reading::persist_activity(store, &selection)?;
    Ok(())
}

/// Recover accepted work, not arbitrary NEXT actions. A delivery survives in its
/// native notebook/draft and accepted artifact even if downstream dispatch never ran.
pub(super) fn reconcile(
    store: &ActionContinuityStore,
    conv: &mut ConversationState,
    reader: &astrid_source_study::Reader,
    deliveries: &Path,
) -> Result<()> {
    let _lock = store.reader_owner_transaction()?;
    let selection = activity_reading::load_activity(store)?;
    if let Some(id) = conv
        .introspect_target
        .as_ref()
        .and_then(|t| t.operation_id.as_deref())
    {
        if let Some(job) = selection.study_handoff.jobs.get(id) {
            intent(store, id, job)?;
            if job.phase.terminal() {
                conv.introspect_target = None;
                conv.wants_introspect = false;
            }
        } else {
            ensure!(
                !id.starts_with("study-job-"),
                "checkpoint study identity missing from authoritative activity"
            );
        }
    }
    conv.activity = selection.clone();
    let Some(id) = selection.study_handoff.active.as_deref() else {
        return Ok(());
    };
    let job = &selection.study_handoff.jobs[id];
    let target = intent(store, id, job)?;
    if let Some(current) = &conv.introspect_target {
        ensure!(
            current.operation_id.as_deref() == Some(id),
            "study checkpoint conflicts with authoritative queued choice"
        );
    }
    ensure!(
        conv.activity.native_focus_window.is_none(),
        "ordinary study and protected focus require explicit reconciliation"
    );
    if job.phase == Phase::Claimed {
        let (receipt, _) =
            crate::llm::recover_retained_delivery_at(deliveries, &content_id(id), 0)?.context(
                "study provider outcome uncertain; retained choice is blocked, not retried",
            )?;
        delivered(store, conv, id, reader, &receipt)?;
        conv.introspect_target = None;
        conv.wants_introspect = false;
        conv.pending_file_listing = Some(format!(
            "Study job {id} recovered from verified delivery. Its exact response and NEXT remain in the native study record; recovery did not dispatch NEXT or repeat generation."
        ));
    } else {
        conv.introspect_target = Some(target);
        conv.wants_introspect = true;
    }
    Ok(())
}

pub(super) fn reconcile_configured(conv: &mut ConversationState) -> Result<()> {
    reconcile(
        &ActionContinuityStore::for_astrid_workspace(),
        conv,
        &super::activity_focus::reader()?,
        &crate::paths::bridge_paths()
            .bridge_workspace()
            .join("diagnostics/accepted_deliveries"),
    )
}

#[cfg(test)]
#[path = "study_handoff_tests.rs"]
mod tests;
