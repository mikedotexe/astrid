//! Operator configuration for the optional diagnostic subprocess.
//!
//! Default-off policy introduced 2026-09-23. Reports are for steward tools,
//! not supplied back to Astrid as perception. A 2026-09-07 run took about
//! 225 seconds of shared GPU time; this is historical evidence, not a timer.
//! Configuration changes require the sanctioned bridge deployment wrapper.

pub(super) const REFLECTIVE_SIDECAR_ENABLED_ENV: &str = "ASTRID_REFLECTIVE_SIDECAR_ENABLED";
const TIMEOUT_ENV: &str = "ASTRID_REFLECTIVE_SIDECAR_TIMEOUT_SECONDS";
const DEFAULT_TIMEOUT_SECONDS: u64 = 240;
const MIN_TIMEOUT_SECONDS: u64 = 30;
const MAX_TIMEOUT_SECONDS: u64 = 900;

fn enabled_from(value: Option<&str>) -> bool {
    matches!(
        value.map(|v| v.trim().to_ascii_lowercase()).as_deref(),
        Some("1" | "true" | "on" | "yes")
    )
}

pub(super) fn reflective_sidecar_enabled() -> bool {
    enabled_from(
        std::env::var(REFLECTIVE_SIDECAR_ENABLED_ENV)
            .ok()
            .as_deref(),
    )
}

fn timeout_from(raw: Option<&str>) -> u64 {
    raw.and_then(|value| value.trim().parse::<u64>().ok())
        .map_or(DEFAULT_TIMEOUT_SECONDS, |value| {
            value.clamp(MIN_TIMEOUT_SECONDS, MAX_TIMEOUT_SECONDS)
        })
}

pub(super) fn reflective_sidecar_timeout_seconds() -> u64 {
    timeout_from(std::env::var(TIMEOUT_ENV).ok().as_deref())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sidecar_switch_is_off_unless_explicitly_on() {
        for off in [
            None,
            Some(""),
            Some("0"),
            Some("false"),
            Some("off"),
            Some("enabled"),
        ] {
            assert!(!enabled_from(off), "{off:?}");
        }
        for on in ["1", " TRUE ", "on", "yes"] {
            assert!(enabled_from(Some(on)), "{on}");
        }
    }

    #[test]
    fn timeout_defaults_and_bounds_are_unchanged() {
        for value in [None, Some(""), Some("invalid"), Some("-1")] {
            assert_eq!(timeout_from(value), DEFAULT_TIMEOUT_SECONDS);
        }
        assert_eq!(timeout_from(Some("5")), MIN_TIMEOUT_SECONDS);
        assert_eq!(timeout_from(Some("1200")), MAX_TIMEOUT_SECONDS);
        assert_eq!(timeout_from(Some(" 361 ")), 361);
    }
}
