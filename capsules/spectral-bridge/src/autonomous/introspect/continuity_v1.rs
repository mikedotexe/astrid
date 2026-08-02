use std::collections::HashSet;
use std::fs::{self, OpenOptions};
use std::io::Write as _;
#[cfg(unix)]
use std::os::unix::fs::{OpenOptionsExt as _, PermissionsExt as _};
use std::path::{Component, Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};
use sha2::{Digest as _, Sha256};

const STATE_RELATIVE: &str = "diagnostics/introspection_continuity_v1";
const INDEX_SCHEMA: &str = "introspection_continuity_index_v1";
const CARD_SCHEMA: &str = "introspection_continuity_card_v1";
const RESPONSE_SCHEMA: &str = "introspection_continuity_response_v1";
const MAX_INDEX_BYTES: u64 = 4 * 1024 * 1024;
const MAX_CARD_BYTES: u64 = 256 * 1024;
const MAX_INDEX_CARDS: usize = 10_000;
const MAX_PRIOR_CARDS: usize = 3;
const MAX_PROMPT_CHARS: usize = 24_000;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct PriorEvidenceBindingV1 {
    card_id: String,
    introspection_id: String,
    claim_ids: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(in crate::autonomous) struct PriorEvidenceContextV1 {
    prompt_context: String,
    bindings: Vec<PriorEvidenceBindingV1>,
}

impl PriorEvidenceContextV1 {
    pub(in crate::autonomous) fn prompt_context(&self) -> &str {
        &self.prompt_context
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
enum AssessmentStatusV1 {
    MechanicalOnly,
    StillFriction,
    Contradicted,
    NotAssessed,
}

impl AssessmentStatusV1 {
    const fn as_str(self) -> &'static str {
        match self {
            Self::MechanicalOnly => "mechanical_only",
            Self::StillFriction => "still_friction",
            Self::Contradicted => "contradicted",
            Self::NotAssessed => "not_assessed",
        }
    }

    fn parse(value: &str) -> Option<Self> {
        match value {
            "mechanical_only" => Some(Self::MechanicalOnly),
            "still_friction" => Some(Self::StillFriction),
            "contradicted" => Some(Self::Contradicted),
            "not_assessed" => Some(Self::NotAssessed),
            _ => None,
        }
    }
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct AuthorityStateV1 {
    schema: String,
    schema_version: u8,
    state: String,
    witness_only: bool,
}

impl AuthorityStateV1 {
    fn validate(&self, context: &str) -> Result<(), String> {
        if self.schema != "artifact_authority_state_v1"
            || self.schema_version != 1
            || self.state != "evidence_only"
            || !self.witness_only
        {
            return Err(format!("{context}: authority state is not evidence_only"));
        }
        Ok(())
    }
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct ContinuityIndexV1 {
    schema: String,
    schema_version: u8,
    cards: Vec<ContinuityIndexEntryV1>,
    card_count: usize,
    right_to_ignore: bool,
    silence_is_neutral: bool,
    mechanical_evidence_only: bool,
    artifact_authority_state_v1: AuthorityStateV1,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct ContinuityIndexEntryV1 {
    card_id: String,
    card_path: String,
    card_sha256: String,
    introspection_id: String,
    captured_at_unix: u64,
    report_sha256: String,
    stable_source_identity: String,
    source_sha256: String,
    read_session_id: String,
    #[serde(default)]
    assessment_status: Option<AssessmentStatusV1>,
    #[serde(default)]
    assessment_receipt_id: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct ContinuityCardV1 {
    schema: String,
    schema_version: u8,
    card_id: String,
    introspection_id: String,
    captured_at_unix: u64,
    canonical_report: CanonicalReportV1,
    source: ContinuitySourceV1,
    claims: Vec<ContinuityClaimV1>,
    claim_count: usize,
    remaining_gaps: Vec<RemainingGapV1>,
    authority_waits: Vec<AuthorityWaitV1>,
    #[serde(default)]
    prior_evidence_assessment: Option<PriorEvidenceAssessmentV1>,
    right_to_ignore: bool,
    silence_is_neutral: bool,
    mechanical_evidence_only: bool,
    felt_closure_inferred: bool,
    consent_inferred: bool,
    no_authority: bool,
    authority_boundary: String,
    artifact_authority_state_v1: AuthorityStateV1,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct CanonicalReportV1 {
    path: String,
    sha256: String,
    lived_state_witness_id: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct ContinuitySourceV1 {
    label: String,
    stable_identity: String,
    locator: Option<String>,
    sha256: String,
    read_session_id: String,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct ContinuityClaimV1 {
    claim_id: String,
    summary: String,
    classification: String,
    disposition: String,
    grounded_disposition: String,
    evidence_refs: Vec<EvidenceRefV1>,
    omitted_evidence_ref_count: usize,
    authority_wait: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct EvidenceRefV1 {
    kind: String,
    target: String,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct RemainingGapV1 {
    claim_id: String,
    classification: String,
    grounded_disposition: String,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct AuthorityWaitV1 {
    claim_id: String,
    classification: String,
    authority_wait: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct PriorEvidenceAssessmentV1 {
    status: AssessmentStatusV1,
    receipt_id: String,
    observed_in_introspection: CurrentIntrospectionV1,
    recorded_at_unix_ms: u64,
    response_line: usize,
    mechanical_evidence_only: bool,
    felt_closure_inferred: bool,
    consent_inferred: bool,
    no_authority: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct CurrentIntrospectionV1 {
    path: String,
    introspection_id: String,
    sha256: String,
}

#[derive(Debug, Serialize)]
struct ResponseReceiptV1 {
    schema: &'static str,
    schema_version: u8,
    receipt_id: String,
    card_id: String,
    assessment_status: AssessmentStatusV1,
    current_introspection: CurrentIntrospectionV1,
    response_line: usize,
    recorded_at_unix_ms: u64,
    bound: bool,
    linked_prior_introspection_id: Option<String>,
    linked_claim_ids: Vec<String>,
    right_to_ignore: bool,
    silence_is_neutral: bool,
    mechanical_evidence_only: bool,
    felt_closure_inferred: bool,
    consent_inferred: bool,
    no_authority: bool,
    authority_boundary: &'static str,
    artifact_authority_state_v1: ResponseAuthorityStateV1,
}

#[derive(Debug, Serialize)]
struct ResponseAuthorityStateV1 {
    schema: &'static str,
    schema_version: u8,
    state: &'static str,
    witness_only: bool,
}

pub(in crate::autonomous) fn load_prior_evidence_v1(
    workspace: &Path,
    label: &str,
    source_path: &Path,
    source_sha256: &str,
) -> Result<Option<PriorEvidenceContextV1>, String> {
    require_sha256(source_sha256, "current source SHA-256")?;
    let index_path = workspace.join(STATE_RELATIVE).join("index.json");
    if !index_path.is_file() {
        return Ok(None);
    }
    let index_raw = read_bounded_owner_file(&index_path, MAX_INDEX_BYTES)?;
    let index: ContinuityIndexV1 = serde_json::from_slice(&index_raw)
        .map_err(|error| format!("parse {}: {error}", index_path.display()))?;
    validate_index(&index)?;

    let locator = source_locator(source_path)?;
    let stable_identity = stable_source_identity(label, locator.as_deref());
    let mut matching = index
        .cards
        .iter()
        .filter(|entry| {
            entry.stable_source_identity == stable_identity && entry.source_sha256 == source_sha256
        })
        .collect::<Vec<_>>();
    matching.sort_by(|left, right| {
        right
            .captured_at_unix
            .cmp(&left.captured_at_unix)
            .then_with(|| right.card_id.cmp(&left.card_id))
    });

    let mut cards = Vec::new();
    for entry in matching.into_iter().take(MAX_PRIOR_CARDS) {
        cards.push(load_card(
            workspace,
            entry,
            label,
            locator.as_deref(),
            &stable_identity,
            source_sha256,
        )?);
    }
    if cards.is_empty() {
        return Ok(None);
    }
    Ok(Some(render_context(cards)))
}

fn validate_index(index: &ContinuityIndexV1) -> Result<(), String> {
    if index.schema != INDEX_SCHEMA || index.schema_version != 1 {
        return Err("continuity index schema/version mismatch".to_string());
    }
    if index.card_count != index.cards.len() || index.cards.len() > MAX_INDEX_CARDS {
        return Err("continuity index cardinality is invalid".to_string());
    }
    if !index.right_to_ignore || !index.silence_is_neutral || !index.mechanical_evidence_only {
        return Err("continuity index lost its right-to-ignore boundary".to_string());
    }
    index
        .artifact_authority_state_v1
        .validate("continuity index")?;
    let mut card_ids = HashSet::new();
    for entry in &index.cards {
        require_prefixed_sha256(&entry.card_id, "icv1_", "card_id")?;
        require_sha256(&entry.card_sha256, "card_sha256")?;
        require_sha256(&entry.report_sha256, "report_sha256")?;
        require_sha256(&entry.source_sha256, "source_sha256")?;
        require_sha256(&entry.read_session_id, "read_session_id")?;
        require_prefixed_sha256(
            &entry.stable_source_identity,
            "source_",
            "stable_source_identity",
        )?;
        safe_relative(&entry.card_path, Some(&format!("{STATE_RELATIVE}/cards")))?;
        if entry.introspection_id.is_empty() || entry.captured_at_unix == 0 {
            return Err("continuity index entry has an empty identity or timestamp".to_string());
        }
        if !card_ids.insert(entry.card_id.as_str()) {
            return Err(format!("duplicate continuity card id {}", entry.card_id));
        }
        if entry.assessment_status.is_some() != entry.assessment_receipt_id.is_some() {
            return Err(format!(
                "continuity index assessment binding is incomplete for {}",
                entry.card_id
            ));
        }
        if let Some(receipt_id) = entry.assessment_receipt_id.as_deref() {
            require_prefixed_sha256(receipt_id, "icrv1_", "assessment_receipt_id")?;
        }
    }
    Ok(())
}

fn load_card(
    workspace: &Path,
    entry: &ContinuityIndexEntryV1,
    label: &str,
    locator: Option<&str>,
    stable_identity: &str,
    source_sha256: &str,
) -> Result<ContinuityCardV1, String> {
    let relative = safe_relative(&entry.card_path, Some(&format!("{STATE_RELATIVE}/cards")))?;
    let card_path = workspace.join(relative);
    let cards_root = workspace.join(STATE_RELATIVE).join("cards");
    let resolved_root = cards_root
        .canonicalize()
        .map_err(|error| format!("resolve {}: {error}", cards_root.display()))?;
    let resolved_card = card_path
        .canonicalize()
        .map_err(|error| format!("resolve {}: {error}", card_path.display()))?;
    if !resolved_card.starts_with(&resolved_root) {
        return Err(format!(
            "continuity card escapes card root: {}",
            entry.card_path
        ));
    }
    let raw = read_bounded_owner_file(&resolved_card, MAX_CARD_BYTES)?;
    if sha256_bytes(&raw) != entry.card_sha256 {
        return Err(format!("continuity card hash mismatch: {}", entry.card_id));
    }
    let card: ContinuityCardV1 = serde_json::from_slice(&raw)
        .map_err(|error| format!("parse {}: {error}", resolved_card.display()))?;
    validate_card(&card, entry, label, locator, stable_identity, source_sha256)?;
    Ok(card)
}

fn validate_card(
    card: &ContinuityCardV1,
    entry: &ContinuityIndexEntryV1,
    label: &str,
    locator: Option<&str>,
    stable_identity: &str,
    source_sha256: &str,
) -> Result<(), String> {
    if card.schema != CARD_SCHEMA || card.schema_version != 1 {
        return Err(format!(
            "continuity card schema mismatch: {}",
            entry.card_id
        ));
    }
    if card.card_id != entry.card_id
        || card.introspection_id != entry.introspection_id
        || card.captured_at_unix != entry.captured_at_unix
        || card.canonical_report.sha256 != entry.report_sha256
        || card.source.stable_identity != *stable_identity
        || card.source.sha256 != source_sha256
        || card.source.read_session_id != entry.read_session_id
        || card.source.label != label
        || card.source.locator.as_deref() != locator
    {
        return Err(format!(
            "continuity card binding mismatch: {}",
            entry.card_id
        ));
    }
    safe_relative(&card.canonical_report.path, Some("introspections"))?;
    require_sha256(&card.canonical_report.sha256, "canonical report sha256")?;
    if let Some(witness_id) = card.canonical_report.lived_state_witness_id.as_deref()
        && witness_id.len() > 160
    {
        return Err(format!(
            "continuity witness id is oversized: {}",
            entry.card_id
        ));
    }
    if card.claim_count != card.claims.len() || card.claims.len() > 24 {
        return Err(format!(
            "continuity claim count mismatch: {}",
            entry.card_id
        ));
    }
    let mut claim_ids = HashSet::new();
    for claim in &card.claims {
        if claim.claim_id.is_empty()
            || claim.claim_id.len() > 120
            || claim.summary.len() > 2_000
            || claim.classification.len() > 240
            || claim.disposition.len() > 1_000
            || claim.grounded_disposition.len() > 4_000
            || !claim_ids.insert(claim.claim_id.as_str())
        {
            return Err(format!("invalid continuity claim in {}", entry.card_id));
        }
        for evidence in &claim.evidence_refs {
            if evidence.kind.is_empty() || evidence.kind.len() > 80 {
                return Err(format!("invalid evidence kind in {}", entry.card_id));
            }
            safe_relative(&evidence.target, None)?;
        }
        if claim
            .authority_wait
            .as_ref()
            .is_some_and(|value| value.len() > 1_000)
        {
            return Err(format!("oversized authority wait in {}", entry.card_id));
        }
        let _ = claim.omitted_evidence_ref_count;
    }
    for gap in &card.remaining_gaps {
        if !claim_ids.contains(gap.claim_id.as_str())
            || gap.classification.is_empty()
            || gap.grounded_disposition.is_empty()
        {
            return Err(format!("invalid remaining gap in {}", entry.card_id));
        }
    }
    for wait in &card.authority_waits {
        if !claim_ids.contains(wait.claim_id.as_str()) || wait.classification.is_empty() {
            return Err(format!("invalid authority wait in {}", entry.card_id));
        }
        let _ = wait.authority_wait.as_deref();
    }
    if !card.right_to_ignore
        || !card.silence_is_neutral
        || !card.mechanical_evidence_only
        || card.felt_closure_inferred
        || card.consent_inferred
        || !card.no_authority
        || card.authority_boundary
            != "mechanical_evidence_not_felt_closure_control_approval_or_activation"
    {
        return Err(format!(
            "continuity authority boundary mismatch: {}",
            entry.card_id
        ));
    }
    card.artifact_authority_state_v1
        .validate("continuity card")?;
    if let Some(assessment) = card.prior_evidence_assessment.as_ref() {
        require_prefixed_sha256(&assessment.receipt_id, "icrv1_", "receipt_id")?;
        safe_relative(
            &assessment.observed_in_introspection.path,
            Some("introspections"),
        )?;
        require_sha256(
            &assessment.observed_in_introspection.sha256,
            "assessment introspection sha256",
        )?;
        if assessment
            .observed_in_introspection
            .introspection_id
            .is_empty()
            || assessment.recorded_at_unix_ms == 0
            || assessment.response_line == 0
            || !assessment.mechanical_evidence_only
            || assessment.felt_closure_inferred
            || assessment.consent_inferred
            || !assessment.no_authority
        {
            return Err(format!("invalid prior assessment in {}", entry.card_id));
        }
        if entry.assessment_status != Some(assessment.status)
            || entry.assessment_receipt_id.as_deref() != Some(assessment.receipt_id.as_str())
        {
            return Err(format!("assessment index mismatch in {}", entry.card_id));
        }
    } else if entry.assessment_status.is_some() || entry.assessment_receipt_id.is_some() {
        return Err(format!("missing indexed assessment in {}", entry.card_id));
    }
    Ok(())
}

fn render_context(cards: Vec<ContinuityCardV1>) -> PriorEvidenceContextV1 {
    let mut lines = vec![
        "--- Prior Steward Evidence V1 (separate mechanical evidence lane) ---".to_string(),
        "These cards report bounded prior steward dispositions for this exact source hash. They are not source bytes, felt closure, consent, approval, activation, or control authority.".to_string(),
        "Right to ignore: yes. Silence is neutral. A later felt report or contradiction outranks mechanical continuity.".to_string(),
    ];
    let mut bindings = Vec::new();
    for card in cards {
        let assessment = card
            .prior_evidence_assessment
            .as_ref()
            .map_or("not_assessed", |value| value.status.as_str());
        lines.push(format!(
            "Card {} | prior introspection {} | assessment {}",
            card.card_id, card.introspection_id, assessment
        ));
        for claim in &card.claims {
            lines.push(format!(
                "- {} [{}] {} | disposition: {} | grounded: {}",
                claim.claim_id,
                claim.classification,
                bounded_text(&claim.summary, 480),
                bounded_text(&claim.disposition, 240),
                bounded_text(&claim.grounded_disposition, 720),
            ));
        }
        if !card.remaining_gaps.is_empty() {
            lines.push(format!(
                "  Remaining claim IDs: {}",
                card.remaining_gaps
                    .iter()
                    .map(|gap| gap.claim_id.as_str())
                    .collect::<Vec<_>>()
                    .join(",")
            ));
        }
        if !card.authority_waits.is_empty() {
            lines.push(format!(
                "  Authority-wait claim IDs: {}",
                card.authority_waits
                    .iter()
                    .map(|wait| wait.claim_id.as_str())
                    .collect::<Vec<_>>()
                    .join(",")
            ));
        }
        lines.push(format!(
            "Optional exact standalone response: Prior Evidence: {} :: mechanical_only|still_friction|contradicted|not_assessed",
            card.card_id
        ));
        bindings.push(PriorEvidenceBindingV1 {
            card_id: card.card_id,
            introspection_id: card.introspection_id,
            claim_ids: card
                .claims
                .into_iter()
                .map(|claim| claim.claim_id)
                .collect(),
        });
    }
    lines.push("--- End Prior Steward Evidence V1 ---".to_string());
    PriorEvidenceContextV1 {
        prompt_context: bounded_text(&lines.join("\n"), MAX_PROMPT_CHARS),
        bindings,
    }
}

pub(in crate::autonomous) fn record_prior_evidence_responses_v1(
    workspace: &Path,
    artifact_path: &Path,
    response: &str,
    context: &PriorEvidenceContextV1,
) -> Result<Vec<PathBuf>, String> {
    let relative = artifact_path.strip_prefix(workspace).map_err(|_| {
        format!(
            "introspection artifact is outside workspace: {}",
            artifact_path.display()
        )
    })?;
    let relative_text = safe_relative(
        &relative.to_string_lossy().replace('\\', "/"),
        Some("introspections"),
    )?
    .to_string_lossy()
    .replace('\\', "/");
    let artifact_raw = fs::read(artifact_path)
        .map_err(|error| format!("read {}: {error}", artifact_path.display()))?;
    let artifact_text = std::str::from_utf8(&artifact_raw)
        .map_err(|error| format!("decode {}: {error}", artifact_path.display()))?;
    let current = CurrentIntrospectionV1 {
        path: relative_text,
        introspection_id: artifact_path
            .file_stem()
            .and_then(|value| value.to_str())
            .ok_or_else(|| "introspection artifact has no UTF-8 stem".to_string())?
            .to_string(),
        sha256: sha256_bytes(&artifact_raw),
    };
    let recorded_at_unix_ms = now_unix_ms();
    let mut written = Vec::new();
    let mut observed = HashSet::new();
    for line in response.lines() {
        let Some((card_id, status)) = parse_response_line(line) else {
            continue;
        };
        if !observed.insert(card_id.to_string()) {
            continue;
        }
        let response_line = artifact_text
            .lines()
            .position(|artifact_line| artifact_line == line)
            .map(|line_index| line_index.saturating_add(1))
            .ok_or_else(|| {
                format!(
                    "exact prior-evidence response is absent from canonical artifact: {card_id}"
                )
            })?;
        let binding = context
            .bindings
            .iter()
            .find(|binding| binding.card_id == card_id);
        let receipt_id = format!(
            "icrv1_{}",
            sha256_bytes(
                format!(
                    "{card_id}\0{}\0{}\0{response_line}\0{}",
                    current.path,
                    current.sha256,
                    status.as_str()
                )
                .as_bytes()
            )
        );
        let receipt = ResponseReceiptV1 {
            schema: RESPONSE_SCHEMA,
            schema_version: 1,
            receipt_id: receipt_id.clone(),
            card_id: card_id.to_string(),
            assessment_status: status,
            current_introspection: current.clone(),
            response_line,
            recorded_at_unix_ms,
            bound: binding.is_some(),
            linked_prior_introspection_id: binding.map(|value| value.introspection_id.clone()),
            linked_claim_ids: binding.map_or_else(Vec::new, |value| value.claim_ids.clone()),
            right_to_ignore: true,
            silence_is_neutral: true,
            mechanical_evidence_only: true,
            felt_closure_inferred: false,
            consent_inferred: false,
            no_authority: true,
            authority_boundary: "response_is_evidence_only_not_felt_closure_control_approval_or_activation",
            artifact_authority_state_v1: ResponseAuthorityStateV1 {
                schema: "artifact_authority_state_v1",
                schema_version: 1,
                state: "evidence_only",
                witness_only: true,
            },
        };
        let path = workspace
            .join(STATE_RELATIVE)
            .join("responses")
            .join(format!("{receipt_id}.json"));
        write_owner_json(&path, &receipt)?;
        written.push(path);
    }
    Ok(written)
}

fn parse_response_line(line: &str) -> Option<(&str, AssessmentStatusV1)> {
    if line.trim() != line {
        return None;
    }
    let remainder = line.strip_prefix("Prior Evidence: ")?;
    let (card_id, status) = remainder.split_once(" :: ")?;
    require_prefixed_sha256(card_id, "icv1_", "card_id").ok()?;
    Some((card_id, AssessmentStatusV1::parse(status)?))
}

fn source_locator(path: &Path) -> Result<Option<String>, String> {
    let normalized = path.to_string_lossy().replace('\\', "/");
    if let Some((_, suffix)) = normalized.split_once("/capsules/spectral-bridge/") {
        let locator = format!("capsules/spectral-bridge/{suffix}");
        safe_relative(&locator, None)?;
        return Ok(Some(locator));
    }
    if path.is_absolute() {
        return Ok(None);
    }
    let safe = safe_relative(&normalized, None)?;
    Ok(Some(safe.to_string_lossy().replace('\\', "/")))
}

fn stable_source_identity(label: &str, locator: Option<&str>) -> String {
    let label_json = serde_json::to_string(label).expect("string serialization cannot fail");
    let locator_json = serde_json::to_string(&locator).expect("optional string serialization");
    format!(
        "source_{}",
        sha256_bytes(format!("{{\"label\":{label_json},\"locator\":{locator_json}}}").as_bytes())
    )
}

fn safe_relative(value: &str, prefix: Option<&str>) -> Result<PathBuf, String> {
    if value.trim().is_empty() {
        return Err("relative path is empty".to_string());
    }
    let candidate = Path::new(value);
    if candidate.is_absolute()
        || candidate
            .components()
            .any(|component| !matches!(component, Component::Normal(_)))
    {
        return Err(format!("unsafe relative path: {value}"));
    }
    if let Some(prefix) = prefix
        && !candidate.starts_with(prefix)
    {
        return Err(format!("relative path must begin with {prefix}: {value}"));
    }
    Ok(candidate.to_path_buf())
}

fn read_bounded_owner_file(path: &Path, max_bytes: u64) -> Result<Vec<u8>, String> {
    let metadata =
        fs::metadata(path).map_err(|error| format!("stat {}: {error}", path.display()))?;
    if !metadata.is_file() || metadata.len() > max_bytes {
        return Err(format!(
            "bounded evidence file is invalid: {}",
            path.display()
        ));
    }
    #[cfg(unix)]
    if metadata.permissions().mode() & 0o077 != 0 {
        return Err(format!(
            "evidence file is not owner-only: {}",
            path.display()
        ));
    }
    fs::read(path).map_err(|error| format!("read {}: {error}", path.display()))
}

fn require_sha256(value: &str, field: &str) -> Result<(), String> {
    if value.len() != 64
        || !value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
    {
        return Err(format!("{field} must be a lowercase SHA-256"));
    }
    Ok(())
}

fn require_prefixed_sha256(value: &str, prefix: &str, field: &str) -> Result<(), String> {
    let digest = value
        .strip_prefix(prefix)
        .ok_or_else(|| format!("{field} must begin with {prefix}"))?;
    require_sha256(digest, field)
}

fn bounded_text(value: &str, max_chars: usize) -> String {
    let mut result = value.chars().take(max_chars).collect::<String>();
    if value.chars().count() > max_chars {
        result.push_str("...[bounded]");
    }
    result
}

fn sha256_bytes(value: &[u8]) -> String {
    format!("{:x}", Sha256::digest(value))
}

fn now_unix_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .ok()
        .and_then(|duration| u64::try_from(duration.as_millis()).ok())
        .unwrap_or(0)
}

fn write_owner_json<T: Serialize>(path: &Path, value: &T) -> Result<(), String> {
    let mut bytes = serde_json::to_vec_pretty(value)
        .map_err(|error| format!("serialize {}: {error}", path.display()))?;
    bytes.push(b'\n');
    let parent = path
        .parent()
        .ok_or_else(|| format!("{} has no parent", path.display()))?;
    fs::create_dir_all(parent).map_err(|error| format!("create {}: {error}", parent.display()))?;
    #[cfg(unix)]
    fs::set_permissions(parent, fs::Permissions::from_mode(0o700))
        .map_err(|error| format!("set owner-only {}: {error}", parent.display()))?;
    let temporary = parent.join(format!(
        ".{}.{}.tmp",
        path.file_name()
            .and_then(|value| value.to_str())
            .unwrap_or("receipt"),
        std::process::id()
    ));
    let mut options = OpenOptions::new();
    options.create(true).truncate(true).write(true);
    #[cfg(unix)]
    options.mode(0o600);
    let mut file = options
        .open(&temporary)
        .map_err(|error| format!("open {}: {error}", temporary.display()))?;
    file.write_all(&bytes)
        .map_err(|error| format!("write {}: {error}", temporary.display()))?;
    file.sync_all()
        .map_err(|error| format!("sync {}: {error}", temporary.display()))?;
    drop(file);
    fs::rename(&temporary, path).map_err(|error| format!("replace {}: {error}", path.display()))?;
    #[cfg(unix)]
    fs::set_permissions(path, fs::Permissions::from_mode(0o600))
        .map_err(|error| format!("set owner-only {}: {error}", path.display()))?;
    Ok(())
}

#[cfg(test)]
#[path = "continuity_v1/tests.rs"]
mod tests;
