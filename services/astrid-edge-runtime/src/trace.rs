use std::{collections::VecDeque, sync::Mutex};

use serde::{Deserialize, Serialize};
use serde_json::Value;
use uuid::Uuid;

const TRACE_SCHEMA_VERSION_V1: u8 = 1;
const TRACE_LABEL_MAX_CHARS: usize = 96;
const PRODUCER_SCHEMA_VERSION_V1: u8 = 1;
const MAX_REGISTERED_AUTONOMY_TRACES: usize = 128;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AutonomyTraceMatch {
    NotRegistered,
    Registered,
    RegisteredIdentityConflict,
}

#[derive(Debug, Clone)]
#[allow(
    clippy::struct_field_names,
    reason = "registry keys mirror the canonical trace/session/chain/turn identifier contract"
)]
struct RegisteredAutonomyTrace {
    trace_id: Uuid,
    session_id: Option<String>,
    chain_id: Option<String>,
    canonical_turn_id: Option<Uuid>,
}

/// Process-local positive registry for scheduler-owned traces.
///
/// It grants no Action authority. It prevents the IPC observer from treating a
/// scheduled response as interactive before durable authorship classification,
/// and binds the first kernel-minted turn ID for replay-safe scheduler use.
#[derive(Debug, Default)]
pub struct AutonomyTraceRegistry {
    entries: Mutex<VecDeque<RegisteredAutonomyTrace>>,
}

impl AutonomyTraceRegistry {
    pub fn register(&self, trace: &IpcTraceContextV1) -> anyhow::Result<()> {
        if !trace.is_supported() {
            anyhow::bail!("cannot register unsupported autonomy trace context");
        }
        let mut entries = self
            .entries
            .lock()
            .map_err(|_| anyhow::anyhow!("autonomy trace registry lock poisoned"))?;
        if entries.iter().any(|entry| entry.trace_id == trace.trace_id) {
            anyhow::bail!("autonomy trace identifier was already registered");
        }
        entries.push_back(RegisteredAutonomyTrace {
            trace_id: trace.trace_id,
            session_id: trace.session_id.clone(),
            chain_id: trace.chain_id.clone(),
            canonical_turn_id: None,
        });
        while entries.len() > MAX_REGISTERED_AUTONOMY_TRACES {
            entries.pop_front();
        }
        Ok(())
    }

    pub fn observe_or_bind(&self, trace: &IpcTraceContextV1) -> anyhow::Result<AutonomyTraceMatch> {
        if !trace.is_supported() {
            return Ok(AutonomyTraceMatch::RegisteredIdentityConflict);
        }
        let mut entries = self
            .entries
            .lock()
            .map_err(|_| anyhow::anyhow!("autonomy trace registry lock poisoned"))?;
        let Some(entry) = entries
            .iter_mut()
            .find(|entry| entry.trace_id == trace.trace_id)
        else {
            return Ok(AutonomyTraceMatch::NotRegistered);
        };
        let Some(turn_id) = trace.turn_id else {
            return Ok(AutonomyTraceMatch::RegisteredIdentityConflict);
        };
        if entry.session_id != trace.session_id || entry.chain_id != trace.chain_id {
            return Ok(AutonomyTraceMatch::RegisteredIdentityConflict);
        }
        if let Some(canonical_turn_id) = entry.canonical_turn_id {
            if canonical_turn_id != turn_id {
                return Ok(AutonomyTraceMatch::RegisteredIdentityConflict);
            }
        } else {
            entry.canonical_turn_id = Some(turn_id);
        }
        Ok(AutonomyTraceMatch::Registered)
    }
}

/// Host-attested IPC producer metadata mirrored from `astrid-types`.
#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq)]
pub struct IpcProducerV1 {
    #[serde(default = "default_producer_schema_version")]
    pub schema_version: u8,
    pub kind: String,
    pub id: String,
}

impl IpcProducerV1 {
    pub const fn is_supported(&self) -> bool {
        self.schema_version == PRODUCER_SCHEMA_VERSION_V1
    }
}

