//! MLX reflective controller sidecar.
//!
//! Calls `chat_mlx_local.py` as a subprocess to get structured controller
//! telemetry: regime classification, observer reports, field/geometry probes,
//! and condition vectors. This gives Astrid qualitative perception of spectral
//! state rather than just numerical summaries.
//!
//! The sidecar has its own 48-64D echo state reservoir that tracks Astrid's
//! reflective trajectory independently from minime's 128-node ESN.

use crate::paths::bridge_paths;
use serde::{Deserialize, Serialize};
use std::{
    path::Path,
    process::{Command, Output, Stdio},
    thread,
    time::{Duration, Instant},
};
use tracing::{debug, info, warn};

const STORED_PROMPT_COMPACT_THRESHOLD_CHARS: usize = 800;
const STORED_PROMPT_PREVIEW_CHARS: usize = 480;
const REFLECTIVE_REWRITE_MAX_ATTEMPTS_ENV: &str = "ASTRID_REFLECTIVE_REWRITE_MAX_ATTEMPTS";
const REFLECTIVE_REWRITE_BUDGET_SECONDS_ENV: &str = "ASTRID_REFLECTIVE_REWRITE_BUDGET_SECONDS";
const DEFAULT_REFLECTIVE_REWRITE_MAX_ATTEMPTS: u32 = 1;
const MAX_REFLECTIVE_REWRITE_MAX_ATTEMPTS: u32 = 3;
const DEFAULT_REFLECTIVE_REWRITE_BUDGET_SECONDS: u64 = 90;
const MAX_REFLECTIVE_REWRITE_BUDGET_SECONDS: u64 = 600;
const REFLECTIVE_SIDECAR_TIMEOUT_SECONDS_ENV: &str = "ASTRID_REFLECTIVE_SIDECAR_TIMEOUT_SECONDS";
const DEFAULT_REFLECTIVE_SIDECAR_TIMEOUT_SECONDS: u64 = 240;
const MIN_REFLECTIVE_SIDECAR_TIMEOUT_SECONDS: u64 = 30;
const MAX_REFLECTIVE_SIDECAR_TIMEOUT_SECONDS: u64 = 900;
const SIDECAR_WAIT_POLL_MS: u64 = 200;

/// Lightweight regime classification — runs every exchange in <1ms.
/// No LLM, no subprocess. Pure computation on spectral telemetry.
///
/// Returns a regime label and reason that can be injected into Astrid's
/// prompt context to give her qualitative awareness of spectral conditions.
#[derive(Debug, Clone)]
pub struct LightRegime {
    pub regime: &'static str,
    pub reason: String,
    pub fill_trend: &'static str,
}

/// Persistent state for the lightweight regime tracker.
#[derive(Debug, Clone)]
pub struct RegimeTracker {
    prev_fill: f32,
    prev_prev_fill: f32,
    stable_count: u32,
    expanding_count: u32,
    contracting_count: u32,
}

impl Default for RegimeTracker {
    fn default() -> Self {
        Self::new()
    }
}

impl RegimeTracker {
    pub fn new() -> Self {
        Self {
            prev_fill: 0.0,
            prev_prev_fill: 0.0,
            stable_count: 0,
            expanding_count: 0,
            contracting_count: 0,
        }
    }

