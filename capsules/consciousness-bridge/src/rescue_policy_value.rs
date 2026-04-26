use serde_json::Value;

pub(super) fn bool_field(value: &Value, field: &str) -> Option<bool> {
    value.get(field).and_then(Value::as_bool)
}

pub(super) fn string_field(value: &Value, field: &str) -> Option<String> {
    value.get(field)?.as_str().map(ToString::to_string)
}

pub(super) fn u64_field(value: &Value, field: &str) -> Option<u64> {
    value.get(field).and_then(Value::as_u64)
}

pub(super) fn f32_field(value: &Value, field: &str) -> Option<f32> {
    value.get(field).and_then(Value::as_f64).map(|v| v as f32)
}

pub(super) fn string_array_field(value: &Value, field: &str) -> Option<Vec<String>> {
    let values = value.get(field)?.as_array()?;
    Some(
        values
            .iter()
            .filter_map(Value::as_str)
            .map(ToString::to_string)
            .collect(),
    )
}
