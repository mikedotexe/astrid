use std::collections::BTreeMap;
use std::path::Path;

use ed25519_dalek::{Signer as _, SigningKey};
use serde::Serialize;
use serde_json::Value;
use sha2::{Digest, Sha256};

use crate::util::{
    canonical_json, read_stable_regular, sha256, validate_hex64, validate_identifier,
};
use crate::{Error, Result};

const SCHEDULED_AUTHORSHIP_KEY_DOMAIN: &[u8] = b"astrid.edge.scheduled_authorship.ed25519.seed.v1";

pub const CANDIDATE_SCHEMA: &str = "astrid.edge_self_change.candidate.v1";
pub const INTENT_SCHEMA: &str = "astrid.edge_self_change.scheduled_model_intent.v1";
pub const ENVELOPE_SCHEMA: &str = "astrid.edge_self_change.intent_attestor_envelope.v1";
pub const COMPLETED_ENVELOPE_SCHEMA: &str = "astrid.edge_self_change.completed_intent_envelope.v1";
const COMPLETION_ENVELOPE_SCHEMA: &str =
    "astrid.edge.steward_helper.authored_completion_envelope.v2";
const COMPLETION_SCHEMA: &str = "astrid.edge.steward_helper.authored_completion.v2";

#[derive(Debug, Clone)]
pub struct HmacSigner {
    key: [u8; 32],
    pub key_id: String,
}

impl HmacSigner {
    pub fn from_file(path: &Path) -> Result<Self> {
        let bytes = read_stable_regular(path, 32)?;
        let key: [u8; 32] = bytes
            .try_into()
            .map_err(|_| Error::new("HMAC key must contain exactly 32 bytes"))?;
        let key_id = format!("hmac-sha256:{}", &sha256(&key)[..16]);
        Ok(Self { key, key_id })
    }

    #[must_use]
    pub fn sign(&self, message: &[u8]) -> String {
        hmac_sha256(&self.key, message)
    }

    #[must_use]
    pub fn verify(&self, message: &[u8], expected: &str) -> bool {
        constant_time_equal(self.sign(message).as_bytes(), expected.as_bytes())
    }

    /// Return the public half of the domain-separated scheduled-authorship key.
    ///
    /// The mutable runtime receives only these bytes.  The existing intent key
    /// remains private to the immutable steward, and key separation prevents a
    /// signature or public-key disclosure from weakening intent HMACs.
    #[must_use]
    pub fn scheduled_authorship_verifying_key(&self) -> [u8; 32] {
        self.scheduled_authorship_signing_key()
            .verifying_key()
            .to_bytes()
    }

    /// Lowercase raw-key encoding used by the root bootstrap transaction.
    #[must_use]
    pub fn scheduled_authorship_verifying_key_hex(&self) -> String {
        lower_hex(&self.scheduled_authorship_verifying_key())
    }

    /// Sign one canonical scheduled-authorship envelope.
    #[must_use]
    pub fn sign_scheduled_authorship(&self, message: &[u8]) -> String {
        lower_hex(
            &self
                .scheduled_authorship_signing_key()
                .sign(message)
                .to_bytes(),
        )
    }

    /// Stable public identifier for scheduled-authorship attestations.
    #[must_use]
    pub fn scheduled_authorship_key_id(&self) -> String {
        format!(
            "ed25519:{}",
            &sha256(&self.scheduled_authorship_verifying_key())[..16]
        )
    }

    fn scheduled_authorship_signing_key(&self) -> SigningKey {
        let mut digest = Sha256::new();
        digest.update(SCHEDULED_AUTHORSHIP_KEY_DOMAIN);
        digest.update(self.key);
        let seed: [u8; 32] = digest.finalize().into();
        SigningKey::from_bytes(&seed)
    }
}

fn lower_hex(bytes: &[u8]) -> String {
    use std::fmt::Write as _;

    bytes.iter().fold(
        String::with_capacity(bytes.len().saturating_mul(2)),
        |mut encoded, byte| {
            write!(&mut encoded, "{byte:02x}").expect("writing to String cannot fail");
            encoded
        },
    )
}