/// Observational IPC trace metadata mirrored from `astrid-types`.
///
/// The edge sidecar intentionally does not use this data for authority,
/// validation, pacing, or Action selection.
#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq)]
pub struct IpcTraceContextV1 {
    #[serde(default = "default_schema_version")]
    pub schema_version: u8,
    pub trace_id: Uuid,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub turn_id: Option<Uuid>,
    pub span_id: Uuid,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub parent_span_id: Option<Uuid>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub session_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub chain_id: Option<String>,
}

impl IpcTraceContextV1 {
    pub fn root(trace_id: Uuid, session_id: String, chain_id: Option<String>) -> Self {
        Self {
            schema_version: TRACE_SCHEMA_VERSION_V1,
            trace_id,
            turn_id: Some(Uuid::new_v4()),
            span_id: Uuid::new_v4(),
            parent_span_id: None,
            session_id: Some(session_id),
            chain_id,
        }
    }

    pub fn child(&self) -> Self {
        Self {
            schema_version: TRACE_SCHEMA_VERSION_V1,
            trace_id: self.trace_id,
            turn_id: self.turn_id,
            span_id: Uuid::new_v4(),
            parent_span_id: Some(self.span_id),
            session_id: self.session_id.clone(),
            chain_id: self.chain_id.clone(),
        }
    }

    pub fn is_supported(&self) -> bool {
        self.schema_version == TRACE_SCHEMA_VERSION_V1
            && !self.trace_id.is_nil()
            && !self.span_id.is_nil()
            && self.turn_id.is_none_or(|turn_id| !turn_id.is_nil())
            && self.parent_span_id.is_none_or(|parent_span_id| {
                !parent_span_id.is_nil() && parent_span_id != self.span_id
            })
            && self
                .session_id
                .as_deref()
                .is_none_or(trace_label_is_supported)
            && self
                .chain_id
                .as_deref()
                .is_none_or(trace_label_is_supported)
    }
}

fn trace_label_is_supported(value: &str) -> bool {
    !value.trim().is_empty()
        && value.chars().count() <= TRACE_LABEL_MAX_CHARS
        && !value.chars().any(char::is_control)
}

pub fn message_trace(message: &Value) -> Option<IpcTraceContextV1> {
    serde_json::from_value::<IpcTraceContextV1>(message.get("trace")?.clone())
        .ok()
        .filter(IpcTraceContextV1::is_supported)
}

pub fn message_producer(message: &Value) -> Option<IpcProducerV1> {
    serde_json::from_value::<IpcProducerV1>(message.get("producer")?.clone())
        .ok()
        .filter(IpcProducerV1::is_supported)
}

const fn default_schema_version() -> u8 {
    TRACE_SCHEMA_VERSION_V1
}

const fn default_producer_schema_version() -> u8 {
    PRODUCER_SCHEMA_VERSION_V1
}

#[cfg(test)]
mod tests {
    use super::{
        AutonomyTraceMatch, AutonomyTraceRegistry, IpcTraceContextV1, message_producer,
        message_trace,
    };
    use serde_json::json;
    use uuid::Uuid;

    #[test]
    fn child_span_preserves_trace_and_session() {
        let trace_id = Uuid::new_v4();
        let root =
            IpcTraceContextV1::root(trace_id, "session".to_string(), Some("chain".to_string()));
        let child = root.child();
        assert_eq!(child.trace_id, trace_id);
        assert_eq!(child.turn_id, root.turn_id);
        assert_eq!(child.parent_span_id, Some(root.span_id));
        assert_eq!(child.session_id.as_deref(), Some("session"));
        assert_eq!(child.chain_id.as_deref(), Some("chain"));
    }

    #[test]
    fn legacy_trace_without_turn_id_remains_supported() {
        let decoded: IpcTraceContextV1 = serde_json::from_value(json!({
            "schema_version": 1,
            "trace_id": Uuid::new_v4(),
            "span_id": Uuid::new_v4(),
            "session_id": "legacy-session",
            "chain_id": "legacy-chain"
        }))
        .unwrap();
        assert_eq!(decoded.turn_id, None);
        assert!(decoded.is_supported());
    }

