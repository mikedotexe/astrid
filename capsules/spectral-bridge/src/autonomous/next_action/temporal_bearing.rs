use std::{
    fs,
    path::{Component, Path, PathBuf},
    time::{SystemTime, UNIX_EPOCH},
};

use serde_json::Value;

use super::{ConversationState, NextActionContext, bridge_paths, strip_action};

const ACTION: &str = "TEMPORAL_BEARING";
const STATUS_MAX_BYTES: u64 = 512 * 1024;
const INDEX_MAX_BYTES: u64 = 16 * 1024 * 1024;
const RECORD_MAX_BYTES: u64 = 4 * 1024 * 1024;

#[derive(Debug, Clone, PartialEq, Eq)]
enum TemporalSelector {
    Latest,
    Contact(String),
    Journey(String),
    Passage(String),
}

impl TemporalSelector {
    fn parse(raw: &str) -> Result<Self, String> {
        let trimmed = raw.trim();
        if trimmed.is_empty() || trimmed.eq_ignore_ascii_case("latest") {
            return Ok(Self::Latest);
        }
        for (prefix, constructor) in [
            ("contact:", Self::Contact as fn(String) -> Self),
            ("journey:", Self::Journey as fn(String) -> Self),
            ("passage:", Self::Passage as fn(String) -> Self),
        ] {
            if let Some(value) = trimmed.strip_prefix(prefix) {
                let value = value.trim();
                if valid_identity(value) {
                    return Ok(constructor(value.to_string()));
                }
                return Err(format!(
                    "Temporal Bearing selector `{trimmed}` has an invalid bounded identity."
                ));
            }
        }
        Err(format!(
            "Temporal Bearing selector `{trimmed}` is unsupported. Use latest, contact:<id>, journey:<id>, or passage:<id>."
        ))
    }

    fn render(&self) -> String {
        match self {
            Self::Latest => "latest".to_string(),
            Self::Contact(value) => format!("contact:{value}"),
            Self::Journey(value) => format!("journey:{value}"),
            Self::Passage(value) => format!("passage:{value}"),
        }
    }
}

fn valid_identity(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 240
        && value
            .chars()
            .all(|character| character.is_ascii_alphanumeric() || "_.:-".contains(character))
}

fn unix_now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis()
        .try_into()
        .unwrap_or(u64::MAX)
}

fn read_json_bounded(path: &Path, maximum: u64) -> Result<Value, String> {
    let metadata = fs::metadata(path).map_err(|error| {
        format!(
            "Temporal Bearing evidence missing at {}: {error}",
            path.display()
        )
    })?;
    if metadata.len() > maximum {
        return Err(format!(
            "Temporal Bearing evidence at {} exceeds its read-only size bound.",
            path.display()
        ));
    }
    let text = fs::read_to_string(path).map_err(|error| {
        format!(
            "Temporal Bearing evidence unreadable at {}: {error}",
            path.display()
        )
    })?;
    serde_json::from_str(&text).map_err(|error| {
        format!(
            "Temporal Bearing evidence invalid at {}: {error}",
            path.display()
        )
    })
}

fn safe_record_path(root: &Path, relative: &str) -> Result<PathBuf, String> {
    let path = Path::new(relative);
    if path.is_absolute()
        || path
            .components()
            .any(|component| !matches!(component, Component::Normal(_)))
        || path.extension().and_then(|value| value.to_str()) != Some("json")
    {
        return Err("Temporal Bearing index contains an unsafe record path.".to_string());
    }
    Ok(root.join(path))
}

fn record_for_journey(root: &Path, indexes: &Value, journey_id: &str) -> Result<Value, String> {
    let relative = indexes
        .get("journeys")
        .and_then(|value| value.get(journey_id))
        .and_then(Value::as_str)
        .ok_or_else(|| {
            format!("No generated Temporal Bearing record exists for journey `{journey_id}`.")
        })?;
    let path = safe_record_path(root, relative)?;
    let record = read_json_bounded(&path, RECORD_MAX_BYTES)?;
    if record.get("schema").and_then(Value::as_str) != Some("temporal_bearing_record_v1")
        || record.get("journey_id").and_then(Value::as_str) != Some(journey_id)
    {
        return Err(format!(
            "Temporal Bearing record `{journey_id}` failed exact identity validation."
        ));
    }
    Ok(record)
}

