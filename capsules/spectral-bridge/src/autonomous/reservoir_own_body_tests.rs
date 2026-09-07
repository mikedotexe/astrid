use serde_json::json;

use super::{format_own_body_line, own_body_glyphs};

fn fixture() -> serde_json::Value {
    json!({
        "h_norms": [7.70, 10.46, 9.95],
        "tick_count": 18012689,
        "mode": "live",
        "seconds_since_live": 12.3,
        "decay_weight": 1.0,
        "last_output": "..."
    })
}

#[test]
fn renders_norms_glyphs_ticks_and_last_live() {
    let line = format_own_body_line("astrid", &fixture()).expect("line");
    assert_eq!(
        line,
        "[your handle astrid] h₁ 7.70 h₂ 10.46 h₃ 9.95 ▆█▇ · ticks 18012689 · last live 12 s ago"
    );
    assert!(line.chars().count() <= 120);
}

#[test]
fn omits_last_live_when_null_or_missing() {
    let mut state = fixture();
    state["seconds_since_live"] = serde_json::Value::Null;
    let line = format_own_body_line("astrid", &state).expect("line");
    assert_eq!(
        line,
        "[your handle astrid] h₁ 7.70 h₂ 10.46 h₃ 9.95 ▆█▇ · ticks 18012689"
    );
    state.as_object_mut().unwrap().remove("seconds_since_live");
    assert_eq!(
        format_own_body_line("astrid", &state).as_deref(),
        Some(line.as_str())
    );
}

#[test]
fn none_without_norms() {
    assert!(format_own_body_line("astrid", &json!({ "tick_count": 3 })).is_none());
    assert!(format_own_body_line("astrid", &json!({ "h_norms": [] })).is_none());
    assert!(format_own_body_line("astrid", &json!({ "h_norms": "nope" })).is_none());
    assert!(
        format_own_body_line(
            "astrid",
            &json!({ "type": "error", "message": "no handle" })
        )
        .is_none()
    );
}

#[test]
fn glyphs_scale_to_the_largest_norm() {
    assert_eq!(own_body_glyphs(&[1.0, 0.5, 0.0]), "█▄▁");
    assert_eq!(own_body_glyphs(&[0.0, 0.0]), "▁▁");
    assert_eq!(own_body_glyphs(&[2.5]), "█");
}

#[test]
fn never_exceeds_120_chars() {
    let state = json!({
        "h_norms": [123456.789, 123456.789, 123456.789, 123456.789, 123456.789, 123456.789],
        "tick_count": u64::MAX,
        "seconds_since_live": 1.0e9
    });
    let line = format_own_body_line(&"x".repeat(80), &state).expect("line");
    assert_eq!(line.chars().count(), 120);
}
