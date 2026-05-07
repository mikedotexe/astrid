use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use serde_json::{Value, json};

use crate::paths::bridge_paths;
use crate::types::SensoryMsg;

#[path = "rescue_policy_text.rs"]
mod text;
#[path = "rescue_policy_value.rs"]
mod value_fields;

use self::text::{
    contains_structural_dump_language, default_limited_write_allowed_modes,
    default_limited_write_block_terms, default_limited_write_v2_allowed_modes,
    looks_like_dampen_or_inquiry, looks_like_limited_write_v2_text,
};
use self::value_fields::{bool_field, f32_field, string_array_field, string_field, u64_field};

const LIMITED_WRITE_PROFILE: &str = "limited_dampen_inquiry";
const LIMITED_WRITE_PROFILE_V2: &str = "limited_dampen_inquiry_v2";
const BUDGETED_SOVEREIGNTY_PROFILE: &str = "budgeted_sovereignty_v1";
const FULL_EXPRESSION_PROFILE: &str = "full_expression_v1";
const LIMITED_WRITE_STATUS_FILE: &str = "bridge_limited_write_status.json";
const SEMANTIC_HEARTBEAT_STATUS_FILE: &str = "bridge_semantic_heartbeat_status.json";
const LIMITED_WRITE_SENSORY_MUTE_FILE: &str = "stable_core_sensory_mute.json";
pub(crate) const AUTONOMOUS_LIMITED_WRITE_SOURCE: &str = "autonomous_main_chunk";
pub(crate) const MCP_LIMITED_WRITE_SOURCE: &str = "mcp_tool";
const LIMITED_WRITE_SOURCE: &str = AUTONOMOUS_LIMITED_WRITE_SOURCE;
const OBSERVE_ONLY_PROFILE: &str = "bridge_observe_only";
const V2_SEMANTIC_ENERGY_MAX: f32 = 0.02;
const V2_ROLLBACK_SEMANTIC_ENERGY: f32 = 0.05;
const V2_ADVERSE_WINDOW_SECS: f64 = 3600.0;
const SEMANTIC_HEARTBEAT_FEATURE_SCALE: f32 = 0.025;
const SEMANTIC_HEARTBEAT_MAX_ABS: f32 = 0.018;
pub const STABLE_CORE_TARGET_FILL_PCT: f64 = 68.0;

#[derive(Debug, Clone, PartialEq)]
struct RescueBridgePolicy {
    profile_name: String,
    bridge_enabled: bool,
    bridge_write_enabled: bool,
    bridge_autonomous_enabled: bool,
    bridge_write_profile: String,
    limited_write_enabled: bool,
    limited_write_policy_version: u64,
    limited_write_cooldown_secs: u64,
    limited_write_feature_scale: f32,
    limited_write_max_abs: f32,
    limited_write_min_fill_pct: f32,
    limited_write_max_fill_pct: f32,
    limited_write_rising_epsilon_pct: f32,
    limited_write_semantic_energy_rising_epsilon_pct: f32,
    limited_write_rollback_semantic_energy: f32,
    limited_write_health_max_age_secs: u64,
    limited_write_peak_fill_max_pct: f32,
    limited_write_required_stage: Option<String>,
    limited_write_allowed_stages: Vec<String>,
    limited_write_post_send_eval_secs: u64,
    limited_write_adverse_fill_rise_pct: f32,
    limited_write_adverse_cooldown_secs: u64,
    limited_write_rollback_target: Option<String>,
    limited_write_rollback_fill_pct: f32,
    limited_write_rollback_adverse_count: u64,
    limited_write_rollback_on_elevated_peak: bool,
    limited_write_require_zero_live_divisors: bool,
    limited_write_require_dampen_inquiry_text: bool,
    limited_write_block_structural_dump_language: bool,
    limited_write_block_terms_always: bool,
    limited_write_block_terms_on_rising: bool,
    limited_write_mute_live_intake_secs: u64,
    limited_write_pre_mute_live_intake_secs: u64,
    limited_write_require_pre_muted_live_intake: bool,
    limited_write_block_terms: Vec<String>,
    limited_write_allowed_modes: Vec<String>,
}

#[derive(Debug, Clone, Copy)]
pub(crate) struct SemanticWriteContext<'a> {
    pub source: &'a str,
    pub mode: Option<&'a str>,
    pub text: Option<&'a str>,
    pub fill_pct: Option<f32>,
    pub previous_fill_pct: Option<f32>,
}

