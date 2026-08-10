use std::path::Path;

use serde::Serialize;
use sha2::{Digest, Sha256};
use uuid::Uuid;

use super::schema::{
    CANDIDATE_PATCH_SCHEMA_V1, CandidateFileChangeV1, CandidatePatchV1, ChangeOperationV1,
    EXACT_MODEL_ATTESTATION_SCHEMA_V1, ExactModelAttestationV1, ImmutablePathClassV1,
    SCHEDULED_INTROSPECTION_SCHEMA_V1, ScheduledIntrospectionAuthorityV1, ScheduledIntrospectionV1,
};
use super::{SelfChangeError, SelfChangeResult};

pub const MAX_CHANGED_FILES: usize = 25;
pub const MAX_CHANGED_LINES: u32 = 4_000;
pub const MAX_ATTESTATION_AGE_MS: i64 = 10 * 60 * 1_000;
const MAX_ATTESTATION_FUTURE_SKEW_MS: i64 = 30 * 1_000;
const MAX_SOURCE_PATH_BYTES: usize = 240;
const MAX_PATH_COMPONENT_BYTES: usize = 64;
const MAX_QUESTION_CHARS: usize = 600;
const MAX_SCHEDULE_WINDOW_MS: i64 = 48 * 60 * 60 * 1_000;

#[must_use]
pub fn sha256_hex(bytes: impl AsRef<[u8]>) -> String {
    format!("{:x}", Sha256::digest(bytes.as_ref()))
}

pub(crate) fn canonical_sha256<T: Serialize>(value: &T) -> SelfChangeResult<String> {
    Ok(sha256_hex(serde_json::to_vec(value)?))
}

/// Validates one exact `cpu-edge:<sha256>` source identity.
///
/// # Errors
///
/// Returns an error when the prefix or lowercase SHA-256 digest is invalid.
pub fn validate_source_id(source_id: &str) -> SelfChangeResult<()> {
    let Some(revision) = source_id.strip_prefix("cpu-edge:") else {
        return Err(SelfChangeError::InvalidIdentifier(
            "source_id must start with cpu-edge:",
        ));
    };
    validate_sha256_named(revision, "source revision")
}

/// Validates a bounded candidate patch and its aggregate limits.
///
/// # Errors
///
/// Returns an error for an unsupported schema, invalid identity/hash/path,
/// immutable surface, duplicate ordering, inconsistent operation, or limit.
pub fn validate_candidate_patch(patch: &CandidatePatchV1) -> SelfChangeResult<()> {
    if patch.schema != CANDIDATE_PATCH_SCHEMA_V1 {
        return Err(SelfChangeError::InvalidSchema("candidate patch"));
    }
    validate_prefixed_hex_id(&patch.candidate_id, "sc", "candidate_id")?;
    validate_source_id(&patch.source_id)?;
    validate_sha256_named(&patch.source_manifest_sha256, "source manifest")?;
    validate_sha256_named(&patch.proposal_sha256, "proposal")?;
    validate_sha256_named(&patch.patch_sha256, "patch")?;
    if patch.files.is_empty() {
        return Err(SelfChangeError::InvalidPatch(
            "candidate must change at least one file",
        ));
    }
    if patch.files.len() > MAX_CHANGED_FILES {
        return Err(SelfChangeError::LimitExceeded("changed file count"));
    }

    let mut previous_path: Option<&str> = None;
    let mut changed_lines = 0_u32;
    for change in &patch.files {
        validate_file_change(change)?;
        if previous_path.is_some_and(|previous| previous >= change.path.as_str()) {
            return Err(SelfChangeError::InvalidPatch(
                "file paths must be strictly sorted and unique",
            ));
        }
        previous_path = Some(&change.path);
        let per_file = change
            .added_lines
            .checked_add(change.removed_lines)
            .ok_or(SelfChangeError::ArithmeticOverflow)?;
        changed_lines = changed_lines
            .checked_add(per_file)
            .ok_or(SelfChangeError::ArithmeticOverflow)?;
        if changed_lines > MAX_CHANGED_LINES {
            return Err(SelfChangeError::LimitExceeded("changed line count"));
        }
    }
    Ok(())
}

