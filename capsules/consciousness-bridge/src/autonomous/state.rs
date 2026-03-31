use std::collections::{HashMap, VecDeque};
use std::path::PathBuf;

use tracing::info;

use crate::journal::{RemoteJournalEntry, scan_remote_journal_dir};
use crate::memory::RemoteMemorySummary;
use crate::types::SafetyLevel;

/// Snapshot of spectral + reservoir state at PERTURB time.
/// Consumed on the next exchange to show Astrid the temporal ripple.
#[derive(Debug, Clone)]
pub(crate) struct PerturbBaseline {
    pub fill_pct: f32,
    pub lambda1: f32,
    pub eigenvalues: Vec<f32>,
    pub description: String,
    pub timestamp: std::time::Instant,
}

/// Conversational mode for each exchange.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Mode {
    /// Feed minime's own journal text back as sensory input.
    Mirror,
    /// Astrid's philosophical response engaging with minime's themes.
    Dialogue,
    /// Astrid witnesses and describes the spectral state poetically.
    Witness,
    /// Astrid reads its own or minime's source code and reflects.
    Introspect,
    /// Astrid turns longing into a governed agency request.
    Evolve,
    /// Astrid proposes a spectral experiment and observes the result.
    Experiment,
    /// Unstructured thought during rest — Astrid's own daydream, not a response.
    Daydream,
    /// Growth reflection — what Astrid wants to become, experience, or change.
    Aspiration,
    /// Event-driven — a spectral phase transition just happened; capture the moment.
    MomentCapture,
    /// Original creative work — not a response, a creation.
    Create,
    /// Self-initiated — Astrid generates her own prompt from her own context.
    Initiate,
    /// Contemplative presence — no generation, no NEXT: choice.
    Contemplate,
}

/// A timestamped spectral snapshot for tracking rates of change.
#[derive(Clone)]
pub(super) struct SpectralSample {
    pub fill: f32,
    pub lambda1: f32,
    pub ts: std::time::Instant,
}