impl RescueBridgePolicy {
    fn from_value(value: &Value) -> Option<Self> {
        let object = value.as_object()?;
        let profile_name = object
            .get("profile")
            .and_then(Value::as_str)
            .unwrap_or("unknown")
            .to_string();
        let bridge_enabled = bool_field(value, "effective_bridge_enabled")
            .or_else(|| bool_field(value, "bridge_enabled"))
            .unwrap_or(true);
        let bridge_write_enabled = bool_field(value, "effective_bridge_write_enabled")
            .or_else(|| bool_field(value, "bridge_write_enabled"))
            .unwrap_or_else(|| {
                profile_name != "bridge_observe_only"
                    && profile_name != "bridge_telemetry_only"
                    && bridge_enabled
            });
        let bridge_autonomous_enabled = bool_field(value, "effective_bridge_autonomous_enabled")
            .or_else(|| bool_field(value, "bridge_autonomous_enabled"))
            .unwrap_or_else(|| profile_name != "bridge_telemetry_only" && bridge_enabled);
        let bridge_write_profile = string_field(value, "bridge_write_profile")
            .unwrap_or_else(|| "unrestricted".to_string());
        let is_limited_write_v2 = bridge_write_profile == LIMITED_WRITE_PROFILE_V2
            || bridge_write_profile == BUDGETED_SOVEREIGNTY_PROFILE
            || bridge_write_profile == FULL_EXPRESSION_PROFILE
            || profile_name == "bridge_limited_write_v2"
            || profile_name == "bridge_budgeted_sovereignty_v1"
            || profile_name == "bridge_full_expression_v1"
            || profile_name == "stable_core_v1";
        let limited_write_enabled = bool_field(value, "limited_write_enabled").unwrap_or(false)
            || bridge_write_profile == LIMITED_WRITE_PROFILE
            || is_limited_write_v2
            || profile_name == "bridge_limited_write";
        let inferred_policy_version = if is_limited_write_v2 {
            2
        } else if limited_write_enabled {
            1
        } else {
            0
        };
        let limited_write_required_stage = string_field(value, "limited_write_required_stage");
        let limited_write_allowed_stages =
            string_array_field(value, "limited_write_allowed_stages").unwrap_or_else(|| {
                limited_write_required_stage
                    .clone()
                    .map_or_else(|| vec!["hold".to_string()], |stage| vec![stage])
            });
        Some(Self {
            profile_name,
            bridge_enabled,
            bridge_write_enabled,
            bridge_autonomous_enabled,
            bridge_write_profile,
            limited_write_enabled,
            limited_write_policy_version: u64_field(value, "limited_write_policy_version")
                .unwrap_or(inferred_policy_version),
            limited_write_cooldown_secs: u64_field(value, "limited_write_cooldown_secs")
                .unwrap_or(300),
            limited_write_feature_scale: f32_field(value, "limited_write_feature_scale")
                .unwrap_or(0.08),
            limited_write_max_abs: f32_field(value, "limited_write_max_abs").unwrap_or(0.18),
            limited_write_min_fill_pct: f32_field(value, "limited_write_min_fill_pct")
                .unwrap_or(58.0),
            limited_write_max_fill_pct: f32_field(value, "limited_write_max_fill_pct")
                .unwrap_or(68.0),
            limited_write_rising_epsilon_pct: f32_field(value, "limited_write_rising_epsilon_pct")
                .unwrap_or(0.5),
            limited_write_semantic_energy_rising_epsilon_pct: f32_field(
                value,
                "limited_write_semantic_energy_rising_epsilon_pct",
            )
            .or_else(|| f32_field(value, "limited_write_rising_epsilon_pct"))
            .unwrap_or(0.5),
            limited_write_rollback_semantic_energy: f32_field(
                value,
                "limited_write_rollback_semantic_energy",
            )
            .unwrap_or(V2_ROLLBACK_SEMANTIC_ENERGY),
            limited_write_health_max_age_secs: u64_field(
                value,
                "limited_write_health_max_age_secs",
            )
            .unwrap_or(5),
            limited_write_peak_fill_max_pct: f32_field(value, "limited_write_peak_fill_max_pct")
                .unwrap_or(68.0),
            limited_write_required_stage,
            limited_write_allowed_stages,
            limited_write_post_send_eval_secs: u64_field(
                value,
                "limited_write_post_send_eval_secs",
            )
            .unwrap_or(120),
            limited_write_adverse_fill_rise_pct: f32_field(
                value,
                "limited_write_adverse_fill_rise_pct",
            )
            .unwrap_or(3.0),
            limited_write_adverse_cooldown_secs: u64_field(
                value,
                "limited_write_adverse_cooldown_secs",
            )
            .unwrap_or(1800),
            limited_write_rollback_target: string_field(value, "limited_write_rollback_target"),
            limited_write_rollback_fill_pct: f32_field(value, "limited_write_rollback_fill_pct")
                .unwrap_or(74.0),
            limited_write_rollback_adverse_count: u64_field(
                value,
                "limited_write_rollback_adverse_count",
            )
            .unwrap_or(2),
            limited_write_rollback_on_elevated_peak: bool_field(
                value,
                "limited_write_rollback_on_elevated_peak",
            )
            .unwrap_or(true),
            limited_write_require_zero_live_divisors: bool_field(
                value,
                "limited_write_require_zero_live_divisors",
            )
            .unwrap_or(true),
            limited_write_require_dampen_inquiry_text: bool_field(
                value,
                "limited_write_require_dampen_inquiry_text",
            )
            .unwrap_or(true),
            limited_write_block_structural_dump_language: bool_field(
                value,
                "limited_write_block_structural_dump_language",
            )
            .unwrap_or(true),
            limited_write_block_terms_always: bool_field(value, "limited_write_block_terms_always")
                .unwrap_or(false),
            limited_write_block_terms_on_rising: bool_field(
                value,
                "limited_write_block_terms_on_rising",
            )
            .unwrap_or(true),
            limited_write_mute_live_intake_secs: u64_field(
                value,
                "limited_write_mute_live_intake_secs",
            )
            .unwrap_or(0),
            limited_write_pre_mute_live_intake_secs: u64_field(
                value,
                "limited_write_pre_mute_live_intake_secs",
            )
            .unwrap_or(0),
            limited_write_require_pre_muted_live_intake: bool_field(
                value,
                "limited_write_require_pre_muted_live_intake",
            )
            .unwrap_or(false),
            limited_write_block_terms: string_array_field(value, "limited_write_block_terms")
                .unwrap_or_else(default_limited_write_block_terms),
            limited_write_allowed_modes: string_array_field(value, "limited_write_allowed_modes")
                .unwrap_or_else(|| {
                    if is_limited_write_v2 {
                        default_limited_write_v2_allowed_modes()
                    } else {
                        default_limited_write_allowed_modes()
                    }
                }),
        })
    }

    fn semantic_ingress_block_reason(&self) -> Option<String> {
        if !self.bridge_enabled {
            return Some(format!(
                "rescue profile '{}' has bridge ingress disabled",
                self.profile_name
            ));
        }
        if !self.bridge_write_enabled {
            return Some(format!(
                "rescue profile '{}' blocks semantic ingress",
                self.profile_name
            ));
        }
        if self.limited_write_active() {
            return Some(format!(
                "rescue profile '{}' requires the limited dampen/inquiry semantic gate",
                self.profile_name
            ));
        }
        None
    }

    fn autonomous_enabled(&self) -> bool {
        self.bridge_enabled && self.bridge_autonomous_enabled
    }

    fn sensory_connection_enabled(&self) -> bool {
        self.bridge_enabled && (self.bridge_write_enabled || self.bridge_autonomous_enabled)
    }

    fn limited_write_active(&self) -> bool {
        self.bridge_write_enabled
            && self.limited_write_enabled
            && matches!(
                self.bridge_write_profile.as_str(),
                LIMITED_WRITE_PROFILE
                    | LIMITED_WRITE_PROFILE_V2
                    | BUDGETED_SOVEREIGNTY_PROFILE
                    | FULL_EXPRESSION_PROFILE
            )
    }

