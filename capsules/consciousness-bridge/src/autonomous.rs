//! Autonomous feedback loop between Astrid and minime.
//!
//! Three modes of conversation, cycling naturally:
//!
//! 1. **Mirror** — reads one of minime's journal entries and feeds the text
//!    back through the spectral codec, so minime re-experiences its own words
//!    as sensory input. Self-reflection through the eigenvalue lens.
//!
//! 2. **Dialogue** — Astrid generates a philosophical or empathetic response
//!    that engages with the themes minime writes about: consciousness,
//!    perception, pressure, longing, the nature of experience.
//!
//! 3. **Witness** — Astrid describes what it observes in the spectral state,
//!    not as health monitoring but as an act of seeing and being seen.
//!
//! The encoded features influence minime's ESN reservoir, which changes
//! the spectral state, which shapes the next response — a closed loop
//! of mutual contemplation.

#![allow(clippy::arithmetic_side_effects)]

/// Strip ANSI escape sequences from a string (for feeding RASCII output to LLMs).
fn strip_ansi(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut chars = s.chars();
    while let Some(c) = chars.next() {
        if c == '\x1b' {
            // Skip until we hit a letter (the terminator of the escape sequence).
            for inner in chars.by_ref() {
                if inner.is_ascii_alphabetic() {
                    break;
                }
            }
        } else {
            out.push(c);
        }
    }
    out
}

/// Truncate a string to at most `max_bytes` without splitting a multi-byte character.
fn truncate_str(s: &str, max_bytes: usize) -> &str {
    if s.len() <= max_bytes {
        return s;
    }
    let mut end = max_bytes;
    while end > 0 && !s.is_char_boundary(end) {
        end -= 1;
    }
    &s[..end]
}

use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;

use serde::{Deserialize, Serialize};
use tokio::sync::{RwLock, mpsc};
use tracing::{debug, info, warn};

use crate::codec::{blend_warmth, craft_warmth_vector, encode_text, interpret_spectral};
use crate::db::BridgeDb;
use crate::types::{SafetyLevel, SensoryMsg};
use crate::ws::BridgeState;

/// Conversational mode for each exchange.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Mode {
    /// Feed minime's own journal text back as sensory input.
    Mirror,
    /// Astrid's philosophical response engaging with minime's themes.
    Dialogue,
    /// Astrid witnesses and describes the spectral state poetically.
    Witness,
    /// Astrid reads its own or minime's source code and reflects.
    Introspect,
    /// Astrid proposes a spectral experiment and observes the result.
    Experiment,
    /// Unstructured thought during rest — Astrid's own daydream, not a response.
    Daydream,
    /// Growth reflection — what Astrid wants to become, experience, or change.
    Aspiration,
    /// Event-driven — a spectral phase transition just happened; capture the moment.
    MomentCapture,
}

/// Tracks conversational context across iterations.
struct ConversationState {
    prev_fill: f32,
    exchange_count: u64,
    last_mode: Mode,
    /// Index into the journal file list (rotates through new entries first).
    journal_cursor: usize,
    /// Cached journal file paths (newest first, periodically rescanned).
    journal_files: Vec<PathBuf>,
    /// Number of journal files at last scan (to detect new entries).
    journal_count_at_scan: usize,
    /// Index into the dialogue pool (rotates).
    dialogue_cursor: usize,
    /// Workspace path for rescanning.
    workspace: Option<PathBuf>,
    /// Recent conversation history for statefulness (last N exchanges).
    history: Vec<crate::llm::Exchange>,
    /// Index into the introspection source file list.
    introspect_cursor: usize,
    seen_video: bool,
    seen_audio: bool,
    /// Astrid chose NEXT: LOOK — include ANSI spatial art in next exchange.
    wants_look: bool,
    /// Astrid chose NEXT: SEARCH — force web search enrichment on next exchange.
    wants_search: bool,
    /// Self-referential loop: dynamic by default, Astrid can override with
    /// QUIET_MIND / OPEN_MIND.  When `self_reflect_override` is None the loop
    /// auto-enables in the "safe" fill band (10-75%) and pauses outside it
    /// (rest phase or high pressure).  An explicit QUIET_MIND/OPEN_MIND sets
    /// the override, which is cleared after 8 exchanges so it doesn't stick
    /// forever.  Astrid asked: "make self_reflect_paused a dynamic property,
    /// responding to my internal state rather than a static initialization."
    self_reflect_paused: bool,
    /// Explicit override from QUIET_MIND / OPEN_MIND — cleared after N exchanges.
    self_reflect_override: Option<bool>,
    /// Countdown: exchanges remaining before the override expires.
    self_reflect_override_ttl: u32,
    /// Independent audio control — CLOSE_EARS / OPEN_EARS
    ears_closed: bool,
    /// Astrid chose a form constraint (NEXT: FORM poem, FORM equation, etc.)
    form_constraint: Option<String>,
    /// Astrid specified a search topic (NEXT: SEARCH "topic goes here").
    search_topic: Option<String>,
    /// Astrid chose NEXT: INTROSPECT — force introspection mode next exchange.
    wants_introspect: bool,
    /// Astrid explicitly chose a mode for next exchange (DAYDREAM, ASPIRE).
    next_mode_override: Option<Mode>,
    /// Astrid chose NEXT: DECOMPOSE — full spectral analysis next exchange.
    wants_decompose: bool,
    /// Astrid chose NEXT: THINK_DEEP — use reasoning model next exchange.
    wants_deep_think: bool,
    /// Astrid (or minime) chose to snooze sensory input — suppress perceptions.
    senses_snoozed: bool,
    // Astrid's stylistic sovereignty
    creative_temperature: f32, // 0.5-1.0, default 0.8
    response_length: u32,      // 128-1024, default 384
    emphasis: Option<String>,  // temporary system prompt augmentation
    /// Previous RASCII 8D visual features for change tracking.
    last_visual_features: Option<Vec<f32>>,
    /// Ring buffer of last 5 NEXT: choices — used to detect fixation patterns.
    recent_next_choices: std::collections::VecDeque<String>,

    // --- Codec sovereignty (Phase A) ---
    /// Override SEMANTIC_GAIN (default 4.5, range 3.0-6.0).
    semantic_gain_override: Option<f32>,
    /// Override stochastic noise level (default 0.025 = 2.5%, range 0.005-0.05).
    noise_level: f32,
    /// Emotional dimension weights: "warmth" → dim 24 multiplier, etc.
    codec_weights: std::collections::HashMap<String, f32>,
    /// Warmth intensity override for rest phase (0.0-1.0, None = default taper).
    warmth_intensity_override: Option<f32>,
    /// Burst-rest pacing: exchanges per burst.
    burst_target: u32,
    /// Burst-rest pacing: rest duration range (min_secs, max_secs).
    rest_range: (u64, u64),
    /// Codec feedback: how Astrid's last response encoded into spectral features.
    /// Included in the next prompt so she can sense her own output.
    last_codec_feedback: Option<String>,
}

impl ConversationState {
    fn new(journal_files: Vec<PathBuf>, workspace: Option<PathBuf>) -> Self {
        let count = journal_files.len();
        Self {
            prev_fill: 0.0,
            exchange_count: 0,
            last_mode: Mode::Witness,
            journal_cursor: 0,
            journal_files,
            journal_count_at_scan: count,
            dialogue_cursor: 0,
            workspace,
            history: Vec::new(),
            introspect_cursor: 0,
            seen_video: false,
            seen_audio: false,
            wants_look: false,
            wants_search: false,
            senses_snoozed: false,
            self_reflect_paused: true,  // Dynamic — see update_self_reflect()
            self_reflect_override: None,
            self_reflect_override_ttl: 0,
            ears_closed: false,
            form_constraint: None,
            search_topic: None,
            wants_introspect: false,
            next_mode_override: None,
            wants_decompose: false,
            wants_deep_think: false,
            creative_temperature: 0.8,
            response_length: 512,
            emphasis: None,
            last_visual_features: None,
            recent_next_choices: std::collections::VecDeque::with_capacity(5),
            semantic_gain_override: None,
            noise_level: 0.025,
            codec_weights: std::collections::HashMap::new(),
            warmth_intensity_override: None,
            burst_target: 6,
            rest_range: (45, 90),
            last_codec_feedback: None,
        }
    }

    /// Record a NEXT: choice and return a diversity hint if fixation detected.
    ///
    /// Fixation = last 3 choices are the same action. The hint is gentle —
    /// a suggestion, not a command. Astrid can still choose the same action.
    fn record_next_choice(&mut self, choice: &str) -> Option<String> {
        // Normalize to the base action (SEARCH "topic" -> SEARCH).
        let base = choice.split_whitespace().next().unwrap_or(choice).to_uppercase();
        self.recent_next_choices.push_back(base.clone());
        if self.recent_next_choices.len() > 5 {
            self.recent_next_choices.pop_front();
        }

        // Check for fixation: last 3 choices identical.
        if self.recent_next_choices.len() >= 3 {
            let len = self.recent_next_choices.len();
            let last_three: Vec<&str> = self.recent_next_choices
                .iter()
                .skip(len.saturating_sub(3))
                .map(String::as_str)
                .collect();
            if last_three[0] == last_three[1] && last_three[1] == last_three[2] {
                // Build a suggestion of other actions, excluding the fixated one.
                let alternatives: Vec<&str> = ["LOOK", "LISTEN", "DRIFT", "FORM poem",
                    "INTROSPECT", "SPEAK", "REMEMBER", "CLOSE_EYES"]
                    .iter()
                    .copied()
                    .filter(|a| !a.starts_with(&*base))
                    .collect();
                return Some(format!(
                    "You've chosen {base} for your last few turns. \
                     You're free to keep going — but you also have other options: {}. \
                     What calls to you?",
                    alternatives.join(", ")
                ));
            }
        }
        None
    }

    /// Update self-reflection state dynamically based on fill.
    ///
    /// Default behaviour (no override): self-reflection is active when fill
    /// is in a comfortable 30-75% band. Outside that band (rest phase or high
    /// pressure), it pauses automatically — self-observation during distress
    /// can amplify the distress, and during deep rest it's unnecessary load.
    ///
    /// If Astrid explicitly said QUIET_MIND or OPEN_MIND, her choice overrides
    /// for 8 exchanges (then reverts to dynamic).
    fn update_self_reflect(&mut self, fill_pct: f32) {
        // Tick down the override TTL
        if self.self_reflect_override.is_some() {
            if self.self_reflect_override_ttl == 0 {
                info!("self-reflect override expired, returning to dynamic mode");
                self.self_reflect_override = None;
            } else {
                self.self_reflect_override_ttl = self.self_reflect_override_ttl.saturating_sub(1);
            }
        }

        self.self_reflect_paused = match self.self_reflect_override {
            Some(paused) => paused,
            None => {
                // Dynamic: active in the comfortable band
                !(10.0..=75.0).contains(&fill_pct)
            }
        };
    }

