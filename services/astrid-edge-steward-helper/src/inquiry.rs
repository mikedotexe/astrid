//! Exact authored-inquiry parsing and immutable signed history.
//!
//! The rich model may describe its reasoning in prose, but only the final two
//! exact declarations below create a structured continuity event.  This is an
//! authored intellectual record, not hidden provider chain-of-thought and not
//! candidate, build, or deployment authority.

use std::collections::BTreeSet;
use std::fs::{self, OpenOptions};
use std::io::Write as _;
use std::os::unix::fs::{MetadataExt as _, OpenOptionsExt as _, PermissionsExt as _};
use std::path::{Path, PathBuf};
use std::sync::Mutex;

use fs2::FileExt as _;
use serde::{Deserialize, Serialize};

use crate::attestation::HmacSigner;
use crate::config::Config;
use crate::context_provenance::ContextProvenance;
use crate::reporting::workspace_write;
use crate::util::{
    bounded_text, canonical_json, read_stable_regular, require_absolute_no_symlink, sha256,
    validate_hex64, validate_identifier,
};
use crate::{Error, Result};

pub(crate) const STEP_SCHEMA: &str = "astrid.edge.inquiry.step.v1";
const ENTRY_SCHEMA: &str = "astrid.edge.inquiry.entry.v1";
const ENTRY_ENVELOPE_SCHEMA: &str = "astrid.edge.inquiry.entry_envelope.v1";
const HEAD_SCHEMA: &str = "astrid.edge.inquiry.head.v1";
const HEAD_ENVELOPE_SCHEMA: &str = "astrid.edge.inquiry.head_envelope.v1";
pub(crate) const CURRENT_SCHEMA: &str = "astrid.edge.inquiry.current.v1";
const SIGNATURE_ALGORITHM: &str = "ed25519";
const ENTRY_ID_DOMAIN: &[u8] = b"astrid.edge.inquiry.entry-id.v1\0";
const ADMISSION_ID_DOMAIN: &[u8] = b"astrid.edge.inquiry.admission.v1\0";
const STEP_ID_DOMAIN: &[u8] = b"astrid.edge.inquiry.step-id.v1\0";