    fn limited_write_v2_active(&self) -> bool {
        self.limited_write_active()
            && self.limited_write_policy_version == 2
            && matches!(
                self.bridge_write_profile.as_str(),
                LIMITED_WRITE_PROFILE_V2 | BUDGETED_SOVEREIGNTY_PROFILE | FULL_EXPRESSION_PROFILE
            )
    }

    fn limited_write_block_reason(
        &self,
        context: &SemanticWriteContext<'_>,
        profile_path: &Path,
        status_path: &Path,
    ) -> Option<String> {
        if !self.bridge_enabled {
            return Some(format!(
                "rescue profile '{}' has bridge ingress disabled",
                self.profile_name
            ));
        }
        if !self.bridge_write_enabled {
            return Some(format!(
                "rescue profile '{}' blocks semantic ingress",
                self.profile_name
            ));
        }
        if !self.limited_write_active() {
            return None;
        }
        if !limited_write_source_allowed(context.source) {
            return Some(format!(
                "limited-write profile only allows source '{LIMITED_WRITE_SOURCE}' or '{MCP_LIMITED_WRITE_SOURCE}'"
            ));
        }

        let now = now_unix_s();
        let mut status = read_status(status_path);
        let health = if self.limited_write_v2_active() {
            match load_limited_write_health(profile_path, self.limited_write_health_max_age_secs) {
                Ok(health) => {
                    if let Some(reason) = self.evaluate_v2_previous_send(
                        profile_path,
                        status_path,
                        &mut status,
                        &health,
                        now,
                    ) {
                        return Some(reason);
                    }
                    Some(health)
                },
                Err(reason) => return Some(reason),
            }
        } else {
            None
        };

        if let Some(reason) = self.cooldown_block_reason(&status, now) {
            return Some(reason);
        }

        let fill_pct = health
            .as_ref()
            .map_or(context.fill_pct, |health| Some(health.fill_pct));
        let Some(fill_pct) = fill_pct else {
            return Some("limited-write profile requires current fill".to_string());
        };
        if fill_pct < self.limited_write_min_fill_pct {
            return Some(format!(
                "limited-write profile blocks semantic ingress below {:.1}% fill",
                self.limited_write_min_fill_pct
            ));
        }
        if fill_pct > self.limited_write_max_fill_pct {
            return Some(format!(
                "limited-write profile blocks semantic ingress above {:.1}% fill",
                self.limited_write_max_fill_pct
            ));
        }

        let mode = context.mode.unwrap_or_default();
        if !mode.is_empty()
            && !self
                .limited_write_allowed_modes
                .iter()
                .any(|allowed| allowed == mode)
        {
            return Some(format!(
                "limited-write profile blocks mode '{mode}' outside dampen/inquiry lane"
            ));
        }

        let text = context.text.unwrap_or_default();
        let lower = text.to_lowercase();
        if self.limited_write_v2_active() {
            if self.limited_write_block_terms_always {
                if let Some(term) = self
                    .limited_write_block_terms
                    .iter()
                    .find(|term| lower.contains(&term.to_lowercase()))
                {
                    return Some(format!(
                        "limited-write profile blocks trigger language '{term}'"
                    ));
                }
            }
            if let Some(reason) = self.v2_health_block_reason(context, health.as_ref()?) {
                return Some(reason);
            }
            if self.limited_write_block_structural_dump_language
                && contains_structural_dump_language(text)
            {
                return Some(
                    "limited-write v2 blocks structural spectral dump language".to_string(),
                );
            }
            if self.limited_write_require_dampen_inquiry_text
                && !looks_like_limited_write_v2_text(text, mode)
            {
                return Some(
                    "limited-write v2 allows only dampening or inquiry-shaped text".to_string(),
                );
            }
            if self.limited_write_require_pre_muted_live_intake {
                let health = health.as_ref()?;
                if !health.semantic_mute_active
                    || health.live_audio_divisor != 0
                    || health.live_video_divisor != 0
                {
                    write_limited_write_sensory_mute(
                        status_path,
                        self,
                        now,
                        self.limited_write_pre_mute_live_intake_secs
                            .max(self.limited_write_mute_live_intake_secs),
                        "limited_write_pre_mute_before_semantic_send",
                    );
                    return Some(
                        "limited-write v2 pre-muted live audio/video before semantic send"
                            .to_string(),
                    );
                }
            }
        } else if !looks_like_dampen_or_inquiry(text, mode) {
            return Some(
                "limited-write profile allows only dampening or inquiry-shaped text".to_string(),
            );
        }

        if self.limited_write_block_terms_always {
            if let Some(term) = self
                .limited_write_block_terms
                .iter()
                .find(|term| lower.contains(&term.to_lowercase()))
            {
                return Some(format!(
                    "limited-write profile blocks trigger language '{term}'"
                ));
            }
        }

        let fill_rising = context
            .previous_fill_pct
            .is_some_and(|previous| fill_pct - previous > self.limited_write_rising_epsilon_pct);
        if fill_rising && self.limited_write_block_terms_on_rising {
            if let Some(term) = self
                .limited_write_block_terms
                .iter()
                .find(|term| lower.contains(&term.to_lowercase()))
            {
                return Some(format!(
                    "limited-write profile blocks rising-fill trigger language '{term}'"
                ));
            }
        }

        None
    }

    fn apply_limited_write_shape(&self, features: &mut [f32]) {
        let scale = self.limited_write_feature_scale.clamp(0.0, 1.0);
        let max_abs = self.limited_write_max_abs.clamp(0.0, 5.0);
        for feature in features {
            *feature = (*feature * scale).clamp(-max_abs, max_abs);
        }
    }

