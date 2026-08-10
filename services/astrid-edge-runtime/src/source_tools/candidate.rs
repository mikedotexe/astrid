//! Private, append-preserving candidate drafts. This module creates intent, never authority.

use std::{
    fmt::Write as _,
    fs::{self, OpenOptions},
    os::unix::fs::OpenOptionsExt as _,
    path::{Path, PathBuf},
};

use super::{
    digest::sha256_hex,
    fs::{
        SourceIndex, atomic_private_write, ensure_private_child, ensure_private_directory,
        ensure_private_root, read_bounded_text, require_sha256, validate_component, validate_id,
        validate_label, validate_text_bytes,
    },
    source::take_characters,
    types::{
        AbandonCandidateRequest, BUILD_EVIDENCE_SCHEMA_V1, BeginCandidateRequest, BrokerError,
        BrokerResult, BuildEvidence, BuildEvidenceRequest, CANDIDATE_STATE_SCHEMA_V1,
        CANDIDATE_SUBMISSION_SCHEMA_V1, CandidateChange, CandidateInspection,
        CandidatePatchRequest, CandidateSubmissionReceipt, ExactSubmissionAttestationV1,
        FormatCandidateRequest, GenerationDiffRequest, GenerationDiffResult,
        InspectCandidateRequest, MAX_CANDIDATE_FILES, MAX_CHANGED_LINES, MAX_CHUNK_CHARS,
        MAX_CHUNK_LINES, MAX_PATCH_BYTES, NumberedLine, SUBMISSION_ATTESTATION_SCHEMA_V1,
        SubmitCandidateRequest,
    },
};

const ACTIVE_NONE: &str = "candidate_id=none\n";
const STATE_FILE_BYTES: u64 = 65_536;
const EVIDENCE_FILE_BYTES: u64 = 16_384;
const INTENT_AUTHORITY: &str = "intent_only_no_build_install_activation_or_execution_authority";

#[derive(Debug, Clone, Eq, PartialEq)]
struct CandidateState {
    candidate_id: String,
    source_id: String,
    source_manifest_sha256: String,
    base_generation: String,
    proposal_sha256: String,
    revision: u64,
    status: String,
    changes: Vec<CandidateChange>,
    digest: String,
}

impl CandidateState {
    fn new(source: &SourceIndex, request: &BeginCandidateRequest) -> BrokerResult<Self> {
        validate_id(&request.candidate_id, "candidate id")?;
        validate_label(&request.base_generation, "base generation")?;
        require_sha256(&request.proposal_sha256, "proposal digest")?;
        let mut state = Self {
            candidate_id: request.candidate_id.clone(),
            source_id: source.source_id.clone(),
            source_manifest_sha256: source.manifest_sha256.clone(),
            base_generation: request.base_generation.clone(),
            proposal_sha256: request.proposal_sha256.clone(),
            revision: 0,
            status: "draft".to_string(),
            changes: Vec::new(),
            digest: String::new(),
        };
        state.refresh_digest();
        Ok(state)
    }

    fn canonical_body(&self) -> String {
        let mut output = format!(
            "schema={CANDIDATE_STATE_SCHEMA_V1}\ncandidate_id={}\nsource_id={}\nsource_manifest_sha256={}\nbase_generation={}\nproposal_sha256={}\nrevision={}\nstatus={}\n",
            self.candidate_id,
            self.source_id,
            self.source_manifest_sha256,
            self.base_generation,
            self.proposal_sha256,
            self.revision,
            self.status
        );
        for change in &self.changes {
            writeln!(
                output,
                "change={}|{}|{}|{}|{}|{}",
                change.source_file_id,
                change.basename,
                change.old_sha256,
                change.new_sha256,
                change.changed_lines,
                u8::from(change.formatted)
            )
            .expect("writing to a String cannot fail");
        }
        output
    }

    fn refresh_digest(&mut self) {
        self.digest = sha256_hex(self.canonical_body().as_bytes());
    }

    fn serialize(&self) -> String {
        format!("{}digest={}\n", self.canonical_body(), self.digest)
    }