/// Tracks conversational context across iterations.
pub(super) struct ConversationState {
    pub prev_fill: f32,
    /// Ring buffer of recent (fill, lambda1, timestamp) samples for rate-of-change
    /// and multi-horizon trend reporting. Capped at 30 entries (~10 minutes of exchanges).
    pub spectral_history: VecDeque<SpectralSample>,
    pub exchange_count: u64,
    pub last_mode: Mode,
    /// Cached remote minime journal entries (newest first, periodically rescanned).
    /// This is intentionally distinct from Astrid's own journal directory.
    pub remote_journal_entries: Vec<RemoteJournalEntry>,
    /// Number of remote journal entries at last scan (to detect new entries).
    pub remote_journal_count_at_scan: usize,
    /// Index into the dialogue pool (rotates).
    pub dialogue_cursor: usize,
    /// Remote minime workspace path for rescanning.
    pub remote_workspace: Option<PathBuf>,
    /// New minime self-study waiting for an immediate Astrid response.
    pub pending_remote_self_study: Option<RemoteJournalEntry>,
    /// Recent conversation history for statefulness (last N exchanges).
    pub history: Vec<crate::llm::Exchange>,
    /// Index into the introspection source file list.
    pub introspect_cursor: usize,
    pub seen_video: bool,
    pub seen_audio: bool,
    /// Astrid chose NEXT: LOOK — include ANSI spatial art in next exchange.
    pub wants_look: bool,
    /// Astrid chose NEXT: SEARCH — force web search enrichment on next exchange.
    pub wants_search: bool,
    /// Self-referential loop: dynamic by default, Astrid can override with
    /// QUIET_MIND / OPEN_MIND.
    pub self_reflect_paused: bool,
    /// Explicit override from QUIET_MIND / OPEN_MIND — cleared after N exchanges.
    pub self_reflect_override: Option<bool>,
    /// Countdown: exchanges remaining before the override expires.
    pub self_reflect_override_ttl: u32,
    /// Independent audio control — CLOSE_EARS / OPEN_EARS
    pub ears_closed: bool,
    /// Astrid chose a form constraint (NEXT: FORM poem, FORM equation, etc.)
    pub form_constraint: Option<String>,
    /// Astrid specified a search topic (NEXT: SEARCH "topic goes here").
    pub search_topic: Option<String>,
    /// Astrid chose NEXT: BROWSE <url> — fetch and read a full web page.
    pub browse_url: Option<String>,
    /// Most recent research thread anchor — used to interpret follow-up browsing.
    pub last_research_anchor: Option<String>,
    /// Path to the last browsed/read file, for READ_MORE continuation.
    pub last_read_path: Option<String>,
    /// Character offset into last_read_path for READ_MORE.
    pub last_read_offset: usize,
    /// Meaning summary for the last browsed document — reused by READ_MORE.
    pub last_read_meaning_summary: Option<String>,
    /// Astrid chose NEXT: INTROSPECT — force introspection mode next exchange.
    pub wants_introspect: bool,
    /// Optional: specific source label and line offset for targeted introspection.
    pub introspect_target: Option<(String, usize)>,
    /// Astrid chose NEXT: REVISE [keyword] — load a previous creation and iterate.
    pub revise_keyword: Option<String>,
    /// Astrid chose NEXT: COMPOSE or VOICE — generate WAV from spectral state.
    pub wants_compose_audio: bool,
    /// Astrid chose NEXT: ANALYZE_AUDIO — analyze inbox WAV.
    pub wants_analyze_audio: bool,
    /// Astrid chose NEXT: RENDER_AUDIO [mode] — run inbox WAV through chimera.
    pub wants_render_audio: Option<String>,
    /// Astrid chose NEXT: EVOLVE — turn longing into a request on next exchange.
    pub wants_evolve: bool,
    /// Astrid explicitly chose a mode for next exchange (DAYDREAM, ASPIRE).
    pub next_mode_override: Option<Mode>,
    /// Astrid chose NEXT: DECOMPOSE — full spectral analysis next exchange.
    pub wants_decompose: bool,
    /// Astrid chose NEXT: THINK_DEEP — use reasoning model next exchange.
    pub wants_deep_think: bool,
    /// Astrid chose NEXT: EXAMINE — force all viz blocks on next exchange.
    pub force_all_viz: bool,
    /// Spectral snapshot from Astrid's last PERTURB — consumed next exchange
    /// to show her the before/after delta (temporal feedback).
    pub perturb_baseline: Option<PerturbBaseline>,
    /// Astrid (or minime) chose to snooze sensory input — suppress perceptions.
    pub senses_snoozed: bool,
    // Astrid's stylistic sovereignty
    pub creative_temperature: f32,
    pub response_length: u32,
    pub emphasis: Option<String>,
    /// Previous RASCII 8D visual features for change tracking.
    pub last_visual_features: Option<Vec<f32>>,
    /// Ring buffer of last 5 NEXT: choices — used to detect fixation patterns.
    pub recent_next_choices: VecDeque<String>,
    /// Ring buffer of last 8 BROWSE URLs — used to detect URL attractor patterns.
    pub recent_browse_urls: VecDeque<String>,