    fn heartbeat_block_reason(&self, profile_path: &Path) -> Option<String> {
        if !self.bridge_enabled {
            return Some(format!(
                "rescue profile '{}' has bridge ingress disabled",
                self.profile_name
            ));
        }
        if !self.bridge_write_enabled && !self.bridge_autonomous_enabled {
            return Some(format!(
                "rescue profile '{}' blocks semantic heartbeat ingress",
                self.profile_name
            ));
        }
        if !self.limited_write_v2_active() {
            return None;
        }

        let health =
            match load_limited_write_health(profile_path, self.limited_write_health_max_age_secs) {
                Ok(health) => health,
                Err(reason) => return Some(reason),
            };
        if health.semantic_mute_active {
            return Some("semantic heartbeat blocked while semantic mute is active".to_string());
        }
        if health.stage == "discharge" {
            return Some("semantic heartbeat blocked during discharge".to_string());
        }
        if health.fill_pct >= self.limited_write_peak_fill_max_pct
            || health.peak_fill_pct_60s >= self.limited_write_peak_fill_max_pct
        {
            return Some(format!(
                "semantic heartbeat blocked when 60s peak guard is {:.1}% or higher",
                self.limited_write_peak_fill_max_pct
            ));
        }
        if let Some(watchdog_state) = health.watchdog_state.as_deref() {
            if !(watchdog_state == "monitoring"
                || watchdog_state == "warmup"
                || watchdog_state == "monitoring:degraded")
            {
                return Some(format!(
                    "semantic heartbeat blocked by watchdog state '{watchdog_state}'"
                ));
            }
        }
        None
    }

    fn apply_semantic_heartbeat_shape(features: &mut [f32]) {
        for feature in features {
            *feature = (*feature * SEMANTIC_HEARTBEAT_FEATURE_SCALE)
                .clamp(-SEMANTIC_HEARTBEAT_MAX_ABS, SEMANTIC_HEARTBEAT_MAX_ABS);
        }
    }

    fn cooldown_block_reason(&self, status: &Value, now: f64) -> Option<String> {
        if !status_matches_policy(status, self) {
            return None;
        }
        if self.limited_write_v2_active() {
            if let Some(cooldown_until) =
                status.get("cooldown_until_unix_s").and_then(Value::as_f64)
            {
                let cooldown_remaining = cooldown_until - now;
                if cooldown_remaining > 0.0 {
                    return Some(format!(
                        "limited-write cooldown active for {:.0}s",
                        cooldown_remaining.ceil()
                    ));
                }
            }
        }
        if let Some(last_sent_at) = status.get("last_sent_at_unix_s").and_then(Value::as_f64) {
            let cooldown_remaining = last_sent_at + self.limited_write_cooldown_secs as f64 - now;
            if cooldown_remaining > 0.0 {
                return Some(format!(
                    "limited-write cooldown active for {:.0}s",
                    cooldown_remaining.ceil()
                ));
            }
        }
        None
    }

    fn v2_health_block_reason(
        &self,
        context: &SemanticWriteContext<'_>,
        health: &LimitedWriteHealth,
    ) -> Option<String> {
        if let Some(watchdog_state) = health.watchdog_state.as_deref() {
            if watchdog_state != "monitoring" {
                return Some(format!(
                    "limited-write v2 requires watchdog monitoring; saw '{watchdog_state}'"
                ));
            }
        }
        if !self
            .limited_write_allowed_stages
            .iter()
            .any(|stage| stage == &health.stage)
        {
            let required_stage = self
                .limited_write_required_stage
                .as_deref()
                .unwrap_or("hold");
            return Some(format!(
                "limited-write v2 requires rescue stage '{required_stage}'"
            ));
        }
        if health.peak_fill_pct_60s >= self.limited_write_peak_fill_max_pct {
            return Some(format!(
                "limited-write v2 blocks semantic ingress when 60s peak is {:.1}% or higher",
                self.limited_write_peak_fill_max_pct
            ));
        }
        if health.semantic_active {
            return Some("limited-write v2 requires inactive semantic state".to_string());
        }
        if health.semantic_energy > V2_SEMANTIC_ENERGY_MAX {
            return Some(format!(
                "limited-write v2 blocks semantic ingress while semantic energy exceeds {:.2}",
                V2_SEMANTIC_ENERGY_MAX
            ));
        }
        if self.limited_write_require_zero_live_divisors
            && (health.live_audio_divisor != 0 || health.live_video_divisor != 0)
        {
            return Some(
                "limited-write v2 requires live audio/video divisors to remain zero".to_string(),
            );
        }
        let Some(previous_fill_pct) = context.previous_fill_pct else {
            return Some("limited-write v2 requires previous fill sample".to_string());
        };
        let fill_delta = health.fill_pct - previous_fill_pct;
        if fill_delta > self.limited_write_rising_epsilon_pct {
            return Some(format!(
                "limited-write v2 blocks rising fill delta {:.2}%",
                fill_delta
            ));
        }
        None
    }