    fn parse(text: &str) -> BrokerResult<Self> {
        let lines = text.lines().collect::<Vec<_>>();
        if lines.len() < 9 {
            return Err(BrokerError::Integrity("candidate state is incomplete"));
        }
        exact_line(&lines, 0, "schema", CANDIDATE_STATE_SCHEMA_V1)?;
        let candidate_id = line_value(&lines, 1, "candidate_id")?.to_string();
        let source_id = line_value(&lines, 2, "source_id")?.to_string();
        let source_manifest_sha256 = line_value(&lines, 3, "source_manifest_sha256")?.to_string();
        let base_generation = line_value(&lines, 4, "base_generation")?.to_string();
        let proposal_sha256 = line_value(&lines, 5, "proposal_sha256")?.to_string();
        let revision = line_value(&lines, 6, "revision")?
            .parse::<u64>()
            .map_err(|_| BrokerError::Integrity("candidate revision is malformed"))?;
        let status = line_value(&lines, 7, "status")?.to_string();
        validate_id(&candidate_id, "candidate id")?;
        validate_id(&source_id, "source id")?;
        validate_label(&base_generation, "base generation")?;
        require_sha256(&source_manifest_sha256, "source manifest digest")?;
        require_sha256(&proposal_sha256, "proposal digest")?;
        if !matches!(
            status.as_str(),
            "draft" | "formatted" | "submitted" | "abandoned"
        ) {
            return Err(BrokerError::Integrity("candidate status is invalid"));
        }

        let digest_line_index = lines.len().saturating_sub(1);
        let digest = line_value(&lines, digest_line_index, "digest")?.to_string();
        require_sha256(&digest, "candidate digest")?;
        let mut changes = Vec::new();
        for line in &lines[8..digest_line_index] {
            let encoded = line
                .strip_prefix("change=")
                .ok_or(BrokerError::Integrity("unknown candidate state field"))?;
            let fields = encoded.split('|').collect::<Vec<_>>();
            if fields.len() != 6 {
                return Err(BrokerError::Integrity("candidate change is malformed"));
            }
            validate_id(fields[0], "source file id")?;
            validate_component(fields[1])?;
            require_sha256(fields[2], "old source digest")?;
            require_sha256(fields[3], "new source digest")?;
            let changed_lines = fields[4]
                .parse::<usize>()
                .map_err(|_| BrokerError::Integrity("changed-line count is malformed"))?;
            let formatted = match fields[5] {
                "0" => false,
                "1" => true,
                _ => return Err(BrokerError::Integrity("formatted flag is malformed")),
            };
            changes.push(CandidateChange {
                source_file_id: fields[0].to_string(),
                basename: fields[1].to_string(),
                old_sha256: fields[2].to_string(),
                new_sha256: fields[3].to_string(),
                changed_lines,
                formatted,
            });
        }
        if changes.len() > MAX_CANDIDATE_FILES
            || total_changed_lines(&changes)? > MAX_CHANGED_LINES
            || !changes
                .windows(2)
                .all(|pair| pair[0].source_file_id < pair[1].source_file_id)
        {
            return Err(BrokerError::Integrity(
                "candidate limits or ordering violated",
            ));
        }
        let state = Self {
            candidate_id,
            source_id,
            source_manifest_sha256,
            base_generation,
            proposal_sha256,
            revision,
            status,
            changes,
            digest,
        };
        if sha256_hex(state.canonical_body().as_bytes()) != state.digest {
            return Err(BrokerError::Integrity("candidate state digest mismatch"));
        }
        Ok(state)
    }

    fn inspection(&self) -> BrokerResult<CandidateInspection> {
        Ok(CandidateInspection {
            schema: CANDIDATE_STATE_SCHEMA_V1.to_string(),
            candidate_id: self.candidate_id.clone(),
            source_id: self.source_id.clone(),
            source_manifest_sha256: self.source_manifest_sha256.clone(),
            base_generation: self.base_generation.clone(),
            proposal_sha256: self.proposal_sha256.clone(),
            revision: self.revision,
            status: self.status.clone(),
            candidate_digest: self.digest.clone(),
            changed_lines: total_changed_lines(&self.changes)?,
            changes: self.changes.clone(),
            authority: INTENT_AUTHORITY.to_string(),
        })
    }
}

