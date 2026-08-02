use serde::{Deserialize, Serialize};
use serde_json::Value;
use uuid::Uuid;

const TRACE_SCHEMA_VERSION_V1: u8 = 1;

/// Observational IPC trace metadata mirrored from `astrid-types`.
///
/// The edge sidecar intentionally does not use this data for authority,
/// validation, pacing, or Action selection.
#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq)]
pub struct IpcTraceContextV1 {
    #[serde(default = "default_schema_version")]
    pub schema_version: u8,
    pub trace_id: Uuid,
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
            span_id: Uuid::new_v4(),
            parent_span_id: Some(self.span_id),
            session_id: self.session_id.clone(),
            chain_id: self.chain_id.clone(),
        }
    }

    pub const fn is_supported(&self) -> bool {
        self.schema_version == TRACE_SCHEMA_VERSION_V1
    }
}

pub fn message_trace(message: &Value) -> Option<IpcTraceContextV1> {
    serde_json::from_value::<IpcTraceContextV1>(message.get("trace")?.clone())
        .ok()
        .filter(IpcTraceContextV1::is_supported)
}

const fn default_schema_version() -> u8 {
    TRACE_SCHEMA_VERSION_V1
}

#[cfg(test)]
mod tests {
    use super::{IpcTraceContextV1, message_trace};
    use serde_json::json;
    use uuid::Uuid;

    #[test]
    fn child_span_preserves_trace_and_session() {
        let trace_id = Uuid::new_v4();
        let root =
            IpcTraceContextV1::root(trace_id, "session".to_string(), Some("chain".to_string()));
        let child = root.child();
        assert_eq!(child.trace_id, trace_id);
        assert_eq!(child.parent_span_id, Some(root.span_id));
        assert_eq!(child.session_id.as_deref(), Some("session"));
        assert_eq!(child.chain_id.as_deref(), Some("chain"));
    }

    #[test]
    fn malformed_or_unsupported_message_trace_is_absent() {
        assert!(message_trace(&json!({"trace": {"trace_id": "bad"}})).is_none());
        let trace = IpcTraceContextV1 {
            schema_version: 2,
            trace_id: Uuid::new_v4(),
            span_id: Uuid::new_v4(),
            parent_span_id: None,
            session_id: None,
            chain_id: None,
        };
        assert!(message_trace(&json!({"trace": trace})).is_none());
    }
}
