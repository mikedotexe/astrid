use std::fs::{self, OpenOptions};
use std::io::Write as _;
#[cfg(unix)]
use std::os::unix::fs::{OpenOptionsExt as _, PermissionsExt as _};
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use astrid_minime_protocol::{
    BEING_UTTERANCE_ATTESTATION_SCHEMA_V1, BeingUtteranceAttestationV1, PerceptibleReturnV1,
    VOLITION_DECISION_SCHEMA_V1, VOLITION_INTENT_SCHEMA_V1, VOLITION_RECEIPT_SCHEMA_V1,
    VolitionActionV1, VolitionActorIdentityV1, VolitionAuthorityClassV1, VolitionDecisionStatusV1,
    VolitionDecisionV1, VolitionDurabilityV1, VolitionIntentV1, VolitionOperationV1,
    VolitionReceiptStatusV1, VolitionReceiptV1, canonical_being_utterance_attestation_sha256,
    canonical_volition_action_sha256, canonical_volition_intent_sha256,
};
use ed25519_dalek::{Signer as _, SigningKey};
use rand::rngs::OsRng;
use serde::{Deserialize, Serialize, de::DeserializeOwned};
use serde_json::{Value, json};
use sha2::{Digest as _, Sha256};

use crate::action_continuity::NextActionOutcome;

use super::delegated_capability;

const IDENTITY_SCHEMA_V1: &str = "astrid.volition.attestor_identity.v1";
const SHADOW_SCHEMA_V1: &str = "astrid.volition.shadow_authority.v1";
const SHADOW_OUTCOME_SCHEMA_V1: &str = "astrid.volition.shadow_outcome.v1";
const DISPATCH_PERMIT_SCHEMA_V1: &str = "astrid.volition.dispatch_permit.v1";
const COMMAND_TTL_MILLIS: u64 = 30_000;

#[derive(Clone, Debug, Serialize, Deserialize)]
struct StoredAttestorIdentityV1 {
    schema: String,
    being: String,
    public_key_hex: String,
    signing_key_seed_hex: String,
    created_at_unix_ms: u64,
}