fn validate_file_change(change: &CandidateFileChangeV1) -> SelfChangeResult<()> {
    validate_candidate_source_path(&change.path)?;
    if change.added_lines == 0 && change.removed_lines == 0 {
        return Err(SelfChangeError::InvalidPatch(
            "each file must change at least one line",
        ));
    }
    if let Some(old_hash) = &change.old_sha256 {
        validate_sha256_named(old_hash, "old file")?;
    }
    if let Some(new_hash) = &change.new_sha256 {
        validate_sha256_named(new_hash, "new file")?;
    }
    match change.operation {
        ChangeOperationV1::Create
            if change.old_sha256.is_none()
                && change.new_sha256.is_some()
                && change.added_lines > 0
                && change.removed_lines == 0 =>
        {
            Ok(())
        },
        ChangeOperationV1::Modify
            if change.old_sha256.is_some()
                && change.new_sha256.is_some()
                && change.old_sha256 != change.new_sha256 =>
        {
            Ok(())
        },
        ChangeOperationV1::Delete
            if change.old_sha256.is_some()
                && change.new_sha256.is_none()
                && change.added_lines == 0
                && change.removed_lines > 0 =>
        {
            Ok(())
        },
        _ => Err(SelfChangeError::InvalidPatch(
            "file hashes and line counts do not match operation",
        )),
    }
}

/// Validates a lexical source-bundle path. A broker must additionally reject symlinks, hardlinks,
/// devices, and mount escapes while materializing the reviewed bundle.
///
/// # Errors
///
/// Returns an error for traversal, absolute/hidden/oversized components,
/// immutable paths, or files outside the explicit mutable CPU-edge surface.
pub fn validate_candidate_source_path(path: &str) -> SelfChangeResult<()> {
    if path.is_empty()
        || path.len() > MAX_SOURCE_PATH_BYTES
        || path.starts_with('/')
        || path.ends_with('/')
        || path.contains('\\')
        || path.contains(':')
        || path.chars().any(char::is_control)
        || Path::new(path).is_absolute()
    {
        return Err(SelfChangeError::InvalidPatch("invalid source path"));
    }
    for component in path.split('/') {
        if component.is_empty()
            || component == "."
            || component == ".."
            || component.starts_with('.')
            || component.len() > MAX_PATH_COMPONENT_BYTES
        {
            return Err(SelfChangeError::InvalidPatch(
                "invalid source path component",
            ));
        }
    }
    if let Some(class) = classify_immutable_path(path) {
        return Err(SelfChangeError::ImmutablePath(class));
    }
    if is_mutable_edge_runtime_leaf(path)
        || is_mutable_build_manifest(path)
        || is_mutable_required_core(path)
        || is_mutable_edge_capsule_leaf(path)
        || is_mutable_edge_report(path)
        || is_mutable_appliance_profile(path)
        || is_mutable_astrid_service_template(path)
    {
        Ok(())
    } else {
        Err(SelfChangeError::UnsupportedMutableSurface)
    }
}

fn is_mutable_build_manifest(path: &str) -> bool {
    const CAPSULE_MANIFESTS: &[&str] = &[
        "capsules/astralis/astrid-capsule-edge-context/Cargo.toml",
        "capsules/astralis/astrid-capsule-edge-introspector/Cargo.toml",
        "capsules/astralis/astrid-capsule-edge-spectral/Cargo.toml",
    ];

    if matches!(
        path,
        "Cargo.toml"
            | "Cargo.lock"
            | "services/astrid-edge-runtime/Cargo.toml"
            | "services/astrid-edge-runtime/Cargo.lock"
    ) {
        return true;
    }
    if let Some(relative) = path.strip_prefix("crates/") {
        let Some((crate_name, leaf)) = relative.split_once('/') else {
            return false;
        };
        return is_required_core_crate(crate_name) && leaf == "Cargo.toml";
    }
    CAPSULE_MANIFESTS.contains(&path)
}