pub(crate) fn initialize_candidate_root(root: &Path) -> BrokerResult<()> {
    ensure_private_root(root)?;
    for child in ["drafts", "build-evidence", "submissions"] {
        ensure_private_child(root, child)?;
    }
    let active = root.join("active.state");
    if active.exists() {
        let _candidate = read_active(root)?;
    } else {
        atomic_private_write(&active, ACTIVE_NONE.as_bytes())?;
    }
    Ok(())
}

pub(crate) fn begin_candidate(
    source: &SourceIndex,
    root: &Path,
    request: &BeginCandidateRequest,
) -> BrokerResult<CandidateInspection> {
    let _guard = OperationLock::acquire(root)?;
    if read_active(root)?.is_some() {
        return Err(BrokerError::Conflict(
            "one candidate draft is already active",
        ));
    }
    let state = CandidateState::new(source, request)?;
    let drafts = ensure_private_child(root, "drafts")?;
    let directory = drafts.join(&request.candidate_id);
    if directory.exists() {
        return Err(BrokerError::Conflict("candidate id already exists"));
    }
    let directory = ensure_private_child(&drafts, &request.candidate_id)?;
    ensure_private_child(&directory, "replacements")?;
    write_state(root, &state)?;
    write_active(root, Some(&request.candidate_id))?;
    state.inspection()
}

pub(crate) fn apply_candidate_patch(
    source: &SourceIndex,
    root: &Path,
    request: &CandidatePatchRequest,
) -> BrokerResult<CandidateInspection> {
    let _guard = OperationLock::acquire(root)?;
    let mut state = load_active_state(root, &request.candidate_id)?;
    require_mutable(&state)?;
    require_sha256(&request.expected_old_sha256, "expected old source digest")?;
    if request.replacement.len() > MAX_PATCH_BYTES {
        return Err(BrokerError::LimitExceeded("candidate replacement bytes"));
    }
    validate_text_bytes(request.replacement.as_bytes())?;
    let source_file = source.file(&request.source_file_id)?;
    if source_file.sha256 != request.expected_old_sha256 {
        return Err(BrokerError::Stale("candidate patch old digest mismatch"));
    }
    let old_text = source.read_file(source_file)?;
    let new_digest = sha256_hex(request.replacement.as_bytes());
    if new_digest == source_file.sha256 {
        return Err(BrokerError::Conflict("replacement is identical to source"));
    }
    let changed_lines = changed_line_count(&old_text, &request.replacement);
    let replacement = CandidateChange {
        source_file_id: source_file.source_file_id.clone(),
        basename: source_file.basename.clone(),
        old_sha256: source_file.sha256.clone(),
        new_sha256: new_digest,
        changed_lines,
        formatted: false,
    };
    match state
        .changes
        .binary_search_by(|change| change.source_file_id.cmp(&replacement.source_file_id))
    {
        Ok(index) => state.changes[index] = replacement,
        Err(index) => state.changes.insert(index, replacement),
    }
    if state.changes.len() > MAX_CANDIDATE_FILES {
        return Err(BrokerError::LimitExceeded("candidate changed-file count"));
    }
    if total_changed_lines(&state.changes)? > MAX_CHANGED_LINES {
        return Err(BrokerError::LimitExceeded("candidate changed-line count"));
    }
    atomic_private_write(
        &replacement_path(root, &state.candidate_id, &source_file.source_file_id)?,
        request.replacement.as_bytes(),
    )?;
    state.status = "draft".to_string();
    state.revision = state.revision.saturating_add(1);
    state.refresh_digest();
    write_state(root, &state)?;
    state.inspection()
}