#[must_use]
pub fn hmac_sha256(key: &[u8], message: &[u8]) -> String {
    const BLOCK_BYTES: usize = 64;
    let mut normalized = [0_u8; BLOCK_BYTES];
    if key.len() > BLOCK_BYTES {
        normalized[..32].copy_from_slice(&Sha256::digest(key));
    } else {
        normalized[..key.len()].copy_from_slice(key);
    }
    let mut inner_pad = [0x36_u8; BLOCK_BYTES];
    let mut outer_pad = [0x5c_u8; BLOCK_BYTES];
    for index in 0..BLOCK_BYTES {
        inner_pad[index] ^= normalized[index];
        outer_pad[index] ^= normalized[index];
    }
    let mut inner = Sha256::new();
    inner.update(inner_pad);
    inner.update(message);
    let inner_digest = inner.finalize();
    let mut outer = Sha256::new();
    outer.update(outer_pad);
    outer.update(inner_digest);
    format!("{:x}", outer.finalize())
}

fn constant_time_equal(left: &[u8], right: &[u8]) -> bool {
    if left.len() != right.len() {
        return false;
    }
    left.iter()
        .zip(right)
        .fold(0_u8, |difference, (left, right)| {
            difference | (left ^ right)
        })
        == 0
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SupervisorCandidate {
    pub schema: String,
    pub candidate_id: String,
    pub base_generation: String,
    pub proposal_sha256: String,
    pub patch_sha256: String,
    pub changed_paths: Vec<String>,
    pub created_at: u64,
    pub privilege_envelope: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct SupervisorIntent {
    pub schema: &'static str,
    pub intent_id: String,
    pub appliance_id: String,
    pub trace_id: String,
    pub session_id: String,
    pub turn_id: String,
    pub response_sha256: String,
    pub terminal_declaration_sha256: String,
    pub candidate_id: String,
    pub candidate_sha256: String,
    pub base_generation: String,
    pub current_generation: String,
    pub observed_at: u64,
    pub origin: &'static str,
    pub authorship_status: &'static str,
    pub transport_status: &'static str,
    pub declaration_provenance: &'static str,
    pub fallback: bool,
    pub executor_repair: bool,
    pub operator_harness: bool,
}

pub fn envelope(
    signer: &HmacSigner,
    envelope_id: String,
    created_at: u64,
    candidate: &SupervisorCandidate,
    intent: &SupervisorIntent,
) -> Result<Value> {
    validate_identifier(&envelope_id, "envelope_id")?;
    validate_intent(candidate, intent)?;
    let candidate_value = serde_json::to_value(candidate)?;
    let intent_value = serde_json::to_value(intent)?;
    let candidate_sha256 = sha256(&canonical_json(&candidate_value)?);
    if candidate_sha256 != intent.candidate_sha256 {
        return Err(Error::new("intent does not bind canonical candidate"));
    }
    let mut core = BTreeMap::new();
    core.insert("candidate", candidate_value);
    core.insert("candidate_sha256", Value::String(candidate_sha256));
    core.insert("created_at", Value::from(created_at));
    core.insert("envelope_id", Value::String(envelope_id));
    core.insert("intent", intent_value);
    let core = serde_json::to_value(core)?;
    let unsigned_envelope = serde_json::json!({"schema": ENVELOPE_SCHEMA, "core": core});
    let signature = signer.sign(&canonical_json(&unsigned_envelope)?);
    Ok(serde_json::json!({
        "schema": ENVELOPE_SCHEMA,
        "core": unsigned_envelope["core"].clone(),
        "auth": {
            "algorithm": "hmac-sha256",
            "key_id": signer.key_id,
            "signature": signature
        }
    }))
}

/// Bind an existing exact intent envelope to the already-durable signed authored-completion
/// proof. The wrapper is the only form that may be placed in the immutable supervisor inbox.
pub fn completed_envelope(
    signer: &HmacSigner,
    intent_envelope: &Value,
    authored_completion: &Value,
) -> Result<Value> {
    validate_completed_binding(signer, intent_envelope, authored_completion)?;
    let unsigned = serde_json::json!({
        "schema": COMPLETED_ENVELOPE_SCHEMA,
        "intent_envelope": intent_envelope,
        "authored_completion": authored_completion,
    });
    let signature = signer.sign(&canonical_json(&unsigned)?);
    Ok(serde_json::json!({
        "schema": COMPLETED_ENVELOPE_SCHEMA,
        "intent_envelope": intent_envelope,
        "authored_completion": authored_completion,
        "auth": {
            "algorithm": "hmac-sha256",
            "key_id": signer.key_id,
            "signature": signature,
        },
    }))
}

struct NestedIntent<'a> {
    envelope: &'a Value,
    core: &'a serde_json::Map<String, Value>,
    candidate: &'a serde_json::Map<String, Value>,
    intent: &'a serde_json::Map<String, Value>,
}

struct CompletionProof<'a> {
    core: &'a serde_json::Map<String, Value>,
    publication: &'a serde_json::Map<String, Value>,
}

