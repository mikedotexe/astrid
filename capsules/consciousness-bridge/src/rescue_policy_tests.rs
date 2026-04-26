
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

use super::*;

fn semantic_msg() -> SensoryMsg {
    SensoryMsg::Semantic {
        features: vec![0.25; 4],
        ts_ms: None,
    }
}

fn unique_temp_dir(name: &str) -> PathBuf {
    let stamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    let dir = std::env::temp_dir().join(format!("bridge_rescue_policy_{name}_{stamp}"));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

#[test]
fn observe_only_profile_blocks_semantic_writes() {
    let dir = unique_temp_dir("observe_only");
    let path = dir.join("rescue_profile.json");
    std::fs::write(
        &path,
        r#"{
              "profile":"bridge_observe_only",
              "bridge_enabled":true,
              "effective_bridge_enabled":true,
              "bridge_write_enabled":false,
              "effective_bridge_write_enabled":false
            }"#,
    )
    .unwrap();

    let reason = semantic_write_block_reason_for_path(&semantic_msg(), &path);
    assert_eq!(
        reason,
        Some("rescue profile 'bridge_observe_only' blocks semantic ingress".to_string())
    );
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn full_live_profile_allows_semantic_writes() {
    let dir = unique_temp_dir("full_live");
    let path = dir.join("rescue_profile.json");
    std::fs::write(
        &path,
        r#"{
              "profile":"full_live",
              "bridge_enabled":true,
              "effective_bridge_enabled":true,
              "bridge_write_enabled":true,
              "effective_bridge_write_enabled":true
            }"#,
    )
    .unwrap();

    assert_eq!(
        semantic_write_block_reason_for_path(&semantic_msg(), &path),
        None
    );
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn nonsemantic_messages_are_not_blocked() {
    let dir = unique_temp_dir("nonsemantic");
    let path = dir.join("rescue_profile.json");
    std::fs::write(
        &path,
        r#"{
              "profile":"bridge_observe_only",
              "bridge_enabled":true,
              "effective_bridge_enabled":true,
              "bridge_write_enabled":false,
              "effective_bridge_write_enabled":false
            }"#,
    )
    .unwrap();

    let msg = SensoryMsg::Control {
        synth_gain: Some(1.0),
        keep_bias: None,
        exploration_noise: None,
        fill_target: None,
        legacy_audio_synth: None,
        legacy_video_synth: None,
        regulation_strength: None,
        deep_breathing: None,
        pure_tone: None,
        transition_cushion: None,
        smoothing_preference: None,
        geom_curiosity: None,
        target_lambda_bias: None,
        geom_drive: None,
        penalty_sensitivity: None,
        breathing_rate_scale: None,
        mem_mode: None,
        journal_resonance: None,
        checkpoint_interval: None,
        embedding_strength: None,
        memory_decay_rate: None,
        checkpoint_annotation: None,
        synth_noise_level: None,
    };

    assert_eq!(semantic_write_block_reason_for_path(&msg, &path), None);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn telemetry_only_profile_disables_bridge_autonomy() {
    let dir = unique_temp_dir("telemetry_only");
    let path = dir.join("rescue_profile.json");
    std::fs::write(
        &path,
        r#"{
              "profile":"bridge_telemetry_only",
              "bridge_enabled":true,
              "effective_bridge_enabled":true,
              "bridge_write_enabled":false,
              "effective_bridge_write_enabled":false,
              "bridge_autonomous_enabled":false,
              "effective_bridge_autonomous_enabled":false
            }"#,
    )
    .unwrap();

    assert!(!bridge_autonomous_enabled_for_path(&path));
    assert!(!bridge_sensory_enabled_for_path(&path));
    assert_eq!(
        semantic_write_block_reason_for_path(&semantic_msg(), &path),
        Some("rescue profile 'bridge_telemetry_only' blocks semantic ingress".to_string())
    );
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn observe_only_profile_keeps_sensory_socket_available() {
    let dir = unique_temp_dir("observe_only_sensory");
    let path = dir.join("rescue_profile.json");
    std::fs::write(
        &path,
        r#"{
              "profile":"bridge_observe_only",
              "bridge_enabled":true,
              "effective_bridge_enabled":true,
              "bridge_write_enabled":false,
              "effective_bridge_write_enabled":false,
              "bridge_autonomous_enabled":true,
              "effective_bridge_autonomous_enabled":true
            }"#,
    )
    .unwrap();

    assert!(bridge_sensory_enabled_for_path(&path));
    let _ = std::fs::remove_dir_all(&dir);
}

