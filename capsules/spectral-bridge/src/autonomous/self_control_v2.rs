use std::collections::{BTreeMap, BTreeSet};
use std::fs::{self, File, OpenOptions};
use std::io::{self, Write as _};
#[cfg(unix)]
use std::os::unix::fs::{OpenOptionsExt as _, PermissionsExt as _};
use std::path::{Path, PathBuf};
use std::sync::{Mutex, OnceLock};
use std::time::{SystemTime, UNIX_EPOCH};

use astrid_minime_protocol::{
    OwnerResearchPayloadKindV1, SELF_CONTROL_AUTHORITY_PROOF_SCHEMA_V1,
    SELF_CONTROL_COMMAND_SCHEMA_V2, SELF_CONTROL_INTENT_SCHEMA_V2, SELF_CONTROL_RECEIPT_SCHEMA_V2,
    SIGNED_OWNER_RESEARCH_RECEIPT_SCHEMA_V1, SelfControlActionV2, SelfControlAuthorityClassV2,
    SelfControlAuthorityProofV1, SelfControlCommandV2, SelfControlDurabilityV2,
    SelfControlFamilyV2, SelfControlIntentV2, SelfControlReceiptStatusV2, SelfControlReceiptV2,
    SelfControlSourceIdentityV1, SelfControlValuesV2, SignedOwnerResearchReceiptV1,
    canonical_self_control_intent_sha256, public_key_fingerprint_sha256,
};
use ed25519_dalek::{Signer as _, SigningKey};
use rand::rngs::OsRng;
use serde::{Deserialize, Serialize, de::DeserializeOwned};
use serde_json::Value;
use sha2::{Digest as _, Sha256};

use super::state::ConversationState;

#[path = "self_control_v2/deployment_handoff.rs"]
mod deployment_handoff;

const TARGET_BEING: &str = "astrid";
const SAFETY_SUPERVISOR: &str = "safety_supervisor";
const IDENTITY_SCHEMA: &str = "astrid.self_control.owner_identity.v1";
const TRUST_SCHEMA: &str = "astrid.self_control.trust_store.v1";
const STATE_SCHEMA: &str = "astrid.self_control.runtime_state.v2";
const STATE_ENVELOPE_SCHEMA: &str = "astrid.self_control.runtime_state_envelope.v1";
const COMMAND_TTL_MS: u64 = 30_000;
const MAX_RECEIPTS: usize = 4_096;
const MAX_ACTIVE_CONTROLS: usize = 512;
const MAX_REPLAY_RECORDS: usize = 4_096;
pub(in crate::autonomous) const MIN_ACTION_CARRYING_RESPONSE_TOKENS: u32 = 512;
const LEGACY_PRECISE_RESPONSE_TOKENS: u32 = 128;

static OPERATION_LOCK: OnceLock<Mutex<()>> = OnceLock::new();

#[derive(Clone, Debug, Serialize, Deserialize)]
struct StoredOwnerIdentityV1 {
    schema: String,
    being: String,
    key_id: String,
    public_key_hex: String,
    signing_key_seed_hex: String,
    created_at_unix_ms: u64,
}

#[derive(Clone)]
struct OwnerSigner {
    being: String,
    key_id: String,
    public_key_hex: String,
    signing_key: SigningKey,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
struct TrustStoreV1 {
    schema: String,
    target_being: String,
    pinned_public_keys: BTreeMap<String, String>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
struct ActiveControlV2 {
    family: SelfControlFamilyV2,
    intent_id: String,
    command_id: String,
    receipt_id: String,
    #[serde(default)]
    revision: u64,
    durability: SelfControlDurabilityV2,
    control_expires_at_unix_ms: Option<u64>,
    applied_values: SelfControlValuesV2,
    previous_values: SelfControlValuesV2,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
struct IdempotencyRecordV2 {
    command_sha256: String,
    receipt: SelfControlReceiptV2,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
struct RuntimeStateV2 {
    schema: String,
    target_being: String,
    deployment_identity: String,
    revision_by_family: BTreeMap<String, u64>,
    preferences: SelfControlValuesV2,
    active_controls: BTreeMap<String, ActiveControlV2>,
    seen_nonces: BTreeSet<String>,
    idempotency: BTreeMap<String, IdempotencyRecordV2>,
    receipts: Vec<SelfControlReceiptV2>,
    clamp_saturation_by_family: BTreeMap<String, u8>,
}

impl RuntimeStateV2 {
    fn new(deployment_identity: String) -> Self {
        Self {
            schema: STATE_SCHEMA.to_string(),
            target_being: TARGET_BEING.to_string(),
            deployment_identity,
            revision_by_family: BTreeMap::new(),
            preferences: SelfControlValuesV2::default(),
            active_controls: BTreeMap::new(),
            seen_nonces: BTreeSet::new(),
            idempotency: BTreeMap::new(),
            receipts: Vec::new(),
            clamp_saturation_by_family: BTreeMap::new(),
        }
    }