fn validate_nested_intent<'a>(
    signer: &HmacSigner,
    intent_envelope: &'a Value,
) -> Result<NestedIntent<'a>> {
    let intent_top = exact_object(
        intent_envelope,
        &["auth", "core", "schema"],
        "intent envelope",
    )?;
    if string_field(intent_top, "schema", "intent envelope")? != ENVELOPE_SCHEMA {
        return Err(Error::new(
            "completed wrapper has wrong nested intent schema",
        ));
    }
    let core = exact_object(
        intent_top
            .get("core")
            .ok_or_else(|| Error::new("intent envelope core is absent"))?,
        &[
            "candidate",
            "candidate_sha256",
            "created_at",
            "envelope_id",
            "intent",
        ],
        "intent envelope core",
    )?;
    let candidate = core
        .get("candidate")
        .and_then(Value::as_object)
        .ok_or_else(|| Error::new("completed wrapper candidate is malformed"))?;
    let intent = core
        .get("intent")
        .and_then(Value::as_object)
        .ok_or_else(|| Error::new("completed wrapper intent is malformed"))?;
    let intent_auth = exact_object(
        intent_top
            .get("auth")
            .ok_or_else(|| Error::new("intent envelope auth is absent"))?,
        &["algorithm", "key_id", "signature"],
        "intent envelope auth",
    )?;
    let unsigned_intent = serde_json::json!({
        "schema": ENVELOPE_SCHEMA,
        "core": core,
    });
    if string_field(intent_auth, "algorithm", "intent envelope auth")? != "hmac-sha256"
        || string_field(intent_auth, "key_id", "intent envelope auth")? != signer.key_id
        || !signer.verify(
            &canonical_json(&unsigned_intent)?,
            string_field(intent_auth, "signature", "intent envelope auth")?,
        )
    {
        return Err(Error::new(
            "completed wrapper nested intent authentication failed",
        ));
    }
    Ok(NestedIntent {
        envelope: intent_envelope,
        core,
        candidate,
        intent,
    })
}

fn validate_completion_proof<'a>(
    signer: &HmacSigner,
    completion: &'a Value,
) -> Result<CompletionProof<'a>> {
    let completion_top = exact_object(
        completion,
        &["auth", "core", "core_sha256", "schema"],
        "authored completion",
    )?;
    if string_field(completion_top, "schema", "authored completion")? != COMPLETION_ENVELOPE_SCHEMA
    {
        return Err(Error::new("completed wrapper has wrong completion schema"));
    }
    let completion_core_value = completion_top
        .get("core")
        .ok_or_else(|| Error::new("authored completion core is absent"))?;
    let core = exact_completion_core(completion_core_value)?;
    validate_completion_core(completion_top, core, completion_core_value)?;
    validate_completion_auth(signer, completion_top, completion_core_value)?;
    let publication = exact_candidate_publication(core)?;
    Ok(CompletionProof { core, publication })
}

