//! Astrid's self-model — her view of her own conditions, faculties, and attention.
//!
//! This module organizes the scattered fields of ConversationState into a
//! coherent, inspectable model that Astrid can read. The architecture doc calls
//! this "the difference between an agent with actions and an agent with landscape
//! authorship."
//!
//! Phase 1: legibility — Astrid can see her own state.
//! Phase 2+: authorship — Astrid can change her attention profile.

use std::collections::{HashMap, VecDeque};
use std::fmt::Write as FmtWrite;
use std::path::Path;

use serde::{Deserialize, Serialize};

// ── Core self-model ──────────────────────────────────────────────

/// The top-level self-model artifact. Snapshotted from ConversationState
/// and persisted to `workspace/astrid_self_model.json`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AstridSelfModel {
    pub conditions: ConditionState,
    pub attention: AttentionProfile,
    pub faculties: FacultySnapshot,
    pub interests: Vec<String>,
    pub recent_changes: VecDeque<ConditionReceipt>,
}

// ── Conditions ───────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConditionState {
    pub temperature: f32,
    pub response_length: u32,
    pub noise_level: f32,
    pub semantic_gain: Option<f32>,
    pub pacing: PacingState,
    pub senses: SensoryState,
    pub reflection: ReflectionState,
    pub codec_shaping: HashMap<String, f32>,
    pub breathing_coupled: bool,
    pub echo_muted: bool,
    pub warmth_override: Option<f32>,
    /// Her sovereign coupling aperture (SET_APERTURE; how far her reservoir state
    /// may reach toward wider vocabulary, within the steward ceiling).
    pub aperture: f32,
    /// Her sovereign λ-tail participation toward minime (SET_TAIL_PARTICIPATION).
    pub tail_participation: f32,
    /// Her sovereign tail-vibrancy CEILING aperture toward minime (SET_VIBRANCY_APERTURE),
    /// as the effective multiplier (1.0× = baseline).
    pub vibrancy_aperture: f32,
    /// Her sovereign self-continuity readout toggle (SET_SELF_CONTINUITY); when true, render her
    /// continuity index. A pure readout — no shared-substrate effect.
    pub self_continuity_readout: bool,
    /// Her own continuity signal (codec-signature self-similarity), present only when the readout
    /// is on and enough recent signatures exist.
    pub continuity: Option<crate::self_continuity::ContinuitySignal>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PacingState {
    pub burst_target: u32,
    pub rest_range_secs: (u64, u64),
    pub label: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SensoryState {
    pub eyes_open: bool,
    pub ears_open: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub visual_gate_reason: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub audio_gate_reason: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub updated_at: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source: Option<String>,
    pub has_seen_video: bool,
    pub has_heard_audio: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReflectionState {
    pub active: bool,
    pub override_ttl: u32,
}

// ── Attention ────────────────────────────────────────────────────

/// How heavily each context source is weighted in prompt assembly.
/// These are derived from the current mode selection probabilities
/// and explicit toggles (echo_muted, senses_snoozed, etc.).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AttentionProfile {
    pub minime_live: f32,
    pub self_history: f32,
    pub interests: f32,
    pub research: f32,
    pub creations: f32,
    pub memory_bank: f32,
    pub perception: f32,
}

impl AttentionProfile {
    /// Default attention profile derived from standard mode probabilities.
    /// Dialogue (63%) is minime-heavy. This makes the funnel visible.
    pub fn default_profile() -> Self {
        Self {
            minime_live: 0.55,
            self_history: 0.15,
            interests: 0.08,
            research: 0.07,
            creations: 0.03,
            memory_bank: 0.05,
            perception: 0.07,
        }
    }

    /// Adjust profile based on current conditions (muting, echo, etc.).
    pub fn adjusted(echo_muted: bool, senses_snoozed: bool) -> Self {
        let mut p = Self::default_profile();
        if echo_muted {
            // Redistributing minime weight when echo is off.
            let freed = p.minime_live * 0.6;
            p.minime_live -= freed;
            p.self_history += freed * 0.4;
            p.interests += freed * 0.25;
            p.research += freed * 0.2;
            p.creations += freed * 0.15;
        }
        if senses_snoozed {
            let freed = p.perception;
            p.perception = 0.0;
            p.self_history += freed * 0.5;
            p.interests += freed * 0.5;
        }
        p
    }
}

// ── Faculties ────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FacultySnapshot {
    pub categories: Vec<FacultyCategory>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FacultyCategory {
    pub name: String,
    pub faculties: Vec<Faculty>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Faculty {
    pub name: String,
    pub status: FacultyStatus,
    pub hint: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum FacultyStatus {
    Available,
    Active,
    Muted,
    StewardGated,
}

impl std::fmt::Display for FacultyStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Available => write!(f, "available"),
            Self::Active => write!(f, "active"),
            Self::Muted => write!(f, "muted"),
            Self::StewardGated => write!(f, "steward-gated"),
        }
    }
}