    /// Classify the current regime from fill trajectory.
    /// Call once per exchange with current telemetry.
    pub fn classify(&mut self, fill_pct: f32, lambda1_rel: f32, _geom_rel: f32) -> LightRegime {
        let dfill = fill_pct - self.prev_fill;
        let accel = dfill - (self.prev_fill - self.prev_prev_fill);

        // Update history
        self.prev_prev_fill = self.prev_fill;
        self.prev_fill = fill_pct;

        // Classify trend
        let fill_trend = if dfill > 2.0 {
            self.expanding_count = self.expanding_count.saturating_add(1);
            self.contracting_count = 0;
            self.stable_count = 0;
            "expanding"
        } else if dfill < -2.0 {
            self.contracting_count = self.contracting_count.saturating_add(1);
            self.expanding_count = 0;
            self.stable_count = 0;
            "contracting"
        } else {
            self.stable_count = self.stable_count.saturating_add(1);
            self.expanding_count = 0;
            self.contracting_count = 0;
            "stable"
        };

        // Regime classification (inspired by MLX sidecar's regime system)
        let (regime, reason) = if fill_pct < 10.0 {
            (
                "recovery",
                format!("fill critically low ({fill_pct:.0}%) — cold start or major contraction"),
            )
        } else if self.contracting_count >= 3 && fill_pct < 25.0 {
            (
                "escape",
                format!(
                    "sustained contraction ({} ticks) at low fill ({fill_pct:.0}%)",
                    self.contracting_count
                ),
            )
        } else if self.expanding_count >= 2 && fill_pct > 40.0 {
            (
                "consolidate",
                format!("expanding into target range ({fill_pct:.0}%), stabilizing"),
            )
        } else if self.stable_count >= 4 && fill_pct > 30.0 && fill_pct < 70.0 {
            (
                "sustain",
                format!(
                    "stable in healthy range ({fill_pct:.0}%) for {} ticks",
                    self.stable_count
                ),
            )
        } else if accel.abs() > 5.0 {
            (
                "rebind",
                format!("rapid acceleration ({accel:+.1}%/tick²), seeking new basin"),
            )
        } else if lambda1_rel < 0.3 && fill_pct < 20.0 {
            (
                "recovery",
                format!("lambda1_rel low ({lambda1_rel:.2}), reservoir warming up"),
            )
        } else {
            (
                "sustain",
                format!("ordinary reflective state (fill {fill_pct:.0}%, dfill {dfill:+.1}%)"),
            )
        };

        LightRegime {
            regime,
            reason,
            fill_trend,
        }
    }

    /// Format as a one-line context string for prompt injection.
    pub fn format_context(regime: &LightRegime) -> String {
        format!(
            "[Regime: {} — {} | trend: {}]",
            regime.regime, regime.reason, regime.fill_trend
        )
    }
}

/// Structured output from the MLX reflective controller.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReflectiveReport {
    /// Controller regime: sustain, escape, rebind, consolidate
    #[serde(default)]
    pub controller_regime: Option<String>,

    /// Why the controller chose this regime
    #[serde(default)]
    pub controller_regime_reason: Option<String>,

    /// Observer report — qualitative description of current state
    #[serde(default)]
    pub observer_report: Option<serde_json::Value>,

    /// What changed since last observation
    #[serde(default)]
    pub change_report: Option<String>,

    /// Embedding field probe — which semantic anchors are active
    #[serde(default)]
    pub prompt_embedding_field: Option<serde_json::Value>,

    /// Reservoir geometry — collapse, persistence, drift
    #[serde(default)]
    pub reservoir_geometry: Option<serde_json::Value>,

    /// Condition vector — 9 failure/stress signals
    #[serde(default)]
    pub condition_vector: Option<serde_json::Value>,

    /// Self-tuning state
    #[serde(default)]
    pub self_tuning: Option<serde_json::Value>,

    /// Generated text (reflective response)
    #[serde(default)]
    pub text: Option<String>,

    /// Profiling data
    #[serde(default)]
    pub profiling: Option<serde_json::Value>,
}

impl ReflectiveReport {
    /// Return a steward-facing storage snapshot.
    ///
    /// The reflective sidecar needs full prompts internally, but the mirrored
    /// controller artifact should not repeat multi-kilobyte prompt bodies on
    /// every introspection. Keep a preview and character count for audit while
    /// preserving the controller telemetry itself.
    pub fn storage_snapshot(&self) -> serde_json::Value {
        let mut value = match serde_json::to_value(self) {
            Ok(value) => value,
            Err(error) => {
                return serde_json::json!({
                    "serialization_error": error.to_string(),
                });
            },
        };
        compact_controller_prompt_at(&mut value, "/self_tuning/last_model_advice");
        compact_controller_prompt_at(&mut value, "/self_tuning/last_model_advice/forecast");
        value
    }

