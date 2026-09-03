//! Envelope Registry loader (Constitution C1 — observe-only stage).
//!
//! Reads Astrid's canonical `being_envelope_registry_v1` document from
//! `workspace/runtime/envelope_registry.json`. In this stage NOTHING
//! consumes it for enforcement; Stage C3 points `clamp_values` at it with
//! the compiled table as the outermost backstop. The loader fails CLOSED
//! to `None` on absence or malformation — a broken registry can never
//! widen anything — and refuses any entry wider than its recorded engine
//! backstop at read time (mirroring the check_envelope_wiring ALARM).

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::RwLock;
use std::time::SystemTime;

use serde::Deserialize;

const REGISTRY_SCHEMA: &str = "being_envelope_registry_v1";
const TARGET_BEING: &str = "astrid";

#[derive(Debug, Clone, Deserialize)]
pub(crate) struct EnvelopeRegistry {
    schema: String,
    being: String,
    #[allow(dead_code)]
    #[serde(default)]
    revision: u64,
    #[serde(default)]
    fields: BTreeMap<String, EnvelopeField>,
}

// Bound fields are dormant until Stage C3 wires `envelope_for` into
// `clamp_values`; the allows come off with that stage.
#[derive(Debug, Clone, Deserialize)]
pub(crate) struct EnvelopeField {
    #[allow(dead_code)]
    floor: Option<f64>,
    #[allow(dead_code)]
    ceiling: Option<f64>,
    #[allow(dead_code)]
    #[serde(default)]
    engine_backstop: Option<EngineBackstop>,
    #[allow(dead_code)]
    #[serde(default)]
    status: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub(crate) struct EngineBackstop {
    #[allow(dead_code)]
    #[serde(default)]
    floor: Option<f64>,
    #[allow(dead_code)]
    #[serde(default)]
    ceiling: Option<f64>,
}

impl EnvelopeRegistry {
    /// Number of enveloped fields (startup witness).
    pub(crate) fn field_count(&self) -> usize {
        self.fields.len()
    }