pub(crate) fn format_candidate(
    source: &SourceIndex,
    root: &Path,
    request: &FormatCandidateRequest,
) -> BrokerResult<CandidateInspection> {
    let _guard = OperationLock::acquire(root)?;
    let mut state = load_active_state(root, &request.candidate_id)?;
    require_mutable(&state)?;
    for change in &mut state.changes {
        let source_file = source.file(&change.source_file_id)?;
        let old_text = source.read_file(source_file)?;
        let path = replacement_path(root, &state.candidate_id, &change.source_file_id)?;
        let replacement = read_bounded_text(&path, MAX_PATCH_BYTES as u64, true)?;
        if sha256_hex(replacement.as_bytes()) != change.new_sha256 {
            return Err(BrokerError::Integrity(
                "candidate replacement digest mismatch",
            ));
        }
        let formatted = normalize_text(&replacement);
        if formatted.len() > MAX_PATCH_BYTES {
            return Err(BrokerError::LimitExceeded("formatted replacement bytes"));
        }
        atomic_private_write(&path, formatted.as_bytes())?;
        change.new_sha256 = sha256_hex(formatted.as_bytes());
        change.changed_lines = changed_line_count(&old_text, &formatted);
        change.formatted = true;
    }
    if total_changed_lines(&state.changes)? > MAX_CHANGED_LINES {
        return Err(BrokerError::LimitExceeded("formatted changed-line count"));
    }
    state.status = "formatted".to_string();
    state.revision = state.revision.saturating_add(1);
    state.refresh_digest();
    write_state(root, &state)?;
    state.inspection()
}

pub(crate) fn inspect_candidate(
    source: &SourceIndex,
    root: &Path,
    request: &InspectCandidateRequest,
) -> BrokerResult<CandidateInspection> {
    let state = load_state(root, &request.candidate_id)?;
    verify_state(source, root, &state)?;
    state.inspection()
}

pub(crate) fn abandon_candidate(
    source: &SourceIndex,
    root: &Path,
    request: &AbandonCandidateRequest,
) -> BrokerResult<CandidateInspection> {
    let _guard = OperationLock::acquire(root)?;
    let mut state = load_active_state(root, &request.candidate_id)?;
    verify_state(source, root, &state)?;
    require_sha256(
        &request.expected_candidate_digest,
        "expected candidate digest",
    )?;
    if state.digest != request.expected_candidate_digest {
        return Err(BrokerError::Stale("candidate digest changed"));
    }
    state.status = "abandoned".to_string();
    state.revision = state.revision.saturating_add(1);
    state.refresh_digest();
    write_state(root, &state)?;
    write_active(root, None)?;
    state.inspection()
}

pub(crate) fn read_generation_diff(
    source: &SourceIndex,
    root: &Path,
    request: &GenerationDiffRequest,
) -> BrokerResult<GenerationDiffResult> {
    if request.start_line == 0 {
        return Err(BrokerError::InvalidInput("diff line numbers are one-based"));
    }
    if request.max_lines == 0 || request.max_lines > MAX_CHUNK_LINES {
        return Err(BrokerError::LimitExceeded("diff line count"));
    }
    let state = load_state(root, &request.candidate_id)?;
    verify_state(source, root, &state)?;
    let change = state
        .changes
        .iter()
        .find(|change| change.source_file_id == request.source_file_id)
        .ok_or(BrokerError::NotFound(
            "source file is not changed by candidate",
        ))?;
    let source_file = source.file(&change.source_file_id)?;
    let before_text = source.read_file(source_file)?;
    let after_text = read_bounded_text(
        &replacement_path(root, &state.candidate_id, &change.source_file_id)?,
        MAX_PATCH_BYTES as u64,
        true,
    )?;
    let before_lines = before_text.lines().collect::<Vec<_>>();
    let after_lines = after_text.lines().collect::<Vec<_>>();
    let start = request.start_line.saturating_sub(1);
    let maximum_len = before_lines.len().max(after_lines.len());
    if start > maximum_len {
        return Err(BrokerError::InvalidInput("diff start line is out of range"));
    }
    let end = start.saturating_add(request.max_lines).min(maximum_len);
    let (before, before_chars) = numbered_slice(&before_lines, start, end, MAX_CHUNK_CHARS);
    let after_budget = MAX_CHUNK_CHARS.saturating_sub(before_chars);
    let (after, _after_chars) = numbered_slice(&after_lines, start, end, after_budget);
    let consumed = before.len().min(after.len()).max(1);
    let next_index = start.saturating_add(consumed);
    Ok(GenerationDiffResult {
        candidate_id: state.candidate_id,
        candidate_digest: state.digest,
        source_file_id: change.source_file_id.clone(),
        basename: change.basename.clone(),
        old_sha256: change.old_sha256.clone(),
        new_sha256: change.new_sha256.clone(),
        before,
        after,
        next_line: (next_index < maximum_len).then_some(next_index.saturating_add(1)),
        authority: INTENT_AUTHORITY.to_string(),
    })
}