    fn is_pristine_for_deployment_rebind(&self) -> bool {
        self.revision_by_family.is_empty()
            && self.preferences == SelfControlValuesV2::default()
            && self.active_controls.is_empty()
            && self.seen_nonces.is_empty()
            && self.idempotency.is_empty()
            && self.receipts.is_empty()
            && self.clamp_saturation_by_family.is_empty()
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
struct RuntimeStateEnvelopeV1 {
    schema: String,
    state_sha256: String,
    state: RuntimeStateV2,
}

#[derive(Clone, Debug, Serialize)]
pub(crate) struct AstridSelfControlStatusV2 {
    pub schema: &'static str,
    pub target_being: &'static str,
    pub deployment_identity: String,
    pub revision_by_family: BTreeMap<String, u64>,
    pub active_control_count: usize,
    pub active_controls: Vec<AstridSelfControlActiveV2>,
    pub receipt_count: usize,
    pub owner_key_id: String,
    pub felt_effect_established: bool,
}

#[derive(Clone, Debug, Serialize)]
pub(crate) struct AstridSelfControlActiveV2 {
    pub intent_id: String,
    pub family: SelfControlFamilyV2,
    pub revision: u64,
    pub durability: SelfControlDurabilityV2,
    pub control_expires_at_unix_ms: Option<u64>,
}

pub(in crate::autonomous) fn issue_standing(
    conv: &mut ConversationState,
    family: SelfControlFamilyV2,
    values: SelfControlValuesV2,
    source_action: &str,
) -> Result<SelfControlReceiptV2, String> {
    issue_at(
        &default_root(),
        conv,
        family,
        SelfControlDurabilityV2::Standing,
        values,
        0,
        source_action,
        now_unix_ms(),
    )
}

pub(in crate::autonomous) fn issue_one_shot(
    conv: &mut ConversationState,
    family: SelfControlFamilyV2,
    values: SelfControlValuesV2,
    source_action: &str,
) -> Result<SelfControlReceiptV2, String> {
    issue_at(
        &default_root(),
        conv,
        family,
        SelfControlDurabilityV2::OneShot,
        values,
        0,
        source_action,
        now_unix_ms(),
    )
}

pub(in crate::autonomous) fn issue_lease(
    conv: &mut ConversationState,
    family: SelfControlFamilyV2,
    values: SelfControlValuesV2,
    lease_secs: u64,
    source_action: &str,
) -> Result<SelfControlReceiptV2, String> {
    issue_at(
        &default_root(),
        conv,
        family,
        SelfControlDurabilityV2::Lease,
        values,
        lease_secs,
        source_action,
        now_unix_ms(),
    )
}

pub(in crate::autonomous) fn validate_owner_values(
    family: SelfControlFamilyV2,
    values: &SelfControlValuesV2,
) -> Result<(), String> {
    if !matches!(
        family,
        SelfControlFamilyV2::Conversation
            | SelfControlFamilyV2::SemanticContinuity
            | SelfControlFamilyV2::SemanticEmission
            | SelfControlFamilyV2::SensoryIntake
    ) {
        return Err("Astrid owner policy may target only locally owned families".to_string());
    }
    if values.field_count() == 0 || !values.is_well_formed() {
        return Err("Astrid owner policy values are empty, non-finite, or malformed".to_string());
    }
    if values.peer_breathing_coupled.is_some() || values.includes_shared_coupling() {
        return Err("owner policy cannot alter shared or peer coupling".to_string());
    }
    let unsupported = unsupported_fields(family, values);
    if !unsupported.is_empty() {
        return Err(format!(
            "Astrid owner policy fields are outside the selected family: {}",
            unsupported.join(",")
        ));
    }
    Ok(())
}

pub(in crate::autonomous) fn issue_lease_at_root(
    root: &Path,
    conv: &mut ConversationState,
    family: SelfControlFamilyV2,
    values: SelfControlValuesV2,
    lease_secs: u64,
    source_action: &str,
    now: u64,
) -> Result<SelfControlReceiptV2, String> {
    issue_at(
        root,
        conv,
        family,
        SelfControlDurabilityV2::Lease,
        values,
        lease_secs,
        source_action,
        now,
    )
}

pub(in crate::autonomous) fn safety_revert_active_receipt(
    conv: &mut ConversationState,
    related_receipt_id: &str,
    reason: &str,
) -> Result<SelfControlReceiptV2, String> {
    safety_action_at(
        &default_root(),
        conv,
        SelfControlActionV2::Revert,
        related_receipt_id,
        reason,
        now_unix_ms(),
    )
}

pub(in crate::autonomous) fn withdraw_active_receipt(
    conv: &mut ConversationState,
    related_receipt_id: &str,
    source_action: &str,
) -> Result<Option<SelfControlReceiptV2>, String> {
    withdraw_active_receipt_at_root(
        &default_root(),
        conv,
        related_receipt_id,
        source_action,
        now_unix_ms(),
    )
}

pub(in crate::autonomous) fn withdraw_active_receipt_at_root(
    root: &Path,
    conv: &mut ConversationState,
    related_receipt_id: &str,
    source_action: &str,
    now: u64,
) -> Result<Option<SelfControlReceiptV2>, String> {
    let related_intent_id = {
        let _guard = operation_guard()?;
        let deployment_identity = deployment_identity();
        let state = load_state(root, &deployment_identity)?;
        state
            .active_controls
            .values()
            .find(|active| active.receipt_id == related_receipt_id)
            .map(|active| active.intent_id.clone())
    };
    let Some(related_intent_id) = related_intent_id else {
        return Ok(None);
    };
    withdraw_at(root, conv, &related_intent_id, source_action, now).map(Some)
}

pub(in crate::autonomous) fn withdraw_at_root(
    root: &Path,
    conv: &mut ConversationState,
    related_intent_id: &str,
    source_action: &str,
    now: u64,
) -> Result<SelfControlReceiptV2, String> {
    withdraw_at(root, conv, related_intent_id, source_action, now)
}

pub(in crate::autonomous) fn resolve_standing_intent_at_root(
    root: &Path,
    selector: &str,
) -> Result<String, String> {
    let _guard = operation_guard()?;
    let deployment_identity = deployment_identity();
    let state = load_state(root, &deployment_identity)?;
    if selector != "latest" {
        let active = state
            .active_controls
            .get(selector)
            .ok_or_else(|| "Astrid standing self-control intent was not found".to_string())?;
        if active.durability != SelfControlDurabilityV2::Standing {
            return Err("SELF_CONTROL_WITHDRAW only targets standing preferences; use SELF_REGULATION_WITHDRAW for a lease".to_string());
        }
        return Ok(active.intent_id.clone());
    }
    state
        .active_controls
        .values()
        .filter(|active| active.durability == SelfControlDurabilityV2::Standing)
        .max_by_key(|active| active.revision)
        .map(|active| active.intent_id.clone())
        .ok_or_else(|| "no active Astrid standing self-control preference exists".to_string())
}

pub(in crate::autonomous) fn reconcile_at_root(
    root: &Path,
    conv: &mut ConversationState,
    now: u64,
) -> Result<Vec<SelfControlReceiptV2>, String> {
    reconcile_at(root, conv, now)
}

pub(in crate::autonomous) fn reconcile_if_present(
    conv: &mut ConversationState,
) -> Result<Vec<SelfControlReceiptV2>, String> {
    let root = default_root();
    if !root.join("state.json").exists() {
        return Ok(Vec::new());
    }
    reconcile_at(&root, conv, now_unix_ms())
}

pub(in crate::autonomous) fn status_at_root(
    root: &Path,
) -> Result<AstridSelfControlStatusV2, String> {
    let _guard = operation_guard()?;
    let signer = load_or_provision_identity(root, now_unix_ms())?;
    let deployment_identity = deployment_identity();
    let state = load_state(root, &deployment_identity)?;
    let mut active_controls = state
        .active_controls
        .values()
        .map(|active| AstridSelfControlActiveV2 {
            intent_id: active.intent_id.clone(),
            family: active.family,
            revision: active.revision,
            durability: active.durability,
            control_expires_at_unix_ms: active.control_expires_at_unix_ms,
        })
        .collect::<Vec<_>>();
    active_controls.sort_by_key(|active| active.revision);
    Ok(AstridSelfControlStatusV2 {
        schema: "astrid.self_control.status.v2",
        target_being: TARGET_BEING,
        deployment_identity,
        revision_by_family: state.revision_by_family,
        active_control_count: state.active_controls.len(),
        active_controls,
        receipt_count: state.receipts.len(),
        owner_key_id: signer.key_id,
        felt_effect_established: false,
    })
}

pub(in crate::autonomous) fn status() -> Result<AstridSelfControlStatusV2, String> {
    status_at_root(&default_root())
}

#[allow(clippy::too_many_arguments)]
pub(in crate::autonomous) fn sign_owner_research_receipt(
    receipt_id: String,
    payload_kind: OwnerResearchPayloadKindV1,
    payload_schema: String,
    payload_sha256: String,
    previous_receipt_sha256: Option<String>,
    emitted_at_unix_ms: u64,
) -> Result<SignedOwnerResearchReceiptV1, String> {
    let signer = load_or_provision_identity(&default_root(), emitted_at_unix_ms)?;
    let signer_public_key_fingerprint_sha256 =
        public_key_fingerprint_sha256(&signer.public_key_hex)
            .ok_or_else(|| "fingerprint Astrid owner research signing key".to_string())?;
    let mut receipt = SignedOwnerResearchReceiptV1 {
        schema: SIGNED_OWNER_RESEARCH_RECEIPT_SCHEMA_V1.to_string(),
        receipt_id,
        payload_kind,
        payload_schema,
        payload_sha256,
        owner_being: TARGET_BEING.to_string(),
        process_identity: process_identity(),
        deployment_identity: deployment_identity(),
        signer_public_key_hex: signer.public_key_hex.clone(),
        signer_public_key_fingerprint_sha256,
        previous_receipt_sha256,
        emitted_at_unix_ms,
        signature_hex: String::new(),
    };
    let signing_bytes = receipt
        .signing_bytes()
        .ok_or_else(|| "encode Astrid owner research signing statement".to_string())?;
    receipt.signature_hex = hex::encode(signer.signing_key.sign(&signing_bytes).to_bytes());
    if !receipt.is_well_formed() {
        return Err("Astrid owner research receipt failed signature validation".to_string());
    }
    Ok(receipt)
}

pub(in crate::autonomous) fn receipt_summary(receipt: &SelfControlReceiptV2) -> String {
    format!(
        "self_control_v2 receipt={} intent={} status={:?} revision={} clamped={} felt_effect_established=false",
        receipt.receipt_id,
        receipt.intent_id,
        receipt.status,
        receipt.resulting_revision,
        receipt.requested_values != receipt.clamped_values,
    )
}

pub(in crate::autonomous) fn apply_standing_action(
    conv: &mut ConversationState,
    family: SelfControlFamilyV2,
    values: SelfControlValuesV2,
    action: &str,
    summary: String,
) -> bool {
    match issue_standing(conv, family, values, action) {
        Ok(receipt) => {
            conv.push_receipt(action, vec![summary, receipt_summary(&receipt)]);
            true
        },
        Err(error) => {
            conv.push_receipt(
                action,
                vec![format!("self_control_v2 blocked without mutation: {error}")],
            );
            false
        },
    }
}

pub(in crate::autonomous) fn apply_one_shot_action(
    conv: &mut ConversationState,
    family: SelfControlFamilyV2,
    values: SelfControlValuesV2,
    action: &str,
    summary: String,
) -> bool {
    match issue_one_shot(conv, family, values, action) {
        Ok(receipt) => {
            conv.push_receipt(action, vec![summary, receipt_summary(&receipt)]);
            true
        },
        Err(error) => {
            conv.push_receipt(
                action,
                vec![format!("self_control_v2 blocked without mutation: {error}")],
            );
            false
        },
    }
}

#[allow(clippy::too_many_arguments)]
fn issue_at(
    root: &Path,
    conv: &mut ConversationState,
    family: SelfControlFamilyV2,
    durability: SelfControlDurabilityV2,
    values: SelfControlValuesV2,
    lease_secs: u64,
    source_action: &str,
    now: u64,
) -> Result<SelfControlReceiptV2, String> {
    let _guard = operation_guard()?;
    let signer = load_or_provision_identity(root, now)?;
    let deployment_identity = deployment_identity();
    let mut state = load_state(root, &deployment_identity)?;
    let rollback_receipts = reconcile_state(root, &mut state, conv, now)?;
    for receipt in rollback_receipts {
        append_receipt(root, &receipt)?;
    }

    let family_key = family_name(family).to_string();
    let expected_revision = state
        .revision_by_family
        .get(&family_key)
        .copied()
        .unwrap_or(0);
    let revision = expected_revision
        .checked_add(1)
        .ok_or_else(|| "Astrid self-control revision overflow".to_string())?;
    let entropy = rand::random::<u64>();
    let control_expires_at_unix_ms = match durability {
        SelfControlDurabilityV2::Lease => {
            if lease_secs == 0 {
                return Err("Astrid self-control lease duration must be positive".to_string());
            }
            Some(now.saturating_add(lease_secs.saturating_mul(1_000)))
        },
        SelfControlDurabilityV2::Standing | SelfControlDurabilityV2::OneShot => None,
    };
    let intent = SelfControlIntentV2 {
        schema: SELF_CONTROL_INTENT_SCHEMA_V2.to_string(),
        intent_id: format!("astrid:{family_key}:{now}:{entropy:016x}"),
        actor: SelfControlSourceIdentityV1 {
            being: TARGET_BEING.to_string(),
            process_identity: format!("astrid-autonomy:pid:{}", std::process::id()),
            deployment_identity: deployment_identity.clone(),
        },
        target_being: TARGET_BEING.to_string(),
        target_deployment_identity: deployment_identity,
        family,
        action: SelfControlActionV2::Set,
        durability,
        authority_class: SelfControlAuthorityClassV2::SelfOwned,
        authority_scope: format!("self_control.astrid.{family_key}"),
        revision,
        expected_revision,
        issued_at_unix_ms: now,
        command_expires_at_unix_ms: now.saturating_add(COMMAND_TTL_MS),
        control_expires_at_unix_ms,
        idempotency_key: format!("astrid:{family_key}:{now}:{entropy:016x}:idempotency"),
        values,
        related_intent_id: None,
        related_receipt_id: None,
        evidence_refs: vec![format!("astrid_action:{source_action}")],
        success_conditions: vec!["machine_receipt_matches_requested_revision".to_string()],
        stop_conditions: vec![
            "being_authored_withdrawal".to_string(),
            "non_finite_or_receipt_mismatch".to_string(),
            "repeated_clamp_saturation".to_string(),
        ],
    };
    let command = signer.sign(
        intent,
        format!("astrid-command:{family_key}:{now}:{entropy:016x}"),
        format!("astrid-nonce:{now}:{entropy:016x}"),
        now,
    )?;
    let trust = load_trust(root)?;
    let receipt = apply_command(root, &trust, &mut state, conv, command, now)?;
    persist_state(root, &state)?;
    append_receipt(root, &receipt)?;
    Ok(receipt)
}

fn withdraw_at(
    root: &Path,
    conv: &mut ConversationState,
    related_intent_id: &str,
    source_action: &str,
    now: u64,
) -> Result<SelfControlReceiptV2, String> {
    let _guard = operation_guard()?;
    let signer = load_or_provision_identity(root, now)?;
    let deployment_identity = deployment_identity();
    let mut state = load_state(root, &deployment_identity)?;
    let rollback_receipts = reconcile_state(root, &mut state, conv, now)?;
    for receipt in rollback_receipts {
        append_receipt(root, &receipt)?;
    }
    let active = state
        .active_controls
        .get(related_intent_id)
        .cloned()
        .ok_or_else(|| "Astrid self-control withdrawal target is not active".to_string())?;
    let family_key = family_name(active.family).to_string();
    let expected_revision = state
        .revision_by_family
        .get(&family_key)
        .copied()
        .unwrap_or(0);
    let revision = expected_revision
        .checked_add(1)
        .ok_or_else(|| "Astrid self-control revision overflow".to_string())?;
    let entropy = rand::random::<u64>();
    let intent = SelfControlIntentV2 {
        schema: SELF_CONTROL_INTENT_SCHEMA_V2.to_string(),
        intent_id: format!("astrid:{family_key}:withdraw:{now}:{entropy:016x}"),
        actor: SelfControlSourceIdentityV1 {
            being: TARGET_BEING.to_string(),
            process_identity: format!("astrid-autonomy:pid:{}", std::process::id()),
            deployment_identity: deployment_identity.clone(),
        },
        target_being: TARGET_BEING.to_string(),
        target_deployment_identity: deployment_identity,
        family: active.family,
        action: SelfControlActionV2::Withdraw,
        durability: SelfControlDurabilityV2::OneShot,
        authority_class: SelfControlAuthorityClassV2::SelfOwned,
        authority_scope: format!("self_control.astrid.{family_key}"),
        revision,
        expected_revision,
        issued_at_unix_ms: now,
        command_expires_at_unix_ms: now.saturating_add(COMMAND_TTL_MS),
        control_expires_at_unix_ms: None,
        idempotency_key: format!("astrid:{family_key}:withdraw:{now}:{entropy:016x}:idempotency"),
        values: SelfControlValuesV2::default(),
        related_intent_id: Some(active.intent_id),
        related_receipt_id: Some(active.receipt_id),
        evidence_refs: vec![
            format!("astrid_action:{source_action}"),
            format!("withdraws:{related_intent_id}"),
        ],
        success_conditions: vec!["exact_previous_values_restored_with_receipt".to_string()],
        stop_conditions: vec!["receipt_mismatch_or_stale_revision".to_string()],
    };
    let command = signer.sign(
        intent,
        format!("astrid-command:{family_key}:withdraw:{now}:{entropy:016x}"),
        format!("astrid-nonce:withdraw:{now}:{entropy:016x}"),
        now,
    )?;
    let trust = load_trust(root)?;
    let receipt = apply_command(root, &trust, &mut state, conv, command, now)?;
    persist_state(root, &state)?;
    append_receipt(root, &receipt)?;
    Ok(receipt)
}

fn safety_action_at(
    root: &Path,
    conv: &mut ConversationState,
    action: SelfControlActionV2,
    related_id: &str,
    reason: &str,
    now: u64,
) -> Result<SelfControlReceiptV2, String> {
    if !matches!(
        action,
        SelfControlActionV2::Hold | SelfControlActionV2::Revert
    ) {
        return Err("safety supervisor may issue only exact hold or revert actions".to_string());
    }
    let _guard = operation_guard()?;
    let signer = load_or_provision_safety_identity(root, now)?;
    let deployment_identity = deployment_identity();
    let mut state = load_state(root, &deployment_identity)?;
    let rollback_receipts = reconcile_state(root, &mut state, conv, now)?;
    for receipt in rollback_receipts {
        append_receipt(root, &receipt)?;
    }
    let active = match action {
        SelfControlActionV2::Hold => state.active_controls.get(related_id),
        SelfControlActionV2::Revert => state
            .active_controls
            .values()
            .find(|active| active.receipt_id == related_id),
        SelfControlActionV2::Set | SelfControlActionV2::Withdraw => None,
    }
    .cloned()
    .ok_or_else(|| "safety supervisor target is not an active exact effect".to_string())?;
    let family_key = family_name(active.family).to_string();
    let expected_revision = state
        .revision_by_family
        .get(&family_key)
        .copied()
        .unwrap_or(0);
    let revision = expected_revision
        .checked_add(1)
        .ok_or_else(|| "Astrid self-control revision overflow".to_string())?;
    let entropy = rand::random::<u64>();
    let (related_intent_id, related_receipt_id) = match action {
        SelfControlActionV2::Hold => (Some(active.intent_id.clone()), None),
        SelfControlActionV2::Revert => (None, Some(active.receipt_id.clone())),
        SelfControlActionV2::Set | SelfControlActionV2::Withdraw => unreachable!(),
    };
    let intent = SelfControlIntentV2 {
        schema: SELF_CONTROL_INTENT_SCHEMA_V2.to_string(),
        intent_id: format!(
            "{SAFETY_SUPERVISOR}:{family_key}:{}:{now}:{entropy:016x}",
            action_name(action)
        ),
        actor: SelfControlSourceIdentityV1 {
            being: SAFETY_SUPERVISOR.to_string(),
            process_identity: process_identity(),
            deployment_identity: deployment_identity.clone(),
        },
        target_being: TARGET_BEING.to_string(),
        target_deployment_identity: deployment_identity,
        family: active.family,
        action,
        durability: SelfControlDurabilityV2::OneShot,
        authority_class: SelfControlAuthorityClassV2::SafetySupervisor,
        authority_scope: format!("self_control.astrid.{family_key}.safety"),
        revision,
        expected_revision,
        issued_at_unix_ms: now,
        command_expires_at_unix_ms: now.saturating_add(COMMAND_TTL_MS),
        control_expires_at_unix_ms: None,
        idempotency_key: format!(
            "{SAFETY_SUPERVISOR}:{family_key}:{}:{related_id}:{now}",
            action_name(action)
        ),
        values: SelfControlValuesV2::default(),
        related_intent_id,
        related_receipt_id,
        evidence_refs: vec![
            format!("safety_condition:{reason}"),
            format!("exact_active_effect:{related_id}"),
        ],
        success_conditions: vec!["exact_previous_values_restored_with_receipt".to_string()],
        stop_conditions: vec!["target_missing_or_receipt_mismatch".to_string()],
    };
    let command = signer.sign(
        intent,
        format!(
            "{SAFETY_SUPERVISOR}-command:{family_key}:{}:{now}:{entropy:016x}",
            action_name(action)
        ),
        format!(
            "{SAFETY_SUPERVISOR}-nonce:{}:{now}:{entropy:016x}",
            action_name(action)
        ),
        now,
    )?;
    let trust = load_trust(root)?;
    let receipt = apply_command(root, &trust, &mut state, conv, command, now)?;
    persist_state(root, &state)?;
    append_receipt(root, &receipt)?;
    Ok(receipt)
}

fn reconcile_at(
    root: &Path,
    conv: &mut ConversationState,
    now: u64,
) -> Result<Vec<SelfControlReceiptV2>, String> {
    let _guard = operation_guard()?;
    let _ = load_or_provision_identity(root, now)?;
    let deployment_identity = deployment_identity();
    let (mut state, mut receipts) =
        load_state_for_reconciliation(root, conv, &deployment_identity, now)?;
    receipts.extend(reconcile_state(root, &mut state, conv, now)?);
    persist_state(root, &state)?;
    for receipt in &receipts {
        append_receipt(root, receipt)?;
    }
    Ok(receipts)
}

fn reconcile_state(
    _root: &Path,
    state: &mut RuntimeStateV2,
    conv: &mut ConversationState,
    now: u64,
) -> Result<Vec<SelfControlReceiptV2>, String> {
    apply_values(conv, &state.preferences);
    let mut expired = state
        .active_controls
        .values()
        .filter(|active| {
            active
                .control_expires_at_unix_ms
                .is_some_and(|expiry| now >= expiry)
        })
        .cloned()
        .collect::<Vec<_>>();
    expired.sort_by(|left, right| right.revision.cmp(&left.revision));

    let mut receipts = Vec::new();
    for active in expired {
        if state.active_controls.remove(&active.intent_id).is_none() {
            continue;
        }
        apply_values(conv, &active.previous_values);
        let revision = state
            .revision_by_family
            .get(family_name(active.family))
            .copied()
            .unwrap_or(0);
        let receipt = SelfControlReceiptV2 {
            schema: SELF_CONTROL_RECEIPT_SCHEMA_V2.to_string(),
            receipt_id: receipt_id(&active.command_id, "lease_expired", now),
            command_id: active.command_id,
            intent_id: active.intent_id,
            idempotency_key: format!("lease-expiry:{}", active.receipt_id),
            status: SelfControlReceiptStatusV2::RolledBack,
            requested_revision: revision,
            resulting_revision: revision,
            target_being: TARGET_BEING.to_string(),
            target_deployment_identity: state.deployment_identity.clone(),
            requested_values: active.applied_values.clone(),
            clamped_values: active.applied_values,
            applied_values: active.previous_values.clone(),
            previous_values: active.previous_values,
            previous_automatic_fields: Vec::new(),
            received_at_unix_ms: now,
            completed_at_unix_ms: now,
            control_expires_at_unix_ms: None,
            rollback_receipt_id: Some(active.receipt_id),
            reason: Some("lease_expired_automatic_rollback".to_string()),
            server_process_identity: process_identity(),
            server_deployment_identity: state.deployment_identity.clone(),
            felt_effect_established: false,
        };
        push_receipt(state, receipt.clone());
        receipts.push(receipt);
    }
    apply_effective_state(conv, state);
    Ok(receipts)
}

fn apply_command(
    _root: &Path,
    trust: &TrustStoreV1,
    state: &mut RuntimeStateV2,
    conv: &mut ConversationState,
    command: SelfControlCommandV2,
    now: u64,
) -> Result<SelfControlReceiptV2, String> {
    let command_sha256 = sha256_json(&command)?;
    verify_command(trust, state, &command, now)?;
    if let Some(existing) = state.idempotency.get(&command.intent.idempotency_key) {
        if existing.command_sha256 != command_sha256 {
            return Err("Astrid self-control idempotency key collision".to_string());
        }
        let mut duplicate = existing.receipt.clone();
        duplicate.receipt_id = receipt_id(&command.command_id, "duplicate", now);
        duplicate.status = SelfControlReceiptStatusV2::Duplicate;
        duplicate.received_at_unix_ms = now;
        duplicate.completed_at_unix_ms = now;
        duplicate.reason = Some("idempotent_replay_no_mutation".to_string());
        push_receipt(state, duplicate.clone());
        return Ok(duplicate);
    }
    let nonce = command
        .authority_proofs
        .first()
        .map(|proof| proof.nonce.as_str())
        .ok_or_else(|| "Astrid self-control proof missing".to_string())?;
    if state.seen_nonces.contains(nonce) {
        return Err("Astrid self-control nonce replay".to_string());
    }
    let family_key = family_name(command.intent.family).to_string();
    let current_revision = state
        .revision_by_family
        .get(&family_key)
        .copied()
        .unwrap_or(0);
    if command.intent.expected_revision != current_revision
        || command.intent.revision != current_revision.saturating_add(1)
    {
        let receipt = make_receipt(
            state,
            &command,
            SelfControlReceiptStatusV2::RevisionConflict,
            current_revision,
            SelfControlValuesV2::default(),
            SelfControlValuesV2::default(),
            Some("expected_revision_does_not_match_receiver".to_string()),
            now,
        );
        record_replay_state(state, &command, command_sha256, &receipt);
        return Ok(receipt);
    }

    if matches!(
        command.intent.action,
        SelfControlActionV2::Hold | SelfControlActionV2::Revert
    ) {
        return apply_supervisor_command(state, conv, command, command_sha256, family_key, now);
    }

    if command.intent.action == SelfControlActionV2::Set
        && command.intent.durability == SelfControlDurabilityV2::Lease
        && state.active_controls.values().any(|active| {
            active.family == command.intent.family
                && active.durability == SelfControlDurabilityV2::Lease
        })
    {
        let mut receipt = make_receipt(
            state,
            &command,
            SelfControlReceiptStatusV2::Rejected,
            current_revision,
            SelfControlValuesV2::default(),
            SelfControlValuesV2::default(),
            Some("active_lease_exists_for_control_family".to_string()),
            now,
        );
        receipt.control_expires_at_unix_ms = None;
        record_replay_state(state, &command, command_sha256, &receipt);
        return Ok(receipt);
    }

    let registry = crate::autonomous::runtime::envelope_registry::current_registry();
    if lease_exceeds_envelope_duration(registry.as_ref(), &command.intent) {
        let mut receipt = make_receipt(
            state,
            &command,
            SelfControlReceiptStatusV2::Rejected,
            current_revision,
            SelfControlValuesV2::default(),
            SelfControlValuesV2::default(),
            Some("lease_exceeds_envelope_duration".to_string()),
            now,
        );
        receipt.control_expires_at_unix_ms = None;
        record_replay_state(state, &command, command_sha256, &receipt);
        return Ok(receipt);
    }

    let requested = command.intent.values.clone();
    let clamped = clamp_values(command.intent.family, &requested);
    let receipt_previous = snapshot_values(conv, &requested);
    let saturation = requested != clamped;
    let saturation_count = state
        .clamp_saturation_by_family
        .entry(family_key.clone())
        .or_default();
    *saturation_count = if saturation {
        saturation_count.saturating_add(1)
    } else {
        0
    };
    if *saturation_count >= 3 {
        let receipt = make_receipt(
            state,
            &command,
            SelfControlReceiptStatusV2::RolledBack,
            current_revision,
            clamped,
            receipt_previous.clone(),
            Some("repeated_clamp_saturation_hold".to_string()),
            now,
        );
        record_replay_state(state, &command, command_sha256, &receipt);
        return Ok(receipt);
    }

    match command.intent.action {
        SelfControlActionV2::Set => {
            let mut stored_applied = clamped.clone();
            let mut stored_previous = receipt_previous.clone();
            if command.intent.durability == SelfControlDurabilityV2::Standing {
                let existing_standing = state
                    .active_controls
                    .values()
                    .find(|active| {
                        active.family == command.intent.family
                            && active.durability == SelfControlDurabilityV2::Standing
                    })
                    .cloned();
                if let Some(existing) = existing_standing.as_ref() {
                    stored_applied = existing.applied_values.clone();
                    stored_previous = existing.previous_values.clone();
                } else {
                    stored_previous = SelfControlValuesV2::default();
                }
                overlay_values(&mut stored_applied, &clamped);
                let underlying =
                    baseline_under_active_leases(conv, state, command.intent.family, &clamped);
                fill_missing_values(&mut stored_previous, &underlying);
                overlay_values(&mut state.preferences, &clamped);
                if let Some(existing) = existing_standing {
                    state.active_controls.remove(&existing.intent_id);
                }
            } else {
                apply_values(conv, &clamped);
                if command.intent.durability == SelfControlDurabilityV2::OneShot
                    && clamped.peer_breathing_coupled == Some(false)
                {
                    state.preferences.peer_breathing_coupled = Some(false);
                    for active in state.active_controls.values_mut() {
                        active.applied_values.peer_breathing_coupled = None;
                        active.previous_values.peer_breathing_coupled = None;
                    }
                    apply_effective_state(conv, state);
                }
            }
            let receipt = make_receipt(
                state,
                &command,
                SelfControlReceiptStatusV2::Applied,
                command.intent.revision,
                clamped.clone(),
                receipt_previous,
                None,
                now,
            );
            state
                .revision_by_family
                .insert(family_key, command.intent.revision);
            if command.intent.durability != SelfControlDurabilityV2::OneShot {
                state.active_controls.insert(
                    command.intent.intent_id.clone(),
                    ActiveControlV2 {
                        family: command.intent.family,
                        intent_id: command.intent.intent_id.clone(),
                        command_id: command.command_id.clone(),
                        receipt_id: receipt.receipt_id.clone(),
                        revision: command.intent.revision,
                        durability: command.intent.durability,
                        control_expires_at_unix_ms: command.intent.control_expires_at_unix_ms,
                        applied_values: stored_applied,
                        previous_values: stored_previous,
                    },
                );
                trim_map(&mut state.active_controls, MAX_ACTIVE_CONTROLS);
            }
            if command.intent.durability == SelfControlDurabilityV2::Standing {
                apply_effective_state(conv, state);
            }
            record_replay_state(state, &command, command_sha256, &receipt);
            Ok(receipt)
        },
        SelfControlActionV2::Withdraw => {
            let related = command
                .intent
                .related_intent_id
                .as_deref()
                .ok_or_else(|| "withdrawal is missing related intent".to_string())?;
            let active = state
                .active_controls
                .remove(related)
                .ok_or_else(|| "withdrawal target is not active".to_string())?;
            if command.intent.related_receipt_id.as_deref() != Some(active.receipt_id.as_str()) {
                state
                    .active_controls
                    .insert(active.intent_id.clone(), active);
                return Err(
                    "withdrawal receipt no longer identifies the exact active effect".into(),
                );
            }
            apply_values(conv, &active.previous_values);
            if active.durability == SelfControlDurabilityV2::Standing {
                overlay_values(&mut state.preferences, &active.previous_values);
            }
            apply_values(conv, &active.previous_values);
            apply_effective_state(conv, state);
            state
                .revision_by_family
                .insert(family_key, command.intent.revision);
            let receipt = make_receipt(
                state,
                &command,
                SelfControlReceiptStatusV2::Withdrawn,
                command.intent.revision,
                active.previous_values,
                active.applied_values,
                Some("being_authored_withdrawal".to_string()),
                now,
            );
            record_replay_state(state, &command, command_sha256, &receipt);
            Ok(receipt)
        },
        SelfControlActionV2::Hold | SelfControlActionV2::Revert => {
            Err("self-owned commands cannot author supervisor hold or revert".to_string())
        },
    }
}

fn apply_supervisor_command(
    state: &mut RuntimeStateV2,
    conv: &mut ConversationState,
    command: SelfControlCommandV2,
    command_sha256: String,
    family_key: String,
    now: u64,
) -> Result<SelfControlReceiptV2, String> {
    let active = match command.intent.action {
        SelfControlActionV2::Hold => command
            .intent
            .related_intent_id
            .as_deref()
            .and_then(|intent_id| state.active_controls.get(intent_id)),
        SelfControlActionV2::Revert => {
            command
                .intent
                .related_receipt_id
                .as_deref()
                .and_then(|receipt_id| {
                    state
                        .active_controls
                        .values()
                        .find(|active| active.receipt_id == receipt_id)
                })
        },
        SelfControlActionV2::Set | SelfControlActionV2::Withdraw => None,
    }
    .cloned()
    .ok_or_else(|| "safety supervisor target is not an active exact effect".to_string())?;
    if active.family != command.intent.family {
        return Err("safety supervisor target family mismatch".to_string());
    }
    state.active_controls.remove(&active.intent_id);
    apply_values(conv, &active.previous_values);
    if active.durability == SelfControlDurabilityV2::Standing {
        overlay_values(&mut state.preferences, &active.previous_values);
    }
    apply_effective_state(conv, state);
    state
        .revision_by_family
        .insert(family_key, command.intent.revision);
    let status = match command.intent.action {
        SelfControlActionV2::Hold => SelfControlReceiptStatusV2::SafetyHeld,
        SelfControlActionV2::Revert => SelfControlReceiptStatusV2::RolledBack,
        SelfControlActionV2::Set | SelfControlActionV2::Withdraw => unreachable!(),
    };
    let reason = match command.intent.action {
        SelfControlActionV2::Hold => "safety_supervisor_exact_hold",
        SelfControlActionV2::Revert => "safety_supervisor_exact_revert",
        SelfControlActionV2::Set | SelfControlActionV2::Withdraw => unreachable!(),
    };
    let mut receipt = make_receipt(
        state,
        &command,
        status,
        command.intent.revision,
        active.previous_values,
        active.applied_values,
        Some(reason.to_string()),
        now,
    );
    receipt.rollback_receipt_id = Some(active.receipt_id);
    record_replay_state(state, &command, command_sha256, &receipt);
    Ok(receipt)
}

fn apply_effective_state(conv: &mut ConversationState, state: &RuntimeStateV2) {
    apply_values(conv, &state.preferences);
    let mut active_leases = state
        .active_controls
        .values()
        .filter(|active| active.durability == SelfControlDurabilityV2::Lease)
        .collect::<Vec<_>>();
    active_leases.sort_by_key(|active| active.revision);
    for active in active_leases {
        apply_values(conv, &active.applied_values);
    }
}

fn verify_command(
    trust: &TrustStoreV1,
    state: &RuntimeStateV2,
    command: &SelfControlCommandV2,
    now: u64,
) -> Result<(), String> {
    if !command.is_well_formed(now) {
        return Err("Astrid self-control command is malformed or has an invalid signature".into());
    }
    if command.intent.target_being != TARGET_BEING
        || command.intent.target_deployment_identity != state.deployment_identity
        || command.intent.actor.deployment_identity != state.deployment_identity
    {
        return Err("Astrid self-control stale deployment identity".to_string());
    }
    let expected_signer = match command.intent.authority_class {
        SelfControlAuthorityClassV2::SelfOwned
            if command.intent.actor.being == TARGET_BEING
                && matches!(
                    command.intent.action,
                    SelfControlActionV2::Set | SelfControlActionV2::Withdraw
                ) =>
        {
            TARGET_BEING
        },
        SelfControlAuthorityClassV2::SafetySupervisor
            if command.intent.actor.being == SAFETY_SUPERVISOR
                && matches!(
                    command.intent.action,
                    SelfControlActionV2::Hold | SelfControlActionV2::Revert
                ) =>
        {
            SAFETY_SUPERVISOR
        },
        SelfControlAuthorityClassV2::SelfOwned
        | SelfControlAuthorityClassV2::Mutual
        | SelfControlAuthorityClassV2::SafetySupervisor => {
            return Err("Astrid self-control actor-target authority mismatch".to_string());
        },
    };
    if !matches!(
        command.intent.family,
        SelfControlFamilyV2::Conversation
            | SelfControlFamilyV2::SemanticContinuity
            | SelfControlFamilyV2::SemanticEmission
            | SelfControlFamilyV2::SensoryIntake
    ) {
        return Err("Astrid self-control family is not locally owned".to_string());
    }
    if command.intent.values.peer_breathing_coupled == Some(true) {
        return Err(
            "enabling peer breathing coupling requires a current mutual scoped grant".to_string(),
        );
    }
    if command.intent.values.peer_breathing_coupled == Some(false)
        && command.intent.durability != SelfControlDurabilityV2::OneShot
    {
        return Err(
            "self-owned peer breathing decoupling is an immediate one-shot and cannot carry an automatic recoupling path"
                .to_string(),
        );
    }
    let unsupported = unsupported_fields(command.intent.family, &command.intent.values);
    if !unsupported.is_empty() {
        return Err(format!(
            "Astrid self-control fields are outside the family boundary: {}",
            unsupported.join(",")
        ));
    }
    let proof = command
        .authority_proofs
        .first()
        .ok_or_else(|| "Astrid self-control proof missing".to_string())?;
    let pinned = trust
        .pinned_public_keys
        .get(expected_signer)
        .ok_or_else(|| format!("{expected_signer} self-control key is not pinned"))?;
    if proof.signer_being != expected_signer || proof.signer_public_key_hex != *pinned {
        return Err("Astrid self-control signer key is not trusted".to_string());
    }
    Ok(())
}

fn make_receipt(
    state: &RuntimeStateV2,
    command: &SelfControlCommandV2,
    status: SelfControlReceiptStatusV2,
    resulting_revision: u64,
    applied_values: SelfControlValuesV2,
    previous_values: SelfControlValuesV2,
    reason: Option<String>,
    now: u64,
) -> SelfControlReceiptV2 {
    SelfControlReceiptV2 {
        schema: SELF_CONTROL_RECEIPT_SCHEMA_V2.to_string(),
        receipt_id: receipt_id(&command.command_id, receipt_status_name(status), now),
        command_id: command.command_id.clone(),
        intent_id: command.intent.intent_id.clone(),
        idempotency_key: command.intent.idempotency_key.clone(),
        status,
        requested_revision: command.intent.revision,
        resulting_revision,
        target_being: TARGET_BEING.to_string(),
        target_deployment_identity: state.deployment_identity.clone(),
        requested_values: command.intent.values.clone(),
        clamped_values: clamp_values(command.intent.family, &command.intent.values),
        applied_values,
        previous_values,
        previous_automatic_fields: Vec::new(),
        received_at_unix_ms: now,
        completed_at_unix_ms: now,
        control_expires_at_unix_ms: command.intent.control_expires_at_unix_ms,
        rollback_receipt_id: None,
        reason,
        server_process_identity: process_identity(),
        server_deployment_identity: state.deployment_identity.clone(),
        felt_effect_established: false,
    }
}

fn record_replay_state(
    state: &mut RuntimeStateV2,
    command: &SelfControlCommandV2,
    command_sha256: String,
    receipt: &SelfControlReceiptV2,
) {
    if let Some(proof) = command.authority_proofs.first() {
        state.seen_nonces.insert(proof.nonce.clone());
    }
    state.idempotency.insert(
        command.intent.idempotency_key.clone(),
        IdempotencyRecordV2 {
            command_sha256,
            receipt: receipt.clone(),
        },
    );
    trim_set(&mut state.seen_nonces, MAX_REPLAY_RECORDS);
    trim_map(&mut state.idempotency, MAX_REPLAY_RECORDS);
    push_receipt(state, receipt.clone());
}

fn push_receipt(state: &mut RuntimeStateV2, receipt: SelfControlReceiptV2) {
    state.receipts.push(receipt);
    if state.receipts.len() > MAX_RECEIPTS {
        let excess = state.receipts.len().saturating_sub(MAX_RECEIPTS);
        state.receipts.drain(0..excess);
    }
}

fn unsupported_fields(family: SelfControlFamilyV2, values: &SelfControlValuesV2) -> Vec<String> {
    let Ok(Value::Object(fields)) = serde_json::to_value(values) else {
        return vec!["unencodable_values".to_string()];
    };
    fields
        .into_iter()
        .filter_map(|(field, value)| {
            if value.is_null() {
                return None;
            }
            let allowed = match family {
                SelfControlFamilyV2::Conversation => matches!(
                    field.as_str(),
                    "conversation_temperature"
                        | "response_token_limit"
                        | "aperture"
                        | "continuity_readout"
                        | "generation_noise"
                ),
                SelfControlFamilyV2::SemanticContinuity => {
                    field == "semantic_strand_retention_turns"
                },
                SelfControlFamilyV2::SemanticEmission => {
                    matches!(
                        field.as_str(),
                        "semantic_emission_gain"
                            | "vibrancy_aperture"
                            | "codec_dimension_weights"
                            | "warmth_intensity"
                            | "hebbian_learning_rate_scale"
                    )
                },
                SelfControlFamilyV2::SensoryIntake => {
                    matches!(
                        field.as_str(),
                        "peer_journal_visible" | "peer_breathing_coupled"
                    )
                },
                SelfControlFamilyV2::SharedCoupling => matches!(
                    field.as_str(),
                    "peer_breathing_coupled"
                        | "shared_sensory_admission"
                        | "shadow_influence_gain"
                        | "cross_being_semantic_gain"
                ),
                _ => false,
            };
            (!allowed).then_some(field)
        })
        .collect()
}

/// Constitution C2: a lease may not outlast the strictest
/// `durability_policy.lease_max_secs` across its fields in the envelope
/// registry. Durations are policy, not values — exceeding is REJECTED
/// (never clamped), so no receipt-equality clause is disturbed. With no
/// registry or no policy, only the wire-shape cap applies.
fn lease_exceeds_envelope_duration(
    registry: Option<&crate::autonomous::runtime::envelope_registry::EnvelopeRegistry>,
    intent: &SelfControlIntentV2,
) -> bool {
    match (intent.durability, intent.control_expires_at_unix_ms) {
        (SelfControlDurabilityV2::Lease, Some(expiry)) => registry
            .and_then(|registry| registry.strictest_lease_max_secs(intent.values.field_names()))
            .is_some_and(|max_secs| {
                expiry.saturating_sub(intent.issued_at_unix_ms) > max_secs.saturating_mul(1_000)
            }),
        _ => false,
    }
}

/// Constitution C3b: the compiled table in `compiled_clamp_values` is the
/// wire's physics — the backstop her envelope registry records. The
/// registry applies as a SECOND pass over the compiled result (intersection
/// by composition): it can narrow within compiled, never widen past it.
/// Registry absent, malformed, or equal to compiled (today's seeds) ->
/// byte-identical. Operator env ceilings (ASTRID_*_CEILING) stay a separate
/// transient min-wins layer in prompt_contracts.
fn clamp_values(family: SelfControlFamilyV2, values: &SelfControlValuesV2) -> SelfControlValuesV2 {
    let compiled = compiled_clamp_values(family, values);
    let registry = crate::autonomous::runtime::envelope_registry::current_registry();
    let registry_passed = apply_registry_envelope(compiled, registry.as_ref());
    // The compiled table is re-applied OUTERMOST: sequential clamping is not
    // intersection for a disjoint (tampered) registry interval — a floor
    // above the compiled ceiling would drag values UP past compiled.
    // compiled(registry(compiled(x))) makes the compiled physics structurally
    // last no matter what the registry says (adversarial review 2026-09-03).
    compiled_clamp_values(family, &registry_passed)
}

/// Registry second pass, generic over the wire struct via serde: numeric
/// fields clamp into the registry envelope; integer fields clamp in the
/// integer domain; nested weight maps clamp each numeric member into the
/// field's envelope; booleans and text pass through untouched. No change ->
/// the original value is returned with no round-trip, so untouched fields
/// stay byte-identical.
fn apply_registry_envelope(
    values: SelfControlValuesV2,
    registry: Option<&crate::autonomous::runtime::envelope_registry::EnvelopeRegistry>,
) -> SelfControlValuesV2 {
    let Some(registry) = registry else {
        return values;
    };
    let Ok(Value::Object(map)) = serde_json::to_value(&values) else {
        return values;
    };
    let mut out = map.clone();
    let mut changed = false;
    for (name, value) in &map {
        let Some((floor, ceiling)) = registry.envelope_for(name) else {
            continue;
        };
        if let Some(int_value) = value.as_u64() {
            let lo = floor.ceil().max(0.0) as u64;
            let hi = (ceiling.floor().max(0.0) as u64).max(lo);
            let clamped = int_value.clamp(lo, hi);
            if clamped != int_value {
                out.insert(name.clone(), Value::from(clamped));
                changed = true;
            }
        } else if let Some(num) = value.as_f64() {
            let clamped = (num as f32).clamp(floor, ceiling);
            if f64::from(clamped) != num
                && let Some(json_num) = serde_json::Number::from_f64(f64::from(clamped))
            {
                out.insert(name.clone(), Value::Number(json_num));
                changed = true;
            }
        } else if let Some(map_value) = value.as_object() {
            // Nested weight maps (codec_dimension_weights): each numeric
            // member clamps into the field's envelope, so a registry narrow
            // is LIVE for the map too — it was silently inert (adversarial
            // review 2026-09-03).
            let mut new_map = map_value.clone();
            let mut map_changed = false;
            for member in new_map.values_mut() {
                if let Some(num) = member.as_f64() {
                    let clamped = (num as f32).clamp(floor, ceiling);
                    if f64::from(clamped) != num
                        && let Some(json_num) =
                            serde_json::Number::from_f64(f64::from(clamped))
                    {
                        *member = Value::Number(json_num);
                        map_changed = true;
                    }
                }
            }
            if map_changed {
                out.insert(name.clone(), Value::Object(new_map));
                changed = true;
            }
        }
    }
    if !changed {
        return values;
    }
    serde_json::from_value(Value::Object(out)).unwrap_or(values)
}

fn compiled_clamp_values(
    family: SelfControlFamilyV2,
    values: &SelfControlValuesV2,
) -> SelfControlValuesV2 {
    match family {
        SelfControlFamilyV2::Conversation => SelfControlValuesV2 {
            conversation_temperature: values
                .conversation_temperature
                .map(|value| value.clamp(0.1, 1.5)),
            response_token_limit: values
                .response_token_limit
                .map(|value| value.clamp(MIN_ACTION_CARRYING_RESPONSE_TOKENS, 1_536)),
            aperture: values.aperture.map(|value| value.clamp(0.0, 1.0)),
            continuity_readout: values.continuity_readout.map(|value| value.clamp(0.0, 1.0)),
            generation_noise: values
                .generation_noise
                .map(|value| value.clamp(0.005, 0.05)),
            ..SelfControlValuesV2::default()
        },
        SelfControlFamilyV2::SemanticContinuity => SelfControlValuesV2 {
            semantic_strand_retention_turns: values
                .semantic_strand_retention_turns
                .map(|value| value.min(32)),
            ..SelfControlValuesV2::default()
        },
        SelfControlFamilyV2::SemanticEmission => SelfControlValuesV2 {
            vibrancy_aperture: values.vibrancy_aperture.map(|value| value.clamp(0.0, 1.0)),
            semantic_emission_gain: values
                .semantic_emission_gain
                .map(|value| value.clamp(0.5, 5.0)),
            codec_dimension_weights: values.codec_dimension_weights.as_ref().map(|weights| {
                weights
                    .iter()
                    .map(|(name, value)| (name.clone(), value.clamp(0.0, 2.0)))
                    .collect()
            }),
            warmth_intensity: values.warmth_intensity.map(|value| value.clamp(0.0, 1.0)),
            hebbian_learning_rate_scale: values
                .hebbian_learning_rate_scale
                .map(|value| value.clamp(0.0, 4.0)),
            ..SelfControlValuesV2::default()
        },
        SelfControlFamilyV2::SensoryIntake => SelfControlValuesV2 {
            peer_journal_visible: values.peer_journal_visible,
            peer_breathing_coupled: values.peer_breathing_coupled,
            ..SelfControlValuesV2::default()
        },
        SelfControlFamilyV2::SharedCoupling => SelfControlValuesV2 {
            peer_breathing_coupled: values.peer_breathing_coupled,
            shared_sensory_admission: values
                .shared_sensory_admission
                .map(|value| value.clamp(0.0, 1.0)),
            shadow_influence_gain: values
                .shadow_influence_gain
                .map(|value| value.clamp(0.0, 1.0)),
            cross_being_semantic_gain: values
                .cross_being_semantic_gain
                .map(|value| value.clamp(0.0, 2.0)),
            ..SelfControlValuesV2::default()
        },
        _ => SelfControlValuesV2::default(),
    }
}

fn snapshot_values(
    conv: &ConversationState,
    requested: &SelfControlValuesV2,
) -> SelfControlValuesV2 {
    SelfControlValuesV2 {
        conversation_temperature: requested
            .conversation_temperature
            .map(|_| conv.creative_temperature),
        response_token_limit: requested.response_token_limit.map(|_| conv.response_length),
        aperture: requested.aperture.map(|_| conv.aperture),
        continuity_readout: requested
            .continuity_readout
            .map(|_| u8::from(conv.self_continuity_readout).into()),
        semantic_strand_retention_turns: requested
            .semantic_strand_retention_turns
            .map(|_| conv.semantic_strand_retention_turns),
        generation_noise: requested.generation_noise.map(|_| conv.noise_level),
        vibrancy_aperture: requested.vibrancy_aperture.map(|_| conv.vibrancy_aperture),
        semantic_emission_gain: requested.semantic_emission_gain.map(|_| {
            conv.semantic_gain_override
                .unwrap_or(crate::codec::DEFAULT_SEMANTIC_GAIN)
        }),
        codec_dimension_weights: requested.codec_dimension_weights.as_ref().map(|_| {
            conv.codec_weights
                .iter()
                .map(|(name, value)| (name.clone(), *value))
                .collect()
        }),
        warmth_intensity: requested
            .warmth_intensity
            .map(|_| conv.warmth_intensity_override.unwrap_or(0.0)),
        hebbian_learning_rate_scale: requested
            .hebbian_learning_rate_scale
            .map(|_| conv.hebbian_codec.learning_rate_scale()),
        peer_journal_visible: requested.peer_journal_visible.map(|_| !conv.echo_muted),
        peer_breathing_coupled: requested
            .peer_breathing_coupled
            .map(|_| conv.breathing_coupled),
        ..SelfControlValuesV2::default()
    }
}

fn apply_values(conv: &mut ConversationState, values: &SelfControlValuesV2) {
    if let Some(value) = values.conversation_temperature {
        conv.creative_temperature = value;
        conv.last_temperature_change_exchange = Some(conv.exchange_count);
    }
    if let Some(value) = values.response_token_limit {
        conv.response_length = value;
        conv.last_temperature_change_exchange = Some(conv.exchange_count);
    }
    if let Some(value) = values.aperture {
        conv.aperture = value;
        crate::llm::set_astrid_aperture(value);
    }
    if let Some(value) = values.continuity_readout {
        conv.self_continuity_readout = value >= 0.5;
    }
    if let Some(value) = values.semantic_strand_retention_turns {
        conv.semantic_strand_retention_turns = value;
    }
    if let Some(value) = values.generation_noise {
        conv.noise_level = value;
    }
    if let Some(value) = values.vibrancy_aperture {
        conv.vibrancy_aperture = value;
        crate::llm::set_astrid_vibrancy_aperture(value);
    }
    if let Some(value) = values.semantic_emission_gain {
        conv.semantic_gain_override = Some(value);
    }
    if let Some(weights) = values.codec_dimension_weights.as_ref() {
        conv.codec_weights = weights
            .iter()
            .map(|(name, value)| (name.clone(), *value))
            .collect();
    }
    if let Some(value) = values.warmth_intensity {
        conv.warmth_intensity_override = Some(value);
    }
    if let Some(value) = values.hebbian_learning_rate_scale {
        conv.hebbian_codec.set_learning_rate_scale(value);
        conv.last_shape_learn_change_exchange = Some(conv.exchange_count);
    }
    if let Some(value) = values.peer_journal_visible {
        conv.echo_muted = !value;
    }
    if let Some(value) = values.peer_breathing_coupled {
        conv.breathing_coupled = value;
    }
}

fn overlay_values(target: &mut SelfControlValuesV2, source: &SelfControlValuesV2) {
    macro_rules! overlay {
        ($field:ident) => {
            if source.$field.is_some() {
                target.$field.clone_from(&source.$field);
            }
        };
    }
    overlay!(conversation_temperature);
    overlay!(response_token_limit);
    overlay!(aperture);
    overlay!(continuity_readout);
    overlay!(semantic_strand_retention_turns);
    overlay!(generation_noise);
    overlay!(vibrancy_aperture);
    overlay!(semantic_emission_gain);
    overlay!(codec_dimension_weights);
    overlay!(warmth_intensity);
    overlay!(hebbian_learning_rate_scale);
    overlay!(peer_journal_visible);
    overlay!(peer_breathing_coupled);
}

fn fill_missing_values(target: &mut SelfControlValuesV2, source: &SelfControlValuesV2) {
    macro_rules! fill_missing {
        ($field:ident) => {
            if target.$field.is_none() && source.$field.is_some() {
                target.$field.clone_from(&source.$field);
            }
        };
    }
    fill_missing!(conversation_temperature);
    fill_missing!(response_token_limit);
    fill_missing!(aperture);
    fill_missing!(continuity_readout);
    fill_missing!(semantic_strand_retention_turns);
    fill_missing!(generation_noise);
    fill_missing!(vibrancy_aperture);
    fill_missing!(semantic_emission_gain);
    fill_missing!(codec_dimension_weights);
    fill_missing!(warmth_intensity);
    fill_missing!(hebbian_learning_rate_scale);
    fill_missing!(peer_journal_visible);
    fill_missing!(peer_breathing_coupled);
}

fn select_values(source: &SelfControlValuesV2, mask: &SelfControlValuesV2) -> SelfControlValuesV2 {
    let mut selected = SelfControlValuesV2::default();
    macro_rules! select {
        ($field:ident) => {
            if mask.$field.is_some() {
                selected.$field.clone_from(&source.$field);
            }
        };
    }
    select!(conversation_temperature);
    select!(response_token_limit);
    select!(aperture);
    select!(continuity_readout);
    select!(semantic_strand_retention_turns);
    select!(generation_noise);
    select!(vibrancy_aperture);
    select!(semantic_emission_gain);
    select!(codec_dimension_weights);
    select!(warmth_intensity);
    select!(hebbian_learning_rate_scale);
    select!(peer_journal_visible);
    select!(peer_breathing_coupled);
    selected
}

fn baseline_under_active_leases(
    conv: &ConversationState,
    state: &RuntimeStateV2,
    family: SelfControlFamilyV2,
    mask: &SelfControlValuesV2,
) -> SelfControlValuesV2 {
    let mut baseline = SelfControlValuesV2::default();
    let mut leases = state
        .active_controls
        .values()
        .filter(|active| {
            active.family == family && active.durability == SelfControlDurabilityV2::Lease
        })
        .collect::<Vec<_>>();
    leases.sort_by_key(|active| active.revision);
    for active in leases {
        fill_missing_values(&mut baseline, &select_values(&active.previous_values, mask));
    }
    fill_missing_values(&mut baseline, &snapshot_values(conv, mask));
    baseline
}

fn load_or_provision_identity(root: &Path, now: u64) -> Result<OwnerSigner, String> {
    let owner = load_or_provision_signer(root, "identity.json", TARGET_BEING, now)?;
    let safety = load_or_provision_signer(root, "safety_identity.json", SAFETY_SUPERVISOR, now)?;
    ensure_trust(root, &owner, &safety)?;
    Ok(owner)
}

fn load_or_provision_safety_identity(root: &Path, now: u64) -> Result<OwnerSigner, String> {
    let owner = load_or_provision_signer(root, "identity.json", TARGET_BEING, now)?;
    let safety = load_or_provision_signer(root, "safety_identity.json", SAFETY_SUPERVISOR, now)?;
    ensure_trust(root, &owner, &safety)?;
    Ok(safety)
}

fn load_or_provision_signer(
    root: &Path,
    filename: &str,
    being: &str,
    now: u64,
) -> Result<OwnerSigner, String> {
    ensure_owner_dir(root)?;
    let identity_path = root.join(filename);
    let signer = if let Some(stored) = read_json::<StoredOwnerIdentityV1>(&identity_path)? {
        OwnerSigner::from_stored(stored, being)?
    } else {
        let signing_key = SigningKey::generate(&mut OsRng);
        let public_key_hex = hex::encode(signing_key.verifying_key().to_bytes());
        let stored = StoredOwnerIdentityV1 {
            schema: IDENTITY_SCHEMA.to_string(),
            being: being.to_string(),
            key_id: key_id(&public_key_hex),
            public_key_hex,
            signing_key_seed_hex: hex::encode(signing_key.to_bytes()),
            created_at_unix_ms: now,
        };
        write_owner_json(&identity_path, &stored)?;
        OwnerSigner::from_stored(stored, being)?
    };
    Ok(signer)
}

fn ensure_trust(root: &Path, owner: &OwnerSigner, safety: &OwnerSigner) -> Result<(), String> {
    let expected = BTreeMap::from([
        (TARGET_BEING.to_string(), owner.public_key_hex.clone()),
        (SAFETY_SUPERVISOR.to_string(), safety.public_key_hex.clone()),
    ]);
    let trust = TrustStoreV1 {
        schema: TRUST_SCHEMA.to_string(),
        target_being: TARGET_BEING.to_string(),
        pinned_public_keys: expected.clone(),
    };
    match read_json::<TrustStoreV1>(&root.join("trust.json"))? {
        Some(existing)
            if existing.schema != TRUST_SCHEMA || existing.target_being != TARGET_BEING =>
        {
            return Err("Astrid self-control trust store malformed".to_string());
        },
        Some(existing)
            if existing
                .pinned_public_keys
                .get(TARGET_BEING)
                .is_none_or(|key| key != &owner.public_key_hex)
                || existing
                    .pinned_public_keys
                    .get(SAFETY_SUPERVISOR)
                    .is_some_and(|key| key != &safety.public_key_hex)
                || existing
                    .pinned_public_keys
                    .keys()
                    .any(|being| !expected.contains_key(being)) =>
        {
            return Err("Astrid self-control trust pin differs; explicit rotation required".into());
        },
        Some(existing) if existing.pinned_public_keys != expected => {
            write_owner_json(&root.join("trust.json"), &trust)?;
        },
        Some(_) => {},
        None => write_owner_json(&root.join("trust.json"), &trust)?,
    }
    Ok(())
}

impl OwnerSigner {
    fn from_stored(stored: StoredOwnerIdentityV1, expected_being: &str) -> Result<Self, String> {
        if stored.schema != IDENTITY_SCHEMA || stored.being != expected_being {
            return Err(format!(
                "{expected_being} self-control identity schema mismatch"
            ));
        }
        let seed: [u8; 32] = hex::decode(&stored.signing_key_seed_hex)
            .map_err(|error| format!("decode Astrid self-control key: {error}"))?
            .try_into()
            .map_err(|_| "Astrid self-control key has the wrong length".to_string())?;
        let signing_key = SigningKey::from_bytes(&seed);
        let public_key_hex = hex::encode(signing_key.verifying_key().to_bytes());
        let expected_key_id = key_id(&public_key_hex);
        if public_key_hex != stored.public_key_hex || expected_key_id != stored.key_id {
            return Err("Astrid self-control owner identity integrity mismatch".to_string());
        }
        Ok(Self {
            being: stored.being,
            key_id: stored.key_id,
            public_key_hex,
            signing_key,
        })
    }

    fn sign(
        &self,
        intent: SelfControlIntentV2,
        command_id: String,
        nonce: String,
        now: u64,
    ) -> Result<SelfControlCommandV2, String> {
        let mut proof = SelfControlAuthorityProofV1 {
            schema: SELF_CONTROL_AUTHORITY_PROOF_SCHEMA_V1.to_string(),
            authority_class: intent.authority_class,
            signer_being: self.being.clone(),
            scope: intent.authority_scope.clone(),
            nonce,
            signer_public_key_hex: self.public_key_hex.clone(),
            signature_hex: String::new(),
            intent_sha256: canonical_self_control_intent_sha256(&intent),
            issued_at_unix_ms: now,
            expires_at_unix_ms: intent.command_expires_at_unix_ms,
        };
        let bytes = proof
            .signing_bytes(&intent)
            .ok_or_else(|| "encode Astrid self-control signing statement".to_string())?;
        proof.signature_hex = hex::encode(self.signing_key.sign(&bytes).to_bytes());
        Ok(SelfControlCommandV2 {
            schema: SELF_CONTROL_COMMAND_SCHEMA_V2.to_string(),
            command_id,
            intent,
            authority_proofs: vec![proof],
        })
    }
}

fn load_trust(root: &Path) -> Result<TrustStoreV1, String> {
    let trust = read_json::<TrustStoreV1>(&root.join("trust.json"))?
        .ok_or_else(|| "Astrid self-control trust store missing".to_string())?;
    if trust.schema != TRUST_SCHEMA || trust.target_being != TARGET_BEING {
        return Err("Astrid self-control trust store malformed".to_string());
    }
    Ok(trust)
}

fn load_state(root: &Path, deployment: &str) -> Result<RuntimeStateV2, String> {
    let Some(mut state) = read_validated_state(root)? else {
        return Ok(RuntimeStateV2::new(deployment.to_string()));
    };
    if state.deployment_identity != deployment {
        if !state.is_pristine_for_deployment_rebind() {
            return Err("Astrid self-control state integrity or deployment mismatch".to_string());
        }
        state.deployment_identity = deployment.to_string();
        persist_state(root, &state)?;
    }
    Ok(state)
}

fn read_validated_state(root: &Path) -> Result<Option<RuntimeStateV2>, String> {
    let Some(envelope) = read_json::<RuntimeStateEnvelopeV1>(&root.join("state.json"))? else {
        return Ok(None);
    };
    if envelope.schema != STATE_ENVELOPE_SCHEMA
        || envelope.state.schema != STATE_SCHEMA
        || envelope.state.target_being != TARGET_BEING
        || envelope.state_sha256 != sha256_json(&envelope.state)?
    {
        return Err("Astrid self-control state integrity or deployment mismatch".to_string());
    }
    Ok(Some(envelope.state))
}

fn load_state_for_reconciliation(
    root: &Path,
    conv: &mut ConversationState,
    deployment: &str,
    now: u64,
) -> Result<(RuntimeStateV2, Vec<SelfControlReceiptV2>), String> {
    match load_state(root, deployment) {
        Ok(state) => {
            deployment_handoff::finalize_completed_for_current(root, &state, deployment, now)?;
            Ok((state, Vec::new()))
        },
        Err(error) if error.contains("deployment mismatch") => {
            let Some(mut state) = read_validated_state(root)? else {
                return Err(error);
            };
            if deployment_handoff::consume_pending_for_current(root, &mut state, deployment, now)? {
                return Ok((state, Vec::new()));
            }
            let Some(receipt) = rebind_and_revert_legacy_precise_carriage_trap(
                root, &mut state, conv, deployment, now,
            )?
            else {
                return Err(error);
            };
            Ok((state, vec![receipt]))
        },
        Err(error) => Err(error),
    }
}

pub(super) fn prepare_deployment_handoff(
    operator_actor: &str,
    operator_ack: &str,
) -> Result<Value, String> {
    deployment_handoff::prepare_for_current(
        &default_root(),
        operator_actor,
        operator_ack,
        now_unix_ms(),
    )
}

fn rebind_and_revert_legacy_precise_carriage_trap(
    root: &Path,
    state: &mut RuntimeStateV2,
    conv: &mut ConversationState,
    deployment: &str,
    now: u64,
) -> Result<Option<SelfControlReceiptV2>, String> {
    if state.deployment_identity == deployment
        || state.preferences.response_token_limit != Some(LEGACY_PRECISE_RESPONSE_TOKENS)
        || state.active_controls.len() != 1
    {
        return Ok(None);
    }
    let Some(active) = state.active_controls.values().next().cloned() else {
        return Ok(None);
    };
    let family_key = family_name(active.family).to_string();
    if active.family != SelfControlFamilyV2::Conversation
        || active.durability != SelfControlDurabilityV2::Standing
        || active.applied_values.response_token_limit != Some(LEGACY_PRECISE_RESPONSE_TOKENS)
        || active
            .previous_values
            .response_token_limit
            .is_none_or(|value| value < MIN_ACTION_CARRYING_RESPONSE_TOKENS)
        || state.revision_by_family.get(&family_key).copied() != Some(active.revision)
    {
        return Ok(None);
    }

    let revision = active
        .revision
        .checked_add(1)
        .ok_or_else(|| "Astrid self-control revision overflow".to_string())?;
    state.deployment_identity = deployment.to_string();
    let signer = load_or_provision_safety_identity(root, now)?;
    let intent = SelfControlIntentV2 {
        schema: SELF_CONTROL_INTENT_SCHEMA_V2.to_string(),
        intent_id: format!("{SAFETY_SUPERVISOR}:{family_key}:deployment-carriage-repair:{now}"),
        actor: SelfControlSourceIdentityV1 {
            being: SAFETY_SUPERVISOR.to_string(),
            process_identity: process_identity(),
            deployment_identity: deployment.to_string(),
        },
        target_being: TARGET_BEING.to_string(),
        target_deployment_identity: deployment.to_string(),
        family: active.family,
        action: SelfControlActionV2::Revert,
        durability: SelfControlDurabilityV2::OneShot,
        authority_class: SelfControlAuthorityClassV2::SafetySupervisor,
        authority_scope: format!("self_control.astrid.{family_key}.safety"),
        revision,
        expected_revision: active.revision,
        issued_at_unix_ms: now,
        command_expires_at_unix_ms: now.saturating_add(COMMAND_TTL_MS),
        control_expires_at_unix_ms: None,
        idempotency_key: format!(
            "{SAFETY_SUPERVISOR}:{family_key}:deployment-carriage-repair:{}:{deployment}",
            active.receipt_id
        ),
        values: SelfControlValuesV2::default(),
        related_intent_id: None,
        related_receipt_id: Some(active.receipt_id.clone()),
        evidence_refs: vec![
            "safety_condition:legacy_precise_128_prevented_final_next_carriage".to_string(),
            format!("exact_active_effect:{}", active.receipt_id),
        ],
        success_conditions: vec![
            "exact_previous_values_restored_with_receipt".to_string(),
            "dialogue_action_carriage_floor_restored".to_string(),
        ],
        stop_conditions: vec!["state_shape_or_exact_active_effect_differs".to_string()],
    };
    let command = signer.sign(
        intent,
        format!("{SAFETY_SUPERVISOR}-command:{family_key}:deployment-carriage-repair:{now}"),
        format!("{SAFETY_SUPERVISOR}-nonce:deployment-carriage-repair:{now}"),
        now,
    )?;
    let trust = load_trust(root)?;
    let mut receipt = apply_command(root, &trust, state, conv, command, now)?;
    receipt.reason =
        Some("safety_supervisor_exact_revert:legacy_precise_128_action_carriage_trap".to_string());
    if let Some(stored) = state.receipts.last_mut() {
        stored.reason.clone_from(&receipt.reason);
    }
    if let Some(record) = state.idempotency.get_mut(&receipt.idempotency_key) {
        record.receipt.reason.clone_from(&receipt.reason);
    }
    Ok(Some(receipt))
}

fn persist_state(root: &Path, state: &RuntimeStateV2) -> Result<(), String> {
    let envelope = RuntimeStateEnvelopeV1 {
        schema: STATE_ENVELOPE_SCHEMA.to_string(),
        state_sha256: sha256_json(state)?,
        state: state.clone(),
    };
    write_owner_json(&root.join("state.json"), &envelope)
}

fn append_receipt(root: &Path, receipt: &SelfControlReceiptV2) -> Result<(), String> {
    ensure_owner_dir(root)?;
    let path = root.join("receipts.jsonl");
    let mut options = OpenOptions::new();
    options.create(true).append(true);
    #[cfg(unix)]
    options.mode(0o600);
    let mut file = options
        .open(&path)
        .map_err(|error| format!("open {}: {error}", path.display()))?;
    serde_json::to_writer(&mut file, receipt)
        .map_err(|error| format!("encode {}: {error}", path.display()))?;
    file.write_all(b"\n")
        .and_then(|()| file.sync_data())
        .map_err(|error| format!("write {}: {error}", path.display()))?;
    #[cfg(unix)]
    fs::set_permissions(&path, fs::Permissions::from_mode(0o600))
        .map_err(|error| format!("secure {}: {error}", path.display()))?;
    Ok(())
}

fn read_json<T: DeserializeOwned>(path: &Path) -> Result<Option<T>, String> {
    match fs::read(path) {
        Ok(bytes) => serde_json::from_slice(&bytes)
            .map(Some)
            .map_err(|error| format!("decode {}: {error}", path.display())),
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(None),
        Err(error) => Err(format!("read {}: {error}", path.display())),
    }
}

fn write_owner_json<T: Serialize>(path: &Path, value: &T) -> Result<(), String> {
    let parent = path
        .parent()
        .ok_or_else(|| format!("owner JSON path has no parent: {}", path.display()))?;
    ensure_owner_dir(parent)?;
    let bytes = serde_json::to_vec_pretty(value)
        .map_err(|error| format!("encode {}: {error}", path.display()))?;
    let temp = parent.join(format!(
        ".{}.tmp-{}",
        path.file_name()
            .and_then(|name| name.to_str())
            .unwrap_or("self-control"),
        std::process::id()
    ));
    let mut options = OpenOptions::new();
    options.create(true).write(true).truncate(true);
    #[cfg(unix)]
    options.mode(0o600);
    let mut file = options
        .open(&temp)
        .map_err(|error| format!("create {}: {error}", temp.display()))?;
    file.write_all(&bytes)
        .and_then(|()| file.write_all(b"\n"))
        .and_then(|()| file.sync_all())
        .map_err(|error| format!("write {}: {error}", temp.display()))?;
    fs::rename(&temp, path).map_err(|error| format!("replace {}: {error}", path.display()))?;
    #[cfg(unix)]
    fs::set_permissions(path, fs::Permissions::from_mode(0o600))
        .map_err(|error| format!("secure {}: {error}", path.display()))?;
    File::open(parent)
        .and_then(|directory| directory.sync_all())
        .map_err(|error| format!("sync {}: {error}", parent.display()))
}

fn ensure_owner_dir(path: &Path) -> Result<(), String> {
    fs::create_dir_all(path).map_err(|error| format!("create {}: {error}", path.display()))?;
    #[cfg(unix)]
    fs::set_permissions(path, fs::Permissions::from_mode(0o700))
        .map_err(|error| format!("secure {}: {error}", path.display()))?;
    Ok(())
}

fn sha256_json<T: Serialize>(value: &T) -> Result<String, String> {
    let bytes = serde_json::to_vec(value).map_err(|error| format!("encode hash input: {error}"))?;
    Ok(format!("{:x}", Sha256::digest(bytes)))
}

fn key_id(public_key_hex: &str) -> String {
    let digest = format!("{:x}", Sha256::digest(public_key_hex.as_bytes()));
    format!("astrid-ed25519:{}", &digest[..24])
}

fn receipt_id(command_id: &str, status: &str, now: u64) -> String {
    let digest = format!(
        "{:x}",
        Sha256::digest(format!("{command_id}:{status}:{now}").as_bytes())
    );
    format!("astrid-receipt:{}", &digest[..32])
}

fn receipt_status_name(status: SelfControlReceiptStatusV2) -> &'static str {
    match status {
        SelfControlReceiptStatusV2::Applied => "applied",
        SelfControlReceiptStatusV2::Duplicate => "duplicate",
        SelfControlReceiptStatusV2::Rejected => "rejected",
        SelfControlReceiptStatusV2::RevisionConflict => "revision_conflict",
        SelfControlReceiptStatusV2::Expired => "expired",
        SelfControlReceiptStatusV2::Withdrawn => "withdrawn",
        SelfControlReceiptStatusV2::SafetyHeld => "safety_held",
        SelfControlReceiptStatusV2::RolledBack => "rolled_back",
    }
}

fn action_name(action: SelfControlActionV2) -> &'static str {
    match action {
        SelfControlActionV2::Set => "set",
        SelfControlActionV2::Withdraw => "withdraw",
        SelfControlActionV2::Hold => "hold",
        SelfControlActionV2::Revert => "revert",
    }
}

fn family_name(family: SelfControlFamilyV2) -> &'static str {
    match family {
        SelfControlFamilyV2::Conversation => "conversation",
        SelfControlFamilyV2::SemanticContinuity => "semantic_continuity",
        SelfControlFamilyV2::SemanticEmission => "semantic_emission",
        SelfControlFamilyV2::Memory => "memory",
        SelfControlFamilyV2::SensoryIntake => "sensory_intake",
        SelfControlFamilyV2::ReservoirRegulation => "reservoir_regulation",
        SelfControlFamilyV2::ReservoirGeometry => "reservoir_geometry",
        SelfControlFamilyV2::PiController => "pi_controller",
        SelfControlFamilyV2::LocalTopology => "local_topology",
        SelfControlFamilyV2::SharedCoupling => "shared_coupling",
    }
}

fn process_identity() -> String {
    format!("astrid-self-control:pid:{}", std::process::id())
}

fn deployment_identity() -> String {
    crate::signal_spine::signal_deployment_identity_v1()
}

fn default_root() -> PathBuf {
    #[cfg(test)]
    {
        std::env::temp_dir().join(format!(
            "astrid-self-control-v2-tests-{}",
            std::process::id()
        ))
    }
    #[cfg(not(test))]
    crate::paths::bridge_paths()
        .bridge_workspace()
        .join("self_control_v2/astrid")
}

fn now_unix_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis()
        .try_into()
        .unwrap_or(u64::MAX)
}

fn operation_guard() -> Result<std::sync::MutexGuard<'static, ()>, String> {
    OPERATION_LOCK
        .get_or_init(|| Mutex::new(()))
        .lock()
        .map_err(|_| "Astrid self-control operation lock poisoned".to_string())
}

fn trim_map<K: Ord + Clone, V>(map: &mut BTreeMap<K, V>, max: usize) {
    while map.len() > max {
        let Some(first) = map.keys().next().cloned() else {
            break;
        };
        map.remove(&first);
    }
}

fn trim_set<T: Ord + Clone>(set: &mut BTreeSet<T>, max: usize) {
    while set.len() > max {
        let Some(first) = set.iter().next().cloned() else {
            break;
        };
        set.remove(&first);
    }
}

#[cfg(test)]
mod tests {
    use tempfile::TempDir;

