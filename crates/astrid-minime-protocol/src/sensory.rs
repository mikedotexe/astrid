use serde::{Deserialize, Serialize};

use crate::{CompatibilityStatus, ProtocolHeaderV1, classify_protocol, current_protocol};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SensoryPacketV1 {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub protocol: Option<ProtocolHeaderV1>,
    #[serde(flatten)]
    pub message: SensoryMsg,
}

impl SensoryPacketV1 {
    #[must_use]
    pub fn versioned(message: SensoryMsg) -> Self {
        Self {
            protocol: Some(current_protocol()),
            message,
        }
    }

    #[must_use]
    pub const fn legacy(message: SensoryMsg) -> Self {
        Self {
            protocol: None,
            message,
        }
    }

    #[must_use]
    pub fn compatibility(&self) -> CompatibilityStatus {
        classify_protocol(self.protocol.as_ref())
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "lowercase")]
pub enum SensoryMsg {
    Video {
        features: Vec<f32>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        ts_ms: Option<u64>,
    },
    Audio {
        features: Vec<f32>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        ts_ms: Option<u64>,
    },
    Aux {
        features: Vec<f32>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        ts_ms: Option<u64>,
    },
    Semantic {
        features: Vec<f32>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        ts_ms: Option<u64>,
    },
    #[serde(rename = "attractor_pulse")]
    AttractorPulse {
        intent_id: String,
        label: String,
        command: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        stage: Option<String>,
        #[serde(default)]
        features: Vec<f32>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        max_abs: Option<f32>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        duration_ticks: Option<u32>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        decay_ticks: Option<u32>,
    },
    #[serde(rename = "shadow_influence")]
    ShadowInfluence {
        intent_id: String,
        label: String,
        command: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        stage: Option<String>,
        #[serde(default)]
        features: Vec<f32>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        max_abs: Option<f32>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        duration_ticks: Option<u32>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        decay_ticks: Option<u32>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        basis: Option<String>,
    },
    Control {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        synth_gain: Option<f32>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        keep_bias: Option<f32>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        exploration_noise: Option<f32>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        fill_target: Option<f32>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        regulation_strength: Option<f32>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        smoothing_preference: Option<f32>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        geom_curiosity: Option<f32>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        target_lambda_bias: Option<f32>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        geom_drive: Option<f32>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        penalty_sensitivity: Option<f32>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        breathing_rate_scale: Option<f32>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        mem_mode: Option<u8>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        journal_resonance: Option<f32>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        checkpoint_interval: Option<f32>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        embedding_strength: Option<f32>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        memory_decay_rate: Option<f32>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        transition_cushion: Option<f32>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        checkpoint_annotation: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        deep_breathing: Option<bool>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        synth_noise_level: Option<f32>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pure_tone: Option<bool>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        legacy_audio_synth: Option<bool>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        legacy_video_synth: Option<bool>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        live_audio_enabled: Option<bool>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        live_video_enabled: Option<bool>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pi_kp: Option<f32>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pi_ki: Option<f32>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pi_max_step: Option<f32>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pi_geom_weight: Option<f32>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pi_integrator_leak: Option<f32>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        esn_leak_override: Option<f32>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        esn_leak_override_ticks: Option<u32>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        esn_leak_authority_request_id: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        mode_disperse: Option<f32>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        mode_disperse_duration_ticks: Option<u32>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        mode_disperse_decay_ticks: Option<u32>,
    },
}