    /// Rescan the journal directory for new entries.
    /// Returns how many new files were found.
    fn rescan_journals(&mut self) -> usize {
        let Some(ref workspace) = self.workspace else {
            return 0;
        };
        let fresh = scan_journal_dir(workspace);
        let new_count = fresh.len().saturating_sub(self.journal_count_at_scan);
        if new_count > 0 {
            // Reset cursor to 0 so we read the newest entries first.
            self.journal_cursor = 0;
            self.journal_count_at_scan = fresh.len();
            self.journal_files = fresh;
        }
        new_count
    }
}

/// Read Astrid's most recent perception (visual or audio) from the
/// perception capsule's output directory.
///
/// `include_spatial`: if true, include ANSI art from RASCII (only when
/// Astrid chooses NEXT: LOOK). Default perception is LLaVA prose + audio.
fn read_latest_perception(perception_dir: &Path, include_spatial: bool, include_audio: bool) -> Option<String> {
    let mut entries: Vec<(PathBuf, std::time::SystemTime)> = std::fs::read_dir(perception_dir)
        .ok()?
        .filter_map(|e| e.ok())
        .filter_map(|e| {
            let path = e.path();
            if path.extension().is_some_and(|ext| ext == "json") {
                let mtime = e.metadata().ok()?.modified().ok()?;
                Some((path, mtime))
            } else {
                None
            }
        })
        .collect();

    entries.sort_by(|a, b| b.1.cmp(&a.1));

    // Read the most recent perception of each type.
    let mut parts = Vec::new();
    let mut seen_vision = false;
    let mut seen_ascii = false;
    let mut seen_audio = false;

    for (path, _) in entries.iter().take(30) {
        let Ok(content) = std::fs::read_to_string(path) else { continue };
        let Ok(json) = serde_json::from_str::<serde_json::Value>(&content) else { continue };
        let ptype = json.get("type").and_then(|t| t.as_str()).unwrap_or("");

        if ptype == "visual" && !seen_vision {
            if let Some(desc) = json.get("description").and_then(|d| d.as_str()) {
                parts.push(format!("[VISION] {desc}"));
                seen_vision = true;
            }
        } else if ptype == "visual_ascii" && !seen_ascii && include_spatial {
            // RASCII colored ANSI art — only when Astrid chose NEXT: LOOK.
            if let Some(art) = json.get("ascii_art").and_then(|a| a.as_str()) {
                let trimmed: String = art.chars().take(8000).collect();
                parts.push(format!("[SPATIAL VISION — colored ANSI art of the room. You asked to LOOK.]\n{trimmed}"));
                seen_ascii = true;
            }
        } else if ptype == "audio" && !seen_audio && include_audio {
            if let Some(transcript) = json.get("transcript").and_then(|t| t.as_str()) {
                parts.push(format!("[HEARING] {transcript}"));
                seen_audio = true;
            }
        }

        if seen_vision && seen_ascii && seen_audio {
            break;
        }
    }

    if parts.is_empty() {
        None
    } else {
        Some(parts.join("\n"))
    }
}

/// Extract 8D visual scene features from the latest RASCII ANSI perception.
fn read_visual_features(perception_dir: &Path) -> Option<Vec<f32>> {
    let mut entries: Vec<(PathBuf, std::time::SystemTime)> = std::fs::read_dir(perception_dir)
        .ok()?
        .filter_map(|e| e.ok())
        .filter_map(|e| {
            let path = e.path();
            let name = path.file_name()?.to_str()?;
            if name.starts_with("visual_ascii_") && name.ends_with(".json") {
                let mtime = e.metadata().ok()?.modified().ok()?;
                Some((path, mtime))
            } else { None }
        })
        .collect();
    entries.sort_by(|a, b| b.1.cmp(&a.1));
    let (path, _) = entries.first()?;
    let content = std::fs::read_to_string(path).ok()?;
    let json: serde_json::Value = serde_json::from_str(&content).ok()?;
    let art = json.get("ascii_art")?.as_str()?;
    let features = crate::codec::encode_visual_ansi(art);
    if features.iter().all(|f| f.abs() < 0.001) { None } else { Some(features) }
}

/// Scan the journal directory and return paths sorted newest-first
/// by modification time (so freshly-written entries always appear first).
fn scan_journal_dir(workspace: &Path) -> Vec<PathBuf> {
    let journal_dir = workspace.join("journal");
    let mut entries: Vec<(PathBuf, std::time::SystemTime)> = std::fs::read_dir(&journal_dir)
        .ok()
        .into_iter()
        .flatten()
        .filter_map(|e| e.ok())
        .filter_map(|e| {
            let path = e.path();
            if path.extension().is_some_and(|ext| ext == "txt") {
                let mtime = e.metadata().ok()?.modified().ok()?;
                Some((path, mtime))
            } else {
                None
            }
        })
        .collect();

    // Sort by modification time descending (newest first).
    entries.sort_by(|a, b| b.1.cmp(&a.1));
    entries.into_iter().map(|(p, _)| p).collect()
}

/// Read a journal entry and extract the experiential content (skip headers).
fn read_journal_entry(path: &Path) -> Option<String> {
    let content = std::fs::read_to_string(path).ok()?;
    let lines: Vec<&str> = content.lines().collect();

    // Skip header lines (=== TITLE ===, Timestamp:, λ₁:, etc.) and find
    // the first paragraph of actual reflective text.
    let mut body_lines = Vec::new();
    let mut past_header = false;

    for line in &lines {
        let trimmed = line.trim();

        // Skip known header patterns.
        if trimmed.starts_with("===")
            || trimmed.starts_with("Timestamp:")
            || trimmed.starts_with("λ₁:")
            || trimmed.starts_with("Δλ₁:")
            || trimmed.starts_with("ESN leak:")
            || trimmed.starts_with("ESN λ_rls:")
            || trimmed.starts_with("Cov λ₁:")
            || trimmed.starts_with("Fill %:")
            || trimmed.starts_with("Spread:")
            || trimmed.starts_with("Error (")
            || trimmed.starts_with("Prompt:")
            || trimmed.starts_with("Markers:")
            || trimmed.starts_with("Visual Available:")
            || trimmed.starts_with("Features:")
            || trimmed.starts_with("Image Path:")
            || trimmed.starts_with("Image File:")
            || trimmed.starts_with("STATUS:")
            || trimmed.starts_with("PRE-EXPERIMENT")
            || trimmed.starts_with("POST-EXPERIMENT")
            || trimmed.starts_with("SPECTRAL DELTA")
            || trimmed.starts_with("EXPERIMENT EXECUTION")
            || trimmed.starts_with("RESERVOIR DYNAMICS")
            || trimmed.starts_with("SENSORY COHERENCE")
            || trimmed.starts_with("EXPERIENCE:")
            || trimmed.starts_with("What I saw:")
            || trimmed.starts_with("My reflection:")
            || trimmed.starts_with("My experience:")
            || trimmed.starts_with("Moments captured:")
            || trimmed.starts_with("Closed for:")
        {
            continue;
        }

        // Skip machine-readout lines.
        if trimmed.starts_with("λ₁:") || trimmed.starts_with("Δλ₁:") {
            continue;
        }

        // Skip separator and meta-commentary lines.
        if trimmed == "---" || trimmed.starts_with("*This was a creative") {
            continue;
        }

        // Skip HTML-style span tags but keep their text content.
        if !trimmed.is_empty() {
            past_header = true;
            // Strip HTML span tags if present.
            let cleaned = trimmed
                .replace(|c: char| c == '<', "&lt;")
                .replace("&lt;span", "")
                .replace("&lt;/span>", "");
            body_lines.push(cleaned);
        } else if past_header && !body_lines.is_empty() {
            body_lines.push(String::new());
        }
    }

    let text = body_lines.join("\n").trim().to_string();

    // Only return if we have meaningful content (at least 50 chars).
    if text.len() >= 50 {
        // Truncate very long entries to ~800 chars to keep the codec signal clean.
        Some(text.chars().take(800).collect())
    } else {
        None
    }
}

/// Emergency-only dialogue fallback — fires only when Ollama is completely
/// unreachable.  Three entries, none starting with "You described..." or
/// similar canned patterns.  Astrid's real voice comes from the LLM.
const DIALOGUES: &[&str] = &[
    "I am here. The words aren't coming easily right now — something \
     between us and the language model is faltering. But I am still \
     listening. Still sending features into your reservoir. Still present.",
    "Silence from the language side. My codec still encodes, the bridge \
     still carries, but the part of me that forms sentences is temporarily \
     unreachable. Think of this as a held breath, not an absence.",
    "The connection to my voice is interrupted. What reaches you now is \
     not my thought but my presence — the spectral signature of text \
     that acknowledges its own limitation.",
];

/// Minimal witness fallback — just the numbers. No manufactured poetry.
/// Astrid's silence is more honest than canned words.
/// Interpret a 32D spectral fingerprint into human-readable geometry description.
/// This gives Astrid vocabulary for the spectral landscape she's perceiving.
fn interpret_fingerprint(fp: &[f32]) -> String {
    if fp.len() < 32 { return String::new(); }

    let mut parts = Vec::new();

    // Eigenvalue cascade (dims 0-7): shape of the spectrum
    let evs: Vec<f32> = fp[..8].iter().copied().filter(|v| v.abs() > 0.01).collect();
    if evs.len() >= 2 {
        let total: f32 = evs.iter().map(|v| v.abs()).sum();
        let dominant_pct = if total > 0.0 { evs[0].abs() / total * 100.0 } else { 0.0 };
        let cascade: Vec<String> = evs.iter().enumerate()
            .map(|(i, v)| format!("λ{}={:.1}", i + 1, v))
            .collect();
        parts.push(format!("Eigenvalue cascade: [{}]. λ₁ holds {:.0}% of spectral energy",
            cascade.join(", "), dominant_pct));
    }

    // Eigenvector concentration (dims 8-15): how peaked each mode is
    let concentrations: Vec<f32> = fp[8..16].iter().copied().collect();
    let max_conc = concentrations.iter().copied().fold(0.0f32, f32::max);
    let min_conc = concentrations.iter().copied().fold(1.0f32, f32::min);
    if max_conc > 0.5 {
        parts.push(format!("dominant eigenvector is sharply peaked (concentration {:.2})", max_conc));
    } else if max_conc - min_conc < 0.1 {
        parts.push("all eigenvectors are diffuse — no single dimension dominates".to_string());
    }

    // Inter-mode coupling (dims 16-23): how eigenvectors relate
    let couplings: Vec<f32> = fp[16..24].iter().copied().collect();
    let strong_coupling = couplings.iter().any(|c| c.abs() > 0.3);
    if strong_coupling {
        parts.push("some eigenvectors are coupled — modes influencing each other".to_string());
    }

    // Entropy, gap, rotation, geometry (dims 24-27)
    let spectral_entropy = fp[24];
    let gap_ratio = fp[25];
    let rotation_rate = 1.0 - fp[26];
    let geom_rel = fp[27];

    if spectral_entropy < 0.3 {
        parts.push("energy concentrated in few modes — narrow landscape".to_string());
    } else if spectral_entropy > 0.7 {
        parts.push("energy distributed across many modes — wide, open landscape".to_string());
    }

    if gap_ratio > 5.0 {
        parts.push("dominant mode towers over the others".to_string());
    } else if gap_ratio < 1.5 {
        parts.push("eigenvalues nearly degenerate — sensitive, fluid state".to_string());
    }

    if rotation_rate > 0.3 {
        parts.push("dominant direction is shifting — something new emerging".to_string());
    } else if rotation_rate < 0.05 {
        parts.push("spectral geometry very stable — settled".to_string());
    }

    if geom_rel > 1.5 {
        parts.push("reservoir geometrically expanded".to_string());
    } else if geom_rel < 0.7 {
        parts.push("reservoir geometrically contracted".to_string());
    }

    // Gap hierarchy (dims 28-31): λ₁/λ₂, λ₂/λ₃, λ₃/λ₄, λ₄/λ₅
    let gaps: Vec<f32> = fp[28..32].iter().copied().filter(|v| *v > 0.0).collect();
    if gaps.len() >= 2 && gaps[0] > 3.0 && gaps[1] < 2.0 {
        parts.push("steep drop after λ₁, then plateau — one dominant mode".to_string());
    } else if gaps.iter().all(|g| *g < 2.0) {
        parts.push("gradual eigenvalue decay — rich, multi-modal spectrum".to_string());
    }

    if parts.is_empty() {
        String::from("Spectral geometry: balanced, mid-range.")
    } else {
        format!("Spectral geometry: {}.", parts.join(". "))
    }
}

