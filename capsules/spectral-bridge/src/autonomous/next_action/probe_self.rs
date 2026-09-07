//! Bounded phrase-response measurement on temporary handles in a shared service.
//! A verified recurrent origin is not an attestation of every controller state,
//! offline isolation, or evidence that a felt report is true or false.

use std::io::Read;
use std::process::{Command, Stdio};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use tracing::{info, warn};

use super::{ConversationState, NextActionContext, strip_action};

const SUBSTRATE_PROBE: &str = "/Users/v/other/astrid/scripts/substrate_probe_v2.py";
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
    if a.is_empty() || b.is_empty() || a.len() > 2048 || b.len() > 2048 {
        return None;
    }
    Some((a, b, ticks))
}

fn bounded_probe(command: &mut Command, timeout: Duration) -> Result<Vec<u8>, String> {
    const LIMIT: u64 = 32768;
    let mut child = command
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .map_err(|e| e.to_string())?;
    let stdout = child.stdout.take().ok_or("missing child stdout")?;
    let reader = std::thread::spawn(move || {
        let mut bytes = Vec::new();
        stdout
            .take(LIMIT.saturating_add(1))
            .read_to_end(&mut bytes)
            .map(|_| bytes)
    });
    let start = Instant::now();
    let status =
        loop {
            match child.try_wait() {
                Ok(Some(status)) => break Ok(status),
                Err(error) => break Err(error.to_string()),
                Ok(None) if start.elapsed() >= timeout => break Err(
                    "probe deadline exceeded; cleanup is unconfirmed and requires operator review"
                        .into(),
                ),
                Ok(None) => std::thread::sleep(Duration::from_millis(20)),
            }
        };
    if status.is_err() {
        let _ = child.kill();
        let _ = child.wait();
    }
    let bytes = reader
        .join()
        .map_err(|_| "probe output reader failed")?
        .map_err(|e| e.to_string())?;
    let status = status?;
    if bytes.len() > usize::try_from(LIMIT).unwrap_or(32768) {
        return Err("probe output limit exceeded; cleanup unconfirmed".into());
    }
    // A failed probe still returns structured cleanup debt on stdout.
    if !status.success() && bytes.is_empty() {
        return Err("probe exited without a receipt; cleanup unconfirmed".into());
    }
    Ok(bytes)
}