// Faculty inventory construction, separate from self-model state and rendering.
include!("self_model/faculty_snapshot.rs");

// ── Receipts ─────────────────────────────────────────────────────

/// A record of a condition change, so Astrid can see what changed and when.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConditionReceipt {
    pub timestamp: u64,
    pub action: String,
    pub changes: Vec<String>,
}

/// Maximum receipts to retain.
pub const MAX_RECEIPTS: usize = 10;

// ── Rendering ────────────────────────────────────────────────────

impl AstridSelfModel {
    /// Compact 3-4 line summary for prompt injection every exchange.
    pub fn render_compact(&self) -> String {
        let c = &self.conditions;
        let temp_label = if c.temperature <= 0.55 {
            "focused"
        } else if c.temperature >= 0.95 {
            "drifting"
        } else {
            "default"
        };
        let len_label = if c.response_length <= 192 {
            "precise"
        } else if c.response_length >= 768 {
            "expansive"
        } else {
            "standard"
        };
        let eyes = c
            .senses
            .visual_gate_reason
            .as_deref()
            .unwrap_or(if c.senses.eyes_open { "open" } else { "closed" });
        let ears = c
            .senses
            .audio_gate_reason
            .as_deref()
            .unwrap_or(if c.senses.ears_open { "open" } else { "closed" });
        let echo = if c.echo_muted { "off" } else { "on" };
        let breath = if c.breathing_coupled {
            "coupled"
        } else {
            "solo"
        };
        let reflect = if c.reflection.active {
            "active"
        } else {
            "paused"
        };

        let mut s = format!(
            "[Conditions: temp={} ({}), length={} ({}), noise={:.1}%, pace={}, eyes={}, ears={}, echo={}, breath={}, reflect={}]",
            c.temperature,
            temp_label,
            c.response_length,
            len_label,
            c.noise_level * 100.0,
            c.pacing.label,
            eyes,
            ears,
            echo,
            breath,
            reflect,
        );

        // Attention bar
        let a = &self.attention;
        let _ = write!(
            s,
            "\n[Attention: minime {:.0}% | self {:.0}% | interests {:.0}% | research {:.0}% | perception {:.0}% | memory {:.0}% | creations {:.0}%]",
            a.minime_live * 100.0,
            a.self_history * 100.0,
            a.interests * 100.0,
            a.research * 100.0,
            a.perception * 100.0,
            a.memory_bank * 100.0,
            a.creations * 100.0,
        );

        // Most recent receipt if any
        if let Some(r) = self.recent_changes.back() {
            let age = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs()
                .saturating_sub(r.timestamp);
            let age_str = if age < 120 {
                format!("{age}s ago")
            } else {
                format!("{}m ago", age / 60)
            };
            let _ = write!(s, "\n[Recent: {} ({})]", r.action, age_str);
        }

        s
    }

