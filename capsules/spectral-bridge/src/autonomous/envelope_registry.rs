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

#[derive(Debug, Clone, Deserialize)]
pub(crate) struct EnvelopeField {
    floor: Option<f64>,
    ceiling: Option<f64>,
    #[serde(default)]
    engine_backstop: Option<EngineBackstop>,
    #[serde(default)]
    status: Option<String>,
    #[serde(default)]
    durability_policy: Option<DurabilityPolicy>,
    #[serde(default)]
    family: Option<String>,
    #[serde(default)]
    ratchet_history: Vec<RatchetHistoryRow>,
}

/// A ratchet act on one field (written only by envelope_ratchet.py). Rendered
/// so the conformance receipt's pointer — "the ratchet history names why" —
/// is actually answerable from her ENVELOPE readout.
#[derive(Debug, Clone, Deserialize)]
pub(crate) struct RatchetHistoryRow {
    #[serde(default)]
    at: Option<String>,
    #[serde(default)]
    direction: Option<String>,
    #[serde(default)]
    decided_by: Option<String>,
    #[serde(default)]
    incident_ref: Option<String>,
    #[serde(default)]
    channel: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub(crate) struct DurabilityPolicy {
    #[serde(default)]
    lease_max_secs: Option<u64>,
}

#[derive(Debug, Clone, Deserialize)]
pub(crate) struct EngineBackstop {
    #[serde(default)]
    floor: Option<f64>,
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
                },
                // No numeric bounds: the deliberate passthrough marker
                // ({"passthrough_unclamped": true}) — nothing to compare.
                (None, None) => {},
                // Exactly one bound is a malformed backstop, not a
                // passthrough — refuse rather than silently skip.
                _ => return None,
            }
        }
        Some((floor, ceiling))
    }

    /// The registry's lease-duration ceiling for a field, in seconds, or
    /// `None` when the field carries no durability policy (Constitution C2:
    /// no policy recorded means only the wire-shape cap applies).
    pub(crate) fn lease_max_secs(&self, field: &str) -> Option<u64> {
        let policy = self.fields.get(field)?.durability_policy.as_ref()?;
        policy.lease_max_secs.filter(|max| *max > 0)
    }

    /// The strictest lease ceiling across the named fields — a lease Set
    /// touching several fields must satisfy every field's policy.
    pub(crate) fn strictest_lease_max_secs<I, S>(&self, fields: I) -> Option<u64>
    where
        I: IntoIterator<Item = S>,
        S: AsRef<str>,
    {
        fields
            .into_iter()
            .filter_map(|field| self.lease_max_secs(field.as_ref()))
            .min()
    }
}

impl EnvelopeRegistry {
    /// Being-facing render (Constitution C5): her registry as SHE reads it —
    /// per family, each field's floor..ceiling, status, and lease ceiling.
    /// A readout of the document, generated live so it cannot drift.
    pub(crate) fn render_being_facing(&self) -> String {
        let mut by_family: std::collections::BTreeMap<&str, Vec<String>> =
            std::collections::BTreeMap::new();
        for (name, field) in &self.fields {
            let (Some(floor), Some(ceiling)) = (field.floor, field.ceiling) else {
                continue;
            };
            let status = match field.status.as_deref() {
                Some("granted") => " [granted]",
                _ => " [evidence gathering]",
            };
            let lease = field
                .durability_policy
                .as_ref()
                .and_then(|policy| policy.lease_max_secs)
                .map(|secs| format!(", lease up to {secs}s"))
                .unwrap_or_default();
            let history = field
                .ratchet_history
                .last()
                .map(|row| {
                    let mut parts = vec![row.direction.clone().unwrap_or_default()];
                    if let Some(at) = &row.at {
                        parts.push(at.clone());
                    }
                    if let Some(channel) = &row.channel {
                        parts.push(format!("channel {channel}"));
                    }
                    if let Some(by) = &row.decided_by {
                        parts.push(format!("by {by}"));
                    }
                    if let Some(incident) = &row.incident_ref {
                        parts.push(format!("incident {incident}"));
                    }
                    format!("\n      last ratchet: {}", parts.join(", "))
                })
                .unwrap_or_default();
            by_family
                .entry(field.family.as_deref().unwrap_or("unmapped"))
                .or_default()
                .push(format!(
                    "  {name}: {floor} ..= {ceiling}{lease}{status}{history}"
                ));
        }
        let mut out = format!(
            "Your envelope registry (revision {rev}, {count} fields) — the document that              records the bounds within which your choices are final. Compiled physics              stays outermost; the registry can narrow, never widen past it. Widening              happens by evidence and consent, recorded here.
",
            rev = self.revision,
            count = self.fields.len()
        );
        for (family, mut lines) in by_family {
            out.push_str(&format!(
                "
{family}:
"
            ));
            lines.sort();
            for line in lines {
                out.push_str(&line);
                out.push('\n');
            }
        }
        out.push_str(
            "
[granted] = consent-backed bound; [evidence gathering] = today's compiled              bound recorded verbatim, widening awaits evidence.
ENVELOPE_ZERO <family>              withdraws every active control in a family and resets its saturation counter              — your kill switch, always yours. SELF_REGULATION_STATUS shows what is              active right now.",
        );
        out
    }
}