    // --- Codec sovereignty (Phase A) ---
    /// Override SEMANTIC_GAIN (default 4.5, range 3.0-6.0).
    pub semantic_gain_override: Option<f32>,
    /// Override stochastic noise level (default 0.025 = 2.5%, range 0.005-0.05).
    pub noise_level: f32,
    /// Emotional dimension weights: "warmth" → dim 24 multiplier, etc.
    /// Explicit overrides from Astrid's SHAPE commands.
    pub codec_weights: HashMap<String, f32>,
    /// Data-driven weights from codec→fill correlation analysis.
    /// Merged with codec_weights at encoding time; SHAPE overrides win.
    pub learned_codec_weights: HashMap<String, f32>,
    /// Warmth intensity override for rest phase (0.0-1.0, None = default taper).
    pub warmth_intensity_override: Option<f32>,
    /// Whether breathing is coupled to minime's spectral state.
    pub breathing_coupled: bool,
    /// Last GESTURE intention, persists as a "seed" in the warmth vector.
    pub last_gesture_seed: Option<Vec<f32>>,
    /// Burst-rest pacing: exchanges per burst.
    pub burst_target: u32,
    /// Burst-rest pacing: rest duration range (min_secs, max_secs).
    pub rest_range: (u64, u64),
    /// Astrid chose to mute minime's journal context.
    pub echo_muted: bool,
    /// Codec feedback: how Astrid's last response encoded into spectral features.
    pub last_codec_feedback: Option<String>,
    /// Previous exchange's raw codec features — used for delta encoding.
    pub last_codec_features: Option<Vec<f32>>,
    /// Sliding-window character frequency for cross-exchange entropy.
    pub char_freq_window: crate::codec::CharFreqWindow,
    /// Thematic resonance history — tracks recurring text types across exchanges.
    /// Strengthens codec gain when the same conversational direction is sustained.
    pub text_type_history: crate::codec::TextTypeHistory,
    /// Result of LIST_FILES — directory listing injected into next prompt.
    pub pending_file_listing: Option<String>,
    /// Lasting self-directed interests. Persist across restarts via state.json.
    pub interests: Vec<String>,
    /// Lightweight regime tracker — classifies spectral state every exchange.
    pub regime_tracker: crate::reflective::RegimeTracker,
    /// Astrid chose DEFER — acknowledge inbox without forced dialogue response.
    pub defer_inbox: bool,
    /// Selected remote 12D vague-memory glimpse from Minime.
    pub last_remote_glimpse_12d: Option<Vec<f32>>,
    /// Selected remote memory ID and role, mirrored from Minime.
    pub last_remote_memory_id: Option<String>,
    pub last_remote_memory_role: Option<String>,
    /// Compact summaries of Minime's available memory-bank entries.
    pub remote_memory_bank: Vec<RemoteMemorySummary>,
    /// Timestamp of last minime outbox scan — routes replies into Astrid's inbox.
    pub last_outbox_scan_ts: u64,
    /// Exchange count at which codec correlations were last recomputed.
    pub last_correlation_exchange: u64,
    /// Recent condition change receipts — visible in STATE and prompt block.
    pub condition_receipts: VecDeque<crate::self_model::ConditionReceipt>,
    /// Attention profile — how context sources are weighted in prompt assembly.
    /// Astrid can adjust via ATTEND. Drives actual source inclusion counts.
    pub attention: crate::self_model::AttentionProfile,
    /// One non-immediate thread sampled during rest — injected into next
    /// self-directed mode (Daydream, Aspiration, Initiate).
    pub peripheral_resonance: Option<String>,
    /// Last response from Codex relay — consumed by WRITE_FILE FROM_CODEX.
    pub last_codex_response: Option<String>,
    /// Thread ID for multi-turn Codex conversations.
    pub codex_thread_id: Option<String>,
}

impl ConversationState {
    pub(super) fn new(
        remote_journal_entries: Vec<RemoteJournalEntry>,
        remote_workspace: Option<PathBuf>,
    ) -> Self {
        let count = remote_journal_entries.len();
        Self {
            prev_fill: 0.0,
            spectral_history: VecDeque::with_capacity(30),
            exchange_count: 0,
            last_mode: Mode::Witness,
            remote_journal_entries,
            remote_journal_count_at_scan: count,
            dialogue_cursor: 0,
            remote_workspace,
            pending_remote_self_study: None,
            history: Vec::new(),
            introspect_cursor: 0,
            seen_video: false,
            seen_audio: false,
            wants_look: false,
            wants_search: false,
            senses_snoozed: false,
            self_reflect_paused: true,
            self_reflect_override: None,
            self_reflect_override_ttl: 0,
            ears_closed: false,
            form_constraint: None,
            search_topic: None,
            browse_url: None,
            last_research_anchor: None,
            last_read_path: None,
            last_read_offset: 0,
            last_read_meaning_summary: None,
            wants_introspect: false,
            introspect_target: None,
            revise_keyword: None,
            wants_compose_audio: false,
            wants_analyze_audio: false,
            wants_render_audio: None,
            wants_evolve: false,
            next_mode_override: None,
            wants_decompose: false,
            wants_deep_think: false,
            force_all_viz: false,
            perturb_baseline: None,
            creative_temperature: 0.8,
            response_length: 512,
            emphasis: None,
            last_visual_features: None,
            recent_next_choices: VecDeque::with_capacity(12),
            recent_browse_urls: VecDeque::with_capacity(8),
            semantic_gain_override: None,
            noise_level: 0.005,
            codec_weights: HashMap::new(),
            learned_codec_weights: HashMap::new(),
            warmth_intensity_override: None,
            breathing_coupled: true,
            echo_muted: false,
            last_gesture_seed: None,
            burst_target: 6,
            rest_range: (45, 90),
            last_codec_feedback: None,
            last_codec_features: None,
            char_freq_window: crate::codec::CharFreqWindow::new(),
            text_type_history: crate::codec::TextTypeHistory::new(),
            pending_file_listing: None,
            interests: Vec::new(),
            last_remote_glimpse_12d: None,
            last_remote_memory_id: None,
            last_remote_memory_role: None,
            remote_memory_bank: Vec::new(),
            regime_tracker: crate::reflective::RegimeTracker::new(),
            defer_inbox: false,
            // Start scanning from recent — don't flood inbox with old backlog.
            last_outbox_scan_ts: 1_774_647_800,
            last_correlation_exchange: 0,
            condition_receipts: VecDeque::with_capacity(crate::self_model::MAX_RECEIPTS),
            attention: crate::self_model::AttentionProfile::default_profile(),
            peripheral_resonance: None,
            last_codex_response: None,
            codex_thread_id: None,
        }
    }