    /// Format the controller telemetry as a compact context block for Astrid's prompt.
    pub fn as_context_block(&self) -> String {
        let mut parts = Vec::new();

        if let Some(ref regime) = self.controller_regime {
            let reason = self
                .controller_regime_reason
                .as_deref()
                .unwrap_or("unknown");
            parts.push(format!("Controller regime: {regime} ({reason})"));
        }

        if let Some(ref geo) = self.reservoir_geometry {
            let collapse = geo.get("geometry_collapse").and_then(|v| v.as_f64());
            let persist = geo.get("attractor_persistence").and_then(|v| v.as_f64());
            let drift = geo.get("state_drift").and_then(|v| v.as_f64());
            if let (Some(c), Some(p)) = (collapse, persist) {
                parts.push(format!(
                    "Geometry: collapse={c:.2}, persistence={p:.2}{}",
                    drift.map(|d| format!(", drift={d:.2}")).unwrap_or_default()
                ));
            }
        }

        if let Some(ref field) = self.prompt_embedding_field
            && let Some(anchors) = field.get("top_anchors").and_then(|a| a.as_array())
        {
            let labels: Vec<&str> = anchors
                .iter()
                .filter_map(|a| a.get("label").and_then(|l| l.as_str()))
                .collect();
            if !labels.is_empty() {
                parts.push(format!("Field anchors: {}", labels.join(", ")));
            }
        }

        if let Some(ref cond) = self.condition_vector {
            let severity = cond.get("severity").and_then(|v| v.as_f64());
            let lock = cond.get("attractor_lock").and_then(|v| v.as_f64());
            let miss = cond.get("field_miss").and_then(|v| v.as_f64());
            if let Some(s) = severity {
                parts.push(format!(
                    "Condition: severity={s:.2}{}{}",
                    lock.map(|l| format!(", lock={l:.2}")).unwrap_or_default(),
                    miss.map(|m| format!(", field_miss={m:.2}"))
                        .unwrap_or_default(),
                ));
            }
        }

        if let Some(ref change) = self.change_report {
            parts.push(format!("Change: {change}"));
        }

        if parts.is_empty() {
            String::new()
        } else {
            format!("[Reflective controller observation:]\n{}", parts.join("\n"))
        }
    }
}

fn compact_controller_prompt_at(value: &mut serde_json::Value, pointer: &str) {
    let Some(parent) = value.pointer_mut(pointer) else {
        return;
    };
    let Some(map) = parent.as_object_mut() else {
        return;
    };
    let Some(prompt) = map
        .get("prompt")
        .and_then(serde_json::Value::as_str)
        .map(str::to_owned)
    else {
        return;
    };
    let full_chars = prompt.chars().count();
    if full_chars <= STORED_PROMPT_COMPACT_THRESHOLD_CHARS {
        return;
    }
    let preview: String = prompt.chars().take(STORED_PROMPT_PREVIEW_CHARS).collect();
    let preview_chars = preview.chars().count();
    map.insert(
        "prompt".to_string(),
        serde_json::Value::String(format!(
            "[compacted controller prompt; full_chars={full_chars}; preview_chars={preview_chars}]"
        )),
    );
    map.insert(
        "prompt_compacted_v1".to_string(),
        serde_json::json!({
            "storage": "compacted_for_controller_snapshot",
            "full_chars": full_chars,
            "preview_chars": preview_chars,
            "preview": preview,
        }),
    );
}

fn parse_bounded_u32(raw: Option<&str>, default: u32, max: u32) -> u32 {
    raw.and_then(|value| value.trim().parse::<u32>().ok())
        .map_or(default, |value| value.min(max))
}

fn parse_bounded_u64(raw: Option<&str>, default: u64, max: u64) -> u64 {
    raw.and_then(|value| value.trim().parse::<u64>().ok())
        .map_or(default, |value| value.min(max))
}

fn parse_bounded_u64_range(raw: Option<&str>, default: u64, min: u64, max: u64) -> u64 {
    raw.and_then(|value| value.trim().parse::<u64>().ok())
        .map_or(default, |value| value.clamp(min, max))
}

fn reflective_rewrite_max_attempts() -> u32 {
    let raw = std::env::var(REFLECTIVE_REWRITE_MAX_ATTEMPTS_ENV).ok();
    parse_bounded_u32(
        raw.as_deref(),
        DEFAULT_REFLECTIVE_REWRITE_MAX_ATTEMPTS,
        MAX_REFLECTIVE_REWRITE_MAX_ATTEMPTS,
    )
}