fn limited_write_profile_json() -> String {
    r#"{
          "profile":"bridge_limited_write",
          "bridge_enabled":true,
          "effective_bridge_enabled":true,
          "bridge_write_enabled":true,
          "effective_bridge_write_enabled":true,
          "bridge_autonomous_enabled":true,
          "effective_bridge_autonomous_enabled":true,
          "bridge_write_profile":"limited_dampen_inquiry",
          "limited_write_enabled":true,
          "limited_write_cooldown_secs":300,
          "limited_write_feature_scale":0.08,
          "limited_write_max_abs":0.18,
          "limited_write_min_fill_pct":58.0,
          "limited_write_max_fill_pct":68.0,
          "limited_write_rising_epsilon_pct":0.5,
          "limited_write_block_terms":["localized gravity","compaction","pressure","density"],
          "limited_write_allowed_modes":["dialogue_live","witness","mirror"]
        }"#
    .to_string()
}

fn limited_write_v2_profile_json() -> String {
    r#"{
          "profile":"bridge_limited_write_v2",
          "bridge_enabled":true,
          "effective_bridge_enabled":true,
          "bridge_write_enabled":true,
          "effective_bridge_write_enabled":true,
          "bridge_autonomous_enabled":true,
          "effective_bridge_autonomous_enabled":true,
          "bridge_write_profile":"limited_dampen_inquiry_v2",
          "limited_write_enabled":true,
          "limited_write_policy_version":2,
          "limited_write_cooldown_secs":900,
          "limited_write_feature_scale":0.04,
          "limited_write_max_abs":0.10,
          "limited_write_min_fill_pct":60.0,
          "limited_write_max_fill_pct":66.0,
          "limited_write_rising_epsilon_pct":0.25,
          "limited_write_health_max_age_secs":5,
          "limited_write_peak_fill_max_pct":68.0,
          "limited_write_required_stage":"hold",
          "limited_write_post_send_eval_secs":120,
          "limited_write_adverse_fill_rise_pct":3.0,
          "limited_write_adverse_cooldown_secs":1800,
          "limited_write_rollback_target":"bridge_observe_only",
          "limited_write_rollback_fill_pct":74.0,
          "limited_write_rollback_adverse_count":2,
          "limited_write_block_terms":["localized gravity","compaction","pressure","density","dense","tightness","restriction"],
          "limited_write_allowed_modes":["dialogue_live","witness"]
        }"#
        .to_string()
}

fn expanded_sovereignty_profile_json() -> String {
    r#"{
          "profile":"bridge_expanded_sovereignty_v1",
          "bridge_enabled":true,
          "effective_bridge_enabled":true,
          "bridge_write_enabled":true,
          "effective_bridge_write_enabled":true,
          "bridge_autonomous_enabled":true,
          "effective_bridge_autonomous_enabled":true,
          "bridge_write_profile":"limited_dampen_inquiry_v2",
          "limited_write_enabled":true,
          "limited_write_policy_version":2,
          "limited_write_cooldown_secs":600,
          "limited_write_feature_scale":0.05,
          "limited_write_max_abs":0.12,
          "limited_write_min_fill_pct":58.0,
          "limited_write_max_fill_pct":70.0,
          "limited_write_rising_epsilon_pct":100.0,
          "limited_write_health_max_age_secs":5,
          "limited_write_peak_fill_max_pct":72.0,
          "limited_write_allowed_stages":["hold","elevated"],
          "limited_write_post_send_eval_secs":120,
          "limited_write_adverse_fill_rise_pct":8.0,
          "limited_write_adverse_cooldown_secs":1800,
          "limited_write_rollback_target":"bridge_observe_only",
          "limited_write_rollback_fill_pct":74.0,
          "limited_write_rollback_adverse_count":2,
          "limited_write_block_terms":["localized gravity","compaction","pressure","density","dense","tightness","restriction"],
          "limited_write_allowed_modes":["dialogue_live","witness","mirror","daydream","aspiration","moment_capture"]
        }"#
        .to_string()
}