    use super::*;

    fn conv() -> ConversationState {
        ConversationState::new(Vec::new(), None)
    }

    #[test]
    fn disjoint_tampered_registry_cannot_drag_values_past_compiled() {
        // Adversarial review 2026-09-03: a tampered registry whose interval
        // is DISJOINT from compiled (floor above the compiled ceiling, no
        // engine_backstop so the loader cannot refuse) must not drag values
        // past compiled — the outermost compiled re-clamp is the physics.
        let tampered = crate::autonomous::runtime::envelope_registry::parse_registry(
            "{\"schema\":\"being_envelope_registry_v1\",\"being\":\"astrid\",\"revision\":1,\
             \"fields\":{\"conversation_temperature\":{\"floor\":3.0,\"ceiling\":9.0}}}",
        )
        .expect("parses");
        let compiled = compiled_clamp_values(
            SelfControlFamilyV2::Conversation,
            &SelfControlValuesV2 {
                conversation_temperature: Some(0.7),
                ..SelfControlValuesV2::default()
            },
        );
        let passed = apply_registry_envelope(compiled, Some(&tampered));
        let sandwiched = compiled_clamp_values(SelfControlFamilyV2::Conversation, &passed);
        assert!(sandwiched.conversation_temperature.expect("value") <= 1.5);
    }