    #[test]
    fn trace_validation_matches_core_bounds_and_non_nil_rules() {
        let valid =
            IpcTraceContextV1::root(Uuid::new_v4(), "session".to_string(), Some("chain".into()));
        assert!(valid.is_supported());

        let mut invalid = valid.clone();
        invalid.trace_id = Uuid::nil();
        assert!(!invalid.is_supported());

        let mut invalid = valid.clone();
        invalid.span_id = Uuid::nil();
        assert!(!invalid.is_supported());

        let mut invalid = valid.clone();
        invalid.turn_id = Some(Uuid::nil());
        assert!(!invalid.is_supported());

        let mut invalid = valid.clone();
        invalid.parent_span_id = Some(invalid.span_id);
        assert!(!invalid.is_supported());

        let mut legacy_compatible = valid.clone();
        legacy_compatible.turn_id = None;
        assert!(legacy_compatible.is_supported());

        let mut invalid = valid.clone();
        invalid.session_id = Some("\n".into());
        assert!(!invalid.is_supported());

        let mut invalid = valid.clone();
        invalid.chain_id = Some("x".repeat(97));
        assert!(!invalid.is_supported());

        let mut boundary = valid;
        boundary.session_id = Some("s".repeat(96));
        boundary.chain_id = None;
        assert!(boundary.is_supported());
    }

    #[test]
    fn scheduler_registry_binds_one_kernel_turn_and_fails_closed_on_conflict() {
        let registry = AutonomyTraceRegistry::default();
        let requested = IpcTraceContextV1::root(
            Uuid::new_v4(),
            "session".to_string(),
            Some("chain".to_string()),
        );
        registry.register(&requested).unwrap();

        let mut malformed = requested.clone();
        malformed.span_id = Uuid::nil();
        assert!(registry.register(&malformed).is_err());
        assert_eq!(
            registry.observe_or_bind(&malformed).unwrap(),
            AutonomyTraceMatch::RegisteredIdentityConflict
        );

        let canonical = IpcTraceContextV1::root(
            requested.trace_id,
            "session".to_string(),
            Some("chain".to_string()),
        );
        assert_eq!(
            registry.observe_or_bind(&canonical).unwrap(),
            AutonomyTraceMatch::Registered
        );
        assert_eq!(
            registry.observe_or_bind(&canonical.child()).unwrap(),
            AutonomyTraceMatch::Registered
        );

        let conflicting = IpcTraceContextV1::root(
            requested.trace_id,
            "session".to_string(),
            Some("chain".to_string()),
        );
        assert_eq!(
            registry.observe_or_bind(&conflicting).unwrap(),
            AutonomyTraceMatch::RegisteredIdentityConflict
        );
        assert_eq!(
            registry
                .observe_or_bind(&IpcTraceContextV1::root(
                    Uuid::new_v4(),
                    "interactive".to_string(),
                    None,
                ))
                .unwrap(),
            AutonomyTraceMatch::NotRegistered
        );
    }

    #[test]
    fn malformed_or_unsupported_message_trace_is_absent() {
        assert!(message_trace(&json!({"trace": {"trace_id": "bad"}})).is_none());
        let trace = IpcTraceContextV1 {
            schema_version: 2,
            trace_id: Uuid::new_v4(),
            turn_id: None,
            span_id: Uuid::new_v4(),
            parent_span_id: None,
            session_id: None,
            chain_id: None,
        };
        assert!(message_trace(&json!({"trace": trace})).is_none());
    }

    #[test]
    fn only_supported_host_producer_metadata_is_decoded() {
        let producer = message_producer(&json!({
            "producer": {
                "schema_version": 1,
                "kind": "wasm_capsule",
                "id": "astrid-capsule-react"
            }
        }))
        .unwrap();
        assert_eq!(producer.kind, "wasm_capsule");
        assert_eq!(producer.id, "astrid-capsule-react");
        assert!(
            message_producer(&json!({
                "producer": {"schema_version": 9, "kind": "x", "id": "y"}
            }))
            .is_none()
        );
    }
}
