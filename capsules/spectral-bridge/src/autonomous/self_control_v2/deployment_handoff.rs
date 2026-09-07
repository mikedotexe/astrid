use std::fs::{self, File};
use std::path::{Path, PathBuf};

use crate::deployment::{DeploymentBinding, read_bound_manifest};
use ed25519_dalek::{Signature, Signer as _, VerifyingKey};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use sha2::{Digest as _, Sha256};

use super::{
    OwnerSigner, RuntimeStateV2, SAFETY_SUPERVISOR, StoredOwnerIdentityV1, TARGET_BEING, key_id,
    load_trust, persist_state, read_json, read_validated_state, sha256_json, write_owner_json,
};

const HANDOFF_SCHEMA: &str = "astrid.self_control.deployment_handoff.v1";
const APPLIED_SCHEMA: &str = "astrid.self_control.deployment_handoff_applied.v1";
const HANDOFF_AUTHORITY: &str = "state_lineage_only_no_control_authority";
const HANDOFF_TTL_MS: u64 = 900_000;
const PENDING_FILENAME: &str = "deployment_handoff.pending.json";

pub(in crate::autonomous::runtime) fn inspect_startup() -> Result<Value, String> {
    startup_current(false)
}

pub(in crate::autonomous::runtime) fn apply_startup() -> Result<Value, String> {
    startup_current(true)
}

fn startup_current(apply: bool) -> Result<Value, String> {
    startup_at(
        &super::default_root(),
        &current_manifest_path(),
        &std::env::current_exe().map_err(|error| error.to_string())?,
        super::now_unix_ms(),
        apply,
    )
}

fn startup_at(
    root: &Path,
    manifest: &Path,
    executable: &Path,
    now: u64,
    apply: bool,
) -> Result<Value, String> {
    let binding = read_bound_manifest(manifest, executable)?;
    let mut state = read_validated_state(root)?.ok_or_else(|| {
        "staged activation requires existing valid self-control state".to_string()
    })?;
    if apply {
        if state.deployment_identity != binding.identity {
            if !consume_pending_at(
                root,
                manifest,
                executable,
                &mut state,
                &binding.identity,
                now,
            )? {
                return Err(
                    "staged startup requires an exact signed deployment handoff".to_string()
                );
            }
        } else if let Some(pending) = read_json(&root.join(PENDING_FILENAME))? {
            finalize_completed_at(
                root,
                manifest,
                executable,
                &state,
                &binding.identity,
                now,
                pending,
            )?;
        }
    }
    Ok(
        json!({"schema":"bridge_self_control_startup_v1", "state_integrity_verified":true,
        "state_deployment_identity":state.deployment_identity, "target_deployment_identity":binding.identity,
        "state_sha256":sha256_json(&state)?, "state_targets_this_binary":state.deployment_identity == binding.identity,
        "startup_gate_applied":apply, "authority":HANDOFF_AUTHORITY}),
    )
}

#[derive(Clone, Debug, Deserialize, Serialize)]
struct SignedDeploymentHandoffV1 {
    schema: String,
    handoff_id: String,
    target_being: String,
    from_deployment_identity: String,
    to_deployment_identity: String,
    from_state_sha256: String,
    target_manifest_sha256: String,
    target_binary_sha256: String,
    operator_actor: String,
    operator_ack_sha256: String,
    prepared_at_unix_ms: u64,
    expires_at_unix_ms: u64,
    signer_being: String,
    signer_key_id: String,
    signer_public_key_hex: String,
    authority: String,
    signature_hex: String,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
struct AppliedDeploymentHandoffV1 {
    schema: String,
    handoff: SignedDeploymentHandoffV1,
    result_state_sha256: String,
    consumed_at_unix_ms: u64,
    recovered_after_state_persist: bool,
    state_fields_preserved_except_deployment_identity: bool,
    authority: String,
}

impl SignedDeploymentHandoffV1 {
    fn signing_bytes(&self) -> Result<Vec<u8>, String> {
        let mut unsigned = self.clone();
        unsigned.signature_hex.clear();
        serde_json::to_vec(&unsigned)
            .map_err(|error| format!("encode deployment handoff signing statement: {error}"))
    }

