//! Facts about retained records, never a score for recovering a thought.
use serde_json::{Value, json};

pub(super) fn project(
    reference: Option<&str>,
    thread_next: Option<&str>,
    record: Option<&Value>,
    guarded_return_kind: Option<&str>,
) -> Value {
    let reference = reference.filter(|id| !id.trim().is_empty());
    let record = record.filter(|record| {
        reference.is_some() && record.get("experiment_id").and_then(Value::as_str) == reference
    });
    let (record_status, next, scope) = if let Some(record) = record {
        (
            "verified_readable",
            Some(record.get("planned_next").and_then(Value::as_str)),
            "experiment",
        )
    } else if reference.is_some() {
        ("unavailable", None, "unavailable")
    } else {
        ("absent", Some(thread_next), "thread")
    };
    json!({
        "schema_version": 1,
        "experiment_reference": reference,
        "record_status": record_status,
        "saved_next_action": match next {
            None => "unknown",
            Some(Some(text)) if !text.trim().is_empty() => "present",
            _ => "absent",
        },
        "saved_next_action_scope": scope,
        "guarded_return_kind": record.and(guarded_return_kind),
        "execution_eligibility": "not_evaluated",
    })
}

pub(super) fn render(snapshot: &Value) -> String {
    if let Some(support) = snapshot.get("return_support_v1") {
        return format!(
            "Saved return-support snapshot: record={}; saved next={} ({}); paused return={}. Facts at capture, not a live eligibility check or evidence of recovered understanding.",
            support["record_status"].as_str().unwrap_or("unavailable"),
            support["saved_next_action"].as_str().unwrap_or("unknown"),
            support["saved_next_action_scope"]
                .as_str()
                .unwrap_or("unavailable"),
            support["guarded_return_kind"]
                .as_str()
                .unwrap_or("not established"),
        );
    }
    if snapshot.get("returnability").is_some() {
        return "Historical returnability: retired heuristic, not measured recovery capability. Current saved-return support unavailable.".into();
    }
    "Saved-return support unavailable; execution eligibility not evaluated.".into()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn paired_contract_fixtures() {
        let cases: Value = serde_json::from_str(include_str!(
            "../../../tests/fixtures/return_support_v1.json"
        ))
        .unwrap();
        for case in cases.as_array().unwrap() {
            assert_eq!(
                project(
                    case["reference"].as_str(),
                    case["thread_next"].as_str(),
                    case.get("record").filter(|r| !r.is_null()),
                    case["guarded_return_kind"].as_str(),
                ),
                case["expected"],
                "{}",
                case["name"],
            );
        }
    }

    #[test]
    fn legacy_numeric_score_is_not_rendered() {
        let original = json!({"returnability": 0.78});
        let text = render(&original);
        assert!(!text.contains("0.78"));
        assert!(text.contains("retired heuristic"));
        assert_eq!(original["returnability"], 0.78);
    }
}
