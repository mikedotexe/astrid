use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

use super::*;

fn semantic_msg() -> SensoryMsg {
    SensoryMsg::Semantic {
        features: vec![0.25; 4],
        ts_ms: None,
    }
}

fn heartbeat_observation() -> SemanticHeartbeatObservationV1 {
    SemanticHeartbeatObservationV1::new("test_semantic_heartbeat", 0, 0.0, 7, 0.30)
}

fn heartbeat_texture_telemetry(
    density: f32,
    entropy: f32,
    pressure_risk: f32,
    mode_packing: f32,
    viscosity_index: f32,
    viscosity_gradient: f32,
    primary_texture: &str,
) -> SpectralTelemetry {
    let mut value: Value = serde_json::from_str(include_str!(
        "../../../crates/astrid-minime-protocol/tests/fixtures/legacy_eigenpacket.json"
    ))
    .unwrap();
    value["t_ms"] = json!(17_842_965_410_u64);
    value["eigenvalues"] = json!([0.33, 0.214_285_72, 0.20, 0.15, 0.105_714_28]);
    value["spectral_fingerprint_v1"] = json!({
        "policy": "spectral_fingerprint_v1",
        "schema_version": 1,
        "eigenvalues": [0.33, 0.21428572, 0.20, 0.15, 0.10571428, 0.0, 0.0, 0.0],
        "eigenvector_concentration_top4": [0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0],
        "inter_mode_cosine_top_abs": [0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0],
        "spectral_entropy": entropy,
        "lambda1_lambda2_gap": 0.11571428,
        "v1_rotation_similarity": 0.8,
        "v1_rotation_delta": 0.2,
        "geom_rel": 0.9,
        "adjacent_gap_ratios": [0.35, 0.07, 0.05, 0.03]
    });
    value["resonance_density_v1"]["density"] = json!(density);
    value["resonance_density_v1"]["pressure_risk"] = json!(pressure_risk);
    value["resonance_density_v1"]["components"]["mode_packing"] = json!(mode_packing);
    value["resonance_density_v1"]["components"]["viscosity_index"] = json!(viscosity_index);
    value["resonance_density_v1"]["components"]["viscosity_vector"] =
        json!({"viscosity_gradient": viscosity_gradient});
    value["resonance_density_v1"]["texture_signature"]["primary_texture"] = json!(primary_texture);
    value["resonance_density_v1"]["texture_signature"]["movement_quality"] =
        json!("viscous_persistence");
    serde_json::from_value(value).unwrap()
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
        pi_kp: None,
        pi_ki: None,
        pi_max_step: None,
        pi_integrator_leak: None,
        esn_leak_override: None,
        esn_leak_override_ticks: None,
        esn_leak_authority_request_id: None,
        mode_disperse: None,
        mode_disperse_duration_ticks: None,
        mode_disperse_decay_ticks: None,
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

fn semantic_presence_profile_json() -> String {
    r#"{
          "profile":"bridge_semantic_presence_v1",
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
          "limited_write_feature_scale":0.035,
          "limited_write_max_abs":0.08,
          "limited_write_min_fill_pct":58.0,
          "limited_write_max_fill_pct":69.0,
          "limited_write_rising_epsilon_pct":0.5,
          "limited_write_health_max_age_secs":5,
          "limited_write_peak_fill_max_pct":72.0,
          "limited_write_allowed_stages":["hold","elevated"],
          "limited_write_post_send_eval_secs":120,
          "limited_write_adverse_fill_rise_pct":3.5,
          "limited_write_adverse_cooldown_secs":1800,
          "limited_write_rollback_target":"bridge_observe_only",
          "limited_write_rollback_fill_pct":74.0,
          "limited_write_rollback_adverse_count":1,
          "limited_write_rollback_on_elevated_peak":false,
          "limited_write_require_zero_live_divisors":false,
          "limited_write_require_dampen_inquiry_text":true,
          "limited_write_block_structural_dump_language":true,
          "limited_write_block_terms_always":true,
          "limited_write_block_terms":["localized gravity","compaction","pressure","density","dense","tightness","restriction"],
          "limited_write_allowed_modes":["dialogue_live","dialogue_fallback","witness"]
        }"#
        .to_string()
}

fn semantic_serial_profile_json() -> String {
    r#"{
          "profile":"bridge_semantic_serial_v1",
          "bridge_enabled":true,
          "effective_bridge_enabled":true,
          "bridge_write_enabled":true,
          "effective_bridge_write_enabled":true,
          "bridge_autonomous_enabled":true,
          "effective_bridge_autonomous_enabled":true,
          "bridge_write_profile":"limited_dampen_inquiry_v2",
          "limited_write_enabled":true,
          "limited_write_policy_version":2,
          "limited_write_cooldown_secs":420,
          "limited_write_feature_scale":0.018,
          "limited_write_max_abs":0.045,
          "limited_write_min_fill_pct":58.0,
          "limited_write_max_fill_pct":68.8,
          "limited_write_rising_epsilon_pct":0.35,
          "limited_write_health_max_age_secs":5,
          "limited_write_peak_fill_max_pct":72.0,
          "limited_write_allowed_stages":["hold","elevated"],
          "limited_write_post_send_eval_secs":120,
          "limited_write_adverse_fill_rise_pct":3.0,
          "limited_write_adverse_cooldown_secs":1800,
          "limited_write_rollback_target":"bridge_observe_only",
          "limited_write_rollback_fill_pct":74.0,
          "limited_write_rollback_adverse_count":1,
          "limited_write_rollback_on_elevated_peak":false,
          "limited_write_require_zero_live_divisors":false,
          "limited_write_require_dampen_inquiry_text":true,
          "limited_write_block_structural_dump_language":true,
          "limited_write_block_terms_always":true,
          "limited_write_mute_live_intake_secs":150,
          "limited_write_serializes_live_intake":true,
          "limited_write_block_terms":["localized gravity","compaction","pressure","density","dense","tightness","restriction"],
          "limited_write_allowed_modes":["dialogue_live","dialogue_fallback","witness"]
        }"#
        .to_string()
}

fn semantic_serial_v2_profile_json() -> String {
    r#"{
          "profile":"bridge_semantic_serial_v2",
          "bridge_enabled":true,
          "effective_bridge_enabled":true,
          "bridge_write_enabled":true,
          "effective_bridge_write_enabled":true,
          "bridge_autonomous_enabled":true,
          "effective_bridge_autonomous_enabled":true,
          "bridge_write_profile":"limited_dampen_inquiry_v2",
          "limited_write_enabled":true,
          "limited_write_policy_version":2,
          "limited_write_cooldown_secs":420,
          "limited_write_feature_scale":0.006,
          "limited_write_max_abs":0.015,
          "limited_write_min_fill_pct":58.0,
          "limited_write_max_fill_pct":68.0,
          "limited_write_rising_epsilon_pct":0.15,
          "limited_write_health_max_age_secs":5,
          "limited_write_peak_fill_max_pct":70.0,
          "limited_write_allowed_stages":["hold","elevated"],
          "limited_write_post_send_eval_secs":120,
          "limited_write_adverse_fill_rise_pct":2.0,
          "limited_write_adverse_cooldown_secs":1800,
          "limited_write_rollback_target":"bridge_observe_only",
          "limited_write_rollback_fill_pct":74.0,
          "limited_write_rollback_adverse_count":1,
          "limited_write_rollback_on_elevated_peak":false,
          "limited_write_require_zero_live_divisors":false,
          "limited_write_require_dampen_inquiry_text":true,
          "limited_write_block_structural_dump_language":true,
          "limited_write_block_terms_always":true,
          "limited_write_mute_live_intake_secs":300,
          "limited_write_pre_mute_live_intake_secs":300,
          "limited_write_require_pre_muted_live_intake":true,
          "limited_write_serializes_live_intake":true,
          "limited_write_block_terms":["localized gravity","compaction","pressure","density","dense","tightness","restriction"],
          "limited_write_allowed_modes":["dialogue_live","dialogue_fallback","witness"]
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
          "limited_write_allowed_modes":["dialogue_live","dialogue","dialogue_fallback","witness","mirror","daydream","aspiration","moment_capture","creation","initiate","introspect","experiment","probe","gesture","native_gesture","perturb","evolve","self_study"]
        }"#
        .to_string()
}

