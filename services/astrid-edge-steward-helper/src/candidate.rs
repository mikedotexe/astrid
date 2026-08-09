use std::collections::BTreeMap;
use std::fs::{self, File};
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use serde_json::Value;
use uuid::Uuid;

use crate::attestation::{CANDIDATE_SCHEMA, HmacSigner, SupervisorCandidate};
use crate::candidate_ledger::{CandidateEditLedger, EventContext, PreparedRecord};
use crate::source::{SourceSnapshot, repository_path};
use crate::util::{
    atomic_private_write, canonical_json, read_stable_regular, sha256, unix_seconds,
    validate_hex64, validate_identifier,
};
use crate::{Error, Result};

const LEGACY_DRAFT_SCHEMA: &str = "astrid.edge.steward_helper.candidate_draft.v1";
const DRAFT_SCHEMA: &str = "astrid.edge.steward_helper.candidate_draft.v2";
const DRAFT_AUTHORING_SCHEMA: &str = "astrid.edge.steward_helper.candidate_authoring_provenance.v2";
const CLEAN_AUTHORING_AUTHORITY: &str = "clean_context_chain_untrusted_external_content_forbidden";
const LEGACY_AUTHORING_AUTHORITY: &str = "legacy_unattributed_draft_quarantined_no_edit_or_submit";
const EDIT_TRANSACTION_SCHEMA: &str = "astrid.edge.steward_helper.candidate_edit_transaction.v1";
const PATCH_SCHEMA: &str = "astrid.edge_self_change.full_replacement_patch.v1";
const MAX_FILES: usize = 25;
const MAX_CHANGED_LINES: usize = 4_000;
const MAX_CONTENT_BYTES: usize = 512 * 1024;
const MAX_TOTAL_CONTENT_BYTES: usize = 8 * 1024 * 1024;
const MAX_LINE_EDITS: usize = 64;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Draft {
    schema: String,
    candidate_id: String,
    source_id: String,
    base_generation: String,
    title: String,
    created_at: u64,
    stage: DraftStage,
    #[serde(default, skip_serializing_if = "DraftAuthoringProvenance::is_legacy")]
    authoring_provenance: DraftAuthoringProvenance,
    replacements: BTreeMap<String, Replacement>,
    submission: Option<Submission>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct DraftAuthoringProvenance {
    schema: String,
    authority: String,
    untrusted_external_content: bool,
    context_count: u64,
    context_chain_sha256: String,
    last_context_provenance_sha256: String,
    last_due_nonce: String,
    last_trace_id: String,
    last_session_id: String,
    last_turn_id: String,
}

impl Default for DraftAuthoringProvenance {
    fn default() -> Self {
        Self {
            schema: String::new(),
            authority: LEGACY_AUTHORING_AUTHORITY.to_owned(),
            untrusted_external_content: true,
            context_count: 0,
            context_chain_sha256: String::new(),
            last_context_provenance_sha256: String::new(),
            last_due_nonce: String::new(),
            last_trace_id: String::new(),
            last_session_id: String::new(),
            last_turn_id: String::new(),
        }
    }
}

impl DraftAuthoringProvenance {
    fn is_legacy(&self) -> bool {
        self.schema.is_empty() && self.authority == LEGACY_AUTHORING_AUTHORITY
    }

    fn clean(context: &EventContext<'_>) -> Result<Self> {
        let mut value = Self {
            schema: DRAFT_AUTHORING_SCHEMA.to_owned(),
            authority: CLEAN_AUTHORING_AUTHORITY.to_owned(),
            untrusted_external_content: false,
            context_count: 0,
            context_chain_sha256: "0".repeat(64),
            last_context_provenance_sha256: context.context_provenance_sha256.to_owned(),
            last_due_nonce: context.due_nonce.to_owned(),
            last_trace_id: context.trace_id.to_owned(),
            last_session_id: context.session_id.to_owned(),
            last_turn_id: context.turn_id.to_owned(),
        };
        value.record(context)?;
        Ok(value)
    }

    fn record(&mut self, context: &EventContext<'_>) -> Result<()> {
        self.require_clean_base()?;
        for (value, label) in [
            (context.due_nonce, "candidate due nonce"),
            (context.trace_id, "candidate trace id"),
            (context.session_id, "candidate session id"),
            (context.turn_id, "candidate turn id"),
        ] {
            validate_identifier(value, label)?;
        }
        validate_hex64(
            context.context_provenance_sha256,
            "candidate context provenance hash",
        )?;
        let next_count = self
            .context_count
            .checked_add(1)
            .ok_or_else(|| Error::new("candidate authoring context count overflow"))?;
        let link = serde_json::json!({
            "schema": "astrid.edge.steward_helper.candidate_authoring_context_link.v1",
            "prior_chain_sha256": self.context_chain_sha256,
            "sequence": next_count,
            "due_nonce": context.due_nonce,
            "trace_id": context.trace_id,
            "session_id": context.session_id,
            "turn_id": context.turn_id,
            "response_sha256": context.response_sha256,
            "context_provenance_sha256": context.context_provenance_sha256
        });
        self.context_chain_sha256 = sha256(&canonical_json(&link)?);
        self.context_count = next_count;
        context
            .context_provenance_sha256
            .clone_into(&mut self.last_context_provenance_sha256);
        context.due_nonce.clone_into(&mut self.last_due_nonce);
        context.trace_id.clone_into(&mut self.last_trace_id);
        context.session_id.clone_into(&mut self.last_session_id);
        context.turn_id.clone_into(&mut self.last_turn_id);
        Ok(())
    }

    fn require_clean(&self) -> Result<()> {
        self.require_clean_base()?;
        if self.context_count == 0 {
            return Err(Error::new(
                "candidate draft has no durable authoring context",
            ));
        }
        Ok(())
    }

    fn require_clean_base(&self) -> Result<()> {
        if self.schema != DRAFT_AUTHORING_SCHEMA
            || self.authority != CLEAN_AUTHORING_AUTHORITY
            || self.untrusted_external_content
        {
            return Err(Error::new(
                "candidate draft lacks clean durable authoring provenance",
            ));
        }
        validate_hex64(
            &self.context_chain_sha256,
            "candidate authoring context chain hash",
        )?;
        validate_hex64(
            &self.last_context_provenance_sha256,
            "candidate latest context provenance hash",
        )?;
        for (value, label) in [
            (&self.last_due_nonce, "candidate latest due nonce"),
            (&self.last_trace_id, "candidate latest trace id"),
            (&self.last_session_id, "candidate latest session id"),
            (&self.last_turn_id, "candidate latest turn id"),
        ] {
            validate_identifier(value, label)?;
        }
        Ok(())
    }

    fn require_exact_terminal_context(
        &self,
        due_nonce: &str,
        trace_id: &str,
        session_id: &str,
        turn_id: &str,
        context_provenance_sha256: &str,
    ) -> Result<()> {
        self.require_clean()?;
        validate_hex64(
            context_provenance_sha256,
            "terminal context provenance hash",
        )?;
        if self.last_due_nonce != due_nonce
            || self.last_trace_id != trace_id
            || self.last_session_id != session_id
            || self.last_turn_id != turn_id
            || self.last_context_provenance_sha256 != context_provenance_sha256
        {
            return Err(Error::new(
                "candidate terminal context differs from its exact latest clean authoring context",
            ));
        }
        Ok(())
    }

    fn digest(&self) -> Result<String> {
        self.require_clean()?;
        Ok(sha256(&canonical_json(self)?))
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
enum DraftStage {
    Editing,
    Prepared,
    Submitted,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Replacement {
    source_sha256: String,
    content_sha256: String,
    content: String,
    conservative_changed_lines: usize,
    #[serde(default)]
    original_line_count: usize,
    #[serde(default)]
    new_line_count: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Submission {
    candidate_sha256: String,
    patch_sha256: String,
    proposal_sha256: String,
    manifest: SupervisorCandidate,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct DraftEnvelope {
    schema: String,
    draft: Draft,
    draft_sha256: String,
    key_id: String,
    hmac_sha256: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct PendingEditCore {
    schema: String,
    before_active_sha256: Option<String>,
    after_active_sha256: Option<String>,
    after_active: Option<DraftEnvelope>,
    ledger_record: PreparedRecord,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct PendingEditEnvelope {
    schema: String,
    core: PendingEditCore,
    core_sha256: String,
    key_id: String,
    hmac_sha256: String,
}

#[derive(Debug, Clone, Serialize)]
struct PatchBundle {
    schema: &'static str,
    candidate_id: String,
    source_id: String,
    base_generation: String,
    authoring_provenance_sha256: String,
    files: Vec<PatchFile>,
}

#[derive(Debug, Clone, Serialize)]
struct PatchFile {
    path: String,
    source_sha256: String,
    content_sha256: String,
    content: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
struct LineEdit {
    start_line: usize,
    end_line: usize,
    replacement: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct SubmittedCandidate {
    pub candidate_id: String,
    pub candidate_sha256: String,
    pub patch_sha256: String,
    pub manifest: SupervisorCandidate,
}

#[derive(Debug, Clone)]
pub enum ActiveDraft {
    Editing,
    Prepared(Box<SubmittedCandidate>),
    Submitted(Box<SubmittedCandidate>),
}

#[derive(Debug, Clone)]
pub struct TerminalArchive {
    pub candidate: SubmittedCandidate,
    pub source_id: String,
    pub patch: Value,
    pub history_root: PathBuf,
    pub touched_paths: Vec<String>,
    pub changed_lines: usize,
    pub added_lines: usize,
    pub removed_lines: usize,
}

pub struct CandidateManager<'a> {
    state_root: &'a Path,
    outbox: &'a Path,
    snapshot: Option<&'a SourceSnapshot>,
    signer: &'a HmacSigner,
    current_generation_path: &'a Path,
    current_generation: String,
}

impl<'a> CandidateManager<'a> {
    pub fn new(
        state_root: &'a Path,
        outbox: &'a Path,
        snapshot: &'a SourceSnapshot,
        signer: &'a HmacSigner,
        current_generation_path: &'a Path,
        expected_generation: &str,
    ) -> Result<Self> {
        let bytes = read_stable_regular(current_generation_path, 256)?;
        let current_generation = std::str::from_utf8(&bytes)
            .map_err(|_| Error::new("current generation is not UTF-8"))?
            .trim()
            .to_owned();
        validate_identifier(&current_generation, "current generation")?;
        if current_generation != expected_generation {
            return Err(Error::new(
                "active source and current generation binding changed",
            ));
        }
        let manager = Self {
            state_root,
            outbox,
            snapshot: Some(snapshot),
            signer,
            current_generation_path,
            current_generation,
        };
        manager.recover_pending_edit()?;
        Ok(manager)
    }

    pub fn new_reconciler(
        state_root: &'a Path,
        outbox: &'a Path,
        signer: &'a HmacSigner,
        current_generation_path: &'a Path,
    ) -> Result<Self> {
        let bytes = read_stable_regular(current_generation_path, 256)?;
        let current_generation = std::str::from_utf8(&bytes)
            .map_err(|_| Error::new("current generation is not UTF-8"))?
            .trim()
            .to_owned();
        validate_identifier(&current_generation, "current generation")?;
        let manager = Self {
            state_root,
            outbox,
            snapshot: None,
            signer,
            current_generation_path,
            current_generation,
        };
        manager.recover_pending_edit()?;
        Ok(manager)
    }

    fn snapshot(&self) -> Result<&SourceSnapshot> {
        self.snapshot.ok_or_else(|| {
            Error::new("source tools are unavailable during lifecycle reconciliation")
        })
    }

    pub fn execute(
        &self,
        name: &str,
        arguments: &Value,
        proposal_binding: &str,
        context: &EventContext<'_>,
    ) -> Result<Value> {
        let result = match name {
            "begin_candidate" => self.begin(arguments, context),
            "apply_candidate_patch" => self.apply(arguments, context),
            "inspect_candidate" => exact_keys(arguments, &[]).and_then(|()| self.inspect(context)),
            "format_candidate" => exact_keys(arguments, &[]).and_then(|()| self.format(context)),
            "abandon_candidate" => exact_keys(arguments, &[]).and_then(|()| self.abandon(context)),
            "submit_candidate" => self.submit(arguments, proposal_binding, context),
            _ => Err(Error::new("unsupported candidate tool")),
        };
        if let Err(error) = &result {
            if self.pending_edit_path().exists() {
                self.recover_pending_edit()?;
            }
            self.record_rejection(name, error, context)?;
        }
        result
    }

    pub fn submitted(&self) -> Result<Option<SubmittedCandidate>> {
        let Some(draft) = self.load()? else {
            return Ok(None);
        };
        if draft.authoring_provenance.is_legacy() {
            return Ok(None);
        }
        draft.authoring_provenance.require_clean()?;
        let Some(submission) = draft.submission else {
            return Ok(None);
        };
        Ok(Some(SubmittedCandidate {
            candidate_id: draft.candidate_id,
            candidate_sha256: submission.candidate_sha256,
            patch_sha256: submission.patch_sha256,
            manifest: submission.manifest,
        }))
    }

    /// Return a prepared candidate only when its final declaration belongs to
    /// the exact same clean scheduled model transaction that prepared it.
    #[allow(clippy::too_many_arguments)]
    pub fn submitted_for_terminal(
        &self,
        due_nonce: &str,
        trace_id: &str,
        session_id: &str,
        turn_id: &str,
        context_provenance_sha256: &str,
        proposal_binding: &str,
    ) -> Result<Option<SubmittedCandidate>> {
        validate_hex64(proposal_binding, "terminal proposal binding")?;
        let Some(draft) = self.load()? else {
            return Ok(None);
        };
        if draft.stage != DraftStage::Prepared {
            return Err(Error::new(
                "candidate terminal declaration requires an exact prepared draft",
            ));
        }
        draft.authoring_provenance.require_exact_terminal_context(
            due_nonce,
            trace_id,
            session_id,
            turn_id,
            context_provenance_sha256,
        )?;
        let submission = draft
            .submission
            .ok_or_else(|| Error::new("prepared candidate lacks submission state"))?;
        if submission.proposal_sha256 != proposal_binding {
            return Err(Error::new(
                "candidate terminal proposal belongs to another scheduled model transaction",
            ));
        }
        Ok(Some(SubmittedCandidate {
            candidate_id: draft.candidate_id,
            candidate_sha256: submission.candidate_sha256,
            patch_sha256: submission.patch_sha256,
            manifest: submission.manifest,
        }))
    }

    /// A `Prepared` draft without a retained authenticated authored transaction
    /// is only an interrupted tool operation, never deployment authority.
    pub fn reconcile_orphan_prepared(&self) -> Result<bool> {
        let Some(draft) = self.load()? else {
            return Ok(false);
        };
        if draft.stage != DraftStage::Prepared {
            return Ok(false);
        }
        let proposal_binding = draft
            .submission
            .as_ref()
            .ok_or_else(|| Error::new("prepared candidate lacks submission state"))?
            .proposal_sha256
            .clone();
        self.reopen_unattested(&proposal_binding)
    }

    pub fn reopen_unattested(&self, proposal_binding: &str) -> Result<bool> {
        validate_hex64(proposal_binding, "proposal binding")?;
        let Some(mut draft) = self.load()? else {
            return Ok(false);
        };
        let Some(submission) = draft.submission.as_ref() else {
            return Ok(false);
        };
        if draft.stage != DraftStage::Prepared || submission.proposal_sha256 != proposal_binding {
            return Ok(false);
        }
        draft.stage = DraftStage::Editing;
        draft.submission = None;
        self.save(&draft)?;
        Ok(true)
    }

    pub fn active(&self) -> Result<Option<ActiveDraft>> {
        let Some(draft) = self.load()? else {
            return Ok(None);
        };
        if draft.authoring_provenance.is_legacy() {
            return Ok(Some(ActiveDraft::Editing));
        }
        draft.authoring_provenance.require_clean()?;
        let Some(submission) = draft.submission else {
            return Ok(Some(ActiveDraft::Editing));
        };
        let candidate = Box::new(SubmittedCandidate {
            candidate_id: draft.candidate_id,
            candidate_sha256: submission.candidate_sha256,
            patch_sha256: submission.patch_sha256,
            manifest: submission.manifest,
        });
        match draft.stage {
            DraftStage::Prepared => Ok(Some(ActiveDraft::Prepared(candidate))),
            DraftStage::Submitted => Ok(Some(ActiveDraft::Submitted(candidate))),
            DraftStage::Editing => Err(Error::new(
                "editing candidate unexpectedly contains submission metadata",
            )),
        }
    }

    /// Return a bounded body-free summary for scheduled reflection context.
    pub fn prompt_status(&self) -> Result<Value> {
        Ok(prompt_status_for_draft(self.load()?.as_ref()))
    }

    /// Return the exact digest of the authenticated prepared draft envelope.
    pub fn prepared_draft_sha256(&self, candidate: &SubmittedCandidate) -> Result<String> {
        let draft = self
            .load()?
            .ok_or_else(|| Error::new("no candidate draft exists"))?;
        require_candidate_stage(&draft, candidate, DraftStage::Prepared)?;
        Ok(sha256(&read_stable_regular(
            &self.draft_path(),
            16 * 1024 * 1024,
        )?))
    }

    /// Publish the inert patch artifact idempotently. The patch alone grants no authority.
    pub fn publish_patch(&self, candidate: &SubmittedCandidate) -> Result<()> {
        let draft = self
            .load()?
            .ok_or_else(|| Error::new("no candidate draft exists"))?;
        if draft.stage != DraftStage::Prepared && draft.stage != DraftStage::Submitted {
            return Err(Error::new(
                "candidate patch is not prepared for publication",
            ));
        }
        require_candidate_identity(&draft, candidate)?;
        let patch_bytes = canonical_json(&patch_bundle(&draft)?)?;
        if sha256(&patch_bytes) != candidate.patch_sha256 {
            return Err(Error::new(
                "candidate patch digest changed before publication",
            ));
        }
        let patch_path = self
            .outbox
            .join(format!("candidate-patch-{}.json", candidate.patch_sha256));
        write_exact_or_verify(&patch_path, &patch_bytes)
    }

    /// Commit a prepared draft only after its exact signed intent is durably published.
    pub fn mark_submitted(
        &self,
        candidate: &SubmittedCandidate,
        context: &EventContext<'_>,
    ) -> Result<()> {
        let mut draft = self
            .load()?
            .ok_or_else(|| Error::new("no candidate draft exists"))?;
        if draft.stage == DraftStage::Submitted {
            require_candidate_identity(&draft, candidate)?;
        } else {
            require_candidate_stage(&draft, candidate, DraftStage::Prepared)?;
            self.publish_patch(candidate)?;
            draft.stage = DraftStage::Submitted;
            self.save(&draft)?;
        }
        let active_sha256 = self
            .active_envelope_bytes()?
            .as_deref()
            .map(sha256)
            .ok_or_else(|| Error::new("submitted candidate draft disappeared"))?;
        let record = CandidateEditLedger::new(self.state_root, self.signer).prepare(
            "submit_candidate_attested",
            "completed",
            Some(&draft.candidate_id),
            Some(&draft.source_id),
            Some(&draft.base_generation),
            Some(&active_sha256),
            Some(&active_sha256),
            serde_json::json!({
                "candidate_sha256": candidate.candidate_sha256,
                "patch_sha256": candidate.patch_sha256,
                "files": draft.replacements.len(),
                "changed_lines": total_lines(&draft)
            }),
            context,
        )?;
        CandidateEditLedger::new(self.state_root, self.signer).append(&record)
    }

    pub fn archive_terminal(
        &self,
        terminal_status: &str,
        terminal_reason_sha256: Option<&str>,
    ) -> Result<TerminalArchive> {
        let draft_bytes = read_stable_regular(&self.draft_path(), 16 * 1024 * 1024)?;
        let draft = self
            .load()?
            .ok_or_else(|| Error::new("no candidate draft exists"))?;
        let submission = draft
            .submission
            .clone()
            .ok_or_else(|| Error::new("editing draft cannot be terminally reconciled"))?;
        if !matches!(
            terminal_status,
            "accepted" | "rejected" | "rolled_back" | "abandoned"
        ) {
            return Err(Error::new("candidate lifecycle status is not terminal"));
        }
        if let Some(reason) = terminal_reason_sha256 {
            validate_hex64(reason, "terminal_reason_sha256")?;
        }
        let patch_path = self
            .outbox
            .join(format!("candidate-patch-{}.json", submission.patch_sha256));
        let patch_bytes = read_stable_regular(&patch_path, 16 * 1024 * 1024)?;
        if sha256(&patch_bytes) != submission.patch_sha256 {
            return Err(Error::new("submitted candidate patch digest changed"));
        }
        let patch: Value = serde_json::from_slice(&patch_bytes)?;
        let touched_paths = draft
            .replacements
            .keys()
            .map(|source_id| repository_path(source_id))
            .collect::<Result<Vec<_>>>()?;
        let changed_lines = total_lines(&draft);
        let added_lines = draft
            .replacements
            .values()
            .map(|replacement| {
                replacement
                    .new_line_count
                    .saturating_sub(replacement.original_line_count)
            })
            .fold(0_usize, usize::saturating_add);
        let removed_lines = draft
            .replacements
            .values()
            .map(|replacement| {
                replacement
                    .original_line_count
                    .saturating_sub(replacement.new_line_count)
            })
            .fold(0_usize, usize::saturating_add);
        let history_root = self.outbox.join("history").join(format!(
            "{}-{}",
            draft.candidate_id, submission.candidate_sha256
        ));
        crate::util::ensure_private_dir(&history_root)?;
        write_exact_or_verify(&history_root.join("signed-draft.json"), &draft_bytes)?;
        write_exact_or_verify(&history_root.join("candidate-patch.json"), &patch_bytes)?;
        let terminal = serde_json::json!({
            "schema": "astrid.edge.steward_helper.candidate_terminal_archive.v1",
            "candidate_id": draft.candidate_id,
            "candidate_sha256": submission.candidate_sha256,
            "patch_sha256": submission.patch_sha256,
            "source_id": draft.source_id,
            "base_generation": draft.base_generation,
            "terminal_status": terminal_status,
            "terminal_reason_sha256": terminal_reason_sha256,
            "recorded_at": unix_seconds()
        });
        let terminal_bytes = canonical_json(&terminal)?;
        let envelope = serde_json::json!({
            "schema": "astrid.edge.steward_helper.candidate_terminal_archive_envelope.v1",
            "core": terminal,
            "core_sha256": sha256(&terminal_bytes),
            "auth": {
                "algorithm": "hmac-sha256",
                "key_id": self.signer.key_id,
                "signature": self.signer.sign(&terminal_bytes)
            }
        });
        write_exact_or_verify(
            &history_root.join("terminal-receipt.json"),
            &canonical_json(&envelope)?,
        )?;
        Ok(TerminalArchive {
            candidate: SubmittedCandidate {
                candidate_id: draft.candidate_id,
                candidate_sha256: submission.candidate_sha256,
                patch_sha256: submission.patch_sha256,
                manifest: submission.manifest,
            },
            source_id: draft.source_id,
            patch,
            history_root,
            touched_paths,
            changed_lines,
            added_lines,
            removed_lines,
        })
    }

    pub fn clear_terminal(&self, candidate_id: &str, candidate_sha256: &str) -> Result<()> {
        let active = self
            .submitted()?
            .ok_or_else(|| Error::new("submitted candidate disappeared before reconciliation"))?;
        if active.candidate_id != candidate_id || active.candidate_sha256 != candidate_sha256 {
            return Err(Error::new(
                "submitted candidate changed before terminal reconciliation",
            ));
        }
        fs::remove_file(self.draft_path())?;
        File::open(self.state_root)?.sync_all()?;
        Ok(())
    }

    fn begin(&self, arguments: &Value, context: &EventContext<'_>) -> Result<Value> {
        exact_keys(arguments, &["title"])?;
        if self.load()?.is_some() {
            return Err(Error::new("one candidate draft is already active"));
        }
        let title = argument_string(arguments, "title", 1, 160)?;
        validate_plain_text(&title, "candidate title")?;
        let candidate_id = format!("candidate-{}", Uuid::new_v4().simple());
        let draft = Draft {
            schema: DRAFT_SCHEMA.to_owned(),
            candidate_id: candidate_id.clone(),
            source_id: self.snapshot()?.source_id.clone(),
            base_generation: self.current_generation.clone(),
            title,
            created_at: unix_seconds(),
            stage: DraftStage::Editing,
            authoring_provenance: DraftAuthoringProvenance::clean(context)?,
            replacements: BTreeMap::new(),
            submission: None,
        };
        self.commit_tool_edit(
            "begin_candidate",
            None,
            Some(&draft),
            context,
            serde_json::json!({
                "files": 0,
                "changed_lines": 0
            }),
        )?;
        Ok(serde_json::json!({
            "status": "editing",
            "candidate_id": candidate_id,
            "source_id": self.snapshot()?.source_id,
            "base_generation": self.current_generation,
            "limits": {"files": MAX_FILES, "changed_lines": MAX_CHANGED_LINES}
        }))
    }

    fn apply(&self, arguments: &Value, event_context: &EventContext<'_>) -> Result<Value> {
        let edit_mode = if arguments.get("content").is_some() {
            exact_keys(arguments, &["source_id", "expected_sha256", "content"])?;
            "full_content"
        } else {
            exact_keys(arguments, &["source_id", "expected_sha256", "edits"])?;
            "line_hunks"
        };
        let source_id = argument_string(arguments, "source_id", 1, 512)?;
        let expected = argument_string(arguments, "expected_sha256", 64, 64)?;
        validate_hex64(&expected, "expected_sha256")?;
        let snapshot = self.snapshot()?;
        let entry = snapshot.mutable_entry(&source_id)?;
        let original = snapshot.full_text(&entry)?;
        let before = self.active_envelope_bytes()?;
        let mut draft = self.require_current_editing()?;
        draft.authoring_provenance.record(event_context)?;
        let current_hash = draft
            .replacements
            .get(&source_id)
            .map_or(entry.sha256.as_str(), |replacement| {
                replacement.content_sha256.as_str()
            });
        if expected != current_hash {
            return Err(Error::new("stale candidate file hash"));
        }
        let current = draft.replacements.get(&source_id).map_or_else(
            || original.clone(),
            |replacement| replacement.content.clone(),
        );
        let (content, edit_count) = if edit_mode == "full_content" {
            (
                argument_string(arguments, "content", 0, MAX_CONTENT_BYTES)?,
                0,
            )
        } else {
            let edits = parse_line_edits(arguments)?;
            let count = edits.len();
            (apply_line_edits(&current, &edits)?, count)
        };
        validate_candidate_content(&content)?;
        if content == current {
            return Err(Error::new("candidate edit made no change"));
        }
        if content.len() > MAX_CONTENT_BYTES {
            return Err(Error::new("candidate replacement exceeds its byte bound"));
        }
        let content_sha256 = sha256(content.as_bytes());
        if content_sha256 == entry.sha256 {
            draft.replacements.remove(&source_id);
        } else {
            let changed = bounded_changed_lines(&original, &content, MAX_CHANGED_LINES);
            let original_line_count = original.lines().count();
            let new_line_count = content.lines().count();
            draft.replacements.insert(
                source_id.clone(),
                Replacement {
                    source_sha256: entry.sha256,
                    content_sha256: content_sha256.clone(),
                    content,
                    conservative_changed_lines: changed,
                    original_line_count,
                    new_line_count,
                },
            );
        }
        validate_limits(&draft)?;
        self.commit_tool_edit(
            "apply_candidate_patch",
            before.as_deref(),
            Some(&draft),
            event_context,
            serde_json::json!({
                "edit_mode": edit_mode,
                "edit_count": edit_count,
                "target_source_id_sha256": sha256(source_id.as_bytes()),
                "source_sha256": expected,
                "content_sha256": content_sha256,
                "files": draft.replacements.len(),
                "changed_lines": total_lines(&draft)
            }),
        )?;
        Ok(serde_json::json!({
            "status": "applied",
            "edit_mode": edit_mode,
            "edit_count": edit_count,
            "source_id": source_id,
            "content_sha256": content_sha256,
            "files": draft.replacements.len(),
            "changed_lines": total_lines(&draft)
        }))
    }

    fn inspect(&self, context: &EventContext<'_>) -> Result<Value> {
        let draft = self
            .load()?
            .ok_or_else(|| Error::new("no candidate draft exists"))?;
        let value = serde_json::json!({
            "candidate_id": draft.candidate_id,
            "source_id": draft.source_id,
            "base_generation": draft.base_generation,
            "stage": draft.stage,
            "files": draft.replacements.iter().map(|(path, replacement)| serde_json::json!({
                "source_id": path,
                "content_sha256": replacement.content_sha256,
                "changed_lines": replacement.conservative_changed_lines
            })).collect::<Vec<_>>(),
            "changed_lines": total_lines(&draft),
            "submission": draft.submission.as_ref().map(|value| &value.candidate_sha256)
        });
        let draft_sha256 = self
            .active_envelope_bytes()?
            .as_deref()
            .map(sha256)
            .ok_or_else(|| Error::new("candidate draft disappeared during inspection"))?;
        let record = CandidateEditLedger::new(self.state_root, self.signer).prepare(
            "inspect_candidate",
            "completed",
            Some(&draft.candidate_id),
            Some(&draft.source_id),
            Some(&draft.base_generation),
            Some(&draft_sha256),
            Some(&draft_sha256),
            serde_json::json!({
                "files": draft.replacements.len(),
                "changed_lines": total_lines(&draft),
                "stage": draft.stage
            }),
            context,
        )?;
        CandidateEditLedger::new(self.state_root, self.signer).append(&record)?;
        Ok(value)
    }

    fn format(&self, context: &EventContext<'_>) -> Result<Value> {
        let before = self.active_envelope_bytes()?;
        let mut draft = self.require_current_editing()?;
        draft.authoring_provenance.record(context)?;
        let mut unchanged = Vec::new();
        for (source_id, replacement) in &mut draft.replacements {
            let formatted = replacement
                .content
                .lines()
                .map(str::trim_end)
                .collect::<Vec<_>>()
                .join("\n")
                + "\n";
            replacement.content_sha256 = sha256(formatted.as_bytes());
            let snapshot = self.snapshot()?;
            let entry = snapshot.mutable_entry(source_id)?;
            let original = snapshot.full_text(&entry)?;
            replacement.conservative_changed_lines =
                bounded_changed_lines(&original, &formatted, MAX_CHANGED_LINES);
            replacement.original_line_count = original.lines().count();
            replacement.new_line_count = formatted.lines().count();
            replacement.content = formatted;
            if replacement.content_sha256 == entry.sha256 {
                unchanged.push(source_id.clone());
            }
        }
        for source_id in unchanged {
            draft.replacements.remove(&source_id);
        }
        validate_limits(&draft)?;
        let replacement_contents = draft
            .replacements
            .iter()
            .map(|(source_id, replacement)| (source_id.clone(), replacement.content.clone()))
            .collect::<BTreeMap<_, _>>();
        self.snapshot()?
            .validate_dependency_changes(&replacement_contents)?;
        self.commit_tool_edit(
            "format_candidate",
            before.as_deref(),
            Some(&draft),
            context,
            serde_json::json!({
                "files": draft.replacements.len(),
                "changed_lines": total_lines(&draft)
            }),
        )?;
        Ok(serde_json::json!({
            "status": "deterministic_whitespace_format_complete",
            "files": draft.replacements.len(),
            "changed_lines": total_lines(&draft),
            "note": "immutable build gates still run canonical language formatters"
        }))
    }

    fn abandon(&self, context: &EventContext<'_>) -> Result<Value> {
        let Some(draft) = self.load()? else {
            let record = CandidateEditLedger::new(self.state_root, self.signer).prepare(
                "abandon_candidate",
                "completed",
                None,
                None,
                None,
                None,
                None,
                serde_json::json!({"no_active_candidate": true}),
                context,
            )?;
            CandidateEditLedger::new(self.state_root, self.signer).append(&record)?;
            return Ok(serde_json::json!({"status": "no_active_candidate"}));
        };
        let before = self.active_envelope_bytes()?;
        self.commit_tool_edit(
            "abandon_candidate",
            before.as_deref(),
            None,
            context,
            serde_json::json!({
                "files": draft.replacements.len(),
                "changed_lines": total_lines(&draft),
                "prior_stage": draft.stage
            }),
        )?;
        Ok(serde_json::json!({
            "status": "abandoned",
            "candidate_id": draft.candidate_id
        }))
    }

    fn submit(
        &self,
        arguments: &Value,
        proposal_binding: &str,
        context: &EventContext<'_>,
    ) -> Result<Value> {
        exact_keys(arguments, &["reason"])?;
        let reason = argument_string(arguments, "reason", 1, 240)?;
        validate_plain_text(&reason, "candidate submission reason")?;
        validate_hex64(proposal_binding, "proposal binding")?;
        let before = self.active_envelope_bytes()?;
        let mut draft = self.require_current_editing()?;
        draft.authoring_provenance.record(context)?;
        if draft.replacements.is_empty() {
            return Err(Error::new("cannot submit an empty candidate"));
        }
        validate_limits(&draft)?;
        self.validate_replacements_against_snapshot(&draft)?;
        let patch = patch_bundle(&draft)?;
        let patch_bytes = canonical_json(&patch)?;
        let patch_sha256 = sha256(&patch_bytes);
        let changed_paths = patch
            .files
            .iter()
            .map(|file| file.path.clone())
            .collect::<Vec<_>>();
        let manifest = SupervisorCandidate {
            schema: CANDIDATE_SCHEMA.to_owned(),
            candidate_id: draft.candidate_id.clone(),
            base_generation: draft.base_generation.clone(),
            proposal_sha256: proposal_binding.to_owned(),
            patch_sha256: patch_sha256.clone(),
            changed_paths,
            created_at: unix_seconds(),
            privilege_envelope: "proposal-only:no-execution:v1".to_owned(),
        };
        let candidate_sha256 = sha256(&canonical_json(&manifest)?);
        draft.stage = DraftStage::Prepared;
        draft.submission = Some(Submission {
            candidate_sha256: candidate_sha256.clone(),
            patch_sha256: patch_sha256.clone(),
            proposal_sha256: proposal_binding.to_owned(),
            manifest,
        });
        self.commit_tool_edit(
            "submit_candidate",
            before.as_deref(),
            Some(&draft),
            context,
            serde_json::json!({
                "candidate_sha256": candidate_sha256,
                "patch_sha256": patch_sha256,
                "reason_sha256": sha256(reason.as_bytes()),
                "files": draft.replacements.len(),
                "changed_lines": total_lines(&draft)
            }),
        )?;
        Ok(serde_json::json!({
            "status": "submitted_for_exact_terminal_declaration",
            "candidate_id": draft.candidate_id,
            "candidate_sha256": candidate_sha256,
            "patch_sha256": patch_sha256,
            "required_final_line": format!("CHANGESET: SUBMIT {} {} :: <reason>", draft.candidate_id, candidate_sha256)
        }))
    }

    fn require_editing(&self) -> Result<Draft> {
        let draft = self
            .load()?
            .ok_or_else(|| Error::new("no candidate draft exists"))?;
        if draft.stage != DraftStage::Editing {
            return Err(Error::new("candidate is frozen after submit"));
        }
        Ok(draft)
    }

    fn require_current_editing(&self) -> Result<Draft> {
        let draft = self.require_editing()?;
        draft.authoring_provenance.require_clean()?;
        let live_generation = read_generation(self.current_generation_path)?;
        let snapshot = self.snapshot()?;
        if live_generation != self.current_generation
            || draft.source_id != snapshot.source_id
            || draft.base_generation != self.current_generation
        {
            return Err(Error::new("candidate draft source or generation is stale"));
        }
        self.validate_replacements_against_snapshot(&draft)?;
        Ok(draft)
    }

    fn validate_replacements_against_snapshot(&self, draft: &Draft) -> Result<()> {
        let snapshot = self.snapshot()?;
        let mut contents = BTreeMap::new();
        for (source_id, replacement) in &draft.replacements {
            let entry = snapshot.mutable_entry(source_id)?;
            let original = snapshot.full_text(&entry)?;
            validate_candidate_content(&replacement.content)?;
            if replacement.source_sha256 != entry.sha256
                || replacement.content_sha256 != sha256(replacement.content.as_bytes())
                || replacement.original_line_count != original.lines().count()
                || replacement.new_line_count != replacement.content.lines().count()
                || replacement.conservative_changed_lines
                    != bounded_changed_lines(&original, &replacement.content, MAX_CHANGED_LINES)
                || replacement.content_sha256 == entry.sha256
            {
                return Err(Error::new(
                    "candidate replacement no longer binds exact signed source",
                ));
            }
            contents.insert(source_id.clone(), replacement.content.clone());
        }
        snapshot.validate_dependency_changes(&contents)
    }

    fn draft_path(&self) -> PathBuf {
        self.state_root.join("active-candidate.json")
    }

    fn load(&self) -> Result<Option<Draft>> {
        let path = self.draft_path();
        if !path.exists() {
            if path.is_symlink() {
                return Err(Error::new("candidate draft is a broken symlink"));
            }
            return Ok(None);
        }
        let bytes = read_stable_regular(&path, 16 * 1024 * 1024)?;
        let envelope: DraftEnvelope = serde_json::from_slice(&bytes)?;
        let draft_bytes = canonical_json(&envelope.draft)?;
        if envelope.schema != DRAFT_SCHEMA
            || envelope.draft.schema != DRAFT_SCHEMA
            || envelope.draft_sha256 != sha256(&draft_bytes)
            || envelope.key_id != self.signer.key_id
            || !self.signer.verify(&draft_bytes, &envelope.hmac_sha256)
        {
            return Err(Error::new("candidate draft authentication failed"));
        }
        validate_draft(&envelope.draft)?;
        if envelope.draft.stage == DraftStage::Submitted {
            self.validate_submitted_artifacts(&envelope.draft)?;
        }
        Ok(Some(envelope.draft))
    }

    fn validate_submitted_artifacts(&self, draft: &Draft) -> Result<()> {
        let submission = draft
            .submission
            .as_ref()
            .ok_or_else(|| Error::new("submitted candidate lacks submission state"))?;
        let patch = patch_bundle(draft)?;
        let patch_bytes = canonical_json(&patch)?;
        if sha256(&patch_bytes) != submission.patch_sha256 {
            return Err(Error::new("submitted candidate patch binding failed"));
        }
        let patch_path = self
            .outbox
            .join(format!("candidate-patch-{}.json", submission.patch_sha256));
        let stored = read_stable_regular(&patch_path, 16 * 1024 * 1024)?;
        if stored != patch_bytes {
            return Err(Error::new("submitted candidate patch artifact changed"));
        }
        Ok(())
    }

    fn commit_tool_edit(
        &self,
        operation: &str,
        before_bytes: Option<&[u8]>,
        after: Option<&Draft>,
        context: &EventContext<'_>,
        metadata: Value,
    ) -> Result<()> {
        if self.pending_edit_path().exists() || self.pending_edit_path().is_symlink() {
            return Err(Error::new(
                "candidate edit transaction was not reconciled before a new operation",
            ));
        }
        let before_sha256 = before_bytes.map(sha256);
        let (after_envelope, after_bytes) = after
            .map(|draft| signed_draft_envelope(draft, self.signer))
            .transpose()?
            .map_or((None, None), |(envelope, bytes)| {
                (Some(envelope), Some(bytes))
            });
        let after_sha256 = after_bytes.as_deref().map(sha256);
        let decoded_before = before_bytes
            .map(|bytes| decode_draft_envelope(bytes, self.signer))
            .transpose()?;
        let identity = after.or(decoded_before.as_ref());
        // Keep owned identity strings alive independently of the temporary decode above.
        let candidate_id = identity.map(|draft| draft.candidate_id.clone());
        let source_id = identity.map(|draft| draft.source_id.clone());
        let base_generation = identity.map(|draft| draft.base_generation.clone());
        let ledger = CandidateEditLedger::new(self.state_root, self.signer);
        let record = ledger.prepare(
            operation,
            "completed",
            candidate_id.as_deref(),
            source_id.as_deref(),
            base_generation.as_deref(),
            before_sha256.as_deref(),
            after_sha256.as_deref(),
            metadata,
            context,
        )?;
        let core = PendingEditCore {
            schema: EDIT_TRANSACTION_SCHEMA.to_owned(),
            before_active_sha256: before_sha256,
            after_active_sha256: after_sha256,
            after_active: after_envelope,
            ledger_record: record,
        };
        let core_bytes = canonical_json(&core)?;
        let pending = PendingEditEnvelope {
            schema: EDIT_TRANSACTION_SCHEMA.to_owned(),
            core,
            core_sha256: sha256(&core_bytes),
            key_id: self.signer.key_id.clone(),
            hmac_sha256: self.signer.sign(&core_bytes),
        };
        atomic_private_write(&self.pending_edit_path(), &canonical_json(&pending)?)?;
        self.finish_pending_edit(&pending)
    }

    fn recover_pending_edit(&self) -> Result<()> {
        let path = self.pending_edit_path();
        if !path.exists() {
            if path.is_symlink() {
                return Err(Error::new("candidate edit transaction is a broken symlink"));
            }
            return Ok(());
        }
        let bytes = read_stable_regular(&path, 20 * 1024 * 1024)?;
        let pending: PendingEditEnvelope = serde_json::from_slice(&bytes)?;
        let core_bytes = canonical_json(&pending.core)?;
        if pending.schema != EDIT_TRANSACTION_SCHEMA
            || pending.core.schema != EDIT_TRANSACTION_SCHEMA
            || pending.core_sha256 != sha256(&core_bytes)
            || pending.key_id != self.signer.key_id
            || !self.signer.verify(&core_bytes, &pending.hmac_sha256)
        {
            return Err(Error::new(
                "candidate edit transaction authentication failed",
            ));
        }
        let after_bytes = pending
            .core
            .after_active
            .as_ref()
            .map(canonical_json)
            .transpose()?;
        if after_bytes.as_deref().map(sha256) != pending.core.after_active_sha256 {
            return Err(Error::new(
                "candidate edit transaction next-state hash failed",
            ));
        }
        self.finish_pending_edit(&pending)
    }

    fn finish_pending_edit(&self, pending: &PendingEditEnvelope) -> Result<()> {
        let current = self.active_envelope_bytes()?;
        let current_sha256 = current.as_deref().map(sha256);
        if current_sha256 == pending.core.before_active_sha256 {
            if let Some(after) = &pending.core.after_active {
                let bytes = canonical_json(after)?;
                atomic_private_write(&self.draft_path(), &bytes)?;
            } else {
                let path = self.draft_path();
                if path.exists() {
                    fs::remove_file(path)?;
                    File::open(self.state_root)?.sync_all()?;
                } else if path.is_symlink() {
                    return Err(Error::new("candidate draft became a broken symlink"));
                }
            }
        } else if current_sha256 != pending.core.after_active_sha256 {
            return Err(Error::new(
                "candidate active state differs from both sides of pending edit",
            ));
        }
        CandidateEditLedger::new(self.state_root, self.signer)
            .append(&pending.core.ledger_record)?;
        fs::remove_file(self.pending_edit_path())?;
        File::open(self.state_root)?.sync_all()?;
        Ok(())
    }

    fn record_rejection(
        &self,
        requested_operation: &str,
        error: &Error,
        context: &EventContext<'_>,
    ) -> Result<()> {
        let draft = self.load().ok().flatten();
        let active_sha256 = self.active_envelope_bytes()?.as_deref().map(sha256);
        let error_text = error.to_string();
        let error_class = if error_text.contains("stale") {
            "stale_binding"
        } else if error_text.contains("limit") || error_text.contains("exceed") {
            "bounded_policy_rejection"
        } else if error_text.contains("unsupported") || error_text.contains("unadvertised") {
            "authority_rejection"
        } else {
            "validation_rejection"
        };
        let record = CandidateEditLedger::new(self.state_root, self.signer).prepare(
            "candidate_operation_rejected",
            "rejected",
            draft.as_ref().map(|value| value.candidate_id.as_str()),
            draft.as_ref().map(|value| value.source_id.as_str()),
            draft.as_ref().map(|value| value.base_generation.as_str()),
            active_sha256.as_deref(),
            active_sha256.as_deref(),
            serde_json::json!({
                "requested_operation": requested_operation,
                "error_class": error_class,
                "error_sha256": sha256(error_text.as_bytes())
            }),
            context,
        )?;
        CandidateEditLedger::new(self.state_root, self.signer).append(&record)
    }

    fn active_envelope_bytes(&self) -> Result<Option<Vec<u8>>> {
        let path = self.draft_path();
        if !path.exists() {
            if path.is_symlink() {
                return Err(Error::new("candidate draft is a broken symlink"));
            }
            return Ok(None);
        }
        Ok(Some(read_stable_regular(&path, 16 * 1024 * 1024)?))
    }

    fn pending_edit_path(&self) -> PathBuf {
        self.state_root.join("candidate-edit-pending.json")
    }

    fn save(&self, draft: &Draft) -> Result<()> {
        let (_, bytes) = signed_draft_envelope(draft, self.signer)?;
        atomic_private_write(&self.draft_path(), &bytes)
    }
}

fn signed_draft_envelope(draft: &Draft, signer: &HmacSigner) -> Result<(DraftEnvelope, Vec<u8>)> {
    validate_draft(draft)?;
    let draft_bytes = canonical_json(draft)?;
    let envelope = DraftEnvelope {
        schema: DRAFT_SCHEMA.to_owned(),
        draft: draft.clone(),
        draft_sha256: sha256(&draft_bytes),
        key_id: signer.key_id.clone(),
        hmac_sha256: signer.sign(&draft_bytes),
    };
    Ok((envelope.clone(), canonical_json(&envelope)?))
}

fn decode_draft_envelope(bytes: &[u8], signer: &HmacSigner) -> Result<Draft> {
    let envelope: DraftEnvelope = serde_json::from_slice(bytes)?;
    let draft_bytes = canonical_json(&envelope.draft)?;
    let schema_pair_valid = (envelope.schema == DRAFT_SCHEMA
        && envelope.draft.schema == DRAFT_SCHEMA)
        || (envelope.schema == LEGACY_DRAFT_SCHEMA && envelope.draft.schema == LEGACY_DRAFT_SCHEMA);
    if !schema_pair_valid
        || envelope.draft_sha256 != sha256(&draft_bytes)
        || envelope.key_id != signer.key_id
        || !signer.verify(&draft_bytes, &envelope.hmac_sha256)
    {
        return Err(Error::new("candidate draft authentication failed"));
    }
    validate_draft(&envelope.draft)?;
    Ok(envelope.draft)
}

fn write_exact_or_verify(path: &Path, bytes: &[u8]) -> Result<()> {
    if path.exists() || path.is_symlink() {
        if read_stable_regular(path, 16 * 1024 * 1024)? != bytes {
            return Err(Error::new("candidate history collision"));
        }
        return Ok(());
    }
    atomic_private_write(path, bytes)
}

fn patch_bundle(draft: &Draft) -> Result<PatchBundle> {
    let authoring_provenance_sha256 = draft.authoring_provenance.digest()?;
    let files = draft
        .replacements
        .iter()
        .map(|(source_id, replacement)| {
            Ok(PatchFile {
                path: repository_path(source_id)?,
                source_sha256: replacement.source_sha256.clone(),
                content_sha256: replacement.content_sha256.clone(),
                content: replacement.content.clone(),
            })
        })
        .collect::<Result<Vec<_>>>()?;
    Ok(PatchBundle {
        schema: PATCH_SCHEMA,
        candidate_id: draft.candidate_id.clone(),
        source_id: draft.source_id.clone(),
        base_generation: draft.base_generation.clone(),
        authoring_provenance_sha256,
        files,
    })
}

fn prompt_status_for_draft(draft: Option<&Draft>) -> Value {
    let Some(draft) = draft else {
        return serde_json::json!({"stage": "none"});
    };
    serde_json::json!({
        "stage": draft.stage,
        "candidate_id": draft.candidate_id,
        "source_id": draft.source_id,
        "base_generation": draft.base_generation,
        "files": draft.replacements.len(),
        "changed_lines": total_lines(draft),
        "candidate_sha256": draft.submission.as_ref().map(|submission| &submission.candidate_sha256),
        "authoring_provenance": if draft.authoring_provenance.is_legacy() {
            "legacy_unattributed_quarantined"
        } else {
            "clean_context_chain"
        },
        "authoring_context_chain_sha256": (!draft.authoring_provenance.is_legacy())
            .then_some(&draft.authoring_provenance.context_chain_sha256)
    })
}

fn validate_draft(draft: &Draft) -> Result<()> {
    let schema_valid = (draft.schema == DRAFT_SCHEMA && !draft.authoring_provenance.is_legacy())
        || (draft.schema == LEGACY_DRAFT_SCHEMA && draft.authoring_provenance.is_legacy());
    if !schema_valid
        || !draft.candidate_id.starts_with("candidate-")
        || draft.title.trim() != draft.title
        || draft.created_at == 0
        || draft.created_at > unix_seconds().saturating_add(60)
    {
        return Err(Error::new("candidate draft identity or time is invalid"));
    }
    validate_identifier(&draft.candidate_id, "candidate_id")?;
    validate_identifier(&draft.base_generation, "base_generation")?;
    validate_source_identity(&draft.source_id)?;
    validate_plain_text(&draft.title, "candidate title")?;
    if draft.schema == DRAFT_SCHEMA {
        draft.authoring_provenance.require_clean()?;
    }
    match (draft.stage, draft.submission.as_ref()) {
        (DraftStage::Editing, None) | (DraftStage::Prepared | DraftStage::Submitted, Some(_)) => {},
        _ => {
            return Err(Error::new(
                "candidate stage and submission state are inconsistent",
            ));
        },
    }
    for (source_id, replacement) in &draft.replacements {
        let _ = repository_path(source_id)?;
        validate_hex64(&replacement.source_sha256, "replacement source hash")?;
        validate_hex64(&replacement.content_sha256, "replacement content hash")?;
        validate_candidate_content(&replacement.content)?;
        if replacement.content_sha256 != sha256(replacement.content.as_bytes())
            || replacement.new_line_count != replacement.content.lines().count()
            || replacement.conservative_changed_lines == 0
            || replacement.conservative_changed_lines
                > replacement
                    .original_line_count
                    .saturating_add(replacement.new_line_count)
                    .saturating_add(1)
        {
            return Err(Error::new("candidate replacement metadata is inconsistent"));
        }
    }
    validate_limits(draft)?;
    if let Some(submission) = &draft.submission {
        validate_submission(draft, submission)?;
    }
    Ok(())
}

fn validate_submission(draft: &Draft, submission: &Submission) -> Result<()> {
    draft.authoring_provenance.require_clean()?;
    for (value, label) in [
        (&submission.candidate_sha256, "candidate hash"),
        (&submission.patch_sha256, "candidate patch hash"),
        (&submission.proposal_sha256, "candidate proposal hash"),
    ] {
        validate_hex64(value, label)?;
    }
    let expected_paths = draft
        .replacements
        .keys()
        .map(|source_id| repository_path(source_id))
        .collect::<Result<Vec<_>>>()?;
    let manifest = &submission.manifest;
    let expected_patch_sha256 = sha256(&canonical_json(&patch_bundle(draft)?)?);
    if manifest.schema != CANDIDATE_SCHEMA
        || manifest.candidate_id != draft.candidate_id
        || manifest.base_generation != draft.base_generation
        || manifest.proposal_sha256 != submission.proposal_sha256
        || manifest.patch_sha256 != submission.patch_sha256
        || submission.patch_sha256 != expected_patch_sha256
        || manifest.changed_paths != expected_paths
        || manifest.created_at < draft.created_at
        || manifest.created_at > unix_seconds().saturating_add(60)
        || manifest.privilege_envelope != "proposal-only:no-execution:v1"
        || submission.candidate_sha256 != sha256(&canonical_json(manifest)?)
    {
        return Err(Error::new("candidate submission binding is inconsistent"));
    }
    Ok(())
}

fn require_candidate_identity(draft: &Draft, candidate: &SubmittedCandidate) -> Result<()> {
    let submission = draft
        .submission
        .as_ref()
        .ok_or_else(|| Error::new("candidate submission metadata is absent"))?;
    if draft.candidate_id != candidate.candidate_id
        || submission.candidate_sha256 != candidate.candidate_sha256
        || submission.patch_sha256 != candidate.patch_sha256
        || submission.manifest != candidate.manifest
    {
        return Err(Error::new("candidate identity changed during publication"));
    }
    Ok(())
}

fn require_candidate_stage(
    draft: &Draft,
    candidate: &SubmittedCandidate,
    stage: DraftStage,
) -> Result<()> {
    require_candidate_identity(draft, candidate)?;
    if draft.stage != stage {
        return Err(Error::new("candidate publication stage is not exact"));
    }
    Ok(())
}

fn validate_source_identity(value: &str) -> Result<()> {
    let digest = value
        .strip_prefix("cpu-edge:")
        .ok_or_else(|| Error::new("candidate source identity has unsupported form"))?;
    validate_hex64(digest, "candidate source identity")
}

fn validate_plain_text(value: &str, label: &str) -> Result<()> {
    if value.trim() != value
        || value.chars().any(|character| {
            character.is_control()
                || matches!(character, '\u{202a}'..='\u{202e}' | '\u{2066}'..='\u{2069}')
        })
    {
        return Err(Error::new(format!("{label} is not exact plain text")));
    }
    Ok(())
}

fn validate_candidate_content(content: &str) -> Result<()> {
    if content.chars().any(|character| {
        (character.is_control() && !matches!(character, '\n' | '\t'))
            || matches!(character, '\u{202a}'..='\u{202e}' | '\u{2066}'..='\u{2069}')
    }) {
        return Err(Error::new(
            "binary or display-ambiguous candidate content rejected",
        ));
    }
    Ok(())
}

fn read_generation(path: &Path) -> Result<String> {
    let bytes = read_stable_regular(path, 256)?;
    let value = std::str::from_utf8(&bytes)
        .map_err(|_| Error::new("current generation is not UTF-8"))?
        .trim()
        .to_owned();
    validate_identifier(&value, "current generation")?;
    Ok(value)
}

fn validate_limits(draft: &Draft) -> Result<()> {
    let total_content_bytes = draft
        .replacements
        .values()
        .map(|replacement| replacement.content.len())
        .fold(0_usize, usize::saturating_add);
    if draft.replacements.len() > MAX_FILES
        || total_lines(draft) > MAX_CHANGED_LINES
        || total_content_bytes > MAX_TOTAL_CONTENT_BYTES
    {
        return Err(Error::new(
            "candidate exceeds 25 files, 4,000 changed lines, or 8 MiB total content",
        ));
    }
    if draft.replacements.values().any(|replacement| {
        replacement.content.len() > MAX_CONTENT_BYTES
            || replacement.content_sha256 != sha256(replacement.content.as_bytes())
    }) {
        return Err(Error::new("candidate replacement integrity or size failed"));
    }
    Ok(())
}

fn total_lines(draft: &Draft) -> usize {
    draft
        .replacements
        .values()
        .map(|replacement| replacement.conservative_changed_lines)
        .fold(0_usize, usize::saturating_add)
}

fn parse_line_edits(arguments: &Value) -> Result<Vec<LineEdit>> {
    let raw = arguments
        .get("edits")
        .and_then(Value::as_array)
        .ok_or_else(|| Error::new("candidate line edits must be an array"))?;
    if raw.is_empty() || raw.len() > MAX_LINE_EDITS {
        return Err(Error::new("candidate line edits must contain 1..64 hunks"));
    }
    raw.iter()
        .cloned()
        .map(|value| {
            let edit: LineEdit = serde_json::from_value(value)
                .map_err(|_| Error::new("candidate line edit schema is invalid"))?;
            validate_candidate_content(&edit.replacement)?;
            if edit.replacement.len() > MAX_CONTENT_BYTES {
                return Err(Error::new("candidate line edit replacement is oversized"));
            }
            Ok(edit)
        })
        .collect()
}

/// Apply one-based line edits against one exact, hash-bound file image.
/// Ranges are evaluated against the unmodified input and committed atomically.
fn apply_line_edits(input: &str, edits: &[LineEdit]) -> Result<String> {
    if edits.is_empty() || edits.len() > MAX_LINE_EDITS {
        return Err(Error::new("candidate line edits must contain 1..64 hunks"));
    }
    let mut starts = vec![0_usize];
    for (index, byte) in input.bytes().enumerate() {
        if byte == b'\n' {
            starts.push(index.saturating_add(1));
        }
    }
    let line_count = if input.is_empty() {
        0
    } else if input.ends_with('\n') {
        starts.len().saturating_sub(1)
    } else {
        starts.len()
    };
    let byte_offset = |line_index: usize| -> Result<usize> {
        if line_index > line_count {
            return Err(Error::new("candidate line edit range exceeds the file"));
        }
        Ok(if line_index == line_count {
            input.len()
        } else {
            starts[line_index]
        })
    };

    let mut ranges = Vec::with_capacity(edits.len());
    let mut prior_start = None;
    let mut prior_end = 0_usize;
    for edit in edits {
        if edit.start_line == 0 || edit.end_line == 0 {
            return Err(Error::new("candidate line edits are one-based"));
        }
        if edit.end_line < edit.start_line {
            return Err(Error::new(
                "candidate line edit end must not precede its start",
            ));
        }
        let start = edit.start_line.saturating_sub(1);
        let end = edit.end_line.saturating_sub(1);
        if start > line_count
            || end > line_count
            || prior_start.is_some_and(|previous| start <= previous)
            || start < prior_end
        {
            return Err(Error::new(
                "candidate line edits must be sorted, distinct, and non-overlapping",
            ));
        }
        let start_byte = byte_offset(start)?;
        let end_byte = byte_offset(end)?;
        if !edit.replacement.is_empty()
            && end_byte < input.len()
            && !edit.replacement.ends_with('\n')
        {
            return Err(Error::new(
                "candidate line replacement before retained text must end with newline",
            ));
        }
        if !edit.replacement.is_empty()
            && start_byte == input.len()
            && start_byte > 0
            && !input.ends_with('\n')
            && !edit.replacement.starts_with('\n')
        {
            return Err(Error::new(
                "candidate append after an unterminated line must begin with newline",
            ));
        }
        ranges.push((start_byte, end_byte, edit.replacement.as_str()));
        prior_start = Some(start);
        prior_end = end;
    }

    let replacement_bytes = edits
        .iter()
        .map(|edit| edit.replacement.len())
        .fold(0_usize, usize::saturating_add);
    let deleted_bytes = ranges
        .iter()
        .map(|(start, end, _)| end.saturating_sub(*start))
        .fold(0_usize, usize::saturating_add);
    let capacity = input
        .len()
        .saturating_sub(deleted_bytes)
        .saturating_add(replacement_bytes);
    if capacity > MAX_CONTENT_BYTES {
        return Err(Error::new("candidate line edit output is oversized"));
    }
    let mut output = String::with_capacity(capacity);
    let mut cursor = 0_usize;
    for (start, end, replacement) in ranges {
        output.push_str(&input[cursor..start]);
        output.push_str(replacement);
        cursor = end;
    }
    output.push_str(&input[cursor..]);
    Ok(output)
}

/// Bounded Myers line edit distance. Insertions and deletions count one line;
/// a replacement therefore counts two. Work stops at `limit + 1`.
fn bounded_changed_lines(original: &str, replacement: &str, limit: usize) -> usize {
    if original == replacement {
        return 0;
    }
    let before = original.lines().collect::<Vec<_>>();
    let after = replacement.lines().collect::<Vec<_>>();
    let prefix = before
        .iter()
        .zip(&after)
        .take_while(|(left, right)| left == right)
        .count();
    let mut before_end = before.len();
    let mut after_end = after.len();
    while before_end > prefix
        && after_end > prefix
        && before[before_end.saturating_sub(1)] == after[after_end.saturating_sub(1)]
    {
        before_end = before_end.saturating_sub(1);
        after_end = after_end.saturating_sub(1);
    }
    let before = &before[prefix..before_end];
    let after = &after[prefix..after_end];
    if before.len().abs_diff(after.len()) > limit {
        return limit.saturating_add(1);
    }
    let maximum = before.len().saturating_add(after.len()).min(limit);
    let offset = maximum.saturating_add(1);
    let mut furthest = vec![0_usize; maximum.saturating_mul(2).saturating_add(3)];
    for distance in 0..=maximum {
        let distance_signed = isize::try_from(distance).unwrap_or(isize::MAX);
        let negative_distance = distance_signed.checked_neg().unwrap_or(isize::MIN);
        let mut diagonal = negative_distance;
        while diagonal <= distance_signed {
            let index_signed =
                diagonal.saturating_add(isize::try_from(offset).unwrap_or(isize::MAX));
            let Ok(index) = usize::try_from(index_signed) else {
                return limit.saturating_add(1);
            };
            let mut x = if diagonal == negative_distance
                || (diagonal != distance_signed
                    && furthest[index.saturating_sub(1)] < furthest[index.saturating_add(1)])
            {
                furthest[index.saturating_add(1)]
            } else {
                furthest[index.saturating_sub(1)].saturating_add(1)
            };
            let Some(mut y) = isize::try_from(x)
                .ok()
                .and_then(|value| value.checked_sub(diagonal))
                .and_then(|value| usize::try_from(value).ok())
            else {
                return limit.saturating_add(1);
            };
            while x < before.len() && y < after.len() && before[x] == after[y] {
                x = x.saturating_add(1);
                y = y.saturating_add(1);
            }
            furthest[index] = x;
            if x == before.len() && y == after.len() {
                return distance;
            }
            diagonal = diagonal.saturating_add(2);
        }
    }
    limit.saturating_add(1)
}

fn exact_keys(value: &Value, expected: &[&str]) -> Result<()> {
    let object = value
        .as_object()
        .ok_or_else(|| Error::new("tool arguments must be an object"))?;
    let actual = object
        .keys()
        .map(String::as_str)
        .collect::<std::collections::BTreeSet<_>>();
    let expected = expected
        .iter()
        .copied()
        .collect::<std::collections::BTreeSet<_>>();
    if actual != expected {
        return Err(Error::new(
            "tool arguments contain missing or unadvertised fields",
        ));
    }
    Ok(())
}

fn argument_string(value: &Value, key: &str, minimum: usize, maximum: usize) -> Result<String> {
    let text = value
        .get(key)
        .and_then(Value::as_str)
        .ok_or_else(|| Error::new(format!("{key} must be a string")))?;
    let length = text.chars().count();
    if length < minimum || length > maximum {
        return Err(Error::new(format!("{key} exceeds its bound")));
    }
    Ok(text.to_owned())
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;
    use std::fs;

    use super::{
        CandidateManager, DRAFT_SCHEMA, Draft, DraftAuthoringProvenance, DraftEnvelope, DraftStage,
        EDIT_TRANSACTION_SCHEMA, LEGACY_DRAFT_SCHEMA, LineEdit, MAX_CHANGED_LINES, PendingEditCore,
        PendingEditEnvelope, Replacement, Submission, apply_line_edits, bounded_changed_lines,
        decode_draft_envelope, exact_keys, parse_line_edits, patch_bundle, prompt_status_for_draft,
        sha256, signed_draft_envelope, validate_candidate_content, validate_draft, validate_limits,
    };
    use crate::attestation::{CANDIDATE_SCHEMA, HmacSigner, SupervisorCandidate};
    use crate::candidate_ledger::{CandidateEditLedger, EventContext};
    use crate::util::{atomic_private_write, canonical_json};

    fn context<'a>() -> EventContext<'a> {
        EventContext {
            due_nonce: "due-10000",
            trace_id: "11111111-1111-4111-8111-111111111111",
            session_id: "session-one",
            turn_id: "22222222-2222-4222-8222-222222222222",
            response_sha256: "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
            declaration_sha256: "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb",
            context_provenance_sha256: "cccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccc",
        }
    }

    #[test]
    fn terminal_context_requires_the_exact_latest_clean_scheduled_trace() {
        let exact = context();
        let provenance = DraftAuthoringProvenance::clean(&exact).unwrap();
        assert!(
            provenance
                .require_exact_terminal_context(
                    exact.due_nonce,
                    exact.trace_id,
                    exact.session_id,
                    exact.turn_id,
                    exact.context_provenance_sha256,
                )
                .is_ok()
        );
        for changed in [
            EventContext {
                due_nonce: "due-foreign",
                ..exact
            },
            EventContext {
                trace_id: "foreign-trace",
                ..exact
            },
            EventContext {
                session_id: "foreign-session",
                ..exact
            },
            EventContext {
                turn_id: "foreign-turn",
                ..exact
            },
            EventContext {
                context_provenance_sha256: "dddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddd",
                ..exact
            },
        ] {
            assert!(
                provenance
                    .require_exact_terminal_context(
                        changed.due_nonce,
                        changed.trace_id,
                        changed.session_id,
                        changed.turn_id,
                        changed.context_provenance_sha256,
                    )
                    .is_err()
            );
        }
    }

    fn draft_with_replacement(original_lines: usize, new_lines: usize) -> Draft {
        let content = "new\n".repeat(new_lines);
        let event_context = context();
        Draft {
            schema: DRAFT_SCHEMA.to_owned(),
            candidate_id: "candidate-test".to_owned(),
            source_id: format!("cpu-edge:{}", "a".repeat(64)),
            base_generation: "generation-test".to_owned(),
            title: "bounded test".to_owned(),
            created_at: super::unix_seconds(),
            stage: DraftStage::Editing,
            authoring_provenance: DraftAuthoringProvenance::clean(&event_context).unwrap(),
            replacements: BTreeMap::from([(
                "source/services/astrid-edge-runtime/src/lib.rs".to_owned(),
                Replacement {
                    source_sha256: "b".repeat(64),
                    content_sha256: sha256(content.as_bytes()),
                    content,
                    conservative_changed_lines: original_lines.saturating_add(new_lines),
                    original_line_count: original_lines.saturating_sub(1),
                    new_line_count: new_lines,
                },
            )]),
            submission: None,
        }
    }

    #[test]
    fn prompt_status_distinguishes_no_candidate_editing_and_prepared() {
        assert_eq!(prompt_status_for_draft(None)["stage"], "none");
        let mut draft = draft_with_replacement(2, 3);
        let editing = prompt_status_for_draft(Some(&draft));
        assert_eq!(editing["stage"], "editing");
        assert_eq!(editing["files"], 1);
        assert!(editing["candidate_sha256"].is_null());

        draft.stage = DraftStage::Prepared;
        draft.submission = Some(Submission {
            candidate_sha256: "c".repeat(64),
            patch_sha256: "d".repeat(64),
            proposal_sha256: "e".repeat(64),
            manifest: SupervisorCandidate {
                schema: CANDIDATE_SCHEMA.to_owned(),
                candidate_id: draft.candidate_id.clone(),
                base_generation: draft.base_generation.clone(),
                proposal_sha256: "e".repeat(64),
                patch_sha256: "d".repeat(64),
                changed_paths: vec!["services/astrid-edge-runtime/src/lib.rs".to_owned()],
                created_at: draft.created_at,
                privilege_envelope: "proposal-only:no-execution:v1".to_owned(),
            },
        });
        let prepared = prompt_status_for_draft(Some(&draft));
        assert_eq!(prepared["stage"], "prepared");
        assert_eq!(prepared["candidate_sha256"], "c".repeat(64));
    }

    #[test]
    fn legacy_draft_survives_authenticated_restart_but_cannot_author_or_submit() {
        let temporary = tempfile::tempdir_in(std::env::current_dir().unwrap()).unwrap();
        let key = temporary.path().join("key");
        fs::write(&key, [b'k'; 32]).unwrap();
        let signer = HmacSigner::from_file(&key).unwrap();
        let mut legacy = draft_with_replacement(2, 2);
        legacy.schema = LEGACY_DRAFT_SCHEMA.to_owned();
        legacy.authoring_provenance = DraftAuthoringProvenance::default();
        let draft_bytes = canonical_json(&legacy).unwrap();
        let envelope = DraftEnvelope {
            schema: LEGACY_DRAFT_SCHEMA.to_owned(),
            draft: legacy,
            draft_sha256: sha256(&draft_bytes),
            key_id: signer.key_id.clone(),
            hmac_sha256: signer.sign(&draft_bytes),
        };
        let decoded = decode_draft_envelope(&canonical_json(&envelope).unwrap(), &signer).unwrap();
        assert_eq!(
            prompt_status_for_draft(Some(&decoded))["authoring_provenance"],
            "legacy_unattributed_quarantined"
        );
        assert!(patch_bundle(&decoded).is_err());
        assert!(decoded.authoring_provenance.require_clean().is_err());
    }

    #[test]
    fn zero_argument_tools_reject_smuggled_fields() {
        assert!(exact_keys(&serde_json::json!({}), &[]).is_ok());
        assert!(exact_keys(&serde_json::json!({"command":"rm -rf /"}), &[]).is_err());
    }

    #[test]
    fn changed_line_metric_is_bounded_and_counts_actual_edits() {
        assert_eq!(bounded_changed_lines("a\n", "b\n", MAX_CHANGED_LINES), 2);
        assert_eq!(
            bounded_changed_lines("same\n", "same\n", MAX_CHANGED_LINES),
            0
        );
        assert_eq!(bounded_changed_lines("a\nb\n", "a\nx\nb\n", 4_000), 1);
        assert_eq!(bounded_changed_lines("a\nx\nb\n", "a\nb\n", 4_000), 1);
        assert_eq!(bounded_changed_lines("a\nb\n", "b\na\n", 4_000), 2);
        let large = "stable\n".repeat(12_000);
        let changed = large.replacen("stable\n", "changed\n", 1);
        assert_eq!(bounded_changed_lines(&large, &changed, 4_000), 2);
        let unrelated = "different\n".repeat(12_000);
        assert_eq!(bounded_changed_lines(&large, &unrelated, 4_000), 4_001);

        let mut at_limit = draft_with_replacement(1, 1);
        at_limit
            .replacements
            .values_mut()
            .next()
            .unwrap()
            .conservative_changed_lines = 4_000;
        assert!(validate_limits(&at_limit).is_ok());
        at_limit
            .replacements
            .values_mut()
            .next()
            .unwrap()
            .conservative_changed_lines = 4_001;
        assert!(validate_limits(&at_limit).is_err());
        assert_eq!(MAX_CHANGED_LINES, 4_000);
    }

    #[test]
    fn line_hunks_are_atomic_sorted_and_newline_safe() {
        let source = "stable\n".repeat(5_000);
        let updated = apply_line_edits(
            &source,
            &[LineEdit {
                start_line: 2_500,
                end_line: 2_501,
                replacement: "replacement\n".to_owned(),
            }],
        )
        .unwrap();
        assert!(updated.contains("stable\nreplacement\nstable\n"));
        assert_eq!(bounded_changed_lines(&source, &updated, 4_000), 2);

        let overlap = [
            LineEdit {
                start_line: 2,
                end_line: 4,
                replacement: "two\n".to_owned(),
            },
            LineEdit {
                start_line: 3,
                end_line: 4,
                replacement: "three\n".to_owned(),
            },
        ];
        assert!(apply_line_edits("a\nb\nc\nd\n", &overlap).is_err());
        assert!(
            apply_line_edits(
                "a\nb\n",
                &[LineEdit {
                    start_line: 2,
                    end_line: 1,
                    replacement: "impossible\n".to_owned(),
                }]
            )
            .is_err()
        );
        assert!(
            apply_line_edits(
                "a\nb\n",
                &[LineEdit {
                    start_line: 4,
                    end_line: 4,
                    replacement: "c\n".to_owned(),
                }]
            )
            .is_err()
        );
        assert!(
            apply_line_edits(
                "a\nb\n",
                &[LineEdit {
                    start_line: 1,
                    end_line: 2,
                    replacement: "unterminated".to_owned(),
                }]
            )
            .is_err()
        );
        assert_eq!(
            apply_line_edits(
                "unterminated",
                &[LineEdit {
                    start_line: 2,
                    end_line: 2,
                    replacement: "\nappended\n".to_owned(),
                }]
            )
            .unwrap(),
            "unterminated\nappended\n"
        );
        assert!(
            parse_line_edits(&serde_json::json!({
                "edits": [{"start_line": 1, "delete_lines": 1, "replacement": "legacy\n"}]
            }))
            .is_err()
        );
    }

    #[test]
    fn file_ceiling_is_exactly_twenty_five_across_the_whole_draft() {
        let mut draft = draft_with_replacement(2, 2);
        let template = draft.replacements.values().next().unwrap().clone();
        draft.replacements.clear();
        for index in 0..25 {
            draft.replacements.insert(
                format!("source/crates/astrid-core/src/generated_{index}.rs"),
                template.clone(),
            );
        }
        assert!(validate_limits(&draft).is_ok());
        draft.replacements.insert(
            "source/crates/astrid-core/src/overflow.rs".to_owned(),
            template,
        );
        assert!(validate_limits(&draft).is_err());
    }

    #[test]
    fn signed_draft_shape_and_candidate_text_are_unambiguous() {
        assert!(validate_draft(&draft_with_replacement(2, 2)).is_ok());
        assert!(validate_candidate_content("fn main() {\n\twork();\n}\n").is_ok());
        assert!(validate_candidate_content("safe\rhidden\n").is_err());
        assert!(validate_candidate_content("safe\u{202e}hidden\n").is_err());

        let mut inconsistent = draft_with_replacement(2, 2);
        inconsistent.stage = DraftStage::Submitted;
        assert!(validate_draft(&inconsistent).is_err());
    }

    #[test]
    fn pending_edit_after_state_switch_recovers_once_without_replay() {
        let temporary = tempfile::tempdir_in(std::env::current_dir().unwrap()).unwrap();
        let state = temporary.path().join("state");
        let outbox = temporary.path().join("outbox");
        fs::create_dir(&state).unwrap();
        fs::create_dir(&outbox).unwrap();
        let key = temporary.path().join("key");
        fs::write(&key, [b'k'; 32]).unwrap();
        let signer = HmacSigner::from_file(&key).unwrap();
        let generation = temporary.path().join("generation");
        fs::write(&generation, b"generation-test\n").unwrap();
        let context = context();
        let draft = Draft {
            schema: DRAFT_SCHEMA.to_owned(),
            candidate_id: "candidate-recovery".to_owned(),
            source_id: format!("cpu-edge:{}", "a".repeat(64)),
            base_generation: "generation-test".to_owned(),
            title: "recover exact pending edit".to_owned(),
            created_at: 1,
            stage: DraftStage::Editing,
            authoring_provenance: DraftAuthoringProvenance::clean(&context).unwrap(),
            replacements: BTreeMap::new(),
            submission: None,
        };
        let (after_envelope, after_bytes) = signed_draft_envelope(&draft, &signer).unwrap();
        let record = CandidateEditLedger::new(&state, &signer)
            .prepare(
                "begin_candidate",
                "completed",
                Some(&draft.candidate_id),
                Some(&draft.source_id),
                Some(&draft.base_generation),
                None,
                Some(&sha256(&after_bytes)),
                serde_json::json!({"files": 0, "changed_lines": 0}),
                &context,
            )
            .unwrap();
        let core = PendingEditCore {
            schema: EDIT_TRANSACTION_SCHEMA.to_owned(),
            before_active_sha256: None,
            after_active_sha256: Some(sha256(&after_bytes)),
            after_active: Some(after_envelope),
            ledger_record: record,
        };
        let core_bytes = canonical_json(&core).unwrap();
        let pending = PendingEditEnvelope {
            schema: EDIT_TRANSACTION_SCHEMA.to_owned(),
            core,
            core_sha256: sha256(&core_bytes),
            key_id: signer.key_id.clone(),
            hmac_sha256: signer.sign(&core_bytes),
        };
        atomic_private_write(
            &state.join("candidate-edit-pending.json"),
            &canonical_json(&pending).unwrap(),
        )
        .unwrap();
        // Crash boundary: next candidate state became durable, but ledger append and cleanup did
        // not. Construction must finish that exact prepared record without inventing a new one.
        atomic_private_write(&state.join("active-candidate.json"), &after_bytes).unwrap();
        let manager =
            CandidateManager::new_reconciler(&state, &outbox, &signer, &generation).unwrap();
        assert!(matches!(
            manager.active().unwrap(),
            Some(super::ActiveDraft::Editing)
        ));
        assert!(!state.join("candidate-edit-pending.json").exists());
        let first = fs::read(state.join("candidate-edits.jsonl")).unwrap();
        assert_eq!(
            first
                .split(|byte| *byte == b'\n')
                .filter(|line| !line.is_empty())
                .count(),
            1
        );
        let _ = CandidateManager::new_reconciler(&state, &outbox, &signer, &generation).unwrap();
        assert_eq!(
            fs::read(state.join("candidate-edits.jsonl")).unwrap(),
            first
        );
    }
}