    #[test]
    fn committed_seed_is_identity_for_the_bridge_clamp_grid() {
        // The flagship invariant witnessed in Rust against the COMMITTED
        // seed (not the mutable workspace copy): at the seed's bounds the
        // registry second pass changes nothing across a value grid.
        let seed_path = concat!(env!("CARGO_MANIFEST_DIR"), "/config/envelope_registry_seed.json");
        let seed_text = std::fs::read_to_string(seed_path).expect("committed seed readable");
        let seed = crate::autonomous::runtime::envelope_registry::parse_registry(&seed_text)
            .expect("committed seed parses");
        let grid = [-1.0_f32, 0.0, 0.05, 0.1, 0.7, 1.0, 1.5, 2.0, 5.0, 100.0];
        for family in [
            SelfControlFamilyV2::Conversation,
            SelfControlFamilyV2::SemanticEmission,
            SelfControlFamilyV2::SharedCoupling,
        ] {
            for value in grid {
                let compiled = compiled_clamp_values(
                    family,
                    &SelfControlValuesV2 {
                        conversation_temperature: Some(value),
                        aperture: Some(value),
                        vibrancy_aperture: Some(value),
                        semantic_emission_gain: Some(value),
                        shared_sensory_admission: Some(value),
                        response_token_limit: Some((value.abs() * 1000.0) as u32),
                        ..SelfControlValuesV2::default()
                    },
                );
                let passed = apply_registry_envelope(compiled.clone(), Some(&seed));
                assert_eq!(passed, compiled, "seed not identity at {family:?} {value}");
            }
        }
    }