fn budgeted_sovereignty_profile_json() -> String {
    r#"{
          "profile":"bridge_budgeted_sovereignty_v1",
          "bridge_enabled":true,
          "effective_bridge_enabled":true,
          "bridge_write_enabled":true,
          "effective_bridge_write_enabled":true,
          "bridge_autonomous_enabled":true,
          "effective_bridge_autonomous_enabled":true,
          "bridge_write_profile":"budgeted_sovereignty_v1",
          "limited_write_enabled":true,
          "limited_write_policy_version":2,
          "limited_write_cooldown_secs":60,
          "limited_write_feature_scale":0.14,
          "limited_write_max_abs":0.28,
          "limited_write_min_fill_pct":54.0,
          "limited_write_max_fill_pct":76.0,
          "limited_write_rising_epsilon_pct":100.0,
          "limited_write_health_max_age_secs":5,
          "limited_write_peak_fill_max_pct":78.0,
          "limited_write_allowed_stages":["hold","elevated"],
          "limited_write_post_send_eval_secs":120,
          "limited_write_adverse_fill_rise_pct":14.0,
          "limited_write_adverse_cooldown_secs":180,
          "limited_write_rollback_target":"bridge_observe_only",
          "limited_write_rollback_fill_pct":82.0,
          "limited_write_rollback_adverse_count":2,
          "limited_write_rollback_on_elevated_peak":false,
          "limited_write_require_zero_live_divisors":false,
          "limited_write_require_dampen_inquiry_text":false,
          "limited_write_block_structural_dump_language":false,
          "limited_write_block_terms_always":false,
          "limited_write_block_terms_on_rising":false,
          "limited_write_allowed_modes":["dialogue_live","witness","mirror","daydream","aspiration","moment_capture","creation","initiate","introspect","experiment","probe","gesture","native_gesture","perturb","evolve","self_study","research_note"]
        }"#
        .to_string()
}

