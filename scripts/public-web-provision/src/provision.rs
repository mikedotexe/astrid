use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use astrid_capabilities::{AuditEntryId, CapabilityToken, ResourcePattern, TokenScope};
use astrid_core::Permission;
use astrid_crypto::KeyPair;
use astrid_minime_protocol::public_web::{
    PUBLIC_WEB_APPROVAL_SCHEMA_V2, PUBLIC_WEB_CAPABILITY_RESOURCE_V2, PUBLIC_WEB_POLICY_SCHEMA_V2,
    PUBLIC_WEB_STATE_SCHEMA_V2, PublicWebApprovalV2, PublicWebLimitsV2, PublicWebMutationV2,
    PublicWebOperationV2, PublicWebOwnerV2, PublicWebPolicyStateV2, PublicWebPolicyStatusV2,
    PublicWebPolicyV2,
};
use astrid_public_web::{CONFIG_SCHEMA, PublicWebBundle, PublicWebConfig};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use sha2::{Digest as _, Sha256};
use zeroize::Zeroizing;

use crate::Result;
use crate::private_fs::{self, Directory};

const YEAR_MS: u64 = 31_536_000_000;
const APPROVAL_MS: u64 = 300_000;
const KEY_HEADER: &[u8] = b"APWPKEY1";
const KEY_NAME: &str = "issuer.key";

#[derive(Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct PrepareRequest {
    schema: String,
    owner: PublicWebOwnerV2,
    policy_id: String,
    install_root: PathBuf,
    owner_lock_dir: PathBuf,
    approval_audit_ref: String,
}

#[derive(Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct ExactInput {
    owner: PublicWebOwnerV2,
    owner_lock_dir: PathBuf,
    config_path: PathBuf,
    bundle_path: PathBuf,
    config_sha256: String,
    policy_sha256: String,
    state_sha256: String,
}

#[derive(Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct RenewRequest {
    schema: String,
    exact: ExactInput,
    approval_path: PathBuf,
    approval_file_sha256: String,
    approval_audit_ref: String,
}

#[derive(Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct RevokeRequest {
    schema: String,
    exact: ExactInput,
    expected_previous_floor_sha256: String,
    expected_state_revision: u64,
    approval_audit_ref: String,
}

// The runtime's trust reader is private. This unsigned bootstrap document mirrors
// its existing serde wire schema; all signed statements use the exported types.
#[derive(Serialize)]
struct TrustDocument {
    schema: &'static str,
    target_being: String,
    trusted_issuer_public_keys: Vec<String>,
    bootstrap_audit_refs: Vec<String>,
    authorized_at_unix_ms: u64,
}

fn now_ms() -> Result<u64> {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .ok()
        .and_then(|duration| u64::try_from(duration.as_millis()).ok())
        .ok_or("clock_unavailable")
}

fn sha(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}

fn encoded(value: &impl Serialize) -> Result<Vec<u8>> {
    serde_json::to_vec(value).map_err(|_| "serialization_failed")
}

fn identifier(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 256
        && value.trim() == value
        && !value.chars().any(char::is_control)
}

fn disjoint(left: &Path, right: &Path) -> Result<()> {
    private_fs::absolute(left)?;
    private_fs::absolute(right)?;
    if left.starts_with(right) || right.starts_with(left) {
        return Err("overlapping_paths");
    }
    Ok(())
}

fn lock_location(directory: &Path, issuer: &Path, out: &Path) -> Result<()> {
    disjoint(directory, issuer)?;
    disjoint(directory, out)?;
    Directory::open(directory)?;
    Ok(())
}