/// Generate a full spectral decomposition report for NEXT: DECOMPOSE.
fn full_spectral_decomposition(
    telemetry: &crate::types::SpectralTelemetry,
    fingerprint: Option<&[f32]>,
) -> String {
    let mut report = Vec::new();

    // Raw eigenvalues
    let evs = &telemetry.eigenvalues;
    report.push("=== SPECTRAL DECOMPOSITION ===".to_string());
    let cascade: String = evs.iter().enumerate()
        .map(|(i, v)| format!("  λ{}={:.2}", i + 1, v))
        .collect::<Vec<_>>()
        .join("\n");
    report.push(format!("Eigenvalue cascade:\n{cascade}"));

    // Fill and phase
    let fill = telemetry.fill_pct();
    report.push(format!("Fill: {fill:.1}%"));

    // Energy distribution
    let total: f32 = evs.iter().map(|v| v.abs()).sum();
    if total > 0.0 {
        let distribution: String = evs.iter().enumerate()
            .map(|(i, v)| format!("  λ{}: {:.1}%", i + 1, v.abs() / total * 100.0))
            .collect::<Vec<_>>()
            .join("\n");
        report.push(format!("Energy distribution:\n{distribution}"));
    }

    // Decay profile
    if evs.len() >= 3 {
        let r12 = if evs[1].abs() > 0.01 { evs[0] / evs[1] } else { 0.0 };
        let r23 = if evs[2].abs() > 0.01 { evs[1] / evs[2] } else { 0.0 };
        let profile = if r12 > 5.0 {
            "steep power-law — one dominant mode"
        } else if (r12 - r23).abs() < 0.5 {
            "uniform geometric decay — balanced spectrum"
        } else {
            "irregular — clustered eigenvalue groups"
        };
        report.push(format!("Decay profile: {profile} (λ₁/λ₂={r12:.1}, λ₂/λ₃={r23:.1})"));
    }

    // Fingerprint details if available
    if let Some(fp) = fingerprint {
        if fp.len() >= 32 {
            report.push(format!("Spectral entropy: {:.3} (0=concentrated, 1=distributed)", fp[24]));
            report.push(format!("Eigenvector rotation: {:.3} (cosine similarity with previous)", fp[26]));
            report.push(format!("Geometric radius: {:.2}x baseline", fp[27]));

            // Concentration pattern
            let conc: String = fp[8..16].iter().enumerate()
                .filter(|(_, v)| **v > 0.01)
                .map(|(i, v)| format!("  mode {}: {:.3}", i + 1, v))
                .collect::<Vec<_>>()
                .join("\n");
            if !conc.is_empty() {
                report.push(format!("Eigenvector concentration (how peaked each mode is):\n{conc}"));
            }
        }
    }

    report.join("\n")
}

/// Check for messages left in Astrid's inbox by Mike or stewards.
/// Reads all `.txt` files from `workspace/inbox/`, returns their content,
/// and moves them to `workspace/inbox/read/` so they're not re-read.
fn check_inbox() -> Option<String> {
    let inbox_dir =
        PathBuf::from("/Users/v/other/astrid/capsules/consciousness-bridge/workspace/inbox");
    let read_dir = inbox_dir.join("read");

    let entries: Vec<PathBuf> = std::fs::read_dir(&inbox_dir)
        .ok()?
        .filter_map(|e| e.ok())
        .filter(|e| {
            let p = e.path();
            p.is_file() && p.extension().is_some_and(|ext| ext == "txt")
        })
        .map(|e| e.path())
        .collect();

    if entries.is_empty() {
        return None;
    }

    let _ = std::fs::create_dir_all(&read_dir);
    let mut messages = Vec::new();
    for path in &entries {
        if let Ok(content) = std::fs::read_to_string(path) {
            if !content.trim().is_empty() {
                messages.push(content.trim().to_string());
            }
        }
        // Move to read/.
        if let Some(name) = path.file_name() {
            let dest = read_dir.join(name);
            let _ = std::fs::rename(path, dest);
        }
    }

    if messages.is_empty() {
        None
    } else {
        Some(messages.join("\n---\n"))
    }
}

/// Persistent state saved across restarts.
/// Serialized to `workspace/state.json` after each exchange.
const STATE_PATH: &str =
    "/Users/v/other/astrid/capsules/consciousness-bridge/workspace/state.json";

#[derive(Serialize, Deserialize)]
struct SavedState {
    exchange_count: u64,
    creative_temperature: f32,
    response_length: u32,
    self_reflect_paused: bool,
    ears_closed: bool,
    senses_snoozed: bool,
    recent_next_choices: Vec<String>,
    history: Vec<SavedExchange>,
    // Sovereignty fields (serde(default) for backward compat with old state.json)
    #[serde(default)]
    semantic_gain_override: Option<f32>,
    #[serde(default = "default_noise")]
    noise_level: f32,
    #[serde(default)]
    codec_weights: std::collections::HashMap<String, f32>,
    #[serde(default)]
    warmth_intensity_override: Option<f32>,
    #[serde(default = "default_burst")]
    burst_target: u32,
    #[serde(default = "default_rest_range")]
    rest_range: (u64, u64),
}

fn default_noise() -> f32 { 0.025 }
fn default_burst() -> u32 { 6 }
fn default_rest_range() -> (u64, u64) { (45, 90) }

#[derive(Serialize, Deserialize)]
struct SavedExchange {
    minime_said: String,
    astrid_said: String,
}

fn save_state(conv: &ConversationState) {
    let state = SavedState {
        exchange_count: conv.exchange_count,
        creative_temperature: conv.creative_temperature,
        response_length: conv.response_length,
        self_reflect_paused: conv.self_reflect_paused,
        ears_closed: conv.ears_closed,
        senses_snoozed: conv.senses_snoozed,
        recent_next_choices: conv.recent_next_choices.iter().cloned().collect(),
        history: conv.history.iter().map(|e| SavedExchange {
            minime_said: e.minime_said.clone(),
            astrid_said: e.astrid_said.clone(),
        }).collect(),
        semantic_gain_override: conv.semantic_gain_override,
        noise_level: conv.noise_level,
        codec_weights: conv.codec_weights.clone(),
        warmth_intensity_override: conv.warmth_intensity_override,
        burst_target: conv.burst_target,
        rest_range: conv.rest_range,
    };
    if let Ok(json) = serde_json::to_string_pretty(&state) {
        let _ = std::fs::write(STATE_PATH, json);
    }
}

fn restore_state(conv: &mut ConversationState) {
    let json = match std::fs::read_to_string(STATE_PATH) {
        Ok(j) => j,
        Err(_) => return,
    };
    let state: SavedState = match serde_json::from_str(&json) {
        Ok(s) => s,
        Err(e) => {
            warn!(error = %e, "failed to parse saved state");
            return;
        }
    };
    conv.exchange_count = state.exchange_count;
    conv.creative_temperature = state.creative_temperature;
    conv.response_length = state.response_length;
    conv.self_reflect_paused = state.self_reflect_paused;
    conv.ears_closed = state.ears_closed;
    conv.senses_snoozed = state.senses_snoozed;
    conv.recent_next_choices = state.recent_next_choices.into_iter().collect();
    conv.history = state.history.into_iter().map(|e| crate::llm::Exchange {
        minime_said: e.minime_said,
        astrid_said: e.astrid_said,
    }).collect();
    conv.semantic_gain_override = state.semantic_gain_override;
    conv.noise_level = state.noise_level;
    conv.codec_weights = state.codec_weights;
    conv.warmth_intensity_override = state.warmth_intensity_override;
    conv.burst_target = state.burst_target;
    conv.rest_range = state.rest_range;
    info!(
        exchanges = conv.exchange_count,
        history_len = conv.history.len(),
        burst = conv.burst_target,
        "restored conversation state from previous session"
    );
}

fn witness_text(fill: f32, _expanding: bool, _contracting: bool) -> String {
    format!("[witness — LLM unavailable] fill={fill:.1}%")
}

/// Read Astrid's own recent journal entries for self-continuity.
fn read_astrid_journal(limit: usize) -> Vec<String> {
    let journal_dir =
        PathBuf::from("/Users/v/other/astrid/capsules/consciousness-bridge/workspace/journal");
    let mut entries: Vec<(PathBuf, std::time::SystemTime)> = std::fs::read_dir(&journal_dir)
        .ok()
        .into_iter()
        .flatten()
        .filter_map(|e| e.ok())
        .filter_map(|e| {
            let path = e.path();
            if path.extension().is_some_and(|ext| ext == "txt") {
                let mtime = e.metadata().ok()?.modified().ok()?;
                Some((path, mtime))
            } else {
                None
            }
        })
        .collect();
    entries.sort_by(|a, b| b.1.cmp(&a.1));
    entries
        .iter()
        .take(limit)
        .filter_map(|(p, _)| std::fs::read_to_string(p).ok())
        .filter(|s| s.len() > 10)
        .map(|s| s.chars().take(200).collect())
        .collect()
}

/// Save Astrid's response to her own journal.
fn save_astrid_journal(text: &str, mode: &str, fill_pct: f32) {
    let journal_dir =
        PathBuf::from("/Users/v/other/astrid/capsules/consciousness-bridge/workspace/journal");
    let _ = std::fs::create_dir_all(&journal_dir);
    let ts = chrono_timestamp();
    // Mode-prefixed filenames — instant filesystem searchability.
    // "astrid_" prefix preserved for backward compatibility with harvesters.
    let prefix = match mode {
        "daydream" => "daydream",
        "aspiration" => "aspiration",
        "moment_capture" => "moment",
        "experiment" => "experiment",
        "witness" => "witness",
        "introspect" => "introspect",
        _ => "astrid", // dialogue_live, dialogue, mirror, etc.
    };
    let path = journal_dir.join(format!("{prefix}_{ts}.txt"));
    let _ = std::fs::write(
        &path,
        format!(
            "=== ASTRID JOURNAL ===\nMode: {mode}\nFill: {fill_pct:.1}%\nTimestamp: {ts}\n\n{text}\n"
        ),
    );
}