fn exact_completion_core(completion_core_value: &Value) -> Result<&serde_json::Map<String, Value>> {
    let base_fields = [
        "appliance_id",
        "candidate_publication",
        "completed_at_unix_ms",
        "due_nonce",
        "provenance",
        "response_sha256",
        "schema",
        "session_id",
        "status",
        "trace_id",
        "transaction_sha256",
        "turn_id",
    ];
    let extended_fields = [
        "appliance_id",
        "candidate_publication",
        "completed_at_unix_ms",
        "due_nonce",
        "provenance",
        "response_sha256",
        "schema",
        "session_id",
        "source_review_response_sha256",
        "source_review_status",
        "source_review_turn_id",
        "status",
        "trace_id",
        "transaction_sha256",
        "turn_id",
    ];
    let has_source_review_fields = [
        "source_review_response_sha256",
        "source_review_status",
        "source_review_turn_id",
    ]
    .iter()
    .all(|field| completion_core_value.get(field).is_some());
    let has_partial_source_review_fields = [
        "source_review_response_sha256",
        "source_review_status",
        "source_review_turn_id",
    ]
    .iter()
    .any(|field| completion_core_value.get(field).is_some());
    if has_partial_source_review_fields && !has_source_review_fields {
        return Err(Error::new(
            "authored completion has partial source-review binding fields",
        ));
    }
    exact_object(
        completion_core_value,
        if has_source_review_fields {
            &extended_fields
        } else {
            &base_fields
        },
        "authored completion core",
    )
}

fn validate_completion_core(
    completion_top: &serde_json::Map<String, Value>,
    core: &serde_json::Map<String, Value>,
    completion_core_value: &Value,
) -> Result<()> {
    let has_source_review_fields = core.contains_key("source_review_status");
    let completion_core_sha256 = sha256(&canonical_json(completion_core_value)?);
    if string_field(core, "schema", "completion core")? != COMPLETION_SCHEMA
        || string_field(core, "status", "completion core")? != "authored_completed"
        || string_field(core, "provenance", "completion core")?
            != "model_authored_runtime_scheduled"
        || core
            .get("completed_at_unix_ms")
            .and_then(Value::as_u64)
            .is_none_or(|value| value == 0)
        || completion_top.get("core_sha256").and_then(Value::as_str)
            != Some(completion_core_sha256.as_str())
    {
        return Err(Error::new("authored completion proof fields are not exact"));
    }
    if !has_source_review_fields
        || string_field(core, "source_review_status", "completion core")? != "candidate_attested"
    {
        return Err(Error::new(
            "candidate completion lacks an attested clean source-review binding",
        ));
    }
    Ok(())
}

fn validate_completion_auth(
    signer: &HmacSigner,
    completion_top: &serde_json::Map<String, Value>,
    completion_core_value: &Value,
) -> Result<()> {
    let completion_auth = exact_object(
        completion_top
            .get("auth")
            .ok_or_else(|| Error::new("authored completion auth is absent"))?,
        &["algorithm", "key_id", "signature"],
        "authored completion auth",
    )?;
    if string_field(completion_auth, "algorithm", "completion auth")? != "hmac-sha256"
        || string_field(completion_auth, "key_id", "completion auth")? != signer.key_id
        || !signer.verify(
            &canonical_json(completion_core_value)?,
            string_field(completion_auth, "signature", "completion auth")?,
        )
    {
        return Err(Error::new("authored completion auth is malformed"));
    }
    Ok(())
}

fn exact_candidate_publication(
    core: &serde_json::Map<String, Value>,
) -> Result<&serde_json::Map<String, Value>> {
    exact_object(
        core.get("candidate_publication")
            .ok_or_else(|| Error::new("candidate completion binding is absent"))?,
        &[
            "base_generation",
            "candidate_id",
            "candidate_sha256",
            "intent_envelope_id",
            "intent_envelope_sha256",
            "intent_id",
            "terminal_declaration_sha256",
        ],
        "candidate completion binding",
    )
}

fn validate_completed_binding(
    signer: &HmacSigner,
    intent_envelope: &Value,
    completion: &Value,
) -> Result<()> {
    let nested = validate_nested_intent(signer, intent_envelope)?;
    let completion = validate_completion_proof(signer, completion)?;
    let nested_sha256 = sha256(&canonical_json(nested.envelope)?);
    validate_authorship_joins(&completion, &nested)?;
    validate_envelope_joins(&completion, &nested, &nested_sha256)?;
    validate_candidate_joins(&completion, &nested)?;
    Ok(())
}

fn validate_authorship_joins(
    completion: &CompletionProof<'_>,
    nested: &NestedIntent<'_>,
) -> Result<()> {
    for (completion_field, intent_field) in [
        ("appliance_id", "appliance_id"),
        ("trace_id", "trace_id"),
        ("session_id", "session_id"),
        ("source_review_turn_id", "turn_id"),
        ("source_review_response_sha256", "response_sha256"),
    ] {
        require_same(
            string_field(completion.core, completion_field, "completion core")?,
            string_field(nested.intent, intent_field, "intent")?,
        )?;
    }
    Ok(())
}