#[must_use]
pub fn classify_immutable_path(path: &str) -> Option<ImmutablePathClassV1> {
    let normalized = path.trim_start_matches("./");
    if normalized.starts_with("services/astrid-edge-self-change-")
        || normalized.starts_with("services/astrid-edge-steward-helper/")
        || normalized.starts_with("services/astrid-edge-rescue-helper/")
        || normalized.starts_with("services/astrid-edge-web-broker/")
        || normalized.starts_with("services/astrid-edge-checkpoint/")
        || matches!(
            normalized,
            "scripts/edge_self_change_supervisor.py"
                | "scripts/test_edge_self_change_supervisor.py"
        )
        || normalized
            .strip_prefix("packaging/systemd/")
            .is_some_and(|relative| relative.contains("self-change"))
    {
        return Some(ImmutablePathClassV1::ImmutableRescueRoot);
    }

    if normalized.starts_with("capsules/spectral-bridge/")
        || normalized.starts_with("capsules/introspector/")
        || normalized.starts_with("minime/")
        || normalized.starts_with("../minime/")
        || normalized.starts_with("crates/astrid-minime-protocol/")
    {
        return Some(ImmutablePathClassV1::MacMinimeOrBridge);
    }

    if normalized.starts_with(".git/")
        || normalized.starts_with(".github/")
        || normalized == ".gitignore"
        || normalized == ".gitattributes"
        || normalized.ends_with("/CODEOWNERS")
    {
        return Some(ImmutablePathClassV1::VcsOrCi);
    }

    if normalized.starts_with("workspace/")
        || normalized.starts_with("state/")
        || normalized.starts_with("home/")
        || normalized.starts_with("target/")
        || normalized.starts_with("operator-quarantine/")
        || normalized.starts_with("backups/")
        || normalized.contains("/workspace/")
        || normalized.contains("/state/")
        || normalized.contains("/backups/")
        || normalized.contains("/operator-quarantine/")
        || normalized.contains("/peer/trusted/")
        || matches!(
            Path::new(normalized)
                .extension()
                .and_then(|value| value.to_str()),
            Some("jsonl" | "sqlite" | "db" | "key" | "pem")
        )
        || contains_private_identity_component(normalized)
    {
        return Some(ImmutablePathClassV1::PrivateStateOrSecrets);
    }
    None
}

fn contains_private_identity_component(path: &str) -> bool {
    path.split('/').any(|component| {
        matches!(
            component,
            ".ssh" | "private-keys" | "secrets" | "credentials" | "trusted"
        )
    })
}

fn is_mutable_edge_runtime_leaf(path: &str) -> bool {
    path.strip_prefix("services/astrid-edge-runtime/src/")
        .is_some_and(|relative| !relative.is_empty() && has_exact_extension(relative, "rs"))
}

fn is_mutable_edge_capsule_leaf(path: &str) -> bool {
    const PREFIXES: &[&str] = &[
        "capsules/astralis/astrid-capsule-edge-context/src/",
        "capsules/astralis/astrid-capsule-edge-introspector/src/",
        "capsules/astralis/astrid-capsule-edge-spectral/src/",
    ];
    PREFIXES.iter().any(|prefix| {
        let capsule_root = prefix.trim_end_matches("src/");
        path == format!("{capsule_root}Capsule.toml")
            || path
                .strip_prefix(prefix)
                .is_some_and(|relative| !relative.is_empty() && has_exact_extension(relative, "rs"))
    })
}

fn is_mutable_required_core(path: &str) -> bool {
    let Some(relative) = path.strip_prefix("crates/") else {
        return false;
    };
    let Some((crate_name, crate_path)) = relative.split_once('/') else {
        return false;
    };
    is_required_core_crate(crate_name)
        && (crate_path.starts_with("src/") || crate_path.starts_with("tests/"))
        && has_exact_extension(crate_path, "rs")
}

fn is_required_core_crate(crate_name: &str) -> bool {
    const CRATES: &[&str] = &[
        "astrid-approval",
        "astrid-audit",
        "astrid-capabilities",
        "astrid-capsule",
        "astrid-cli",
        "astrid-config",
        "astrid-core",
        "astrid-crypto",
        "astrid-daemon",
        "astrid-events",
        "astrid-guest",
        "astrid-hooks",
        "astrid-kernel",
        "astrid-mcp",
        "astrid-spectral-core",
        "astrid-storage",
        "astrid-telemetry",
        "astrid-types",
        "astrid-vfs",
        "astrid-workspace",
    ];
    CRATES.contains(&crate_name)
}