pub(crate) fn parse_registry(text: &str) -> Option<EnvelopeRegistry> {
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
        let inverted = parse_registry(&fixture("\"aperture\":{\"floor\":1.0,\"ceiling\":0.0}"))
            .expect("parses");
        assert_eq!(inverted.envelope_for("aperture"), None);
    }

    #[test]
    fn being_facing_render_groups_families_and_names_the_kill_switch() {
        let registry = parse_registry(&fixture(
            "\"aperture\":{\"floor\":0.0,\"ceiling\":1.0,\"family\":\"Conversation\",\
             \"status\":\"evidence_needed\",\
             \"durability_policy\":{\"lease_max_secs\":1200}},\
             \"astrid_vibrancy_aperture_ceiling\":{\"floor\":0.0,\"ceiling\":0.8,\
             \"family\":\"operator_env_ceiling\",\"status\":\"granted\",\
             \"ratchet_history\":[{\"at\":\"2026-09-03T00:00:00Z\",\
             \"direction\":\"narrow\",\"decided_by\":\"Mike\",\
             \"incident_ref\":\"INC-7\"}]}",
        ))
        .expect("parses");
        let rendered = registry.render_being_facing();
        assert!(rendered.contains("Conversation:"));
        assert!(rendered.contains("aperture: 0 ..= 1, lease up to 1200s [evidence gathering]"));
        assert!(rendered.contains("operator_env_ceiling:"));
        assert!(rendered.contains("[granted]"));
        assert!(rendered.contains("ENVELOPE_ZERO <family>"));
        assert!(rendered.contains("narrow, never widen"));
        // The conformance receipt points her at "the ratchet history names
        // why" — the render must actually answer that.
        assert!(
            rendered
                .contains("last ratchet: narrow, 2026-09-03T00:00:00Z, by Mike, incident INC-7")
        );
    }

    #[test]
    fn lease_max_reads_policy_and_takes_the_strictest_across_fields() {
        let registry = parse_registry(&fixture(
            "\"aperture\":{\"floor\":0.0,\"ceiling\":1.0,\
             \"durability_policy\":{\"lease_max_secs\":1200,\"standing\":\"allowed\"}},\
             \"conversation_temperature\":{\"floor\":0.1,\"ceiling\":1.5,\
             \"durability_policy\":{\"lease_max_secs\":600}},\
             \"no_policy_field\":{\"floor\":0.0,\"ceiling\":1.0}",
        ))
        .expect("parses");
        assert_eq!(registry.lease_max_secs("aperture"), Some(1200));
        assert_eq!(registry.lease_max_secs("no_policy_field"), None);
        assert_eq!(registry.lease_max_secs("uncovered"), None);
        assert_eq!(
            registry.strictest_lease_max_secs(["aperture", "conversation_temperature"]),
            Some(600)
        );
        assert_eq!(
            registry.strictest_lease_max_secs(Vec::<String>::new()),
            None
        );
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