fn latest_for_ids(root: &Path, indexes: &Value, ids: &[Value]) -> Result<Value, String> {
    let mut records = ids
        .iter()
        .filter_map(Value::as_str)
        .map(|journey_id| record_for_journey(root, indexes, journey_id))
        .collect::<Result<Vec<_>, _>>()?;
    records.sort_by(|left, right| {
        let left_time = left
            .get("latest_machine_time_unix_ms")
            .and_then(Value::as_u64)
            .unwrap_or(0);
        let right_time = right
            .get("latest_machine_time_unix_ms")
            .and_then(Value::as_u64)
            .unwrap_or(0);
        left_time.cmp(&right_time).then_with(|| {
            left.get("journey_id")
                .and_then(Value::as_str)
                .cmp(&right.get("journey_id").and_then(Value::as_str))
        })
    });
    records
        .pop()
        .ok_or_else(|| "Temporal Bearing index names no generated journey records.".to_string())
}

fn select_record(
    root: &Path,
    indexes: &Value,
    selector: &TemporalSelector,
) -> Result<Value, String> {
    match selector {
        TemporalSelector::Latest => {
            let latest = read_json_bounded(&root.join("latest.json"), RECORD_MAX_BYTES)?;
            latest
                .get("record")
                .filter(|value| value.is_object())
                .cloned()
                .ok_or_else(|| "Temporal Bearing has no journey evidence yet.".to_string())
        },
        TemporalSelector::Journey(journey_id) => record_for_journey(root, indexes, journey_id),
        TemporalSelector::Contact(contact_id) => {
            let ids = indexes
                .get("contacts")
                .and_then(|value| value.get(contact_id))
                .and_then(Value::as_array)
                .ok_or_else(|| {
                    format!(
                        "No generated Temporal Bearing record exists for contact `{contact_id}`."
                    )
                })?;
            latest_for_ids(root, indexes, ids)
        },
        TemporalSelector::Passage(passage_id) => {
            let ids = indexes
                .get("passages")
                .and_then(|value| value.get(passage_id))
                .and_then(Value::as_array)
                .ok_or_else(|| {
                    format!("No exactly linked Temporal Bearing record exists for passage `{passage_id}`.")
                })?;
            latest_for_ids(root, indexes, ids)
        },
    }
}

fn array_len(value: Option<&Value>) -> usize {
    value.and_then(Value::as_array).map_or(0, Vec::len)
}