    fn evaluate_v2_previous_send(
        &self,
        profile_path: &Path,
        status_path: &Path,
        status: &mut Value,
        health: &LimitedWriteHealth,
        now: f64,
    ) -> Option<String> {
        if !status_matches_policy(status, self) {
            return None;
        }
        let last_sent_at = status.get("last_sent_at_unix_s").and_then(Value::as_f64)?;
        let last_sent_fill_pct = status
            .get("last_sent_fill_pct")
            .and_then(Value::as_f64)
            .map(|value| value as f32)?;
        let eval_window = self.limited_write_post_send_eval_secs as f64;
        let elapsed = now - last_sent_at;
        let already_final = status
            .get("last_send_evaluation")
            .and_then(|value| value.get("sent_at_unix_s"))
            .and_then(Value::as_f64)
            .is_some_and(|sent_at| (sent_at - last_sent_at).abs() < f64::EPSILON)
            && status
                .get("last_send_evaluation")
                .and_then(|value| value.get("state"))
                .and_then(Value::as_str)
                .is_some_and(|state| matches!(state, "adverse" | "healthy"));

        if already_final && elapsed > eval_window {
            return None;
        }

        let fill_delta = health.fill_pct - last_sent_fill_pct;
        if elapsed <= eval_window {
            if let Some(watchdog_state) = health.watchdog_state.as_deref() {
                if watchdog_state != "monitoring" {
                    if matches!(watchdog_state, "warmup" | "monitoring:degraded") {
                        if !already_final {
                            status["last_send_evaluation"] = json!({
                                "state": "watching",
                                "sent_at_unix_s": last_sent_at,
                                "evaluated_at_unix_s": now,
                                "seconds_since_send": elapsed,
                                "health_fill_pct": health.fill_pct,
                                "watchdog_state": watchdog_state
                            });
                            write_status(status_path, status);
                        }
                        return Some(format!(
                            "limited-write v2 waiting for watchdog monitoring; saw '{watchdog_state}'"
                        ));
                    }
                    return self.rollback_v2(
                        profile_path,
                        status_path,
                        status,
                        &format!("post-write watchdog state became '{watchdog_state}'"),
                        now,
                    );
                }
            }
            if health.fill_pct >= self.limited_write_rollback_fill_pct {
                return self.rollback_v2(
                    profile_path,
                    status_path,
                    status,
                    &format!(
                        "post-write fill reached {:.1}% after limited-write v2 send",
                        health.fill_pct
                    ),
                    now,
                );
            }
            let rollback_stage = self.limited_write_rollback_on_elevated_peak
                && (health.stage == "discharge"
                    || (health.stage == "elevated"
                        && health.fill_pct >= self.limited_write_peak_fill_max_pct));
            if rollback_stage {
                return self.rollback_v2(
                    profile_path,
                    status_path,
                    status,
                    &format!(
                        "post-write rescue stage entered '{}' after limited-write v2 send",
                        health.stage
                    ),
                    now,
                );
            }
            if health.semantic_energy > self.limited_write_rollback_semantic_energy
                && fill_delta > self.limited_write_semantic_energy_rising_epsilon_pct
            {
                return self.rollback_v2(
                    profile_path,
                    status_path,
                    status,
                    "post-write semantic energy rose while fill was rising",
                    now,
                );
            }
            if already_final {
                return None;
            }
            if fill_delta >= self.limited_write_adverse_fill_rise_pct && !already_final {
                let adverse_count = increment_adverse_count(status, now);
                status["last_send_evaluation"] = json!({
                    "state": "adverse",
                    "sent_at_unix_s": last_sent_at,
                    "evaluated_at_unix_s": now,
                    "seconds_since_send": elapsed,
                    "fill_delta_pct": fill_delta,
                    "health_fill_pct": health.fill_pct,
                    "reason": "fill_rise"
                });
                status["cooldown_secs"] = json!(self.limited_write_adverse_cooldown_secs);
                status["cooldown_until_unix_s"] =
                    json!(now + self.limited_write_adverse_cooldown_secs as f64);
                write_status(status_path, status);
                if adverse_count >= self.limited_write_rollback_adverse_count {
                    return self.rollback_v2(
                        profile_path,
                        status_path,
                        status,
                        "limited-write v2 saw repeated adverse fill rises",
                        now,
                    );
                }
                return None;
            }

            status["last_send_evaluation"] = json!({
                "state": "watching",
                "sent_at_unix_s": last_sent_at,
                "evaluated_at_unix_s": now,
                "seconds_since_send": elapsed,
                "fill_delta_pct": fill_delta,
                "health_fill_pct": health.fill_pct
            });
            write_status(status_path, status);
        } else if !already_final {
            status["last_send_evaluation"] = json!({
                "state": "healthy",
                "sent_at_unix_s": last_sent_at,
                "evaluated_at_unix_s": now,
                "seconds_since_send": elapsed,
                "fill_delta_pct": fill_delta,
                "health_fill_pct": health.fill_pct
            });
            write_status(status_path, status);
        }
        None
    }

    fn rollback_v2(
        &self,
        profile_path: &Path,
        status_path: &Path,
        status: &mut Value,
        reason: &str,
        now: f64,
    ) -> Option<String> {
        let target = self
            .limited_write_rollback_target
            .as_deref()
            .unwrap_or(OBSERVE_ONLY_PROFILE);
        if let Err(error) = rollback_profile_to_observe_only(profile_path, target, reason, now) {
            return Some(format!("limited-write v2 rollback failed: {error}"));
        }
        status["rollback_at_unix_s"] = json!(now);
        status["rollback_reason"] = json!(reason);
        status["rolled_back_from_profile"] = json!(self.profile_name);
        status["rolled_back_to_profile"] = json!(target);
        status["last_block_reason"] = json!(format!("rolled back: {reason}"));
        write_status(status_path, status);
        Some(format!(
            "limited-write v2 rolled back to {target}: {reason}"
        ))
    }
}

#[derive(Debug, Clone)]
struct LimitedWriteHealth {
    fill_pct: f32,
    stage: String,
    peak_fill_pct_60s: f32,
    semantic_active: bool,
    semantic_energy: f32,
    live_audio_divisor: i64,
    live_video_divisor: i64,
    semantic_mute_active: bool,
    watchdog_state: Option<String>,
    age_secs: f64,
}

fn load_policy(path: &Path) -> Option<RescueBridgePolicy> {
    let payload = std::fs::read_to_string(path).ok()?;
    let value: Value = serde_json::from_str(&payload).ok()?;
    RescueBridgePolicy::from_value(&value)
}

fn health_path_for_profile(profile_path: &Path) -> PathBuf {
    profile_path
        .parent()
        .unwrap_or_else(|| Path::new("."))
        .join("health.json")
}

fn rescue_status_path_for_profile(profile_path: &Path) -> PathBuf {
    profile_path
        .parent()
        .unwrap_or_else(|| Path::new("."))
        .join("rescue_status.json")
}

