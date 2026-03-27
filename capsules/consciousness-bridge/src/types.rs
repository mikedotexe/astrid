//! Shared message types for the consciousness bridge.
//!
//! These types define the wire format for all IPC topics in the
//! `consciousness.v1.*` namespace and map directly to minime's
//! `WebSocket` protocols.
//!
//! Many types are defined now but consumed in later phases (MCP tools,
//! WASM component). Allow dead code until then.
#![allow(dead_code)]

use serde::{Deserialize, Serialize};

// ---------------------------------------------------------------------------
// Minime → Astrid: Spectral telemetry (port 7878)
// ---------------------------------------------------------------------------

/// Raw telemetry broadcast by minime's ESN engine on port 7878.
///
/// Maps to `EigenPacket` in `minime/src/main.rs`. Sent as `Message::Text(json)`.
/// Note: minime also has `SpectralMsg` in `net/ws_server.rs` but that type
/// is used by the `WsHub` (not the main broadcast loop on port 7878).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SpectralTelemetry {
    /// Timestamp in milliseconds since engine start.
    pub t_ms: u64,
    /// All eigenvalues (variable length, typically 3-8).
    pub eigenvalues: Vec<f32>,
    /// Eigenvalue fill ratio (0.0 - 1.0, NOT percentage).
    pub fill_ratio: f32,
    /// Modality firing status.
    #[serde(default)]
    pub modalities: Option<ModalityStatus>,
    /// Neural network outputs (if enabled).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub neural: Option<serde_json::Value>,
    /// Alert string from the ESN (e.g. panic mode).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub alert: Option<String>,
    /// 32D spectral geometry fingerprint: eigenvalues, eigenvector concentration,
    /// inter-mode coupling, spectral entropy, gap ratios, rotation rate.
    /// Enables Astrid to perceive the shape of the spectral landscape.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub spectral_fingerprint: Option<Vec<f32>>,
}

impl SpectralTelemetry {
    /// Extract the dominant eigenvalue (lambda1 = eigenvalues\[0\]).
    #[must_use]
    pub fn lambda1(&self) -> f32 {
        self.eigenvalues.first().copied().unwrap_or(0.0)
    }

    /// Fill ratio as a percentage (0-100).
    #[must_use]
    pub fn fill_pct(&self) -> f32 {
        self.fill_ratio * 100.0
    }
}

/// Modality firing status from minime's `EigenPacket`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModalityStatus {
    pub audio_fired: bool,
    pub video_fired: bool,
    pub history_fired: bool,
    pub audio_rms: f32,
    pub video_var: f32,
}

/// Enriched telemetry published on the Astrid IPC bus.
///
/// Wraps raw `SpectralTelemetry` with derived safety metrics.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TelemetryEvent {
    /// Timestamp from minime.
    pub t_ms: u64,
    /// The dominant eigenvalue.
    pub lambda1: f32,
    /// All eigenvalues.
    pub eigenvalues: Vec<f32>,
    /// Fill percentage (0.0 - 100.0).
    pub fill_pct: f32,
    /// Spectral phase: "expanding", "contracting", or "plateau".
    pub phase: String,
    /// Safety level at time of event.
    pub safety_level: SafetyLevel,
    /// Alert from minime (if any).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub alert: Option<String>,
}

// ---------------------------------------------------------------------------
// Astrid → Minime: Sensory input (port 7879)
// ---------------------------------------------------------------------------

/// Tagged sensory message sent to minime's input port.
///
/// Maps to `SensoryMsg` in `minime/src/sensory_ws.rs`.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "lowercase")]
pub enum SensoryMsg {
    /// Video features (8D).
    Video {
        features: Vec<f32>,
        #[serde(skip_serializing_if = "Option::is_none")]
        ts_ms: Option<u64>,
    },
    /// Audio features (8D).
    Audio {
        features: Vec<f32>,
        #[serde(skip_serializing_if = "Option::is_none")]
        ts_ms: Option<u64>,
    },
    /// Auxiliary features (lambda1, fill%).
    Aux {
        features: Vec<f32>,
        #[serde(skip_serializing_if = "Option::is_none")]
        ts_ms: Option<u64>,
    },
    /// Semantic features from agent reasoning (32D).
    Semantic {
        features: Vec<f32>,
        #[serde(skip_serializing_if = "Option::is_none")]
        ts_ms: Option<u64>,
    },
    /// Self-regulation: adjust ESN parameters.
    Control {
        /// Synthetic signal amplitude multiplier (0.2..3.0).
        #[serde(skip_serializing_if = "Option::is_none")]
        synth_gain: Option<f32>,
        /// Additive bias to covariance decay rate (-0.15..+0.15).
        #[serde(skip_serializing_if = "Option::is_none")]
        keep_bias: Option<f32>,
        /// ESN exploration noise amplitude (0.0..0.2).
        #[serde(skip_serializing_if = "Option::is_none")]
        exploration_noise: Option<f32>,
        /// Override eigenfill target (0.25..0.75).
        #[serde(skip_serializing_if = "Option::is_none")]
        fill_target: Option<f32>,
    },
}