pub(crate) fn read_build_evidence(
    root: &Path,
    request: &BuildEvidenceRequest,
) -> BrokerResult<BuildEvidence> {
    validate_id(&request.build_id, "build id")?;
    let directory = ensure_private_child(root, "build-evidence")?;
    let path = directory.join(format!("build_{}.evidence", request.build_id));
    let text = read_bounded_text(&path, EVIDENCE_FILE_BYTES, true)?;
    parse_build_evidence(&text, &request.build_id)
}

pub(crate) fn submit_candidate(
    source: &SourceIndex,
    root: &Path,
    request: &SubmitCandidateRequest,
) -> BrokerResult<CandidateSubmissionReceipt> {
    let _guard = OperationLock::acquire(root)?;
    let mut state = load_active_state(root, &request.candidate_id)?;
    verify_state(source, root, &state)?;
    if state.status != "formatted" || state.changes.is_empty() {
        return Err(BrokerError::Conflict(
            "candidate must have formatted changes",
        ));
    }
    require_sha256(
        &request.expected_candidate_digest,
        "expected candidate digest",
    )?;
    if state.digest != request.expected_candidate_digest {
        return Err(BrokerError::Stale("candidate digest changed"));
    }
    let attestation_body = validate_attestation(&request.attestation, &state)?;
    let attestation_sha256 = sha256_hex(attestation_body.as_bytes());
    let mut submission = format!(
        "schema={CANDIDATE_SUBMISSION_SCHEMA_V1}\ncandidate_id={}\ncandidate_digest={}\nsource_id={}\nsource_manifest_sha256={}\nattestation_sha256={}\nauthority={INTENT_AUTHORITY}\n",
        state.candidate_id,
        state.digest,
        state.source_id,
        state.source_manifest_sha256,
        attestation_sha256
    );
    for change in &state.changes {
        writeln!(
            submission,
            "change={}|{}|{}|{}|{}",
            change.source_file_id,
            change.basename,
            change.old_sha256,
            change.new_sha256,
            change.changed_lines
        )
        .expect("writing to a String cannot fail");
    }
    let submission_sha256 = sha256_hex(submission.as_bytes());
    let artifact = format!(
        "submission_{}_{}.manifest",
        state.candidate_id,
        &submission_sha256[..12]
    );
    let directory = ensure_private_child(root, "submissions")?;
    let path = directory.join(&artifact);
    if path.exists() {
        let existing = read_bounded_text(&path, STATE_FILE_BYTES, true)?;
        if existing != submission {
            return Err(BrokerError::Integrity("submission artifact collision"));
        }
    } else {
        atomic_private_write(&path, submission.as_bytes())?;
    }
    state.status = "submitted".to_string();
    state.revision = state.revision.saturating_add(1);
    state.refresh_digest();
    write_state(root, &state)?;
    write_active(root, None)?;
    Ok(CandidateSubmissionReceipt {
        schema: CANDIDATE_SUBMISSION_SCHEMA_V1.to_string(),
        candidate_id: request.candidate_id.clone(),
        candidate_digest: request.expected_candidate_digest.clone(),
        submission_artifact: artifact,
        submission_sha256,
        attestation_sha256,
        authority: INTENT_AUTHORITY.to_string(),
    })
}