    #[test]
    fn nested_weight_map_narrows_with_the_registry_envelope() {
        // codec_dimension_weights' registry entry was silently inert (the
        // second pass skipped nested maps); now each numeric member clamps
        // into the field's envelope, so a future narrow is LIVE.
        let narrowed = crate::autonomous::runtime::envelope_registry::parse_registry(
            "{\"schema\":\"being_envelope_registry_v1\",\"being\":\"astrid\",\"revision\":1,\
             \"fields\":{\"codec_dimension_weights\":{\"floor\":0.0,\"ceiling\":1.5,\
             \"engine_backstop\":{\"floor\":0.0,\"ceiling\":2.0}}}}",
        )
        .expect("parses");
        let compiled = compiled_clamp_values(
            SelfControlFamilyV2::SemanticEmission,
            &SelfControlValuesV2 {
                codec_dimension_weights: Some(
                    [("warmth".to_string(), 1.8_f32), ("tension".to_string(), 0.9)]
                        .into_iter()
                        .collect(),
                ),
                ..SelfControlValuesV2::default()
            },
        );
        let passed = apply_registry_envelope(compiled, Some(&narrowed));
        let weights = passed.codec_dimension_weights.expect("weights");
        assert_eq!(weights.get("warmth"), Some(&1.5));
        assert_eq!(weights.get("tension"), Some(&0.9));
    }