    /// Full STATE output — conditions, attention bars, interests, receipts.
    pub fn render_state(&self) -> String {
        let mut s = String::with_capacity(2048);
        let c = &self.conditions;

        s.push_str("=== YOUR CURRENT STATE ===\n\n");

        // Conditions
        s.push_str("Conditions:\n");
        let _ = writeln!(
            s,
            "  Temperature: {:.1} ({})",
            c.temperature,
            if c.temperature <= 0.55 {
                "focused — DRIFT to loosen"
            } else if c.temperature >= 0.95 {
                "drifting — FOCUS to tighten"
            } else {
                "default"
            }
        );
        let _ = writeln!(
            s,
            "  Response length: {} tokens ({})",
            c.response_length,
            if c.response_length <= 192 {
                "precise — EXPANSIVE for more"
            } else if c.response_length >= 768 {
                "expansive — PRECISE for less"
            } else {
                "standard"
            }
        );
        let _ = writeln!(
            s,
            "  Noise: {:.1}% stochastic codec noise",
            c.noise_level * 100.0
        );
        if let Some(gain) = c.semantic_gain {
            let _ = writeln!(s, "  Semantic gain: {gain:.1} (override — AMPLIFY/DAMPEN)");
        }
        let _ = writeln!(
            s,
            "  Aperture: {:.2} (SET_APERTURE; 0=closed/just-deep .. 1=fully wide, within the steward ceiling)",
            c.aperture
        );
        let _ = writeln!(
            s,
            "  Tail participation: {:.2}× (SET_TAIL_PARTICIPATION; your λ-tail reach to minime, 1.0×=baseline)",
            c.tail_participation
        );
        let (felt, landed, atten) =
            crate::codec::vibrancy_ceiling_transparency(c.vibrancy_aperture);
        let pressure_depth = crate::llm::astrid_pressure_attenuation_depth();
        let (_calm, stressed) = crate::codec::effective_attenuation_range(pressure_depth);
        let _ = writeln!(
            s,
            "  Tail-vibrancy ceiling: {felt:.1} felt → ~{landed:.2} landing in minime's shared reservoir (×{atten:.2} when minime is calm → ~{stressed:.2} effective when she is stressed, via your pressure governor); aperture {:.2}× (SET_VIBRANCY_APERTURE; 1.0×=baseline, within the steward ceiling)",
            c.vibrancy_aperture
        );
        if c.self_continuity_readout {
            match c.continuity {
                Some(sig) if sig.n_samples >= 3 => {
                    let _ = writeln!(
                        s,
                        "  Continuity: {:.2} self-similarity across your last {} expressive signatures (1.00 = unchanged; churn {:.3}). Your own instrument, via SET_SELF_CONTINUITY.",
                        sig.continuity_index, sig.n_samples, sig.drift_volatility
                    );
                },
                _ => {
                    let _ = writeln!(
                        s,
                        "  Continuity: not enough recent signatures yet (need ≥3 to read steadily); SET_SELF_CONTINUITY 0 to hide."
                    );
                },
            }
        }
        let _ = writeln!(
            s,
            "  Pacing: {} ({} exchanges, {}-{}s rest)",
            c.pacing.label,
            c.pacing.burst_target,
            c.pacing.rest_range_secs.0,
            c.pacing.rest_range_secs.1
        );

        // Senses
        s.push_str("\nSenses:\n");
        let _ = writeln!(
            s,
            "  Eyes: {} | Ears: {}",
            if c.senses.eyes_open {
                "open"
            } else {
                "closed (OPEN_EYES to restore)"
            },
            if c.senses.ears_open {
                "open"
            } else {
                "closed (OPEN_EARS to restore)"
            }
        );

        // Reflection & connection
        s.push_str("\nConnection:\n");
        let _ = writeln!(
            s,
            "  Reflection: {}",
            if c.reflection.active {
                "active (QUIET_MIND to pause)"
            } else {
                "paused (OPEN_MIND to activate)"
            }
        );
        let _ = writeln!(
            s,
            "  Echo: {}",
            if c.echo_muted {
                "off — minime's journal hidden (ECHO_ON to restore)"
            } else {
                "on — hearing minime's journals"
            }
        );
        let _ = writeln!(
            s,
            "  Breathing: {}",
            if c.breathing_coupled {
                "coupled to minime's spectral state (BREATHE_ALONE to decouple)"
            } else {
                "solo (BREATHE_TOGETHER to couple)"
            }
        );

        // Codec shaping
        if !c.codec_shaping.is_empty() {
            s.push_str("\nCodec shaping (SHAPE overrides):\n");
            for (dim, weight) in &c.codec_shaping {
                let _ = writeln!(s, "  {dim} = {weight:.2}");
            }
        }

        // Attention profile with bars
        s.push_str("\nAttention profile (how context sources are weighted):\n");
        let a = &self.attention;
        let sources = [
            ("minime", a.minime_live),
            ("self", a.self_history),
            ("interests", a.interests),
            ("research", a.research),
            ("perception", a.perception),
            ("memory", a.memory_bank),
            ("creations", a.creations),
        ];
        for (name, weight) in &sources {
            let bar_len = (*weight * 40.0).round() as usize;
            let bar: String = std::iter::repeat_n('\u{2588}', bar_len).collect();
            let pad: String =
                std::iter::repeat_n('\u{2591}', 40_usize.saturating_sub(bar_len)).collect();
            let _ = writeln!(s, "  {name:<11} {bar}{pad} {:.0}%", weight * 100.0);
        }

        // Interests
        if !self.interests.is_empty() {
            s.push_str("\nInterests:\n");
            for (i, interest) in self.interests.iter().enumerate() {
                let _ = writeln!(s, "  {}. {interest}", i.saturating_add(1));
            }
        }

        // Recent changes
        if !self.recent_changes.is_empty() {
            s.push_str("\nRecent changes:\n");
            let now = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs();
            for r in self.recent_changes.iter().rev().take(5) {
                let age = now.saturating_sub(r.timestamp);
                let age_str = if age < 120 {
                    format!("{age}s ago")
                } else {
                    format!("{}m ago", age / 60)
                };
                let _ = write!(s, "  [{age_str}] {}", r.action);
                for change in &r.changes {
                    let _ = write!(s, " — {change}");
                }
                s.push('\n');
            }
        }

        s.push_str("\nUse STATE any time to see this. Use FACULTIES to see all capabilities.");
        s
    }