fn verify_state(source: &SourceIndex, root: &Path, state: &CandidateState) -> BrokerResult<()> {
    if state.source_id != source.source_id || state.source_manifest_sha256 != source.manifest_sha256
    {
        return Err(BrokerError::Stale(
            "candidate is bound to another source snapshot",
        ));
    }
    for change in &state.changes {
        let source_file = source.file(&change.source_file_id)?;
        if source_file.sha256 != change.old_sha256 || source_file.basename != change.basename {
            return Err(BrokerError::Stale("candidate source binding changed"));
        }
        let replacement = read_bounded_text(
            &replacement_path(root, &state.candidate_id, &change.source_file_id)?,
            MAX_PATCH_BYTES as u64,
            true,
        )?;
        if sha256_hex(replacement.as_bytes()) != change.new_sha256
            || changed_line_count(&source.read_file(source_file)?, &replacement)
                != change.changed_lines
        {
            return Err(BrokerError::Integrity("candidate replacement changed"));
        }
    }
    Ok(())
}

fn require_mutable(state: &CandidateState) -> BrokerResult<()> {
    if matches!(state.status.as_str(), "draft" | "formatted") {
        Ok(())
    } else {
        Err(BrokerError::Conflict("candidate is no longer mutable"))
    }
}

fn normalize_text(value: &str) -> String {
    let mut normalized = value.replace("\r\n", "\n").replace('\r', "\n");
    if !normalized.is_empty() && !normalized.ends_with('\n') {
        normalized.push('\n');
    }
    normalized
}

fn changed_line_count(before: &str, after: &str) -> usize {
    let before = before.lines().collect::<Vec<_>>();
    let after = after.lines().collect::<Vec<_>>();
    let prefix = before
        .iter()
        .zip(&after)
        .take_while(|(left, right)| left == right)
        .count();
    let before_remaining = before.len().saturating_sub(prefix);
    let after_remaining = after.len().saturating_sub(prefix);
    let suffix = before[prefix..]
        .iter()
        .rev()
        .zip(after[prefix..].iter().rev())
        .take_while(|(left, right)| left == right)
        .count()
        .min(before_remaining)
        .min(after_remaining);
    before_remaining
        .saturating_sub(suffix)
        .saturating_add(after_remaining.saturating_sub(suffix))
}

fn total_changed_lines(changes: &[CandidateChange]) -> BrokerResult<usize> {
    changes.iter().try_fold(0_usize, |total, change| {
        total
            .checked_add(change.changed_lines)
            .ok_or(BrokerError::LimitExceeded("candidate changed-line count"))
    })
}

fn candidate_directory(root: &Path, candidate_id: &str) -> BrokerResult<PathBuf> {
    validate_id(candidate_id, "candidate id")?;
    let drafts = root.join("drafts");
    ensure_private_directory(&drafts)?;
    let directory = drafts.join(candidate_id);
    ensure_private_directory(&directory)?;
    Ok(directory)
}

fn replacement_path(root: &Path, candidate_id: &str, file_id: &str) -> BrokerResult<PathBuf> {
    validate_id(file_id, "source file id")?;
    let replacements = candidate_directory(root, candidate_id)?.join("replacements");
    ensure_private_directory(&replacements)?;
    Ok(replacements.join(format!("{file_id}.replacement")))
}

fn state_path(root: &Path, candidate_id: &str) -> BrokerResult<PathBuf> {
    Ok(candidate_directory(root, candidate_id)?.join("state.v1"))
}

fn load_state(root: &Path, candidate_id: &str) -> BrokerResult<CandidateState> {
    let text = read_bounded_text(&state_path(root, candidate_id)?, STATE_FILE_BYTES, true)?;
    let state = CandidateState::parse(&text)?;
    if state.candidate_id != candidate_id {
        return Err(BrokerError::Integrity("candidate state identity mismatch"));
    }
    Ok(state)
}

fn load_active_state(root: &Path, candidate_id: &str) -> BrokerResult<CandidateState> {
    validate_id(candidate_id, "candidate id")?;
    if read_active(root)?.as_deref() != Some(candidate_id) {
        return Err(BrokerError::Conflict("candidate is not the active draft"));
    }
    load_state(root, candidate_id)
}