pub(crate) fn init_issuer(path: &Path) -> Result<Value> {
    let key = KeyPair::generate();
    let secret = Zeroizing::new(key.secret_key_bytes());
    let mut bytes = Zeroizing::new(Vec::with_capacity(72));
    bytes.extend_from_slice(KEY_HEADER);
    bytes.extend_from_slice(secret.as_ref());
    bytes.extend_from_slice(key.public_key_bytes());
    Directory::create(path)?.write_new(KEY_NAME, &bytes)?;
    Ok(
        json!({"status":"issuer_created", "issuer_key_path":path.join(KEY_NAME),
        "issuer_public_key_hex":key.export_public_key().to_hex()}),
    )
}

fn load_issuer(path: &Path) -> Result<KeyPair> {
    let bytes = private_fs::read(&path.join(KEY_NAME))?;
    if bytes.len() != 72 || !bytes.starts_with(KEY_HEADER) {
        return Err("corrupt_issuer");
    }
    let key = KeyPair::from_secret_key(&bytes[8..40]).map_err(|_| "corrupt_issuer")?;
    if key.public_key_bytes().as_slice() != &bytes[40..72] {
        return Err("corrupt_issuer");
    }
    Ok(key)
}

fn config_for(
    root: &Path,
    owner_lock_dir: &Path,
    owner: PublicWebOwnerV2,
    policy_hash: String,
) -> PublicWebConfig {
    PublicWebConfig {
        schema: CONFIG_SCHEMA.into(),
        delegation_root: root.join("delegation"),
        policy_path: root.join("policy.json"),
        state_path: root.join("state.json"),
        state_floor_path: root.join("state-floor.json"),
        owner_lock_dir: owner_lock_dir.to_path_buf(),
        storage_path: root.join("storage"),
        expected_owner: owner,
        expected_policy_sha256: policy_hash,
        minimum_state_revision: 1,
    }
}

fn sign_state(key: &KeyPair, state: &mut PublicWebPolicyStateV2) -> Result<()> {
    state.signature_hex = key
        .sign(&state.signing_bytes().ok_or("invalid_state")?)
        .to_hex();
    Ok(())
}

fn approval_for(
    key: &KeyPair,
    bundle: &PublicWebBundle,
    config: &PublicWebConfig,
    previous: Option<String>,
    audit_ref: String,
) -> Result<PublicWebApprovalV2> {
    let now = now_ms()?;
    let mut approval = PublicWebApprovalV2 {
        schema: PUBLIC_WEB_APPROVAL_SCHEMA_V2.into(),
        operation: if bundle.state.status == PublicWebPolicyStatusV2::Active {
            PublicWebMutationV2::Install
        } else {
            PublicWebMutationV2::Revoke
        },
        owner: bundle.policy.owner.clone(),
        policy_sha256: bundle.policy.sha256(),
        state_sha256: bundle.state.sha256(),
        config_sha256: config.sha256(),
        expected_previous_floor_sha256: previous,
        approved_at_unix_ms: now,
        expires_at_unix_ms: now.checked_add(APPROVAL_MS).ok_or("clock_overflow")?,
        approval_audit_ref: audit_ref,
        signer_public_key_hex: key.export_public_key().to_hex(),
        signature_hex: String::new(),
    };
    approval.signature_hex = key
        .sign(&approval.signing_bytes().ok_or("invalid_approval")?)
        .to_hex();
    if !approval.verifies_for(&bundle.policy, &bundle.state, &config.sha256(), now) {
        return Err("invalid_approval");
    }
    Ok(approval)
}

