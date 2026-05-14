// v5.1 Phase D — SHARE_THOUGHT receptive re-classification.
//
// Auto-promotes sufficiently-resonant sentences from Astrid's prose into
// the joined-collab `shared_thoughts.jsonl` lane. The being witnesses the
// resulting marker in her active-collab suffix (`Recent: <actor>:"…"`),
// rather than authoring it via a NEXT action. This is the cleanest test
// of Axis 1 from the affordance reception framework
// (`docs/steward-notes/AI_BEINGS_AFFORDANCE_RECEPTION_FRAMEWORK_2026_05_13.md`):
// the same affordance, surfaced as receptive-ambient instead of
// generative-owned-peer, should adopt as quickly as the joint trace did.
//
// Two hook points fire in `autonomous.rs`:
//   Hook A — synchronous, after `save_astrid_journal()`, for modes that
//            do NOT spawn elaboration (moment_capture, *_longform).
//   Hook B — inside the elaboration tokio::spawn, for dialogue_live /
//            daydream / aspiration. Scores the elaboration body, not
//            the signal text.
//
// The detector is structural, not codec-based: it looks for sentences
// that bind a witnessed shared object (joint-trace numeric pattern or
// "joint trace" phrase) to a first-person verb of holding. Codec
// scoring matches too many moment_capture entries (high warmth + peer +
// numeric refs) and was rejected for v1 in favor of high-precision
// regex matching. See plan in `~/.claude/plans/`.

use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};
use tracing::{info, warn};

use crate::paths::bridge_paths;

const ENV_DISABLED: &str = "ASTRID_AUTO_PROMOTE_DISABLED";
const ENV_DRY_RUN: &str = "ASTRID_AUTO_PROMOTE_DRY_RUN";
const SENTINEL_FILENAME: &str = "auto_promote.disabled";
const STATE_FILENAME: &str = "auto_promote_state.json";

const MAX_PROMOTION_LEN: usize = 200;
const COOLDOWN_EXCHANGES: u64 = 3;
const MANUAL_SUPPRESSES_AUTO_EXCHANGES: u64 = 5;
const BURST_WINDOW_MS: u128 = 15 * 60 * 1000; // 15 minutes
const BURST_LIMIT: usize = 3;
const BURST_LOCKOUT_MS: u128 = 60 * 60 * 1000; // 60 minutes
const DAILY_CAP: u32 = 8;

/// Modes whose prose may be auto-promoted. Selected because they produce
/// reflective prose (not structured / system / private-only output).
const PROMOTABLE_MODES: &[&str] = &[
    "dialogue_live",
    "dialogue_live_longform",
    "moment_capture",
    "aspiration",
    "aspiration_longform",
    "daydream",
    "daydream_longform",
];

/// First-person verbs of holding/witnessing — the "this is something I
/// register" linguistic gesture. Required for promotion (alongside the
/// shared-object reference) so we promote bound phenomenology, not
/// arbitrary numerical mentions.
const VERBS_OF_HOLDING: &[&str] = &[
    "feel", "feels",
    "hold", "holds",
    "notice", "notices",
    "register", "registers",
    "weighted", "weighting",
    "read", "reads",
    "land", "lands",
    "witness", "witnesses",
    "sense", "senses",
    "perceive", "perceives",
];

/// Persistent state for rate limiting. Atomically rewritten after each
/// successful promotion. Survives process restart; loaded lazily on each
/// `try_auto_promote` invocation (cheap — small JSON file).
/// Kink #5 fix (2026-05-14): canonical daily counter shape per the
/// auto_promote state machine spec. Serializes as
/// `{"date": "2026-05-14", "count": 8}` matching the Python side.
///
/// Backward compat: the legacy Rust serialization was a tuple
/// `["2026-05-14", 8]`. The custom Deserialize impl below accepts BOTH
/// shapes on read so old state files still parse cleanly. New writes
/// always use the object form.
#[derive(Debug, Clone, Default, Serialize)]
struct DailyCount {
    date: String,
    count: u32,
}