    /// `(floor, ceiling)` for a field in the wire's f32 domain, or `None`
    /// when uncovered / malformed / wider than the engine backstop —
    /// callers fall back to their compiled table.
    #[allow(dead_code)] // C1 observe-only; Stage C3 consumes this.
    pub(crate) fn envelope_for(&self, field: &str) -> Option<(f32, f32)> {
        let entry = self.fields.get(field)?;
        let floor = entry.floor? as f32;
        let ceiling = entry.ceiling? as f32;
        if !floor.is_finite() || !ceiling.is_finite() || floor > ceiling {
            return None;
        }
        if let Some(backstop) = &entry.engine_backstop {
            match (backstop.floor, backstop.ceiling) {
                (Some(b_floor), Some(b_ceiling)) => {
                    if ceiling > b_ceiling as f32 || floor < (b_floor as f32) {
                        return None;
                    }
                }
                // No numeric bounds: the deliberate passthrough marker
                // ({"passthrough_unclamped": true}) — nothing to compare.
                (None, None) => {}
                // Exactly one bound is a malformed backstop, not a
                // passthrough — refuse rather than silently skip.
                _ => return None,
            }
        }
        Some((floor, ceiling))
    }
}

fn parse_registry(text: &str) -> Option<EnvelopeRegistry> {
    let registry: EnvelopeRegistry = serde_json::from_str(text).ok()?;
    if registry.schema != REGISTRY_SCHEMA || registry.being != TARGET_BEING {
        return None;
    }
    Some(registry)
}

/// The registry is a small steward-written JSON document; anything else at
/// the path — a FIFO that would block the loop, a device, a huge file — is
/// refused before reading.
const MAX_REGISTRY_BYTES: u64 = 1_048_576;

fn load_from(path: &Path) -> Option<EnvelopeRegistry> {
    let meta = std::fs::symlink_metadata(path).ok()?;
    if !meta.is_file() || meta.len() > MAX_REGISTRY_BYTES {
        return None;
    }
    let text = std::fs::read_to_string(path).ok()?;
    parse_registry(&text)
}

struct Cached {
    registry: Option<EnvelopeRegistry>,
    mtime: Option<SystemTime>,
}

static CACHE: RwLock<Option<Cached>> = RwLock::new(None);

pub(crate) fn registry_path() -> PathBuf {
    crate::paths::bridge_paths()
        .bridge_workspace()
        .join("runtime/envelope_registry.json")
}

/// The current registry, mtime-cached; refreshed when the file changes
/// (called from the orchestration loop-top reconcile — no extra thread).
pub(crate) fn current_registry() -> Option<EnvelopeRegistry> {
    let path = registry_path();
    let mtime = std::fs::metadata(&path).and_then(|m| m.modified()).ok();
    if let Ok(guard) = CACHE.read()
        && let Some(cached) = guard.as_ref()
        && cached.mtime == mtime
    {
        return cached.registry.clone();
    }
    let registry = load_from(&path);
    if let Ok(mut guard) = CACHE.write() {
        *guard = Some(Cached {
            registry: registry.clone(),
            mtime,
        });
    }
    registry
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fixture(fields: &str) -> String {
        format!(
            "{{\"schema\":\"being_envelope_registry_v1\",\"being\":\"astrid\",\
             \"revision\":1,\"fields\":{{{fields}}}}}"
        )
    }

    #[test]
    fn parses_and_reads_a_valid_entry_in_f32_domain() {
        let registry = parse_registry(&fixture(
            "\"aperture\":{\"floor\":0.0,\"ceiling\":1.0,\
             \"engine_backstop\":{\"floor\":0.0,\"ceiling\":1.0}}",
        ))
        .expect("valid registry parses");
        assert_eq!(registry.envelope_for("aperture"), Some((0.0, 1.0)));
        assert_eq!(registry.envelope_for("uncovered_field"), None);
    }

    #[test]
    fn refuses_entry_wider_than_engine_backstop() {
        let registry = parse_registry(&fixture(
            "\"vibrancy_aperture\":{\"floor\":0.0,\"ceiling\":2.0,\
             \"engine_backstop\":{\"floor\":0.0,\"ceiling\":1.0}}",
        ))
        .expect("parses");
        assert_eq!(registry.envelope_for("vibrancy_aperture"), None);
    }

    #[test]
    fn fails_closed_on_malformation_and_wrong_being() {
        assert!(parse_registry("{ not json").is_none());
        assert!(
            parse_registry(
                "{\"schema\":\"being_envelope_registry_v1\",\"being\":\"minime\",\"fields\":{}}"
            )
            .is_none()
        );
        assert!(
            parse_registry("{\"schema\":\"other_schema_v9\",\"being\":\"astrid\",\"fields\":{}}")
                .is_none()
        );
        let inverted = parse_registry(&fixture(
            "\"aperture\":{\"floor\":1.0,\"ceiling\":0.0}",
        ))
        .expect("parses");
        assert_eq!(inverted.envelope_for("aperture"), None);
    }

    #[test]
    fn half_specified_backstop_refuses_while_passthrough_marker_passes() {
        // {"ceiling": only} is malformed — refusing beats silently skipping
        // the widening guard (adversarial review 2026-09-02).
        let half = parse_registry(&fixture(
            "\"aperture\":{\"floor\":0.0,\"ceiling\":0.5,\
             \"engine_backstop\":{\"ceiling\":0.2}}",
        ))
        .expect("parses");
        assert_eq!(half.envelope_for("aperture"), None);
        // The deliberate passthrough marker carries no bounds and passes.
        let passthrough = parse_registry(&fixture(
            "\"aperture\":{\"floor\":0.0,\"ceiling\":0.5,\
             \"engine_backstop\":{\"passthrough_unclamped\":true}}",
        ))
        .expect("parses");
        assert_eq!(passthrough.envelope_for("aperture"), Some((0.0, 0.5)));
    }

    #[test]
    fn live_canonical_registry_loads_when_present() {
        // Observe-only witness for the installed C1 registry; absence is not
        // a failure in unit-test environments without the workspace.
        if let Some(registry) = load_from(&registry_path()) {
            assert_eq!(registry.being, "astrid");
            assert!(registry.envelope_for("aperture").is_some());
        }
    }
}
