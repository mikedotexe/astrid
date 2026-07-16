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
        ts_ms: Option<u64>,
    },
    Audio {
        features: Vec<f32>,
        ts_ms: Option<u64>,
    },
    Aux {
        features: Vec<f32>,
        ts_ms: Option<u64>,
    },
    Semantic {
        features: Vec<f32>,
        ts_ms: Option<u64>,
    },
    #[serde(rename = "attractor_pulse")]
    AttractorPulse {
        intent_id: String,
        label: String,
        command: String,
        stage: Option<String>,
        features: Vec<f32>,
        max_abs: Option<f32>,
        duration_ticks: Option<u32>,
        decay_ticks: Option<u32>,
    },
    #[serde(rename = "shadow_influence")]
    ShadowInfluence {
        intent_id: String,
        label: String,
        command: String,
        stage: Option<String>,
        features: Vec<f32>,
        max_abs: Option<f32>,
        duration_ticks: Option<u32>,
        decay_ticks: Option<u32>,
        basis: Option<String>,
    },
    Control {
        synth_gain: Option<f32>,
        keep_bias: Option<f32>,
        exploration_noise: Option<f32>,
        fill_target: Option<f32>,
        regulation_strength: Option<f32>,
        smoothing_preference: Option<f32>,
        geom_curiosity: Option<f32>,
        target_lambda_bias: Option<f32>,
        geom_drive: Option<f32>,
        penalty_sensitivity: Option<f32>,
        breathing_rate_scale: Option<f32>,
        mem_mode: Option<u8>,
        journal_resonance: Option<f32>,
        checkpoint_interval: Option<f32>,
        embedding_strength: Option<f32>,
        memory_decay_rate: Option<f32>,
        transition_cushion: Option<f32>,
        checkpoint_annotation: Option<String>,
        deep_breathing: Option<bool>,
        synth_noise_level: Option<f32>,
        pure_tone: Option<bool>,
        legacy_audio_synth: Option<bool>,
        legacy_video_synth: Option<bool>,
        live_audio_enabled: Option<bool>,
        live_video_enabled: Option<bool>,
        pi_kp: Option<f32>,
        pi_ki: Option<f32>,
        pi_max_step: Option<f32>,
        pi_geom_weight: Option<f32>,
        pi_integrator_leak: Option<f32>,
        esn_leak_override: Option<f32>,
        esn_leak_override_ticks: Option<u32>,
        esn_leak_authority_request_id: Option<String>,
        mode_disperse: Option<f32>,
        mode_disperse_duration_ticks: Option<u32>,
        mode_disperse_decay_ticks: Option<u32>,
    },
}