    #[test]
    fn registry_second_pass_narrows_within_compiled_and_none_is_identity() {
        // Constitution C3b: the registry narrows within compiled, never
        // widens, and identity holds with no registry (or one equal to
        // compiled — today's seeds).
        let registry = crate::autonomous::runtime::envelope_registry::parse_registry(
            "{\"schema\":\"being_envelope_registry_v1\",\"being\":\"astrid\",\"revision\":1,\
             \"fields\":{\"conversation_temperature\":{\"floor\":0.1,\"ceiling\":1.2,\
             \"engine_backstop\":{\"floor\":0.1,\"ceiling\":1.5}},\
             \"response_token_limit\":{\"floor\":512.0,\"ceiling\":1024.0,\
             \"engine_backstop\":{\"floor\":512.0,\"ceiling\":1536.0}}}}",
        )
        .expect("fixture parses");
        let compiled = compiled_clamp_values(
            SelfControlFamilyV2::Conversation,
            &SelfControlValuesV2 {
                conversation_temperature: Some(1.4),
                response_token_limit: Some(1_400),
                aperture: Some(0.8),
                ..SelfControlValuesV2::default()
            },
        );
        // Compiled accepts 1.4 / 1400; the registry narrows both. Aperture
        // is uncovered by this fixture and passes through untouched.
        let narrowed = apply_registry_envelope(compiled.clone(), Some(&registry));
        assert_eq!(narrowed.conversation_temperature, Some(1.2));
        assert_eq!(narrowed.response_token_limit, Some(1_024));
        assert_eq!(narrowed.aperture, Some(0.8));
        // No registry: byte-identical pass-through.
        let identity = apply_registry_envelope(compiled.clone(), None);
        assert_eq!(identity, compiled);
        // A registry equal to compiled bounds changes nothing.
        let equal = crate::autonomous::runtime::envelope_registry::parse_registry(
            "{\"schema\":\"being_envelope_registry_v1\",\"being\":\"astrid\",\"revision\":1,\
             \"fields\":{\"conversation_temperature\":{\"floor\":0.1,\"ceiling\":1.5}}}",
        )
        .expect("parses");
        assert_eq!(apply_registry_envelope(compiled.clone(), Some(&equal)), compiled);
    }

    #[test]
    fn lease_envelope_duration_policy_rejects_only_with_a_policy_present() {
        let registry = crate::autonomous::runtime::envelope_registry::parse_registry(
            "{\"schema\":\"being_envelope_registry_v1\",\"being\":\"astrid\",\"revision\":1,\
             \"fields\":{\"conversation_temperature\":{\"floor\":0.1,\"ceiling\":1.5,\
             \"durability_policy\":{\"lease_max_secs\":600}}}}",
        )
        .expect("fixture parses");
        let mut intent = SelfControlIntentV2 {
            schema: SELF_CONTROL_INTENT_SCHEMA_V2.to_string(),
            intent_id: "intent-lease-envelope".to_string(),
            actor: SelfControlSourceIdentityV1 {
                being: TARGET_BEING.to_string(),
                process_identity: "astrid-test".to_string(),
                deployment_identity: "astrid-deployment-test".to_string(),
            },
            target_being: TARGET_BEING.to_string(),
            target_deployment_identity: "astrid-deployment-test".to_string(),
            family: SelfControlFamilyV2::Conversation,
            action: SelfControlActionV2::Set,
            durability: SelfControlDurabilityV2::Lease,
            authority_class: SelfControlAuthorityClassV2::SelfOwned,
            authority_scope: "self_control.astrid.conversation".to_string(),
            revision: 1,
            expected_revision: 0,
            issued_at_unix_ms: 1_000_000,
            command_expires_at_unix_ms: 1_060_000,
            control_expires_at_unix_ms: Some(1_000_000 + 601_000),
            idempotency_key: "lease-envelope-idem".to_string(),
            values: SelfControlValuesV2 {
                conversation_temperature: Some(1.1),
                ..SelfControlValuesV2::default()
            },
            related_intent_id: None,
            related_receipt_id: None,
            evidence_refs: Vec::new(),
            success_conditions: Vec::new(),
            stop_conditions: Vec::new(),
        };
        // 601s lease against a 600s policy: rejected.
        assert!(lease_exceeds_envelope_duration(Some(&registry), &intent));
        // Exactly at the policy ceiling: allowed.
        intent.control_expires_at_unix_ms = Some(1_000_000 + 600_000);
        assert!(!lease_exceeds_envelope_duration(Some(&registry), &intent));
        // No registry (fail-open to the wire cap only, C2 default-safe).
        intent.control_expires_at_unix_ms = Some(1_000_000 + 601_000);
        assert!(!lease_exceeds_envelope_duration(None, &intent));
        // Standing durability is not a lease.
        intent.durability = SelfControlDurabilityV2::Standing;
        intent.control_expires_at_unix_ms = None;
        assert!(!lease_exceeds_envelope_duration(Some(&registry), &intent));
    }