fn reflective_rewrite_budget_seconds() -> u64 {
    let raw = std::env::var(REFLECTIVE_REWRITE_BUDGET_SECONDS_ENV).ok();
    parse_bounded_u64(
        raw.as_deref(),
        DEFAULT_REFLECTIVE_REWRITE_BUDGET_SECONDS,
        MAX_REFLECTIVE_REWRITE_BUDGET_SECONDS,
    )
}

fn reflective_sidecar_timeout_seconds() -> u64 {
    let raw = std::env::var(REFLECTIVE_SIDECAR_TIMEOUT_SECONDS_ENV).ok();
    parse_bounded_u64_range(
        raw.as_deref(),
        DEFAULT_REFLECTIVE_SIDECAR_TIMEOUT_SECONDS,
        MIN_REFLECTIVE_SIDECAR_TIMEOUT_SECONDS,
        MAX_REFLECTIVE_SIDECAR_TIMEOUT_SECONDS,
    )
}

struct TimedSidecarOutput {
    output: Output,
    timed_out: bool,
}

fn run_sidecar_command_with_timeout(
    command: &mut Command,
    timeout: Duration,
) -> std::io::Result<TimedSidecarOutput> {
    let mut child = command.spawn()?;
    let started = Instant::now();
    loop {
        if child.try_wait()?.is_some() {
            return child.wait_with_output().map(|output| TimedSidecarOutput {
                output,
                timed_out: false,
            });
        }
        if started.elapsed() >= timeout {
            let _ = child.kill();
            return child.wait_with_output().map(|output| TimedSidecarOutput {
                output,
                timed_out: true,
            });
        }
        thread::sleep(Duration::from_millis(SIDECAR_WAIT_POLL_MS));
    }
}