pub(crate) fn authored_provenance(trigger_kind: &str) -> &'static str {
    match trigger_kind {
        "scheduled" => "model_authored_runtime_scheduled",
        "evidence_integration" => "model_authored_runtime_evidence_integration",
        _ => "invalid_authored_trigger_provenance",
    }
}
const GENESIS_HASH: &str = "0000000000000000000000000000000000000000000000000000000000000000";
pub(crate) const SEGMENT_BYTES: u64 = 4 * 1024 * 1024;
const MAX_ENTRY_BYTES: u64 = 32 * 1024;
const MAX_HEAD_BYTES: u64 = 16 * 1024;
const MAX_CURRENT_BYTES: u64 = 48 * 1024;
static HISTORY_SETUP_LOCK: Mutex<()> = Mutex::new(());

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum ThreadOperation {
    Continue,
    Open,
    Branch,
    Pause,
    Close,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum Confidence {
    Tentative,
    Moderate,
    Strong,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum BeliefOperation {
    Unchanged,
    Propose,
    Support,
    Weaken,
    Revise,
    Suspend,
    Resolve,
}

/// The exact, bounded JSON object authored after `INQUIRY_STEP: `.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct InquiryStepV1 {
    pub schema: String,
    pub thread_operation: ThreadOperation,
    pub thread_id: String,
    pub parent_step_id: Option<String>,
    pub observation: String,
    pub interpretation: String,
    pub uncertainty: String,
    pub decision: String,
    pub counterpoint: Option<String>,
    pub next_test: Option<String>,
    #[serde(default)]
    pub evidence_ids: Vec<String>,
    pub confidence: Confidence,
    pub belief_operation: Option<BeliefOperation>,
    pub belief_id: Option<String>,
    pub belief_claim: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum SourceReviewDecision {
    None,
    Request,
}

impl SourceReviewDecision {
    pub(crate) const fn requested(self) -> bool {
        matches!(self, Self::Request)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct StructuredInquiry {
    pub step: InquiryStepV1,
    pub declaration: String,
    pub declaration_sha256: String,
    pub source_review: SourceReviewDecision,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct InquiryClassification {
    pub status: String,
    pub structured: Option<StructuredInquiry>,
    pub failure_class: Option<String>,
}

impl InquiryClassification {
    pub(crate) fn structured(&self) -> Option<&StructuredInquiry> {
        self.structured.as_ref()
    }

    pub(crate) fn is_structured(&self) -> bool {
        self.status == "model_authored_structured" && self.structured.is_some()
    }

    pub(crate) fn source_review_requested(&self) -> bool {
        self.structured()
            .is_some_and(|value| value.source_review.requested())
    }

    pub(crate) fn partial_generation() -> Self {
        Self {
            status: "model_authored_unstructured".to_owned(),
            structured: None,
            failure_class: Some("provider_output_ceiling_partial_generation".to_owned()),
        }
    }

    pub(crate) fn forced_unstructured(failure_class: &str) -> Self {
        Self {
            status: "model_authored_unstructured".to_owned(),
            structured: None,
            failure_class: Some(bounded_text(failure_class, 96)),
        }
    }
}

/// Parse the two exact terminal declarations without repairing any syntax.
///
/// Invalid terminal shape deliberately returns a non-error, unstructured
/// classification: the complete provider response is still model-authored and
/// must be retained verbatim, but it gains no continuity, reservoir, source
/// review, or candidate effect.
pub(crate) fn classify(response: &str) -> InquiryClassification {
    classify_exact(response).unwrap_or_else(|error| InquiryClassification {
        status: "model_authored_unstructured".to_owned(),
        structured: None,
        failure_class: Some(bounded_failure_class(&error)),
    })
}

fn classify_exact(response: &str) -> Result<InquiryClassification> {
    if response.contains('\r') || response.contains('\0') {
        return Err(Error::new("inquiry response contains ambiguous controls"));
    }
    let response = response.strip_suffix('\n').unwrap_or(response);
    if response.ends_with('\n') {
        return Err(Error::new(
            "inquiry response has multiple trailing newlines",
        ));
    }
    let lines = response.split('\n').collect::<Vec<_>>();
    if lines.len() < 2 {
        return Err(Error::new("inquiry terminal declarations are absent"));
    }
    let inquiry_markers = lines
        .iter()
        .enumerate()
        .filter(|(_, line)| line.starts_with("INQUIRY_STEP:"))
        .collect::<Vec<_>>();
    let source_markers = lines
        .iter()
        .enumerate()
        .filter(|(_, line)| line.starts_with("SOURCE_REVIEW:"))
        .collect::<Vec<_>>();
    if inquiry_markers.len() != 1
        || source_markers.len() != 1
        || inquiry_markers[0].0 != lines.len().saturating_sub(2)
        || source_markers[0].0 != lines.len().saturating_sub(1)
    {
        return Err(Error::new(
            "inquiry declarations must be the unique final two lines",
        ));
    }
    let declaration = *inquiry_markers[0].1;
    let raw = declaration
        .strip_prefix("INQUIRY_STEP: ")
        .ok_or_else(|| Error::new("inquiry declaration prefix is not exact"))?;
    if raw.is_empty() || raw.trim() != raw || raw.contains(['\n', '\r', '\0']) {
        return Err(Error::new("inquiry JSON is not one exact line"));
    }
    let step: InquiryStepV1 = serde_json::from_str(raw)?;
    validate_step(&step)?;
    let source_review = match *source_markers[0].1 {
        "SOURCE_REVIEW: NONE" => SourceReviewDecision::None,
        "SOURCE_REVIEW: REQUEST" => SourceReviewDecision::Request,
        _ => return Err(Error::new("source-review declaration is not exact")),
    };
    Ok(InquiryClassification {
        status: "model_authored_structured".to_owned(),
        structured: Some(StructuredInquiry {
            step,
            declaration: declaration.to_owned(),
            declaration_sha256: sha256(declaration.as_bytes()),
            source_review,
        }),
        failure_class: None,
    })
}

fn validate_step(step: &InquiryStepV1) -> Result<()> {
    if step.schema != STEP_SCHEMA {
        return Err(Error::new("inquiry step schema is unsupported"));
    }
    validate_short_id(&step.thread_id, "thread_id")?;
    if let Some(parent) = &step.parent_step_id {
        validate_short_id(parent, "parent_step_id")?;
    }
    let parent_required = !matches!(step.thread_operation, ThreadOperation::Open);
    if parent_required != step.parent_step_id.is_some() {
        return Err(Error::new(
            "thread operation has inconsistent semantic parentage",
        ));
    }
    validate_text(&step.observation, 480, "observation")?;
    validate_text(&step.interpretation, 480, "interpretation")?;
    validate_text(&step.uncertainty, 320, "uncertainty")?;
    validate_text(&step.decision, 480, "decision")?;
    if let Some(value) = &step.counterpoint {
        validate_text(value, 320, "counterpoint")?;
    }
    if let Some(value) = &step.next_test {
        validate_text(value, 320, "next_test")?;
    }
    if step.evidence_ids.len() > 6 {
        return Err(Error::new("inquiry step cites more than six evidence IDs"));
    }
    let mut evidence = BTreeSet::new();
    for evidence_id in &step.evidence_ids {
        validate_short_id(evidence_id, "evidence_id")?;
        if !evidence.insert(evidence_id) {
            return Err(Error::new("inquiry step repeats an evidence ID"));
        }
    }
    match step.belief_operation {
        None => {
            if step.belief_id.is_some() || step.belief_claim.is_some() {
                return Err(Error::new(
                    "belief fields require an explicit belief operation",
                ));
            }
        },
        Some(BeliefOperation::Unchanged) => {
            if step.belief_id.is_some() || step.belief_claim.is_some() {
                return Err(Error::new(
                    "unchanged belief operation carries no belief mutation",
                ));
            }
        },
        Some(_) => {
            let id = step
                .belief_id
                .as_deref()
                .ok_or_else(|| Error::new("belief operation is missing belief_id"))?;
            let claim = step
                .belief_claim
                .as_deref()
                .ok_or_else(|| Error::new("belief operation is missing belief_claim"))?;
            validate_short_id(id, "belief_id")?;
            validate_text(claim, 480, "belief_claim")?;
        },
    }
    Ok(())
}

fn validate_short_id(value: &str, label: &str) -> Result<()> {
    if value.len() > 96 || value.contains(['/', '\\', '\0']) {
        return Err(Error::new(format!("invalid {label}")));
    }
    validate_identifier(value, label)
}

fn validate_text(value: &str, maximum: usize, label: &str) -> Result<()> {
    if value.is_empty()
        || value.trim() != value
        || value.chars().count() > maximum
        || value.chars().any(char::is_control)
    {
        return Err(Error::new(format!("invalid inquiry {label}")));
    }
    Ok(())
}

fn bounded_failure_class(error: &Error) -> String {
    let value = error
        .to_string()
        .split_whitespace()
        .collect::<Vec<_>>()
        .join("_")
        .to_ascii_lowercase();
    bounded_text(&value, 96)
}

#[derive(Debug, Clone)]
pub(crate) struct PersistInput<'a> {
    pub appliance_id: &'a str,
    pub trigger_kind: &'a str,
    pub due_nonce: &'a str,
    pub trigger_nonce: &'a str,
    pub recorded_at_unix_ms: u64,
    pub trace_id: &'a str,
    pub session_id: &'a str,
    pub turn_id: &'a str,
    pub span_id: &'a str,
    pub prompt_sha256: &'a str,
    pub response_sha256: &'a str,
    pub context_provenance: &'a ContextProvenance,
    pub reflection_path: &'a Path,
    pub reflection_sha256: &'a str,
    pub inquiry: &'a StructuredInquiry,
}

#[derive(Debug, Clone)]
pub(crate) struct ProjectionReceipt {
    pub bytes: Vec<u8>,
    pub sha256: String,
    pub signed_entry_id: String,
    pub step_id: String,
    pub admission_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct TraceV1 {
    schema_version: u8,
    trace_id: String,
    turn_id: String,
    span_id: String,
    session_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct EntryCore {
    schema: String,
    appliance_id: String,
    signed_entry_id: String,
    step_id: String,
    admission_id: String,
    recorded_at_unix_ms: u64,
    trigger_kind: String,
    due_nonce: String,
    trigger_nonce: String,
    trace: TraceV1,
    prompt_sha256: String,
    response_sha256: String,
    context_provenance_sha256: String,
    reflection_path: String,
    reflection_sha256: String,
    declaration: String,
    declaration_sha256: String,
    inquiry_step: InquiryStepV1,
    inquiry_step_sha256: String,
    summary: String,
    summary_sha256: String,
    prior_entry_sha256: String,
    mechanical_predecessor: String,
    semantic_parent_step_id: Option<String>,
    provenance: String,
    authority: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Auth {
    algorithm: String,
    key_id: String,
    signature: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct SignedEntry {
    schema: String,
    core: EntryCore,
    core_sha256: String,
    auth: Auth,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct HeadCore {
    schema: String,
    appliance_id: String,
    entry_count: u64,
    segment: u64,
    entry_index: u64,
    signed_entry_id: String,
    entry_sha256: String,
    segment_bytes: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct SignedHead {
    schema: String,
    core: HeadCore,
    core_sha256: String,
    auth: Auth,
}

#[derive(Debug, Clone)]
struct ChainTail {
    entry_count: u64,
    segment: u64,
    entry_index: u64,
    signed_entry_id: Option<String>,
    entry_sha256: String,
    segment_bytes: u64,
    existing_entry: Option<(SignedEntry, String, u64, u64)>,
}

#[derive(Debug, Clone, Serialize)]
struct LedgerProjection {
    segment: u64,
    entry_index: u64,
    prior_entry_sha256: String,
    entry_sha256: String,
    key_id: String,
    signature_algorithm: String,
    signature: String,
}

#[derive(Debug, Clone, Serialize)]
struct InquiryCurrentCore {
    schema: &'static str,
    appliance_id: String,
    signed_entry_id: String,
    step_id: String,
    admission_id: String,
    recorded_at_unix_ms: u64,
    summary: String,
    summary_sha256: String,
    inquiry_step: InquiryStepV1,
    inquiry_step_sha256: String,
    declaration_sha256: String,
    response_sha256: String,
    trace: TraceV1,
    trigger_kind: String,
    due_nonce: String,
    trigger_nonce: String,
    reflection_path: String,
    reflection_sha256: String,
    ledger: LedgerProjection,
    provenance: &'static str,
    authority: &'static str,
}

#[derive(Debug, Clone, Serialize)]
struct InquiryCurrentProjection {
    #[serde(flatten)]
    core: InquiryCurrentCore,
    core_sha256: String,
    auth: Auth,
}

pub(crate) fn persist(
    config: &Config,
    signer: &HmacSigner,
    input: &PersistInput<'_>,
) -> Result<ProjectionReceipt> {
    validate_persist_input(config, input)?;
    let root = inquiry_root(config);
    let setup_guard = HISTORY_SETUP_LOCK
        .lock()
        .map_err(|_| Error::new("inquiry history setup lock is poisoned"))?;
    ensure_history_dir(&root, config.workspace_gid)?;
    let ledger_lock = open_ledger_lock(&root, config.workspace_gid)?;
    let segments = root.join("segments");
    ensure_history_dir(&segments, config.workspace_gid)?;
    drop(setup_guard);
    ledger_lock.lock_exclusive()?;
    let mut tail = scan_chain(config, signer, &segments, None)?;
    let signed_entry_id = signed_entry_id(input);
    let admission_id = admission_id(&config.appliance_id, &signed_entry_id);
    let step_id = step_id(&config.appliance_id, &signed_entry_id);
    if let Some((entry, entry_sha256, segment, entry_index)) = &tail.existing_entry
        && entry.core.signed_entry_id == signed_entry_id
    {
        let expected = build_entry(
            config,
            signer,
            input,
            &tail_for_existing(entry, entry_sha256, *segment, *entry_index),
            &signed_entry_id,
            &step_id,
            &admission_id,
        )?;
        if canonical_json(&expected)? != canonical_json(entry)? {
            return Err(Error::new("signed inquiry entry replay differs"));
        }
        return write_current_projection(
            config,
            signer,
            entry,
            entry_sha256,
            *segment,
            *entry_index,
        );
    }
    if find_entry_by_id(config, signer, &segments, &signed_entry_id)?.is_some() {
        return Err(Error::new(
            "signed inquiry entry replay is not the chain tail",
        ));
    }
    let mut segment = tail.segment.max(1);
    let mut entry_index = tail.entry_index.saturating_add(1);
    let entry = build_entry(
        config,
        signer,
        input,
        &tail,
        &signed_entry_id,
        &step_id,
        &admission_id,
    )?;
    let mut line = canonical_json(&entry)?;
    line.push(b'\n');
    if line.len() as u64 > MAX_ENTRY_BYTES {
        return Err(Error::new("signed inquiry entry exceeds its bound"));
    }
    if should_roll_segment(tail.segment_bytes, line.len() as u64, SEGMENT_BYTES) {
        segment = segment.saturating_add(1);
        entry_index = 1;
        // Segment placement is projection metadata, not part of the signed
        // intellectual record. Rebuilding is unnecessary and would create a
        // second signature for the same semantic step.
    }
    let segment_path = segments.join(segment_name(segment));
    append_history(&segment_path, &line, config.workspace_gid)?;
    let entry_sha256 = sha256(&canonical_json(&entry)?);
    tail.entry_count = tail.entry_count.saturating_add(1);
    tail.segment = segment;
    tail.entry_index = entry_index;
    tail.signed_entry_id = Some(signed_entry_id.clone());
    tail.entry_sha256.clone_from(&entry_sha256);
    tail.segment_bytes = if segment_path.exists() {
        fs::symlink_metadata(&segment_path)?.len()
    } else {
        0
    };
    write_head(config, signer, &tail)?;
    write_current_projection(config, signer, &entry, &entry_sha256, segment, entry_index)
}

fn open_ledger_lock(root: &Path, runtime_gid: u32) -> Result<std::fs::File> {
    let path = root.join("ledger.lock");
    let existed = path.exists();
    if existed || path.is_symlink() {
        validate_history_file(&path, 0, runtime_gid, "inquiry ledger lock")?;
    }
    let mut options = OpenOptions::new();
    options.read(true).write(true).create(true).mode(0o640);
    #[cfg(target_os = "linux")]
    options.custom_flags(nix::libc::O_NOFOLLOW);
    let file = options.open(&path)?;
    if !existed {
        fs::set_permissions(&path, fs::Permissions::from_mode(0o640))?;
    }
    validate_history_file(&path, 0, runtime_gid, "inquiry ledger lock")?;
    Ok(file)
}

fn should_roll_segment(current_bytes: u64, record_bytes: u64, maximum_bytes: u64) -> bool {
    current_bytes > 0 && current_bytes.saturating_add(record_bytes) > maximum_bytes
}

fn validate_persist_input(config: &Config, input: &PersistInput<'_>) -> Result<()> {
    if input.appliance_id != config.appliance_id
        || !matches!(input.trigger_kind, "scheduled" | "evidence_integration")
        || input.recorded_at_unix_ms == 0
    {
        return Err(Error::new("inquiry persistence identity is invalid"));
    }
    for (value, label) in [
        (input.due_nonce, "inquiry due nonce"),
        (input.trigger_nonce, "inquiry trigger nonce"),
        (input.trace_id, "inquiry trace id"),
        (input.session_id, "inquiry session id"),
        (input.turn_id, "inquiry turn id"),
        (input.span_id, "inquiry span id"),
    ] {
        validate_identifier(value, label)?;
    }
    for (value, label) in [
        (input.prompt_sha256, "inquiry prompt hash"),
        (input.response_sha256, "inquiry response hash"),
        (input.reflection_sha256, "inquiry reflection hash"),
        (
            &input.inquiry.declaration_sha256,
            "inquiry declaration hash",
        ),
    ] {
        validate_hex64(value, label)?;
    }
    validate_step(&input.inquiry.step)?;
    if input.inquiry.declaration_sha256 != sha256(input.inquiry.declaration.as_bytes())
        || !input.inquiry.declaration.starts_with("INQUIRY_STEP: ")
    {
        return Err(Error::new("inquiry declaration binding is invalid"));
    }
    input.context_provenance.validate()?;
    let relative = input
        .reflection_path
        .strip_prefix(&config.workspace_root)
        .map_err(|_| Error::new("inquiry reflection escaped workspace"))?;
    if relative
        .components()
        .any(|component| !matches!(component, std::path::Component::Normal(_)))
    {
        return Err(Error::new("inquiry reflection path is non-canonical"));
    }
    let reflection = read_stable_regular(input.reflection_path, 64 * 1024)?;
    if sha256(&reflection) != input.reflection_sha256
        || input.reflection_sha256 != input.response_sha256
    {
        return Err(Error::new("inquiry reflection content binding failed"));
    }
    Ok(())
}

fn signed_entry_id(input: &PersistInput<'_>) -> String {
    recompute_entry_id(
        input.appliance_id,
        input.trigger_kind,
        input.trigger_nonce,
        input.trace_id,
        input.turn_id,
        input.response_sha256,
        &input.inquiry.declaration_sha256,
    )
}

pub(crate) fn admission_id(appliance_id: &str, signed_entry_id: &str) -> String {
    let mut preimage = ADMISSION_ID_DOMAIN.to_vec();
    preimage.extend_from_slice(appliance_id.as_bytes());
    preimage.push(0);
    preimage.extend_from_slice(signed_entry_id.as_bytes());
    format!("inquiry-admission-{}", sha256(&preimage))
}

fn step_id(appliance_id: &str, signed_entry_id: &str) -> String {
    let mut preimage = STEP_ID_DOMAIN.to_vec();
    preimage.extend_from_slice(appliance_id.as_bytes());
    preimage.push(0);
    preimage.extend_from_slice(signed_entry_id.as_bytes());
    format!("inquiry-step-{}", sha256(&preimage))
}

fn build_entry(
    config: &Config,
    signer: &HmacSigner,
    input: &PersistInput<'_>,
    tail: &ChainTail,
    signed_entry_id: &str,
    step_id: &str,
    admission_id: &str,
) -> Result<SignedEntry> {
    let inquiry_step_sha256 = sha256(&canonical_json(&input.inquiry.step)?);
    let summary = summary(&input.inquiry.step);
    let relative_reflection = input
        .reflection_path
        .strip_prefix(&config.workspace_root)
        .map_err(|_| Error::new("inquiry reflection escaped workspace"))?
        .to_string_lossy()
        .into_owned();
    let core = EntryCore {
        schema: ENTRY_SCHEMA.to_owned(),
        appliance_id: config.appliance_id.clone(),
        signed_entry_id: signed_entry_id.to_owned(),
        step_id: step_id.to_owned(),
        admission_id: admission_id.to_owned(),
        recorded_at_unix_ms: input.recorded_at_unix_ms,
        trigger_kind: input.trigger_kind.to_owned(),
        due_nonce: input.due_nonce.to_owned(),
        trigger_nonce: input.trigger_nonce.to_owned(),
        trace: TraceV1 {
            schema_version: 1,
            trace_id: input.trace_id.to_owned(),
            turn_id: input.turn_id.to_owned(),
            span_id: input.span_id.to_owned(),
            session_id: input.session_id.to_owned(),
        },
        prompt_sha256: input.prompt_sha256.to_owned(),
        response_sha256: input.response_sha256.to_owned(),
        context_provenance_sha256: input.context_provenance.digest()?,
        reflection_path: relative_reflection,
        reflection_sha256: input.reflection_sha256.to_owned(),
        declaration: input.inquiry.declaration.clone(),
        declaration_sha256: input.inquiry.declaration_sha256.clone(),
        inquiry_step: input.inquiry.step.clone(),
        inquiry_step_sha256,
        summary_sha256: sha256(summary.as_bytes()),
        summary,
        prior_entry_sha256: tail.entry_sha256.clone(),
        mechanical_predecessor: tail
            .signed_entry_id
            .clone()
            .unwrap_or_else(|| "genesis".to_owned()),
        semantic_parent_step_id: input.inquiry.step.parent_step_id.clone(),
        provenance: authored_provenance(input.trigger_kind).to_owned(),
        authority: "signed_authored_inquiry_not_hidden_chain_of_thought_not_code_authority"
            .to_owned(),
    };
    let core_bytes = canonical_json(&core)?;
    Ok(SignedEntry {
        schema: ENTRY_ENVELOPE_SCHEMA.to_owned(),
        core_sha256: sha256(&core_bytes),
        auth: Auth {
            algorithm: SIGNATURE_ALGORITHM.to_owned(),
            key_id: signer.scheduled_authorship_key_id(),
            signature: signer.sign_scheduled_authorship(&core_bytes),
        },
        core,
    })
}

pub(crate) fn summary(step: &InquiryStepV1) -> String {
    let value = format!(
        "Observed: {} Interpreted: {} Uncertain: {} Decided: {}",
        step.observation, step.interpretation, step.uncertainty, step.decision
    );
    bounded_text(&value.split_whitespace().collect::<Vec<_>>().join(" "), 320)
}

fn inquiry_root(config: &Config) -> PathBuf {
    config.inquiry_history_root.clone()
}

fn segment_name(segment: u64) -> String {
    format!("segment-{segment:020}.jsonl")
}

fn parse_segment_name(name: &str) -> Option<u64> {
    name.strip_prefix("segment-")?
        .strip_suffix(".jsonl")?
        .parse()
        .ok()
}

fn scan_chain(
    config: &Config,
    signer: &HmacSigner,
    segments: &Path,
    wanted_entry_id: Option<&str>,
) -> Result<ChainTail> {
    let mut names = fs::read_dir(segments)?
        .map(|entry| entry.map_err(Error::from))
        .collect::<Result<Vec<_>>>()?;
    names.sort_by_key(std::fs::DirEntry::file_name);
    let mut tail = ChainTail {
        entry_count: 0,
        segment: 1,
        entry_index: 0,
        signed_entry_id: None,
        entry_sha256: GENESIS_HASH.to_owned(),
        segment_bytes: 0,
        existing_entry: None,
    };
    for entry in names {
        let name = entry
            .file_name()
            .into_string()
            .map_err(|_| Error::new("inquiry segment name is not UTF-8"))?;
        let segment = parse_segment_name(&name)
            .ok_or_else(|| Error::new("inquiry segment directory has an unknown entry"))?;
        let expected_segment = if tail.entry_count == 0 {
            1
        } else {
            tail.segment.saturating_add(1)
        };
        if segment != expected_segment {
            return Err(Error::new("inquiry segment sequence has a gap"));
        }
        let path = entry.path();
        validate_history_file(
            &path,
            SEGMENT_BYTES,
            config.workspace_gid,
            "inquiry segment",
        )?;
        let bytes = read_stable_regular(&path, SEGMENT_BYTES)?;
        if !bytes.is_empty() && !bytes.ends_with(b"\n") {
            return Err(Error::new("inquiry segment has a torn tail"));
        }
        let mut index = 0_u64;
        for line in bytes
            .split(|byte| *byte == b'\n')
            .filter(|line| !line.is_empty())
        {
            index = index.saturating_add(1);
            let ledger_entry: SignedEntry = serde_json::from_slice(line)?;
            verify_entry(config, signer, &ledger_entry, &tail.entry_sha256)?;
            let entry_sha256 = sha256(&canonical_json(&ledger_entry)?);
            tail.entry_count = tail.entry_count.saturating_add(1);
            tail.segment = segment;
            tail.entry_index = index;
            tail.signed_entry_id = Some(ledger_entry.core.signed_entry_id.clone());
            tail.entry_sha256.clone_from(&entry_sha256);
            tail.segment_bytes = bytes.len() as u64;
            if wanted_entry_id == Some(ledger_entry.core.signed_entry_id.as_str())
                || wanted_entry_id.is_none()
            {
                tail.existing_entry = Some((ledger_entry, entry_sha256, segment, index));
            }
        }
    }
    reconcile_head(config, signer, &tail)?;
    Ok(tail)
}

fn find_entry_by_id(
    config: &Config,
    signer: &HmacSigner,
    segments: &Path,
    entry_id: &str,
) -> Result<Option<(SignedEntry, String, u64, u64)>> {
    let result = scan_chain(config, signer, segments, Some(entry_id))?;
    Ok(result
        .existing_entry
        .filter(|(entry, _, _, _)| entry.core.signed_entry_id == entry_id))
}

fn verify_entry(
    config: &Config,
    signer: &HmacSigner,
    ledger_entry: &SignedEntry,
    expected_prior: &str,
) -> Result<()> {
    let core_bytes = canonical_json(&ledger_entry.core)?;
    validate_step(&ledger_entry.core.inquiry_step)?;
    if ledger_entry.schema != ENTRY_ENVELOPE_SCHEMA
        || ledger_entry.core.schema != ENTRY_SCHEMA
        || ledger_entry.core.appliance_id != config.appliance_id
        || ledger_entry.core.prior_entry_sha256 != expected_prior
        || ledger_entry.core.core_bindings_invalid()
        || ledger_entry.core_sha256 != sha256(&core_bytes)
        || ledger_entry.auth.algorithm != SIGNATURE_ALGORITHM
        || ledger_entry.auth.key_id != signer.scheduled_authorship_key_id()
        || !signer.verify_scheduled_authorship(&core_bytes, &ledger_entry.auth.signature)
    {
        return Err(Error::new("signed inquiry ledger verification failed"));
    }
    Ok(())
}

impl EntryCore {
    fn core_bindings_invalid(&self) -> bool {
        validate_identifier(&self.signed_entry_id, "signed inquiry entry id").is_err()
            || validate_identifier(&self.step_id, "inquiry step id").is_err()
            || validate_identifier(&self.admission_id, "inquiry admission id").is_err()
            || validate_hex64(&self.prompt_sha256, "inquiry prompt hash").is_err()
            || validate_hex64(&self.response_sha256, "inquiry response hash").is_err()
            || validate_hex64(
                &self.context_provenance_sha256,
                "inquiry context provenance hash",
            )
            .is_err()
            || validate_hex64(&self.reflection_sha256, "inquiry reflection hash").is_err()
            || validate_hex64(&self.declaration_sha256, "inquiry declaration hash").is_err()
            || validate_hex64(&self.inquiry_step_sha256, "inquiry step hash").is_err()
            || validate_hex64(&self.summary_sha256, "inquiry summary hash").is_err()
            || validate_hex64(&self.prior_entry_sha256, "inquiry prior hash").is_err()
            || self.declaration_sha256 != sha256(self.declaration.as_bytes())
            || self.inquiry_step_sha256
                != canonical_json(&self.inquiry_step)
                    .map(|bytes| sha256(&bytes))
                    .unwrap_or_default()
            || self.summary_sha256 != sha256(self.summary.as_bytes())
            || self.summary != summary(&self.inquiry_step)
            || self.semantic_parent_step_id != self.inquiry_step.parent_step_id
            || self.provenance != authored_provenance(&self.trigger_kind)
            || self.authority
                != "signed_authored_inquiry_not_hidden_chain_of_thought_not_code_authority"
            || !matches!(
                self.trigger_kind.as_str(),
                "scheduled" | "evidence_integration"
            )
            || self.signed_entry_id
                != recompute_entry_id(
                    &self.appliance_id,
                    &self.trigger_kind,
                    &self.trigger_nonce,
                    &self.trace.trace_id,
                    &self.trace.turn_id,
                    &self.response_sha256,
                    &self.declaration_sha256,
                )
            || self.admission_id != admission_id(&self.appliance_id, &self.signed_entry_id)
            || self.step_id != step_id(&self.appliance_id, &self.signed_entry_id)
    }
}

fn recompute_entry_id(
    appliance_id: &str,
    trigger_kind: &str,
    trigger_nonce: &str,
    trace_id: &str,
    turn_id: &str,
    response_sha256: &str,
    declaration_sha256: &str,
) -> String {
    let mut preimage = ENTRY_ID_DOMAIN.to_vec();
    let values = [
        appliance_id,
        trigger_kind,
        trigger_nonce,
        trace_id,
        turn_id,
        response_sha256,
        declaration_sha256,
    ];
    for (index, value) in values.iter().enumerate() {
        preimage.extend_from_slice(value.as_bytes());
        if index < values.len().saturating_sub(1) {
            preimage.push(0);
        }
    }
    format!("inquiry-entry-{}", sha256(&preimage))
}

fn tail_for_existing(
    entry: &SignedEntry,
    _entry_sha256: &str,
    segment: u64,
    entry_index: u64,
) -> ChainTail {
    ChainTail {
        entry_count: entry_index.saturating_sub(1),
        segment,
        entry_index: entry_index.saturating_sub(1),
        signed_entry_id: (entry.core.mechanical_predecessor != "genesis")
            .then(|| entry.core.mechanical_predecessor.clone()),
        entry_sha256: entry.core.prior_entry_sha256.clone(),
        segment_bytes: 0,
        existing_entry: None,
    }
}

fn write_head(config: &Config, signer: &HmacSigner, tail: &ChainTail) -> Result<()> {
    let signed_entry_id = tail
        .signed_entry_id
        .clone()
        .ok_or_else(|| Error::new("cannot write an empty inquiry head"))?;
    let core = HeadCore {
        schema: HEAD_SCHEMA.to_owned(),
        appliance_id: config.appliance_id.clone(),
        entry_count: tail.entry_count,
        segment: tail.segment,
        entry_index: tail.entry_index,
        signed_entry_id,
        entry_sha256: tail.entry_sha256.clone(),
        segment_bytes: tail.segment_bytes,
    };
    let core_bytes = canonical_json(&core)?;
    let head = SignedHead {
        schema: HEAD_ENVELOPE_SCHEMA.to_owned(),
        core_sha256: sha256(&core_bytes),
        auth: Auth {
            algorithm: SIGNATURE_ALGORITHM.to_owned(),
            key_id: signer.scheduled_authorship_key_id(),
            signature: signer.sign_scheduled_authorship(&core_bytes),
        },
        core,
    };
    atomic_history_write(
        &inquiry_root(config).join("head.json"),
        &canonical_json(&head)?,
        config.workspace_gid,
    )
}

fn reconcile_head(config: &Config, signer: &HmacSigner, tail: &ChainTail) -> Result<()> {
    let path = inquiry_root(config).join("head.json");
    if tail.entry_count == 0 {
        if path.exists() || path.is_symlink() {
            return Err(Error::new("inquiry head exists without ledger entries"));
        }
        return Ok(());
    }
    if !path.exists() {
        if path.is_symlink() {
            return Err(Error::new("inquiry head is a broken symlink"));
        }
        return write_head(config, signer, tail);
    }
    validate_history_file(&path, MAX_HEAD_BYTES, config.workspace_gid, "inquiry head")?;
    let bytes = read_stable_regular(&path, MAX_HEAD_BYTES)?;
    let head: SignedHead = serde_json::from_slice(&bytes)?;
    let core_bytes = canonical_json(&head.core)?;
    if head.schema != HEAD_ENVELOPE_SCHEMA
        || head.core.schema != HEAD_SCHEMA
        || head.core.appliance_id != config.appliance_id
        || head.core_sha256 != sha256(&core_bytes)
        || head.auth.algorithm != SIGNATURE_ALGORITHM
        || head.auth.key_id != signer.scheduled_authorship_key_id()
        || !signer.verify_scheduled_authorship(&core_bytes, &head.auth.signature)
    {
        return Err(Error::new("inquiry head authentication failed"));
    }
    let exact_tail = head.core.entry_count == tail.entry_count
        && head.core.segment == tail.segment
        && head.core.entry_index == tail.entry_index
        && Some(head.core.signed_entry_id.as_str()) == tail.signed_entry_id.as_deref()
        && head.core.entry_sha256 == tail.entry_sha256
        && head.core.segment_bytes == tail.segment_bytes;
    if exact_tail {
        return Ok(());
    }
    // A fully fsynced entry may precede the atomic head update after a crash.
    // Only a strict, authenticated prefix head may be advanced automatically.
    if head.core.entry_count < tail.entry_count
        && head.core.segment <= tail.segment
        && find_entry_by_hash(
            config,
            &inquiry_root(config).join("segments"),
            &head.core.entry_sha256,
        )?
    {
        return write_head(config, signer, tail);
    }
    Err(Error::new("inquiry head does not identify a ledger prefix"))
}

fn find_entry_by_hash(config: &Config, segments: &Path, wanted: &str) -> Result<bool> {
    validate_hex64(wanted, "inquiry prefix hash")?;
    for entry in fs::read_dir(segments)? {
        let path = entry?.path();
        validate_history_file(
            &path,
            SEGMENT_BYTES,
            config.workspace_gid,
            "inquiry segment",
        )?;
        let bytes = read_stable_regular(&path, SEGMENT_BYTES)?;
        for line in bytes
            .split(|byte| *byte == b'\n')
            .filter(|line| !line.is_empty())
        {
            let signed: SignedEntry = serde_json::from_slice(line)?;
            if signed.core.appliance_id != config.appliance_id {
                return Err(Error::new("cross-appliance inquiry entry rejected"));
            }
            if sha256(&canonical_json(&signed)?) == wanted {
                return Ok(true);
            }
        }
    }
    Ok(false)
}

fn write_current_projection(
    config: &Config,
    signer: &HmacSigner,
    entry: &SignedEntry,
    entry_sha256: &str,
    segment: u64,
    entry_index: u64,
) -> Result<ProjectionReceipt> {
    let core = InquiryCurrentCore {
        schema: CURRENT_SCHEMA,
        appliance_id: config.appliance_id.clone(),
        signed_entry_id: entry.core.signed_entry_id.clone(),
        step_id: entry.core.step_id.clone(),
        admission_id: entry.core.admission_id.clone(),
        recorded_at_unix_ms: entry.core.recorded_at_unix_ms,
        summary: entry.core.summary.clone(),
        summary_sha256: entry.core.summary_sha256.clone(),
        inquiry_step: entry.core.inquiry_step.clone(),
        inquiry_step_sha256: entry.core.inquiry_step_sha256.clone(),
        declaration_sha256: entry.core.declaration_sha256.clone(),
        response_sha256: entry.core.response_sha256.clone(),
        trace: entry.core.trace.clone(),
        trigger_kind: entry.core.trigger_kind.clone(),
        due_nonce: entry.core.due_nonce.clone(),
        trigger_nonce: entry.core.trigger_nonce.clone(),
        reflection_path: entry.core.reflection_path.clone(),
        reflection_sha256: entry.core.reflection_sha256.clone(),
        ledger: LedgerProjection {
            segment,
            entry_index,
            prior_entry_sha256: entry.core.prior_entry_sha256.clone(),
            entry_sha256: entry_sha256.to_owned(),
            key_id: entry.auth.key_id.clone(),
            signature_algorithm: entry.auth.algorithm.clone(),
            signature: entry.auth.signature.clone(),
        },
        provenance: authored_provenance(&entry.core.trigger_kind),
        authority: "immutable_steward_signed_bounded_inquiry_projection_observational_only",
    };
    let core_bytes = canonical_json(&core)?;
    let projection = InquiryCurrentProjection {
        core_sha256: sha256(&core_bytes),
        auth: Auth {
            algorithm: SIGNATURE_ALGORITHM.to_owned(),
            key_id: signer.scheduled_authorship_key_id(),
            signature: signer.sign_scheduled_authorship(&core_bytes),
        },
        core,
    };
    let bytes = canonical_json(&projection)?;
    if bytes.len() as u64 > MAX_CURRENT_BYTES {
        return Err(Error::new("inquiry current projection exceeds its bound"));
    }
    workspace_write(
        config,
        &config
            .workspace_root
            .join("runtime/scheduled-introspection/projection/inquiry-current.json"),
        &bytes,
    )?;
    Ok(ProjectionReceipt {
        sha256: sha256(&bytes),
        bytes,
        signed_entry_id: entry.core.signed_entry_id.clone(),
        step_id: entry.core.step_id.clone(),
        admission_id: entry.core.admission_id.clone(),
    })
}

fn ensure_history_dir(path: &Path, runtime_gid: u32) -> Result<()> {
    require_absolute_no_symlink(path, "inquiry history directory")?;
    let existed = path.exists();
    if !existed {
        fs::create_dir_all(path)?;
        fs::set_permissions(path, fs::Permissions::from_mode(0o750))?;
    }
    let metadata = fs::symlink_metadata(path)?;
    if !metadata.is_dir()
        || metadata.file_type().is_symlink()
        || metadata.uid() != nix::unistd::geteuid().as_raw()
        || metadata.gid() != runtime_gid
        || metadata.permissions().mode() & 0o777 != 0o750
    {
        return Err(Error::new("inquiry history directory identity is unsafe"));
    }
    Ok(())
}

fn validate_history_file(path: &Path, maximum: u64, runtime_gid: u32, label: &str) -> Result<()> {
    let metadata = fs::symlink_metadata(path)?;
    if !metadata.is_file()
        || metadata.file_type().is_symlink()
        || metadata.nlink() != 1
        || metadata.uid() != nix::unistd::geteuid().as_raw()
        || metadata.gid() != runtime_gid
        || metadata.permissions().mode() & 0o777 != 0o640
        || metadata.len() > maximum
    {
        return Err(Error::new(format!("{label} identity is unsafe")));
    }
    Ok(())
}

fn append_history(path: &Path, bytes: &[u8], runtime_gid: u32) -> Result<()> {
    let parent = path
        .parent()
        .ok_or_else(|| Error::new("inquiry segment has no parent"))?;
    ensure_history_dir(parent, runtime_gid)?;
    let existed = path.exists();
    if let Ok(metadata) = fs::symlink_metadata(path) {
        validate_history_file(path, SEGMENT_BYTES, runtime_gid, "inquiry segment")?;
        if metadata.len().saturating_add(bytes.len() as u64) > SEGMENT_BYTES {
            return Err(Error::new("inquiry segment exceeds its bound"));
        }
    }
    let mut options = OpenOptions::new();
    options
        .read(true)
        .write(true)
        .append(true)
        .create(true)
        .mode(0o640);
    #[cfg(target_os = "linux")]
    options.custom_flags(nix::libc::O_NOFOLLOW);
    let mut file = options.open(path)?;
    if !existed {
        fs::set_permissions(path, fs::Permissions::from_mode(0o640))?;
    }
    file.lock_exclusive()?;
    validate_history_file(path, SEGMENT_BYTES, runtime_gid, "inquiry segment")?;
    let opened = file.metadata()?;
    let named_before = fs::symlink_metadata(path)?;
    if (opened.dev(), opened.ino()) != (named_before.dev(), named_before.ino()) {
        return Err(Error::new("inquiry segment path changed before append"));
    }
    if opened.len().saturating_add(bytes.len() as u64) > SEGMENT_BYTES {
        return Err(Error::new("inquiry segment exceeds its bound"));
    }
    file.write_all(bytes)?;
    file.sync_all()?;
    let named = fs::symlink_metadata(path)?;
    if (opened.dev(), opened.ino()) != (named.dev(), named.ino()) {
        return Err(Error::new("inquiry segment path changed during append"));
    }
    std::fs::File::open(parent)?.sync_all()?;
    Ok(())
}

fn atomic_history_write(path: &Path, bytes: &[u8], runtime_gid: u32) -> Result<()> {
    let parent = path
        .parent()
        .ok_or_else(|| Error::new("inquiry history output has no parent"))?;
    ensure_history_dir(parent, runtime_gid)?;
    if path.exists() || path.is_symlink() {
        validate_history_file(path, MAX_HEAD_BYTES, runtime_gid, "inquiry head")?;
    }
    let temporary = parent.join(format!(
        ".{}.{}.tmp",
        path.file_name().unwrap_or_default().to_string_lossy(),
        uuid::Uuid::new_v4()
    ));
    let mut options = OpenOptions::new();
    options.write(true).create_new(true).mode(0o640);
    #[cfg(target_os = "linux")]
    options.custom_flags(nix::libc::O_NOFOLLOW);
    let mut file = options.open(&temporary)?;
    fs::set_permissions(&temporary, fs::Permissions::from_mode(0o640))?;
    validate_history_file(
        &temporary,
        MAX_HEAD_BYTES,
        runtime_gid,
        "temporary inquiry head",
    )?;
    file.write_all(bytes)?;
    file.sync_all()?;
    fs::rename(&temporary, path)?;
    validate_history_file(path, MAX_HEAD_BYTES, runtime_gid, "inquiry head")?;
    std::fs::File::open(parent)?.sync_all()?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::os::unix::fs::{PermissionsExt as _, symlink};

    use super::{
        Confidence, InquiryClassification, STEP_SCHEMA, ThreadOperation, classify, summary,
    };

    fn valid_json() -> String {
        serde_json::json!({
            "schema": STEP_SCHEMA,
            "thread_operation": "open",
            "thread_id": "thread-curiosity",
            "parent_step_id": null,
            "observation": "The evidence contains a recurring latency boundary.",
            "interpretation": "The boundary may shape which inquiries finish.",
            "uncertainty": "The sample remains small.",
            "decision": "Collect another independent measurement.",
            "counterpoint": "The apparent boundary may be scheduling alias.",
            "next_test": "Compare three warm generations.",
            "evidence_ids": ["study-latency-1"],
            "confidence": "tentative",
            "belief_operation": "propose",
            "belief_id": "belief-latency-shaping",
            "belief_claim": "Latency shapes feasible inquiry depth."
        })
        .to_string()
    }

    fn response(source: &str) -> String {
        format!(
            "I notice a constraint without treating it as proof.\nINQUIRY_STEP: {}\nSOURCE_REVIEW: {source}",
            valid_json()
        )
    }

    fn assert_unstructured(value: &InquiryClassification) {
        assert_eq!(value.status, "model_authored_unstructured");
        assert!(value.structured.is_none());
        assert!(value.failure_class.is_some());
    }

    #[test]
    fn exact_terminal_parser_accepts_only_the_final_two_lines() {
        let parsed = classify(&response("NONE"));
        assert!(parsed.is_structured());
        let inquiry = parsed.structured().unwrap();
        assert_eq!(inquiry.step.thread_operation, ThreadOperation::Open);
        assert_eq!(inquiry.step.confidence, Confidence::Tentative);
        assert!(!inquiry.source_review.requested());
        assert!(classify(&format!("{}\n", response("REQUEST"))).is_structured());
        assert_unstructured(&classify(&format!("{}\n\n", response("NONE"))));
        assert_unstructured(&classify(&format!("{}\ntrailing", response("NONE"))));
        assert_unstructured(&classify(&format!(
            "INQUIRY_STEP: {}\n{}",
            valid_json(),
            response("NONE")
        )));
    }

    #[test]
    fn malformed_unknown_partial_and_repaired_shapes_remain_unstructured() {
        let mut unknown: serde_json::Value = serde_json::from_str(&valid_json()).unwrap();
        unknown["extra"] = serde_json::json!(true);
        assert_unstructured(&classify(&format!(
            "prose\nINQUIRY_STEP: {unknown}\nSOURCE_REVIEW: NONE"
        )));
        assert_unstructured(&classify("prose only"));
        assert_unstructured(&classify(&format!(
            "prose\nINQUIRY_STEP: {}\nSOURCE_REVIEW: MAYBE",
            valid_json()
        )));
        assert_unstructured(&classify(&format!("prose\nINQUIRY_STEP: {}", valid_json())));
    }

    #[test]
    fn field_bounds_parentage_evidence_and_beliefs_are_strict() {
        let mut value: serde_json::Value = serde_json::from_str(&valid_json()).unwrap();
        value["observation"] = serde_json::json!("x".repeat(481));
        assert_unstructured(&classify(&format!(
            "prose\nINQUIRY_STEP: {value}\nSOURCE_REVIEW: NONE"
        )));
        let mut value: serde_json::Value = serde_json::from_str(&valid_json()).unwrap();
        value["parent_step_id"] = serde_json::json!("unexpected-parent");
        assert_unstructured(&classify(&format!(
            "prose\nINQUIRY_STEP: {value}\nSOURCE_REVIEW: NONE"
        )));
        let mut value: serde_json::Value = serde_json::from_str(&valid_json()).unwrap();
        value["evidence_ids"] = serde_json::json!(["same", "same"]);
        assert_unstructured(&classify(&format!(
            "prose\nINQUIRY_STEP: {value}\nSOURCE_REVIEW: NONE"
        )));
        let mut value: serde_json::Value = serde_json::from_str(&valid_json()).unwrap();
        value["belief_claim"] = serde_json::Value::Null;
        assert_unstructured(&classify(&format!(
            "prose\nINQUIRY_STEP: {value}\nSOURCE_REVIEW: NONE"
        )));
    }

    #[test]
    fn summary_is_deterministic_and_bounded() {
        let parsed = classify(&response("NONE"));
        let value = summary(&parsed.structured().unwrap().step);
        assert!(value.chars().count() <= 320);
        assert_eq!(
            value,
            value.split_whitespace().collect::<Vec<_>>().join(" ")
        );
    }

    #[test]
    fn history_identity_validator_rejects_links() {
        let temporary = tempfile::tempdir_in(std::env::current_dir().unwrap()).unwrap();
        let original = temporary.path().join("original");
        fs::write(&original, b"{}").unwrap();
        fs::set_permissions(&original, fs::Permissions::from_mode(0o640)).unwrap();
        let runtime_gid = nix::unistd::getegid().as_raw();
        let hardlink = temporary.path().join("hardlink");
        fs::hard_link(&original, &hardlink).unwrap();
        assert!(super::validate_history_file(&original, 32, runtime_gid, "test").is_err());
        fs::remove_file(&hardlink).unwrap();
        let link = temporary.path().join("link");
        symlink(&original, &link).unwrap();
        assert!(super::validate_history_file(&link, 32, runtime_gid, "test").is_err());
    }

    #[test]
    fn segment_rollover_happens_before_the_four_mibibyte_ceiling() {
        let maximum = 4 * 1024 * 1024;
        assert!(!super::should_roll_segment(0, maximum, maximum));
        assert!(!super::should_roll_segment(maximum - 10, 10, maximum));
        assert!(super::should_roll_segment(maximum - 10, 11, maximum));
    }
}