fn write_state(root: &Path, state: &CandidateState) -> BrokerResult<()> {
    atomic_private_write(
        &candidate_directory(root, &state.candidate_id)?.join("state.v1"),
        state.serialize().as_bytes(),
    )
}

fn read_active(root: &Path) -> BrokerResult<Option<String>> {
    let text = read_bounded_text(&root.join("active.state"), 256, true)?;
    if text == ACTIVE_NONE {
        return Ok(None);
    }
    let value = text
        .strip_prefix("candidate_id=")
        .and_then(|value| value.strip_suffix('\n'))
        .ok_or(BrokerError::Integrity(
            "active candidate pointer is malformed",
        ))?;
    validate_id(value, "candidate id")?;
    Ok(Some(value.to_string()))
}

fn write_active(root: &Path, candidate_id: Option<&str>) -> BrokerResult<()> {
    let text = if let Some(candidate_id) = candidate_id {
        validate_id(candidate_id, "candidate id")?;
        format!("candidate_id={candidate_id}\n")
    } else {
        ACTIVE_NONE.to_string()
    };
    atomic_private_write(&root.join("active.state"), text.as_bytes())
}

fn validate_attestation(
    attestation: &ExactSubmissionAttestationV1,
    state: &CandidateState,
) -> BrokerResult<String> {
    if attestation.schema != SUBMISSION_ATTESTATION_SCHEMA_V1
        || attestation.provenance != "exact_model"
    {
        return Err(BrokerError::SecurityViolation(
            "submission requires an exact model-authored attestation",
        ));
    }
    for (value, label) in [
        (&attestation.instance_id, "instance id"),
        (&attestation.trace_id, "trace id"),
        (&attestation.session_id, "session id"),
        (&attestation.turn_id, "turn id"),
    ] {
        validate_id(value, label)?;
    }
    validate_label(&attestation.model_id, "model id")?;
    for (value, label) in [
        (&attestation.response_sha256, "response digest"),
        (
            &attestation.terminal_declaration_sha256,
            "terminal declaration digest",
        ),
    ] {
        require_sha256(value, label)?;
    }
    if attestation.candidate_id != state.candidate_id
        || attestation.candidate_digest != state.digest
        || attestation.authored_at_unix_ms == 0
    {
        return Err(BrokerError::Stale(
            "submission attestation does not bind candidate",
        ));
    }
    Ok(format!(
        "schema={}\nprovenance={}\ninstance_id={}\ntrace_id={}\nsession_id={}\nturn_id={}\nresponse_sha256={}\nterminal_declaration_sha256={}\ncandidate_id={}\ncandidate_digest={}\nmodel_id={}\nauthored_at_unix_ms={}\n",
        attestation.schema,
        attestation.provenance,
        attestation.instance_id,
        attestation.trace_id,
        attestation.session_id,
        attestation.turn_id,
        attestation.response_sha256,
        attestation.terminal_declaration_sha256,
        attestation.candidate_id,
        attestation.candidate_digest,
        attestation.model_id,
        attestation.authored_at_unix_ms
    ))
}