fn validate_envelope_joins(
    completion: &CompletionProof<'_>,
    nested: &NestedIntent<'_>,
    nested_sha256: &str,
) -> Result<()> {
    require_same(
        string_field(
            completion.publication,
            "intent_envelope_id",
            "completion binding",
        )?,
        string_field(nested.core, "envelope_id", "intent envelope core")?,
    )?;
    require_same(
        string_field(
            completion.publication,
            "intent_envelope_sha256",
            "completion binding",
        )?,
        nested_sha256,
    )?;
    for field in ["intent_id", "terminal_declaration_sha256"] {
        require_same(
            string_field(completion.publication, field, "completion binding")?,
            string_field(nested.intent, field, "intent")?,
        )?;
    }
    Ok(())
}

fn validate_candidate_joins(
    completion: &CompletionProof<'_>,
    nested: &NestedIntent<'_>,
) -> Result<()> {
    let publication = completion.publication;
    for (field, object, label) in [
        ("candidate_id", nested.candidate, "candidate"),
        ("candidate_id", nested.intent, "intent"),
        ("candidate_sha256", nested.core, "intent envelope core"),
        ("candidate_sha256", nested.intent, "intent"),
        ("base_generation", nested.candidate, "candidate"),
        ("base_generation", nested.intent, "intent"),
    ] {
        require_same(
            string_field(publication, field, "completion binding")?,
            string_field(object, field, label)?,
        )?;
    }
    Ok(())
}

fn require_same(left: &str, right: &str) -> Result<()> {
    if left != right {
        return Err(Error::new(
            "authored completion proof does not match nested intent",
        ));
    }
    Ok(())
}

fn exact_object<'a>(
    value: &'a Value,
    expected_keys: &[&str],
    label: &str,
) -> Result<&'a serde_json::Map<String, Value>> {
    let object = value
        .as_object()
        .ok_or_else(|| Error::new(format!("{label} is not an object")))?;
    if object.len() != expected_keys.len()
        || expected_keys.iter().any(|key| !object.contains_key(*key))
    {
        return Err(Error::new(format!("{label} fields are not exact")));
    }
    Ok(object)
}

fn string_field<'a>(
    object: &'a serde_json::Map<String, Value>,
    field: &str,
    label: &str,
) -> Result<&'a str> {
    object
        .get(field)
        .and_then(Value::as_str)
        .ok_or_else(|| Error::new(format!("{label}.{field} is not a string")))
}