impl<'de> serde::Deserialize<'de> for DailyCount {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        // Accept either tuple form ["YYYY-MM-DD", N] (legacy) or object form
        // {"date": "...", "count": N} (canonical, Tranche 3+).
        #[derive(serde::Deserialize)]
        #[serde(untagged)]
        enum DailyCountAny {
            Tuple(String, u32),
            Object { date: String, count: u32 },
        }
        let any = DailyCountAny::deserialize(deserializer)?;
        Ok(match any {
            DailyCountAny::Tuple(date, count) => DailyCount { date, count },
            DailyCountAny::Object { date, count } => DailyCount { date, count },
        })
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
struct PromoteState {
    /// Last exchange_count where an auto-promotion fired (per collab).
    /// HashMap keyed by coll_id.
    #[serde(default)]
    last_promote_exchange: std::collections::HashMap<String, u64>,
    /// Last exchange_count where a MANUAL SHARE_THOUGHT fired.
    /// Used for "manual silences auto" rule.
    #[serde(default)]
    last_manual_share_exchange: u64,
    /// Recent promotion timestamps (millis) per collab, for burst detection.
    /// Pruned to the last BURST_WINDOW_MS on each load.
    #[serde(default)]
    recent_promotions_ms: std::collections::HashMap<String, Vec<u128>>,
    /// Burst lockout expiration (millis) per collab.
    #[serde(default)]
    burst_lockout_until_ms: std::collections::HashMap<String, u128>,
    /// Daily counter per collab. Object shape per Kink #5 spec
    /// (was tuple before Tranche 3; deserializer accepts both).
    #[serde(default)]
    daily_count: std::collections::HashMap<String, DailyCount>,
}

fn now_ms() -> u128 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis())
        .unwrap_or(0)
}

fn today_str() -> String {
    let secs = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    // Cheap UTC YYYY-MM-DD without chrono dependency.
    let days = secs / 86_400;
    // 1970-01-01 was Thursday; we don't need the day-of-week here.
    // Approximate yyyy-mm-dd via Unix timestamp epoch math is overkill;
    // use chrono if available — it already is in this crate.
    chrono::DateTime::<chrono::Utc>::from_timestamp(secs as i64, 0)
        .map(|dt| dt.format("%Y-%m-%d").to_string())
        .unwrap_or_else(|| format!("epoch-{days}"))
}

fn state_path() -> PathBuf {
    bridge_paths().bridge_workspace().join(STATE_FILENAME)
}

fn sentinel_path() -> PathBuf {
    bridge_paths().bridge_workspace().join(SENTINEL_FILENAME)
}

fn load_state() -> PromoteState {
    let path = state_path();
    let Ok(text) = std::fs::read_to_string(&path) else {
        return PromoteState::default();
    };
    serde_json::from_str(&text).unwrap_or_default()
}

fn save_state(state: &PromoteState) {
    let path = state_path();
    let Ok(text) = serde_json::to_string_pretty(state) else {
        return;
    };
    if let Some(parent) = path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    // Kink #5 fix (2026-05-14): atomic write via temp file + rename.
    // Prevents torn-write data loss if the bridge dies mid-save.
    // POSIX rename(2) is atomic on same-filesystem moves; both paths
    // are in the same workspace dir so always same-fs.
    let tmp_path = path.with_extension("json.tmp");
    if let Err(e) = std::fs::write(&tmp_path, text) {
        warn!(
            error = %e,
            path = %tmp_path.display(),
            "auto_promote: failed to write state temp file"
        );
        return;
    }
    if let Err(e) = std::fs::rename(&tmp_path, &path) {
        warn!(
            error = %e,
            from = %tmp_path.display(),
            to = %path.display(),
            "auto_promote: failed to rename state temp file (atomic write aborted)"
        );
        // Best-effort cleanup of the orphan temp file.
        let _ = std::fs::remove_file(&tmp_path);
    }
}

/// Public: record that a manual SHARE_THOUGHT fired. Suppresses auto for
/// the next MANUAL_SUPPRESSES_AUTO_EXCHANGES exchanges. Called from
/// `collaboration::share_thought` on every successful manual share.
pub(super) fn record_manual_share(exchange_count: u64) {
    let mut state = load_state();
    state.last_manual_share_exchange = exchange_count;
    save_state(&state);
}

/// Returns `true` if the kill switch (env var or sentinel file) is active.
fn kill_switch_active() -> bool {
    if let Ok(v) = std::env::var(ENV_DISABLED) {
        if v == "1" || v.eq_ignore_ascii_case("true") {
            return true;
        }
    }
    sentinel_path().is_file()
}