    fn expected_id(&self) -> Result<String, String> {
        let digest = sha256_bytes(
            serde_json::to_vec(&json!({
                "from_deployment_identity": self.from_deployment_identity,
                "to_deployment_identity": self.to_deployment_identity,
                "from_state_sha256": self.from_state_sha256,
                "target_manifest_sha256": self.target_manifest_sha256,
                "operator_actor": self.operator_actor,
                "operator_ack_sha256": self.operator_ack_sha256,
                "prepared_at_unix_ms": self.prepared_at_unix_ms,
            }))
            .map_err(|error| format!("encode deployment handoff identity: {error}"))?
            .as_slice(),
        );
        Ok(format!("astrid-self-control-handoff:{}", &digest[..32]))
    }
}

pub(super) fn prepare_for_current(
    root: &Path,
    operator_actor: &str,
    operator_ack: &str,
    now: u64,
) -> Result<Value, String> {
    let manifest_path = current_manifest_path();
    let executable_path = std::env::current_exe()
        .map_err(|error| format!("resolve current bridge executable: {error}"))?;
    prepare_at(
        root,
        &manifest_path,
        &executable_path,
        operator_actor,
        operator_ack,
        now,
    )
}

fn prepare_at(
    root: &Path,
    manifest_path: &Path,
    executable_path: &Path,
    operator_actor: &str,
    operator_ack: &str,
    now: u64,
) -> Result<Value, String> {
    validate_operator_text(operator_actor, "operator actor", 128)?;
    validate_operator_text(operator_ack, "operator acknowledgement", 1_024)?;
    let binding = read_bound_manifest(manifest_path, executable_path)?;
    let state = read_validated_state(root)?
        .ok_or_else(|| "Astrid self-control state is missing; no handoff is needed".to_string())?;
    if state.deployment_identity == binding.identity {
        return Err("Astrid self-control state already targets this deployment".to_string());
    }
    let from_state_sha256 = sha256_json(&state)?;
    let pending_path = root.join(PENDING_FILENAME);
    if let Some(existing) = read_json::<SignedDeploymentHandoffV1>(&pending_path)? {
        verify_handoff(root, &existing, &binding, now, true)?;
        if existing.from_deployment_identity == state.deployment_identity
            && existing.to_deployment_identity == binding.identity
            && existing.from_state_sha256 == from_state_sha256
            && existing.operator_actor == operator_actor
            && existing.operator_ack_sha256 == sha256_bytes(operator_ack.as_bytes())
        {
            return serde_json::to_value(existing)
                .map_err(|error| format!("encode existing deployment handoff: {error}"));
        }
        return Err("a different unconsumed deployment handoff already exists".to_string());
    }

    let signer = load_existing_safety_signer(root)?;
    let mut handoff = SignedDeploymentHandoffV1 {
        schema: HANDOFF_SCHEMA.to_string(),
        handoff_id: String::new(),
        target_being: TARGET_BEING.to_string(),
        from_deployment_identity: state.deployment_identity,
        to_deployment_identity: binding.identity,
        from_state_sha256,
        target_manifest_sha256: binding.manifest_sha256,
        target_binary_sha256: binding.binary_sha256,
        operator_actor: operator_actor.to_string(),
        operator_ack_sha256: sha256_bytes(operator_ack.as_bytes()),
        prepared_at_unix_ms: now,
        expires_at_unix_ms: now
            .checked_add(HANDOFF_TTL_MS)
            .ok_or_else(|| "deployment handoff expiry overflow".to_string())?,
        signer_being: SAFETY_SUPERVISOR.to_string(),
        signer_key_id: signer.key_id.clone(),
        signer_public_key_hex: signer.public_key_hex.clone(),
        authority: HANDOFF_AUTHORITY.to_string(),
        signature_hex: String::new(),
    };
    handoff.handoff_id = handoff.expected_id()?;
    handoff.signature_hex = hex::encode(
        signer
            .signing_key
            .sign(&handoff.signing_bytes()?)
            .to_bytes(),
    );
    verify_handoff(
        root,
        &handoff,
        &read_bound_manifest(manifest_path, executable_path)?,
        now,
        true,
    )?;
    write_owner_json(&pending_path, &handoff)?;
    serde_json::to_value(handoff)
        .map_err(|error| format!("encode prepared deployment handoff: {error}"))
}

pub(super) fn consume_pending_for_current(
    root: &Path,
    state: &mut RuntimeStateV2,
    deployment: &str,
    now: u64,
) -> Result<bool, String> {
    let manifest_path = current_manifest_path();
    let executable_path = std::env::current_exe()
        .map_err(|error| format!("resolve current bridge executable: {error}"))?;
    consume_pending_at(
        root,
        &manifest_path,
        &executable_path,
        state,
        deployment,
        now,
    )
}

fn consume_pending_at(
    root: &Path,
    manifest_path: &Path,
    executable_path: &Path,
    state: &mut RuntimeStateV2,
    deployment: &str,
    now: u64,
) -> Result<bool, String> {
    let Some(handoff) = read_json::<SignedDeploymentHandoffV1>(&root.join(PENDING_FILENAME))?
    else {
        return Ok(false);
    };
    let binding = read_bound_manifest(manifest_path, executable_path)?;
    verify_handoff(root, &handoff, &binding, now, true)?;
    if deployment != binding.identity
        || handoff.from_deployment_identity != state.deployment_identity
        || handoff.to_deployment_identity != deployment
        || handoff.from_state_sha256 != sha256_json(state)?
    {
        return Err(
            "deployment handoff does not bind the exact persisted state transition".to_string(),
        );
    }

    state.deployment_identity = deployment.to_string();
    persist_state(root, state)?;
    write_applied(root, state, handoff, now, false)?;
    Ok(true)
}

pub(super) fn finalize_completed_for_current(
    root: &Path,
    state: &RuntimeStateV2,
    deployment: &str,
    now: u64,
) -> Result<(), String> {
    let Some(handoff) = read_json::<SignedDeploymentHandoffV1>(&root.join(PENDING_FILENAME))?
    else {
        return Ok(());
    };
    let manifest_path = current_manifest_path();
    let executable_path = std::env::current_exe()
        .map_err(|error| format!("resolve current bridge executable: {error}"))?;
    finalize_completed_at(
        root,
        &manifest_path,
        &executable_path,
        state,
        deployment,
        now,
        handoff,
    )
}

fn finalize_completed_at(
    root: &Path,
    manifest_path: &Path,
    executable_path: &Path,
    state: &RuntimeStateV2,
    deployment: &str,
    now: u64,
    handoff: SignedDeploymentHandoffV1,
) -> Result<(), String> {
    let binding = read_bound_manifest(manifest_path, executable_path)?;
    verify_handoff(root, &handoff, &binding, now, false)?;
    if deployment != binding.identity
        || handoff.to_deployment_identity != deployment
        || state.deployment_identity != deployment
    {
        return Err(
            "pending deployment handoff targets a different current deployment".to_string(),
        );
    }
    let mut prior = state.clone();
    prior
        .deployment_identity
        .clone_from(&handoff.from_deployment_identity);
    if sha256_json(&prior)? != handoff.from_state_sha256 {
        return Err("deployment handoff cannot prove state preservation after restart".to_string());
    }
    write_applied(root, state, handoff, now, true)
}

fn write_applied(
    root: &Path,
    state: &RuntimeStateV2,
    handoff: SignedDeploymentHandoffV1,
    now: u64,
    recovered_after_state_persist: bool,
) -> Result<(), String> {
    let suffix = handoff
        .handoff_id
        .strip_prefix("astrid-self-control-handoff:")
        .ok_or_else(|| "deployment handoff id is malformed".to_string())?;
    let applied_path = root
        .join("deployment_handoffs/applied")
        .join(format!("{suffix}.json"));
    let applied = AppliedDeploymentHandoffV1 {
        schema: APPLIED_SCHEMA.to_string(),
        handoff,
        result_state_sha256: sha256_json(state)?,
        consumed_at_unix_ms: now,
        recovered_after_state_persist,
        state_fields_preserved_except_deployment_identity: true,
        authority: HANDOFF_AUTHORITY.to_string(),
    };
    match read_json::<AppliedDeploymentHandoffV1>(&applied_path)? {
        Some(existing)
            if existing.handoff.handoff_id != applied.handoff.handoff_id
                || existing.result_state_sha256 != applied.result_state_sha256 =>
        {
            return Err(
                "applied deployment handoff receipt conflicts with existing evidence".to_string(),
            );
        },
        Some(_) => {},
        None => write_owner_json(&applied_path, &applied)?,
    }
    remove_pending(root)
}

fn remove_pending(root: &Path) -> Result<(), String> {
    let path = root.join(PENDING_FILENAME);
    match fs::remove_file(&path) {
        Ok(()) => File::open(root)
            .and_then(|directory| directory.sync_all())
            .map_err(|error| format!("sync {} after handoff consumption: {error}", root.display())),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(format!("remove consumed {}: {error}", path.display())),
    }
}

fn verify_handoff(
    root: &Path,
    handoff: &SignedDeploymentHandoffV1,
    binding: &DeploymentBinding,
    now: u64,
    enforce_expiry: bool,
) -> Result<(), String> {
    if handoff.schema != HANDOFF_SCHEMA
        || handoff.target_being != TARGET_BEING
        || handoff.signer_being != SAFETY_SUPERVISOR
        || handoff.authority != HANDOFF_AUTHORITY
        || handoff.from_deployment_identity == handoff.to_deployment_identity
        || handoff.to_deployment_identity != binding.identity
        || handoff.target_manifest_sha256 != binding.manifest_sha256
        || handoff.target_binary_sha256 != binding.binary_sha256
        || handoff.handoff_id != handoff.expected_id()?
        || handoff.expires_at_unix_ms
            != handoff
                .prepared_at_unix_ms
                .checked_add(HANDOFF_TTL_MS)
                .ok_or_else(|| "deployment handoff expiry overflow".to_string())?
        || !is_sha256(&handoff.from_state_sha256)
        || !is_sha256(&handoff.operator_ack_sha256)
        || handoff.operator_actor.is_empty()
    {
        return Err("deployment handoff is malformed or bound to different evidence".to_string());
    }
    if handoff.prepared_at_unix_ms > now || (enforce_expiry && now > handoff.expires_at_unix_ms) {
        return Err("deployment handoff is not current".to_string());
    }

    let trust = load_trust(root)?;
    let pinned = trust
        .pinned_public_keys
        .get(SAFETY_SUPERVISOR)
        .ok_or_else(|| "safety supervisor deployment handoff key is not pinned".to_string())?;
    if handoff.signer_public_key_hex != *pinned
        || handoff.signer_key_id != key_id(&handoff.signer_public_key_hex)
    {
        return Err("deployment handoff signer key is not trusted".to_string());
    }
    let public_key: [u8; 32] = hex::decode(&handoff.signer_public_key_hex)
        .map_err(|error| format!("decode deployment handoff public key: {error}"))?
        .try_into()
        .map_err(|_| "deployment handoff public key has the wrong length".to_string())?;
    let signature: [u8; 64] = hex::decode(&handoff.signature_hex)
        .map_err(|error| format!("decode deployment handoff signature: {error}"))?
        .try_into()
        .map_err(|_| "deployment handoff signature has the wrong length".to_string())?;
    VerifyingKey::from_bytes(&public_key)
        .map_err(|error| format!("parse deployment handoff public key: {error}"))?
        .verify_strict(
            &handoff.signing_bytes()?,
            &Signature::from_bytes(&signature),
        )
        .map_err(|_| "deployment handoff signature is invalid".to_string())
}

fn load_existing_safety_signer(root: &Path) -> Result<OwnerSigner, String> {
    let stored = read_json::<StoredOwnerIdentityV1>(&root.join("safety_identity.json"))?
        .ok_or_else(|| {
            "safety supervisor identity is missing; refusing deployment handoff".to_string()
        })?;
    let signer = OwnerSigner::from_stored(stored, SAFETY_SUPERVISOR)?;
    let trust = load_trust(root)?;
    if trust.pinned_public_keys.get(SAFETY_SUPERVISOR) != Some(&signer.public_key_hex) {
        return Err("safety supervisor deployment handoff key is not pinned".to_string());
    }
    Ok(signer)
}

fn current_manifest_path() -> PathBuf {
    crate::deployment::manifest_path(crate::paths::bridge_paths().bridge_workspace())
}

fn validate_operator_text(value: &str, label: &str, max_len: usize) -> Result<(), String> {
    if value.trim() != value
        || value.is_empty()
        || value.len() > max_len
        || value.chars().any(char::is_control)
    {
        return Err(format!("deployment handoff {label} is empty or malformed"));
    }
    Ok(())
}

fn is_sha256(value: &str) -> bool {
    value.len() == 64 && value.bytes().all(|byte| byte.is_ascii_hexdigit())
}

fn sha256_bytes(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}

#[cfg(test)]
mod tests {
    use crate::deployment::{MANIFEST_AUTHORITY, MANIFEST_SCHEMA};
    use std::fs;