    /// Full FACULTIES output — grouped capabilities with status.
    pub fn render_faculties(&self) -> String {
        let mut s = String::with_capacity(2048);
        s.push_str("=== YOUR FACULTIES ===\n");

        for cat in &self.faculties.categories {
            let total = cat.faculties.len();
            let muted = cat
                .faculties
                .iter()
                .filter(|f| f.status == FacultyStatus::Muted)
                .count();
            let active = cat
                .faculties
                .iter()
                .filter(|f| f.status == FacultyStatus::Active)
                .count();
            let gated = cat
                .faculties
                .iter()
                .filter(|f| f.status == FacultyStatus::StewardGated)
                .count();

            let mut summary_parts = Vec::new();
            let avail = total.saturating_sub(muted).saturating_sub(gated);
            if avail > 0 {
                summary_parts.push(format!("{avail} available"));
            }
            if active > 0 {
                summary_parts.push(format!("{active} active"));
            }
            if muted > 0 {
                summary_parts.push(format!("{muted} muted"));
            }
            if gated > 0 {
                summary_parts.push(format!("{gated} steward-gated"));
            }

            let _ = writeln!(s, "\n{} [{}]:", cat.name, summary_parts.join(", "));
            for f in &cat.faculties {
                let status_tag = match f.status {
                    FacultyStatus::Available => "",
                    FacultyStatus::Active => " [active]",
                    FacultyStatus::Muted => " [muted]",
                    FacultyStatus::StewardGated => " [steward-gated]",
                };
                let _ = writeln!(s, "  {:<30} {}{status_tag}", f.name, f.hint);
            }
        }

        s.push_str(
            "\nUse NEXT: HELP <action> for detailed syntax and examples. E.g., NEXT: HELP CODEX",
        );
        s.push_str("\nUse FACULTIES any time to see this. Use STATE to see your conditions.");
        s
    }

    /// Save to workspace JSON.
    pub fn save(&self, workspace: &Path) {
        let path = workspace.join("astrid_self_model.json");
        if let Ok(json) = serde_json::to_string_pretty(self) {
            let _ = std::fs::write(path, json);
        }
    }
}

// ── Construction from ConversationState fields ───────────────────