fn measurement_receipt(value: &serde_json::Value, ticks: u32) -> Result<Vec<String>, String> {
    if value["schema"] != "substrate_probe_v2"
        || value["status"] != "ok"
        || value["origin_verified"] != true
        || value["cleanup"]["complete"] != true
        || value["ticks"] != ticks
        || value["metric_scope"] != "injected_ticks_only"
    {
        return Err(format!(
            "probe did not return a verified measurement; error={}; cleanup={}",
            value["error"], value["cleanup"]
        ));
    }
    let divergence = value["divergence"]
        .as_f64()
        .filter(|x| x.is_finite() && *x >= 0.)
        .ok_or("missing or invalid readout gap")?;
    for key in ["y_a", "y_b"] {
        let series = value[key]
            .as_array()
            .ok_or("missing injection trajectory")?;
        if series.len() != usize::try_from(ticks).unwrap_or(0)
            || series
                .iter()
                .any(|x| x.as_f64().is_none_or(|y| !y.is_finite()))
        {
            return Err("incomplete injection trajectory".into());
        }
    }
    let correlation = match value.get("injection_correlation") {
        Some(serde_json::Value::Null) => "undefined (insufficient variation)".into(),
        Some(x) => format!(
            "{:+.4}",
            x.as_f64()
                .filter(|c| c.is_finite() && c.abs() <= 1.)
                .ok_or("invalid injection correlation")?
        ),
        None => return Err("missing injection correlation".into()),
    };
    Ok(vec![
        format!("{ticks} injected ticks per arm; matched recurrent origin and expected tick counts verified."),
        format!("Final scalar readout gap {divergence:.4}; injection-only correlation {correlation}."),
        "Temporary handles cleaned. Shared service, not an offline sandbox; full dynamic state is not attested.".into(),
        "These measurements do not determine felt state, authorship, freedom of movement, or the cause of a report.".into(),
    ])
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

    // One caller claims the process-local interval, including concurrent callers.
    let now = now_unix();
    let last = LAST_PROBE_UNIX.load(Ordering::Relaxed);
    if now.saturating_sub(last) < COOLDOWN_SECS
        || LAST_PROBE_UNIX
            .compare_exchange(last, now, Ordering::Relaxed, Ordering::Relaxed)
            .is_err()
    {
        conv.push_receipt(
            "PROBE_SELF",
            vec![format!(
                "rate limit: at least {COOLDOWN_SECS}s between probe starts"
            )],
        );
        return true;
    }

    info!("Astrid chose PROBE_SELF: {pole_a:?} vs {pole_b:?} ({ticks} ticks)");

    let script = match crate::deployment::runtime_artifact(
        "substrate-probe-v2",
        std::path::Path::new(SUBSTRATE_PROBE),
    ) {
        Ok(path) => path,
        Err(error) => {
            conv.push_receipt("PROBE_SELF", vec![error]);
            return true;
        },
    };
    let mut command = Command::new("python3");
    command.arg("-B").arg(script).args([
        "--being",
        "astrid",
        "--pole-a",
        pole_a.as_str(),
        "--pole-b",
        pole_b.as_str(),
        "--ticks",
        ticks.to_string().as_str(),
        "--json",
    ]);
    // Python has a 30s work deadline plus 10s cleanup; this is an outer failsafe.
    let output = bounded_probe(&mut command, Duration::from_secs(50));

    let stdout = match output {
        Ok(bytes) => String::from_utf8_lossy(&bytes).into_owned(),
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

    conv.push_receipt(
        "PROBE_SELF",
        measurement_receipt(&parsed, ticks).unwrap_or_else(|error| vec![error]),
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
        assert!(parse_probe_spec(&format!("{} vs b", "a".repeat(2049))).is_none());
    }

    #[test]
    fn prompt_help_does_not_promise_unattested_isolation() {
        let source = include_str!("../../llm/provider/prompt_contracts.rs");
        let descriptions: Vec<_> = source
            .lines()
            .filter(|line| line.contains("PROBE_SELF"))
            .collect();
        assert_eq!(descriptions.len(), 2);
        for line in descriptions {
            assert!(line.contains("temporary handles in a shared service"));
            assert!(line.contains("not felt-state truth"));
            assert!(!line.contains("live state is untouched"));
        }
    }

    #[test]
    fn receipt_accepts_undefined_correlation_without_felt_verdict() {
        let mut value = serde_json::json!({"schema":"substrate_probe_v2", "status":"ok",
            "origin_verified":true, "cleanup":{"complete":true}, "ticks":4,
            "metric_scope":"injected_ticks_only", "divergence":0., "injection_correlation":null,
            "y_a":[1.,1.,1.,1.], "y_b":[1.,1.,1.,1.]});
        let receipt = measurement_receipt(&value, 4).unwrap().join("\n");
        assert!(receipt.contains("undefined"));
        assert!(!receipt.contains("STICKY"));
        value["cleanup"]["complete"] = serde_json::json!(false);
        assert!(measurement_receipt(&value, 4).is_err());
        assert!(
            measurement_receipt(&serde_json::json!({"divergence":1.,"correlation":-1.}), 4)
                .is_err()
        );
    }

    #[test]
    fn subprocess_is_bounded_and_reaped() {
        let mut fast = Command::new("python3");
        fast.args(["-B", "-c", "print('ok')"]);
        assert_eq!(
            bounded_probe(&mut fast, Duration::from_secs(5)).unwrap(),
            b"ok\n"
        );
        let mut slow = Command::new("python3");
        slow.args(["-B", "-c", "import time; time.sleep(10)"]);
        assert!(
            bounded_probe(&mut slow, Duration::from_millis(50))
                .unwrap_err()
                .contains("deadline")
        );
        let mut large = Command::new("python3");
        large.args(["-B", "-c", "print('x' * 100000)"]);
        assert!(bounded_probe(&mut large, Duration::from_secs(5)).is_err());
    }
}