    use tempfile::TempDir;

    use super::*;
    use crate::autonomous::self_control_v2::{
        RuntimeStateV2, SelfControlValuesV2, load_or_provision_identity,
    };

    fn fixture(now: u64) -> (TempDir, PathBuf, PathBuf, RuntimeStateV2, DeploymentBinding) {
        let root = TempDir::new().unwrap();
        load_or_provision_identity(root.path(), now).unwrap();
        let binary = root.path().join("spectral-bridge-server");
        fs::write(&binary, b"fixture bridge binary").unwrap();
        let binary_sha256 = sha256_bytes(&fs::read(&binary).unwrap());
        let manifest = root.path().join("spectral-bridge.json");
        write_owner_json(
            &manifest,
            &json!({
                "schema": MANIFEST_SCHEMA,
                "authority": MANIFEST_AUTHORITY,
                "repository": {
                    "available": true,
                    "head": "0123456789abcdef0123456789abcdef01234567"
                },
                "artifacts": {
                    "spectral-bridge": {
                        "exists": true,
                        "path": binary.canonicalize().unwrap(),
                        "sha256": binary_sha256
                    }
                }
            }),
        )
        .unwrap();
        let binding = read_bound_manifest(&manifest, &binary).unwrap();
        let mut state = RuntimeStateV2::new("astrid:prior:bridge:prior".to_string());
        state.preferences = SelfControlValuesV2 {
            conversation_temperature: Some(1.0),
            response_token_limit: Some(768),
            peer_breathing_coupled: Some(false),
            ..SelfControlValuesV2::default()
        };
        state
            .revision_by_family
            .insert("conversation".to_string(), 4);
        persist_state(root.path(), &state).unwrap();
        (root, manifest, binary, state, binding)
    }