fn validate_intent(candidate: &SupervisorCandidate, intent: &SupervisorIntent) -> Result<()> {
    for (value, label) in [
        (&candidate.candidate_id, "candidate_id"),
        (&candidate.base_generation, "base_generation"),
        (&intent.intent_id, "intent_id"),
        (&intent.appliance_id, "appliance_id"),
        (&intent.trace_id, "trace_id"),
        (&intent.session_id, "session_id"),
        (&intent.turn_id, "turn_id"),
    ] {
        validate_identifier(value, label)?;
    }
    for (value, label) in [
        (&candidate.proposal_sha256, "proposal_sha256"),
        (&candidate.patch_sha256, "patch_sha256"),
        (&intent.response_sha256, "response_sha256"),
        (
            &intent.terminal_declaration_sha256,
            "terminal_declaration_sha256",
        ),
        (&intent.candidate_sha256, "candidate_sha256"),
    ] {
        validate_hex64(value, label)?;
    }
    if candidate.schema != CANDIDATE_SCHEMA
        || candidate.privilege_envelope != "proposal-only:no-execution:v1"
        || intent.candidate_id != candidate.candidate_id
        || intent.base_generation != candidate.base_generation
        || intent.current_generation != candidate.base_generation
        || intent.origin != "scheduled_autonomy"
        || intent.authorship_status != "genuinely_authored"
        || intent.transport_status != "authored_completed"
        || intent.declaration_provenance != "exact_terminal_model_declaration"
        || intent.fallback
        || intent.executor_repair
        || intent.operator_harness
    {
        return Err(Error::new("intent authority fields are not exact"));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use std::fs;

    use ed25519_dalek::{Signature, Verifier as _, VerifyingKey};

    use super::{
        CANDIDATE_SCHEMA, HmacSigner, INTENT_SCHEMA, SupervisorCandidate, SupervisorIntent,
        envelope, hmac_sha256,
    };
    use crate::util::canonical_json;

    #[test]
    fn hmac_matches_rfc_4231_case_one() {
        assert_eq!(
            hmac_sha256(&[0x0b; 20], b"Hi There"),
            "b0344c61d8db38535ca8afceaf0bf12b881dc200c9833da726e9376c2e32cff7"
        );
    }

    #[test]
    fn scheduled_authorship_uses_a_deterministic_public_only_verifier() {
        let temporary = tempfile::tempdir().unwrap();
        let key = temporary.path().join("attestor.key");
        fs::write(&key, [b's'; 32]).unwrap();
        let signer = HmacSigner::from_file(&key).unwrap();
        let verifying =
            VerifyingKey::from_bytes(&signer.scheduled_authorship_verifying_key()).unwrap();
        let encoded = signer.sign_scheduled_authorship(b"scheduled-authorship");
        let mut signature_bytes = [0_u8; 64];
        for (index, pair) in encoded.as_bytes().chunks_exact(2).enumerate() {
            signature_bytes[index] = u8::from_str_radix(std::str::from_utf8(pair).unwrap(), 16)
                .expect("lowercase signature hex");
        }
        let signature = Signature::from_bytes(&signature_bytes);
        verifying
            .verify(b"scheduled-authorship", &signature)
            .expect("exact immutable-steward signature");
        assert!(verifying.verify(b"forged", &signature).is_err());
        assert!(signer.scheduled_authorship_key_id().starts_with("ed25519:"));
    }

    #[test]
    fn any_envelope_forgery_invalidates_attestor_signature() {
        let temporary = tempfile::tempdir().unwrap();
        let key = temporary.path().join("attestor.key");
        fs::write(&key, [b'k'; 32]).unwrap();
        let signer = HmacSigner::from_file(&key).unwrap();
        let candidate = SupervisorCandidate {
            schema: CANDIDATE_SCHEMA.to_owned(),
            candidate_id: "candidate-a".to_owned(),
            base_generation: "generation-a".to_owned(),
            proposal_sha256: "a".repeat(64),
            patch_sha256: "b".repeat(64),
            changed_paths: vec!["services/edge/src/lib.rs".to_owned()],
            created_at: 1,
            privilege_envelope: "proposal-only:no-execution:v1".to_owned(),
        };
        let candidate_sha256 = crate::util::sha256(&canonical_json(&candidate).unwrap());
        let intent = SupervisorIntent {
            schema: INTENT_SCHEMA,
            intent_id: "intent-a".to_owned(),
            appliance_id: "avado".to_owned(),
            trace_id: "11111111-1111-4111-8111-111111111111".to_owned(),
            session_id: "session-a".to_owned(),
            turn_id: "22222222-2222-4222-8222-222222222222".to_owned(),
            response_sha256: "c".repeat(64),
            terminal_declaration_sha256: "d".repeat(64),
            candidate_id: candidate.candidate_id.clone(),
            candidate_sha256,
            base_generation: candidate.base_generation.clone(),
            current_generation: candidate.base_generation.clone(),
            observed_at: 2,
            origin: "scheduled_autonomy",
            authorship_status: "genuinely_authored",
            transport_status: "authored_completed",
            declaration_provenance: "exact_terminal_model_declaration",
            fallback: false,
            executor_repair: false,
            operator_harness: false,
        };
        let mut value = envelope(&signer, "envelope-a".to_owned(), 2, &candidate, &intent).unwrap();
        let signature = value["auth"]["signature"].as_str().unwrap().to_owned();
        value["core"]["intent"]["response_sha256"] = serde_json::Value::String("e".repeat(64));
        let forged_unsigned =
            serde_json::json!({"schema": super::ENVELOPE_SCHEMA, "core": value["core"].clone()});
        assert!(!signer.verify(&canonical_json(&forged_unsigned).unwrap(), &signature));
    }
}