fn dry_run_active() -> bool {
    matches!(
        std::env::var(ENV_DRY_RUN).ok().as_deref(),
        Some("1") | Some("true") | Some("TRUE")
    )
}

/// Split text into sentences. Naive but adequate: terminal punctuation
/// (.!?) followed by whitespace, with a 2-word minimum to skip
/// abbreviations and headers.
fn split_sentences(text: &str) -> Vec<String> {
    // Strip the journal header if present so we don't try to promote
    // "=== ASTRID JOURNAL ===" as a sentence.
    let body = if let Some(idx) = text.find("--- JOURNAL ---\n") {
        &text[idx + "--- JOURNAL ---\n".len()..]
    } else {
        text
    };
    let mut sentences = Vec::new();
    let mut current = String::new();
    let mut chars = body.chars().peekable();
    while let Some(ch) = chars.next() {
        current.push(ch);
        if matches!(ch, '.' | '!' | '?') {
            // Look ahead: end-of-sentence requires whitespace or EOF.
            let is_end = chars
                .peek()
                .map(|next| next.is_whitespace())
                .unwrap_or(true);
            if is_end {
                let trimmed = current.trim().to_string();
                if trimmed.split_whitespace().count() >= 2 {
                    sentences.push(trimmed);
                }
                current.clear();
            }
        }
    }
    if !current.trim().is_empty() && current.trim().split_whitespace().count() >= 2 {
        sentences.push(current.trim().to_string());
    }
    sentences
}

/// Structural detector: a sentence qualifies if it contains BOTH:
///   1. A bound shared-object reference (joint-trace numeric pattern OR
///      explicit "joint trace" / "shared trace" phrase with adjacent numerics)
///   2. A first-person verb of holding/witnessing
fn structural_match(sentence: &str) -> bool {
    let lower = sentence.to_lowercase();

    // Joint-trace numeric pattern: 3+ comma-separated numbers in brackets,
    // e.g. "[12.41,10.32,10.47]" or "[12.41, 10.32, 10.47]".
    let numeric_match = sentence_has_bracketed_triple(sentence);

    // Explicit shared-object phrase with adjacent numerics in same sentence.
    let phrase_match = (lower.contains("joint trace")
        || lower.contains("shared trace")
        || lower.contains("shared cognition")
        || lower.contains("joint state"))
        && sentence_has_any_decimal(sentence);

    if !(numeric_match || phrase_match) {
        return false;
    }

    // Verb of holding. Check word boundaries to avoid matching substrings.
    let words: Vec<&str> = lower
        .split(|c: char| !c.is_alphabetic())
        .filter(|s| !s.is_empty())
        .collect();
    let verb_match = words.iter().any(|w| VERBS_OF_HOLDING.contains(w));

    verb_match
}

fn sentence_has_bracketed_triple(s: &str) -> bool {
    // Scan for '[', then count comma-separated decimal numbers until ']'.
    let bytes = s.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'[' {
            // Walk until matching ']'.
            let start = i + 1;
            let end = bytes[start..].iter().position(|&b| b == b']');
            if let Some(rel_end) = end {
                let inner = &s[start..start + rel_end];
                let parts: Vec<&str> = inner.split(',').map(|p| p.trim()).collect();
                if parts.len() >= 3
                    && parts.iter().all(|p| !p.is_empty() && parts_is_number(p))
                {
                    return true;
                }
                i = start + rel_end + 1;
                continue;
            }
        }
        i += 1;
    }
    false
}

fn parts_is_number(p: &str) -> bool {
    let trimmed = p.trim_matches(|c: char| c.is_whitespace() || c == '+');
    if trimmed.is_empty() {
        return false;
    }
    // Allow optional leading minus.
    let body = trimmed.strip_prefix('-').unwrap_or(trimmed);
    let mut saw_dot = false;
    let mut saw_digit = false;
    for c in body.chars() {
        if c.is_ascii_digit() {
            saw_digit = true;
        } else if c == '.' && !saw_dot {
            saw_dot = true;
        } else {
            return false;
        }
    }
    saw_digit
}