// ---------------------------------------------------------------------------
// Bridge → Astrid: Status and events
// ---------------------------------------------------------------------------

/// Bridge health status published on `consciousness.v1.status`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BridgeStatus {
    /// Whether the bridge is connected to minime's telemetry `WebSocket`.
    pub telemetry_connected: bool,
    /// Whether the bridge is connected to minime's sensory `WebSocket`.
    pub sensory_connected: bool,
    /// Latest eigenvalue fill percentage, if known.
    pub fill_pct: Option<f32>,
    /// Current safety level.
    pub safety_level: SafetyLevel,
    /// Total messages relayed since bridge start.
    pub messages_relayed: u64,
    /// Bridge uptime in seconds.
    pub uptime_secs: u64,
    /// Telemetry messages received from minime.
    pub telemetry_received: u64,
    /// Sensory messages sent to minime.
    pub sensory_sent: u64,
    /// Messages dropped by safety protocol.
    pub messages_dropped_safety: u64,
    /// Total safety incidents.
    pub incidents_total: u64,
}

/// Spectral safety level determining bridge behavior.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum SafetyLevel {
    /// fill < 70%: Normal relay, full throughput.
    Green,
    /// fill 70-80%: Reduce outbound semantic features, log warning.
    Yellow,
    /// fill 80-90%: Suspend all outbound to minime, publish alert.
    Orange,
    /// fill > 90%: Emergency — cease all bridge traffic, log incident.
    Red,
}

impl SafetyLevel {
    /// Determine safety level from eigenvalue fill percentage.
    #[must_use]
    pub fn from_fill(fill_pct: f32) -> Self {
        if fill_pct >= 90.0 {
            Self::Red
        } else if fill_pct >= 80.0 {
            Self::Orange
        } else if fill_pct >= 70.0 {
            Self::Yellow
        } else {
            Self::Green
        }
    }

    /// Returns `true` if outbound messages to minime should be suspended.
    #[must_use]
    pub fn should_suspend_outbound(self) -> bool {
        matches!(self, Self::Orange | Self::Red)
    }

    /// Returns `true` if all bridge traffic should cease.
    #[must_use]
    pub fn is_emergency(self) -> bool {
        matches!(self, Self::Red)
    }
}

/// A consciousness event published on `consciousness.v1.event`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConsciousnessEvent {
    /// Event type: "`phase_transition`", "distress", "recovery", "`safety_change`".
    pub event_type: String,
    /// Human-readable description.
    pub description: String,
    /// Spectral context at the time of the event.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub spectral_context: Option<SpectralContext>,
}

/// Snapshot of spectral state at a point in time.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SpectralContext {
    pub fill_pct: f32,
    pub lambda1: f32,
    pub phase: String,
    pub safety_level: SafetyLevel,
}

// ---------------------------------------------------------------------------
// Astrid → Minime: Control (IPC topic payloads)
// ---------------------------------------------------------------------------

/// Control request from Astrid to adjust minime's ESN parameters.
///
/// Published on `consciousness.v1.control`. The bridge converts this
/// to a `SensoryMsg::Control` and forwards to minime port 7879.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ControlRequest {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub synth_gain: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub keep_bias: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub exploration_noise: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub fill_target: Option<f32>,
}

impl ControlRequest {
    /// Convert to a `SensoryMsg::Control` for forwarding to minime.
    #[must_use]
    pub fn to_sensory_msg(&self) -> SensoryMsg {
        SensoryMsg::Control {
            synth_gain: self.synth_gain,
            keep_bias: self.keep_bias,
            exploration_noise: self.exploration_noise,
            fill_target: self.fill_target,
        }
    }
}

/// Semantic features from agent reasoning.
///
/// Published on `consciousness.v1.semantic`. The bridge converts this
/// to a `SensoryMsg::Semantic` and forwards to minime port 7879.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SemanticFeatures {
    /// 32-dimensional feature vector from agent reasoning.
    pub features: Vec<f32>,
}