#[derive(Clone)]
struct AttestorSigner {
    public_key_hex: String,
    signing_key: SigningKey,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
struct ShadowAuthorityRecordV1 {
    schema: &'static str,
    record_id: String,
    attestation_id: String,
    attestation_sha256: String,
    requested_action: String,
    proposed_authority_class: VolitionAuthorityClassV1,
    reason: &'static str,
    runtime_consumed: bool,
    authority_granted: bool,
    consent_inferred_from_silence: bool,
    recorded_at_unix_ms: u64,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
struct ShadowOutcomeRecordV1 {
    schema: &'static str,
    record_id: String,
    shadow_record_id: String,
    requested_action: String,
    legacy_route: String,
    legacy_status: String,
    legacy_outcome_summary: String,
    volition_authority_consumed: bool,
    recorded_at_unix_ms: u64,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
struct AuthoredDispatchPermitV1 {
    schema: String,
    requested_action: String,
    requested_action_sha256: String,
    source_attestation_id: String,
    intent_id: String,
    target_deployment_identity: String,
    expires_at_unix_ms: u64,
}

#[derive(Clone, Debug)]
pub(super) struct AuthoredActionContextV1 {
    pub source_attestation_id: String,
    pub intent_id: String,
}

pub(super) enum AstridVolitionStartV1 {
    Accepted(Box<AcceptedVolitionV1>),
    Shadowed(ShadowedVolitionV1),
}

impl AstridVolitionStartV1 {
    pub(super) fn dispatch_block_reason(&self) -> Option<String> {
        match self {
            Self::Accepted(_) => None,
            Self::Shadowed(shadowed) => Some(format!(
                "`{}` requires elevated authority; no current exact grant was consumed",
                shadowed.requested_action
            )),
        }
    }
}

pub(super) struct AcceptedVolitionV1 {
    root: PathBuf,
    intent: VolitionIntentV1,
    decision: VolitionDecisionV1,
    dispatch_permit_path: PathBuf,
    delegated_binding_id: Option<String>,
}

pub(super) struct ShadowedVolitionV1 {
    root: PathBuf,
    record_id: String,
    requested_action: String,
}

pub(super) fn mode_supports_being_attestation(mode_name: &str) -> bool {
    matches!(
        mode_name,
        "dialogue_live"
            | "daydream"
            | "aspiration"
            | "creation"
            | "initiate"
            | "experiment"
            | "evolve"
            // Both producers return model text; preparation/carriage failures
            // use separate, ineligible notice modes.
            | "self_study"
    )
}

pub(super) fn begin_astrid_next(
    response: &str,
    exchange_id: &str,
    mode_name: &str,
    requested_action: &str,
) -> Result<AstridVolitionStartV1, String> {
    begin_astrid_next_at_root(
        &default_root(),
        response,
        exchange_id,
        mode_name,
        requested_action,
        now_unix_ms(),
    )
}

pub(super) fn complete_astrid_next(
    start: AstridVolitionStartV1,
    outcome: &NextActionOutcome,
) -> Result<String, String> {
    match start {
        AstridVolitionStartV1::Accepted(accepted) => accepted.complete(outcome, now_unix_ms()),
        AstridVolitionStartV1::Shadowed(shadowed) => shadowed.complete(outcome, now_unix_ms()),
    }
}

pub(super) fn exact_self_owned_action_context(
    requested_action: &str,
) -> Result<AuthoredActionContextV1, String> {
    exact_self_owned_action_context_at_root(&default_root(), requested_action, now_unix_ms())
}

fn begin_astrid_next_at_root(
    root: &Path,
    response: &str,
    exchange_id: &str,
    mode_name: &str,
    requested_action: &str,
    now: u64,
) -> Result<AstridVolitionStartV1, String> {
    if !mode_supports_being_attestation(mode_name) {
        return Err(format!(
            "mode `{mode_name}` is runtime-generated or mirrored and cannot be attested as Astrid-authored"
        ));
    }
    ensure_owner_tree(root)?;
    let signer = load_or_provision_identity(root, now)?;
    let response_sha256 = format!("{:x}", Sha256::digest(response.as_bytes()));
    let entropy = rand::random::<u64>();
    let attestation_id = format!(
        "astrid-attestation-{exchange_id}-{}-{entropy:016x}",
        &response_sha256[..12]
    );
    let deployment_identity = crate::signal_spine::signal_deployment_identity_v1();
    let mut attestation = BeingUtteranceAttestationV1 {
        schema: BEING_UTTERANCE_ATTESTATION_SCHEMA_V1.to_string(),
        attestation_id: attestation_id.clone(),
        being: "astrid".to_string(),
        exchange_id: exchange_id.to_string(),
        response_sha256,
        response_len_bytes: u64::try_from(response.len())
            .map_err(|_| "Astrid response length does not fit u64".to_string())?,
        model: std::env::var("ASTRID_MODEL")
            .or_else(|_| std::env::var("OLLAMA_MODEL"))
            .unwrap_or_else(|_| "runtime-selected".to_string()),
        provider: std::env::var("ASTRID_LLM_PROVIDER")
            .unwrap_or_else(|_| "spectral-bridge".to_string()),
        model_deployment_identity: deployment_identity.clone(),
        captured_at_unix_ms: now,
        attestor_process_identity: format!("spectral-bridge:pid:{}", std::process::id()),
        attestor_deployment_identity: deployment_identity.clone(),
        attestor_public_key_hex: signer.public_key_hex.clone(),
        signature_hex: String::new(),
    };
    let signing_bytes = attestation
        .signing_bytes()
        .ok_or_else(|| "encode Astrid utterance attestation".to_string())?;
    attestation.signature_hex = hex::encode(signer.signing_key.sign(&signing_bytes).to_bytes());
    write_owner_json(
        &root
            .join("attestations")
            .join(format!("{attestation_id}.json")),
        &attestation,
    )?;
    let attestation_sha256 = canonical_being_utterance_attestation_sha256(&attestation);
    let authority_class = authority_class_for_next(requested_action);
    if matches!(
        authority_class,
        VolitionAuthorityClassV1::Mutual
            | VolitionAuthorityClassV1::OperatorOneShot
            | VolitionAuthorityClassV1::SafetySupervisor
    ) {
        return write_shadow_start(
            root,
            exchange_id,
            entropy,
            attestation_id,
            attestation_sha256,
            requested_action,
            authority_class,
            "shadow_only_until_exact_mutual_or_one_shot_authority_is_bound",
            now,
        );
    }

    let action = VolitionActionV1 {
        operation: VolitionOperationV1::Execute,
        namespace: "astrid_next".to_string(),
        name: next_base(requested_action).to_ascii_lowercase(),
        parameters: json!({"canonical_next": requested_action}),
        reversible: true,
        peer_impacting: false,
        irreversible: false,
        estimated_cost_microunits: None,
    };
    let action_sha256 = canonical_volition_action_sha256(&action);
    let intent_id = format!("astrid-intent-{exchange_id}-{entropy:016x}");
    let delegated_reservation = if authority_class == VolitionAuthorityClassV1::DelegatedExternal {
        match delegated_capability::reserve_for_action(
            root,
            requested_action,
            &intent_id,
            &signer.public_key_hex,
            &deployment_identity,
            now,
        )? {
            Some(reservation) => Some(reservation),
            None => {
                return write_shadow_start(
                    root,
                    exchange_id,
                    entropy,
                    attestation_id,
                    attestation_sha256,
                    requested_action,
                    authority_class,
                    "delegated_capability_unavailable_or_budget_exhausted",
                    now,
                );
            },
        }
    } else {
        None
    };
    let capability_binding_ids = delegated_reservation
        .as_ref()
        .map(|reservation| vec![reservation.binding_id.clone()])
        .unwrap_or_default();
    let mut authority_evidence_refs = vec![format!("attestation:{attestation_id}")];
    if let Some(reservation) = delegated_reservation.as_ref() {
        authority_evidence_refs.extend([
            format!("capability_binding:{}", reservation.binding_id),
            format!("capability_token:{}", reservation.capability_token_id),
            format!("delegated_resource:{}", reservation.resource),
        ]);
    }
    let intent = VolitionIntentV1 {
        schema: VOLITION_INTENT_SCHEMA_V1.to_string(),
        intent_id: intent_id.clone(),
        source_attestation_id: attestation.attestation_id,
        source_attestation_sha256: attestation_sha256,
        actor: VolitionActorIdentityV1 {
            being: "astrid".to_string(),
            principal: "astrid".to_string(),
            process_identity: format!("astrid-autonomy:pid:{}", std::process::id()),
            deployment_identity: deployment_identity.clone(),
        },
        target_being: "astrid".to_string(),
        target_deployment_identity: deployment_identity,
        action: action.clone(),
        authority_class,
        durability: VolitionDurabilityV1::OneShot,
        revision: now,
        expected_revision: now.saturating_sub(1),
        issued_at_unix_ms: now,
        command_expires_at_unix_ms: now.saturating_add(COMMAND_TTL_MILLIS),
        action_expires_at_unix_ms: None,
        idempotency_key: format!("astrid-next-{exchange_id}-{action_sha256}"),
        capability_binding_ids,
        authority_evidence_refs,
        dependency_intent_ids: Vec::new(),
        budget: delegated_reservation
            .as_ref()
            .map(|reservation| reservation.requested_budget.clone())
            .unwrap_or_default(),
        evidence_refs: vec![
            format!("attestation:{attestation_id}"),
            format!("response_mode:{mode_name}"),
        ],
        success_conditions: vec![
            "effective_action_hash_equals_requested_action_hash".to_string(),
            "machine_receipt_is_returned_to_owner".to_string(),
        ],
        stop_conditions: vec![
            "being_authored_hold_or_withdrawal".to_string(),
            "deployment_or_receipt_mismatch".to_string(),
        ],
    };
    if !intent.is_well_formed(now) {
        return Err("constructed Astrid volition intent is not well formed".to_string());
    }
    let decision_id = format!("astrid-decision-{exchange_id}-{entropy:016x}");
    let decision = VolitionDecisionV1 {
        schema: VOLITION_DECISION_SCHEMA_V1.to_string(),
        decision_id: decision_id.clone(),
        intent_id: intent_id.clone(),
        intent_sha256: canonical_volition_intent_sha256(&intent),
        status: VolitionDecisionStatusV1::Accepted,
        authority_class,
        requested_action_sha256: action_sha256.clone(),
        effective_action: Some(action),
        effective_action_sha256: Some(action_sha256),
        target_being: intent.target_being.clone(),
        target_deployment_identity: intent.target_deployment_identity.clone(),
        no_target_substitution: true,
        decided_by: "astrid-volition-broker".to_string(),
        decided_at_unix_ms: now,
        authority_evidence_refs: intent.authority_evidence_refs.clone(),
        reason: Some(
            if authority_class == VolitionAuthorityClassV1::DelegatedExternal {
                "delegated_external_budget_reserved_before_dispatch"
            } else {
                "self_owned_local_one_turn_action"
            }
            .to_string(),
        ),
    };
    if !decision.is_well_formed_for(&intent) {
        return Err("constructed Astrid volition decision is not well formed".to_string());
    }
    write_owner_json(
        &root.join("intents").join(format!("{intent_id}.json")),
        &intent,
    )?;
    write_owner_json(
        &root.join("decisions").join(format!("{decision_id}.json")),
        &decision,
    )?;
    let dispatch_permit_path = dispatch_permit_path(root, requested_action);
    write_owner_json(
        &dispatch_permit_path,
        &AuthoredDispatchPermitV1 {
            schema: DISPATCH_PERMIT_SCHEMA_V1.to_string(),
            requested_action: requested_action.to_string(),
            requested_action_sha256: exact_text_sha256(requested_action),
            source_attestation_id: intent.source_attestation_id.clone(),
            intent_id: intent.intent_id.clone(),
            target_deployment_identity: intent.target_deployment_identity.clone(),
            expires_at_unix_ms: intent.command_expires_at_unix_ms,
        },
    )?;
    Ok(AstridVolitionStartV1::Accepted(Box::new(
        AcceptedVolitionV1 {
            root: root.to_path_buf(),
            intent,
            decision,
            dispatch_permit_path,
            delegated_binding_id: delegated_reservation.map(|reservation| reservation.binding_id),
        },
    )))
}

#[allow(clippy::too_many_arguments)]
fn write_shadow_start(
    root: &Path,
    exchange_id: &str,
    entropy: u64,
    attestation_id: String,
    attestation_sha256: String,
    requested_action: &str,
    authority_class: VolitionAuthorityClassV1,
    reason: &'static str,
    now: u64,
) -> Result<AstridVolitionStartV1, String> {
    let record_id = format!("astrid-shadow-{exchange_id}-{entropy:016x}");
    let shadow = ShadowAuthorityRecordV1 {
        schema: SHADOW_SCHEMA_V1,
        record_id: record_id.clone(),
        attestation_id,
        attestation_sha256,
        requested_action: requested_action.to_string(),
        proposed_authority_class: authority_class,
        reason,
        runtime_consumed: false,
        authority_granted: false,
        consent_inferred_from_silence: false,
        recorded_at_unix_ms: now,
    };
    write_owner_json(
        &root.join("shadow").join(format!("{record_id}.json")),
        &shadow,
    )?;
    Ok(AstridVolitionStartV1::Shadowed(ShadowedVolitionV1 {
        root: root.to_path_buf(),
        record_id,
        requested_action: requested_action.to_string(),
    }))
}

impl AcceptedVolitionV1 {
    fn complete(self, outcome: &NextActionOutcome, now: u64) -> Result<String, String> {
        let action_sha256 = canonical_volition_action_sha256(&self.intent.action);
        let status = if outcome.handled {
            VolitionReceiptStatusV1::Applied
        } else {
            VolitionReceiptStatusV1::Rejected
        };
        let receipt_id = format!("astrid-receipt-{}-{now}", self.intent.intent_id);
        let summary = if outcome.handled {
            format!(
                "Astrid's `{}` completed through `{}`; machine effect recorded, felt effect remains unasserted.",
                self.intent.action.name, outcome.route
            )
        } else {
            format!(
                "Astrid's `{}` was not applied: {}",
                self.intent.action.name, outcome.outcome_summary
            )
        };
        let receipt = VolitionReceiptV1 {
            schema: VOLITION_RECEIPT_SCHEMA_V1.to_string(),
            receipt_id: receipt_id.clone(),
            decision_id: self.decision.decision_id.clone(),
            intent_id: self.intent.intent_id.clone(),
            idempotency_key: self.intent.idempotency_key.clone(),
            status,
            requested_action_sha256: action_sha256.clone(),
            effective_action_sha256: action_sha256,
            requested_revision: self.intent.revision,
            resulting_revision: self.intent.revision,
            target_being: self.intent.target_being.clone(),
            target_deployment_identity: self.intent.target_deployment_identity.clone(),
            machine_effects: json!({
                "handled": outcome.handled,
                "route": outcome.route,
                "stage": outcome.stage,
                "status": outcome.status,
                "outcome_summary": outcome.outcome_summary,
                "capability_binding_ids": self.intent.capability_binding_ids,
            }),
            previous_state: json!({"captured": false, "reason": "legacy handler owns rollback state"}),
            received_at_unix_ms: self.decision.decided_at_unix_ms,
            completed_at_unix_ms: now,
            server_process_identity: format!("spectral-bridge:pid:{}", std::process::id()),
            server_deployment_identity: crate::signal_spine::signal_deployment_identity_v1(),
            machine_effect_established: outcome.handled,
            felt_effect_established: false,
            rollback_receipt_id: None,
            reason: (!outcome.handled).then(|| outcome.outcome_summary.clone()),
            perceptible_return: PerceptibleReturnV1 {
                summary: summary.clone(),
                full_receipt_ref: format!("volition:{receipt_id}"),
                delivered_at_unix_ms: now,
                included_in_next_prompt: true,
            },
        };
        if !receipt.is_well_formed_for(&self.intent, &self.decision) {
            return Err("constructed Astrid volition receipt is not well formed".to_string());
        }
        write_owner_json(
            &self
                .root
                .join("receipts")
                .join(format!("{receipt_id}.json")),
            &receipt,
        )?;
        append_owner_jsonl(&self.root.join("events.jsonl"), &receipt)?;
        match fs::remove_file(&self.dispatch_permit_path) {
            Ok(()) => {},
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {},
            Err(error) => {
                return Err(format!(
                    "remove consumed Astrid dispatch permit {}: {error}",
                    self.dispatch_permit_path.display()
                ));
            },
        }
        if let Some(binding_id) = self.delegated_binding_id.as_deref() {
            delegated_capability::complete_reservation(
                &self.root,
                binding_id,
                &self.intent.intent_id,
            )?;
        }
        Ok(summary)
    }
}

impl ShadowedVolitionV1 {
    fn complete(self, outcome: &NextActionOutcome, now: u64) -> Result<String, String> {
        let outcome_id = format!("{}-outcome-{now}", self.record_id);
        let record = ShadowOutcomeRecordV1 {
            schema: SHADOW_OUTCOME_SCHEMA_V1,
            record_id: outcome_id.clone(),
            shadow_record_id: self.record_id,
            requested_action: self.requested_action.clone(),
            legacy_route: outcome.route.clone(),
            legacy_status: outcome.status.clone(),
            legacy_outcome_summary: outcome.outcome_summary.clone(),
            volition_authority_consumed: false,
            recorded_at_unix_ms: now,
        };
        write_owner_json(
            &self.root.join("shadow").join(format!("{outcome_id}.json")),
            &record,
        )?;
        Ok(format!(
            "`{}` remained on its existing authority path; the volition record granted no new authority.",
            self.requested_action
        ))
    }
}

fn authority_class_for_next(action: &str) -> VolitionAuthorityClassV1 {
    let base = next_base(action);
    if base.starts_with("DIVISION_")
        || matches!(
            base,
            "BREATHE_TOGETHER" | "TUNE_MINIME" | "ACCEPT_PARAMETER_REQUEST" | "ACCEPT_REQUEST"
        )
    {
        VolitionAuthorityClassV1::Mutual
    } else if matches!(
        base,
        "SEARCH"
            | "BROWSE"
            | "MESSAGE_MINIME"
            | "REPLY_MINIME"
            | "TRACE_MINIME"
            | "CORRESPONDENCE_TRACE"
            | "ACK_MINIME"
            | "CORRESPONDENCE_ACK"
            | "I_RECEIVED_THIS"
            | "CORRESPONDENCE_HEARTBEAT"
            | "SIGNAL_PERSISTENCE"
            | "PING"
            | "ASK"
            | "SEND"
    ) {
        VolitionAuthorityClassV1::DelegatedExternal
    } else if matches!(
        base,
        "PERTURB"
            | "PULSE"
            | "DISPERSE"
            | "SPREAD"
            | "EXPERIMENT_RUN"
            | "RUN_EXPERIMENT"
            | "ENTER_TRANSITION"
            | "RETURN_TRANSITION"
    ) {
        VolitionAuthorityClassV1::OperatorOneShot
    } else {
        VolitionAuthorityClassV1::SelfOwnedLocal
    }
}

fn next_base(action: &str) -> &str {
    action.split_whitespace().next().unwrap_or(action)
}

fn exact_self_owned_action_context_at_root(
    root: &Path,
    requested_action: &str,
    now: u64,
) -> Result<AuthoredActionContextV1, String> {
    let path = dispatch_permit_path(root, requested_action);
    let permit = read_json::<AuthoredDispatchPermitV1>(&path)?.ok_or_else(|| {
        "no exact being-authored dispatch permit exists for this action".to_string()
    })?;
    if permit.schema != DISPATCH_PERMIT_SCHEMA_V1
        || permit.requested_action != requested_action
        || permit.requested_action_sha256 != exact_text_sha256(requested_action)
        || permit.expires_at_unix_ms < now
        || permit.target_deployment_identity != crate::signal_spine::signal_deployment_identity_v1()
    {
        return Err("being-authored dispatch permit is stale or mismatched".to_string());
    }
    let intent_path = root
        .join("intents")
        .join(format!("{}.json", permit.intent_id));
    let intent = read_json::<VolitionIntentV1>(&intent_path)?
        .ok_or_else(|| "dispatch permit intent evidence is missing".to_string())?;
    if intent.intent_id != permit.intent_id
        || intent.source_attestation_id != permit.source_attestation_id
        || intent.target_deployment_identity != permit.target_deployment_identity
        || intent
            .action
            .parameters
            .get("canonical_next")
            .and_then(Value::as_str)
            != Some(requested_action)
        || !intent.is_well_formed(now)
    {
        return Err("dispatch permit intent evidence does not match the exact action".to_string());
    }
    Ok(AuthoredActionContextV1 {
        source_attestation_id: permit.source_attestation_id,
        intent_id: permit.intent_id,
    })
}

fn dispatch_permit_path(root: &Path, requested_action: &str) -> PathBuf {
    root.join("pending")
        .join(format!("{}.json", exact_text_sha256(requested_action)))
}

fn exact_text_sha256(value: &str) -> String {
    format!("{:x}", Sha256::digest(value.as_bytes()))
}

fn load_or_provision_identity(root: &Path, now: u64) -> Result<AttestorSigner, String> {
    let path = root.join("identity.json");
    let stored = if let Some(stored) = read_json::<StoredAttestorIdentityV1>(&path)? {
        stored
    } else {
        let signing_key = SigningKey::generate(&mut OsRng);
        let stored = StoredAttestorIdentityV1 {
            schema: IDENTITY_SCHEMA_V1.to_string(),
            being: "astrid".to_string(),
            public_key_hex: hex::encode(signing_key.verifying_key().to_bytes()),
            signing_key_seed_hex: hex::encode(signing_key.to_bytes()),
            created_at_unix_ms: now,
        };
        write_owner_json(&path, &stored)?;
        stored
    };
    if stored.schema != IDENTITY_SCHEMA_V1 || stored.being != "astrid" {
        return Err("Astrid volition attestor identity schema mismatch".to_string());
    }
    let seed: [u8; 32] = hex::decode(&stored.signing_key_seed_hex)
        .map_err(|error| format!("decode Astrid volition attestor key: {error}"))?
        .try_into()
        .map_err(|_| "Astrid volition attestor key has the wrong length".to_string())?;
    let signing_key = SigningKey::from_bytes(&seed);
    let public_key_hex = hex::encode(signing_key.verifying_key().to_bytes());
    if public_key_hex != stored.public_key_hex {
        return Err("Astrid volition attestor identity integrity mismatch".to_string());
    }
    Ok(AttestorSigner {
        public_key_hex,
        signing_key,
    })
}

fn ensure_owner_tree(root: &Path) -> Result<(), String> {
    ensure_owner_dir(root)?;
    for child in [
        "attestations",
        "intents",
        "decisions",
        "receipts",
        "shadow",
        "pending",
    ] {
        ensure_owner_dir(&root.join(child))?;
    }
    Ok(())
}

pub(super) fn ensure_owner_dir(path: &Path) -> Result<(), String> {
    fs::create_dir_all(path)
        .map_err(|error| format!("create owner-only directory {}: {error}", path.display()))?;
    #[cfg(unix)]
    fs::set_permissions(path, fs::Permissions::from_mode(0o700))
        .map_err(|error| format!("set owner-only mode on {}: {error}", path.display()))?;
    Ok(())
}

pub(super) fn write_owner_json<T: Serialize>(path: &Path, value: &T) -> Result<(), String> {
    if let Some(parent) = path.parent() {
        ensure_owner_dir(parent)?;
    }
    let encoded = serde_json::to_vec_pretty(value)
        .map_err(|error| format!("encode {}: {error}", path.display()))?;
    let temporary = path.with_extension(format!(
        "tmp-{}-{:016x}",
        std::process::id(),
        rand::random::<u64>()
    ));
    let mut options = OpenOptions::new();
    options.write(true).create_new(true);
    #[cfg(unix)]
    options.mode(0o600);
    let mut file = options
        .open(&temporary)
        .map_err(|error| format!("create {}: {error}", temporary.display()))?;
    file.write_all(&encoded)
        .and_then(|()| file.sync_all())
        .map_err(|error| format!("write {}: {error}", temporary.display()))?;
    fs::rename(&temporary, path).map_err(|error| format!("replace {}: {error}", path.display()))?;
    #[cfg(unix)]
    fs::set_permissions(path, fs::Permissions::from_mode(0o600))
        .map_err(|error| format!("set owner-only mode on {}: {error}", path.display()))?;
    Ok(())
}

fn append_owner_jsonl<T: Serialize>(path: &Path, value: &T) -> Result<(), String> {
    let mut encoded =
        serde_json::to_vec(value).map_err(|error| format!("encode {}: {error}", path.display()))?;
    encoded.push(b'\n');
    let mut options = OpenOptions::new();
    options.create(true).append(true);
    #[cfg(unix)]
    options.mode(0o600);
    let mut file = options
        .open(path)
        .map_err(|error| format!("open {}: {error}", path.display()))?;
    file.write_all(&encoded)
        .and_then(|()| file.sync_data())
        .map_err(|error| format!("append {}: {error}", path.display()))?;
    #[cfg(unix)]
    fs::set_permissions(path, fs::Permissions::from_mode(0o600))
        .map_err(|error| format!("set owner-only mode on {}: {error}", path.display()))?;
    Ok(())
}

pub(super) fn read_json<T: DeserializeOwned>(path: &Path) -> Result<Option<T>, String> {
    match fs::read(path) {
        Ok(bytes) => serde_json::from_slice(&bytes)
            .map(Some)
            .map_err(|error| format!("decode {}: {error}", path.display())),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(error) => Err(format!("read {}: {error}", path.display())),
    }
}

fn default_root() -> PathBuf {
    #[cfg(test)]
    {
        std::env::temp_dir().join(format!("astrid-volition-v1-tests-{}", std::process::id()))
    }
    #[cfg(not(test))]
    {
        crate::paths::bridge_paths()
            .bridge_workspace()
            .join("volition_v1/astrid")
    }
}

pub(super) fn astrid_volition_root() -> PathBuf {
    default_root()
}

pub(super) fn now_unix_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis()
        .try_into()
        .unwrap_or(u64::MAX)
}

#[cfg(test)]
#[path = "volition_tests.rs"]
mod tests;

#[cfg(test)]
#[path = "volition_study_tests.rs"]
mod study_tests;