fn render_record(
    record: &Value,
    selector: &TemporalSelector,
    stale: bool,
) -> Result<String, String> {
    let journey_id = record
        .get("journey_id")
        .and_then(Value::as_str)
        .ok_or_else(|| "Temporal Bearing record has no journey identity.".to_string())?;
    let machine = record
        .get("machine_rail")
        .and_then(Value::as_object)
        .ok_or_else(|| "Temporal Bearing machine rail is missing.".to_string())?;
    let owner = record
        .get("owner_rail")
        .and_then(Value::as_object)
        .ok_or_else(|| "Temporal Bearing owner rail is missing.".to_string())?;
    let ingress = machine.get("ingress").and_then(Value::as_object);
    let first = machine.get("first_response").and_then(Value::as_object);
    let clock = machine.get("clock_validity").and_then(Value::as_object);
    let signal = machine.get("signal_spine").and_then(Value::as_object);
    let contact = record
        .get("contact_id")
        .and_then(Value::as_str)
        .unwrap_or("(none; origin is non-contact or ingress evidence is unavailable)");
    let passages = record
        .get("passage_ids")
        .and_then(Value::as_array)
        .map(|values| {
            values
                .iter()
                .filter_map(Value::as_str)
                .collect::<Vec<_>>()
                .join(", ")
        })
        .filter(|value| !value.is_empty())
        .unwrap_or_else(|| "(none exactly linked)".to_string());
    let response_state = first
        .and_then(|value| value.get("stage_time_unix_ms"))
        .and_then(Value::as_u64)
        .map_or_else(
            || "missing".to_string(),
            |value| format!("recorded at {value}"),
        );
    let projection_state = if stale {
        "STALE: regenerate the source-first projection before treating this as current"
    } else {
        "current within the generated projection freshness window"
    };
    Ok(format!(
        "=== TEMPORAL BEARING V1 ===\n\
         Selector: {}\n\
         Projection: {projection_state}\n\
         Journey: {journey_id}\n\
         Contact: {contact}\n\
         Phase Passage: {passages}\n\
         Origin: {} | response origin: {}\n\
         \n\
         MACHINE RAIL\n\
         Ingress: {}\n\
         First response: {response_state}\n\
         Terminal: {}\n\
         Clock: {}\n\
         Signal Spine stages: {} (exact journey artifact hash {})\n\
         Exact Shadow history artifacts: {}\n\
         Exact Texture snapshots: {}\n\
         \n\
         OWNER RAIL\n\
         Lived-state witnesses: {} ({})\n\
         Phase Passages: {} ({})\n\
         Felt weight: unscored; felt status remains owner-authored only\n\
         \n\
         Authority: read-only generated evidence. No timestamp-proximity edge, passage or anchor creation, closure inference, temporal decay, dispatch, scheduler, model, pressure, fill, PI, codec, reservoir, or control authority.",
        selector.render(),
        record
            .get("journey_origin_v1")
            .and_then(|value| value.get("kind"))
            .and_then(Value::as_str)
            .unwrap_or("legacy_unknown"),
        record
            .get("response_origin_v1")
            .and_then(Value::as_str)
            .unwrap_or("legacy_unknown"),
        ingress
            .and_then(|value| value.get("kind"))
            .and_then(Value::as_str)
            .unwrap_or("missing"),
        machine
            .get("terminal_state")
            .and_then(Value::as_str)
            .unwrap_or("missing"),
        clock
            .and_then(|value| value.get("state"))
            .and_then(Value::as_str)
            .unwrap_or("missing"),
        signal
            .and_then(|value| value.get("stage_count"))
            .and_then(Value::as_u64)
            .unwrap_or(0),
        signal
            .and_then(|value| value.get("journey_artifact_sha256"))
            .and_then(Value::as_str)
            .unwrap_or("missing"),
        array_len(machine.get("shadow_history_evidence")),
        array_len(machine.get("texture_dynamics_snapshots")),
        array_len(owner.get("lived_state_witnesses")),
        owner
            .get("lived_state_witness_state")
            .and_then(Value::as_str)
            .unwrap_or("owner_unreported_for_exact_lineage"),
        array_len(owner.get("phase_passages")),
        owner
            .get("phase_passage_state")
            .and_then(Value::as_str)
            .unwrap_or("owner_unreported_for_exact_lineage"),
    ))
}

fn render_at_root(root: &Path, selector: &TemporalSelector, now_ms: u64) -> Result<String, String> {
    let status = read_json_bounded(&root.join("status.json"), STATUS_MAX_BYTES)?;
    if status.get("schema").and_then(Value::as_str) != Some("temporal_bearing_status_v1")
        || status.get("valid").and_then(Value::as_bool) != Some(true)
    {
        return Err("Temporal Bearing status is missing or invalid; regenerate the source-first projection.".to_string());
    }
    let indexes = read_json_bounded(&root.join("indexes.json"), INDEX_MAX_BYTES)?;
    if indexes.get("schema").and_then(Value::as_str) != Some("temporal_bearing_indexes_v1")
        || indexes
            .get("timestamp_proximity_indexed")
            .and_then(Value::as_bool)
            != Some(false)
    {
        return Err("Temporal Bearing exact-identity index failed validation.".to_string());
    }
    let stale = status
        .get("stale_after_unix_ms")
        .and_then(Value::as_u64)
        .is_none_or(|deadline| now_ms > deadline);
    let record = select_record(root, &indexes, selector)?;
    render_record(&record, selector, stale)
}

