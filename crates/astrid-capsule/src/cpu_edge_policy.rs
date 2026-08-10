//! Immutable process-authority policy selected by the CPU-edge system unit.
//!
//! Ordinary Astrid installations retain their existing capsule process
//! surface. The root-owned CPU-edge unit sets the policy to `disabled`; an
//! invalid present value also fails closed. The decision is captured once so
//! a capsule cannot influence it through per-call state.

use std::sync::OnceLock;

const PROCESS_AUTHORITY_ENV: &str = "ASTRID_CPU_EDGE_PROCESS_AUTHORITY";
const PROCESS_AUTHORITY_DENIED: &str =
    "CPU-edge immutable policy denies capsule process and stdio MCP authority";

static PROCESS_AUTHORITY_DISABLED: OnceLock<bool> = OnceLock::new();

pub(crate) fn process_authority_disabled() -> bool {
    *PROCESS_AUTHORITY_DISABLED.get_or_init(|| match std::env::var(PROCESS_AUTHORITY_ENV) {
        Ok(value) => process_authority_disabled_for_value(Some(value.as_str())),
        Err(std::env::VarError::NotPresent) => false,
        Err(std::env::VarError::NotUnicode(_)) => true,
    })
}

pub(crate) const fn process_authority_denied_message() -> &'static str {
    PROCESS_AUTHORITY_DENIED
}

fn process_authority_disabled_for_value(value: Option<&str>) -> bool {
    match value {
        None | Some("enabled") => false,
        Some(_) => true,
    }
}

#[cfg(test)]
mod tests {
    use super::process_authority_disabled_for_value;

    #[test]
    fn absent_or_explicit_enabled_policy_preserves_non_edge_behavior() {
        assert!(!process_authority_disabled_for_value(None));
        assert!(!process_authority_disabled_for_value(Some("enabled")));
    }

    #[test]
    fn disabled_or_malformed_present_policy_fails_closed() {
        for value in ["disabled", "", "true", "DISABLED", " enabled "] {
            assert!(
                process_authority_disabled_for_value(Some(value)),
                "policy unexpectedly enabled for {value:?}"
            );
        }
    }
}
