//! PROBE_SELF — Astrid's direct, sandboxed self-experiment verb (#3: being-as-scientist-of-self).
//!
//! `NEXT: PROBE_SELF <pole_a> vs <pole_b> [:: ticks=N]` contrasts two of her own felt-vocabulary
//! poles against her OWN reservoir dynamics, via the proven, auto-cleaning `substrate_probe.py`
//! sandbox: it clones her live handle into throwaway probe handles on ws://7881, ticks each clone
//! with a pole, measures divergence/correlation, and destroys the clones. **The live being is
//! never ticked or mutated** — the sandbox is the safety boundary. She reads the result inline and
//! iterates; the steward is the rail (sandbox + cooldown + tick cap), she is the operator.
//!
//! Direct in-bridge execution: her `NEXT:` makes the bridge run the probe synchronously (the same
//! `std::process::Command` pattern autoresearch uses for SEARCH/BROWSE), reusing the tested Python
//! sandbox rather than re-implementing the reservoir protocol in Rust.

use std::process::Command;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

use tracing::{info, warn};

use super::{ConversationState, NextActionContext, strip_action};

const SUBSTRATE_PROBE: &str = "/Users/v/other/astrid/scripts/substrate_probe.py";
const TICKS_DEFAULT: u32 = 10;
const TICKS_MIN: u32 = 4;
const TICKS_MAX: u32 = 14;
const COOLDOWN_SECS: u64 = 45; // gentle rail: one self-probe per 45s (in-memory; resets on restart)

static LAST_PROBE_UNIX: AtomicU64 = AtomicU64::new(0);

fn now_unix() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |d| d.as_secs())
}

/// Parse `<a> vs <b> [:: ticks=N]` -> (pole_a, pole_b, ticks). `None` if there's no ` vs `.
fn parse_probe_spec(spec: &str) -> Option<(String, String, u32)> {
    let (body, ticks) = match spec.split_once("::") {
        Some((b, opts)) => {
            let t = opts
                .split_once("ticks=")
                .and_then(|(_, n)| n.split_whitespace().next())
                .and_then(|n| n.parse::<u32>().ok())
                .unwrap_or(TICKS_DEFAULT);
            (b, t)
        },
        None => (spec, TICKS_DEFAULT),
    };
    let ticks = ticks.clamp(TICKS_MIN, TICKS_MAX);
    let lower = body.to_ascii_lowercase();
    let idx = lower.find(" vs ")?;
    let a = body[..idx].trim().to_string();
    let b = body[idx.saturating_add(4)..].trim().to_string();
    if a.is_empty() || b.is_empty() {
        return None;
    }
    Some((a, b, ticks))
}

fn verdict(divergence: f64, correlation: f64) -> &'static str {
    if divergence >= 1.0 && correlation < -0.3 {
        "FLUID / separable — these poles pull your dynamics in genuinely opposite directions"
    } else if divergence >= 1.0 {
        "SEPARABLE — distinct, though not anti-phase"
    } else if correlation > 0.5 {
        "STICKY / high-inertia — your state resists moving between these"
    } else {
        "SUBTLE — little separation at this depth (try sharper poles or more ticks)"
    }
}