pub(crate) fn prepare(issuer: &Path, request_path: &Path, out: &Path) -> Result<Value> {
    let request: PrepareRequest = private_fs::read_json(request_path)?;
    if request.schema != "astrid.public_web.provision.prepare.v1"
        || !request.owner.is_well_formed()
        || !identifier(&request.policy_id)
        || !identifier(&request.approval_audit_ref)
    {
        return Err("invalid_prepare_request");
    }
    disjoint(issuer, out)?;
    disjoint(&request.install_root, issuer)?;
    disjoint(&request.install_root, out)?;
    lock_location(&request.owner_lock_dir, issuer, out)?;
    private_fs::require_absent(&request.install_root)?;
    let key = load_issuer(issuer)?;
    let token = CapabilityToken::create(
        ResourcePattern::exact(PUBLIC_WEB_CAPABILITY_RESOURCE_V2)
            .map_err(|_| "invalid_resource")?,
        vec![Permission::Read],
        TokenScope::Persistent,
        key.key_id(),
        AuditEntryId::new(),
        &key,
        Some(chrono::Duration::days(365)),
    );
    let token_json = serde_json::to_string(&token).map_err(|_| "serialization_failed")?;
    let timestamp =
        u64::try_from(token.issued_at.0.timestamp_millis()).map_err(|_| "clock_unavailable")?;
    let mut policy = PublicWebPolicyV2 {
        schema: PUBLIC_WEB_POLICY_SCHEMA_V2.into(),
        policy_id: request.policy_id,
        revision: 1,
        owner: request.owner,
        capability_token_id: token.id.to_string(),
        capability_token_sha256: sha(token_json.as_bytes()),
        issuer_public_key_hex: key.export_public_key().to_hex(),
        operations: vec![PublicWebOperationV2::Search, PublicWebOperationV2::Page],
        limits: PublicWebLimitsV2::default(),
        issued_at_unix_ms: timestamp,
        expires_at_unix_ms: timestamp.checked_add(YEAR_MS).ok_or("clock_overflow")?,
        approval_audit_refs: vec![request.approval_audit_ref.clone()],
        signature_hex: String::new(),
    };
    policy.signature_hex = key
        .sign(&policy.signing_bytes().ok_or("invalid_policy")?)
        .to_hex();
    let config = config_for(
        &request.install_root,
        &request.owner_lock_dir,
        policy.owner.clone(),
        policy.sha256(),
    );
    let mut state = PublicWebPolicyStateV2 {
        schema: PUBLIC_WEB_STATE_SCHEMA_V2.into(),
        policy_id: policy.policy_id.clone(),
        policy_sha256: policy.sha256(),
        revision: 1,
        status: PublicWebPolicyStatusV2::Active,
        updated_at_unix_ms: timestamp,
        signer_public_key_hex: key.export_public_key().to_hex(),
        signature_hex: String::new(),
    };
    sign_state(&key, &mut state)?;
    let bundle = PublicWebBundle {
        policy,
        state,
        capability_token_json: token_json,
    };
    validate(&key, &config, &bundle)?;
    let approval = approval_for(
        &key,
        &bundle,
        &config,
        None,
        request.approval_audit_ref.clone(),
    )?;
    let trust = TrustDocument {
        schema: "astrid.volition.delegated_trust.v1",
        target_being: bundle.policy.owner.being.clone(),
        trusted_issuer_public_keys: vec![key.export_public_key().to_hex()],
        bootstrap_audit_refs: vec![request.approval_audit_ref],
        authorized_at_unix_ms: timestamp,
    };
    let mut files = common_files(&config, &bundle, &approval)?;
    files.insert("trust.json", encoded(&trust)?);
    write_packet(out, &config, &bundle, &approval, &files)
}