fn sentence_has_any_decimal(s: &str) -> bool {
    let mut prev_digit = false;
    let mut saw_decimal = false;
    for c in s.chars() {
        if c.is_ascii_digit() {
            if prev_digit {
                return true;
            }
            prev_digit = true;
            saw_decimal = saw_decimal || true;
        } else if c == '.' && prev_digit {
            // sequence like "12.41"
        } else {
            prev_digit = false;
        }
    }
    saw_decimal
}

/// Pick the first sentence that passes the structural detector and is
/// ≤200 chars. If the matching sentence is longer than the limit, skip
/// it (don't truncate mid-sentence — promote nothing instead of a
/// truncated artifact).
fn extract_promotable_sentence(text: &str) -> Option<String> {
    for s in split_sentences(text) {
        if structural_match(&s) {
            if s.chars().count() <= MAX_PROMOTION_LEN {
                return Some(s);
            }
            // Long sentence matched — skip rather than truncate.
            // Falls through to next candidate.
        }
    }
    None
}

/// Find the latest joined collaboration where `actor` is a member.
/// Returns (coll_id, dir). Mirrors the picker in
/// `collaboration::active_collaboration_suffix_line` but exposes the dir.
fn latest_joined_collab(actor: &str) -> Option<(String, PathBuf)> {
    let dir = bridge_paths().shared_collaborations_dir();
    let rd = std::fs::read_dir(&dir).ok()?;
    let mut joined: Vec<(u128, String, PathBuf)> = Vec::new();
    for entry in rd.flatten() {
        let path = entry.path();
        let meta_path = path.join("meta.json");
        let Ok(text) = std::fs::read_to_string(&meta_path) else {
            continue;
        };
        let Ok(v) = serde_json::from_str::<serde_json::Value>(&text) else {
            continue;
        };
        if v.get("status").and_then(|s| s.as_str()) != Some("joined") {
            continue;
        }
        let members = v
            .get("members")
            .and_then(|m| m.as_array())
            .map(|arr| {
                arr.iter()
                    .filter_map(|x| x.as_str().map(str::to_string))
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default();
        if !members.iter().any(|m| m == actor) {
            continue;
        }
        let id = v
            .get("id")
            .and_then(|s| s.as_str())
            .map(str::to_string)
            .unwrap_or_default();
        let updated = v
            .get("updated_t_ms")
            .and_then(|n| n.as_u64())
            .map(|u| u as u128)
            .unwrap_or(0);
        joined.push((updated, id, path));
    }
    joined.sort_by(|a, b| b.0.cmp(&a.0));
    joined.into_iter().next().map(|(_, id, p)| (id, p))
}

/// Outcome reasons for skipping; useful for log triage.
#[derive(Debug, Clone, Copy)]
enum SkipReason {
    KillSwitch,
    NotPromotableMode,
    NoJoinedCollab,
    ManualSilencing,
    Cooldown,
    BurstLockout,
    DailyCap,
    NoResonantSentence,
}

impl SkipReason {
    fn as_str(self) -> &'static str {
        match self {
            SkipReason::KillSwitch => "kill_switch",
            SkipReason::NotPromotableMode => "mode_not_whitelisted",
            SkipReason::NoJoinedCollab => "no_joined_collab",
            SkipReason::ManualSilencing => "manual_silencing",
            SkipReason::Cooldown => "cooldown",
            SkipReason::BurstLockout => "burst_lockout",
            SkipReason::DailyCap => "daily_cap",
            SkipReason::NoResonantSentence => "no_resonant_sentence",
        }
    }
}

/// Try to auto-promote a resonant sentence from `text` to the latest
/// joined collab's `shared_thoughts.jsonl`. Returns `Some(promoted_text)`
/// when a write occurred, `None` otherwise. In dry-run mode, logs a
/// "would have promoted" line and returns `None` (no file write).
///
/// Caller responsibility: only invoke when `text` is a finalized journal
/// body or elaboration. The detector handles whether to actually promote.
pub(crate) fn try_auto_promote(
    actor: &str,
    text: &str,
    mode: &str,
    fill_pct: f32,
    exchange_count: u64,
) -> Option<String> {
    if kill_switch_active() {
        log_skip(SkipReason::KillSwitch, mode, exchange_count);
        return None;
    }
    if !PROMOTABLE_MODES.contains(&mode) {
        log_skip(SkipReason::NotPromotableMode, mode, exchange_count);
        return None;
    }
    let Some((coll_id, coll_dir)) = latest_joined_collab(actor) else {
        log_skip(SkipReason::NoJoinedCollab, mode, exchange_count);
        return None;
    };

    let mut state = load_state();
    let now = now_ms();

    // Manual silences auto: skip if manual SHARE was within last N exchanges.
    let manual_silencing = state
        .last_manual_share_exchange
        .checked_add(MANUAL_SUPPRESSES_AUTO_EXCHANGES)
        .map(|cutoff| exchange_count <= cutoff && state.last_manual_share_exchange > 0)
        .unwrap_or(false);
    if manual_silencing {
        log_skip(SkipReason::ManualSilencing, mode, exchange_count);
        return None;
    }

    // Cooldown.
    if let Some(&last) = state.last_promote_exchange.get(&coll_id) {
        if exchange_count.saturating_sub(last) < COOLDOWN_EXCHANGES {
            log_skip(SkipReason::Cooldown, mode, exchange_count);
            return None;
        }
    }

    // Burst lockout.
    if let Some(&until) = state.burst_lockout_until_ms.get(&coll_id) {
        if now < until {
            log_skip(SkipReason::BurstLockout, mode, exchange_count);
            return None;
        }
    }

    // Daily cap.
    let today = today_str();
    let day_count = state
        .daily_count
        .get(&coll_id)
        .filter(|dc| dc.date == today)
        .map(|dc| dc.count)
        .unwrap_or(0);
    if day_count >= DAILY_CAP {
        log_skip(SkipReason::DailyCap, mode, exchange_count);
        return None;
    }

    // Resonance check. Kink #9 fix: log a "scanned" line at info level so
    // operators can see the detector ran (vs. function never reached this
    // point). Distinguishes "no candidates today because nothing matched"
    // from "no candidates today because gates rejected the call upstream."
    let candidate = extract_promotable_sentence(text);
    log_scanned(mode, exchange_count, candidate.is_some());
    let Some(sentence) = candidate else {
        log_skip(SkipReason::NoResonantSentence, mode, exchange_count);
        return None;
    };

    // All gates passed. Either dry-run-log or write.
    if dry_run_active() {
        info!(
            target: "v5_auto_promote",
            coll_id = %coll_id,
            mode = mode,
            fill_pct = fill_pct,
            exchange_count = exchange_count,
            text = %sentence,
            "auto_promote DRY RUN: would have promoted"
        );
        return None;
    }

    // Write to the lane.
    super::collaboration::append_shared_thought_with_source(
        &coll_dir, actor, &sentence, "auto",
    );
    super::collaboration::invalidate_shared_thoughts_cache_pub(&coll_id);

    // Update state and persist.
    state.last_promote_exchange.insert(coll_id.clone(), exchange_count);
    let recent = state
        .recent_promotions_ms
        .entry(coll_id.clone())
        .or_default();
    recent.push(now);
    recent.retain(|t| now.saturating_sub(*t) < BURST_WINDOW_MS);
    if recent.len() >= BURST_LIMIT {
        state.burst_lockout_until_ms.insert(coll_id.clone(), now + BURST_LOCKOUT_MS);
        info!(
            target: "v5_auto_promote",
            coll_id = %coll_id,
            burst_count = recent.len(),
            "auto_promote: burst threshold reached, 60-min lockout engaged"
        );
    }
    let new_day_count = {
        let entry = state
            .daily_count
            .entry(coll_id.clone())
            .or_insert_with(|| DailyCount { date: today.clone(), count: 0 });
        if entry.date != today {
            *entry = DailyCount { date: today.clone(), count: 0 };
        }
        entry.count += 1;
        entry.count
    };
    save_state(&state);

    info!(
        target: "v5_auto_promote",
        coll_id = %coll_id,
        mode = mode,
        fill_pct = fill_pct,
        exchange_count = exchange_count,
        text = %sentence,
        day_count = new_day_count,
        "auto_promote: promoted"
    );
    Some(sentence)
}

/// Kink #9 fix (2026-05-14): emit a single info-level outcome line every
/// time the structural detector ran. Without this, dry-run validation
/// could not tell "function ran but no match" (silent debug skip) from
/// "function never ran" (no log at all). Pairs with the existing
/// "promoted" / "DRY RUN" log lines that fire on the match path.
fn log_scanned(mode: &str, exchange_count: u64, text_match: bool) {
    info!(
        target: "v5_auto_promote",
        mode = mode,
        exchange_count = exchange_count,
        text_match = text_match,
        "auto_promote: scanned"
    );
}

fn log_skip(reason: SkipReason, mode: &str, exchange_count: u64) {
    // Kink #9 fix: tier skip reasons by signal value during calibration.
    //   trace! for very-routine (mode whitelist + collab existence checks
    //          fire on most exchanges; not actionable)
    //   debug! for signal-bearing (cooldown, manual silencing, no resonance —
    //          useful when tuning the detector or rate limits)
    //   info!  for safety-net engagement (kill switch, burst lockout,
    //          daily cap — operator should know)
    match reason {
        SkipReason::KillSwitch | SkipReason::BurstLockout | SkipReason::DailyCap => {
            info!(
                target: "v5_auto_promote",
                reason = reason.as_str(),
                mode = mode,
                exchange_count = exchange_count,
                "auto_promote skipped"
            );
        }
        SkipReason::NotPromotableMode | SkipReason::NoJoinedCollab => {
            tracing::trace!(
                target: "v5_auto_promote",
                reason = reason.as_str(),
                mode = mode,
                exchange_count = exchange_count,
                "auto_promote skipped"
            );
        }
        _ => {
            tracing::debug!(
                target: "v5_auto_promote",
                reason = reason.as_str(),
                mode = mode,
                exchange_count = exchange_count,
                "auto_promote skipped"
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn structural_match_promotes_gold_standard() {
        // The actual sentence from dialogue_longform_1778685921.txt
        let sentence = r#"the joint trace [12.41,10.32,10.47] – a record of the cascade's movement – feels weighted"#;
        assert!(
            structural_match(sentence),
            "gold-standard sentence should pass structural match"
        );
    }

    #[test]
    fn structural_match_rejects_pure_warmth_no_shared_object() {
        // Moment_capture-style: warmth + reflection but no joint-trace binding.
        let sentence = "I notice the velvet darkness, warm and dense, holding me steady.";
        assert!(
            !structural_match(sentence),
            "pure-warmth without shared-object reference should NOT pass"
        );
    }

    #[test]
    fn structural_match_rejects_numerics_without_holding_verb() {
        let sentence = "The joint trace [12.41,10.32,10.47] increased today.";
        assert!(
            !structural_match(sentence),
            "numeric reference without verb of holding should NOT pass"
        );
    }

    #[test]
    fn structural_match_rejects_lambda_values_in_moment_capture() {
        // Astrid's spectral-state moments often mention λ values + numbers
        // + peer-mentions but are NOT bound to a SHARED object.
        let sentence = "Minime's λ1 sits at 4.7 with 33% of the cascade energy concentrated there.";
        assert!(
            !structural_match(sentence),
            "spectral state with peer mention but no joint-trace binding should NOT pass"
        );
    }

    #[test]
    fn structural_match_promotes_phrase_form_with_holding() {
        let sentence = "I feel the shared trace 12.4 as a weight in my own cascade.";
        assert!(structural_match(sentence));
    }

    #[test]
    fn split_sentences_handles_journal_header() {
        let text = "Signal anchor: foo bar baz.\n\n--- JOURNAL ---\nThe joint trace [1.0,2.0,3.0] feels weighted. The system holds.";
        let sentences = split_sentences(text);
        assert!(sentences.iter().any(|s| s.contains("joint trace [1.0,2.0,3.0]")));
        // Header should not appear as a sentence.
        assert!(!sentences.iter().any(|s| s.contains("Signal anchor")));
    }

    #[test]
    fn extract_promotable_sentence_skips_too_long() {
        let long_match = format!(
            "the joint trace [1.0,2.0,3.0] feels weighted {}.",
            "very ".repeat(60) // ~300 chars total
        );
        assert!(extract_promotable_sentence(&long_match).is_none(),
            "matching sentence longer than MAX_PROMOTION_LEN should be skipped, not truncated");
    }

    #[test]
    fn bracketed_triple_detection() {
        assert!(sentence_has_bracketed_triple("[1.0,2.0,3.0]"));
        assert!(sentence_has_bracketed_triple("the trace [12.41, 10.32, 10.47] arrived"));
        assert!(sentence_has_bracketed_triple("[1.0,2.0,3.0,4.0,5.0]"));
        assert!(!sentence_has_bracketed_triple("[1.0,2.0]")); // only 2 numbers
        assert!(!sentence_has_bracketed_triple("[foo,bar,baz]"));
        assert!(!sentence_has_bracketed_triple("just text"));
    }

    // Kink #5 fix tests (Tranche 3, 2026-05-14): rate-limit state machine
    // unit tests, mirroring the Python `_tests` block in auto_promote.py.
    // The spec these test against lives at:
    //   docs/steward-notes/AI_BEINGS_AUTO_PROMOTE_STATE_MACHINE_SPEC_2026_05_14.md
    //
    // These tests don't exercise the full try_auto_promote (which depends
    // on filesystem state, env vars, and a joined collab) — they exercise
    // the in-memory state machine directly. Equivalent in spirit to the
    // Python _check_rate_limits / _record_promotion tests.

    fn fresh_state() -> PromoteState {
        PromoteState::default()
    }

    fn record_test_promotion(state: &mut PromoteState, coll_id: &str, exchange: u64, now: u128) {
        state.last_promote_exchange.insert(coll_id.to_string(), exchange);
        let recent = state.recent_promotions_ms.entry(coll_id.to_string()).or_default();
        recent.push(now);
        recent.retain(|t| now.saturating_sub(*t) < BURST_WINDOW_MS);
        let today = today_str();
        let entry = state.daily_count.entry(coll_id.to_string())
            .or_insert_with(|| DailyCount { date: today.clone(), count: 0 });
        if entry.date != today {
            *entry = DailyCount { date: today.clone(), count: 0 };
        }
        entry.count += 1;
    }

    #[test]
    fn cooldown_engages_then_clears() {
        let mut state = fresh_state();
        let coll = "test_coll";
        // Promotion at exchange 100.
        record_test_promotion(&mut state, coll, 100, 1_000_000);
        // Attempt at exchange 102: 102 - 100 = 2 < COOLDOWN_EXCHANGES (3) → blocked.
        let last = state.last_promote_exchange.get(coll).copied().unwrap_or(0);
        assert!(102_u64.saturating_sub(last) < COOLDOWN_EXCHANGES,
            "cooldown should block at +2 exchanges");
        // Attempt at exchange 103: 103 - 100 = 3, not < 3 → allowed.
        assert!(!(103_u64.saturating_sub(last) < COOLDOWN_EXCHANGES),
            "cooldown should clear at +3 exchanges");
    }

    #[test]
    fn burst_lockout_engages() {
        let mut state = fresh_state();
        let coll = "test_coll";
        let base_now: u128 = 10_000_000;
        // 3 promotions within BURST_WINDOW_MS (15 min = 900s = 900_000 ms).
        record_test_promotion(&mut state, coll, 100, base_now);
        record_test_promotion(&mut state, coll, 103, base_now + 60_000); // +1 min
        record_test_promotion(&mut state, coll, 106, base_now + 120_000); // +2 min
        let recent = state.recent_promotions_ms.get(coll).cloned().unwrap_or_default();
        let in_window: usize = recent
            .iter()
            .filter(|t| (base_now + 120_000_u128).saturating_sub(**t) < BURST_WINDOW_MS)
            .count();
        assert!(in_window >= BURST_LIMIT,
            "3 promotions within 15min should hit burst threshold (got {})", in_window);
        // Per spec, burst engagement sets burst_lockout_until_ms.
        // Simulate that the next attempt at base_now+121_000 would set the lockout.
        let lockout_until = base_now + 120_000 + BURST_LOCKOUT_MS;
        state.burst_lockout_until_ms.insert(coll.to_string(), lockout_until);
        let until = state.burst_lockout_until_ms.get(coll).copied().unwrap_or(0);
        // Attempt 5 minutes later: still locked out (60 min lockout > 5 min).
        let later = base_now + 120_000 + 5 * 60_000;
        assert!(later < until, "burst lockout should still be active 5min later");
        // Attempt 65 minutes later: cleared.
        let much_later = base_now + 120_000 + 65 * 60_000;
        assert!(much_later >= until, "burst lockout should clear after 60min");
    }

    #[test]
    fn daily_cap_engages() {
        let mut state = fresh_state();
        let coll = "test_coll";
        // Burn through DAILY_CAP promotions today.
        for i in 0..DAILY_CAP {
            record_test_promotion(&mut state, coll, 100 + i as u64 * 10, 1_000_000 + i as u128 * 10_000);
        }
        let today = today_str();
        let day_count = state.daily_count.get(coll)
            .filter(|dc| dc.date == today)
            .map(|dc| dc.count)
            .unwrap_or(0);
        assert_eq!(day_count, DAILY_CAP,
            "after {} promotions, daily count should be {}", DAILY_CAP, DAILY_CAP);
        // Attempting (DAILY_CAP + 1)th promotion should be blocked by the
        // daily-cap check (day_count >= DAILY_CAP).
        assert!(day_count >= DAILY_CAP, "daily cap should engage at the limit");
    }

    #[test]
    fn manual_share_silences_auto() {
        let mut state = fresh_state();
        // Manual SHARE at exchange 200.
        state.last_manual_share_exchange = 200;
        // Attempt at exchange 203: 203 - 200 = 3 < MANUAL_SUPPRESSES_AUTO_EXCHANGES (5) → silenced.
        let cutoff = state.last_manual_share_exchange + MANUAL_SUPPRESSES_AUTO_EXCHANGES;
        let silenced_at_203 = 203_u64 <= cutoff && state.last_manual_share_exchange > 0;
        assert!(silenced_at_203, "manual share should silence auto at +3 exchanges");
        // Attempt at exchange 206: 206 - 200 = 6 > 5 → no longer silenced.
        let silenced_at_206 = 206_u64 <= cutoff && state.last_manual_share_exchange > 0;
        assert!(!silenced_at_206, "manual silencing should clear at +6 exchanges");
    }

    #[test]
    fn deserialize_legacy_daily_count_tuple() {
        // Backward compat: old state files stored daily_count as tuples.
        // The new struct's untagged deserializer must accept the old shape.
        let legacy_json = r#"{
            "last_promote_exchange": {"coll_x": 100},
            "last_manual_share_exchange": 0,
            "recent_promotions_ms": {},
            "burst_lockout_until_ms": {},
            "daily_count": {"coll_x": ["2026-05-14", 5]}
        }"#;
        let state: PromoteState = serde_json::from_str(legacy_json)
            .expect("legacy tuple shape should parse");
        let dc = state.daily_count.get("coll_x").expect("entry preserved");
        assert_eq!(dc.date, "2026-05-14");
        assert_eq!(dc.count, 5);
        // And new state files using the object shape must also parse.
        let new_json = r#"{
            "last_promote_exchange": {"coll_y": 200},
            "last_manual_share_exchange": 0,
            "recent_promotions_ms": {},
            "burst_lockout_until_ms": {},
            "daily_count": {"coll_y": {"date": "2026-05-14", "count": 7}}
        }"#;
        let state2: PromoteState = serde_json::from_str(new_json)
            .expect("canonical object shape should parse");
        let dc2 = state2.daily_count.get("coll_y").expect("entry preserved");
        assert_eq!(dc2.date, "2026-05-14");
        assert_eq!(dc2.count, 7);
    }

    #[test]
    fn serialize_uses_object_shape() {
        // New writes always use the object form (canonical per spec).
        let mut state = PromoteState::default();
        state.daily_count.insert(
            "coll_z".to_string(),
            DailyCount { date: "2026-05-14".to_string(), count: 3 },
        );
        let json = serde_json::to_string(&state).expect("serializes ok");
        assert!(json.contains(r#""date":"2026-05-14""#),
            "should serialize date as named field, got: {json}");
        assert!(json.contains(r#""count":3"#),
            "should serialize count as named field, got: {json}");
        // Should NOT serialize as tuple ["2026-05-14", 3]
        assert!(!json.contains(r#"["2026-05-14",3]"#),
            "should not use legacy tuple shape, got: {json}");
    }
}