fn parse_build_evidence(text: &str, expected_build_id: &str) -> BrokerResult<BuildEvidence> {
    let lines = text.lines().collect::<Vec<_>>();
    if lines.len() != 11 {
        return Err(BrokerError::Integrity(
            "build evidence field count is invalid",
        ));
    }
    exact_line(&lines, 0, "schema", BUILD_EVIDENCE_SCHEMA_V1)?;
    let build_id = line_value(&lines, 1, "build_id")?.to_string();
    let candidate_id = line_value(&lines, 2, "candidate_id")?.to_string();
    let candidate_digest = line_value(&lines, 3, "candidate_digest")?.to_string();
    let source_manifest_sha256 = line_value(&lines, 4, "source_manifest_sha256")?.to_string();
    let test_manifest_sha256 = line_value(&lines, 5, "test_manifest_sha256")?.to_string();
    let artifact_value = line_value(&lines, 6, "artifact_sha256")?;
    let status = line_value(&lines, 7, "status")?.to_string();
    let recorded_at_unix_ms = line_value(&lines, 8, "recorded_at_unix_ms")?
        .parse::<u64>()
        .map_err(|_| BrokerError::Integrity("build evidence timestamp is malformed"))?;
    let authority = line_value(&lines, 9, "authority")?.to_string();
    let evidence_sha256 = line_value(&lines, 10, "evidence_sha256")?.to_string();
    validate_id(&build_id, "build id")?;
    validate_id(&candidate_id, "candidate id")?;
    validate_label(&status, "build status")?;
    require_sha256(&candidate_digest, "candidate digest")?;
    require_sha256(&source_manifest_sha256, "source manifest digest")?;
    require_sha256(&test_manifest_sha256, "test manifest digest")?;
    require_sha256(&evidence_sha256, "evidence digest")?;
    if build_id != expected_build_id || recorded_at_unix_ms == 0 {
        return Err(BrokerError::Integrity("build evidence identity is invalid"));
    }
    let artifact_sha256 = if artifact_value == "none" {
        None
    } else {
        require_sha256(artifact_value, "artifact digest")?;
        Some(artifact_value.to_string())
    };
    if authority != "external_build_evidence_no_activation_authority" {
        return Err(BrokerError::SecurityViolation(
            "build evidence claims excess authority",
        ));
    }
    let body = lines[..10].join("\n") + "\n";
    if sha256_hex(body.as_bytes()) != evidence_sha256 {
        return Err(BrokerError::Integrity("build evidence digest mismatch"));
    }
    Ok(BuildEvidence {
        schema: BUILD_EVIDENCE_SCHEMA_V1.to_string(),
        build_id,
        candidate_id,
        candidate_digest,
        source_manifest_sha256,
        test_manifest_sha256,
        artifact_sha256,
        status,
        recorded_at_unix_ms,
        evidence_sha256,
        authority,
    })
}

fn numbered_slice(
    lines: &[&str],
    start: usize,
    end: usize,
    character_budget: usize,
) -> (Vec<NumberedLine>, usize) {
    if start >= lines.len() || character_budget == 0 {
        return (Vec::new(), 0);
    }
    let actual_end = end.min(lines.len());
    let mut output = Vec::new();
    let mut used = 0_usize;
    for (offset, line) in lines[start..actual_end].iter().enumerate() {
        let available = character_budget.saturating_sub(used);
        if available == 0 {
            break;
        }
        let displayed = take_characters(line, available);
        used = used.saturating_add(displayed.chars().count());
        output.push(NumberedLine {
            line_number: start.saturating_add(offset).saturating_add(1),
            text: displayed,
        });
        if line.chars().count() > available {
            break;
        }
    }
    (output, used)
}

fn line_value<'a>(lines: &'a [&str], index: usize, key: &str) -> BrokerResult<&'a str> {
    lines
        .get(index)
        .and_then(|line| line.strip_prefix(&format!("{key}=")))
        .ok_or(BrokerError::Integrity(
            "record field is missing or out of order",
        ))
}

fn exact_line(lines: &[&str], index: usize, key: &str, expected: &str) -> BrokerResult<()> {
    if line_value(lines, index, key)? == expected {
        Ok(())
    } else {
        Err(BrokerError::Integrity("record schema is unsupported"))
    }
}

struct OperationLock {
    path: PathBuf,
}

impl OperationLock {
    fn acquire(root: &Path) -> BrokerResult<Self> {
        ensure_private_directory(root)?;
        let path = root.join("operation.lock");
        OpenOptions::new()
            .write(true)
            .create_new(true)
            .mode(0o600)
            .open(&path)
            .map_err(|error| {
                if error.kind() == std::io::ErrorKind::AlreadyExists {
                    BrokerError::Conflict("candidate operation is already in progress")
                } else {
                    error.into()
                }
            })?;
        Ok(Self { path })
    }
}

impl Drop for OperationLock {
    fn drop(&mut self) {
        drop(fs::remove_file(&self.path));
    }
}