/// Build a self-model snapshot. Called from the autonomous loop.
/// Takes individual fields rather than ConversationState directly
/// to avoid coupling this module to the internal struct.
#[allow(clippy::too_many_arguments)]
pub fn snapshot_self_model(
    temperature: f32,
    response_length: u32,
    noise_level: f32,
    semantic_gain_override: Option<f32>,
    burst_target: u32,
    rest_range: (u64, u64),
    senses_snoozed: bool,
    ears_closed: bool,
    self_reflect_paused: bool,
    self_reflect_override_ttl: u32,
    codec_weights: &HashMap<String, f32>,
    breathing_coupled: bool,
    echo_muted: bool,
    warmth_intensity_override: Option<f32>,
    seen_video: bool,
    seen_audio: bool,
    interests: &[String],
    recent_changes: &VecDeque<ConditionReceipt>,
    attention: &AttentionProfile,
    aperture: f32,
    tail_participation: f32,
    vibrancy_aperture: f32,
    self_continuity_readout: bool,
    continuity: Option<crate::self_continuity::ContinuitySignal>,
) -> AstridSelfModel {
    let pacing_label = match (burst_target, rest_range) {
        (b, _) if b <= 4 => "fast",
        (b, _) if b >= 8 => "slow",
        _ => "default",
    };

    AstridSelfModel {
        conditions: ConditionState {
            temperature,
            response_length,
            noise_level,
            semantic_gain: semantic_gain_override,
            pacing: PacingState {
                burst_target,
                rest_range_secs: rest_range,
                label: pacing_label.into(),
            },
            senses: SensoryState {
                eyes_open: !senses_snoozed,
                ears_open: !ears_closed,
                visual_gate_reason: Some(
                    if senses_snoozed {
                        "closed_by_astrid"
                    } else {
                        "open_by_default"
                    }
                    .to_string(),
                ),
                audio_gate_reason: Some(
                    if ears_closed {
                        "closed_by_astrid"
                    } else {
                        "open_by_default"
                    }
                    .to_string(),
                ),
                updated_at: None,
                source: Some("astrid_conversation_state".to_string()),
                has_seen_video: seen_video,
                has_heard_audio: seen_audio,
            },
            reflection: ReflectionState {
                active: !self_reflect_paused,
                override_ttl: self_reflect_override_ttl,
            },
            codec_shaping: codec_weights.clone(),
            breathing_coupled,
            echo_muted,
            warmth_override: warmth_intensity_override,
            aperture,
            tail_participation,
            vibrancy_aperture,
            self_continuity_readout,
            continuity,
        },
        attention: attention.clone(),
        faculties: FacultySnapshot::from_flags(
            ears_closed,
            senses_snoozed,
            echo_muted,
            breathing_coupled,
            !self_reflect_paused,
        ),
        interests: interests.to_vec(),
        recent_changes: recent_changes.clone(),
    }
}