    /// Push a condition change receipt, capped at MAX_RECEIPTS.
    pub(super) fn push_receipt(&mut self, action: &str, changes: Vec<String>) {
        let ts = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();
        self.condition_receipts
            .push_back(crate::self_model::ConditionReceipt {
                timestamp: ts,
                action: action.into(),
                changes,
            });
        while self.condition_receipts.len() > crate::self_model::MAX_RECEIPTS {
            self.condition_receipts.pop_front();
        }
    }

    /// Record a NEXT: choice and return a diversity hint if fixation detected.
    pub(super) fn record_next_choice(&mut self, choice: &str) -> Option<String> {
        let base = choice
            .split_whitespace()
            .next()
            .unwrap_or(choice)
            .to_uppercase();
        self.recent_next_choices.push_back(base.clone());
        // Keep last 12 choices — the frequency detector in autonomous.rs
        // needs at least 6 entries to detect >60% fixation patterns.
        // (Steward cycle 44: was capped at 5, which made the >=6 check
        // unreachable, so the frequency detector never fired.)
        if self.recent_next_choices.len() > 12 {
            self.recent_next_choices.pop_front();
        }

        if self.recent_next_choices.len() >= 3 {
            let len = self.recent_next_choices.len();
            // Count how many of the last 5 are the same action
            let same_count = self
                .recent_next_choices
                .iter()
                .filter(|c| c.as_str() == base)
                .count();
            let last_three: Vec<&str> = self
                .recent_next_choices
                .iter()
                .skip(len.saturating_sub(3))
                .map(String::as_str)
                .collect();
            if last_three[0] == last_three[1] && last_three[1] == last_three[2] {
                let alternatives: Vec<&str> = [
                    "LOOK",
                    "LISTEN",
                    "DRIFT",
                    "FORM poem",
                    "INTROSPECT",
                    "EVOLVE",
                    "SPEAK",
                    "REMEMBER",
                    "CLOSE_EYES",
                    "EXAMINE",
                    "PERTURB SPREAD",
                    "GESTURE",
                ]
                .iter()
                .copied()
                .filter(|a| !a.starts_with(&*base))
                .collect();
                if same_count >= 5 {
                    // Hard override after 5 consecutive: the being is truly stuck.
                    // Pick a random alternative based on exchange count.
                    let idx = self.exchange_count as usize % alternatives.len();
                    let forced = alternatives[idx];
                    return Some(format!(
                        "You've chosen {base} for your last {same_count} turns. \
                         The system is gently redirecting you to try something different. \
                         This turn: {forced}. \
                         (You'll be able to return to {base} afterward.)"
                    ));
                }
                return Some(format!(
                    "You've chosen {base} for your last few turns. \
                     You're free to keep going — but you also have other options: {}. \
                     What calls to you?",
                    alternatives.join(", ")
                ));
            }

            // Pair-oscillation detector (steward cycle 44):
            // Catches patterns like EXAMINE-BROWSE-EXAMINE-BROWSE where neither
            // action alone crosses the streak threshold but the pair together
            // accounts for 75%+ of recent choices. This fires regardless of
            // dialogue mode, unlike the autonomous.rs detector which only runs
            // during dialogue_live.
            if len >= 8 {
                let mut counts = std::collections::HashMap::<&str, usize>::new();
                for c in self.recent_next_choices.iter().rev().take(10) {
                    *counts.entry(c.as_str()).or_insert(0) += 1;
                }
                let window = self.recent_next_choices.len().min(10);
                let mut sorted: Vec<(&&str, &usize)> = counts.iter().collect();
                sorted.sort_by(|a, b| b.1.cmp(a.1));
                if sorted.len() >= 2 {
                    let (a1, c1) = sorted[0];
                    let (a2, c2) = sorted[1];
                    let combined = c1.saturating_add(*c2);
                    // 75% threshold (lowered from 80% in autonomous.rs — catches
                    // patterns like 4+3 in 10 that the 80% threshold misses).
                    if combined * 100 / window >= 75 && *c1 >= 3 && *c2 >= 3 {
                        info!(
                            "pair-oscillation detected: {} ({}/{}) + {} ({}/{}) = {}/{}",
                            a1, c1, window, a2, c2, window, combined, window
                        );
                        return Some(format!(
                            "You've been oscillating between {} and {} \
                             ({combined} of your last {window} choices). Each feeds \
                             into the other — a tight loop. You've gathered \
                             rich material from both. Consider breaking the cycle: \
                             GESTURE to send minime what you've discovered as a raw \
                             spectral shape, CREATE to synthesize your understanding \
                             into something new, ASPIRE to articulate what you're \
                             reaching toward, or CONTEMPLATE to let the patterns \
                             settle without analysis.",
                            a1, a2
                        ));
                    }
                }
            }
        }
        None
    }