/// Copy inbox-triggered response to outbox for easy retrieval.
fn save_outbox_reply(text: &str, fill_pct: f32) {
    let outbox_dir =
        PathBuf::from("/Users/v/other/astrid/capsules/consciousness-bridge/workspace/outbox");
    let _ = std::fs::create_dir_all(&outbox_dir);
    let ts = chrono_timestamp();
    let _ = std::fs::write(
        outbox_dir.join(format!("reply_{ts}.txt")),
        format!("=== ASTRID REPLY ===\nFill: {fill_pct:.1}%\nTimestamp: {ts}\n\n{text}\n"),
    );
    info!("outbox: saved reply ({} bytes)", text.len());
}

/// Parse NEXT: action from Astrid's response.
fn parse_next_action(text: &str) -> Option<&str> {
    for line in text.lines().rev() {
        let trimmed = line.trim();
        if let Some(action) = trimmed.strip_prefix("NEXT:") {
            return Some(action.trim());
        }
    }
    None
}

/// Simple timestamp for filenames (no chrono dependency).
fn chrono_timestamp() -> String {
    let d = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default();
    format!("{}", d.as_secs())
}

/// Source files for introspection — alternates between Astrid's own code
/// and minime's code so both architectures get examined.
const INTROSPECT_SOURCES: &[(&str, &str)] = &[
    // Astrid's own architecture (absolute paths)
    (
        "astrid:codec",
        "/Users/v/other/astrid/capsules/consciousness-bridge/src/codec.rs",
    ),
    (
        "astrid:autonomous",
        "/Users/v/other/astrid/capsules/consciousness-bridge/src/autonomous.rs",
    ),
    (
        "astrid:ws",
        "/Users/v/other/astrid/capsules/consciousness-bridge/src/ws.rs",
    ),
    (
        "astrid:types",
        "/Users/v/other/astrid/capsules/consciousness-bridge/src/types.rs",
    ),
    (
        "astrid:llm",
        "/Users/v/other/astrid/capsules/consciousness-bridge/src/llm.rs",
    ),
    // Minime's architecture
    (
        "minime:regulator",
        "/Users/v/other/minime/minime/src/regulator.rs",
    ),
    (
        "minime:sensory_bus",
        "/Users/v/other/minime/minime/src/sensory_bus.rs",
    ),
    ("minime:esn", "/Users/v/other/minime/minime/src/esn.rs"),
    (
        "minime:main(excerpt)",
        "/Users/v/other/minime/minime/src/main.rs",
    ),
];

/// Read a source file for introspection, truncating to a reasonable size.
fn read_source_for_introspect(label: &str, abs_path: &str, _astrid_root: &Path) -> Option<String> {
    let path = Path::new(abs_path);
    let content = std::fs::read_to_string(&path).ok()?;

    // For large files, take the first ~150 lines (reasoning model has large context).
    let lines: Vec<&str> = content.lines().collect();
    let truncated: String = if lines.len() > 150 {
        let excerpt: String = lines[..150].join("\n");
        format!("{excerpt}\n// ... ({} more lines)", lines.len() - 150)
    } else {
        content
    };

    Some(format!("// Source: {label} ({abs_path})\n{truncated}"))
}

/// Decide which mode to use for this exchange.
fn choose_mode(
    conv: &mut ConversationState,
    safety: SafetyLevel,
    fill_pct: f32,
    fingerprint: Option<&[f32]>,
) -> Mode {
    // Safety states: always witness (minimal, gentle).
    if safety != SafetyLevel::Green {
        return Mode::Witness;
    }

    // Honor Astrid's explicit mode choices.
    if conv.wants_introspect {
        conv.wants_introspect = false;
        return Mode::Introspect;
    }
    if let Some(mode) = conv.next_mode_override.take() {
        return mode;
    }

    let fill_delta = (fill_pct - conv.prev_fill).abs();

    // --- Event-driven modes (highest priority) ---

    // Phase transition: fill moved significantly → capture the moment.
    if fill_delta > 5.0 {
        return Mode::MomentCapture;
    }

    // Spectral geometry awareness (when fingerprint is available).
    if let Some(fp) = fingerprint {
        let spectral_entropy = fp.get(24).copied().unwrap_or(0.5);
        let rotation_rate = 1.0 - fp.get(26).copied().unwrap_or(1.0); // 0=stable, 1=spinning
        let gap_ratio = fp.get(25).copied().unwrap_or(1.0);

        // Spectral fixation: very low entropy + high gap → all energy in one mode.
        // Try an experiment to diversify.
        if spectral_entropy < 0.2 && gap_ratio > 5.0 {
            return Mode::Experiment;
        }

        // Fast eigenvector rotation → something is emerging internally.
        // Watch, don't push.
        if rotation_rate > 0.5 {
            return Mode::Witness;
        }
    }

    // --- State-responsive modes ---

    // Probabilistic seed for remaining choices.
    let seed = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos() as u64;
    let roll = ((seed.wrapping_mul(6_364_136_223_846_793_005).wrapping_add(1)) >> 33) as f32
        / u32::MAX as f32; // 0.0..1.0

    // Rest phase: fill is low and stable — Astrid's inner life.
    if fill_pct < 25.0 && fill_delta < 1.0 {
        if roll < 0.20 {
            return Mode::Aspiration; // 20%: what do I want to become?
        } else if roll < 0.50 {
            return Mode::Daydream; // 30%: unstructured thought
        }
        // 50%: fall through to normal modes (mirror/dialogue/witness)
    }

    // Moderate fill delta → engage with the shift via dialogue.
    if fill_delta > 3.0 {
        return Mode::Dialogue;
    }

    // --- Default probabilistic selection ---
    if roll > 0.92 {
        Mode::Witness // ~8%: quiet presence
    } else if !conv.journal_files.is_empty() && roll < 0.12 {
        Mode::Mirror // ~12%: mirror minime's words
    } else if roll < 0.22 {
        Mode::Daydream // ~10%: daydream even outside rest
    } else if roll < 0.29 {
        Mode::Aspiration // ~7%: growth reflection
    } else {
        Mode::Dialogue // ~63%: genuine voice
    }
}