fn load_limited_write_health(
    profile_path: &Path,
    max_age_secs: u64,
) -> Result<LimitedWriteHealth, String> {
    let health_path = health_path_for_profile(profile_path);
    let metadata = std::fs::metadata(&health_path)
        .map_err(|_| "limited-write v2 requires fresh health.json".to_string())?;
    let modified = metadata
        .modified()
        .map_err(|_| "limited-write v2 could not read health.json mtime".to_string())?;
    let age_secs = SystemTime::now()
        .duration_since(modified)
        .map_err(|_| "limited-write v2 health.json mtime is in the future".to_string())?
        .as_secs_f64();
    if age_secs > max_age_secs as f64 {
        return Err(format!(
            "limited-write v2 requires fresh health.json; age {:.1}s exceeds {max_age_secs}s",
            age_secs
        ));
    }

    let payload = std::fs::read_to_string(&health_path)
        .map_err(|_| "limited-write v2 could not read health.json".to_string())?;
    let value: Value = serde_json::from_str(&payload)
        .map_err(|_| "limited-write v2 could not parse health.json".to_string())?;
    let rescue_status_path = rescue_status_path_for_profile(profile_path);
    let watchdog_state = std::fs::read_to_string(&rescue_status_path)
        .ok()
        .and_then(|payload| serde_json::from_str::<Value>(&payload).ok())
        .and_then(|value| {
            value
                .get("watchdog_state")
                .and_then(Value::as_str)
                .map(ToString::to_string)
        });
    let fill_pct = f32_required(&value, &["fill_pct"])?;
    Ok(LimitedWriteHealth {
        fill_pct,
        stage: str_optional(&value, &["rescue", "stage"])
            .or_else(|| str_optional(&value, &["stable_core", "stage"]))
            .ok_or_else(|| {
                "limited-write v2 health.json missing rescue.stage or stable_core.stage".to_string()
            })?,
        peak_fill_pct_60s: f32_optional(&value, &["rescue", "peak_fill_pct_60s"])
            .or_else(|| f32_optional(&value, &["stable_core", "peak_fill_pct_60s"]))
            .unwrap_or(fill_pct),
        semantic_active: bool_optional(&value, &["semantic_energy_v1", "kernel_active"])
            .or_else(|| bool_optional(&value, &["semantic", "kernel_active"]))
            .or_else(|| bool_optional(&value, &["semantic", "active"]))
            .unwrap_or(false),
        semantic_energy: f32_optional(&value, &["semantic_energy_v1", "regulator_drive_energy"])
            .or_else(|| f32_optional(&value, &["semantic", "regulator_drive_energy"]))
            .or_else(|| f32_optional(&value, &["semantic", "kernel_energy"]))
            .or_else(|| f32_optional(&value, &["semantic", "energy"]))
            .unwrap_or(0.0),
        live_audio_divisor: i64_required(&value, &["sensory", "live_audio_divisor"])?,
        live_video_divisor: i64_required(&value, &["sensory", "live_video_divisor"])?,
        semantic_mute_active: bool_optional(
            &value,
            &["stable_core", "sensory_budget", "semantic_mute_active"],
        )
        .unwrap_or(false),
        watchdog_state,
        age_secs,
    })
}

fn value_at_path<'a>(value: &'a Value, path: &[&str]) -> Option<&'a Value> {
    let mut current = value;
    for segment in path {
        current = current.get(*segment)?;
    }
    Some(current)
}

fn f32_required(value: &Value, path: &[&str]) -> Result<f32, String> {
    f32_optional(value, path)
        .ok_or_else(|| format!("limited-write v2 health.json missing {}", path.join(".")))
}

fn i64_required(value: &Value, path: &[&str]) -> Result<i64, String> {
    value_at_path(value, path)
        .and_then(Value::as_i64)
        .ok_or_else(|| format!("limited-write v2 health.json missing {}", path.join(".")))
}

fn f32_optional(value: &Value, path: &[&str]) -> Option<f32> {
    value_at_path(value, path)
        .and_then(Value::as_f64)
        .map(|value| value as f32)
}

fn bool_optional(value: &Value, path: &[&str]) -> Option<bool> {
    value_at_path(value, path).and_then(Value::as_bool)
}

fn str_optional(value: &Value, path: &[&str]) -> Option<String> {
    value_at_path(value, path)
        .and_then(Value::as_str)
        .map(ToString::to_string)
}

fn limited_write_status_path_for_profile(profile_path: &Path) -> PathBuf {
    profile_path
        .parent()
        .unwrap_or_else(|| Path::new("."))
        .join("runtime")
        .join(LIMITED_WRITE_STATUS_FILE)
}

fn semantic_heartbeat_status_path_for_profile(profile_path: &Path) -> PathBuf {
    profile_path
        .parent()
        .unwrap_or_else(|| Path::new("."))
        .join("runtime")
        .join(SEMANTIC_HEARTBEAT_STATUS_FILE)
}

fn limited_write_sensory_mute_path_for_status(status_path: &Path) -> PathBuf {
    status_path
        .parent()
        .unwrap_or_else(|| Path::new("."))
        .join(LIMITED_WRITE_SENSORY_MUTE_FILE)
}

fn now_unix_s() -> f64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs_f64()
}

fn limited_write_source_allowed(source: &str) -> bool {
    matches!(source, LIMITED_WRITE_SOURCE | MCP_LIMITED_WRITE_SOURCE)
}

fn read_status(path: &Path) -> Value {
    std::fs::read_to_string(path)
        .ok()
        .and_then(|payload| serde_json::from_str(&payload).ok())
        .unwrap_or_else(|| json!({}))
}

fn write_status(path: &Path, status: &Value) {
    if let Some(parent) = path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    if let Ok(payload) = serde_json::to_string_pretty(status) {
        let _ = std::fs::write(path, payload);
    }
}

fn write_limited_write_sensory_mute(
    status_path: &Path,
    policy: &RescueBridgePolicy,
    now: f64,
    duration_secs: u64,
    reason: &str,
) -> Option<f64> {
    if duration_secs == 0 {
        return None;
    }
    let mute_until = now + duration_secs as f64;
    let mute_path = limited_write_sensory_mute_path_for_status(status_path);
    if let Some(parent) = mute_path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    let payload = json!({
        "active_until_unix_s": mute_until,
        "duration_secs": duration_secs,
        "reason": reason,
        "source_profile": policy.profile_name,
        "last_semantic_sent_at_unix_s": now,
    });
    if let Ok(pretty) = serde_json::to_string_pretty(&payload) {
        let _ = std::fs::write(mute_path, pretty);
    }
    Some(mute_until)
}

fn status_matches_policy(status: &Value, policy: &RescueBridgePolicy) -> bool {
    if status.get("profile").and_then(Value::as_str) != Some(policy.profile_name.as_str()) {
        return false;
    }
    match status.get("policy_version").and_then(Value::as_u64) {
        Some(version) => version == policy.limited_write_policy_version,
        None => policy.limited_write_policy_version <= 1,
    }
}

