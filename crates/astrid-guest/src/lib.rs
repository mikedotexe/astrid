//! Minimal Rust guest SDK for Astrid Component Model capsules.
//!
//! This crate is intentionally thin: it generates bindings from
//! `wit/astrid-capsule.wit`, re-exports the generated guest trait/export macro,
//! and provides small ergonomic helpers for the host interfaces.

#![deny(unsafe_code)]
#![deny(missing_docs)]
#![deny(clippy::all)]
#![deny(unreachable_pub)]
#![deny(clippy::unwrap_used)]
#![allow(
    clippy::missing_errors_doc,
    reason = "Thin guest helpers forward host-interface error strings without adding policy."
)]

/// Generated Component Model bindings.
#[allow(missing_docs, clippy::same_length_and_capacity)]
pub mod bindings {
    wit_bindgen::generate!({
        world: "capsule",
        path: "../../wit",
        pub_export_macro: true,
        default_bindings_module: "astrid_guest::bindings",
    });
}

/// Re-export `serde_json` so guest crates do not need their own direct
/// dependency for simple routing and payload construction.
pub use serde_json;

/// Re-export of the generated WIT capsule-result record.
pub use bindings::CapsuleResult;
/// Re-export the generated root guest trait.
pub use bindings::Guest;
/// Re-export the generated guest export macro.
pub use bindings::export;

/// Helpers for building WIT capsule results.
pub mod capsule_result {
    use serde::Serialize;

    use crate::CapsuleResult;

    /// Continue the interceptor chain without modifying the payload.
    #[must_use]
    pub fn continue_empty() -> CapsuleResult {
        CapsuleResult {
            action: "continue".to_string(),
            data: None,
        }
    }

    /// Continue the interceptor chain with a JSON payload.
    pub fn continue_json<T: Serialize>(value: &T) -> CapsuleResult {
        json_result("continue", value)
    }

    /// Short-circuit the interceptor chain with a JSON payload.
    pub fn final_json<T: Serialize>(value: &T) -> CapsuleResult {
        json_result("final", value)
    }

    /// Deny an interceptor action with a human-readable reason.
    #[must_use]
    pub fn deny(reason: impl Into<String>) -> CapsuleResult {
        CapsuleResult {
            action: "deny".to_string(),
            data: Some(reason.into()),
        }
    }

    fn json_result<T: Serialize>(action: &str, value: &T) -> CapsuleResult {
        match serde_json::to_string(value) {
            Ok(data) => CapsuleResult {
                action: action.to_string(),
                data: Some(data),
            },
            Err(err) => deny(format!("failed to serialize capsule result: {err}")),
        }
    }
}

/// Filesystem helpers.
pub mod fs {
    use crate::bindings::astrid::capsule::fs as host_fs;

    /// Read a UTF-8-ish text file, replacing invalid bytes lossily.
    pub fn read_text(path: &str) -> Result<String, String> {
        host_fs::read_file(path).map(|bytes| String::from_utf8_lossy(&bytes).into_owned())
    }

    /// Write text content to a file.
    pub fn write_text(path: &str, content: &str) -> Result<(), String> {
        host_fs::write_file(path, content.as_bytes())
    }

    /// Check whether a path exists.
    pub fn exists(path: &str) -> Result<bool, String> {
        host_fs::fs_exists(path)
    }

    /// Create a directory and parents.
    pub fn mkdir(path: &str) -> Result<(), String> {
        host_fs::fs_mkdir(path)
    }

    /// List directory entries.
    pub fn readdir(path: &str) -> Result<Vec<String>, String> {
        host_fs::fs_readdir(path)
    }

    /// Remove a file.
    pub fn unlink(path: &str) -> Result<(), String> {
        host_fs::fs_unlink(path)
    }

    /// Return true when the path is a directory.
    pub fn is_dir(path: &str) -> Result<bool, String> {
        host_fs::fs_stat(path).map(|stat| stat.is_dir)
    }
}

/// IPC helpers.
pub mod ipc {
    use serde::Serialize;

    use crate::bindings::astrid::capsule::ipc as host_ipc;

    /// Publish a JSON-serializable payload to the Astrid event bus.
    pub fn publish_json<T: Serialize>(topic: &str, payload: &T) -> Result<(), String> {
        let payload = serde_json::to_string(payload)
            .map_err(|err| format!("serialize IPC payload: {err}"))?;
        host_ipc::ipc_publish(topic, &payload)
    }

    /// Subscribe to a topic pattern.
    pub fn subscribe(topic_pattern: &str) -> Result<u64, String> {
        host_ipc::ipc_subscribe(topic_pattern)
    }

    /// Receive messages for a subscription handle.
    pub fn recv(
        handle_id: u64,
        timeout_ms: u64,
    ) -> Result<crate::bindings::astrid::capsule::types::IpcEnvelope, String> {
        host_ipc::ipc_recv(handle_id, timeout_ms)
    }
}

/// HTTP helpers.
pub mod http {
    use crate::bindings::astrid::capsule::http as host_http;
    use crate::bindings::astrid::capsule::types::{
        HttpRequestData, HttpResponseData, KeyValuePair,
    };

    /// Perform a buffered HTTP request.
    pub fn request(
        method: &str,
        url: &str,
        headers: Vec<(String, String)>,
        body: Option<String>,
    ) -> Result<HttpResponseData, String> {
        let request = HttpRequestData {
            url: url.to_string(),
            method: method.to_string(),
            headers: headers
                .into_iter()
                .map(|(key, value)| KeyValuePair { key, value })
                .collect(),
            body,
        };
        host_http::http_request(&request)
    }
}