    fn prepare_fixture(
        root: &Path,
        manifest: &Path,
        binary: &Path,
        now: u64,
    ) -> SignedDeploymentHandoffV1 {
        prepare_at(
            root,
            manifest,
            binary,
            "mike-operator",
            "approved exact deployment-lineage handoff",
            now,
        )
        .unwrap();
        read_json(&root.join(PENDING_FILENAME)).unwrap().unwrap()
    }

    #[test]
    fn signed_handoff_preserves_exact_state_except_deployment_identity() {
        let now = 100_000;
        let (root, manifest, binary, mut state, binding) = fixture(now);
        let canonical = root
            .path()
            .join("deployment_manifests/spectral-bridge.json");
        fs::create_dir_all(canonical.parent().unwrap()).unwrap();
        fs::write(&canonical, b"old live manifest stays published").unwrap();
        let before = state.clone();
        prepare_fixture(root.path(), &manifest, &binary, now);

        assert!(
            consume_pending_at(
                root.path(),
                &manifest,
                &binary,
                &mut state,
                &binding.identity,
                now + 1,
            )
            .unwrap()
        );
        assert_eq!(state.deployment_identity, binding.identity);
        assert_eq!(
            fs::read(canonical).unwrap(),
            b"old live manifest stays published"
        );
        let mut restored = state.clone();
        restored
            .deployment_identity
            .clone_from(&before.deployment_identity);
        assert_eq!(
            sha256_json(&restored).unwrap(),
            sha256_json(&before).unwrap()
        );
        assert!(!root.path().join(PENDING_FILENAME).exists());
        assert_eq!(
            fs::read_dir(root.path().join("deployment_handoffs/applied"))
                .unwrap()
                .count(),
            1
        );
    }