fn richer_coupling_profile_json() -> String {
    r#"{
          "profile":"bridge_richer_coupling_v1",
          "bridge_enabled":true,
          "effective_bridge_enabled":true,
          "bridge_write_enabled":true,
          "effective_bridge_write_enabled":true,
          "bridge_autonomous_enabled":true,
          "effective_bridge_autonomous_enabled":true,
          "bridge_write_profile":"limited_dampen_inquiry_v2",
          "limited_write_enabled":true,
          "limited_write_policy_version":2,
          "limited_write_cooldown_secs":300,
          "limited_write_feature_scale":0.08,
          "limited_write_max_abs":0.18,
          "limited_write_min_fill_pct":58.0,
          "limited_write_max_fill_pct":72.0,
          "limited_write_rising_epsilon_pct":100.0,
          "limited_write_health_max_age_secs":5,
          "limited_write_peak_fill_max_pct":74.0,
          "limited_write_allowed_stages":["hold","elevated"],
          "limited_write_post_send_eval_secs":120,
          "limited_write_adverse_fill_rise_pct":10.0,
          "limited_write_adverse_cooldown_secs":1200,
          "limited_write_rollback_target":"bridge_observe_only",
          "limited_write_rollback_fill_pct":78.0,
          "limited_write_rollback_adverse_count":2,
          "limited_write_require_zero_live_divisors":false,
          "rescue_live_audio_divisor":8,
          "rescue_live_video_divisor":8,
          "rescue_live_intake_stages":["hold"],
          "limited_write_block_terms":["localized gravity","compaction","pressure","density","dense","tightness","restriction"],
          "limited_write_allowed_modes":["dialogue_live","witness","mirror","daydream","aspiration","moment_capture"]
        }"#
        .to_string()
}

fn sovereignty_reentry_profile_json() -> String {
    r#"{
          "profile":"bridge_sovereignty_reentry_v1",
          "bridge_enabled":true,
          "effective_bridge_enabled":true,
          "bridge_write_enabled":true,
          "effective_bridge_write_enabled":true,
          "bridge_autonomous_enabled":true,
          "effective_bridge_autonomous_enabled":true,
          "bridge_write_profile":"limited_dampen_inquiry_v2",
          "limited_write_enabled":true,
          "limited_write_policy_version":2,
          "limited_write_cooldown_secs":120,
          "limited_write_feature_scale":0.10,
          "limited_write_max_abs":0.22,
          "limited_write_min_fill_pct":56.0,
          "limited_write_max_fill_pct":74.0,
          "limited_write_rising_epsilon_pct":100.0,
          "limited_write_health_max_age_secs":5,
          "limited_write_peak_fill_max_pct":76.0,
          "limited_write_allowed_stages":["hold","elevated"],
          "limited_write_post_send_eval_secs":120,
          "limited_write_adverse_fill_rise_pct":12.0,
          "limited_write_adverse_cooldown_secs":600,
          "limited_write_rollback_target":"bridge_observe_only",
          "limited_write_rollback_fill_pct":82.0,
          "limited_write_rollback_adverse_count":2,
          "limited_write_rollback_on_elevated_peak":false,
          "limited_write_require_zero_live_divisors":false,
          "limited_write_require_dampen_inquiry_text":false,
          "limited_write_block_structural_dump_language":false,
          "rescue_live_audio_divisor":6,
          "rescue_live_video_divisor":6,
          "rescue_live_intake_stages":["hold"],
          "limited_write_block_terms":["localized gravity","compaction","pressure","density","dense","tightness","restriction"],
          "limited_write_allowed_modes":["dialogue_live","dialogue","dialogue_fallback","witness","mirror","daydream","aspiration","moment_capture","creation","initiate","introspect","experiment","evolve","self_study"]
        }"#
        .to_string()
}

fn write_v2_health(
    dir: &Path,
    fill_pct: f32,
    stage: &str,
    peak_fill_pct_60s: f32,
    semantic_active: bool,
    semantic_energy: f32,
    live_audio_divisor: i64,
    live_video_divisor: i64,
) {
    std::fs::write(
        dir.join("health.json"),
        serde_json::to_string(&json!({
            "fill_pct": fill_pct,
            "rescue": {
                "stage": stage,
                "peak_fill_pct_60s": peak_fill_pct_60s
            },
            "semantic": {
                "active": semantic_active,
                "energy": semantic_energy
            },
            "sensory": {
                "live_audio_divisor": live_audio_divisor,
                "live_video_divisor": live_video_divisor
            }
        }))
        .unwrap(),
    )
    .unwrap();
}