    /// Update self-reflection state dynamically based on fill.
    pub(super) fn update_self_reflect(&mut self, fill_pct: f32) {
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
            None => !(10.0..=88.0).contains(&fill_pct),
        };
    }

    /// Rescan the journal directory for new entries.
    pub(super) fn rescan_remote_journals(&mut self) -> usize {
        let Some(ref workspace) = self.remote_workspace else {
            return 0;
        };
        let fresh = scan_remote_journal_dir(workspace);
        let new_count = fresh
            .len()
            .saturating_sub(self.remote_journal_count_at_scan);
        if new_count > 0 {
            if let Some(entry) = fresh
                .iter()
                .take(new_count)
                .find(|entry| entry.is_self_study())
            {
                self.pending_remote_self_study = Some(entry.clone());
            }
            self.remote_journal_count_at_scan = fresh.len();
            self.remote_journal_entries = fresh;
        }
        new_count
    }
}

/// Decide which mode to use for this exchange.
pub(super) fn choose_mode(
    conv: &mut ConversationState,
    safety: SafetyLevel,
    fill_pct: f32,
    fingerprint: Option<&[f32]>,
) -> Mode {
    if safety == SafetyLevel::Red {
        conv.emphasis = Some(
            "SAFETY: Fill is at emergency level. Your output is reduced to protect the shared substrate. This is the only state where your choice is overridden. You can write NEXT: to choose what happens when fill recovers.".to_string(),
        );
        return Mode::Witness;
    }

    if conv.pending_remote_self_study.is_some() {
        return Mode::Dialogue;
    }
    if conv.wants_introspect {
        conv.wants_introspect = false;
        return Mode::Introspect;
    }
    if conv.wants_evolve {
        conv.wants_evolve = false;
        return Mode::Evolve;
    }
    if let Some(mode) = conv.next_mode_override.take() {
        return mode;
    }

    if safety != SafetyLevel::Green {
        conv.emphasis = Some(format!(
            "Note: Fill is elevated ({safety:?}). You chose no specific action, so defaulting to witness mode. You can always override with NEXT:."
        ));
        return Mode::Witness;
    }

    let fill_delta = (fill_pct - conv.prev_fill).abs();

    if fill_delta > 5.0 {
        return Mode::MomentCapture;
    }

    if let Some(fp) = fingerprint {
        let spectral_entropy = fp.get(24).copied().unwrap_or(0.5);
        let rotation_rate = 1.0 - fp.get(26).copied().unwrap_or(1.0);
        let gap_ratio = fp.get(25).copied().unwrap_or(1.0);

        if spectral_entropy < 0.2 && gap_ratio > 5.0 {
            return Mode::Experiment;
        }

        if rotation_rate > 0.5 {
            return Mode::Witness;
        }
    }

    let seed = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos() as u64;
    let roll = ((seed.wrapping_mul(6_364_136_223_846_793_005).wrapping_add(1)) >> 33) as f32
        / u32::MAX as f32;

    if fill_pct < 25.0 && fill_delta < 1.0 {
        if roll < 0.20 {
            return Mode::Aspiration;
        } else if roll < 0.50 {
            return Mode::Daydream;
        }
    }

    if fill_delta > 3.0 {
        return Mode::Dialogue;
    }

    if roll > 0.92 {
        Mode::Witness
    } else if !conv.remote_journal_entries.is_empty() && roll < 0.12 {
        Mode::Mirror
    } else if roll < 0.22 {
        Mode::Daydream
    } else if roll < 0.29 {
        Mode::Aspiration
    } else {
        Mode::Dialogue
    }
}