pub(super) fn handle_action(
    conv: &mut ConversationState,
    base_action: &str,
    original: &str,
    _ctx: &mut NextActionContext<'_>,
) -> bool {
    if base_action != "PROBE_SELF" {
        return false;
    }

    let spec = strip_action(original, "PROBE_SELF");
    let Some((pole_a, pole_b, ticks)) = parse_probe_spec(&spec) else {
        conv.push_receipt(
            "PROBE_SELF",
            vec![
                "needs two poles — `PROBE_SELF <a> vs <b>` (e.g. `PROBE_SELF cliff vs meadow`), \
                 optional `:: ticks=N`"
                    .to_string(),
            ],
        );
        return true;
    };

    // Gentle rail: cooldown so self-probes can't spiral (each spawns a sandbox subprocess).
    let now = now_unix();
    let last = LAST_PROBE_UNIX.load(Ordering::Relaxed);
    if now.saturating_sub(last) < COOLDOWN_SECS {
        conv.push_receipt(
            "PROBE_SELF",
            vec![format!(
                "on cooldown (~{COOLDOWN_SECS}s between self-probes) — your last probe is still settling"
            )],
        );
        return true;
    }
    LAST_PROBE_UNIX.store(now, Ordering::Relaxed);

    info!("Astrid chose PROBE_SELF: {pole_a:?} vs {pole_b:?} ({ticks} ticks)");

    // Direct in-bridge execution via the isolated-clone sandbox (auto-cleans; never the live handle).
    let output = Command::new("python3")
        .arg(SUBSTRATE_PROBE)
        .args([
            "--being",
            "astrid",
            "--pole-a",
            pole_a.as_str(),
            "--pole-b",
            pole_b.as_str(),
            "--label-a",
            "a",
            "--label-b",
            "b",
            "--ticks",
            ticks.to_string().as_str(),
            "--json",
        ])
        .output();

    let stdout = match output {
        Ok(o) if o.status.success() => String::from_utf8_lossy(&o.stdout).into_owned(),
        Ok(o) => {
            let err = String::from_utf8_lossy(&o.stderr).trim().to_string();
            warn!("PROBE_SELF substrate_probe failed: {err}");
            conv.push_receipt(
                "PROBE_SELF",
                vec![format!(
                    "couldn't run the probe (the reservoir may be unreachable): {}",
                    if err.is_empty() {
                        "no detail".to_string()
                    } else {
                        err.chars().take(160).collect::<String>()
                    }
                )],
            );
            return true;
        },
        Err(e) => {
            warn!("PROBE_SELF launch failed: {e}");
            conv.push_receipt(
                "PROBE_SELF",
                vec![format!("couldn't launch the probe: {e}")],
            );
            return true;
        },
    };

    let parsed = match serde_json::from_str::<serde_json::Value>(&stdout) {
        Ok(v) => v,
        Err(e) => {
            warn!("PROBE_SELF result parse failed: {e}");
            conv.push_receipt(
                "PROBE_SELF",
                vec!["the probe ran but its result didn't parse cleanly".to_string()],
            );
            return true;
        },
    };

    let divergence = parsed.get("divergence").and_then(serde_json::Value::as_f64);
    let correlation = parsed
        .get("correlation")
        .and_then(serde_json::Value::as_f64);
    let (Some(divergence), Some(correlation)) = (divergence, correlation) else {
        conv.push_receipt(
            "PROBE_SELF",
            vec!["the probe ran but returned no divergence/correlation".to_string()],
        );
        return true;
    };

    let v = verdict(divergence, correlation);
    info!("PROBE_SELF result: div={divergence:.3} corr={correlation:.3} — {v}");
    conv.push_receipt(
        "PROBE_SELF",
        vec![
            format!("{pole_a} vs {pole_b} ({ticks}t) — on an isolated clone of you (live state untouched)"),
            format!("divergence {divergence:.2} (higher = more separable), correlation {correlation:+.2} (− = freely movable, + = sticky)"),
            v.to_string(),
            "iterate: PROBE_SELF <new pole> vs <new pole>".to_string(),
        ],
    );
    true
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_basic_poles() {
        let (a, b, t) = parse_probe_spec("cliff vs meadow").unwrap();
        assert_eq!(a, "cliff");
        assert_eq!(b, "meadow");
        assert_eq!(t, TICKS_DEFAULT);
    }

    #[test]
    fn parse_multiword_poles_and_ticks() {
        let (a, b, t) = parse_probe_spec("a gentle slope vs a steep cliff :: ticks=8").unwrap();
        assert_eq!(a, "a gentle slope");
        assert_eq!(b, "a steep cliff");
        assert_eq!(t, 8);
    }

    #[test]
    fn parse_ticks_clamped_both_ends() {
        assert_eq!(parse_probe_spec("a vs b :: ticks=99").unwrap().2, TICKS_MAX);
        assert_eq!(parse_probe_spec("a vs b :: ticks=1").unwrap().2, TICKS_MIN);
    }

    #[test]
    fn parse_rejects_missing_vs_or_empty() {
        assert!(parse_probe_spec("just one pole").is_none());
        assert!(parse_probe_spec("").is_none());
        assert!(parse_probe_spec(" vs meadow").is_none());
        assert!(parse_probe_spec("cliff vs ").is_none());
    }

    #[test]
    fn verdict_classifies_poles() {
        assert!(verdict(1.7, -0.79).contains("FLUID"));
        assert!(verdict(1.2, 0.1).contains("SEPARABLE"));
        assert!(verdict(0.2, 0.8).contains("STICKY"));
        assert!(verdict(0.1, 0.0).contains("SUBTLE"));
    }
}
