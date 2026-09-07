//! Read-only learning reference from a fresh, clock-matched controller snapshot.

use std::io::Read as _;
use std::path::Path;
use std::time::{Duration, SystemTime};

use serde::Serialize;
use serde_json::Value;

use crate::learning_clock::LearningObservation;

const MAX_HEALTH_BYTES: u64 = 262_144;
const MAX_HEALTH_AGE: Duration = Duration::from_secs(10);
const MAX_PRODUCER_SKEW_MS: f64 = 5_000.0;

#[derive(Clone, Copy, Debug, PartialEq, Serialize)]
pub(crate) struct LearningTarget {
    pub fill_pct: f32,
    pub source: &'static str,
}

fn target_from_health(health: &Value, producer_t_ms: u64) -> Option<LearningTarget> {
    let health_t_ms = health.get("t_s")?.as_f64()? * 1000.0;
    if !health_t_ms.is_finite()
        || health_t_ms < 0.0
        || (health_t_ms - producer_t_ms as f64).abs() > MAX_PRODUCER_SKEW_MS
    {
        return None;
    }
    let (value, source) = match health.pointer("/stable_core/enabled")?.as_bool()? {
        true => {
            let pi = health.pointer("/stable_core/structural_pi")?;
            if !pi.get("active")?.as_bool()? {
                return None;
            }
            (
                pi.get("target_fill_pct")?,
                "health.stable_core.structural_pi.target_fill_pct",
            )
        },
        false => (health.pointer("/pi/target_fill")?, "health.pi.target_fill"),
    };
    let fill = value.as_f64()?;
    (fill.is_finite() && (0.0..=100.0).contains(&fill)).then_some(LearningTarget {
        fill_pct: fill as f32,
        source,
    })
}

fn read_target(workspace: &Path, producer_t_ms: u64, now: SystemTime) -> Option<LearningTarget> {
    let file = std::fs::File::open(workspace.join("health.json")).ok()?;
    let before = file.metadata().ok()?;
    let modified = before.modified().ok()?;
    if !before.is_file()
        || before.len() > MAX_HEALTH_BYTES
        || now.duration_since(modified).ok()? > MAX_HEALTH_AGE
    {
        return None;
    }
    let mut bytes = Vec::new();
    (&file)
        .take(MAX_HEALTH_BYTES + 1)
        .read_to_end(&mut bytes)
        .ok()?;
    let after = file.metadata().ok()?;
    if bytes.len() as u64 > MAX_HEALTH_BYTES
        || before.len() != after.len()
        || modified != after.modified().ok()?
    {
        return None;
    }
    target_from_health(
        &serde_json::from_slice::<Value>(&bytes).ok()?,
        producer_t_ms,
    )
}

pub(crate) fn attach(
    sample: Option<LearningObservation>,
    workspace: Option<&Path>,
) -> Option<LearningObservation> {
    sample.map(|mut sample| {
        sample.target =
            workspace.and_then(|path| read_target(path, sample.producer_t_ms, SystemTime::now()));
        sample
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn health() -> Value {
        serde_json::json!({"t_s":100.0,"pi":{"target_fill":50.0},
            "stable_core":{"enabled":true,"structural_pi":{"active":true,"target_fill_pct":68.0}}})
    }

    #[test]
    fn active_structural_target_takes_precedence_without_rewriting_it() {
        let mut value = health();
        assert_eq!(target_from_health(&value, 100_000).unwrap().fill_pct, 68.0);
        value["stable_core"]["structural_pi"]["target_fill_pct"] = 63.0.into();
        assert_eq!(target_from_health(&value, 100_000).unwrap().fill_pct, 63.0);
        value["stable_core"]["enabled"] = false.into();
        assert_eq!(target_from_health(&value, 100_000).unwrap().fill_pct, 50.0);
    }

    #[test]
    fn absent_invalid_inactive_or_unmatched_reference_never_defaults() {
        assert!(target_from_health(&health(), 529_076_440).is_none());
        assert!(target_from_health(&serde_json::json!({}), 100_000).is_none());
        for invalid in [
            serde_json::Value::Null,
            serde_json::json!("68"),
            serde_json::json!(101),
            serde_json::json!(-1),
        ] {
            let mut value = health();
            value["stable_core"]["structural_pi"]["target_fill_pct"] = invalid;
            assert!(target_from_health(&value, 100_000).is_none());
        }
        let mut value = health();
        value["stable_core"]["structural_pi"]["active"] = false.into();
        assert!(target_from_health(&value, 100_000).is_none());
    }

    #[test]
    fn file_reference_requires_fresh_bounded_valid_bytes() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("health.json");
        assert!(read_target(dir.path(), 100_000, SystemTime::now()).is_none());
        std::fs::write(&path, health().to_string()).unwrap();
        let modified = path.metadata().unwrap().modified().unwrap();
        assert_eq!(
            read_target(dir.path(), 100_000, modified).unwrap().fill_pct,
            68.0
        );
        assert!(read_target(dir.path(), 100_000, modified + Duration::from_secs(11)).is_none());
        assert!(read_target(dir.path(), 100_000, modified - Duration::from_secs(1)).is_none());
        std::fs::write(&path, "{").unwrap();
        assert!(read_target(dir.path(), 100_000, SystemTime::now()).is_none());
        std::fs::write(&path, vec![b' '; MAX_HEALTH_BYTES as usize + 1]).unwrap();
        assert!(read_target(dir.path(), 100_000, SystemTime::now()).is_none());
    }
}