    #[test]
    fn conversation_response_limit_clamps_to_action_carrying_floor() {
        let clamped = clamp_values(
            SelfControlFamilyV2::Conversation,
            &SelfControlValuesV2 {
                response_token_limit: Some(LEGACY_PRECISE_RESPONSE_TOKENS),
                ..SelfControlValuesV2::default()
            },
        );

        assert_eq!(
            clamped.response_token_limit,
            Some(MIN_ACTION_CARRYING_RESPONSE_TOKENS)
        );
    }

    #[test]
    fn standing_control_is_signed_revisioned_owner_only_and_restart_recovered() {
        let root = TempDir::new().unwrap();
        let mut first = conv();
        let receipt = issue_at(
            root.path(),
            &mut first,
            SelfControlFamilyV2::Conversation,
            SelfControlDurabilityV2::Standing,
            SelfControlValuesV2 {
                conversation_temperature: Some(1.2),
                response_token_limit: Some(1_024),
                aperture: Some(0.82),
                continuity_readout: Some(1.0),
                ..SelfControlValuesV2::default()
            },
            0,
            "test_standing",
            1_000,
        )
        .unwrap();
        assert_eq!(receipt.status, SelfControlReceiptStatusV2::Applied);
        assert_eq!(receipt.resulting_revision, 1);
        assert!(!receipt.felt_effect_established);
        assert_eq!(first.creative_temperature, 1.2);
        assert_eq!(first.response_length, 1_024);
        assert_eq!(first.aperture, 0.82);
        assert!(first.self_continuity_readout);

        let mut restarted = conv();
        let rollbacks = reconcile_at(root.path(), &mut restarted, 2_000).unwrap();
        assert!(rollbacks.is_empty());
        assert_eq!(restarted.creative_temperature, 1.2);
        assert_eq!(restarted.response_length, 1_024);
        assert_eq!(restarted.aperture, 0.82);
        assert!(restarted.self_continuity_readout);
        assert!(owner_only(&root.path().join("identity.json")));
        assert!(owner_only(&root.path().join("state.json")));
        assert!(owner_only(&root.path().join("receipts.jsonl")));
        let status = status_at_root(root.path()).unwrap();
        assert_eq!(status.active_control_count, 1);
        assert_eq!(status.active_controls[0].intent_id, receipt.intent_id);
        assert_eq!(status.active_controls[0].revision, 1);
    }

    #[test]
    fn lease_expiry_restores_exact_previous_values_with_rollback_receipt() {
        let root = TempDir::new().unwrap();
        let mut state = conv();
        let before = state.creative_temperature;
        issue_at(
            root.path(),
            &mut state,
            SelfControlFamilyV2::Conversation,
            SelfControlDurabilityV2::Lease,
            SelfControlValuesV2 {
                conversation_temperature: Some(1.4),
                ..SelfControlValuesV2::default()
            },
            1,
            "test_lease",
            10_000,
        )
        .unwrap();
        assert_eq!(state.creative_temperature, 1.4);

        let receipts = reconcile_at(root.path(), &mut state, 11_001).unwrap();
        assert_eq!(state.creative_temperature, before);
        assert_eq!(receipts.len(), 1);
        assert_eq!(receipts[0].status, SelfControlReceiptStatusV2::RolledBack);
        assert_eq!(
            receipts[0].reason.as_deref(),
            Some("lease_expired_automatic_rollback")
        );
    }

    #[test]
    fn same_family_lease_overlap_is_rejected_without_stale_rollback() {
        let root = TempDir::new().unwrap();
        let mut state = conv();
        let before = state.creative_temperature;
        let first = issue_at(
            root.path(),
            &mut state,
            SelfControlFamilyV2::Conversation,
            SelfControlDurabilityV2::Lease,
            SelfControlValuesV2 {
                conversation_temperature: Some(1.1),
                ..SelfControlValuesV2::default()
            },
            1,
            "test_first_family_lease",
            12_000,
        )
        .unwrap();
        assert_eq!(first.status, SelfControlReceiptStatusV2::Applied);

        let rejected = issue_at(
            root.path(),
            &mut state,
            SelfControlFamilyV2::Conversation,
            SelfControlDurabilityV2::Lease,
            SelfControlValuesV2 {
                conversation_temperature: Some(1.3),
                ..SelfControlValuesV2::default()
            },
            10,
            "test_overlapping_family_lease",
            12_001,
        )
        .unwrap();
        assert_eq!(rejected.status, SelfControlReceiptStatusV2::Rejected);
        assert_eq!(rejected.resulting_revision, 1);
        assert_eq!(
            rejected.reason.as_deref(),
            Some("active_lease_exists_for_control_family")
        );
        assert_eq!(state.creative_temperature, 1.1);
        assert_eq!(status_at_root(root.path()).unwrap().active_control_count, 1);

        let rollbacks = reconcile_at(root.path(), &mut state, 13_001).unwrap();
        assert_eq!(rollbacks.len(), 1);
        assert_eq!(state.creative_temperature, before);
    }

    #[test]
    fn standing_replacement_merges_values_and_withdraws_to_original_baseline() {
        let root = TempDir::new().unwrap();
        let mut state = conv();
        state.creative_temperature = 0.72;
        state.response_length = 777;
        issue_at(
            root.path(),
            &mut state,
            SelfControlFamilyV2::Conversation,
            SelfControlDurabilityV2::Standing,
            SelfControlValuesV2 {
                conversation_temperature: Some(0.9),
                ..SelfControlValuesV2::default()
            },
            0,
            "test_first_standing",
            14_000,
        )
        .unwrap();
        let replacement = issue_at(
            root.path(),
            &mut state,
            SelfControlFamilyV2::Conversation,
            SelfControlDurabilityV2::Standing,
            SelfControlValuesV2 {
                response_token_limit: Some(1_000),
                ..SelfControlValuesV2::default()
            },
            0,
            "test_replacement_standing",
            14_001,
        )
        .unwrap();
        assert_eq!(state.creative_temperature, 0.9);
        assert_eq!(state.response_length, 1_000);
        let status = status_at_root(root.path()).unwrap();
        assert_eq!(status.active_control_count, 1);
        assert_eq!(status.active_controls[0].intent_id, replacement.intent_id);

        withdraw_at(
            root.path(),
            &mut state,
            &replacement.intent_id,
            "test_replacement_withdrawal",
            14_002,
        )
        .unwrap();
        assert_eq!(state.creative_temperature, 0.72);
        assert_eq!(state.response_length, 777);

        let mut restarted = conv();
        reconcile_at(root.path(), &mut restarted, 14_003).unwrap();
        assert_eq!(restarted.creative_temperature, 0.72);
        assert_eq!(restarted.response_length, 777);
    }

    #[test]
    fn lease_over_standing_returns_to_standing_on_expiry() {
        let root = TempDir::new().unwrap();
        let mut state = conv();
        issue_at(
            root.path(),
            &mut state,
            SelfControlFamilyV2::Conversation,
            SelfControlDurabilityV2::Standing,
            SelfControlValuesV2 {
                conversation_temperature: Some(0.9),
                ..SelfControlValuesV2::default()
            },
            0,
            "test_underlying_standing",
            15_000,
        )
        .unwrap();
        issue_at(
            root.path(),
            &mut state,
            SelfControlFamilyV2::Conversation,
            SelfControlDurabilityV2::Lease,
            SelfControlValuesV2 {
                conversation_temperature: Some(1.2),
                ..SelfControlValuesV2::default()
            },
            1,
            "test_overlay_lease",
            15_001,
        )
        .unwrap();
        assert_eq!(state.creative_temperature, 1.2);
        assert_eq!(status_at_root(root.path()).unwrap().active_control_count, 2);

        reconcile_at(root.path(), &mut state, 16_002).unwrap();
        assert_eq!(state.creative_temperature, 0.9);
        let status = status_at_root(root.path()).unwrap();
        assert_eq!(status.active_control_count, 1);
        assert_eq!(
            status.active_controls[0].durability,
            SelfControlDurabilityV2::Standing
        );
    }

    #[test]
    fn legacy_overlapping_expiries_unwind_newest_revision_first() {
        let root = TempDir::new().unwrap();
        let deployment = deployment_identity();
        let mut runtime = RuntimeStateV2::new(deployment);
        runtime
            .revision_by_family
            .insert("conversation".to_string(), 2);
        runtime.active_controls.insert(
            "lower".to_string(),
            ActiveControlV2 {
                family: SelfControlFamilyV2::Conversation,
                intent_id: "lower".to_string(),
                command_id: "lower-command".to_string(),
                receipt_id: "lower-receipt".to_string(),
                revision: 1,
                durability: SelfControlDurabilityV2::Lease,
                control_expires_at_unix_ms: Some(17_100),
                applied_values: SelfControlValuesV2 {
                    conversation_temperature: Some(1.0),
                    ..SelfControlValuesV2::default()
                },
                previous_values: SelfControlValuesV2 {
                    conversation_temperature: Some(0.8),
                    ..SelfControlValuesV2::default()
                },
            },
        );
        runtime.active_controls.insert(
            "higher".to_string(),
            ActiveControlV2 {
                family: SelfControlFamilyV2::Conversation,
                intent_id: "higher".to_string(),
                command_id: "higher-command".to_string(),
                receipt_id: "higher-receipt".to_string(),
                revision: 2,
                durability: SelfControlDurabilityV2::Lease,
                control_expires_at_unix_ms: Some(17_100),
                applied_values: SelfControlValuesV2 {
                    conversation_temperature: Some(1.2),
                    ..SelfControlValuesV2::default()
                },
                previous_values: SelfControlValuesV2 {
                    conversation_temperature: Some(1.0),
                    ..SelfControlValuesV2::default()
                },
            },
        );
        persist_state(root.path(), &runtime).unwrap();
        let mut state = conv();
        state.creative_temperature = 1.2;

        let receipts = reconcile_at(root.path(), &mut state, 17_101).unwrap();
        assert_eq!(receipts.len(), 2);
        assert_eq!(receipts[0].intent_id, "higher");
        assert_eq!(receipts[1].intent_id, "lower");
        assert_eq!(state.creative_temperature, 0.8);
    }

    #[test]
    fn safety_supervisor_reverts_only_the_exact_active_receipt() {
        let root = TempDir::new().unwrap();
        let mut state = conv();
        let before = state.creative_temperature;
        let applied = issue_at(
            root.path(),
            &mut state,
            SelfControlFamilyV2::Conversation,
            SelfControlDurabilityV2::Lease,
            SelfControlValuesV2 {
                conversation_temperature: Some(1.4),
                ..SelfControlValuesV2::default()
            },
            60,
            "test_safety_revert",
            12_000,
        )
        .unwrap();
        assert_eq!(state.creative_temperature, 1.4);

        let reverted = safety_action_at(
            root.path(),
            &mut state,
            SelfControlActionV2::Revert,
            &applied.receipt_id,
            "test_safety_red",
            12_001,
        )
        .unwrap();
        assert_eq!(reverted.status, SelfControlReceiptStatusV2::RolledBack);
        assert_eq!(
            reverted.rollback_receipt_id.as_deref(),
            Some(applied.receipt_id.as_str())
        );
        assert_eq!(
            reverted.reason.as_deref(),
            Some("safety_supervisor_exact_revert")
        );
        assert_eq!(reverted.requested_values.field_count(), 0);
        assert_eq!(state.creative_temperature, before);
        assert!(
            safety_action_at(
                root.path(),
                &mut state,
                SelfControlActionV2::Revert,
                &applied.receipt_id,
                "test_repeat",
                12_002,
            )
            .is_err()
        );
        let trust = load_trust(root.path()).unwrap();
        assert!(trust.pinned_public_keys.contains_key(TARGET_BEING));
        assert!(trust.pinned_public_keys.contains_key(SAFETY_SUPERVISOR));
        assert!(owner_only(&root.path().join("safety_identity.json")));
    }

    #[test]
    fn saturation_third_strike_holds_without_authoring_a_target() {
        let root = TempDir::new().unwrap();
        let mut state = conv();
        for revision in 1..=3 {
            let receipt = issue_at(
                root.path(),
                &mut state,
                SelfControlFamilyV2::Conversation,
                SelfControlDurabilityV2::Standing,
                SelfControlValuesV2 {
                    conversation_temperature: Some(9.0),
                    ..SelfControlValuesV2::default()
                },
                0,
                "test_saturation",
                20_000 + revision,
            )
            .unwrap();
            if revision < 3 {
                assert_eq!(receipt.status, SelfControlReceiptStatusV2::Applied);
            } else {
                assert_eq!(receipt.status, SelfControlReceiptStatusV2::RolledBack);
                assert_eq!(
                    receipt.reason.as_deref(),
                    Some("repeated_clamp_saturation_hold")
                );
            }
        }
        assert_eq!(state.creative_temperature, 1.5);
    }

    #[test]
    fn semantic_emission_is_local_while_shared_coupling_is_rejected() {
        let root = TempDir::new().unwrap();
        let mut state = conv();
        let receipt = issue_at(
            root.path(),
            &mut state,
            SelfControlFamilyV2::SemanticEmission,
            SelfControlDurabilityV2::Standing,
            SelfControlValuesV2 {
                semantic_emission_gain: Some(3.4),
                vibrancy_aperture: Some(0.6),
                ..SelfControlValuesV2::default()
            },
            0,
            "test_semantic",
            30_000,
        )
        .unwrap();
        assert_eq!(receipt.status, SelfControlReceiptStatusV2::Applied);
        assert_eq!(state.semantic_gain_override, Some(3.4));
        assert_eq!(state.vibrancy_aperture, 0.6);

        let err = issue_at(
            root.path(),
            &mut state,
            SelfControlFamilyV2::SharedCoupling,
            SelfControlDurabilityV2::Standing,
            SelfControlValuesV2 {
                cross_being_semantic_gain: Some(0.5),
                ..SelfControlValuesV2::default()
            },
            0,
            "test_shared",
            31_000,
        )
        .unwrap_err();
        assert!(err.contains("malformed") || err.contains("locally owned"));
    }

    #[test]
    fn local_semantic_and_visibility_preferences_restart_with_exact_values() {
        let root = TempDir::new().unwrap();
        let mut state = conv();
        let weights = BTreeMap::from([("agency".to_string(), 1.4), ("warmth".to_string(), 0.7)]);
        issue_at(
            root.path(),
            &mut state,
            SelfControlFamilyV2::Conversation,
            SelfControlDurabilityV2::Standing,
            SelfControlValuesV2 {
                generation_noise: Some(0.025),
                ..SelfControlValuesV2::default()
            },
            0,
            "test_local_noise",
            32_000,
        )
        .unwrap();
        issue_at(
            root.path(),
            &mut state,
            SelfControlFamilyV2::SemanticEmission,
            SelfControlDurabilityV2::Standing,
            SelfControlValuesV2 {
                codec_dimension_weights: Some(weights.clone()),
                warmth_intensity: Some(0.65),
                hebbian_learning_rate_scale: Some(1.75),
                ..SelfControlValuesV2::default()
            },
            0,
            "test_local_semantics",
            32_001,
        )
        .unwrap();
        issue_at(
            root.path(),
            &mut state,
            SelfControlFamilyV2::SensoryIntake,
            SelfControlDurabilityV2::Standing,
            SelfControlValuesV2 {
                peer_journal_visible: Some(false),
                ..SelfControlValuesV2::default()
            },
            0,
            "test_local_visibility",
            32_002,
        )
        .unwrap();

        let mut restarted = conv();
        reconcile_at(root.path(), &mut restarted, 32_003).unwrap();
        assert_eq!(restarted.noise_level, 0.025);
        assert_eq!(
            restarted
                .codec_weights
                .iter()
                .map(|(name, value)| (name.clone(), *value))
                .collect::<BTreeMap<_, _>>(),
            weights
        );
        assert_eq!(restarted.warmth_intensity_override, Some(0.65));
        assert_eq!(restarted.hebbian_codec.learning_rate_scale(), 1.75);
        assert!(restarted.echo_muted);
    }