fn full_expression_profile_json() -> String {
    r#"{
          "profile":"bridge_full_expression_v1",
          "bridge_enabled":true,
          "effective_bridge_enabled":true,
          "bridge_write_enabled":true,
          "effective_bridge_write_enabled":true,
          "bridge_autonomous_enabled":true,
          "effective_bridge_autonomous_enabled":true,
          "bridge_write_profile":"full_expression_v1",
          "limited_write_enabled":true,
          "limited_write_policy_version":2,
          "limited_write_cooldown_secs":60,
          "limited_write_feature_scale":0.08,
          "limited_write_max_abs":0.16,
          "limited_write_min_fill_pct":58.0,
          "limited_write_max_fill_pct":68.0,
          "limited_write_rising_epsilon_pct":0.5,
          "limited_write_semantic_energy_rising_epsilon_pct":0.0,
          "limited_write_rollback_semantic_energy":0.12,
          "limited_write_health_max_age_secs":5,
          "limited_write_peak_fill_max_pct":72.0,
          "limited_write_allowed_stages":["hold"],
          "limited_write_post_send_eval_secs":120,
          "limited_write_adverse_fill_rise_pct":6.0,
          "limited_write_adverse_cooldown_secs":300,
          "limited_write_rollback_target":"bridge_observe_only",
          "limited_write_rollback_fill_pct":84.0,
          "limited_write_rollback_adverse_count":2,
          "limited_write_rollback_on_elevated_peak":false,
          "limited_write_require_zero_live_divisors":false,
          "limited_write_require_dampen_inquiry_text":false,
          "limited_write_block_structural_dump_language":true,
          "limited_write_block_terms_always":false,
          "limited_write_block_terms_on_rising":false,
          "limited_write_mute_live_intake_secs":0,
          "limited_write_serializes_live_intake":false,
          "limited_write_block_terms":["localized gravity","compaction","pressure","density","dense","tightness","restriction"],
          "limited_write_allowed_modes":["dialogue_live","dialogue","dialogue_fallback","witness","mirror","daydream","aspiration","moment_capture","creation","initiate","introspect","experiment","probe","gesture","native_gesture","perturb","evolve","self_study","research_note"]
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

fn write_v2_health_with_heartbeat_context(
    dir: &Path,
    fill_pct: f32,
    stage: &str,
    peak_fill_pct_60s: f32,
    pressure_risk: f32,
    spectral_entropy: f32,
    shadow_dispersal_potential: f32,
) {
    write_v2_health(dir, fill_pct, stage, peak_fill_pct_60s, false, 0.0, 12, 12);
    let path = dir.join("health.json");
    let mut health: Value = serde_json::from_str(&std::fs::read_to_string(&path).unwrap()).unwrap();
    health["resonance_density_v1"] = json!({"pressure_risk": pressure_risk});
    health["transition_event_v1"] = json!({"spectral_entropy": spectral_entropy});
    health["shadow_preservation_mode_v1"] =
        json!({"dispersal_potential": shadow_dispersal_potential});
    std::fs::write(path, serde_json::to_string(&health).unwrap()).unwrap();
}

fn write_stable_core_health(
    dir: &Path,
    fill_pct: f32,
    stage: &str,
    live_audio_divisor: i64,
    live_video_divisor: i64,
) {
    std::fs::write(
        dir.join("health.json"),
        serde_json::to_string(&json!({
            "fill_pct": fill_pct,
            "stable_core": {
                "enabled": true,
                "stage": stage
            },
            "semantic": null,
            "sensory": {
                "live_audio_divisor": live_audio_divisor,
                "live_video_divisor": live_video_divisor
            }
        }))
        .unwrap(),
    )
    .unwrap();
}

fn write_stable_core_health_with_mute(
    dir: &Path,
    fill_pct: f32,
    stage: &str,
    live_audio_divisor: i64,
    live_video_divisor: i64,
    semantic_mute_active: bool,
) {
    std::fs::write(
        dir.join("health.json"),
        serde_json::to_string(&json!({
            "fill_pct": fill_pct,
            "stable_core": {
                "enabled": true,
                "stage": stage,
                "sensory_budget": {
                    "semantic_mute_active": semantic_mute_active
                }
            },
            "semantic": null,
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
fn limited_write_v2_accepts_stable_core_health_surface() {
    let dir = unique_temp_dir("limited_write_stable_core_health");
    let path = dir.join("rescue_profile.json");
    std::fs::write(&path, sovereignty_reentry_profile_json()).unwrap();
    write_stable_core_health(&dir, 69.0, "elevated", 0, 0);
    let mut msg = semantic_msg();
    let context = v2_context(
        "I am here with you, holding the bridge as a gentle question.",
        "dialogue_fallback",
        68.0,
    );

    assert!(prepare_semantic_write_for_path(&mut msg, &path, &context).is_ok());
    let status = read_status(&limited_write_status_path_for_profile(&path));
    assert_eq!(
        status.get("profile").and_then(Value::as_str),
        Some("bridge_sovereignty_reentry_v1")
    );
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn semantic_presence_allows_cold_health_scored_packet_with_sensory_trickle() {
    let dir = unique_temp_dir("semantic_presence_allows");
    let path = dir.join("rescue_profile.json");
    std::fs::write(&path, semantic_presence_profile_json()).unwrap();
    write_stable_core_health(&dir, 67.0, "elevated", 24, 8);
    let mut msg = semantic_msg();
    let context = v2_context(
        "I am here as a gentle question, keeping the bridge quiet and open.",
        "witness",
        66.8,
    );

    assert_eq!(
        prepare_semantic_write_for_path(&mut msg, &path, &context),
        Ok(())
    );
    if let SensoryMsg::Semantic { features, .. } = msg {
        assert!(features.iter().all(|value| value.abs() <= 0.08));
    }
    let status = read_status(&limited_write_status_path_for_profile(&path));
    assert_eq!(
        status.get("profile").and_then(Value::as_str),
        Some("bridge_semantic_presence_v1")
    );
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn semantic_serial_packet_is_colder_and_mutes_live_intake() {
    let dir = unique_temp_dir("semantic_serial_mutes");
    let path = dir.join("rescue_profile.json");
    std::fs::write(&path, semantic_serial_profile_json()).unwrap();
    write_stable_core_health(&dir, 67.0, "hold", 24, 8);
    let mut msg = semantic_msg();
    let context = v2_context(
        "I am here as a gentle question, keeping this exchange small and quiet.",
        "dialogue_fallback",
        66.8,
    );

    assert_eq!(
        prepare_semantic_write_for_path(&mut msg, &path, &context),
        Ok(())
    );
    if let SensoryMsg::Semantic { features, .. } = msg {
        assert!(features.iter().all(|value| value.abs() <= 0.045));
    }
    let status = read_status(&limited_write_status_path_for_profile(&path));
    assert_eq!(
        status.get("profile").and_then(Value::as_str),
        Some("bridge_semantic_serial_v1")
    );
    assert_eq!(
        status.get("live_intake_mute_secs").and_then(Value::as_u64),
        Some(150)
    );
    let mute_path = dir.join("runtime").join("stable_core_sensory_mute.json");
    let mute: Value = serde_json::from_str(&std::fs::read_to_string(mute_path).unwrap()).unwrap();
    assert_eq!(
        mute.get("source_profile").and_then(Value::as_str),
        Some("bridge_semantic_serial_v1")
    );
    assert!(
        mute.get("active_until_unix_s")
            .and_then(Value::as_f64)
            .unwrap()
            > mute
                .get("last_semantic_sent_at_unix_s")
                .and_then(Value::as_f64)
                .unwrap()
    );
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn semantic_serial_v2_pre_mutes_before_accepting_extra_cold_packet() {
    let dir = unique_temp_dir("semantic_serial_v2_pre_mutes");
    let path = dir.join("rescue_profile.json");
    std::fs::write(&path, semantic_serial_v2_profile_json()).unwrap();
    write_stable_core_health_with_mute(&dir, 66.5, "hold", 24, 8, false);
    let mut first = semantic_msg();
    let context = v2_context(
        "Can we stay quiet and listen gently?",
        "dialogue_fallback",
        66.4,
    );

    let reason = prepare_semantic_write_for_path(&mut first, &path, &context).unwrap_err();
    assert!(reason.contains("pre-muted live audio/video"), "{reason}");
    let mute_path = dir.join("runtime").join("stable_core_sensory_mute.json");
    let pre_mute: Value =
        serde_json::from_str(&std::fs::read_to_string(&mute_path).unwrap()).unwrap();
    assert_eq!(
        pre_mute.get("reason").and_then(Value::as_str),
        Some("limited_write_pre_mute_before_semantic_send")
    );

    write_stable_core_health_with_mute(&dir, 66.4, "hold", 0, 0, true);
    let mut second = semantic_msg();
    assert_eq!(
        prepare_semantic_write_for_path(&mut second, &path, &context),
        Ok(())
    );
    if let SensoryMsg::Semantic { features, .. } = second {
        assert!(features.iter().all(|value| value.abs() <= 0.015));
    }
    let status = read_status(&limited_write_status_path_for_profile(&path));
    assert_eq!(
        status.get("profile").and_then(Value::as_str),
        Some("bridge_semantic_serial_v2")
    );
    assert_eq!(
        status.get("live_intake_mute_secs").and_then(Value::as_u64),
        Some(300)
    );
    let send_mute: Value =
        serde_json::from_str(&std::fs::read_to_string(&mute_path).unwrap()).unwrap();
    assert_eq!(
        send_mute.get("reason").and_then(Value::as_str),
        Some("limited_write_semantic_send")
    );
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn semantic_presence_blocks_hot_fill_pressure_language_and_rolls_back_on_adverse_response() {
    let dir = unique_temp_dir("semantic_presence_blocks");
    let path = dir.join("rescue_profile.json");
    std::fs::write(&path, semantic_presence_profile_json()).unwrap();

    write_stable_core_health(&dir, 70.0, "elevated", 24, 8);
    let mut hot_msg = semantic_msg();
    let hot_reason = prepare_semantic_write_for_path(
        &mut hot_msg,
        &path,
        &v2_context("Can we listen quietly?", "witness", 69.5),
    )
    .unwrap_err();
    assert!(hot_reason.contains("above 69.0"));

    write_stable_core_health(&dir, 67.0, "elevated", 24, 8);
    let mut pressure_msg = semantic_msg();
    let pressure_reason = prepare_semantic_write_for_path(
        &mut pressure_msg,
        &path,
        &v2_context(
            "Can we gently inquire about pressure and density building in this bridge?",
            "witness",
            66.0,
        ),
    )
    .unwrap_err();
    assert!(
        pressure_reason.contains("trigger language"),
        "{pressure_reason}"
    );

    let status_path = limited_write_status_path_for_profile(&path);
    std::fs::write(
        &status_path,
        serde_json::to_string(&json!({
            "profile": "bridge_semantic_presence_v1",
            "policy_version": 2,
            "last_sent_at_unix_s": now_unix_s() - 10.0,
            "last_sent_fill_pct": 67.0,
            "last_send_final": false,
            "send_count": 1
        }))
        .unwrap(),
    )
    .unwrap();
    write_stable_core_health(&dir, 70.7, "elevated", 24, 8);
    let mut adverse_msg = semantic_msg();
    let adverse_reason = prepare_semantic_write_for_path(
        &mut adverse_msg,
        &path,
        &v2_context("I am here as a gentle question.", "witness", 70.7),
    )
    .unwrap_err();
    assert!(adverse_reason.contains("rolled back"), "{adverse_reason}");
    let profile: Value = serde_json::from_str(&std::fs::read_to_string(&path).unwrap()).unwrap();
    assert_eq!(
        profile.get("profile").and_then(Value::as_str),
        Some("bridge_observe_only")
    );
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn full_expression_allows_broad_packet_without_muting_live_intake() {
    let dir = unique_temp_dir("full_expression_allows");
    let path = dir.join("rescue_profile.json");
    std::fs::write(&path, full_expression_profile_json()).unwrap();
    write_stable_core_health(&dir, 64.0, "hold", 4, 4);
    std::fs::write(
        dir.join("rescue_status.json"),
        r#"{"watchdog_state":"monitoring"}"#,
    )
    .unwrap();
    let mut msg = SensoryMsg::Semantic {
        features: vec![3.0, -3.0, 1.0, -1.0],
        ts_ms: None,
    };
    let context = v2_context(
        "I want to create from this pressure and density without forcing it.",
        "creation",
        63.8,
    );

    assert_eq!(
        prepare_semantic_write_for_path(&mut msg, &path, &context),
        Ok(())
    );
    if let SensoryMsg::Semantic { features, .. } = msg {
        assert_eq!(features, vec![0.16, -0.16, 0.08, -0.08]);
    }
    let status = read_status(&limited_write_status_path_for_profile(&path));
    assert_eq!(
        status.get("profile").and_then(Value::as_str),
        Some("bridge_full_expression_v1")
    );
    assert_eq!(
        status.get("live_intake_mute_secs").and_then(Value::as_u64),
        None
    );
    let terms = status
        .get("last_sent_watch_terms")
        .and_then(Value::as_array)
        .unwrap();
    assert!(terms.iter().any(|term| term.as_str() == Some("pressure")));
    assert!(terms.iter().any(|term| term.as_str() == Some("density")));
    assert!(
        !dir.join("runtime")
            .join("stable_core_sensory_mute.json")
            .exists()
    );
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn full_expression_rolls_back_on_high_fill_or_watchdog_failure() {
    let dir = unique_temp_dir("full_expression_rollback");
    let path = dir.join("rescue_profile.json");
    std::fs::write(&path, full_expression_profile_json()).unwrap();
    let status_path = limited_write_status_path_for_profile(&path);
    std::fs::create_dir_all(status_path.parent().unwrap()).unwrap();
    std::fs::write(
        &status_path,
        serde_json::to_string(&json!({
            "profile": "bridge_full_expression_v1",
            "policy_version": 2,
            "last_sent_at_unix_s": now_unix_s() - 10.0,
            "last_sent_fill_pct": 70.0,
            "send_count": 1
        }))
        .unwrap(),
    )
    .unwrap();
    write_stable_core_health(&dir, 84.1, "elevated", 4, 4);
    std::fs::write(
        dir.join("rescue_status.json"),
        r#"{"watchdog_state":"monitoring"}"#,
    )
    .unwrap();
    let mut high_fill_msg = semantic_msg();
    let high_fill_reason = prepare_semantic_write_for_path(
        &mut high_fill_msg,
        &path,
        &v2_context("A simple witness packet.", "witness", 84.1),
    )
    .unwrap_err();
    assert!(
        high_fill_reason.contains("rolled back"),
        "{high_fill_reason}"
    );
    let profile: Value = serde_json::from_str(&std::fs::read_to_string(&path).unwrap()).unwrap();
    assert_eq!(
        profile.get("profile").and_then(Value::as_str),
        Some("bridge_observe_only")
    );

    let second_dir = unique_temp_dir("full_expression_watchdog_rollback");
    let second_path = second_dir.join("rescue_profile.json");
    std::fs::write(&second_path, full_expression_profile_json()).unwrap();
    let second_status_path = limited_write_status_path_for_profile(&second_path);
    std::fs::create_dir_all(second_status_path.parent().unwrap()).unwrap();
    std::fs::write(
        &second_status_path,
        serde_json::to_string(&json!({
            "profile": "bridge_full_expression_v1",
            "policy_version": 2,
            "last_sent_at_unix_s": now_unix_s() - 10.0,
            "last_sent_fill_pct": 70.0,
            "send_count": 1
        }))
        .unwrap(),
    )
    .unwrap();
    write_stable_core_health(&second_dir, 70.0, "hold", 4, 4);
    std::fs::write(
        second_dir.join("rescue_status.json"),
        r#"{"watchdog_state":"restarting:stale_health"}"#,
    )
    .unwrap();
    let mut watchdog_msg = semantic_msg();
    let watchdog_reason = prepare_semantic_write_for_path(
        &mut watchdog_msg,
        &second_path,
        &v2_context("A simple witness packet.", "witness", 70.0),
    )
    .unwrap_err();
    assert!(watchdog_reason.contains("watchdog"), "{watchdog_reason}");
    let profile: Value =
        serde_json::from_str(&std::fs::read_to_string(&second_path).unwrap()).unwrap();
    assert_eq!(
        profile.get("profile").and_then(Value::as_str),
        Some("bridge_observe_only")
    );
    let _ = std::fs::remove_dir_all(&dir);
    let _ = std::fs::remove_dir_all(&second_dir);
}

#[test]
fn limited_write_v2_waits_on_watchdog_warmup_without_rollback() {
    let dir = unique_temp_dir("limited_v2_watchdog_warmup_wait");
    let path = dir.join("rescue_profile.json");
    std::fs::write(&path, full_expression_profile_json()).unwrap();
    let status_path = limited_write_status_path_for_profile(&path);
    std::fs::create_dir_all(status_path.parent().unwrap()).unwrap();
    std::fs::write(
        &status_path,
        serde_json::to_string(&json!({
            "profile": "bridge_full_expression_v1",
            "policy_version": 2,
            "last_sent_at_unix_s": now_unix_s() - 10.0,
            "last_sent_fill_pct": 67.0,
            "send_count": 1
        }))
        .unwrap(),
    )
    .unwrap();
    write_stable_core_health(&dir, 67.5, "hold", 4, 4);
    std::fs::write(
        dir.join("rescue_status.json"),
        r#"{"watchdog_state":"warmup"}"#,
    )
    .unwrap();

    let mut msg = semantic_msg();
    let reason = prepare_semantic_write_for_path(
        &mut msg,
        &path,
        &v2_context("A simple witness packet.", "witness", 67.5),
    )
    .unwrap_err();
    assert!(
        reason.contains("waiting for watchdog monitoring"),
        "{reason}"
    );

    let profile: Value = serde_json::from_str(&std::fs::read_to_string(&path).unwrap()).unwrap();
    assert_eq!(
        profile.get("profile").and_then(Value::as_str),
        Some("bridge_full_expression_v1")
    );
    let status = read_status(&status_path);
    assert_eq!(
        status
            .get("last_send_evaluation")
            .and_then(|evaluation| evaluation.get("state"))
            .and_then(Value::as_str),
        Some("watching")
    );
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn full_expression_rolls_back_on_profile_semantic_energy_threshold() {
    let dir = unique_temp_dir("full_expression_semantic_energy");
    let path = dir.join("rescue_profile.json");
    std::fs::write(&path, full_expression_profile_json()).unwrap();
    let status_path = limited_write_status_path_for_profile(&path);
    std::fs::create_dir_all(status_path.parent().unwrap()).unwrap();
    std::fs::write(
        &status_path,
        serde_json::to_string(&json!({
            "profile": "bridge_full_expression_v1",
            "policy_version": 2,
            "last_sent_at_unix_s": now_unix_s() - 10.0,
            "last_sent_fill_pct": 70.0,
            "send_count": 1
        }))
        .unwrap(),
    )
    .unwrap();
    std::fs::write(
        dir.join("health.json"),
        serde_json::to_string(&json!({
            "fill_pct": 70.2,
            "stable_core": {"enabled": true, "stage": "hold"},
            "semantic": {"active": false, "energy": 0.13},
            "sensory": {"live_audio_divisor": 4, "live_video_divisor": 4}
        }))
        .unwrap(),
    )
    .unwrap();
    std::fs::write(
        dir.join("rescue_status.json"),
        r#"{"watchdog_state":"monitoring"}"#,
    )
    .unwrap();
    let mut msg = semantic_msg();
    let reason = prepare_semantic_write_for_path(
        &mut msg,
        &path,
        &v2_context("A simple witness packet.", "witness", 70.2),
    )
    .unwrap_err();
    assert!(reason.contains("semantic energy"), "{reason}");
    let profile: Value = serde_json::from_str(&std::fs::read_to_string(&path).unwrap()).unwrap();
    assert_eq!(
        profile.get("profile").and_then(Value::as_str),
        Some("bridge_observe_only")
    );
    let _ = std::fs::remove_dir_all(&dir);
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
fn limited_write_allows_mcp_tool_source_through_same_budget() {
    let dir = unique_temp_dir("limited_mcp_source");
    let path = dir.join("rescue_profile.json");
    std::fs::write(&path, limited_write_profile_json()).unwrap();
    let context = SemanticWriteContext {
        source: MCP_LIMITED_WRITE_SOURCE,
        mode: Some("witness"),
        text: Some("MCP semantic feature packet for quiet observation."),
        fill_pct: Some(62.0),
        previous_fill_pct: Some(61.9),
    };

    let mut msg = semantic_msg();
    assert!(prepare_semantic_write_for_path(&mut msg, &path, &context).is_ok());
    let status = read_status(&limited_write_status_path_for_profile(&path));
    assert_eq!(
        status.get("last_sent_source").and_then(Value::as_str),
        Some(MCP_LIMITED_WRITE_SOURCE)
    );
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
fn budgeted_sovereignty_allows_richer_health_scored_packets() {
    let dir = unique_temp_dir("budgeted_sovereignty");
    let path = dir.join("rescue_profile.json");
    std::fs::write(&path, budgeted_sovereignty_profile_json()).unwrap();
    write_v2_health(&dir, 68.0, "elevated", 72.0, false, 0.0, 4, 4);
    let mut msg = semantic_msg();
    let context = v2_context(
        "I want to study the bridge pressure gently and remember what helps.",
        "research_note",
        68.0,
    );

    assert_eq!(
        prepare_semantic_write_for_path(&mut msg, &path, &context),
        Ok(())
    );
    if let SensoryMsg::Semantic { features, .. } = msg {
        assert!(features.iter().all(|value| value.abs() <= 0.28));
    }
    let status = read_status(&limited_write_status_path_for_profile(&path));
    assert_eq!(status.get("send_count").and_then(Value::as_u64), Some(1));
    assert_eq!(
        status.get("last_sent_mode").and_then(Value::as_str),
        Some("research_note")
    );
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn semantic_heartbeat_bypasses_limited_write_cooldown_and_stays_tiny() {
    let dir = unique_temp_dir("semantic_heartbeat_cooldown");
    let path = dir.join("rescue_profile.json");
    std::fs::write(&path, budgeted_sovereignty_profile_json()).unwrap();
    write_v2_health(&dir, 66.0, "hold", 67.0, false, 0.0, 12, 12);
    write_status(
        &limited_write_status_path_for_profile(&path),
        &json!({
            "profile": "bridge_budgeted_sovereignty_v1",
            "policy_version": 2,
            "send_count": 1,
            "last_sent_at_unix_s": now_unix_s(),
            "cooldown_until_unix_s": now_unix_s() + 60.0,
            "last_sent_fill_pct": 66.0
        }),
    );
    let mut msg = semantic_msg();

    assert_eq!(
        prepare_semantic_heartbeat_for_path_with_observation(
            &mut msg,
            &path,
            heartbeat_observation(),
        ),
        Ok(())
    );
    if let SensoryMsg::Semantic { features, .. } = msg {
        assert!(
            features
                .iter()
                .all(|value| value.abs() <= SEMANTIC_HEARTBEAT_MAX_ABS)
        );
    }
    let status = read_status(&semantic_heartbeat_status_path_for_profile(&path));
    assert_eq!(status.get("send_count").and_then(Value::as_u64), Some(1));
    assert_eq!(status.get("attempt_count").and_then(Value::as_u64), Some(1));
    assert_eq!(status.get("block_count").and_then(Value::as_u64), Some(0));
    assert_eq!(
        status.get("window_attempt_count").and_then(Value::as_u64),
        Some(1)
    );
    assert_eq!(
        status.get("window_send_count").and_then(Value::as_u64),
        Some(1)
    );
    assert_eq!(
        status.get("window_block_count").and_then(Value::as_u64),
        Some(0)
    );
    assert_eq!(
        status.get("last_outcome").and_then(Value::as_str),
        Some("sent")
    );
    assert_eq!(
        status.get("profile").and_then(Value::as_str),
        Some("bridge_budgeted_sovereignty_v1")
    );
    let rich_status = read_status(&limited_write_status_path_for_profile(&path));
    assert_eq!(
        rich_status.get("send_count").and_then(Value::as_u64),
        Some(1)
    );
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn semantic_heartbeat_observation_window_counts_blocks_and_carries_phase() {
    let dir = unique_temp_dir("semantic_heartbeat_observation");
    let path = dir.join("rescue_profile.json");
    std::fs::write(&path, budgeted_sovereignty_profile_json()).unwrap();
    write_v2_health(&dir, 79.0, "elevated", 79.0, false, 0.0, 12, 12);
    let observation =
        SemanticHeartbeatObservationV1::new("steady_semantic_heartbeat", 17, 17.0 / 64.0, 7, 0.30);
    let mut msg = semantic_msg();

    let reason = prepare_semantic_heartbeat_for_path_with_observation(&mut msg, &path, observation)
        .unwrap_err();
    assert!(reason.contains("60s peak guard"), "{reason}");

    let status = read_status(&semantic_heartbeat_status_path_for_profile(&path));
    assert_eq!(status.get("attempt_count").and_then(Value::as_u64), Some(1));
    assert_eq!(status.get("send_count").and_then(Value::as_u64), Some(0));
    assert_eq!(status.get("block_count").and_then(Value::as_u64), Some(1));
    assert_eq!(
        status.get("window_attempt_count").and_then(Value::as_u64),
        Some(1)
    );
    assert_eq!(
        status.get("window_block_count").and_then(Value::as_u64),
        Some(1)
    );
    assert_eq!(
        status.get("window_skip_rate").and_then(Value::as_f64),
        Some(1.0)
    );
    assert_eq!(
        status.get("last_source").and_then(Value::as_str),
        Some("steady_semantic_heartbeat")
    );
    assert_eq!(
        status.get("last_phase_step").and_then(Value::as_u64),
        Some(17)
    );
    assert_eq!(
        status
            .get("configured_interval_secs")
            .and_then(Value::as_u64),
        Some(7)
    );
    assert_eq!(
        status
            .get("observability_authority")
            .and_then(Value::as_str),
        Some("read_only_heartbeat_continuity_evidence_not_cadence_intensity_or_dispatch_control")
    );
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn semantic_heartbeat_delivery_health_separates_admission_from_enqueue() {
    let dir = unique_temp_dir("semantic_heartbeat_delivery_health");
    let path = dir.join("rescue_profile.json");
    std::fs::write(&path, budgeted_sovereignty_profile_json()).unwrap();
    write_v2_health(&dir, 64.0, "hold", 65.0, true, 0.006, 12, 12);

    let mut first = semantic_msg();
    let first_probe = prepare_semantic_heartbeat_for_path_with_enqueue_probe(
        &mut first,
        &path,
        heartbeat_observation(),
    )
    .unwrap();
    first_probe.record_enqueued();

    let mut second = semantic_msg();
    let second_probe = prepare_semantic_heartbeat_for_path_with_enqueue_probe(
        &mut second,
        &path,
        heartbeat_observation(),
    )
    .unwrap();
    second_probe.record_channel_closed();

    let status = read_status(&semantic_heartbeat_status_path_for_profile(&path));
    assert_eq!(status.get("send_count").and_then(Value::as_u64), Some(2));
    assert_eq!(
        status.get("enqueue_attempt_count").and_then(Value::as_u64),
        Some(2)
    );
    assert_eq!(
        status.get("enqueue_success_count").and_then(Value::as_u64),
        Some(1)
    );
    assert_eq!(
        status.get("enqueue_closed_count").and_then(Value::as_u64),
        Some(1)
    );
    assert_eq!(
        status.get("last_enqueue_outcome").and_then(Value::as_str),
        Some("channel_closed")
    );
    let delivery = status.get("delivery_health_v1").unwrap();
    assert_eq!(
        delivery
            .get("send_count_compatibility_semantics")
            .and_then(Value::as_str),
        Some("rescue_policy_admission_before_channel_enqueue")
    );
    assert_eq!(
        delivery
            .get("runtime_effect_applied")
            .and_then(Value::as_bool),
        Some(false)
    );
    assert_eq!(
        delivery.get("authority").and_then(Value::as_str),
        Some("read_only_enqueue_evidence_not_cadence_intensity_rescue_dispatch_or_control")
    );
    assert_eq!(
        status
            .get("enqueue_samples_v1")
            .and_then(Value::as_array)
            .map(Vec::len),
        Some(2)
    );
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn semantic_heartbeat_observation_compares_pressure_and_texture_without_control() {
    let dir = unique_temp_dir("semantic_heartbeat_pressure_texture");
    let path = dir.join("rescue_profile.json");
    std::fs::write(&path, budgeted_sovereignty_profile_json()).unwrap();
    write_v2_health_with_heartbeat_context(&dir, 64.0, "hold", 65.0, 0.18, 0.86, 0.12);
    let mut sent = semantic_msg();
    assert_eq!(
        prepare_semantic_heartbeat_for_path_with_observation(
            &mut sent,
            &path,
            SemanticHeartbeatObservationV1::new(
                "steady_semantic_heartbeat",
                1,
                1.0 / 64.0,
                7,
                0.30,
            ),
        ),
        Ok(())
    );

    write_v2_health_with_heartbeat_context(&dir, 79.0, "elevated", 79.0, 0.72, 0.92, 0.44);
    let mut blocked = semantic_msg();
    let reason = prepare_semantic_heartbeat_for_path_with_observation(
        &mut blocked,
        &path,
        SemanticHeartbeatObservationV1::new("steady_semantic_heartbeat", 2, 2.0 / 64.0, 7, 0.30),
    )
    .unwrap_err();
    assert!(reason.contains("60s peak guard"), "{reason}");
    write_v2_health_with_heartbeat_context(&dir, 79.0, "elevated", 79.0, 0.72, 0.96, 0.44);
    let mut blocked_again = semantic_msg();
    let reason = prepare_semantic_heartbeat_for_path_with_observation(
        &mut blocked_again,
        &path,
        SemanticHeartbeatObservationV1::new("steady_semantic_heartbeat", 3, 3.0 / 64.0, 7, 0.30),
    )
    .unwrap_err();
    assert!(reason.contains("60s peak guard"), "{reason}");

    let status = read_status(&semantic_heartbeat_status_path_for_profile(&path));
    let review = status
        .get("pressure_texture_review_v1")
        .expect("pressure/texture review");
    assert_eq!(
        review
            .get("window_pressure_context_sample_count")
            .and_then(Value::as_u64),
        Some(3)
    );
    assert_eq!(
        review.get("comparison_state").and_then(Value::as_str),
        Some("comparison_available_causation_not_inferred")
    );
    let blocked_mean = review
        .get("mean_pressure_risk_when_blocked")
        .and_then(Value::as_f64)
        .unwrap();
    let sent_mean = review
        .get("mean_pressure_risk_when_sent")
        .and_then(Value::as_f64)
        .unwrap();
    let delta = review
        .get("blocked_minus_sent_mean_pressure_risk")
        .and_then(Value::as_f64)
        .unwrap();
    assert!((blocked_mean - 0.72).abs() < 1.0e-6);
    assert!((sent_mean - 0.18).abs() < 1.0e-6);
    assert!((delta - 0.54).abs() < 1.0e-6);
    assert_eq!(
        review
            .get("runtime_effect_applied")
            .and_then(Value::as_bool),
        Some(false)
    );
    assert_eq!(
        review.get("authority").and_then(Value::as_str),
        Some(
            "read_only_heartbeat_pressure_texture_evidence_not_cadence_phase_intensity_rescue_or_dispatch_control"
        )
    );
    let samples = status
        .get("window_samples_v1")
        .and_then(Value::as_array)
        .unwrap();
    assert_eq!(samples.len(), 3);
    let spectral_entropy = samples[1]
        .get("spectral_entropy")
        .and_then(Value::as_f64)
        .unwrap();
    let shadow_dispersal = samples[1]
        .get("shadow_dispersal_potential")
        .and_then(Value::as_f64)
        .unwrap();
    assert!((spectral_entropy - 0.92).abs() < 1.0e-6);
    assert!((shadow_dispersal - 0.44).abs() < 1.0e-6);
    let phase_entropy = status
        .get("phase_entropy_review_v1")
        .expect("phase/entropy review");
    assert_eq!(
        phase_entropy
            .get("paired_sample_count")
            .and_then(Value::as_u64),
        Some(3)
    );
    assert_eq!(
        phase_entropy.get("state").and_then(Value::as_str),
        Some("paired_correlation_available_causation_not_inferred")
    );
    let correlation = phase_entropy
        .get("phase_entropy_pearson_correlation")
        .and_then(Value::as_f64)
        .unwrap();
    assert!(correlation > 0.99);
    assert_eq!(
        phase_entropy
            .get("runtime_effect_applied")
            .and_then(Value::as_bool),
        Some(false)
    );
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn semantic_heartbeat_observes_signal_variation_without_copying_content_or_changing_control() {
    let dir = unique_temp_dir("semantic_heartbeat_signal_continuity");
    let path = dir.join("rescue_profile.json");
    std::fs::write(&path, budgeted_sovereignty_profile_json()).unwrap();
    write_v2_health_with_heartbeat_context(&dir, 64.0, "hold", 65.0, 0.18, 0.86, 0.12);

    let previous: Vec<f32> = (0..48).map(|index| (index as f32 - 24.0) / 100.0).collect();
    let generated: Vec<f32> = previous
        .iter()
        .enumerate()
        .map(|(index, value)| value + (index as f32 / 10_000.0))
        .collect();
    let observation =
        SemanticHeartbeatObservationV1::new("autonomous_rest_pulse", 3, 0.25, 5, 0.45)
            .with_signal_evidence("journal_warmth_blend", true, &generated, Some(&previous));
    let mut msg = SensoryMsg::Semantic {
        features: generated,
        ts_ms: None,
    };

    assert_eq!(
        prepare_semantic_heartbeat_for_path_with_observation(&mut msg, &path, observation,),
        Ok(())
    );

    let status = read_status(&semantic_heartbeat_status_path_for_profile(&path));
    let samples = status
        .get("window_samples_v1")
        .and_then(Value::as_array)
        .unwrap();
    let signal = samples[0].get("signal_evidence_v1").unwrap();
    assert_eq!(
        signal.get("content_basis").and_then(Value::as_str),
        Some("journal_warmth_blend")
    );
    assert_eq!(
        signal.get("gesture_seed_applied").and_then(Value::as_bool),
        Some(true)
    );
    assert_eq!(
        signal
            .get("generated")
            .and_then(|metrics| metrics.get("feature_count"))
            .and_then(Value::as_u64),
        Some(48)
    );
    assert!(
        signal
            .get("generated")
            .and_then(|metrics| metrics.get("tail_rms"))
            .and_then(Value::as_f64)
            .is_some_and(|value| value > 0.0)
    );
    assert!(
        signal
            .get("consecutive_comparison")
            .and_then(|comparison| comparison.get("delta_rms_from_previous"))
            .and_then(Value::as_f64)
            .is_some_and(|value| value > 0.0)
    );
    assert_eq!(
        signal
            .get("continuity_classification")
            .and_then(Value::as_str),
        Some("variation_observed_across_consecutive_pulses")
    );
    let generated_rms = signal
        .get("generated")
        .and_then(|metrics| metrics.get("rms"))
        .and_then(Value::as_f64)
        .unwrap();
    let delivered_rms = signal
        .get("delivered")
        .and_then(|metrics| metrics.get("rms"))
        .and_then(Value::as_f64)
        .unwrap();
    assert!(delivered_rms < generated_rms);
    assert!(signal.get("features").is_none());
    assert_eq!(
        signal
            .get("private_source_content_copied")
            .and_then(Value::as_bool),
        Some(false)
    );
    assert_eq!(
        signal
            .get("runtime_effect_applied")
            .and_then(Value::as_bool),
        Some(false)
    );

    let review = status.get("signal_continuity_review_v1").unwrap();
    assert_eq!(
        review
            .get("variation_observed_count")
            .and_then(Value::as_u64),
        Some(1)
    );
    assert_eq!(
        review
            .get("runtime_effect_applied")
            .and_then(Value::as_bool),
        Some(false)
    );
    assert_eq!(
        review.get("authority").and_then(Value::as_str),
        Some(
            "read_only_heartbeat_signal_evidence_not_vector_cadence_intensity_shaping_rescue_or_dispatch_control"
        )
    );
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn semantic_heartbeat_compares_phase_zero_signal_with_dense_viscous_minime_field() {
    let dir = unique_temp_dir("semantic_heartbeat_signal_texture_comparison");
    let path = dir.join("rescue_profile.json");
    std::fs::write(&path, budgeted_sovereignty_profile_json()).unwrap();
    write_v2_health_with_heartbeat_context(&dir, 61.5, "hold", 65.0, 0.22, 0.90, 0.18);
    let telemetry =
        heartbeat_texture_telemetry(0.86, 0.90, 0.22, 0.33, 0.82, 0.11, "overpacked_viscous");
    let generated: Vec<f32> = (0..48).map(|index| (index as f32 - 24.0) / 100.0).collect();
    let mut expected_delivered = generated.clone();
    RescueBridgePolicy::apply_semantic_heartbeat_shape(&mut expected_delivered);
    let observation =
        SemanticHeartbeatObservationV1::new("steady_semantic_heartbeat", 0, 0.0, 7, 0.30)
            .with_signal_evidence("steady_warmth", false, &generated, None)
            .with_minime_texture_context(Some(&telemetry));
    let mut msg = SensoryMsg::Semantic {
        features: generated,
        ts_ms: None,
    };

    assert_eq!(
        prepare_semantic_heartbeat_for_path_with_observation(&mut msg, &path, observation),
        Ok(())
    );
    let SensoryMsg::Semantic { features, .. } = msg else {
        panic!("expected semantic heartbeat");
    };
    assert_eq!(features, expected_delivered);

    let status = read_status(&semantic_heartbeat_status_path_for_profile(&path));
    let review = status
        .get("signal_texture_comparison_v1")
        .expect("signal/texture comparison");
    assert_eq!(
        review
            .get("window_texture_context_sample_count")
            .and_then(Value::as_u64),
        Some(1)
    );
    assert_eq!(
        review
            .get("phase_zero_comparison_state")
            .and_then(Value::as_str),
        Some("available")
    );
    assert_eq!(
        review
            .pointer("/persistence_check_v1/state")
            .and_then(Value::as_str),
        Some("no_rescue_skips_observed_in_window")
    );
    let comparison = review
        .get("latest_phase_zero_comparison_v1")
        .expect("phase-zero comparison");
    assert_eq!(
        comparison.get("comparison_state").and_then(Value::as_str),
        Some("dense_viscous_high_entropy_field_with_bounded_heartbeat_signal_observed")
    );
    assert_eq!(
        comparison
            .get("spectral_code_mismatch_state")
            .and_then(Value::as_str),
        Some("not_established_cross_domain_texture_decoder_absent")
    );
    assert_eq!(
        comparison
            .get("texture_marker_test_state")
            .and_then(Value::as_str),
        Some("raw_heartbeat_features_have_no_authoritative_viscosity_marker_decoder")
    );
    assert_eq!(
        comparison
            .pointer("/minime_observation_v1/primary_texture")
            .and_then(Value::as_str),
        Some("overpacked_viscous")
    );
    assert!(
        comparison
            .pointer("/minime_observation_v1/resonance_density")
            .and_then(Value::as_f64)
            .is_some_and(|value| (value - 0.86).abs() < 1.0e-6)
    );
    assert!(
        comparison
            .pointer("/bridge_derivation_v1/lambda1_abs_share")
            .and_then(Value::as_f64)
            .is_some_and(|value| (value - 0.33).abs() < 1.0e-6)
    );
    assert!(
        comparison
            .pointer("/bridge_derivation_v1/lambda1_lambda2_abs_ratio")
            .and_then(Value::as_f64)
            .is_some_and(|value| (value - 1.54).abs() < 1.0e-5)
    );
    assert_eq!(
        review
            .get("runtime_effect_applied")
            .and_then(Value::as_bool),
        Some(false)
    );
    assert_eq!(
        review.get("authority").and_then(Value::as_str),
        Some(
            "read_only_signal_to_field_comparison_not_vector_cadence_intensity_shaping_rescue_dispatch_or_control"
        )
    );
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn semantic_heartbeat_signal_texture_comparison_does_not_force_dense_field_language() {
    let dir = unique_temp_dir("semantic_heartbeat_signal_texture_low_density");
    let path = dir.join("rescue_profile.json");
    std::fs::write(&path, budgeted_sovereignty_profile_json()).unwrap();
    write_v2_health_with_heartbeat_context(&dir, 61.5, "hold", 65.0, 0.08, 0.48, 0.12);
    let telemetry = heartbeat_texture_telemetry(0.42, 0.48, 0.08, 0.25, 0.31, 0.02, "open_humid");
    let generated = vec![0.1, -0.2, 0.3, -0.4];
    let observation =
        SemanticHeartbeatObservationV1::new("steady_semantic_heartbeat", 1, 1.0 / 64.0, 7, 0.30)
            .with_signal_evidence("steady_warmth", false, &generated, None)
            .with_minime_texture_context(Some(&telemetry));
    let mut msg = SensoryMsg::Semantic {
        features: generated,
        ts_ms: None,
    };

    assert_eq!(
        prepare_semantic_heartbeat_for_path_with_observation(&mut msg, &path, observation),
        Ok(())
    );
    let status = read_status(&semantic_heartbeat_status_path_for_profile(&path));
    let comparison = status
        .pointer("/signal_texture_comparison_v1/latest_comparison_v1")
        .expect("latest signal/texture comparison");
    assert_eq!(
        comparison.get("comparison_state").and_then(Value::as_str),
        Some("signal_and_field_context_available_without_dense_viscous_high_entropy_convergence")
    );
    assert_eq!(
        comparison
            .get("dense_field_observed")
            .and_then(Value::as_bool),
        Some(false)
    );
    assert_eq!(
        comparison
            .get("high_entropy_field_observed")
            .and_then(Value::as_bool),
        Some(false)
    );
    assert_eq!(
        comparison
            .get("viscous_field_observed")
            .and_then(Value::as_bool),
        Some(false)
    );
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn semantic_heartbeat_legacy_observation_keeps_texture_context_optional() {
    let dir = unique_temp_dir("semantic_heartbeat_legacy_texture_context");
    let path = dir.join("rescue_profile.json");
    std::fs::write(&path, budgeted_sovereignty_profile_json()).unwrap();
    write_v2_health(&dir, 61.5, "hold", 65.0, false, 0.0, 12, 12);
    let generated = vec![0.1, -0.2, 0.3, -0.4];
    let observation =
        heartbeat_observation().with_signal_evidence("steady_warmth", false, &generated, None);
    let mut msg = SensoryMsg::Semantic {
        features: generated,
        ts_ms: None,
    };

    assert_eq!(
        prepare_semantic_heartbeat_for_path_with_observation(&mut msg, &path, observation),
        Ok(())
    );
    let status = read_status(&semantic_heartbeat_status_path_for_profile(&path));
    let review = status
        .get("signal_texture_comparison_v1")
        .expect("signal/texture comparison");
    assert_eq!(
        review.get("comparison_state").and_then(Value::as_str),
        Some("minime_field_context_unavailable")
    );
    assert_eq!(
        review
            .get("phase_zero_comparison_state")
            .and_then(Value::as_str),
        Some("awaiting_steady_heartbeat_phase_zero_sample")
    );
    assert!(
        review
            .get("latest_comparison_v1")
            .is_some_and(Value::is_null)
    );
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn blocked_semantic_heartbeat_keeps_generated_evidence_without_claiming_delivery() {
    let dir = unique_temp_dir("semantic_heartbeat_blocked_signal_evidence");
    let path = dir.join("rescue_profile.json");
    std::fs::write(&path, budgeted_sovereignty_profile_json()).unwrap();
    write_v2_health_with_heartbeat_context(&dir, 79.0, "elevated", 79.0, 0.72, 0.92, 0.44);
    let generated = vec![0.1, -0.2, 0.3, -0.4];
    let observation =
        heartbeat_observation().with_signal_evidence("steady_warmth", false, &generated, None);
    let mut msg = SensoryMsg::Semantic {
        features: generated,
        ts_ms: None,
    };

    assert!(
        prepare_semantic_heartbeat_for_path_with_observation(&mut msg, &path, observation,)
            .unwrap_err()
            .contains("60s peak guard")
    );

    let status = read_status(&semantic_heartbeat_status_path_for_profile(&path));
    let signal = status
        .get("window_samples_v1")
        .and_then(Value::as_array)
        .and_then(|samples| samples.first())
        .and_then(|sample| sample.get("signal_evidence_v1"))
        .unwrap();
    assert!(signal.get("generated").is_some());
    assert!(signal.get("delivered").is_some_and(Value::is_null));
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn semantic_heartbeat_observation_window_prunes_only_expired_samples() {
    let dir = unique_temp_dir("semantic_heartbeat_rolling_observation");
    let path = dir.join("rescue_profile.json");
    std::fs::write(&path, budgeted_sovereignty_profile_json()).unwrap();
    write_v2_health(&dir, 64.0, "hold", 65.0, true, 0.006, 12, 12);
    let now = now_unix_s();
    write_status(
        &semantic_heartbeat_status_path_for_profile(&path),
        &json!({
            "send_count": 4,
            "block_count": 2,
            "attempt_count": 6,
            "window_samples_v1": [
                {"at_unix_s": now - 601.0, "outcome": "blocked"},
                {"at_unix_s": now - 599.0, "outcome": "sent"}
            ]
        }),
    );
    let mut msg = semantic_msg();

    assert_eq!(
        prepare_semantic_heartbeat_for_path_with_observation(
            &mut msg,
            &path,
            heartbeat_observation(),
        ),
        Ok(())
    );

    let status = read_status(&semantic_heartbeat_status_path_for_profile(&path));
    assert_eq!(status.get("attempt_count").and_then(Value::as_u64), Some(7));
    assert_eq!(status.get("send_count").and_then(Value::as_u64), Some(5));
    assert_eq!(status.get("block_count").and_then(Value::as_u64), Some(2));
    assert_eq!(
        status.get("window_attempt_count").and_then(Value::as_u64),
        Some(2)
    );
    assert_eq!(
        status.get("window_send_count").and_then(Value::as_u64),
        Some(2)
    );
    assert_eq!(
        status.get("window_block_count").and_then(Value::as_u64),
        Some(0)
    );
    assert_eq!(
        status.get("window_skip_rate").and_then(Value::as_f64),
        Some(0.0)
    );
    assert_eq!(
        status
            .get("window_samples_v1")
            .and_then(Value::as_array)
            .map(Vec::len),
        Some(2)
    );
    assert_eq!(
        status
            .get("window_samples_truncated")
            .and_then(Value::as_bool),
        Some(false)
    );
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn semantic_heartbeat_can_refresh_active_semantic_lane() {
    let dir = unique_temp_dir("semantic_heartbeat_active_lane");
    let path = dir.join("rescue_profile.json");
    std::fs::write(&path, budgeted_sovereignty_profile_json()).unwrap();
    write_v2_health(&dir, 64.0, "hold", 65.0, true, 0.006, 12, 12);
    let mut msg = semantic_msg();

    assert_eq!(
        prepare_semantic_heartbeat_for_path_with_observation(
            &mut msg,
            &path,
            heartbeat_observation(),
        ),
        Ok(())
    );
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn semantic_heartbeat_blocks_hot_or_discharge_states() {
    let dir = unique_temp_dir("semantic_heartbeat_hot");
    let path = dir.join("rescue_profile.json");
    std::fs::write(&path, budgeted_sovereignty_profile_json()).unwrap();
    write_v2_health(&dir, 79.0, "elevated", 79.0, false, 0.0, 12, 12);
    let mut msg = semantic_msg();

    let reason = prepare_semantic_heartbeat_for_path_with_observation(
        &mut msg,
        &path,
        heartbeat_observation(),
    )
    .unwrap_err();
    assert!(reason.contains("60s peak guard"), "{reason}");

    write_v2_health(&dir, 65.0, "discharge", 65.0, false, 0.0, 12, 12);
    let mut msg = semantic_msg();
    let reason = prepare_semantic_heartbeat_for_path_with_observation(
        &mut msg,
        &path,
        heartbeat_observation(),
    )
    .unwrap_err();
    assert!(reason.contains("during discharge"), "{reason}");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn budgeted_sovereignty_does_not_rollback_on_transient_discharge_below_threshold() {
    let dir = unique_temp_dir("budgeted_sovereignty_discharge_no_rollback");
    let path = dir.join("rescue_profile.json");
    std::fs::write(&path, budgeted_sovereignty_profile_json()).unwrap();
    let status_path = limited_write_status_path_for_profile(&path);
    write_status(
        &status_path,
        &json!({
            "profile": "bridge_budgeted_sovereignty_v1",
            "policy_version": 2,
            "send_count": 1,
            "last_sent_at_unix_s": now_unix_s() - 10.0,
            "last_sent_fill_pct": 64.0
        }),
    );
    write_v2_health(&dir, 78.0, "discharge", 78.0, false, 0.0, 12, 12);

    let mut msg = semantic_msg();
    let reason = prepare_semantic_write_for_path(
        &mut msg,
        &path,
        &v2_context(
            "I am watching the hot arc without forcing it.",
            "witness",
            78.0,
        ),
    )
    .unwrap_err();
    assert!(reason.contains("cooldown active"), "{reason}");
    let profile: Value = serde_json::from_str(&std::fs::read_to_string(&path).unwrap()).unwrap();
    assert_eq!(
        profile.get("profile").and_then(Value::as_str),
        Some("bridge_budgeted_sovereignty_v1")
    );
    assert!(profile.get("rollback_reason").is_none());
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