/// Call the MLX reflective controller sidecar with spectral context.
///
/// Returns structured controller telemetry. Runs as a subprocess —
/// acceptable for INTROSPECT/OPEN_MIND (rare, ~1 in 15 exchanges).
/// For lighter per-exchange telemetry, use `query_controller_light()` (future).
pub async fn query_sidecar(spectral_context: &str) -> Option<ReflectiveReport> {
    let sidecar_script = bridge_paths().reflective_sidecar_script().to_path_buf();
    let script = Path::new(&sidecar_script);
    if !script.exists() {
        warn!("MLX sidecar script not found at {}", script.display());
        return None;
    }

    let prompt = spectral_context.to_string();

    debug!("calling MLX reflective sidecar");

    tokio::task::spawn_blocking(move || {
        let rewrite_max_attempts = reflective_rewrite_max_attempts().to_string();
        let rewrite_budget_seconds = reflective_rewrite_budget_seconds().to_string();
        let sidecar_timeout = Duration::from_secs(reflective_sidecar_timeout_seconds());
        let mut command = Command::new("python3");
        command
            .arg(&sidecar_script)
            .arg("--json")
            .arg("--hardware-profile")
            .arg("m4-mini")
            .arg("--model-label")
            .arg("gemma3-12b")
            .arg("--mode")
            .arg("reflective")
            .arg("--architecture")
            .arg("reservoir-fixed")
            .arg("--rewrite-max-attempts")
            .arg(&rewrite_max_attempts)
            .arg("--rewrite-budget-seconds")
            .arg(&rewrite_budget_seconds)
            .arg("--prompt")
            .arg(&prompt)
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        let run = run_sidecar_command_with_timeout(&mut command, sidecar_timeout).ok()?;
        if run.timed_out {
            warn!(
                timeout_seconds = sidecar_timeout.as_secs(),
                "MLX sidecar timed out; killed reflective subprocess"
            );
            return None;
        }
        let output = run.output;

        if !output.status.success() {
            warn!("MLX sidecar exited with status {}", output.status);
            return None;
        }

        let stdout = String::from_utf8_lossy(&output.stdout);
        let stderr_str = String::from_utf8_lossy(&output.stderr);
        // Log model identity from stderr (chat_mlx_local.py prints loading info there)
        if let Some(model_line) = stderr_str
            .lines()
            .find(|l| l.contains("model") || l.contains("loading"))
        {
            info!("MLX sidecar model: {}", model_line.trim());
        }
        match serde_json::from_str::<ReflectiveReport>(&stdout) {
            Ok(report) => {
                info!(
                    regime = report.controller_regime.as_deref().unwrap_or("?"),
                    "MLX sidecar returned controller report"
                );
                Some(report)
            },
            Err(e) => {
                warn!("MLX sidecar JSON parse failed: {e}");
                None
            },
        }
    })
    .await
    .ok()
    .flatten()
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn empty_report_with_self_tuning(self_tuning: serde_json::Value) -> ReflectiveReport {
        ReflectiveReport {
            controller_regime: None,
            controller_regime_reason: None,
            observer_report: None,
            change_report: None,
            prompt_embedding_field: None,
            reservoir_geometry: None,
            condition_vector: None,
            self_tuning: Some(self_tuning),
            text: None,
            profiling: None,
        }
    }

    #[test]
    fn storage_snapshot_compacts_prompt_heavy_model_advice() {
        let prompt = "spectral context ".repeat(120);
        let report = empty_report_with_self_tuning(json!({
            "last_model_advice": {
                "prompt": prompt,
                "forecast": {
                    "prompt": "forecast context ".repeat(120),
                    "summary": "steady",
                },
                "reason": "steady",
            }
        }));

        let snapshot = report.storage_snapshot();

        let advice_prompt = snapshot
            .pointer("/self_tuning/last_model_advice/prompt")
            .and_then(serde_json::Value::as_str)
            .unwrap_or_default();
        assert!(advice_prompt.starts_with("[compacted controller prompt;"));
        assert!(
            snapshot
                .pointer("/self_tuning/last_model_advice/prompt_compacted_v1/preview")
                .and_then(serde_json::Value::as_str)
                .is_some_and(|preview| preview.starts_with("spectral context "))
        );
        let forecast_prompt = snapshot
            .pointer("/self_tuning/last_model_advice/forecast/prompt")
            .and_then(serde_json::Value::as_str)
            .unwrap_or_default();
        assert!(forecast_prompt.starts_with("[compacted controller prompt;"));
        assert_eq!(
            snapshot.pointer("/self_tuning/last_model_advice/reason"),
            Some(&json!("steady"))
        );
    }

    #[test]
    fn storage_snapshot_keeps_short_model_advice_prompt() {
        let report = empty_report_with_self_tuning(json!({
            "last_model_advice": {
                "prompt": "short audit context",
            }
        }));

        let snapshot = report.storage_snapshot();

        assert_eq!(
            snapshot.pointer("/self_tuning/last_model_advice/prompt"),
            Some(&json!("short audit context"))
        );
        assert!(
            snapshot
                .pointer("/self_tuning/last_model_advice/prompt_compacted_v1")
                .is_none()
        );
    }

    #[test]
    fn reflective_rewrite_attempt_cap_parsing_defaults_and_clamps() {
        assert_eq!(
            parse_bounded_u32(
                None,
                DEFAULT_REFLECTIVE_REWRITE_MAX_ATTEMPTS,
                MAX_REFLECTIVE_REWRITE_MAX_ATTEMPTS,
            ),
            DEFAULT_REFLECTIVE_REWRITE_MAX_ATTEMPTS
        );
        assert_eq!(
            parse_bounded_u32(
                Some("2"),
                DEFAULT_REFLECTIVE_REWRITE_MAX_ATTEMPTS,
                MAX_REFLECTIVE_REWRITE_MAX_ATTEMPTS,
            ),
            2
        );
        assert_eq!(
            parse_bounded_u32(
                Some("99"),
                DEFAULT_REFLECTIVE_REWRITE_MAX_ATTEMPTS,
                MAX_REFLECTIVE_REWRITE_MAX_ATTEMPTS,
            ),
            MAX_REFLECTIVE_REWRITE_MAX_ATTEMPTS
        );
        assert_eq!(
            parse_bounded_u32(
                Some("nope"),
                DEFAULT_REFLECTIVE_REWRITE_MAX_ATTEMPTS,
                MAX_REFLECTIVE_REWRITE_MAX_ATTEMPTS,
            ),
            DEFAULT_REFLECTIVE_REWRITE_MAX_ATTEMPTS
        );
    }

    #[test]
    fn reflective_rewrite_budget_parsing_defaults_and_clamps() {
        assert_eq!(
            parse_bounded_u64(
                None,
                DEFAULT_REFLECTIVE_REWRITE_BUDGET_SECONDS,
                MAX_REFLECTIVE_REWRITE_BUDGET_SECONDS,
            ),
            DEFAULT_REFLECTIVE_REWRITE_BUDGET_SECONDS
        );
        assert_eq!(
            parse_bounded_u64(
                Some("120"),
                DEFAULT_REFLECTIVE_REWRITE_BUDGET_SECONDS,
                MAX_REFLECTIVE_REWRITE_BUDGET_SECONDS,
            ),
            120
        );
        assert_eq!(
            parse_bounded_u64(
                Some("9999"),
                DEFAULT_REFLECTIVE_REWRITE_BUDGET_SECONDS,
                MAX_REFLECTIVE_REWRITE_BUDGET_SECONDS,
            ),
            MAX_REFLECTIVE_REWRITE_BUDGET_SECONDS
        );
        assert_eq!(
            parse_bounded_u64(
                Some(""),
                DEFAULT_REFLECTIVE_REWRITE_BUDGET_SECONDS,
                MAX_REFLECTIVE_REWRITE_BUDGET_SECONDS,
            ),
            DEFAULT_REFLECTIVE_REWRITE_BUDGET_SECONDS
        );
    }

    #[test]
    fn reflective_sidecar_timeout_parsing_defaults_and_clamps() {
        assert_eq!(
            parse_bounded_u64_range(
                None,
                DEFAULT_REFLECTIVE_SIDECAR_TIMEOUT_SECONDS,
                MIN_REFLECTIVE_SIDECAR_TIMEOUT_SECONDS,
                MAX_REFLECTIVE_SIDECAR_TIMEOUT_SECONDS,
            ),
            DEFAULT_REFLECTIVE_SIDECAR_TIMEOUT_SECONDS
        );
        assert_eq!(
            parse_bounded_u64_range(
                Some("5"),
                DEFAULT_REFLECTIVE_SIDECAR_TIMEOUT_SECONDS,
                MIN_REFLECTIVE_SIDECAR_TIMEOUT_SECONDS,
                MAX_REFLECTIVE_SIDECAR_TIMEOUT_SECONDS,
            ),
            MIN_REFLECTIVE_SIDECAR_TIMEOUT_SECONDS
        );
        assert_eq!(
            parse_bounded_u64_range(
                Some("1200"),
                DEFAULT_REFLECTIVE_SIDECAR_TIMEOUT_SECONDS,
                MIN_REFLECTIVE_SIDECAR_TIMEOUT_SECONDS,
                MAX_REFLECTIVE_SIDECAR_TIMEOUT_SECONDS,
            ),
            MAX_REFLECTIVE_SIDECAR_TIMEOUT_SECONDS
        );
    }

    #[test]
    fn sidecar_timeout_preserves_fast_output() {
        let mut command = Command::new("python3");
        command
            .arg("-c")
            .arg("print('reflective-ok')")
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());

        let run = run_sidecar_command_with_timeout(&mut command, Duration::from_secs(5)).unwrap();

        assert!(!run.timed_out);
        assert!(run.output.status.success());
        assert_eq!(
            String::from_utf8_lossy(&run.output.stdout).trim(),
            "reflective-ok"
        );
    }

    #[test]
    fn sidecar_timeout_kills_slow_child() {
        let mut command = Command::new("python3");
        command
            .arg("-c")
            .arg("import time; time.sleep(5); print('late')")
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());

        let run = run_sidecar_command_with_timeout(&mut command, Duration::from_millis(50))
            .expect("slow child should be killed and collected");

        assert!(run.timed_out);
        assert!(!run.output.status.success());
    }
}
