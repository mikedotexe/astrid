//! Approved publication only; no key generation, signing, or trust bootstrap.

use std::io::Write as _;
use std::path::Path;

use astrid_minime_protocol::public_web::{
    PublicWebApprovalV2, PublicWebMutationV2, PublicWebOwnerV2, PublicWebPolicyStateV2,
    PublicWebPolicyStatusV2, PublicWebPolicyV2,
};
use serde::{Deserialize, Serialize};

use crate::authority::{load_selected, now_ms, token_path, validate_candidate};
use crate::{PublicWebConfig, PublicWebErrorCode as E, Result, digest, guard};

/// Pre-signed candidate. The token string preserves the exact approved bytes.
#[derive(Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PublicWebBundle {
    pub policy: PublicWebPolicyV2,
    pub state: PublicWebPolicyStateV2,
    pub capability_token_json: String,
}

impl PublicWebBundle {
    /// # Errors
    /// Rejects non-private, oversized, or malformed candidate files.
    pub fn load(path: &Path) -> Result<Self> {
        serde_json::from_slice(&guard::read_private(path)?).map_err(|_| E::InvalidPolicy)
    }
}

/// # Errors
/// Rejects non-private, oversized, or malformed approval files.
pub fn load_approval(path: &Path) -> Result<PublicWebApprovalV2> {
    serde_json::from_slice(&guard::read_private(path)?).map_err(|_| E::InvalidApproval)
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct Floor {
    pub schema: String,
    pub owner_sha256: String,
    pub revision: u64,
    pub policy_sha256: String,
    pub state_sha256: String,
    pub approval_sha256: String,
    pub pending: bool,
}

impl Floor {
    pub(crate) fn sha256(&self) -> String {
        digest(&serde_json::to_vec(self).unwrap_or_default())
    }
}

#[derive(Debug, Serialize)]
pub struct PublicWebMutationPreview {
    pub implementation_scope: &'static str,
    pub operation: PublicWebMutationV2,
    pub policy_sha256: String,
    pub state_sha256: String,
    pub revision: u64,
    pub approval_sha256: String,
    pub previous_floor_sha256: Option<String>,
    pub already_published: bool,
}

#[derive(Debug, Serialize)]
pub struct PublicWebStatus {
    pub implementation_scope: &'static str,
    pub status: PublicWebPolicyStatusV2,
    pub policy_sha256: String,
    pub floor_sha256: String,
    pub state_sha256: String,
    pub state_revision: u64,
    pub expires_at_unix_ms: u64,
}

/// Inspect installed active or revoked authority; does not write or grant.
/// # Errors
/// Returns a fixed error for missing, pending, invalid, or expired authority.
pub fn status(config: &PublicWebConfig) -> Result<PublicWebStatus> {
    let (policy, state) = load_selected(config, true)?;
    let floor = read_floor(config)?.ok_or(E::AuthorityUnavailable)?;
    if floor.pending || floor.state_sha256 != state.sha256() {
        return Err(E::InvalidState);
    }
    Ok(PublicWebStatus {
        implementation_scope: "transport_foundation",
        status: state.status,
        policy_sha256: policy.sha256(),
        floor_sha256: floor.sha256(),
        state_sha256: state.sha256(),
        state_revision: state.revision,
        expires_at_unix_ms: policy.expires_at_unix_ms,
    })
}

pub(crate) fn read_floor(config: &PublicWebConfig) -> Result<Option<Floor>> {
    guard::private_directory(
        config
            .state_floor_path
            .parent()
            .ok_or(E::UnsafeFilesystem)?,
    )?;
    match std::fs::symlink_metadata(&config.state_floor_path) {
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(_) => return Err(E::UnsafeFilesystem),
        Ok(_) => {},
    }
    let floor: Floor = serde_json::from_slice(&guard::read_private(&config.state_floor_path)?)
        .map_err(|_| E::InvalidState)?;
    if floor.schema != "astrid.public_web.revision_floor.v2"
        || floor.owner_sha256 != config.expected_owner.lock_identity_sha256()
        || floor.revision == 0
        || floor.revision < config.minimum_state_revision
        || [
            &floor.policy_sha256,
            &floor.state_sha256,
            &floor.approval_sha256,
        ]
        .iter()
        .any(|hash| hash.len() != 64 || !hash.bytes().all(|b| b.is_ascii_hexdigit()))
    {
        return Err(E::InvalidState);
    }
    Ok(Some(floor))
}

/// Validate an exact signed approval and current head without writing files.
/// # Errors
/// Refuses stale/replayed, mismatched, unsigned, or out-of-scope approval.
pub fn preview_mutation(
    config: &PublicWebConfig,
    runtime_owner: &PublicWebOwnerV2,
    bundle: &PublicWebBundle,
    approval: &PublicWebApprovalV2,
) -> Result<PublicWebMutationPreview> {
    if runtime_owner != &config.expected_owner {
        return Err(E::OwnerMismatch);
    }
    validate_candidate(
        config,
        &bundle.policy,
        &bundle.state,
        bundle.capability_token_json.as_bytes(),
        true,
    )?;
    if !approval.verifies_for(&bundle.policy, &bundle.state, &config.sha256(), now_ms()?) {
        return Err(E::InvalidApproval);
    }
    let token_path = token_path(config, &bundle.policy);
    let paths = [
        &config.policy_path,
        &config.state_path,
        &config.state_floor_path,
        &token_path,
    ];
    for path in paths {
        guard::private_directory(path.parent().ok_or(E::UnsafeFilesystem)?)?;
    }
    if paths
        .iter()
        .collect::<std::collections::BTreeSet<_>>()
        .len()
        != paths.len()
        || paths
            .iter()
            .any(|path| **path == config.delegation_root.join("trust.json"))
    {
        return Err(E::InvalidConfig);
    }
    let previous = read_floor(config)?;
    let previous_hash = previous.as_ref().map(Floor::sha256);
    let approval_hash = approval.sha256();
    let same_publication = previous.as_ref().is_some_and(|floor| {
        floor.approval_sha256 == approval_hash
            && floor.state_sha256 == bundle.state.sha256()
            && floor.policy_sha256 == bundle.policy.sha256()
    });
    if !same_publication && previous_hash != approval.expected_previous_floor_sha256 {
        return Err(E::StaleApproval);
    }
    if let Some(floor) = &previous {
        let pending_resume = floor.pending
            && floor.state_sha256 == bundle.state.sha256()
            && floor.policy_sha256 == bundle.policy.sha256();
        let required_revision = if same_publication || pending_resume {
            floor.revision
        } else {
            floor.revision.checked_add(1).ok_or(E::InvalidState)?
        };
        if bundle.state.revision != required_revision || (floor.pending && !pending_resume) {
            return Err(E::InvalidState);
        }
        if approval.operation == PublicWebMutationV2::Revoke
            && floor.policy_sha256 != bundle.policy.sha256()
        {
            return Err(E::InvalidApproval);
        }
    } else if approval.operation != PublicWebMutationV2::Install
        || bundle.state.revision != 1
        || path_present(&config.policy_path)?
        || path_present(&config.state_path)?
    {
        return Err(E::InvalidState);
    }
    // Existing tokens are immutable. This installer cannot replace another grant.
    if path_present(&token_path)?
        && guard::read_private(&token_path)? != bundle.capability_token_json.as_bytes()
    {
        return Err(E::InvalidCapability);
    }
    let already_published =
        same_publication && previous.as_ref().is_some_and(|floor| !floor.pending);
    if already_published {
        // A matching floor is not evidence that the selected files survived.
        load_selected(config, true)?;
    }
    Ok(PublicWebMutationPreview {
        implementation_scope: "transport_foundation",
        operation: approval.operation,
        policy_sha256: bundle.policy.sha256(),
        state_sha256: bundle.state.sha256(),
        revision: bundle.state.revision,
        approval_sha256: approval_hash,
        previous_floor_sha256: previous_hash,
        already_published,
    })
}

/// Publish only a pre-signed, explicitly approved bundle. Never signs or mints.
/// # Errors
/// An error after publication starts leaves a durable pending floor and denies
/// fetches. Retry the same signed publication, or freshly approve that exact
/// pending candidate; neither path may restore a previous active revision.
pub fn mutate(
    config: &PublicWebConfig,
    runtime_owner: &PublicWebOwnerV2,
    bundle: &PublicWebBundle,
    approval: &PublicWebApprovalV2,
) -> Result<PublicWebMutationPreview> {
    publish(config, runtime_owner, bundle, approval, |_, _, _| Ok(()))
}

pub(crate) fn publish(
    config: &PublicWebConfig,
    runtime_owner: &PublicWebOwnerV2,
    bundle: &PublicWebBundle,
    approval: &PublicWebApprovalV2,
    mut checkpoint: impl FnMut(usize, &Path, &[u8]) -> Result<()>,
) -> Result<PublicWebMutationPreview> {
    let operator_lock = digest(
        format!(
            "public-web-operator:{}",
            config.expected_owner.lock_identity_sha256()
        )
        .as_bytes(),
    );
    let _lease = guard::OwnerLease::acquire(&config.owner_lock_dir, &operator_lock)?;
    let preview = preview_mutation(config, runtime_owner, bundle, approval)?;
    if preview.already_published {
        return Ok(preview);
    }
    let mut floor = Floor {
        schema: "astrid.public_web.revision_floor.v2".into(),
        owner_sha256: config.expected_owner.lock_identity_sha256(),
        revision: bundle.state.revision,
        policy_sha256: bundle.policy.sha256(),
        state_sha256: bundle.state.sha256(),
        approval_sha256: approval.sha256(),
        pending: true,
    };
    atomic_json(&config.state_floor_path, &floor)?;
    checkpoint(0, &config.state_floor_path, &[])?;
    let token_path = token_path(config, &bundle.policy);
    let policy_bytes = serde_json::to_vec(&bundle.policy).map_err(|_| E::InvalidPolicy)?;
    let state_bytes = serde_json::to_vec(&bundle.state).map_err(|_| E::InvalidState)?;
    for (index, (path, bytes)) in [
        (&token_path, bundle.capability_token_json.as_bytes()),
        (&config.policy_path, policy_bytes.as_slice()),
        (&config.state_path, state_bytes.as_slice()),
    ]
    .into_iter()
    .enumerate()
    {
        if index == 0 {
            publish_token(path, bytes)?;
        } else {
            atomic_bytes(path, bytes)?;
        }
        checkpoint(index.saturating_add(1), path, bytes)?;
    }
    // Approval/authority may expire or be revoked during publication.
    validate_candidate(
        config,
        &bundle.policy,
        &bundle.state,
        bundle.capability_token_json.as_bytes(),
        true,
    )?;
    if !approval.verifies_for(&bundle.policy, &bundle.state, &config.sha256(), now_ms()?) {
        return Err(E::InvalidApproval);
    }
    floor.pending = false;
    atomic_json(&config.state_floor_path, &floor)?;
    Ok(preview)
}

fn atomic_json(path: &Path, value: &impl Serialize) -> Result<()> {
    atomic_bytes(
        path,
        &serde_json::to_vec(value).map_err(|_| E::PublicationFailed)?,
    )
}

fn path_present(path: &Path) -> Result<bool> {
    match std::fs::symlink_metadata(path) {
        Ok(_) => Ok(true),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(false),
        Err(_) => Err(E::UnsafeFilesystem),
    }
}

fn publish_token(path: &Path, bytes: &[u8]) -> Result<()> {
    let parent = path.parent().ok_or(E::UnsafeFilesystem)?;
    guard::private_directory(parent)?;
    if !path_present(path)? {
        let mut temporary =
            tempfile::NamedTempFile::new_in(parent).map_err(|_| E::PublicationFailed)?;
        temporary
            .write_all(bytes)
            .map_err(|_| E::PublicationFailed)?;
        temporary
            .as_file()
            .sync_all()
            .map_err(|_| E::PublicationFailed)?;
        match temporary.persist_noclobber(path) {
            Ok(_) => {
                std::fs::File::open(parent)
                    .and_then(|file| file.sync_all())
                    .map_err(|_| E::PublicationFailed)?;
            },
            Err(error) if error.error.kind() == std::io::ErrorKind::AlreadyExists => {},
            Err(_) => return Err(E::PublicationFailed),
        }
    }
    if guard::read_private(path)? != bytes {
        return Err(E::InvalidCapability);
    }
    Ok(())
}

fn atomic_bytes(path: &Path, bytes: &[u8]) -> Result<()> {
    let parent = path.parent().ok_or(E::UnsafeFilesystem)?;
    guard::private_directory(parent)?;
    if u64::try_from(bytes.len()).unwrap_or(u64::MAX) > guard::MAX_AUTHORITY_BYTES {
        return Err(E::PublicationFailed);
    }
    let mut temporary =
        tempfile::NamedTempFile::new_in(parent).map_err(|_| E::PublicationFailed)?;
    temporary
        .write_all(bytes)
        .map_err(|_| E::PublicationFailed)?;
    temporary
        .as_file()
        .sync_all()
        .map_err(|_| E::PublicationFailed)?;
    temporary.persist(path).map_err(|_| E::PublicationFailed)?;
    std::fs::File::open(parent)
        .and_then(|file| file.sync_all())
        .map_err(|_| E::PublicationFailed)
}