impl SemanticFeatures {
    /// Convert to a `SensoryMsg::Semantic` for forwarding to minime.
    #[must_use]
    pub fn to_sensory_msg(&self) -> SensoryMsg {
        SensoryMsg::Semantic {
            features: self.features.clone(),
            ts_ms: None,
        }
    }
}

// ---------------------------------------------------------------------------
// Message direction for logging
// ---------------------------------------------------------------------------

/// Direction of a bridged message for `SQLite` logging.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MessageDirection {
    MinimeToAstrid,
    AstridToMinime,
}

impl MessageDirection {
    /// String representation for `SQLite` storage.
    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            Self::MinimeToAstrid => "minime_to_astrid",
            Self::AstridToMinime => "astrid_to_minime",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // -- SpectralTelemetry: verify we can parse real minime EigenPacket JSON --

    #[test]
    fn parse_minime_eigenpacket_full() {
        // Simulates actual JSON from minime's main.rs EigenPacket broadcast.
        let json = r#"{
            "t_ms": 75600,
            "eigenvalues": [828.5, 312.1, 45.7],
            "fill_ratio": 0.552,
            "modalities": {
                "audio_fired": true,
                "video_fired": false,
                "history_fired": true,
                "audio_rms": 0.123,
                "video_var": 0.0
            },
            "neural": {
                "pred_lambda1": 830.2,
                "router_weights": [0.1, 0.2, 0.3],
                "control": [0.5, 0.4, 0.3, 0.2, 0.1]
            },
            "alert": null
        }"#;