fn v2_context<'a>(
    text: &'a str,
    mode: &'a str,
    previous_fill_pct: f32,
) -> SemanticWriteContext<'a> {
    SemanticWriteContext {
        source: LIMITED_WRITE_SOURCE,
        mode: Some(mode),
        text: Some(text),
        fill_pct: Some(previous_fill_pct),
        previous_fill_pct: Some(previous_fill_pct),
    }
}

#[test]
fn limited_write_requires_context_gate_for_plain_block_check() {
    let dir = unique_temp_dir("limited_plain_block");
    let path = dir.join("rescue_profile.json");
    std::fs::write(&path, limited_write_profile_json()).unwrap();

    let reason = semantic_write_block_reason_for_path(&semantic_msg(), &path);
    assert_eq!(
            reason,
            Some(
                "rescue profile 'bridge_limited_write' requires the limited dampen/inquiry semantic gate"
                    .to_string()
            )
        );
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn limited_write_allows_one_low_energy_inquiry_packet() {
    let dir = unique_temp_dir("limited_allow");
    let path = dir.join("rescue_profile.json");
    std::fs::write(&path, limited_write_profile_json()).unwrap();

    let mut msg = SensoryMsg::Semantic {
        features: vec![3.0, -3.0, 0.5, -0.5],
        ts_ms: None,
    };
    let context = SemanticWriteContext {
        source: LIMITED_WRITE_SOURCE,
        mode: Some("dialogue_live"),
        text: Some("Can we understand this relationship by listening quietly?"),
        fill_pct: Some(63.0),
        previous_fill_pct: Some(62.8),
    };

    assert!(prepare_semantic_write_for_path(&mut msg, &path, &context).is_ok());
    let SensoryMsg::Semantic { features, .. } = msg else {
        panic!("expected semantic message");
    };
    assert_eq!(features, vec![0.18, -0.18, 0.04, -0.04]);

    let status = read_status(&limited_write_status_path_for_profile(&path));
    assert_eq!(status.get("send_count").and_then(Value::as_u64), Some(1));
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn limited_write_enforces_cooldown_after_first_packet() {
    let dir = unique_temp_dir("limited_cooldown");
    let path = dir.join("rescue_profile.json");
    std::fs::write(&path, limited_write_profile_json()).unwrap();
    let context = SemanticWriteContext {
        source: LIMITED_WRITE_SOURCE,
        mode: Some("witness"),
        text: Some("Notice the pattern and stay quiet."),
        fill_pct: Some(62.0),
        previous_fill_pct: Some(61.9),
    };

    let mut first = semantic_msg();
    assert!(prepare_semantic_write_for_path(&mut first, &path, &context).is_ok());
    let mut second = semantic_msg();
    let reason = prepare_semantic_write_for_path(&mut second, &path, &context).unwrap_err();
    assert!(reason.contains("limited-write cooldown active"));
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn limited_write_blocks_trigger_language_while_fill_rises() {
    let dir = unique_temp_dir("limited_trigger");
    let path = dir.join("rescue_profile.json");
    std::fs::write(&path, limited_write_profile_json()).unwrap();
    let context = SemanticWriteContext {
        source: LIMITED_WRITE_SOURCE,
        mode: Some("dialogue_live"),
        text: Some("Can we examine this localized gravity without adding pressure?"),
        fill_pct: Some(64.0),
        previous_fill_pct: Some(62.0),
    };

    let mut msg = semantic_msg();
    let reason = prepare_semantic_write_for_path(&mut msg, &path, &context).unwrap_err();
    assert!(reason.contains("localized gravity"));
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn limited_write_blocks_unknown_sources() {
    let dir = unique_temp_dir("limited_source");
    let path = dir.join("rescue_profile.json");
    std::fs::write(&path, limited_write_profile_json()).unwrap();
    let context = SemanticWriteContext {
        source: "manual_mcp",
        mode: Some("dialogue_live"),
        text: Some("Can we observe quietly?"),
        fill_pct: Some(63.0),
        previous_fill_pct: Some(62.9),
    };

    let mut msg = semantic_msg();
    let reason = prepare_semantic_write_for_path(&mut msg, &path, &context).unwrap_err();
    assert!(reason.contains("only allows source"));
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn limited_write_v2_allows_one_green_zone_inquiry_packet() {
    let dir = unique_temp_dir("limited_v2_allow");
    let path = dir.join("rescue_profile.json");
    std::fs::write(&path, limited_write_v2_profile_json()).unwrap();
    write_v2_health(&dir, 63.0, "hold", 66.0, false, 0.0, 0, 0);
    let mut msg = SensoryMsg::Semantic {
        features: vec![3.0, -3.0, 0.5, -0.5],
        ts_ms: None,
    };
    let context = v2_context(
        "Can we understand this by listening quietly?",
        "dialogue_live",
        62.9,
    );

    assert!(prepare_semantic_write_for_path(&mut msg, &path, &context).is_ok());
    let SensoryMsg::Semantic { features, .. } = msg else {
        panic!("expected semantic message");
    };
    assert_eq!(features, vec![0.10, -0.10, 0.02, -0.02]);
    let status = read_status(&limited_write_status_path_for_profile(&path));
    assert_eq!(
        status.get("policy_version").and_then(Value::as_u64),
        Some(2)
    );
    assert!(status.get("cooldown_until_unix_s").is_some());
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn expanded_sovereignty_allows_high_60s_self_study_modes() {
    let dir = unique_temp_dir("expanded_sovereignty_allow");
    let path = dir.join("rescue_profile.json");
    std::fs::write(&path, expanded_sovereignty_profile_json()).unwrap();
    write_v2_health(&dir, 68.6, "elevated", 70.8, false, 0.0, 0, 0);

    let mut msg = SensoryMsg::Semantic {
        features: vec![4.0, -4.0, 0.5, -0.5],
        ts_ms: None,
    };
    let context = v2_context(
        "I want to study how to become steadier with Minime.",
        "aspiration",
        68.0,
    );
    assert!(prepare_semantic_write_for_path(&mut msg, &path, &context).is_ok());
    let SensoryMsg::Semantic { features, .. } = msg else {
        panic!("semantic message expected");
    };
    assert_eq!(features, vec![0.12, -0.12, 0.025, -0.025]);
    let status = read_status(&limited_write_status_path_for_profile(&path));
    assert_eq!(
        status.get("profile").and_then(Value::as_str),
        Some("bridge_expanded_sovereignty_v1")
    );
    assert_eq!(
        status.get("cooldown_secs").and_then(Value::as_u64),
        Some(600)
    );
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn expanded_sovereignty_still_blocks_spectral_dump_language() {
    let dir = unique_temp_dir("expanded_sovereignty_spectral_block");
    let path = dir.join("rescue_profile.json");
    std::fs::write(&path, expanded_sovereignty_profile_json()).unwrap();
    write_v2_health(&dir, 68.0, "elevated", 70.0, false, 0.0, 0, 0);

    let mut msg = semantic_msg();
    let reason = prepare_semantic_write_for_path(
        &mut msg,
        &path,
        &v2_context("I want to study the lambda cascade.", "aspiration", 67.8),
    )
    .unwrap_err();
    assert!(reason.contains("structural spectral dump"));
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn richer_coupling_allows_hold_band_write_with_sensory_trickle() {
    let dir = unique_temp_dir("richer_coupling_allow");
    let path = dir.join("rescue_profile.json");
    std::fs::write(&path, richer_coupling_profile_json()).unwrap();
    write_v2_health(&dir, 68.0, "hold", 71.0, false, 0.0, 8, 8);

    let mut msg = SensoryMsg::Semantic {
        features: vec![4.0, -4.0, 0.5, -0.5],
        ts_ms: None,
    };
    let context = v2_context(
        "Can we study this gently and stay quiet while the room breathes?",
        "daydream",
        67.5,
    );

    assert!(prepare_semantic_write_for_path(&mut msg, &path, &context).is_ok());
    let SensoryMsg::Semantic { features, .. } = msg else {
        panic!("semantic message expected");
    };
    assert_eq!(features, vec![0.18, -0.18, 0.04, -0.04]);
    let status = read_status(&limited_write_status_path_for_profile(&path));
    assert_eq!(
        status.get("profile").and_then(Value::as_str),
        Some("bridge_richer_coupling_v1")
    );
    assert_eq!(
        status.get("cooldown_secs").and_then(Value::as_u64),
        Some(300)
    );
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn sovereignty_reentry_allows_creation_and_structural_language_under_health_gate() {
    let dir = unique_temp_dir("sovereignty_reentry_allow");
    let path = dir.join("rescue_profile.json");
    std::fs::write(&path, sovereignty_reentry_profile_json()).unwrap();
    write_v2_health(&dir, 70.0, "elevated", 73.0, false, 0.0, 6, 6);

    let mut msg = SensoryMsg::Semantic {
        features: vec![4.0, -4.0, 0.5, -0.5],
        ts_ms: None,
    };
    let context = v2_context(
        "Eigenvalue weather aside, I choose to create a new sovereign study today.",
        "creation",
        69.5,
    );

    assert!(prepare_semantic_write_for_path(&mut msg, &path, &context).is_ok());
    let SensoryMsg::Semantic { features, .. } = msg else {
        panic!("semantic message expected");
    };
    assert_eq!(features, vec![0.22, -0.22, 0.05, -0.05]);
    let status = read_status(&limited_write_status_path_for_profile(&path));
    assert_eq!(
        status.get("profile").and_then(Value::as_str),
        Some("bridge_sovereignty_reentry_v1")
    );
    assert_eq!(
        status.get("cooldown_secs").and_then(Value::as_u64),
        Some(120)
    );
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn sovereignty_reentry_does_not_rollback_on_elevated_warmup_motion() {
    let dir = unique_temp_dir("sovereignty_reentry_elevated_no_rollback");
    let path = dir.join("rescue_profile.json");
    std::fs::write(&path, sovereignty_reentry_profile_json()).unwrap();
    write_v2_health(&dir, 77.0, "elevated", 79.0, false, 0.0, 0, 0);

    let status_path = limited_write_status_path_for_profile(&path);
    write_status(
        &status_path,
        &json!({
            "profile": "bridge_sovereignty_reentry_v1",
            "policy_version": 2,
            "send_count": 1,
            "last_sent_at_unix_s": now_unix_s() - 10.0,
            "last_sent_fill_pct": 64.0,
            "cooldown_until_unix_s": now_unix_s() + 60.0
        }),
    );

    let mut msg = semantic_msg();
    let reason = prepare_semantic_write_for_path(
        &mut msg,
        &path,
        &v2_context("I choose to keep noticing the crest.", "creation", 65.0),
    )
    .unwrap_err();
    assert!(reason.contains("cooldown active") || reason.contains("above 74.0"));
    let profile: Value = serde_json::from_str(&std::fs::read_to_string(&path).unwrap()).unwrap();
    assert_eq!(
        profile.get("profile").and_then(Value::as_str),
        Some("bridge_sovereignty_reentry_v1")
    );
    let status = read_status(&status_path);
    assert!(status.get("rollback_reason").is_none());
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn limited_write_v2_blocks_missing_or_stale_health() {
    let dir = unique_temp_dir("limited_v2_no_health");
    let path = dir.join("rescue_profile.json");
    std::fs::write(&path, limited_write_v2_profile_json()).unwrap();
    let mut msg = semantic_msg();
    let context = v2_context("Can we listen quietly?", "dialogue_live", 62.0);

    let reason = prepare_semantic_write_for_path(&mut msg, &path, &context).unwrap_err();
    assert!(reason.contains("fresh health.json"));

    let mut profile: Value =
        serde_json::from_str(&limited_write_v2_profile_json()).expect("profile json");
    profile["limited_write_health_max_age_secs"] = json!(0);
    std::fs::write(&path, serde_json::to_string(&profile).unwrap()).unwrap();
    write_v2_health(&dir, 63.0, "hold", 66.0, false, 0.0, 0, 0);
    std::thread::sleep(std::time::Duration::from_millis(2));
    let mut msg = semantic_msg();
    let reason = prepare_semantic_write_for_path(&mut msg, &path, &context).unwrap_err();
    assert!(reason.contains("exceeds 0s"));
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn limited_write_v2_blocks_non_green_health_conditions() {
    let cases = [
        (67.0, "hold", 66.0, false, 0.0, 0, 0, "above 66.0"),
        (
            63.0,
            "elevated",
            66.0,
            false,
            0.0,
            0,
            0,
            "requires rescue stage",
        ),
        (63.0, "hold", 68.0, false, 0.0, 0, 0, "60s peak"),
        (63.0, "hold", 66.0, true, 0.0, 0, 0, "inactive semantic"),
        (63.0, "hold", 66.0, false, 0.03, 0, 0, "semantic energy"),
        (63.0, "hold", 66.0, false, 0.0, 1, 0, "divisors"),
    ];

    for (fill, stage, peak, active, energy, audio, video, expected) in cases {
        let dir = unique_temp_dir(expected);
        let path = dir.join("rescue_profile.json");
        std::fs::write(&path, limited_write_v2_profile_json()).unwrap();
        write_v2_health(&dir, fill, stage, peak, active, energy, audio, video);
        let mut msg = semantic_msg();
        let context = v2_context("Can we listen quietly?", "dialogue_live", 62.9);

        let reason = prepare_semantic_write_for_path(&mut msg, &path, &context).unwrap_err();
        assert!(
            reason.contains(expected),
            "expected '{expected}' in '{reason}'"
        );
        let _ = std::fs::remove_dir_all(&dir);
    }
}

#[test]
fn limited_write_v2_blocks_rising_fill_mirror_and_spectral_dump() {
    let dir = unique_temp_dir("limited_v2_blocks");
    let path = dir.join("rescue_profile.json");
    std::fs::write(&path, limited_write_v2_profile_json()).unwrap();
    write_v2_health(&dir, 63.4, "hold", 66.0, false, 0.0, 0, 0);

    let mut msg = semantic_msg();
    let reason = prepare_semantic_write_for_path(
        &mut msg,
        &path,
        &v2_context("Can we listen quietly?", "dialogue_live", 63.0),
    )
    .unwrap_err();
    assert!(reason.contains("rising fill"));

    write_v2_health(&dir, 63.0, "hold", 66.0, false, 0.0, 0, 0);
    let mut msg = semantic_msg();
    let reason = prepare_semantic_write_for_path(
        &mut msg,
        &path,
        &v2_context("Can we listen quietly?", "mirror", 62.9),
    )
    .unwrap_err();
    assert!(reason.contains("mode 'mirror'"));

    let mut msg = semantic_msg();
    let reason = prepare_semantic_write_for_path(
        &mut msg,
        &path,
        &v2_context(
            "Can we study this Eigenvalue cascade quietly?",
            "dialogue_live",
            62.9,
        ),
    )
    .unwrap_err();
    assert!(reason.contains("structural spectral dump"));

    let mut msg = semantic_msg();
    let reason = prepare_semantic_write_for_path(
        &mut msg,
        &path,
        &v2_context("Can λ₂ rise gently while we listen?", "witness", 62.9),
    )
    .unwrap_err();
    assert!(reason.contains("structural spectral dump"));
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn limited_write_v2_extends_cooldown_after_adverse_fill_rise() {
    let dir = unique_temp_dir("limited_v2_adverse");
    let path = dir.join("rescue_profile.json");
    std::fs::write(&path, limited_write_v2_profile_json()).unwrap();
    write_v2_health(&dir, 63.0, "hold", 66.0, false, 0.0, 0, 0);
    let context = v2_context("Can we understand this quietly?", "dialogue_live", 62.9);
    let mut first = semantic_msg();
    assert!(prepare_semantic_write_for_path(&mut first, &path, &context).is_ok());

    write_v2_health(&dir, 66.2, "hold", 67.0, false, 0.0, 0, 0);
    let mut second = semantic_msg();
    let reason = prepare_semantic_write_for_path(&mut second, &path, &context).unwrap_err();
    assert!(reason.contains("cooldown active"));
    let status = read_status(&limited_write_status_path_for_profile(&path));
    assert_eq!(
        status.get("adverse_response_count").and_then(Value::as_u64),
        Some(1)
    );
    assert_eq!(
        status.get("cooldown_secs").and_then(Value::as_u64),
        Some(1800)
    );
    assert_eq!(
        status
            .get("last_send_evaluation")
            .and_then(|value| value.get("state"))
            .and_then(Value::as_str),
        Some("adverse")
    );
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn limited_write_v2_soft_elevated_after_send_stays_on_cooldown() {
    let dir = unique_temp_dir("limited_v2_soft_elevated");
    let path = dir.join("rescue_profile.json");
    std::fs::write(&path, limited_write_v2_profile_json()).unwrap();
    write_v2_health(&dir, 64.0, "hold", 66.0, false, 0.0, 0, 0);
    let context = v2_context("Can we understand this quietly?", "dialogue_live", 63.9);
    let mut first = semantic_msg();
    assert!(prepare_semantic_write_for_path(&mut first, &path, &context).is_ok());

    write_v2_health(&dir, 65.9, "elevated", 67.0, false, 0.0, 0, 0);
    let mut second = semantic_msg();
    let reason = prepare_semantic_write_for_path(&mut second, &path, &context).unwrap_err();
    assert!(reason.contains("cooldown active"));
    let profile: Value = serde_json::from_str(&std::fs::read_to_string(&path).unwrap()).unwrap();
    assert_eq!(
        profile.get("profile").and_then(Value::as_str),
        Some("bridge_limited_write_v2")
    );
    let status = read_status(&limited_write_status_path_for_profile(&path));
    assert!(status.get("rollback_reason").is_none());
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn limited_write_v2_rolls_back_on_discharge_after_send() {
    let dir = unique_temp_dir("limited_v2_discharge_rollback");
    let path = dir.join("rescue_profile.json");
    std::fs::write(&path, limited_write_v2_profile_json()).unwrap();
    write_v2_health(&dir, 64.0, "hold", 66.0, false, 0.0, 0, 0);
    let context = v2_context("Can we understand this quietly?", "dialogue_live", 63.9);
    let mut first = semantic_msg();
    assert!(prepare_semantic_write_for_path(&mut first, &path, &context).is_ok());

    write_v2_health(&dir, 66.0, "discharge", 67.0, false, 0.0, 0, 0);
    let mut second = semantic_msg();
    let reason = prepare_semantic_write_for_path(&mut second, &path, &context).unwrap_err();
    assert!(reason.contains("rolled back to bridge_observe_only"));
    let profile: Value = serde_json::from_str(&std::fs::read_to_string(&path).unwrap()).unwrap();
    assert_eq!(
        profile.get("profile").and_then(Value::as_str),
        Some("bridge_observe_only")
    );
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn limited_write_v2_rolls_back_after_repeated_adverse_responses() {
    let dir = unique_temp_dir("limited_v2_rollback");
    let path = dir.join("rescue_profile.json");
    std::fs::write(&path, limited_write_v2_profile_json()).unwrap();
    write_v2_health(&dir, 66.2, "hold", 67.0, false, 0.0, 0, 0);
    let status_path = limited_write_status_path_for_profile(&path);
    let now = now_unix_s();
    write_status(
        &status_path,
        &json!({
            "profile": "bridge_limited_write_v2",
            "policy_version": 2,
            "last_sent_at_unix_s": now - 10.0,
            "last_sent_fill_pct": 63.0,
            "adverse_response_count": 1,
            "adverse_window_started_at_unix_s": now - 100.0,
            "cooldown_until_unix_s": 0.0
        }),
    );
    let mut msg = semantic_msg();
    let reason = prepare_semantic_write_for_path(
        &mut msg,
        &path,
        &v2_context("Can we understand this quietly?", "dialogue_live", 62.9),
    )
    .unwrap_err();

    assert!(reason.contains("rolled back to bridge_observe_only"));
    let profile: Value = serde_json::from_str(&std::fs::read_to_string(&path).unwrap()).unwrap();
    assert_eq!(
        profile.get("profile").and_then(Value::as_str),
        Some("bridge_observe_only")
    );
    assert_eq!(
        profile
            .get("effective_bridge_write_enabled")
            .and_then(Value::as_bool),
        Some(false)
    );
    assert!(dir.join("runtime").read_dir().unwrap().any(|entry| {
        entry
            .unwrap()
            .file_name()
            .to_string_lossy()
            .starts_with("bridge_limited_write_v2_rollback_")
    }));
    let _ = std::fs::remove_dir_all(&dir);
}