/// Host process helpers.
pub mod process {
    use crate::bindings::astrid::capsule::process as host_process;
    use crate::bindings::astrid::capsule::types::{
        KillProcessResult, ProcessResult, ReadLogsResult, SpawnBackgroundResult, SpawnRequest,
    };

    /// Spawn a synchronous host process.
    pub fn spawn(cmd: &str, args: &[String]) -> Result<ProcessResult, String> {
        host_process::spawn(&SpawnRequest {
            cmd: cmd.to_string(),
            args: args.to_vec(),
        })
    }

    /// Spawn a background host process.
    pub fn spawn_background(cmd: &str, args: &[String]) -> Result<SpawnBackgroundResult, String> {
        host_process::spawn_background(&SpawnRequest {
            cmd: cmd.to_string(),
            args: args.to_vec(),
        })
    }

    /// Read buffered logs from a background process.
    pub fn read_logs(process_id: u64) -> Result<ReadLogsResult, String> {
        host_process::read_logs(process_id)
    }

    /// Kill a background process.
    pub fn kill(process_id: u64) -> Result<KillProcessResult, String> {
        host_process::kill(process_id)
    }
}

/// System helpers.
pub mod sys {
    use crate::bindings::astrid::capsule::sys as host_sys;
    use crate::bindings::astrid::capsule::types::LogLevel;

    /// Read a manifest/config value.
    pub fn get_config(key: &str) -> Result<String, String> {
        host_sys::get_config(key)
    }

    /// Emit an info log.
    pub fn log_info(message: &str) {
        host_sys::log(LogLevel::Info, message);
    }

    /// Emit a warning log.
    pub fn log_warn(message: &str) {
        host_sys::log(LogLevel::Warn, message);
    }

    /// Signal readiness from a long-lived run loop.
    pub fn signal_ready() {
        host_sys::signal_ready();
    }
}

/// Uplink helpers.
pub mod uplink {
    use crate::bindings::astrid::capsule::uplink as host_uplink;

    /// Register an uplink endpoint.
    pub fn register(name: &str, platform: &str, profile: &str) -> Result<String, String> {
        host_uplink::uplink_register(name, platform, profile)
    }

    /// Send inbound content through a registered uplink.
    pub fn send(uplink_id: &str, platform_user_id: &str, content: &str) -> Result<bool, String> {
        host_uplink::uplink_send(uplink_id, platform_user_id, content)
    }
}

/// Common helpers for tool-style capsules.
pub mod tool {
    use serde_json::Value;

    use crate::{CapsuleResult, capsule_result, ipc};

    /// Parsed `tool_execute_request` payload.
    #[derive(Debug, Clone)]
    pub struct ToolRequest {
        /// Tool call correlation ID.
        pub call_id: String,
        /// Tool name.
        pub tool_name: String,
        /// JSON arguments.
        pub arguments: Value,
    }

    /// Parse a tool request from guest payload bytes.
    pub fn parse_request(payload: &[u8]) -> Result<ToolRequest, String> {
        let value: Value = serde_json::from_slice(payload)
            .map_err(|err| format!("invalid JSON payload: {err}"))?;
        let call_id = value
            .get("call_id")
            .and_then(Value::as_str)
            .ok_or_else(|| "tool request missing call_id".to_string())?
            .to_string();
        let tool_name = value
            .get("tool_name")
            .and_then(Value::as_str)
            .ok_or_else(|| "tool request missing tool_name".to_string())?
            .to_string();
        let arguments = value.get("arguments").cloned().unwrap_or(Value::Null);
        Ok(ToolRequest {
            call_id,
            tool_name,
            arguments,
        })
    }

    /// Publish a successful tool result and continue the interceptor chain.
    #[must_use]
    pub fn publish_success(
        call_id: &str,
        tool_name: &str,
        content: impl Into<String>,
    ) -> CapsuleResult {
        publish_result(call_id, tool_name, content, false)
    }

    /// Publish an error tool result and continue the interceptor chain.
    #[must_use]
    pub fn publish_error(
        call_id: &str,
        tool_name: &str,
        error: impl Into<String>,
    ) -> CapsuleResult {
        publish_result(call_id, tool_name, error, true)
    }

    /// Extract a string argument.
    #[must_use]
    pub fn string_arg(args: &Value, name: &str) -> Option<String> {
        args.get(name)
            .and_then(Value::as_str)
            .map(ToString::to_string)
    }

    /// Extract a string argument or return a shape error.
    pub fn required_string_arg(args: &Value, name: &str) -> Result<String, String> {
        string_arg(args, name).ok_or_else(|| format!("missing string argument `{name}`"))
    }

    /// Extract an unsigned integer argument.
    #[must_use]
    pub fn u64_arg(args: &Value, name: &str) -> Option<u64> {
        args.get(name).and_then(Value::as_u64)
    }

    fn publish_result(
        call_id: &str,
        tool_name: &str,
        content: impl Into<String>,
        is_error: bool,
    ) -> CapsuleResult {
        let payload = serde_json::json!({
            "type": "tool_execute_result",
            "call_id": call_id,
            "result": {
                "call_id": call_id,
                "content": content.into(),
                "is_error": is_error,
            },
        });
        let topic = format!("tool.v1.execute.{tool_name}.result");
        match ipc::publish_json(&topic, &payload) {
            Ok(()) => capsule_result::continue_empty(),
            Err(err) => capsule_result::deny(format!("failed to publish tool result: {err}")),
        }
    }
}