fn is_mutable_edge_report(path: &str) -> bool {
    let Some(file) = path.strip_prefix("scripts/") else {
        return false;
    };
    ((file.starts_with("report_edge_") || file.starts_with("test_report_edge_"))
        && matches!(
            Path::new(file).extension().and_then(|value| value.to_str()),
            Some("py" | "sh")
        ))
        || matches!(
            file,
            "astrid_at_a_glance.py" | "edge_hindsight.py" | "test_edge_hindsight.py"
        )
}

fn is_mutable_appliance_profile(path: &str) -> bool {
    path.strip_prefix("packaging/appliances/")
        .is_some_and(|relative| {
            !relative.contains('/')
                && matches!(
                    Path::new(relative)
                        .extension()
                        .and_then(|value| value.to_str()),
                    Some("env" | "json")
                )
        })
}

fn is_mutable_astrid_service_template(path: &str) -> bool {
    let Some(relative) = path.strip_prefix("packaging/systemd/") else {
        return false;
    };
    let file = relative.strip_prefix("icp/").unwrap_or(relative);
    !file.contains('/')
        && file.starts_with("astrid")
        && matches!(
            Path::new(file).extension().and_then(|value| value.to_str()),
            Some("service" | "timer" | "conf" | "env")
        )
}

fn has_exact_extension(path: &str, expected: &str) -> bool {
    Path::new(path).extension().and_then(|value| value.to_str()) == Some(expected)
}

pub(crate) fn validate_exact_model_attestation(
    attestation: &ExactModelAttestationV1,
    expected_instance_id: &str,
    transition_time_ms: i64,
) -> SelfChangeResult<()> {
    validate_exact_model_attestation_static(attestation, expected_instance_id)?;
    if attestation.authored_at_unix_ms <= 0 || transition_time_ms <= 0 {
        return Err(SelfChangeError::InvalidAttestation("invalid timestamp"));
    }
    let age = transition_time_ms
        .checked_sub(attestation.authored_at_unix_ms)
        .ok_or(SelfChangeError::ArithmeticOverflow)?;
    if !(-MAX_ATTESTATION_FUTURE_SKEW_MS..=MAX_ATTESTATION_AGE_MS).contains(&age) {
        return Err(SelfChangeError::InvalidAttestation(
            "attestation is stale or from the future",
        ));
    }
    Ok(())
}

pub(crate) fn validate_exact_model_attestation_static(
    attestation: &ExactModelAttestationV1,
    expected_instance_id: &str,
) -> SelfChangeResult<()> {
    if attestation.schema != EXACT_MODEL_ATTESTATION_SCHEMA_V1 {
        return Err(SelfChangeError::InvalidSchema("exact model attestation"));
    }
    validate_instance_id(&attestation.instance_id)?;
    if attestation.instance_id != expected_instance_id {
        return Err(SelfChangeError::InvalidAttestation("instance mismatch"));
    }
    if attestation.producer_kind != "wasm_capsule"
        || attestation.producer_capsule_id != "astrid-capsule-react"
        || attestation.kernel_sequence == 0
        || attestation.trace_id == Uuid::nil()
        || attestation.span_id == Uuid::nil()
        || attestation.session_id == Uuid::nil()
        || attestation.turn_id == Uuid::nil()
        || attestation.session_generation == 0
    {
        return Err(SelfChangeError::InvalidAttestation(
            "missing canonical kernel provenance",
        ));
    }
    match (attestation.chain_id, attestation.chain_step) {
        (None, None) => {},
        (Some(chain_id), Some(step)) if chain_id != Uuid::nil() && (1..=8).contains(&step) => {},
        _ => {
            return Err(SelfChangeError::InvalidAttestation(
                "chain id and step must be present together",
            ));
        },
    }
    validate_sha256_named(&attestation.response_sha256, "response")?;
    validate_sha256_named(
        &attestation.terminal_declaration_sha256,
        "terminal declaration",
    )?;
    validate_bounded_label(&attestation.model_id, 128, "model_id")?;
    if attestation.authored_at_unix_ms <= 0 {
        return Err(SelfChangeError::InvalidAttestation("invalid timestamp"));
    }
    Ok(())
}