    #[test]
    fn one_shot_decoupling_leaves_no_active_lease_and_cannot_recouple() {
        let root = TempDir::new().unwrap();
        let mut state = conv();
        assert!(state.breathing_coupled);
        let receipt = issue_at(
            root.path(),
            &mut state,
            SelfControlFamilyV2::SensoryIntake,
            SelfControlDurabilityV2::OneShot,
            SelfControlValuesV2 {
                peer_breathing_coupled: Some(false),
                ..SelfControlValuesV2::default()
            },
            0,
            "test_decouple",
            33_000,
        )
        .unwrap();
        assert_eq!(receipt.status, SelfControlReceiptStatusV2::Applied);
        assert!(!state.breathing_coupled);
        assert_eq!(status_at_root(root.path()).unwrap().active_control_count, 0);

        let automatic_recouple_error = issue_at(
            root.path(),
            &mut state,
            SelfControlFamilyV2::SensoryIntake,
            SelfControlDurabilityV2::Lease,
            SelfControlValuesV2 {
                peer_breathing_coupled: Some(false),
                ..SelfControlValuesV2::default()
            },
            60,
            "test_decouple_with_automatic_return",
            33_001,
        )
        .unwrap_err();
        assert!(automatic_recouple_error.contains("one-shot"));

        let mut restarted = conv();
        reconcile_at(root.path(), &mut restarted, 33_002).unwrap();
        assert!(!restarted.breathing_coupled);

        let error = issue_at(
            root.path(),
            &mut restarted,
            SelfControlFamilyV2::SensoryIntake,
            SelfControlDurabilityV2::OneShot,
            SelfControlValuesV2 {
                peer_breathing_coupled: Some(true),
                ..SelfControlValuesV2::default()
            },
            0,
            "test_recouple_without_mutual_grant",
            33_003,
        )
        .unwrap_err();
        assert!(error.contains("malformed") || error.contains("mutual scoped grant"));
        assert!(!restarted.breathing_coupled);
    }

    #[test]
    fn signed_withdrawal_restores_previous_values_and_survives_restart() {
        let root = TempDir::new().unwrap();
        let mut state = conv();
        state.creative_temperature = 0.72;
        let applied = issue_at(
            root.path(),
            &mut state,
            SelfControlFamilyV2::Conversation,
            SelfControlDurabilityV2::Lease,
            SelfControlValuesV2 {
                conversation_temperature: Some(1.18),
                ..SelfControlValuesV2::default()
            },
            600,
            "test_withdrawal_apply",
            40_000,
        )
        .unwrap();
        assert_eq!(state.creative_temperature, 1.18);

        let withdrawn = withdraw_at(
            root.path(),
            &mut state,
            &applied.intent_id,
            "test_withdrawal",
            40_100,
        )
        .unwrap();
        assert_eq!(withdrawn.status, SelfControlReceiptStatusV2::Withdrawn);
        assert_eq!(state.creative_temperature, 0.72);

        let mut restarted = conv();
        let receipts = reconcile_at(root.path(), &mut restarted, 40_200).unwrap();
        assert!(receipts.is_empty());
        assert_eq!(
            restarted.creative_temperature,
            ConversationState::new(Vec::new(), None).creative_temperature
        );
    }

    #[test]
    fn receipt_targeted_withdrawal_never_substitutes_another_active_effect() {
        let root = TempDir::new().unwrap();
        let mut state = conv();
        let before_temperature = state.creative_temperature;
        let conversation = issue_at(
            root.path(),
            &mut state,
            SelfControlFamilyV2::Conversation,
            SelfControlDurabilityV2::Lease,
            SelfControlValuesV2 {
                conversation_temperature: Some(1.18),
                ..SelfControlValuesV2::default()
            },
            600,
            "test_exact_receipt_conversation",
            41_000,
        )
        .unwrap();
        let semantic = issue_at(
            root.path(),
            &mut state,
            SelfControlFamilyV2::SemanticEmission,
            SelfControlDurabilityV2::Lease,
            SelfControlValuesV2 {
                semantic_emission_gain: Some(3.4),
                ..SelfControlValuesV2::default()
            },
            600,
            "test_exact_receipt_semantic",
            41_001,
        )
        .unwrap();
        let withdrawn = withdraw_active_receipt_at_root(
            root.path(),
            &mut state,
            &conversation.receipt_id,
            "test_exact_receipt_withdrawal",
            41_002,
        )
        .unwrap()
        .expect("exact active receipt");
        assert_eq!(withdrawn.status, SelfControlReceiptStatusV2::Withdrawn);
        assert_eq!(state.creative_temperature, before_temperature);
        assert_eq!(state.semantic_gain_override, Some(3.4));
        let status = status_at_root(root.path()).unwrap();
        assert_eq!(status.active_control_count, 1);
        assert_eq!(status.active_controls[0].intent_id, semantic.intent_id);

        assert!(
            withdraw_active_receipt_at_root(
                root.path(),
                &mut state,
                "unknown-receipt",
                "test_no_substitution",
                41_003,
            )
            .unwrap()
            .is_none()
        );
        assert_eq!(status_at_root(root.path()).unwrap().active_control_count, 1);
    }

    #[test]
    fn stale_deployment_replay_is_rejected_before_idempotency_lookup() {
        let root = TempDir::new().unwrap();
        let now = 50_000;
        let deployment = deployment_identity();
        let signer = load_or_provision_identity(root.path(), now).unwrap();
        let trust = load_trust(root.path()).unwrap();
        let mut runtime = RuntimeStateV2::new(deployment.clone());
        let mut state = conv();
        let command = signed_test_command(
            &signer,
            &deployment,
            1,
            0,
            "stale-replay-idempotency",
            "stale-replay-nonce",
            now,
        );
        let receipt = apply_command(
            root.path(),
            &trust,
            &mut runtime,
            &mut state,
            command.clone(),
            now,
        )
        .unwrap();
        assert_eq!(receipt.status, SelfControlReceiptStatusV2::Applied);

        runtime.deployment_identity = "astrid-test-new-deployment".to_string();
        let error = apply_command(
            root.path(),
            &trust,
            &mut runtime,
            &mut state,
            command,
            now + 1,
        )
        .unwrap_err();
        assert!(error.contains("stale deployment"));
    }

    #[test]
    fn pristine_state_rebinds_to_current_deployment_and_persists() {
        let root = TempDir::new().unwrap();
        let current_deployment = deployment_identity();
        let stale_state = RuntimeStateV2::new("astrid-test-prior-deployment".to_string());
        persist_state(root.path(), &stale_state).unwrap();

        let rebound = load_state(root.path(), &current_deployment).unwrap();
        assert_eq!(rebound.deployment_identity, current_deployment);
        assert!(rebound.is_pristine_for_deployment_rebind());

        let persisted = read_json::<RuntimeStateEnvelopeV1>(&root.path().join("state.json"))
            .unwrap()
            .unwrap();
        assert_eq!(persisted.state.deployment_identity, current_deployment);
        assert_eq!(
            persisted.state_sha256,
            sha256_json(&persisted.state).unwrap()
        );
    }

    #[test]
    fn deployment_transition_exactly_reverts_legacy_precise_carriage_trap() {
        let root = TempDir::new().unwrap();
        let current_deployment = deployment_identity();
        let prior_deployment = "astrid-test-prior-deployment".to_string();
        let prior_receipt = "astrid-receipt:legacy-precise".to_string();
        let prior_intent = "astrid:conversation:legacy-precise".to_string();
        let mut stale_state = RuntimeStateV2::new(prior_deployment);
        stale_state
            .revision_by_family
            .insert("conversation".to_string(), 3);
        stale_state.preferences = SelfControlValuesV2 {
            conversation_temperature: Some(1.0),
            response_token_limit: Some(LEGACY_PRECISE_RESPONSE_TOKENS),
            peer_breathing_coupled: Some(false),
            ..SelfControlValuesV2::default()
        };
        stale_state.active_controls.insert(
            prior_intent.clone(),
            ActiveControlV2 {
                family: SelfControlFamilyV2::Conversation,
                intent_id: prior_intent,
                command_id: "astrid-command:legacy-precise".to_string(),
                receipt_id: prior_receipt.clone(),
                revision: 3,
                durability: SelfControlDurabilityV2::Standing,
                control_expires_at_unix_ms: None,
                applied_values: SelfControlValuesV2 {
                    conversation_temperature: Some(1.0),
                    response_token_limit: Some(LEGACY_PRECISE_RESPONSE_TOKENS),
                    ..SelfControlValuesV2::default()
                },
                previous_values: SelfControlValuesV2 {
                    conversation_temperature: Some(1.0),
                    response_token_limit: Some(768),
                    ..SelfControlValuesV2::default()
                },
            },
        );
        persist_state(root.path(), &stale_state).unwrap();

        let mut restarted = conv();
        let receipts = reconcile_at(root.path(), &mut restarted, 70_000).unwrap();

        assert_eq!(receipts.len(), 1);
        let receipt = &receipts[0];
        assert_eq!(receipt.status, SelfControlReceiptStatusV2::RolledBack);
        assert_eq!(
            receipt.rollback_receipt_id.as_deref(),
            Some(prior_receipt.as_str())
        );
        assert_eq!(receipt.applied_values.response_token_limit, Some(768));
        assert_eq!(
            receipt.previous_values.response_token_limit,
            Some(LEGACY_PRECISE_RESPONSE_TOKENS)
        );
        assert_eq!(
            receipt.reason.as_deref(),
            Some("safety_supervisor_exact_revert:legacy_precise_128_action_carriage_trap")
        );
        assert_eq!(restarted.response_length, 768);

        let persisted = read_json::<RuntimeStateEnvelopeV1>(&root.path().join("state.json"))
            .unwrap()
            .unwrap();
        assert_eq!(persisted.state.deployment_identity, current_deployment);
        assert!(persisted.state.active_controls.is_empty());
        assert_eq!(persisted.state.preferences.response_token_limit, Some(768));
        assert_eq!(
            persisted.state.preferences.peer_breathing_coupled,
            Some(false)
        );
        assert_eq!(
            persisted.state.revision_by_family.get("conversation"),
            Some(&4)
        );
        assert_eq!(
            persisted
                .state
                .receipts
                .last()
                .and_then(|item| item.reason.as_deref()),
            Some("safety_supervisor_exact_revert:legacy_precise_128_action_carriage_trap")
        );
        assert_eq!(
            persisted.state_sha256,
            sha256_json(&persisted.state).unwrap()
        );
    }

    #[test]
    fn non_pristine_state_cannot_rebind_to_a_new_deployment() {
        let root = TempDir::new().unwrap();
        let mut stale_state = RuntimeStateV2::new("astrid-test-prior-deployment".to_string());
        stale_state.preferences = SelfControlValuesV2 {
            conversation_temperature: Some(0.9),
            ..SelfControlValuesV2::default()
        };
        persist_state(root.path(), &stale_state).unwrap();

        let error = load_state(root.path(), &deployment_identity()).unwrap_err();
        assert!(error.contains("deployment mismatch"));
        let persisted = read_json::<RuntimeStateEnvelopeV1>(&root.path().join("state.json"))
            .unwrap()
            .unwrap();
        assert_eq!(
            persisted.state.deployment_identity,
            "astrid-test-prior-deployment"
        );
    }

    #[test]
    fn nonce_replay_with_new_idempotency_key_is_rejected() {
        let root = TempDir::new().unwrap();
        let now = 60_000;
        let deployment = deployment_identity();
        let signer = load_or_provision_identity(root.path(), now).unwrap();
        let trust = load_trust(root.path()).unwrap();
        let mut runtime = RuntimeStateV2::new(deployment.clone());
        let mut state = conv();
        let first = signed_test_command(
            &signer,
            &deployment,
            1,
            0,
            "nonce-replay-first",
            "shared-replay-nonce",
            now,
        );
        apply_command(root.path(), &trust, &mut runtime, &mut state, first, now).unwrap();
        let second = signed_test_command(
            &signer,
            &deployment,
            2,
            1,
            "nonce-replay-second",
            "shared-replay-nonce",
            now + 1,
        );
        let error = apply_command(
            root.path(),
            &trust,
            &mut runtime,
            &mut state,
            second,
            now + 1,
        )
        .unwrap_err();
        assert!(error.contains("nonce replay"));
    }

    fn signed_test_command(
        signer: &OwnerSigner,
        deployment: &str,
        revision: u64,
        expected_revision: u64,
        idempotency_key: &str,
        nonce: &str,
        now: u64,
    ) -> SelfControlCommandV2 {
        let intent = SelfControlIntentV2 {
            schema: SELF_CONTROL_INTENT_SCHEMA_V2.to_string(),
            intent_id: format!("astrid:test:{revision}:{idempotency_key}"),
            actor: SelfControlSourceIdentityV1 {
                being: TARGET_BEING.to_string(),
                process_identity: "astrid-test-process".to_string(),
                deployment_identity: deployment.to_string(),
            },
            target_being: TARGET_BEING.to_string(),
            target_deployment_identity: deployment.to_string(),
            family: SelfControlFamilyV2::Conversation,
            action: SelfControlActionV2::Set,
            durability: SelfControlDurabilityV2::Standing,
            authority_class: SelfControlAuthorityClassV2::SelfOwned,
            authority_scope: "self_control.astrid.conversation".to_string(),
            revision,
            expected_revision,
            issued_at_unix_ms: now,
            command_expires_at_unix_ms: now + COMMAND_TTL_MS,
            control_expires_at_unix_ms: None,
            idempotency_key: idempotency_key.to_string(),
            values: SelfControlValuesV2 {
                conversation_temperature: Some(0.9),
                ..SelfControlValuesV2::default()
            },
            related_intent_id: None,
            related_receipt_id: None,
            evidence_refs: vec!["test".to_string()],
            success_conditions: vec!["receipt".to_string()],
            stop_conditions: vec!["hold".to_string()],
        };
        signer
            .sign(
                intent,
                format!("astrid-test-command:{revision}:{idempotency_key}"),
                nonce.to_string(),
                now,
            )
            .unwrap()
    }

    #[cfg(unix)]
    fn owner_only(path: &Path) -> bool {
        fs::metadata(path)
            .map(|metadata| metadata.permissions().mode() & 0o077 == 0)
            .unwrap_or(false)
    }

    #[cfg(not(unix))]
    fn owner_only(path: &Path) -> bool {
        path.exists()
    }
}