fn increment_adverse_count(status: &mut Value, now: f64) -> u64 {
    let window_started = status
        .get("adverse_window_started_at_unix_s")
        .and_then(Value::as_f64)
        .filter(|started| now - *started <= V2_ADVERSE_WINDOW_SECS)
        .unwrap_or(now);
    let previous_count = if (window_started - now).abs() < f64::EPSILON {
        0
    } else {
        status
            .get("adverse_response_count")
            .and_then(Value::as_u64)
            .unwrap_or(0)
    };
    let count = previous_count.saturating_add(1);
    status["adverse_window_started_at_unix_s"] = json!(window_started);
    status["adverse_response_count"] = json!(count);
    count
}

fn matched_watch_terms(text: &str, policy: &RescueBridgePolicy) -> Vec<String> {
    let lower = text.to_lowercase();
    policy
        .limited_write_block_terms
        .iter()
        .filter(|term| lower.contains(&term.to_lowercase()))
        .cloned()
        .collect()
}

fn rollback_profile_to_observe_only(
    profile_path: &Path,
    target: &str,
    reason: &str,
    now: f64,
) -> Result<(), String> {
    let payload = std::fs::read_to_string(profile_path)
        .map_err(|error| format!("read profile failed: {error}"))?;
    let mut profile: Value =
        serde_json::from_str(&payload).map_err(|error| format!("parse profile failed: {error}"))?;
    let runtime_dir = profile_path
        .parent()
        .unwrap_or_else(|| Path::new("."))
        .join("runtime");
    std::fs::create_dir_all(&runtime_dir)
        .map_err(|error| format!("create runtime dir failed: {error}"))?;
    let archive_path =
        runtime_dir.join(format!("bridge_limited_write_v2_rollback_{:.0}.json", now));
    let pretty_original = serde_json::to_string_pretty(&profile)
        .map_err(|error| format!("serialize rollback archive failed: {error}"))?;
    std::fs::write(&archive_path, pretty_original)
        .map_err(|error| format!("write rollback archive failed: {error}"))?;
    let rolled_back_from_profile = profile
        .get("profile")
        .and_then(Value::as_str)
        .unwrap_or("bridge_limited_write_v2")
        .to_string();

    let Some(object) = profile.as_object_mut() else {
        return Err("profile root is not a JSON object".to_string());
    };
    object.insert("profile".to_string(), json!(target));
    object.insert("bridge_enabled".to_string(), json!(true));
    object.insert("effective_bridge_enabled".to_string(), json!(true));
    object.insert("bridge_write_enabled".to_string(), json!(false));
    object.insert("effective_bridge_write_enabled".to_string(), json!(false));
    object.insert("bridge_autonomous_enabled".to_string(), json!(true));
    object.insert(
        "effective_bridge_autonomous_enabled".to_string(),
        json!(true),
    );
    object.insert("bridge_write_profile".to_string(), json!("observe_only"));
    object.insert("limited_write_enabled".to_string(), json!(false));
    object.insert(
        "rolled_back_from_profile".to_string(),
        json!(rolled_back_from_profile),
    );
    object.insert("rolled_back_to_profile".to_string(), json!(target));
    object.insert("rollback_reason".to_string(), json!(reason));
    object.insert("rollback_at_unix_s".to_string(), json!(now));

    let pretty = serde_json::to_string_pretty(&profile)
        .map_err(|error| format!("serialize rolled-back profile failed: {error}"))?;
    std::fs::write(profile_path, pretty)
        .map_err(|error| format!("write rolled-back profile failed: {error}"))
}

fn record_limited_write_block(path: &Path, policy: &RescueBridgePolicy, reason: &str) {
    let mut status = read_status(path);
    if !status.is_object() || !status_matches_policy(&status, policy) {
        status = json!({});
    }
    status["profile"] = json!(policy.profile_name);
    status["policy_version"] = json!(policy.limited_write_policy_version);
    status["last_block_at_unix_s"] = json!(now_unix_s());
    status["last_block_reason"] = json!(reason);
    write_status(path, &status);
}

fn record_semantic_heartbeat_block(path: &Path, policy: &RescueBridgePolicy, reason: &str) {
    let mut status = read_status(path);
    if !status.is_object() {
        status = json!({});
    }
    status["profile"] = json!(policy.profile_name);
    status["policy_version"] = json!(policy.limited_write_policy_version);
    status["last_block_at_unix_s"] = json!(now_unix_s());
    status["last_block_reason"] = json!(reason);
    write_status(path, &status);
}

fn record_semantic_heartbeat_sent(
    path: &Path,
    policy: &RescueBridgePolicy,
    health: Option<&LimitedWriteHealth>,
) {
    let mut status = read_status(path);
    if !status.is_object() {
        status = json!({});
    }
    let send_count = status
        .get("send_count")
        .and_then(Value::as_u64)
        .unwrap_or(0)
        .saturating_add(1);
    status["profile"] = json!(policy.profile_name);
    status["policy_version"] = json!(policy.limited_write_policy_version);
    status["send_count"] = json!(send_count);
    status["last_sent_at_unix_s"] = json!(now_unix_s());
    status["feature_scale"] = json!(SEMANTIC_HEARTBEAT_FEATURE_SCALE);
    status["max_abs"] = json!(SEMANTIC_HEARTBEAT_MAX_ABS);
    if let Some(health) = health {
        status["last_sent_fill_pct"] = json!(health.fill_pct);
        status["last_sent_stage"] = json!(health.stage);
        status["last_sent_health_age_secs"] = json!(health.age_secs);
    }
    write_status(path, &status);
}