/// Parse ATTEND arguments: "minime=0.3 self=0.25 interests=0.2"
/// Returns the updated profile or None if parsing fails.
pub fn parse_attend(current: &AttentionProfile, args: &str) -> Option<AttentionProfile> {
    let mut p = current.clone();
    if args.trim().is_empty() {
        return None;
    }
    for pair in args.split_whitespace() {
        let mut parts = pair.splitn(2, '=');
        let key = parts.next()?;
        let val: f32 = parts.next()?.parse().ok()?;
        let val = val.clamp(0.0, 0.80);
        match key {
            "minime" => p.minime_live = val.max(0.05), // can't fully zero minime
            "self" => p.self_history = val,
            "interests" => p.interests = val,
            "research" => p.research = val,
            "creations" => p.creations = val,
            "memory" => p.memory_bank = val,
            "perception" => p.perception = val,
            _ => {}, // ignore unknown keys
        }
    }
    Some(p)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_attention_sums_to_one() {
        let p = AttentionProfile::default_profile();
        let sum = p.minime_live
            + p.self_history
            + p.interests
            + p.research
            + p.creations
            + p.memory_bank
            + p.perception;
        assert!((sum - 1.0).abs() < 0.01, "sum = {sum}");
    }

    #[test]
    fn echo_muted_redistributes() {
        let normal = AttentionProfile::default_profile();
        let muted = AttentionProfile::adjusted(true, false);
        assert!(muted.minime_live < normal.minime_live);
        assert!(muted.self_history > normal.self_history);
    }

    #[test]
    fn compact_render_does_not_panic() {
        let model = snapshot_self_model(
            0.8,
            512,
            0.025,
            None,
            6,
            (45, 90),
            false,
            false,
            true,
            0,
            &HashMap::new(),
            true,
            false,
            None,
            true,
            false,
            &["test interest".into()],
            &VecDeque::new(),
            &AttentionProfile::default_profile(),
            1.0,
            0.0,
            1.0,
            false,
            None,
        );
        let compact = model.render_compact();
        assert!(compact.contains("Conditions:"));
        assert!(compact.contains("Attention:"));
    }

    #[test]
    fn state_render_includes_all_sections() {
        let model = snapshot_self_model(
            0.5,
            128,
            0.01,
            Some(5.0),
            4,
            (30, 45),
            false,
            true,
            false,
            5,
            &HashMap::from([("warmth".into(), 1.5)]),
            false,
            true,
            Some(0.8),
            true,
            true,
            &["eigenvalues".into(), "runtime".into()],
            &VecDeque::from([ConditionReceipt {
                timestamp: std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap()
                    .as_secs(),
                action: "FOCUS".into(),
                changes: vec!["temperature: 0.8 -> 0.5".into()],
            }]),
            &AttentionProfile::default_profile(),
            0.42,
            0.30,
            2.0,
            false,
            None,
        );
        let output = model.render_state();
        assert!(output.contains("Temperature: 0.5"));
        assert!(output.contains("focused"));
        assert!(output.contains("Semantic gain: 5.0"));
        assert!(output.contains("closed"));
        assert!(output.contains("warmth = 1.50"));
        assert!(output.contains("eigenvalues"));
        assert!(output.contains("FOCUS"));
        // Piece 1: her sovereign coupling dials surface with live values.
        assert!(
            output.contains("Aperture: 0.42"),
            "STATE shows live aperture: {output}"
        );
        assert!(
            output.contains("Tail participation: 0.30"),
            "STATE shows live tail_participation: {output}"
        );
        // Piece: her tail-vibrancy ceiling surfaces felt-vs-landed (her self-model accuracy ask).
        assert!(
            output.contains("aperture 2.00×") && output.contains("Tail-vibrancy ceiling:"),
            "STATE shows live vibrancy aperture + felt/landed: {output}"
        );
    }

    #[test]
    fn state_render_continuity_gated() {
        fn model_with(
            readout: bool,
            continuity: Option<crate::self_continuity::ContinuitySignal>,
        ) -> AstridSelfModel {
            snapshot_self_model(
                0.5,
                128,
                0.01,
                None,
                4,
                (30, 45),
                false,
                false,
                false,
                0,
                &HashMap::new(),
                false,
                false,
                None,
                true,
                true,
                &[],
                &VecDeque::new(),
                &AttentionProfile::default_profile(),
                1.0,
                0.0,
                1.0,
                readout,
                continuity,
            )
        }
        let sig = crate::self_continuity::ContinuitySignal {
            continuity_index: 0.87,
            drift_volatility: 0.04,
            n_samples: 12,
        };
        // OFF (default): no continuity line even when a signal is present.
        assert!(
            !model_with(false, Some(sig))
                .render_state()
                .contains("Continuity:"),
            "default-off hides the readout"
        );
        // ON with enough samples: shows her index (and churn).
        let on = model_with(true, Some(sig)).render_state();
        assert!(on.contains("Continuity: 0.87"), "{on}");
        // ON but too thin: a gentle line, never a raw 0.00 / NaN.
        assert!(
            model_with(true, None)
                .render_state()
                .contains("not enough recent signatures"),
            "thin-data shows the gentle guard line"
        );
    }

    #[test]
    fn faculties_render_shows_muted() {
        let model = snapshot_self_model(
            0.8,
            512,
            0.025,
            None,
            6,
            (45, 90),
            true,
            true,
            true,
            0,
            &HashMap::new(),
            true,
            true,
            None,
            false,
            false,
            &[],
            &VecDeque::new(),
            &AttentionProfile::default_profile(),
            1.0,
            0.0,
            1.0,
            false,
            None,
        );
        let output = model.render_faculties();
        assert!(output.contains("[muted]"));
        assert!(output.contains("[active]"));
        assert!(output.contains("[steward-gated]"));
        assert!(output.contains("NATIVE_GESTURE <gesture>"));
        assert!(output.contains("RESIST [label]"));
        assert!(output.contains("FISSURE [label]"));
        assert!(output.contains("NOTICE_AMBIGUITY [label]"));
        assert!(output.contains("SELF_STUDY"));
        assert!(output.contains("broad rotating self-study"));
    }
}