pub(crate) fn validate_scheduled_introspection(
    schedule: &ScheduledIntrospectionV1,
) -> SelfChangeResult<()> {
    if schedule.schema != SCHEDULED_INTROSPECTION_SCHEMA_V1 {
        return Err(SelfChangeError::InvalidSchema("scheduled introspection"));
    }
    if schedule.authority != ScheduledIntrospectionAuthorityV1::ObservationOnly {
        return Err(SelfChangeError::InvalidSchedule(
            "scheduled introspection must be observational",
        ));
    }
    validate_prefixed_hex_id(&schedule.schedule_id, "si", "schedule_id")?;
    validate_instance_id(&schedule.instance_id)?;
    validate_prefixed_hex_id(&schedule.candidate_id, "sc", "candidate_id")?;
    validate_sha256_named(
        &schedule.expected_candidate_state_sha256,
        "scheduled candidate state",
    )?;
    validate_sha256_named(&schedule.originating_response_sha256, "scheduled response")?;
    if schedule.originating_trace_id == Uuid::nil() || schedule.originating_turn_id == Uuid::nil() {
        return Err(SelfChangeError::InvalidSchedule(
            "scheduled introspection lacks causal identifiers",
        ));
    }
    validate_bounded_text(&schedule.question, MAX_QUESTION_CHARS, "question")?;
    if schedule.not_before_unix_ms <= 0
        || schedule.expires_at_unix_ms <= schedule.not_before_unix_ms
    {
        return Err(SelfChangeError::InvalidSchedule("invalid schedule window"));
    }
    let window = schedule
        .expires_at_unix_ms
        .checked_sub(schedule.not_before_unix_ms)
        .ok_or(SelfChangeError::ArithmeticOverflow)?;
    if window > MAX_SCHEDULE_WINDOW_MS {
        return Err(SelfChangeError::LimitExceeded(
            "scheduled introspection window",
        ));
    }
    Ok(())
}

pub(crate) fn validate_instance_id(value: &str) -> SelfChangeResult<()> {
    validate_bounded_slug(value, 3, 64, "instance_id")
}

pub(crate) fn validate_bounded_label(
    value: &str,
    max_chars: usize,
    name: &'static str,
) -> SelfChangeResult<()> {
    if value.is_empty() || value.chars().count() > max_chars || value.chars().any(char::is_control)
    {
        return Err(SelfChangeError::InvalidIdentifier(name));
    }
    Ok(())
}

pub(crate) fn validate_bounded_text(
    value: &str,
    max_chars: usize,
    name: &'static str,
) -> SelfChangeResult<()> {
    if value.trim().is_empty()
        || value.chars().count() > max_chars
        || value.chars().any(char::is_control)
    {
        return Err(SelfChangeError::InvalidIdentifier(name));
    }
    Ok(())
}

pub(crate) fn validate_prefixed_hex_id(
    value: &str,
    prefix: &'static str,
    name: &'static str,
) -> SelfChangeResult<()> {
    let Some(suffix) = value
        .strip_prefix(prefix)
        .and_then(|rest| rest.strip_prefix('-'))
    else {
        return Err(SelfChangeError::InvalidIdentifier(name));
    };
    if !(24..=64).contains(&suffix.len()) || !is_lower_hex(suffix) {
        return Err(SelfChangeError::InvalidIdentifier(name));
    }
    Ok(())
}

pub(crate) fn validate_bounded_slug(
    value: &str,
    min: usize,
    max: usize,
    name: &'static str,
) -> SelfChangeResult<()> {
    let valid = (min..=max).contains(&value.len())
        && value
            .bytes()
            .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'-')
        && value
            .as_bytes()
            .first()
            .is_some_and(u8::is_ascii_alphanumeric)
        && value
            .as_bytes()
            .last()
            .is_some_and(u8::is_ascii_alphanumeric);
    if valid {
        Ok(())
    } else {
        Err(SelfChangeError::InvalidIdentifier(name))
    }
}

pub(crate) fn validate_sha256_named(value: &str, name: &'static str) -> SelfChangeResult<()> {
    if value.len() == 64 && is_lower_hex(value) {
        Ok(())
    } else {
        Err(SelfChangeError::InvalidHash(name))
    }
}

fn is_lower_hex(value: &str) -> bool {
    value
        .bytes()
        .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
}