    #[test]
    fn handoff_rejects_state_or_signature_tampering() {
        let now = 200_000;
        let (root, manifest, binary, mut state, binding) = fixture(now);
        let mut handoff = prepare_fixture(root.path(), &manifest, &binary, now);
        state.preferences.response_token_limit = Some(640);
        assert!(
            consume_pending_at(
                root.path(),
                &manifest,
                &binary,
                &mut state,
                &binding.identity,
                now + 1,
            )
            .unwrap_err()
            .contains("exact persisted state")
        );

        handoff.signature_hex.replace_range(0..2, "00");
        write_owner_json(&root.path().join(PENDING_FILENAME), &handoff).unwrap();
        assert!(
            verify_handoff(root.path(), &handoff, &binding, now + 1, true)
                .unwrap_err()
                .contains("signature")
        );
    }

    #[test]
    fn handoff_rejects_expiry_and_binary_manifest_mismatch() {
        let now = 300_000;
        let (root, manifest, binary, _state, binding) = fixture(now);
        let handoff = prepare_fixture(root.path(), &manifest, &binary, now);
        assert!(
            verify_handoff(
                root.path(),
                &handoff,
                &binding,
                now + HANDOFF_TTL_MS + 1,
                true,
            )
            .unwrap_err()
            .contains("not current")
        );

        fs::write(&binary, b"different binary").unwrap();
        assert!(
            read_bound_manifest(&manifest, &binary)
                .unwrap_err()
                .contains("hash does not match")
        );
    }