fn validate(key: &KeyPair, config: &PublicWebConfig, bundle: &PublicWebBundle) -> Result<()> {
    let now = now_ms()?;
    let policy = &bundle.policy;
    let root = config.policy_path.parent().ok_or("invalid_config")?;
    private_fs::absolute(root)?;
    disjoint(root, &config.owner_lock_dir)?;
    Directory::open(&config.owner_lock_dir)?;
    let expected = config_for(
        root,
        &config.owner_lock_dir,
        policy.owner.clone(),
        policy.sha256(),
    );
    if config.sha256() != expected.sha256()
        || !policy.verifies(now)
        || policy.issuer_public_key_hex != key.export_public_key().to_hex()
        || policy.operations != [PublicWebOperationV2::Search, PublicWebOperationV2::Page]
        || policy.limits != PublicWebLimitsV2::default()
        || policy
            .expires_at_unix_ms
            .checked_sub(policy.issued_at_unix_ms)
            != Some(YEAR_MS)
        || !bundle
            .state
            .verifies_for(policy, config.minimum_state_revision, now)
    {
        return Err("invalid_exact_authority");
    }
    let token: CapabilityToken =
        serde_json::from_str(&bundle.capability_token_json).map_err(|_| "invalid_capability")?;
    if token.validate_with_skew(0).is_err()
        || token.is_single_use()
        || token.id.to_string() != policy.capability_token_id
        || token.issuer.to_hex() != policy.issuer_public_key_hex
        || token.scope != TokenScope::Persistent
        || token.permissions != [Permission::Read]
        || token.resource.as_str() != PUBLIC_WEB_CAPABILITY_RESOURCE_V2
        || sha(bundle.capability_token_json.as_bytes()) != policy.capability_token_sha256
        || token
            .expires_at
            .as_ref()
            .and_then(|time| u64::try_from(time.0.timestamp_millis()).ok())
            .is_none_or(|expires| expires < policy.expires_at_unix_ms)
    {
        return Err("invalid_capability");
    }
    Ok(())
}

fn load_exact(key: &KeyPair, exact: &ExactInput) -> Result<(PublicWebConfig, PublicWebBundle)> {
    let config: PublicWebConfig = private_fs::read_json(&exact.config_path)?;
    let bundle: PublicWebBundle = private_fs::read_json(&exact.bundle_path)?;
    if exact.owner != config.expected_owner
        || exact.owner != bundle.policy.owner
        || exact.owner_lock_dir != config.owner_lock_dir
        || exact.config_sha256 != config.sha256()
        || exact.policy_sha256 != bundle.policy.sha256()
        || exact.state_sha256 != bundle.state.sha256()
    {
        return Err("reviewed_input_mismatch");
    }
    validate(key, &config, &bundle)?;
    Ok((config, bundle))
}

pub(crate) fn renew(issuer: &Path, request_path: &Path, out: &Path) -> Result<Value> {
    let request: RenewRequest = private_fs::read_json(request_path)?;
    if request.schema != "astrid.public_web.provision.renew.v1" {
        return Err("invalid_renew_request");
    }
    let key = load_issuer(issuer)?;
    let (config, bundle) = load_exact(&key, &request.exact)?;
    lock_location(&config.owner_lock_dir, issuer, out)?;
    disjoint(issuer, out)?;
    disjoint(config.policy_path.parent().ok_or("invalid_config")?, out)?;
    let old_bytes = private_fs::read(&request.approval_path)?;
    if sha(&old_bytes) != request.approval_file_sha256 {
        return Err("reviewed_approval_mismatch");
    }
    let old: PublicWebApprovalV2 =
        serde_json::from_slice(&old_bytes).map_err(|_| "invalid_approval")?;
    // Verify the original signature at its issue time, not by relaxing expiry in
    // the installer. Renewal cannot adopt a different installed or pending head.
    if old.approved_at_unix_ms > now_ms()?
        || !old.verifies_for(
            &bundle.policy,
            &bundle.state,
            &config.sha256(),
            old.approved_at_unix_ms,
        )
        || old.signer_public_key_hex != key.export_public_key().to_hex()
    {
        return Err("invalid_prior_approval");
    }
    let approval = approval_for(
        &key,
        &bundle,
        &config,
        old.expected_previous_floor_sha256,
        request.approval_audit_ref,
    )?;
    write_packet(
        out,
        &config,
        &bundle,
        &approval,
        &common_files(&config, &bundle, &approval)?,
    )
}