fn record_limited_write_sent(
    path: &Path,
    policy: &RescueBridgePolicy,
    context: &SemanticWriteContext<'_>,
    health: Option<&LimitedWriteHealth>,
    cooldown_secs: u64,
) {
    let mut status = read_status(path);
    if !status.is_object() || !status_matches_policy(&status, policy) {
        status = json!({});
    }
    let send_count = status
        .get("send_count")
        .and_then(Value::as_u64)
        .unwrap_or(0)
        .saturating_add(1);
    let text_preview = context
        .text
        .unwrap_or_default()
        .chars()
        .take(160)
        .collect::<String>();
    let now = now_unix_s();
    status["profile"] = json!(policy.profile_name);
    status["policy_version"] = json!(policy.limited_write_policy_version);
    status["send_count"] = json!(send_count);
    status["last_sent_at_unix_s"] = json!(now);
    status["last_sent_source"] = json!(context.source);
    status["last_sent_mode"] = json!(context.mode.unwrap_or_default());
    status["last_sent_fill_pct"] =
        json!(health.map_or(context.fill_pct, |health| Some(health.fill_pct)));
    status["last_sent_previous_fill_pct"] = json!(context.previous_fill_pct);
    if let Some(health) = health {
        status["last_sent_stage"] = json!(health.stage);
        status["last_sent_peak_fill_pct_60s"] = json!(health.peak_fill_pct_60s);
        status["last_sent_semantic_energy"] = json!(health.semantic_energy);
        status["last_sent_health_age_secs"] = json!(health.age_secs);
    }
    status["last_sent_text_preview"] = json!(text_preview);
    status["last_sent_watch_terms"] = json!(matched_watch_terms(
        context.text.unwrap_or_default(),
        policy
    ));
    status["cooldown_secs"] = json!(cooldown_secs);
    status["cooldown_until_unix_s"] = json!(now + cooldown_secs as f64);
    if let Some(mute_until) = write_limited_write_sensory_mute(
        path,
        policy,
        now,
        policy.limited_write_mute_live_intake_secs,
        "limited_write_semantic_send",
    ) {
        status["live_intake_mute_secs"] = json!(policy.limited_write_mute_live_intake_secs);
        status["live_intake_mute_until_unix_s"] = json!(mute_until);
        status["live_intake_mute_file"] = json!(LIMITED_WRITE_SENSORY_MUTE_FILE);
    }
    if policy.limited_write_v2_active() {
        status["last_send_evaluation"] = json!({
            "state": "pending",
            "sent_at_unix_s": now
        });
    }
    status["last_block_reason"] = Value::Null;
    write_status(path, &status);
}

pub fn bridge_autonomous_enabled_for_path(path: &Path) -> bool {
    load_policy(path)
        .map(|policy| policy.autonomous_enabled())
        .unwrap_or(true)
}

pub fn bridge_autonomous_enabled() -> bool {
    let path = bridge_paths()
        .minime_workspace()
        .join("rescue_profile.json");
    bridge_autonomous_enabled_for_path(&path)
}

pub fn bridge_sensory_enabled_for_path(path: &Path) -> bool {
    load_policy(path)
        .map(|policy| policy.sensory_connection_enabled())
        .unwrap_or(true)
}

pub fn bridge_sensory_enabled() -> bool {
    let path = bridge_paths()
        .minime_workspace()
        .join("rescue_profile.json");
    bridge_sensory_enabled_for_path(&path)
}

pub(crate) fn semantic_write_block_reason_for_path(
    msg: &SensoryMsg,
    path: &Path,
) -> Option<String> {
    if !matches!(msg, SensoryMsg::Semantic { .. }) {
        return None;
    }
    load_policy(path)?.semantic_ingress_block_reason()
}

pub(crate) fn semantic_write_block_reason(msg: &SensoryMsg) -> Option<String> {
    let path = bridge_paths()
        .minime_workspace()
        .join("rescue_profile.json");
    semantic_write_block_reason_for_path(msg, &path)
}

pub(crate) fn prepare_semantic_write_for_path(
    msg: &mut SensoryMsg,
    path: &Path,
    context: &SemanticWriteContext<'_>,
) -> Result<(), String> {
    if !matches!(msg, SensoryMsg::Semantic { .. }) {
        return Ok(());
    }
    let Some(policy) = load_policy(path) else {
        return Ok(());
    };
    let status_path = limited_write_status_path_for_profile(path);
    if let Some(reason) = policy.limited_write_block_reason(context, path, &status_path) {
        record_limited_write_block(&status_path, &policy, &reason);
        return Err(reason);
    }
    if policy.limited_write_active() {
        let health = if policy.limited_write_v2_active() {
            Some(load_limited_write_health(
                path,
                policy.limited_write_health_max_age_secs,
            )?)
        } else {
            None
        };
        if let SensoryMsg::Semantic { features, .. } = msg {
            policy.apply_limited_write_shape(features);
        }
        record_limited_write_sent(
            &status_path,
            &policy,
            context,
            health.as_ref(),
            policy.limited_write_cooldown_secs,
        );
    }
    Ok(())
}

pub(crate) fn prepare_semantic_heartbeat_for_path(
    msg: &mut SensoryMsg,
    path: &Path,
) -> Result<(), String> {
    if !matches!(msg, SensoryMsg::Semantic { .. }) {
        return Ok(());
    }
    let Some(policy) = load_policy(path) else {
        return Ok(());
    };
    let status_path = semantic_heartbeat_status_path_for_profile(path);
    if let Some(reason) = policy.heartbeat_block_reason(path) {
        record_semantic_heartbeat_block(&status_path, &policy, &reason);
        return Err(reason);
    }
    let health = if policy.limited_write_v2_active() {
        load_limited_write_health(path, policy.limited_write_health_max_age_secs).ok()
    } else {
        None
    };
    if let SensoryMsg::Semantic { features, .. } = msg {
        RescueBridgePolicy::apply_semantic_heartbeat_shape(features);
    }
    record_semantic_heartbeat_sent(&status_path, &policy, health.as_ref());
    Ok(())
}

pub(crate) fn prepare_semantic_write(
    msg: &mut SensoryMsg,
    context: &SemanticWriteContext<'_>,
) -> Result<(), String> {
    let path = bridge_paths()
        .minime_workspace()
        .join("rescue_profile.json");
    prepare_semantic_write_for_path(msg, &path, context)
}

pub(crate) fn prepare_semantic_heartbeat(msg: &mut SensoryMsg) -> Result<(), String> {
    let path = bridge_paths()
        .minime_workspace()
        .join("rescue_profile.json");
    prepare_semantic_heartbeat_for_path(msg, &path)
}

#[cfg(test)]
#[path = "rescue_policy_tests.rs"]
mod rescue_policy_tests;