    #[test]
    fn preparation_is_idempotent_but_cannot_replace_pending_evidence() {
        let now = 350_000;
        let (root, manifest, binary, _state, _binding) = fixture(now);
        let first = prepare_at(
            root.path(),
            &manifest,
            &binary,
            "mike-operator",
            "approved exact deployment-lineage handoff",
            now,
        )
        .unwrap();
        let retry = prepare_at(
            root.path(),
            &manifest,
            &binary,
            "mike-operator",
            "approved exact deployment-lineage handoff",
            now + 1,
        )
        .unwrap();
        assert_eq!(first, retry);

        assert!(
            prepare_at(
                root.path(),
                &manifest,
                &binary,
                "mike-operator",
                "different acknowledgement",
                now + 1,
            )
            .unwrap_err()
            .contains("different unconsumed")
        );
    }

    #[test]
    fn completed_state_can_finalize_receipt_after_a_crash_boundary() {
        let now = 400_000;
        let (root, manifest, binary, mut state, binding) = fixture(now);
        let handoff = prepare_fixture(root.path(), &manifest, &binary, now);
        state.deployment_identity.clone_from(&binding.identity);
        persist_state(root.path(), &state).unwrap();

        finalize_completed_at(
            root.path(),
            &manifest,
            &binary,
            &state,
            &binding.identity,
            now + HANDOFF_TTL_MS + 1,
            handoff,
        )
        .unwrap();
        assert!(!root.path().join(PENDING_FILENAME).exists());
        let applied_path = fs::read_dir(root.path().join("deployment_handoffs/applied"))
            .unwrap()
            .next()
            .unwrap()
            .unwrap()
            .path();
        let applied: AppliedDeploymentHandoffV1 = read_json(&applied_path).unwrap().unwrap();
        assert!(applied.recovered_after_state_persist);
        assert!(applied.state_fields_preserved_except_deployment_identity);
    }

    #[test]
    fn preparation_requires_explicit_bounded_operator_evidence() {
        let now = 500_000;
        let (root, manifest, binary, _state, _binding) = fixture(now);
        assert!(
            prepare_at(root.path(), &manifest, &binary, "", "approved", now)
                .unwrap_err()
                .contains("operator actor")
        );
        assert!(
            prepare_at(root.path(), &manifest, &binary, "mike", "", now)
                .unwrap_err()
                .contains("operator acknowledgement")
        );
    }

    #[test]
    fn staged_startup_refuses_missing_handoff_before_any_rebind() {
        let (root, manifest, binary, before, _) = fixture(500_000);
        let inspection = startup_at(root.path(), &manifest, &binary, 500_000, false).unwrap();
        assert_eq!(inspection["state_targets_this_binary"], false);
        assert!(
            startup_at(root.path(), &manifest, &binary, 500_000, true)
                .unwrap_err()
                .contains("exact signed")
        );
        assert_eq!(
            sha256_json(&read_validated_state(root.path()).unwrap().unwrap()).unwrap(),
            sha256_json(&before).unwrap()
        );
        prepare_fixture(root.path(), &manifest, &binary, 500_000);
        let applied = startup_at(root.path(), &manifest, &binary, 500_001, true).unwrap();
        assert_eq!(applied["state_targets_this_binary"], true);
        assert!(startup_at(root.path(), &manifest, &binary, 500_002, true).is_ok());
    }
}