pub(crate) fn revoke(issuer: &Path, request_path: &Path, out: &Path) -> Result<Value> {
    let request: RevokeRequest = private_fs::read_json(request_path)?;
    if request.schema != "astrid.public_web.provision.revoke.v1" {
        return Err("invalid_revoke_request");
    }
    let key = load_issuer(issuer)?;
    let (config, mut bundle) = load_exact(&key, &request.exact)?;
    lock_location(&config.owner_lock_dir, issuer, out)?;
    disjoint(issuer, out)?;
    disjoint(config.policy_path.parent().ok_or("invalid_config")?, out)?;
    let current =
        astrid_public_web::status(&config).map_err(|_| "installed_authority_unavailable")?;
    if current.floor_sha256 != request.expected_previous_floor_sha256
        || current.state_revision != request.expected_state_revision
        || current.state_sha256 != request.exact.state_sha256
        || current.policy_sha256 != request.exact.policy_sha256
        || current.status != PublicWebPolicyStatusV2::Active
    {
        return Err("installed_head_conflict");
    }
    bundle.state.status = PublicWebPolicyStatusV2::Revoked;
    bundle.state.revision = current
        .state_revision
        .checked_add(1)
        .ok_or("revision_overflow")?;
    bundle.state.updated_at_unix_ms = now_ms()?;
    bundle.state.signer_public_key_hex = key.export_public_key().to_hex();
    sign_state(&key, &mut bundle.state)?;
    let approval = approval_for(
        &key,
        &bundle,
        &config,
        Some(request.expected_previous_floor_sha256),
        request.approval_audit_ref,
    )?;
    astrid_public_web::preview_mutation(&config, &request.exact.owner, &bundle, &approval)
        .map_err(|_| "revocation_preview_refused")?;
    write_packet(
        out,
        &config,
        &bundle,
        &approval,
        &common_files(&config, &bundle, &approval)?,
    )
}

fn common_files(
    config: &PublicWebConfig,
    bundle: &PublicWebBundle,
    approval: &PublicWebApprovalV2,
) -> Result<BTreeMap<&'static str, Vec<u8>>> {
    Ok(BTreeMap::from([
        ("config.json", encoded(config)?),
        ("bundle.json", encoded(bundle)?),
        ("runtime-owner.json", encoded(&bundle.policy.owner)?),
        ("approval.json", encoded(approval)?),
    ]))
}

fn write_packet(
    out: &Path,
    config: &PublicWebConfig,
    bundle: &PublicWebBundle,
    approval: &PublicWebApprovalV2,
    files: &BTreeMap<&str, Vec<u8>>,
) -> Result<Value> {
    let artifacts: BTreeMap<_, _> = files
        .iter()
        .map(|(name, bytes)| {
            (
                *name,
                json!({"path":out.join(name), "file_sha256":sha(bytes)}),
            )
        })
        .collect();
    let receipt = json!({
        "schema":"astrid.public_web.provision.receipt.v1", "status":"prepared_not_installed",
        "operation":approval.operation, "artifacts":artifacts,
        "issuer_public_key_hex":bundle.policy.issuer_public_key_hex,
        "owner_sha256":bundle.policy.owner.lock_identity_sha256(),
        "policy_sha256":bundle.policy.sha256(), "state_sha256":bundle.state.sha256(),
        "config_sha256":config.sha256(), "approval_sha256":approval.sha256(),
        "state_revision":bundle.state.revision,
        "expected_previous_floor_sha256":approval.expected_previous_floor_sha256,
        "policy_expires_at_unix_ms":bundle.policy.expires_at_unix_ms,
        "approval_expires_at_unix_ms":approval.expires_at_unix_ms,
        "planned_trust_path":config.delegation_root.join("trust.json"),
        "installed":false, "network_used":false,
    });
    let directory = Directory::create(out)?;
    for (name, bytes) in files {
        directory.write_new(name, bytes)?;
    }
    // Written last: absence of this receipt means the packet is incomplete.
    directory.write_json("receipt.json", &receipt)?;
    Ok(receipt)
}

#[cfg(test)]
#[path = "tests.rs"]
mod tests;