pub(super) fn handle_action(
    conv: &mut ConversationState,
    base_action: &str,
    original: &str,
    _ctx: &mut NextActionContext<'_>,
) -> bool {
    if base_action != ACTION {
        return false;
    }
    let raw = strip_action(original, ACTION);
    let rendered = TemporalSelector::parse(&raw).and_then(|selector| {
        let root = bridge_paths()
            .bridge_workspace()
            .join("diagnostics/temporal_bearing_v1");
        render_at_root(&root, &selector, unix_now_ms())
    });
    conv.emphasis = Some(rendered.unwrap_or_else(|error| {
        format!(
            "=== TEMPORAL BEARING V1 ===\nUnavailable: {error}\nAuthority: read-only generated evidence; no source file was written and no control authority was granted."
        )
    }));
    true
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn write_json(path: &Path, value: &Value) {
        fs::create_dir_all(path.parent().expect("parent")).expect("mkdir");
        fs::write(path, serde_json::to_vec_pretty(value).expect("json")).expect("write");
    }

    fn fixture() -> (tempfile::TempDir, String) {
        let temp = tempfile::tempdir().expect("tempdir");
        let root = temp.path();
        let journey_id = format!("journey_{}", "1".repeat(24));
        let record = json!({
            "schema": "temporal_bearing_record_v1",
            "journey_id": journey_id,
            "contact_id": null,
            "passage_ids": [],
            "journey_origin_v1": {"kind": "autonomous"},
            "response_origin_v1": "model_authored",
            "latest_machine_time_unix_ms": 1000,
            "machine_rail": {
                "ingress": {"kind": "autonomous"},
                "first_response": {"stage_time_unix_ms": 900},
                "terminal_state": "delivered",
                "clock_validity": {"state": "not_applicable_without_contact_root"},
                "signal_spine": {
                    "stage_count": 3,
                    "journey_artifact_sha256": "a".repeat(64),
                },
                "shadow_history_evidence": [],
                "texture_dynamics_snapshots": [],
            },
            "owner_rail": {
                "lived_state_witness_state": "owner_unreported_for_exact_lineage",
                "lived_state_witnesses": [],
                "phase_passage_state": "owner_unreported_for_exact_lineage",
                "phase_passages": [],
            },
        });
        write_json(&root.join(format!("by_journey/{journey_id}.json")), &record);
        write_json(
            &root.join("latest.json"),
            &json!({"schema": "temporal_bearing_latest_v1", "record": record}),
        );
        write_json(
            &root.join("indexes.json"),
            &json!({
                "schema": "temporal_bearing_indexes_v1",
                "journeys": {journey_id.clone(): format!("by_journey/{journey_id}.json")},
                "contacts": {},
                "passages": {},
                "timestamp_proximity_indexed": false,
            }),
        );
        write_json(
            &root.join("status.json"),
            &json!({
                "schema": "temporal_bearing_status_v1",
                "valid": true,
                "stale_after_unix_ms": 2000,
            }),
        );
        (temp, journey_id)
    }

    fn file_snapshot(root: &Path) -> Vec<(PathBuf, Vec<u8>)> {
        let mut files = Vec::new();
        let mut pending = vec![root.to_path_buf()];
        while let Some(directory) = pending.pop() {
            for entry in fs::read_dir(directory).expect("read directory") {
                let entry = entry.expect("entry");
                let path = entry.path();
                if entry.file_type().expect("file type").is_dir() {
                    pending.push(path);
                } else {
                    files.push((
                        path.strip_prefix(root).expect("relative").to_path_buf(),
                        fs::read(path).expect("read"),
                    ));
                }
            }
        }
        files.sort_by(|left, right| left.0.cmp(&right.0));
        files
    }

    #[test]
    fn latest_action_reads_projection_without_writing() {
        let (temp, journey_id) = fixture();
        let before = file_snapshot(temp.path());
        let rendered =
            render_at_root(temp.path(), &TemporalSelector::Latest, 1500).expect("render current");
        let after = file_snapshot(temp.path());
        assert_eq!(before, after);
        assert!(rendered.contains(&journey_id));
        assert!(rendered.contains("owner_unreported_for_exact_lineage"));
        assert!(rendered.contains("No timestamp-proximity edge"));
    }

    #[test]
    fn stale_projection_is_reported_plainly() {
        let (temp, _) = fixture();
        let rendered =
            render_at_root(temp.path(), &TemporalSelector::Latest, 2001).expect("render stale");
        assert!(rendered.contains("STALE"));
    }

    #[test]
    fn selector_rejects_path_traversal_and_missing_exact_evidence() {
        assert!(TemporalSelector::parse("journey:../../private").is_err());
        let (temp, _) = fixture();
        let missing = TemporalSelector::Journey(format!("journey_{}", "f".repeat(24)));
        let error = render_at_root(temp.path(), &missing, 1500).expect_err("missing");
        assert!(error.contains("No generated Temporal Bearing record"));
    }
}