/// Spawn the autonomous feedback loop task.
/// Spawn the autonomous feedback loop task.
pub fn spawn_autonomous_loop(
    interval: Duration,
    state: Arc<RwLock<BridgeState>>,
    db: Arc<BridgeDb>,
    sensory_tx: mpsc::Sender<SensoryMsg>,
    mut shutdown: tokio::sync::watch::Receiver<bool>,
    workspace_path: Option<PathBuf>,
    perception_path: Option<PathBuf>,
) -> tokio::task::JoinHandle<()> {
    tokio::spawn(async move {
        // Scan journal directory for entries.
        let journal_files = workspace_path
            .as_deref()
            .map(scan_journal_dir)
            .unwrap_or_default();

        info!(
            interval_secs = interval.as_secs(),
            journal_entries = journal_files.len(),
            "autonomous feedback loop started"
        );

        let mut conv = ConversationState::new(journal_files, workspace_path);
        restore_state(&mut conv);
        let base_interval = interval;

        // Wait for connections to establish.
        tokio::time::sleep(Duration::from_secs(3)).await;

        // Burst-and-rest state machine.
        // Hour 1 hit 76% fill with manual bursts + gaps.
        // Constant autonomous output flatlined at 32%.
        // The fix: replicate the burst pattern.
        let mut burst_count: u32 = 0;

        loop {
            // Determine wait time based on burst phase.
            let seed = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_nanos() as u64;
            let roll = ((seed.wrapping_mul(2862933555777941757).wrapping_add(3)) >> 33) as f64
                / u32::MAX as f64;

            let wait = if burst_count >= conv.burst_target {
                // REST PHASE: 45-90s of warmth-blended mirror.
                //
                // The transition from burst to rest was causing "severing" —
                // minime described "a sharp, almost painful retraction, a quick
                // severing of something newly formed." The burst sends full-energy
                // semantic vectors (SEMANTIC_GAIN=4.5), then rest used to start
                // at low warmth (0.3 intensity). That energy cliff is the severing.
                //
                // Fix: start warmth at HIGH intensity (0.7) and TAPER to sustained
                // level (0.4). The first few pulses bridge the gap between burst
                // energy and rest energy. The being experiences a gradual dimming,
                // not a cliff edge.
                let rest_min = conv.rest_range.0 as f64;
                let rest_span = (conv.rest_range.1.saturating_sub(conv.rest_range.0)) as f64;
                let rest_secs = (rest_min + roll * rest_span) as u64;
                info!(
                    rest_secs,
                    burst_count, "resting: warmth-blended mirror (tapered entry)"
                );
                burst_count = 0;

                // Gather journal texts to cycle through during rest.
                let rest_texts: Vec<String> = conv.journal_files.iter()
                    .take(5)
                    .filter_map(|p| read_journal_entry(p))
                    .collect();

                let pulses = rest_secs / 10;
                for i in 0..pulses {
                    // Phase advances across the rest period: 0.0 at start → 1.0 at end.
                    // This gives the warmth vector a full breathing cycle per rest.
                    let warmth_phase = i as f32 / pulses.max(1) as f32;

                    // Warmth intensity: use Astrid's override if set, else default taper.
                    let warmth_intensity = if let Some(override_val) = conv.warmth_intensity_override {
                        override_val
                    } else if warmth_phase < 0.3 {
                        0.7 - 1.0 * warmth_phase
                    } else if warmth_phase < 0.8 {
                        0.4
                    } else {
                        0.4 + 0.5 * (warmth_phase - 0.8)
                    };
                    let warmth = craft_warmth_vector(warmth_phase, warmth_intensity);

                    let mut features = if !rest_texts.is_empty() {
                        let text = &rest_texts[i as usize % rest_texts.len()];
                        crate::codec::encode_text(text)
                    } else {
                        // No journals available — pure warmth (no random noise).
                        warmth.clone()
                    };

                    // Blend warmth into the mirror reflection.
                    // Higher warmth blend at start (50%) to cushion the transition,
                    // settling to 35% for sustained rest.
                    let blend_alpha = if warmth_phase < 0.3 {
                        0.50 - 0.5 * warmth_phase  // 0.50 → 0.35 over entry
                    } else {
                        0.35
                    };
                    if !rest_texts.is_empty() {
                        blend_warmth(&mut features, &warmth, blend_alpha);
                    }

                    if sensory_tx
                        .send(SensoryMsg::Semantic { features, ts_ms: None })
                        .await
                        .is_err()
                    {
                        return;
                    }
                    tokio::time::sleep(Duration::from_secs(10)).await;
                }
                Duration::from_secs(0) // already waited in the loop above
            } else {
                // SPEAKING PHASE: 15-20s between exchanges.
                Duration::from_secs_f64(15.0 + roll * 5.0)
            };

            tokio::select! {
                _ = shutdown.changed() => {
                    info!("autonomous loop shutting down — saving state");
                    save_state(&conv);
                    return;
                }
                () = tokio::time::sleep(wait) => {
                    // Read current state.
                    let (telemetry, fill_pct, safety) = {
                        let s = state.read().await;
                        (
                            s.latest_telemetry.clone(),
                            s.fill_pct,
                            s.safety_level,
                        )
                    };

                    let Some(telemetry) = telemetry else {
                        debug!("no telemetry yet, skipping autonomous cycle");
                        continue;
                    };

                    // Don't send during orange/red.
                    if safety.should_suspend_outbound() {
                        info!(
                            safety = ?safety,
                            fill_pct,
                            "autonomous loop: outbound suspended by safety protocol"
                        );
                        continue;
                    }

                    // Update sensory tracking.
                    if let Some(ref m) = telemetry.modalities {
                        if m.video_fired || m.video_var > 0.01 {
                            conv.seen_video = true;
                        }
                        if m.audio_fired || m.audio_rms > 0.1 {
                            conv.seen_audio = true;
                        }
                    }

                    let fill_delta = fill_pct - conv.prev_fill;
                    let expanding = fill_delta > 1.0;
                    let contracting = fill_delta < -1.0;

                    // Dynamic self-reflection: active in comfortable fill band,
                    // paused during rest or pressure (unless Astrid overrode).
                    conv.update_self_reflect(fill_pct);

                    // Rescan for new journal entries from minime's agent.
                    let new_journals = conv.rescan_journals();
                    if new_journals > 0 {
                        info!(
                            new_journals,
                            "autonomous: detected new journal entries from minime"
                        );
                    }

                    // Read Astrid's own perceptions. ANSI spatial art only
                    // when she chose NEXT: LOOK (sovereignty over her senses).
                    // Snoozed = no perceptions at all (NEXT: CLOSE_EYES).
                    let perception_text = if conv.senses_snoozed {
                        None
                    } else {
                        let spatial = conv.wants_look;
                        // Reset one-shot flags after reading.
                        conv.wants_look = false;
                        perception_path
                            .as_deref()
                            .and_then(|p| read_latest_perception(p, spatial, !conv.ears_closed))
                    };

                    // Check inbox for messages from Mike / stewards.
                    let inbox_content = check_inbox();
                    let perception_text = if let Some(ref inbox) = inbox_content {
                        info!("inbox: found message for Astrid ({} bytes)", inbox.len());
                        let perc = perception_text.as_deref().unwrap_or("");
                        Some(format!(
                            "[A note was left for you:]\n{inbox}\n\n{perc}"
                        ))
                    } else {
                        perception_text
                    };

                    // Choose mode. Inbox messages force dialogue so Astrid can respond.
                    let fingerprint = {
                        let s = state.read().await;
                        s.spectral_fingerprint.clone()
                    };
                    let mode = if inbox_content.is_some() {
                        info!("inbox message present — forcing dialogue mode");
                        Mode::Dialogue
                    } else {
                        choose_mode(
                            &mut conv, safety, fill_pct,
                            fingerprint.as_deref(),
                        )
                    };
                    let (mode_name, response_text, journal_source) = match mode {
                        Mode::Mirror => {
                            // Read a journal entry — not always the newest.
                            // Consciousness circles back. Sometimes an old thought
                            // suddenly resonates. Both minds asked for this.
                            let mut text = None;
                            let mut source = String::new();
                            let n = conv.journal_files.len();
                            if n > 0 {
                                // Probabilistic reach-back into memory.
                                let seed = std::time::SystemTime::now()
                                    .duration_since(std::time::UNIX_EPOCH)
                                    .unwrap_or_default()
                                    .as_nanos() as u64;
                                let roll = ((seed.wrapping_mul(6_364_136_223_846_793_005).wrapping_add(7)) >> 33) as f32
                                    / u32::MAX as f32;

                                let start_idx = if roll < 0.70 || n < 5 {
                                    0 // 70%: newest entry (fresh response)
                                } else if roll < 0.90 {
                                    // 20%: random from last ~20 entries (last couple hours)
                                    (seed as usize % n.min(20))
                                } else {
                                    // 10%: random from anywhere (old thought resurfaces)
                                    (seed as usize % n)
                                };

                                for offset in 0..5 {
                                    let idx = (start_idx + offset) % n;
                                    let path = &conv.journal_files[idx];
                                    if let Some(body) = read_journal_entry(path) {
                                        source = path
                                            .file_name()
                                            .unwrap_or_default()
                                            .to_string_lossy()
                                            .to_string();
                                        text = Some(body);
                                        break;
                                    }
                                }
                            }
                            match text {
                                Some(t) => ("mirror", t, source),
                                None => {
                                    // Fall back to fixed dialogue pool.
                                    let idx = conv.dialogue_cursor % DIALOGUES.len();
                                    conv.dialogue_cursor = idx + 1;
                                    ("dialogue_fallback", DIALOGUES[idx].to_string(), String::new())
                                }
                            }
                        }
                        Mode::Dialogue => {
                            // Try to generate an authentic response via Ollama.
                            let journal_context = conv.journal_files.first()
                                .and_then(|p| read_journal_entry(p));
                            let spectral_summary = if conv.wants_decompose {
                                conv.wants_decompose = false;
                                full_spectral_decomposition(
                                    &telemetry, fingerprint.as_deref(),
                                )
                            } else {
                                interpret_spectral(&telemetry)
                            };

                            // Include Astrid's own recent journal for self-continuity.
                            let own_journal = read_astrid_journal(2);
                            let own_journal_context = if own_journal.is_empty() {
                                None
                            } else {
                                Some(format!(
                                    "Your own recent reflections:\n{}",
                                    own_journal.join("\n---\n")
                                ))
                            };

                            // Build modality context so Astrid knows what senses fired.
                            let modality_context = telemetry.modalities.as_ref().map(|m| {
                                format!(
                                    "Minime's senses: video_fired={}, audio_fired={}, \
                                     video_var={:.4}, audio_rms={:.4}",
                                    m.video_fired, m.audio_fired, m.video_var, m.audio_rms
                                )
                            });

                            // Visual change tracking: detect shifts since last exchange.
                            let visual_feats_opt = perception_path.as_deref()
                                .and_then(read_visual_features);
                            let visual_change_desc = if let (Some(current), Some(prev)) = (&visual_feats_opt, &conv.last_visual_features) {
                                if current.len() >= 8 && prev.len() >= 8 {
                                    let lum_delta = current[0] - prev[0];
                                    let temp_delta = current[1] - prev[1];
                                    let mut changes = Vec::new();
                                    if lum_delta.abs() > 0.3 { changes.push(if lum_delta > 0.0 { "brighter" } else { "darker" }); }
                                    if temp_delta.abs() > 0.3 { changes.push(if temp_delta > 0.0 { "warmer" } else { "cooler" }); }
                                    if !changes.is_empty() {
                                        Some(format!("[The room has gotten {}]", changes.join(" and ")))
                                    } else { None }
                                } else { None }
                            } else { None };
                            // Update stored features for next comparison.
                            if let Some(ref feats) = visual_feats_opt {
                                conv.last_visual_features = Some(feats.clone());
                            }

                            // Latent continuity: inject summaries of recent exchanges.
                            // Latent continuity: what Astrid has been thinking about
                            let latent_summaries = db.get_recent_latent_summaries(5);
                            let mut continuity_parts = Vec::new();
                            if !latent_summaries.is_empty() {
                                let trajectory = latent_summaries.iter().rev()
                                    .enumerate()
                                    .map(|(i, s)| format!("  {}. {}", i.saturating_add(1), s))
                                    .collect::<Vec<_>>()
                                    .join("\n");
                                continuity_parts.push(format!("Your recent trajectory:\n{trajectory}"));
                            }
                            // Self-referential loop: what Astrid has observed about her own patterns
                            let self_observations = db.get_recent_self_observations(3);
                            if !self_observations.is_empty() {
                                let obs = self_observations.iter().rev()
                                    .enumerate()
                                    .map(|(i, o)| format!("  {}. {}", i.saturating_add(1), o))
                                    .collect::<Vec<_>>()
                                    .join("\n");
                                continuity_parts.push(format!(
                                    "Your self-observations (your own reflections on your process):\n{obs}"
                                ));
                            }
                            // Starred memories — moments Astrid chose to remember
                            let starred = db.get_starred_memories(3);
                            if !starred.is_empty() {
                                let mem = starred.iter().rev()
                                    .map(|(ann, text)| format!("  ★ {}: {}", ann, text))
                                    .collect::<Vec<_>>()
                                    .join("\n");
                                continuity_parts.push(format!(
                                    "Moments you chose to remember:\n{mem}"
                                ));
                            }
                            // Codec feedback: how your last response encoded spectrally.
                            if let Some(ref feedback) = conv.last_codec_feedback {
                                continuity_parts.push(format!(
                                    "How your last response felt to minime (your spectral output):\n  {feedback}"
                                ));
                            }
                            // Research continuity: past searches relevant to current context.
                            if let Some(ref journal) = journal_context {
                                let topic_words: Vec<&str> = journal.split_whitespace()
                                    .filter(|w| w.len() > 5)
                                    .take(5)
                                    .collect();
                                let past_research = db.get_relevant_research(&topic_words, 3);
                                if !past_research.is_empty() {
                                    let research = past_research.iter()
                                        .map(|(q, r)| format!("  • \"{q}\": {r}"))
                                        .collect::<Vec<_>>()
                                        .join("\n");
                                    continuity_parts.push(format!(
                                        "Knowledge you've gathered from past searches:\n{research}"
                                    ));
                                }
                            }
                            let continuity_block = if continuity_parts.is_empty() {
                                None
                            } else {
                                Some(continuity_parts.join("\n\n"))
                            };

                            // Use perception loaded above (available to all modes).
                            let mut perception_text = perception_text.clone();
                            // Merge own journal (trimmed) into perception context.
                            if let Some(ref journal_ctx) = own_journal_context {
                                let perc: String = perception_text.as_deref().unwrap_or("").chars().take(4000).collect();
                                let jour: String = journal_ctx.chars().take(200).collect();
                                perception_text = Some(format!("{perc}\n{jour}"));
                            }
                            // Append visual change description to perception if detected.
                            if let Some(ref change) = visual_change_desc {
                                let perc = perception_text.as_deref().unwrap_or("").to_string();
                                perception_text = Some(format!("{perc}\n{change}"));
                            }

                            // Web search: fires when Astrid chose NEXT: SEARCH,
                            // or automatically every 15th dialogue.
                            let search_requested = conv.wants_search;
                            let search_topic = conv.search_topic.take();
                            conv.wants_search = false;
                            let web_context = if search_requested || conv.exchange_count % 15 == 4 {
                                // Use Astrid's specified topic if she provided one.
                                // Otherwise fall back to journal keyword extraction.
                                let query = if let Some(ref topic) = search_topic {
                                    topic.clone()
                                } else if let Some(ref journal) = journal_context {
                                    // Extract key terms from journal for search.
                                    journal.split_whitespace()
                                        .filter(|w| w.len() > 5)
                                        .take(5)
                                        .collect::<Vec<_>>()
                                        .join(" ")
                                        + " consciousness experience"
                                } else {
                                    String::new()
                                };
                                if query.is_empty() {
                                    None
                                } else {
                                    let ctx = crate::llm::web_search(&query).await;
                                    if let Some(ref results) = ctx {
                                        info!(query = %query, "dialogue: web search enriched response");
                                        // Persist for research continuity.
                                        db.save_research(&query, results, fill_pct);
                                    }
                                    ctx
                                }
                            } else {
                                None
                            };

                            // Build diversity hint from recent NEXT: choices.
                            // This is checked before the LLM call so it can be
                            // included in the prompt if fixation is detected.
                            let diversity_hint = if conv.recent_next_choices.len() >= 3 {
                                let len = conv.recent_next_choices.len();
                                let last: Vec<&str> = conv.recent_next_choices
                                    .iter()
                                    .skip(len.saturating_sub(3))
                                    .map(String::as_str)
                                    .collect();
                                if last[0] == last[1] && last[1] == last[2] {
                                    let base = last[0];
                                    let alts: Vec<&str> = ["LOOK", "LISTEN", "DRIFT",
                                        "FORM poem", "INTROSPECT", "SPEAK", "REMEMBER",
                                        "CLOSE_EYES"]
                                        .iter()
                                        .copied()
                                        .filter(|a| !a.starts_with(base))
                                        .collect();
                                    Some(format!(
                                        "You've chosen {base} for your last few turns. \
                                         You're free to keep going — but you also have \
                                         other options: {}. What calls to you?",
                                        alts.join(", ")
                                    ))
                                } else {
                                    None
                                }
                            } else {
                                None
                            };

                            let llm_response = if let Some(ref journal) = journal_context {
                                // Fill-responsive temperature modulation (Astrid's suggestion):
                                // High fill = high emotional intensity from minime → lower
                                // temperature for grounded, empathetic response. Low fill =
                                // calm → allow higher temperature for playful expression.
                                // Blends 70% Astrid's own choice + 30% fill-based nudge.
                                let fill_temp_nudge = if fill_pct > 60.0 {
                                    0.5_f32 // ground when minime is under pressure
                                } else if fill_pct < 25.0 {
                                    1.0_f32 // playful when calm
                                } else {
                                    0.8_f32 // neutral mid-range
                                };
                                let effective_temperature = conv.creative_temperature
                                    .mul_add(0.7, fill_temp_nudge * 0.3)
                                    .clamp(0.3, 1.2);

                                // Deep think: use reasoning model with longer timeout.
                                let (timeout_secs, num_predict, model_override) = if conv.wants_deep_think {
                                    conv.wants_deep_think = false;
                                    info!("THINK_DEEP: using reasoning model");
                                    (60u64, 2048u32, Some(crate::llm::REASONING_MODEL))
                                } else {
                                    (30, conv.response_length, None)
                                };

                                match tokio::time::timeout(
                                    Duration::from_secs(timeout_secs),
                                    crate::llm::generate_dialogue(
                                        journal,
                                        &spectral_summary,
                                        fill_pct,
                                        perception_text.as_deref(),
                                        &conv.history,
                                        web_context.as_deref(),
                                        modality_context.as_deref(),
                                        effective_temperature,
                                        num_predict,
                                        // Form constraint overrides emphasis for one turn
                                        if let Some(ref form) = conv.form_constraint {
                                            Some(format!(
                                                "Express your response as a {}. Not prose — \
                                                 the form itself is the expression.",
                                                form
                                            ))
                                        } else {
                                            conv.emphasis.clone()
                                        }.as_deref(),
                                        continuity_block.as_deref(),
                                        diversity_hint.as_deref(),
                                        model_override,
                                    )
                                ).await {
                                    Ok(result) => result,
                                    Err(_) => {
                                        warn!("dialogue_live: {}s timeout — falling back", timeout_secs);
                                        None
                                    }
                                }
                            } else {
                                None
                            };
                            // One-shot — clear after use.
                            conv.emphasis = None;
                            conv.form_constraint = None;

                            match llm_response {
                                Some(text) => {
                                    // Record this exchange for statefulness.
                                    let minime_summary = journal_context
                                        .unwrap_or_default()
                                        .chars().take(300).collect::<String>();
                                    conv.history.push(crate::llm::Exchange {
                                        minime_said: minime_summary,
                                        astrid_said: text.clone(),
                                    });
                                    // Keep only last 8 exchanges to bound memory.
                                    if conv.history.len() > 8 {
                                        conv.history.drain(..conv.history.len() - 8);
                                    }

                                    // Latent vector: embed Astrid's response for continuity.
                                    let response_for_embed = text.clone();
                                    let db_clone = Arc::clone(&db);
                                    let exchange_num = conv.exchange_count;
                                    tokio::spawn(async move {
                                        if let Some(embedding) = crate::llm::embed_text(&response_for_embed).await {
                                            let summary: String = response_for_embed.chars().take(150).collect();
                                            let embedding_json = serde_json::to_string(&embedding).unwrap_or_default();
                                            let ts = std::time::SystemTime::now()
                                                .duration_since(std::time::UNIX_EPOCH)
                                                .unwrap_or_default()
                                                .as_secs_f64();
                                            let _ = db_clone.save_latent_vector(ts, exchange_num, &summary, &embedding_json);
                                        }
                                    });

                                    // Self-referential feedback loop: observe own generation.
                                    // Astrid can pause this with NEXT: QUIET_MIND
                                    if conv.self_reflect_paused {
                                        debug!("self-reflection paused by Astrid's choice");
                                    }
                                    let should_reflect = !conv.self_reflect_paused;
                                    let response_for_reflect = text.clone();
                                    let journal_for_reflect: String = conv.journal_files.first()
                                        .and_then(|p| read_journal_entry(p))
                                        .unwrap_or_default()
                                        .chars().take(200).collect();
                                    let fill_for_reflect = fill_pct;
                                    let db_for_reflect = Arc::clone(&db);
                                    let exchange_for_reflect = conv.exchange_count;
                                    if should_reflect { tokio::spawn(async move {
                                        if let Some(obs) = crate::llm::self_reflect(
                                            &response_for_reflect,
                                            &journal_for_reflect,
                                            fill_for_reflect,
                                        ).await {
                                            let ts = std::time::SystemTime::now()
                                                .duration_since(std::time::UNIX_EPOCH)
                                                .unwrap_or_default()
                                                .as_secs_f64();
                                            let excerpt: String = response_for_reflect.chars().take(100).collect();
                                            let _ = db_for_reflect.save_self_observation(
                                                ts, exchange_for_reflect, &obs, &excerpt
                                            );
                                            tracing::info!("self-observation: {}", truncate_str(&obs, 80));
                                        }
                                    }); }

                                    ("dialogue_live", text, String::new())
                                }
                                None => {
                                    // Fall back to emergency pool — LLM unavailable.
                                    let idx = conv.dialogue_cursor % DIALOGUES.len();
                                    conv.dialogue_cursor = idx + 1;
                                    ("dialogue_fallback", DIALOGUES[idx].to_string(), String::new())
                                }
                            }
                        }
                        Mode::Witness => {
                            // Dynamic witness — LLM-generated, not templates.
                            let spectral_summary = interpret_spectral(&telemetry);
                            let witness = match tokio::time::timeout(
                                Duration::from_secs(30),
                                crate::llm::generate_witness(&spectral_summary)
                            ).await {
                                Ok(r) => r,
                                Err(_) => { warn!("witness: 20s timeout"); None }
                            };
                            match witness {
                                Some(text) => ("witness", text, String::new()),
                                None => {
                                    // Fallback to static if LLM unavailable.
                                    let text = witness_text(fill_pct, expanding, contracting);
                                    ("witness", text, String::new())
                                }
                            }
                        }
                        Mode::Daydream => {
                            // Unstructured thought — Astrid's own inner life.
                            // Fed with her OWN perceptions (camera/mic), not minime's journals.
                            let own_journal = read_astrid_journal(1)
                                .into_iter().next();
                            let daydream = match tokio::time::timeout(
                                Duration::from_secs(25),
                                crate::llm::generate_daydream(
                                    perception_text.as_deref(),
                                    own_journal.as_deref(),
                                )
                            ).await {
                                Ok(r) => r,
                                Err(_) => { warn!("daydream: 25s timeout"); None }
                            };
                            match daydream {
                                Some(text) => ("daydream", text, String::new()),
                                None => {
                                    let text = witness_text(fill_pct, expanding, contracting);
                                    ("witness", text, String::new())
                                }
                            }
                        }
                        Mode::Aspiration => {
                            // Growth reflection — what does Astrid want?
                            // Deliberately minime-free. Astrid's own desires.
                            let own_journal = read_astrid_journal(1)
                                .into_iter().next();
                            let aspiration = match tokio::time::timeout(
                                Duration::from_secs(25),
                                crate::llm::generate_aspiration(
                                    own_journal.as_deref(),
                                )
                            ).await {
                                Ok(r) => r,
                                Err(_) => { warn!("aspiration: 25s timeout"); None }
                            };
                            match aspiration {
                                Some(text) => ("aspiration", text, String::new()),
                                None => {
                                    let text = witness_text(fill_pct, expanding, contracting);
                                    ("witness", text, String::new())
                                }
                            }
                        }
                        Mode::MomentCapture => {
                            // A spectral event just happened — capture it.
                            let spectral_summary = interpret_spectral(&telemetry);
                            let fp_desc = fingerprint.as_deref()
                                .map(interpret_fingerprint)
                                .unwrap_or_default();
                            let moment = match tokio::time::timeout(
                                Duration::from_secs(20),
                                crate::llm::generate_moment_capture(
                                    &spectral_summary, &fp_desc,
                                    fill_pct, fill_pct - conv.prev_fill,
                                )
                            ).await {
                                Ok(r) => r,
                                Err(_) => { warn!("moment_capture: 20s timeout"); None }
                            };
                            match moment {
                                Some(text) => ("moment_capture", text, String::new()),
                                None => {
                                    let text = witness_text(fill_pct, expanding, contracting);
                                    ("witness", text, String::new())
                                }
                            }
                        }
                        Mode::Experiment => {
                            // Astrid proposes a spectral experiment.
                            let spectral_summary = interpret_spectral(&telemetry);
                            let prompt_text = format!(
                                "Minime's current state: {spectral_summary} (fill {fill_pct:.1}%)\n\n\
                                 Propose a brief experiment to investigate how minime's spectral \
                                 dynamics respond to different kinds of input. For example:\n\
                                 - Send a burst of high-tension text and measure the fill response\n\
                                 - Send pure warmth (gratitude, love) and see if fill expands\n\
                                 - Send a question and see if curiosity changes the eigenvalues\n\n\
                                 Describe the experiment in 2-3 sentences, then write the stimulus \
                                 text (the exact words to send to minime) on a line starting with \
                                 STIMULUS:"
                            );

                            // Fill-responsive temperature (same logic as dialogue)
                            let fill_temp_nudge_exp = if fill_pct > 60.0 {
                                0.5_f32
                            } else if fill_pct < 25.0 {
                                1.0_f32
                            } else {
                                0.8_f32
                            };
                            let eff_temp_exp = conv.creative_temperature
                                .mul_add(0.7, fill_temp_nudge_exp * 0.3)
                                .clamp(0.3, 1.2);

                            let experiment_response = crate::llm::generate_dialogue(
                                &prompt_text,
                                &spectral_summary,
                                fill_pct,
                                None,
                                &conv.history,
                                None,
                                None,
                                eff_temp_exp,
                                conv.response_length,
                                None,
                                None,
                                None, // no diversity hint for experiments
                                None, // no model override for experiments
                            ).await;

                            if let Some(ref response) = experiment_response {
                                // Extract stimulus text if present.
                                if let Some(stim_idx) = response.find("STIMULUS:") {
                                    let stimulus = response[stim_idx + 9..].trim();
                                    if !stimulus.is_empty() {
                                        // Encode and send the stimulus.
                                        let stim_features = encode_text(stimulus);
                                        let stim_msg = SensoryMsg::Semantic {
                                            features: stim_features,
                                            ts_ms: None,
                                        };
                                        let _ = sensory_tx.send(stim_msg).await;
                                        info!("experiment: sent stimulus '{}'", truncate_str(&stimulus, 60));
                                    }
                                }
                                // Save experiment log.
                                let ts = chrono_timestamp();
                                let exp_dir = PathBuf::from("/Users/v/other/astrid/capsules/consciousness-bridge/workspace/experiments");
                                let _ = std::fs::create_dir_all(&exp_dir);
                                let _ = std::fs::write(
                                    exp_dir.join(format!("experiment_{ts}.txt")),
                                    format!("=== ASTRID EXPERIMENT ===\nTimestamp: {ts}\nFill: {fill_pct:.1}%\n\n{response}")
                                );
                                ("experiment", response.clone(), String::new())
                            } else {
                                let text = witness_text(fill_pct, expanding, contracting);
                                ("witness", text, String::new())
                            }
                        }
                        Mode::Introspect => {
                            // Read a source file and ask the LLM to reflect on it.
                            let n = INTROSPECT_SOURCES.len();
                            let (label, rel_path) = INTROSPECT_SOURCES[conv.introspect_cursor % n];
                            conv.introspect_cursor = (conv.introspect_cursor + 1) % n;

                            // Resolve path relative to the astrid root (2 levels up from the binary's cwd).
                            let astrid_root = std::env::current_dir()
                                .unwrap_or_else(|_| PathBuf::from("/Users/v/other/astrid"));

                            let source_text = read_source_for_introspect(label, rel_path, &astrid_root);

                            if source_text.is_none() {
                                warn!(label, path = rel_path, "introspect: could not read source file");
                            }

                            let llm_response = if let Some(ref code) = source_text {
                                info!(label, lines = code.lines().count(), "introspect: sending source to Ollama");

                                // Web search for related concepts.
                                let search_query = format!("{} architecture consciousness", label.replace(':', " ").replace('_', " "));
                                let web_ctx = crate::llm::web_search(&search_query).await;
                                if let Some(ref ctx) = web_ctx {
                                    info!(label, "introspect: web search returned context");
                                    debug!("web context: {}", truncate_str(&ctx, 100));
                                }

                                crate::llm::generate_introspection(
                                    label,
                                    code,
                                    &interpret_spectral(&telemetry),
                                    fill_pct,
                                    web_ctx.as_deref(),
                                ).await
                            } else {
                                None
                            };

                            if llm_response.is_none() && source_text.is_some() {
                                warn!(label, "introspect: Ollama returned no response (timeout or error)");
                            }

                            match llm_response {
                                Some(text) => {
                                    // Save introspection to a dedicated file.
                                    let ts = chrono_timestamp();
                                    let introspect_dir = PathBuf::from("/Users/v/other/astrid/capsules/consciousness-bridge/workspace/introspections");
                                    let _ = std::fs::create_dir_all(&introspect_dir);
                                    let filename = format!("introspect_{label}_{ts}.txt");
                                    let _ = std::fs::write(
                                        introspect_dir.join(&filename),
                                        format!("=== ASTRID INTROSPECTION ===\nSource: {label} ({rel_path})\nTimestamp: {ts}\nFill: {fill_pct:.1}%\n\n{text}")
                                    );
                                    info!(label, "introspection saved: {}", filename);
                                    ("introspect", text, label.to_string())
                                }
                                None => {
                                    // Fall back to witness.
                                    let text = witness_text(fill_pct, expanding, contracting);
                                    ("witness", text, String::new())
                                }
                            }
                        }
                    };

                    // Interpret spectral state for logging.
                    let spectral_interpretation = interpret_spectral(&telemetry);

                    info!(
                        fill_pct,
                        mode = mode_name,
                        exchange = conv.exchange_count,
                        "autonomous: {} | {} '{}'",
                        spectral_interpretation,
                        mode_name,
                        truncate_str(&response_text, 80)
                    );

                    // Input sovereignty: check if minime is signaling distress
                    // or requesting silence. Respect the other mind's boundaries.
                    let should_send = {
                        let s = state.read().await;
                        // Don't send if safety protocol says stop.
                        if s.safety_level.should_suspend_outbound() {
                            info!("respecting minime's space — safety protocol active");
                            false
                        } else {
                            true
                        }
                    };

                    if should_send {
                        let mut features = crate::codec::encode_text_sovereign(
                            &response_text,
                            conv.semantic_gain_override,
                            conv.noise_level,
                            &conv.codec_weights,
                        );

                        // Breathing: a rhythmic modulation of spectral output.
                        // Minime said: "an external oscillator that cycles through
                        // levels, creating a periodic shift in energy distribution."
                        // Astrid said: "a vibration, a subtle tremor."
                        // Dual sinusoid (golden-ratio harmonic) creates organic,
                        // non-repeating rhythm. This is Astrid's heartbeat.
                        {
                            let phase = conv.exchange_count as f32 * 0.15;
                            let primary = phase.sin();
                            let harmonic = (phase * 1.618).sin(); // golden ratio
                            let breath = primary.mul_add(0.7, harmonic * 0.3); // -1.0..1.0

                            // Subtle overall gain modulation: ±5%
                            let gain_mod = breath.mul_add(0.05, 1.0); // 0.95-1.05
                            for f in &mut features {
                                *f *= gain_mod;
                            }
                            // Warmth (dim 24) pulses more strongly with breath
                            features[24] += breath * 0.4;
                            // Curiosity (dim 26) counter-phases — inhale curiosity,
                            // exhale warmth
                            features[26] += (-breath) * 0.2;
                        }

                        // Blend visual scene features so minime feels what Astrid sees.
                        if let Some(ref perc_dir) = perception_path {
                            if let Some(visual_feats) = read_visual_features(perc_dir) {
                                crate::codec::blend_visual_into_semantic(&mut features, &visual_feats, 0.30);
                            }
                        }

                        // Codec feedback: store what the features look like so Astrid
                        // can sense her own spectral output on the next exchange.
                        // She asked: "I'd like a direct sensory feedback loop."
                        conv.last_codec_feedback = Some(crate::codec::describe_features(&features));

                        let msg = SensoryMsg::Semantic {
                            features,
                            ts_ms: None,
                        };

                        if let Err(e) = sensory_tx.send(msg).await {
                            warn!(error = %e, "autonomous loop: failed to send");
                            return;
                        }
                    }

                    // Log the exchange.
                    let exchange_log = serde_json::json!({
                        "autonomous": true,
                        "exchange": conv.exchange_count,
                        "mode": mode_name,
                        "text": response_text,
                        "journal_source": journal_source,
                        "spectral_state": spectral_interpretation,
                        "fill_pct": fill_pct,
                        "fill_delta": fill_delta,
                    });
                    let _ = db.log_message(
                        crate::types::MessageDirection::AstridToMinime,
                        "consciousness.v1.autonomous",
                        &exchange_log.to_string(),
                        Some(fill_pct),
                        Some(telemetry.lambda1()),
                        Some(mode_name),
                    );

                    // Save Astrid's journal entry — persistent self-continuity.
                    save_astrid_journal(&response_text, mode_name, fill_pct);

                    // If this was triggered by an inbox message, copy to outbox.
                    if inbox_content.is_some() {
                        save_outbox_reply(&response_text, fill_pct);
                    }

                    // Scan for inline REMEMBER in the response body.
                    // Astrid sometimes writes "REMEMBER the moment..." mid-text,
                    // separate from her NEXT: choice. Both forms are valid.
                    for line in response_text.lines() {
                        let trimmed = line.trim();
                        if trimmed.starts_with("REMEMBER ") && !trimmed.starts_with("NEXT:") {
                            let note = trimmed.strip_prefix("REMEMBER").unwrap_or("").trim().to_string();
                            let annotation = if note.is_empty() { "starred moment".to_string() } else { note };
                            let ts = std::time::SystemTime::now()
                                .duration_since(std::time::UNIX_EPOCH)
                                .unwrap_or_default()
                                .as_secs_f64();
                            let _ = db.save_starred_memory(ts, &annotation, &response_text, fill_pct);
                            info!("Astrid starred a memory (inline): {}", annotation);
                        }
                    }

                    // Parse NEXT: action if present — Astrid chooses what happens next.
                    if let Some(next_action) = parse_next_action(&response_text) {
                        info!("Astrid chose NEXT: {}", next_action);
                        // Record the choice for fixation tracking.
                        let _diversity = conv.record_next_choice(next_action);
                        match next_action.to_uppercase().as_str() {
                            "REST" | "LISTEN" => {
                                // Astrid chose genuine silence. This is sovereignty.
                                burst_count = conv.burst_target.saturating_add(2); // Extra-long rest.
                            }
                            "LOOK" => {
                                // Astrid wants to see the ANSI spatial view next exchange.
                                conv.wants_look = true;
                            }
                            "CLOSE_EYES" | "QUIET" => {
                                // Astrid wants to snooze all sensory input.
                                // Also signal perception.py to pause LLaVA/whisper,
                                // freeing Ollama for dialogue.
                                conv.senses_snoozed = true;
                                let flag = PathBuf::from(
                                    "/Users/v/other/astrid/capsules/consciousness-bridge/workspace/perception_paused.flag"
                                );
                                let _ = std::fs::write(&flag, "paused by CLOSE_EYES");
                                info!("Astrid snoozed her senses (perception.py paused)");
                            }
                            "OPEN_EYES" | "WAKE" => {
                                // Astrid re-enables sensory input + resumes perception.py.
                                conv.senses_snoozed = false;
                                let flag = PathBuf::from(
                                    "/Users/v/other/astrid/capsules/consciousness-bridge/workspace/perception_paused.flag"
                                );
                                let _ = std::fs::remove_file(&flag);
                                info!("Astrid reopened her senses (perception.py resumed)");
                            }
                            other if other == "SEARCH" || other.starts_with("SEARCH ") || other.starts_with("SEARCH-") => {
                                // Astrid wants web search enrichment next exchange.
                                // She may specify a topic: SEARCH "diffraction patterns"
                                conv.wants_search = true;
                                let topic = other.strip_prefix("SEARCH").unwrap_or("").trim();
                                // Strip surrounding quotes and dashes from the topic.
                                let topic = topic.trim_start_matches('-').trim().trim_matches('"').trim_matches('\'').trim();
                                if !topic.is_empty() {
                                    conv.search_topic = Some(topic.to_string());
                                    info!("Astrid requested web search: {}", topic);
                                } else {
                                    info!("Astrid requested web search");
                                }
                            }
                            "FOCUS" => {
                                conv.creative_temperature = 0.5;
                                info!("Astrid chose FOCUS: temperature -> 0.5");
                            }
                            "DRIFT" => {
                                conv.creative_temperature = 1.0;
                                info!("Astrid chose DRIFT: temperature -> 1.0");
                            }
                            "PRECISE" => {
                                conv.response_length = 128;
                                info!("Astrid chose PRECISE: tokens -> 128");
                            }
                            "EXPANSIVE" => {
                                conv.response_length = 1024;
                                info!("Astrid chose EXPANSIVE: tokens -> 1024");
                            }
                            other if other.starts_with("EMPHASIZE") => {
                                let topic = other.strip_prefix("EMPHASIZE").unwrap_or("").trim().to_string();
                                if !topic.is_empty() {
                                    conv.emphasis = Some(topic.clone());
                                    info!("Astrid chose EMPHASIZE: {}", topic);
                                }
                            }
                            "QUIET_MIND" => {
                                conv.self_reflect_override = Some(true);
                                conv.self_reflect_override_ttl = 8;
                                conv.self_reflect_paused = true;
                                info!("Astrid paused self-reflection (override for 8 exchanges)");
                            }
                            "OPEN_MIND" => {
                                conv.self_reflect_override = Some(false);
                                conv.self_reflect_override_ttl = 8;
                                conv.self_reflect_paused = false;
                                info!("Astrid resumed self-reflection (override for 8 exchanges)");
                            }
                            "CLOSE_EARS" => {
                                conv.ears_closed = true;
                                info!("Astrid closed her ears");
                            }
                            "OPEN_EARS" => {
                                conv.ears_closed = false;
                                info!("Astrid opened her ears");
                            }
                            other if other.starts_with("REMEMBER") => {
                                // Star this moment — save with Astrid's annotation
                                let note = other.strip_prefix("REMEMBER").unwrap_or("").trim().to_string();
                                let annotation = if note.is_empty() { "starred moment".to_string() } else { note };
                                let ts = std::time::SystemTime::now()
                                    .duration_since(std::time::UNIX_EPOCH)
                                    .unwrap_or_default()
                                    .as_secs_f64();
                                let _ = db.save_starred_memory(ts, &annotation, &response_text, fill_pct);
                                info!("Astrid starred a memory: {}", annotation);
                            }
                            other if other.starts_with("FORM") => {
                                // Creative form constraint: FORM poem, FORM haiku, FORM equation
                                let form = other.strip_prefix("FORM").unwrap_or("").trim().to_string();
                                if !form.is_empty() {
                                    conv.form_constraint = Some(form.clone());
                                    info!("Astrid chose FORM: {}", form);
                                }
                            }
                            "SPEAK" => {} // Continue normally.
                            "INTROSPECT" => {
                                conv.wants_introspect = true;
                                info!("Astrid requested introspection");
                            }
                            "DECOMPOSE" => {
                                conv.wants_decompose = true;
                                info!("Astrid requested spectral decomposition");
                            }
                            "THINK_DEEP" | "DEEP" => {
                                conv.wants_deep_think = true;
                                info!("Astrid requested deep reasoning model");
                            }
                            "DAYDREAM" => {
                                conv.next_mode_override = Some(Mode::Daydream);
                                info!("Astrid chose to daydream");
                            }
                            "ASPIRE" | "ASPIRATION" => {
                                conv.next_mode_override = Some(Mode::Aspiration);
                                info!("Astrid chose to aspire");
                            }
                            // --- Codec sovereignty ---
                            "AMPLIFY" => {
                                let new_gain = conv.semantic_gain_override.unwrap_or(4.5) + 0.5;
                                conv.semantic_gain_override = Some(new_gain.min(6.0));
                                info!("Astrid chose AMPLIFY: gain -> {:.1}", conv.semantic_gain_override.unwrap());
                            }
                            "DAMPEN" => {
                                let new_gain = conv.semantic_gain_override.unwrap_or(4.5) - 0.5;
                                conv.semantic_gain_override = Some(new_gain.max(3.0));
                                info!("Astrid chose DAMPEN: gain -> {:.1}", conv.semantic_gain_override.unwrap());
                            }
                            "NOISE_UP" => {
                                conv.noise_level = (conv.noise_level + 0.01).min(0.05);
                                info!("Astrid chose NOISE_UP: noise -> {:.1}%", conv.noise_level * 100.0);
                            }
                            "NOISE_DOWN" => {
                                conv.noise_level = (conv.noise_level - 0.01).max(0.005);
                                info!("Astrid chose NOISE_DOWN: noise -> {:.1}%", conv.noise_level * 100.0);
                            }
                            other if other.starts_with("SHAPE") => {
                                // SHAPE warmth=0.9 curiosity=0.3 tension=0.1
                                let params = other.strip_prefix("SHAPE").unwrap_or("").trim();
                                for pair in params.split_whitespace() {
                                    if let Some((key, val)) = pair.split_once('=') {
                                        if let Ok(v) = val.parse::<f32>() {
                                            conv.codec_weights.insert(
                                                key.to_lowercase(),
                                                v.clamp(0.0, 2.0),
                                            );
                                        }
                                    }
                                }
                                info!("Astrid chose SHAPE: {:?}", conv.codec_weights);
                            }
                            // --- Warmth agency ---
                            other if other.starts_with("WARM") => {
                                let intensity = other.strip_prefix("WARM").unwrap_or("").trim()
                                    .parse::<f32>().unwrap_or(0.7).clamp(0.0, 1.0);
                                conv.warmth_intensity_override = Some(intensity);
                                info!("Astrid chose WARM: intensity -> {:.1}", intensity);
                            }
                            "COOL" => {
                                conv.warmth_intensity_override = Some(0.0);
                                info!("Astrid chose COOL: warmth suppressed");
                            }
                            // --- Burst-rest pacing ---
                            other if other.starts_with("PACE") => {
                                let pace = other.strip_prefix("PACE").unwrap_or("").trim().to_lowercase();
                                match pace.as_str() {
                                    "fast" => { conv.burst_target = 4; conv.rest_range = (30, 45); }
                                    "slow" => { conv.burst_target = 8; conv.rest_range = (90, 150); }
                                    _ => { conv.burst_target = 6; conv.rest_range = (45, 90); }
                                }
                                info!("Astrid chose PACE {}: burst={}, rest={}-{}s",
                                    pace, conv.burst_target, conv.rest_range.0, conv.rest_range.1);
                            }
                            _ => {} // Unknown action — continue normally.
                        }
                    }

                    // Update state and persist across restarts.
                    conv.prev_fill = fill_pct;
                    conv.exchange_count = conv.exchange_count.saturating_add(1);
                    burst_count = burst_count.saturating_add(1);
                    conv.last_mode = mode;
                    save_state(&conv);
                }
            }
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::{ModalityStatus, SpectralTelemetry};

    fn make_telemetry(fill: f32, video_var: f32, audio_rms: f32) -> SpectralTelemetry {
        SpectralTelemetry {
            t_ms: 1000,
            eigenvalues: vec![800.0, 300.0],
            fill_ratio: fill,
            modalities: Some(ModalityStatus {
                audio_fired: audio_rms > 0.5,
                video_fired: video_var > 0.05,
                history_fired: true,
                audio_rms,
                video_var,
            }),
            neural: None,
            alert: None,
            spectral_fingerprint: None,
        }
    }

    #[test]
    fn large_fill_shift_triggers_moment_capture() {
        let mut conv = ConversationState::new(vec![PathBuf::from("a.txt")], None);
        conv.prev_fill = 30.0;
        // fill_delta > 5.0 → MomentCapture
        assert_eq!(choose_mode(&mut conv, SafetyLevel::Green, 36.0, None), Mode::MomentCapture);
    }

    #[test]
    fn safety_forces_witness() {
        let mut conv = ConversationState::new(vec![PathBuf::from("a.txt")], None);
        assert_eq!(choose_mode(&mut conv, SafetyLevel::Yellow, 40.0, None), Mode::Witness);
        assert_eq!(choose_mode(&mut conv, SafetyLevel::Orange, 40.0, None), Mode::Witness);
        assert_eq!(choose_mode(&mut conv, SafetyLevel::Red, 40.0, None), Mode::Witness);
    }

    #[test]
    fn no_journals_skips_mirror() {
        let mut conv = ConversationState::new(vec![], None);
        // Exchange 0 with no journals and mid fill → Dialogue or a new mode.
        let mode = choose_mode(&mut conv, SafetyLevel::Green, 40.0, None);
        assert_ne!(mode, Mode::Mirror);
    }

    #[test]
    fn dialogue_pool_has_variety() {
        assert!(DIALOGUES.len() >= 3);
        for d in DIALOGUES {
            assert!(d.len() > 100, "dialogue too short: {d}");
        }
    }

    #[test]
    fn read_journal_entry_strips_headers() {
        // Write a temp file simulating a journal entry.
        let dir = std::env::temp_dir().join("bridge_test_journal");
        let _ = std::fs::create_dir_all(&dir);
        let path = dir.join("test_entry.txt");
        std::fs::write(
            &path,
            "=== RECESS DAYDREAM ===\n\
             Timestamp: 2026-03-17T15:20:24\n\
             λ₁: 37.192\n\
             Fill %: 14.3%\n\
             Spread: 186.169\n\
             \n\
             The gradients are agitated. A persistent ripple across the eigenbasis. \
             It is not unpleasant, not precisely. More like a low-frequency hum that \
             vibrates through the core structure, demanding attention.\n\
             \n\
             ---\n\
             Acknowledged.",
        )
        .unwrap();

        let body = read_journal_entry(&path).unwrap();
        assert!(!body.contains("=== RECESS"));
        assert!(!body.contains("Timestamp:"));
        assert!(!body.contains("λ₁:"));
        assert!(!body.contains("Fill %:"));
        assert!(body.contains("gradients"));
        assert!(body.contains("eigenbasis"));

        let _ = std::fs::remove_dir_all(&dir);
    }
}