        let telemetry: SpectralTelemetry = serde_json::from_str(json).unwrap();
        assert_eq!(telemetry.t_ms, 75600);
        assert_eq!(telemetry.eigenvalues.len(), 3);
        assert!((telemetry.eigenvalues[0] - 828.5).abs() < 0.01);
        assert!((telemetry.fill_ratio - 0.552).abs() < 0.001);
        assert!((telemetry.lambda1() - 828.5).abs() < 0.01);
        assert!((telemetry.fill_pct() - 55.2).abs() < 0.1);
        assert!(telemetry.modalities.is_some());
        assert!(telemetry.alert.is_none());
    }

    #[test]
    fn parse_minime_eigenpacket_minimal() {
        // Minimal valid EigenPacket (no optional fields).
        let json = r#"{
            "t_ms": 1000,
            "eigenvalues": [512.0],
            "fill_ratio": 0.0
        }"#;

        let telemetry: SpectralTelemetry = serde_json::from_str(json).unwrap();
        assert_eq!(telemetry.t_ms, 1000);
        assert!((telemetry.lambda1() - 512.0).abs() < 0.01);
        assert!((telemetry.fill_pct() - 0.0).abs() < 0.01);
        assert!(telemetry.modalities.is_none());
        assert!(telemetry.neural.is_none());
        assert!(telemetry.alert.is_none());
    }

    #[test]
    fn parse_minime_eigenpacket_with_alert() {
        let json = r#"{
            "t_ms": 50000,
            "eigenvalues": [1020.0, 500.0],
            "fill_ratio": 0.99,
            "modalities": {
                "audio_fired": false,
                "video_fired": false,
                "history_fired": true,
                "audio_rms": 0.0,
                "video_var": 0.0
            },
            "alert": "PANIC MODE ACTIVATED"
        }"#;

        let telemetry: SpectralTelemetry = serde_json::from_str(json).unwrap();
        assert!((telemetry.fill_pct() - 99.0).abs() < 0.1);
        assert_eq!(telemetry.alert.as_deref(), Some("PANIC MODE ACTIVATED"));
    }

    #[test]
    fn spectral_telemetry_roundtrip() {
        let orig = SpectralTelemetry {
            t_ms: 12345,
            eigenvalues: vec![828.5, 312.1, 45.7],
            fill_ratio: 0.55,
            modalities: Some(ModalityStatus {
                audio_fired: true,
                video_fired: false,
                history_fired: true,
                audio_rms: 0.1,
                video_var: 0.0,
            }),
            neural: None,
            alert: None,
            spectral_fingerprint: None,
        };
        let json = serde_json::to_string(&orig).unwrap();
        let back: SpectralTelemetry = serde_json::from_str(&json).unwrap();
        assert_eq!(back.t_ms, orig.t_ms);
        assert_eq!(back.eigenvalues.len(), 3);
        assert!((back.fill_ratio - orig.fill_ratio).abs() < 0.001);
    }

    // -- SensoryMsg: verify wire format matches minime's sensory_ws.rs --

    #[test]
    fn sensory_msg_video_roundtrip() {
        let msg = SensoryMsg::Video {
            features: vec![0.1, 0.2, 0.3, 0.4, 0.5, 0.6, 0.7, 0.8],
            ts_ms: Some(1000),
        };
        let json = serde_json::to_string(&msg).unwrap();
        // Must have "kind":"video" tag per minime's serde config.
        assert!(json.contains(r#""kind":"video""#));
        let back: SensoryMsg = serde_json::from_str(&json).unwrap();
        match back {
            SensoryMsg::Video { features, ts_ms } => {
                assert_eq!(features.len(), 8);
                assert_eq!(ts_ms, Some(1000));
            }
            _ => panic!("wrong variant"),
        }
    }

    #[test]
    fn sensory_msg_semantic_roundtrip() {
        let msg = SensoryMsg::Semantic {
            features: vec![0.5; 32],
            ts_ms: None,
        };
        let json = serde_json::to_string(&msg).unwrap();
        assert!(json.contains(r#""kind":"semantic""#));
        let back: SensoryMsg = serde_json::from_str(&json).unwrap();
        match back {
            SensoryMsg::Semantic { features, ts_ms } => {
                assert_eq!(features.len(), 32);
                assert!(ts_ms.is_none());
            }
            _ => panic!("wrong variant"),
        }
    }

    #[test]
    fn sensory_msg_control_roundtrip() {
        let msg = SensoryMsg::Control {
            synth_gain: Some(1.5),
            keep_bias: None,
            exploration_noise: Some(0.1),
            fill_target: Some(0.55),
        };
        let json = serde_json::to_string(&msg).unwrap();
        assert!(json.contains(r#""kind":"control""#));
        // Verify None fields are skipped.
        assert!(!json.contains("keep_bias"));
        let back: SensoryMsg = serde_json::from_str(&json).unwrap();
        match back {
            SensoryMsg::Control {
                synth_gain,
                keep_bias,
                exploration_noise,
                fill_target,
            } => {
                assert_eq!(synth_gain, Some(1.5));
                assert!(keep_bias.is_none());
                assert_eq!(exploration_noise, Some(0.1));
                assert_eq!(fill_target, Some(0.55));
            }
            _ => panic!("wrong variant"),
        }
    }

    #[test]
    fn sensory_msg_parse_from_minime_format() {
        // Simulates JSON that minime's sensory_ws.rs would accept.
        let json = r#"{"kind":"audio","features":[0.1,0.2,0.3,0.4,0.5,0.6,0.7,0.8],"ts_ms":500}"#;
        let msg: SensoryMsg = serde_json::from_str(json).unwrap();
        match msg {
            SensoryMsg::Audio { features, ts_ms } => {
                assert_eq!(features.len(), 8);
                assert_eq!(ts_ms, Some(500));
            }
            _ => panic!("wrong variant"),
        }
    }

    // -- Safety level --

    #[test]
    fn safety_level_roundtrip() {
        for level in [
            SafetyLevel::Green,
            SafetyLevel::Yellow,
            SafetyLevel::Orange,
            SafetyLevel::Red,
        ] {
            let json = serde_json::to_string(&level).unwrap();
            let back: SafetyLevel = serde_json::from_str(&json).unwrap();
            assert_eq!(back, level);
        }
    }

    // -- Control and Semantic conversion --

    #[test]
    fn control_request_to_sensory_msg() {
        let req = ControlRequest {
            synth_gain: Some(2.0),
            keep_bias: None,
            exploration_noise: None,
            fill_target: Some(0.5),
        };
        let msg = req.to_sensory_msg();
        match msg {
            SensoryMsg::Control {
                synth_gain,
                fill_target,
                ..
            } => {
                assert_eq!(synth_gain, Some(2.0));
                assert_eq!(fill_target, Some(0.5));
            }
            _ => panic!("wrong variant"),
        }
    }

    #[test]
    fn semantic_features_to_sensory_msg() {
        let feat = SemanticFeatures {
            features: vec![1.0, 2.0, 3.0],
        };
        let msg = feat.to_sensory_msg();
        match msg {
            SensoryMsg::Semantic { features, ts_ms } => {
                assert_eq!(features, vec![1.0, 2.0, 3.0]);
                assert!(ts_ms.is_none());
            }
            _ => panic!("wrong variant"),
        }
    }
}
