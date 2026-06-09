mod ask_steward;
mod attractor;
mod audio;
pub(crate) mod auto_promote;
mod autoresearch;
mod codex;
pub(crate) mod collaboration;
mod identify_pattern;
mod mike;
mod modes;
mod native_gesture;
mod operations;
mod pdf;
pub(crate) mod protected_diagnostics;
mod resource_governor;
pub(crate) mod shadow;
pub(crate) mod sovereignty;
mod space_hold;
mod spectral_drift;
mod workspace;

pub(crate) const PDF_READ_PREFIX: &str = pdf::PDF_READ_PREFIX;

use tokio::sync::mpsc;
use tracing::info;

use super::{ConversationState, Mode, list_directory, save_astrid_journal, truncate_str};
use crate::action_continuity::{self, NextActionOutcome};
use crate::db::BridgeDb;
use crate::paths::bridge_paths;
use crate::types::{SensoryMsg, SpectralTelemetry};

use super::reservoir;

#[derive(Debug, Clone, serde::Serialize)]
pub(crate) struct ActionPreflightReport {
    pub schema_version: u32,
    pub policy: &'static str,
    pub dry_run: bool,
    pub raw_action: String,
    pub canonical_action: String,
    pub base_action: String,
    pub effective_route: String,
    pub stage: String,
    pub visibility: String,
    pub authority_required: String,
    pub expected_continuity_effect: String,
    pub likely_gate: String,
    pub expected_artifact_kinds: Vec<String>,
    pub suggested_next: String,
}

impl ActionPreflightReport {
    #[must_use]
    pub(crate) fn render(&self) -> String {
        format!(
            "=== ACTION PREFLIGHT V1 ===\n\
             Dry run: {}\n\
             Raw action: {}\n\
             Canonical action: {}\n\
             Base action: {}\n\
             Effective route: {}\n\
             Stage: {}\n\
             Visibility: {}\n\
             Authority required: {}\n\
             Expected continuity: {}\n\
             Likely gate: {}\n\
             Expected artifacts: {}\n\
             Suggested next: {}",
            self.dry_run,
            self.raw_action,
            self.canonical_action,
            self.base_action,
            self.effective_route,
            self.stage,
            self.visibility,
            self.authority_required,
            self.expected_continuity_effect,
            self.likely_gate,
            if self.expected_artifact_kinds.is_empty() {
                "(none)".to_string()
            } else {
                self.expected_artifact_kinds.join(", ")
            },
            self.suggested_next
        )
    }
}

pub(super) struct NextActionContext<'a> {
    pub burst_count: &'a mut u32,
    pub db: &'a BridgeDb,
    pub sensory_tx: &'a mpsc::Sender<SensoryMsg>,
    pub telemetry: &'a SpectralTelemetry,
    pub fill_pct: f32,
    pub response_text: &'a str,
    pub workspace: Option<&'a std::path::Path>,
}

/// Parse NEXT: action from Astrid's response.
pub(crate) fn parse_next_action(text: &str) -> Option<&str> {
    let mut in_fence = false;
    for line in text.lines().rev() {
        let trimmed = line.trim();
        if trimmed.starts_with("```") {
            in_fence = !in_fence;
            continue;
        }
        if in_fence {
            continue;
        }
        if let Some(action) = trimmed.strip_prefix("NEXT:") {
            let mut clean = action.trim();
            for token in &[
                "<end_of_turn>",
                "<END_OF_TURN>",
                "<End_of_turn>",
                "</s>",
                "<|endoftext|>",
            ] {
                clean = clean.trim_end_matches(token);
            }
            if let Some(pos) = clean.rfind('<') {
                let after = &clean[pos..];
                if after.contains("end")
                    || after.contains("turn")
                    || after.contains("eos")
                    || after.len() < 20
                {
                    clean = clean[..pos].trim();
                }
            }
            // Kink follow-up (2026-05-14, post-Tranche-5): strip markdown
            // decorations from leading/trailing positions. Recurring LLM
            // artifact: `**READ_MORE**`, ` `RELEASE_SHADOW ...` `, etc. land
            // in NEXT lines because chat models emit markdown bold/code
            // formatting around action names. See
            // project_unwired_actions_catalog.md for the diagnostic.
            // Slice-based trim preserves &str return type — no allocation.
            clean = clean.trim_matches(|c| c == '`' || c == '*');
            return Some(clean.trim());
        }
    }
    None
}

pub(crate) fn attractor_suggestion_prompt_note() -> Option<String> {
    attractor::pending_suggestion_prompt_note()
}

fn first_quoted_span(text: &str) -> Option<&str> {
    let open_idx = text.find(['"', '\'', '“'])?;
    let open = text[open_idx..].chars().next()?;
    let close = match open {
        '“' => '”',
        '"' | '\'' => open,
        _ => return None,
    };
    let rest = &text[open_idx + open.len_utf8()..];
    let close_idx = rest.find(close)?;
    Some(rest[..close_idx].trim())
}

fn clean_search_topic(candidate: &str) -> Option<String> {
    let topic = candidate
        .split('<')
        .next()
        .unwrap_or(candidate)
        .trim()
        .trim_matches(|c: char| matches!(c, '"' | '\'' | '“' | '”'))
        .trim()
        .trim_end_matches(['.', ',', ';', ':'])
        .trim();

    if topic.chars().any(char::is_alphanumeric) {
        Some(topic.to_string())
    } else {
        None
    }
}

pub(crate) fn extract_search_topic(next_action: &str) -> Option<String> {
    let trimmed = next_action.trim();
    if trimmed.len() < 6 || !trimmed[..6].eq_ignore_ascii_case("SEARCH") {
        return None;
    }

    let rest = trimmed[6..]
        .trim()
        .trim_start_matches(['-', '\u{2014}', ':'])
        .trim();

    if rest.is_empty() {
        return None;
    }

    if let Some(quoted) = first_quoted_span(rest) {
        return clean_search_topic(quoted);
    }

    let mut end = rest.len();
    if let Some(idx) = rest.find('\u{2014}') {
        end = end.min(idx);
    }
    if let Some(idx) = rest.find(" - ") {
        end = end.min(idx);
    }

    clean_search_topic(rest[..end].trim())
}

fn clean_alias_arg(raw: &str) -> String {
    raw.trim()
        .trim_start_matches([':', '-', '\u{2014}'])
        .trim()
        .trim_matches(|c: char| matches!(c, '[' | ']' | '"' | '\'' | '`' | '“' | '”'))
        .trim()
        .to_string()
}

fn clean_shadow_decompose_focus(raw: &str) -> String {
    let focus = clean_alias_arg(raw);
    let normalized = focus
        .to_ascii_lowercase()
        .replace(['.', ',', ';'], "")
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ");
    if focus.is_empty() || normalized == "observer with memory" {
        "lambda-tail/lambda4".to_string()
    } else {
        focus
    }
}

fn clean_weave_trace_focus(raw: &str) -> String {
    let focus = clean_alias_arg(raw);
    let normalized = focus
        .to_ascii_lowercase()
        .replace(['.', ',', ';'], "")
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ");
    if focus.is_empty() || normalized == "observer with memory" {
        "weave/lambda4".to_string()
    } else if normalized.starts_with("weave/") || normalized.starts_with("weave ") {
        focus
    } else {
        format!("weave/{focus}")
    }
}

fn normalize_codeish_target(raw: &str) -> Option<String> {
    let mut target = clean_alias_arg(raw);
    if target.is_empty() {
        return None;
    }

    if let Some((_, value)) = target.split_once('=') {
        target = value.trim().to_string();
    }

    target = target
        .split('#')
        .next()
        .unwrap_or(&target)
        .trim()
        .to_string();

    if let Some(last) = target.rsplit('/').next() {
        target = last.trim().to_string();
    }

    let lower = target.to_ascii_lowercase();
    for suffix in [".rs", ".py", ".md", ".json", ".toml"] {
        if lower.ends_with(suffix) {
            target.truncate(target.len().saturating_sub(suffix.len()));
            break;
        }
    }

    let target = target.trim().to_lowercase();
    (!target.is_empty()).then_some(target)
}

fn humanize_examine_suffix(suffix: &str) -> String {
    suffix
        .split('_')
        .filter(|part| !part.is_empty())
        .map(|part| part.to_lowercase())
        .collect::<Vec<_>>()
        .join(" ")
}

fn unwrap_outer_action_wrappers(original: &str) -> String {
    let mut current = original.trim().to_string();
    loop {
        let Some(open) = current.chars().next() else {
            break;
        };
        let close = match open {
            '[' => ']',
            '(' => ')',
            '{' => '}',
            '<' => '>',
            _ => break,
        };
        if !current.ends_with(close) {
            break;
        }
        let inner = current[open.len_utf8()..current.len().saturating_sub(close.len_utf8())]
            .trim()
            .to_string();
        if inner
            .chars()
            .next()
            .is_none_or(|c| !(c.is_ascii_alphanumeric() || c == '_'))
        {
            break;
        }
        current = inner;
    }
    current
}

fn leading_action_token(original: &str) -> String {
    original
        .chars()
        .take_while(|c| c.is_ascii_alphanumeric() || *c == '_')
        .collect::<String>()
        .to_uppercase()
}

/// v4.0 Phase 1: action-token-likeness heuristic for multi-NEXT splitting.
/// Returns true when `token` looks like an action verb that's safe to chain.
///
/// v4.0 Phase 2.3 (strict heuristic): require the post-AND token to contain
/// at least one underscore. Earlier denylist approach was whack-a-mole —
/// production caught WHAT, then DECIDE, then LOCAL as false positives, with
/// no end in sight. Almost all action verbs in our vocabulary contain
/// underscores (TUNE_MINIME, SHADOW_FIELD, READ_MORE, COMPARE_BASELINE,
/// ACCEPT_PARAMETER_REQUEST, etc.); bare single-word verbs (BROWSE, ASK,
/// EXAMINE, DEFER, ACCEPT) lose chain-as-second-action capability but can
/// still be emitted as single NEXTs or as the FIRST segment in a chain.
/// This trades a small expressivity loss for ~zero false-positive splits
/// from natural English conjunctions.
fn is_action_token_like(token: &str) -> bool {
    if token.len() < 5 {
        return false;
    }
    if !token
        .chars()
        .all(|c| c.is_ascii_uppercase() || c.is_ascii_digit() || c == '_')
    {
        return false;
    }
    // Require at least one underscore — separates compound action verbs
    // (TUNE_MINIME, READ_MORE) from natural English words (LOCAL, DECIDE).
    token.contains('_')
}

/// v4.0 Phase 1: split a NEXT body on ` AND ` boundaries when the
/// post-AND segment starts with a token that looks like an action verb.
/// Returns 1 segment for single-action NEXTs (backward compatible).
/// Capped at `MAX_MULTI_ACTION_SEGMENTS` (3) to limit blast radius; extra
/// segments are logged and dropped.
///
/// Examples:
///   "BROWSE arxiv AND READ_MORE"             → ["BROWSE arxiv", "READ_MORE"]
///   "EXAMINE λ2 AND λ3 dynamics"             → ["EXAMINE λ2 AND λ3 dynamics"]  (single)
///   "EXAMINE foo AND DEFER reason"           → ["EXAMINE foo", "DEFER reason"]
///   "A AND B AND C AND D"                    → ["A", "B", "C AND D"]  (truncated to 3)
fn split_multi_action(original: &str) -> Vec<String> {
    const MAX_MULTI_ACTION_SEGMENTS: usize = 3;
    let mut segments: Vec<String> = Vec::new();
    let mut remaining = original;
    while segments.len() + 1 < MAX_MULTI_ACTION_SEGMENTS {
        // Search for next case-insensitive " AND " whose post-segment
        // begins with an action-token-like word. Iterate occurrences
        // until we find one that satisfies the gate; if none, stop.
        let mut search_from = 0usize;
        let lower = remaining.to_ascii_lowercase();
        let mut found: Option<usize> = None;
        while let Some(pos_rel) = lower[search_from..].find(" and ") {
            let abs = search_from + pos_rel;
            let post = &remaining[abs + 5..]; // skip " AND "
            let post_token = leading_action_token(post);
            if is_action_token_like(&post_token) {
                found = Some(abs);
                break;
            }
            search_from = abs + 5;
            if search_from >= lower.len() {
                break;
            }
        }
        match found {
            Some(abs) => {
                let pre = remaining[..abs].trim();
                if !pre.is_empty() {
                    segments.push(pre.to_string());
                }
                remaining = &remaining[abs + 5..];
            },
            None => break,
        }
    }
    let tail = remaining.trim();
    if !tail.is_empty() {
        segments.push(tail.to_string());
    }
    if segments.is_empty() {
        // Shouldn't happen for non-empty input, but guard anyway.
        segments.push(original.trim().to_string());
    }
    segments
}

fn unresolved_angle_placeholder(text: &str) -> Option<String> {
    let mut rest = text;
    while let Some(start) = rest.find('<') {
        let after_start = &rest[start + 1..];
        let Some(end) = after_start.find('>') else {
            break;
        };
        let token = after_start[..end].trim();
        let normalized = token.to_ascii_lowercase();
        let known_placeholder = matches!(
            normalized.as_str(),
            "action"
                | "cmd"
                | "command"
                | "description"
                | "dir"
                | "dirname"
                | "file"
                | "gesture"
                | "job"
                | "label"
                | "name"
                | "note"
                | "path"
                | "project"
                | "prompt"
                | "question"
                | "src"
                | "text"
                | "topic"
                | "url"
                | "value"
                | "workspace"
                | "ws"
        );
        if known_placeholder || normalized.contains('|') {
            return Some(format!("<{token}>"));
        }
        rest = &after_start[end + 1..];
    }
    None
}

fn strip_action_call_wrapper(original: &str, base_action: &str) -> Option<String> {
    let rest = original.get(base_action.len()..)?.trim_start();
    if !(rest.starts_with('(') && rest.ends_with(')')) {
        return None;
    }
    let inner = rest[1..rest.len().saturating_sub(1)].trim();
    (!inner.is_empty()).then(|| inner.to_string())
}

fn normalize_gesture_alias(base_action: &str, original: &str) -> Option<(String, String)> {
    if base_action == "GESTURE" {
        if let Some(inner) = strip_action_call_wrapper(original, base_action) {
            return Some(("GESTURE".to_string(), format!("GESTURE {inner}")));
        }
        return None;
    }

    let suffix = base_action.strip_prefix("GESTURE_")?;
    let raw_arg = strip_action(original, base_action);
    let alias_focus = humanize_examine_suffix(suffix);
    let clean_arg = clean_alias_arg(&raw_arg);
    let combined = if clean_arg.is_empty() {
        alias_focus
    } else if alias_focus.is_empty() {
        clean_arg
    } else {
        format!("{alias_focus} {clean_arg}")
    };
    let normalized_original = if combined.is_empty() {
        "GESTURE".to_string()
    } else {
        format!("GESTURE {combined}")
    };
    Some(("GESTURE".to_string(), normalized_original))
}

fn normalize_native_trace_alias(base_action: &str, original: &str) -> Option<(String, String)> {
    if !matches!(base_action, "TRACE" | "TRACE_LAMBDA" | "LAMBDA_TRACE") {
        return None;
    }
    let label = clean_alias_arg(&strip_action(original, base_action));
    let normalized_original = if label.is_empty() {
        "NATIVE_GESTURE trace lambda-edge".to_string()
    } else {
        format!("NATIVE_GESTURE trace {label}")
    };
    Some(("NATIVE_GESTURE".to_string(), normalized_original))
}

fn normalize_native_fissure_alias(base_action: &str, original: &str) -> Option<(String, String)> {
    if !matches!(base_action, "FISSURE" | "FISSIURE") {
        return None;
    }
    let label = clean_alias_arg(&strip_action(original, base_action));
    let normalized_original = if label.is_empty() {
        "NATIVE_GESTURE fissure".to_string()
    } else {
        format!("NATIVE_GESTURE fissure {label}")
    };
    Some(("NATIVE_GESTURE".to_string(), normalized_original))
}

fn normalize_fissure_trace_alias(base_action: &str, original: &str) -> Option<(String, String)> {
    if !matches!(
        base_action,
        "FISSURE_TRACE" | "NOTICE_AMBIGUITY" | "AMBIGUITY_TRACE"
    ) {
        return None;
    }
    let label = clean_alias_arg(&strip_action(original, base_action));
    let normalized_original = if label.is_empty() {
        "FISSURE_TRACE".to_string()
    } else {
        format!("FISSURE_TRACE {label}")
    };
    Some(("FISSURE_TRACE".to_string(), normalized_original))
}

fn normalize_sca_reflect_alias(base_action: &str, original: &str) -> Option<(String, String)> {
    if !matches!(base_action, "SCA" | "SCA_REFLECT" | "SCA_REFLECTION") {
        return None;
    }
    let label = clean_alias_arg(&strip_action(original, base_action));
    let normalized_original = if label.is_empty() {
        "SCA_REFLECT".to_string()
    } else {
        format!("SCA_REFLECT {label}")
    };
    Some(("SCA_REFLECT".to_string(), normalized_original))
}

fn normalize_memory_search_alias(base_action: &str, original: &str) -> Option<(String, String)> {
    if base_action != "MEMORY" {
        return None;
    }
    let rest = strip_action(original, "MEMORY");
    let rest_base = leading_action_token(&rest);
    if rest_base != "SEARCH" && rest_base != "RESEARCH" {
        return None;
    }
    let topic = clean_alias_arg(&strip_action(&rest, &rest_base));
    let normalized_original = if topic.is_empty() {
        "SEARCH".to_string()
    } else {
        format!("SEARCH {topic}")
    };
    Some(("SEARCH".to_string(), normalized_original))
}

fn normalize_gemma4_observed_action_alias(
    base_action: &str,
    original: &str,
) -> Option<(String, String)> {
    if base_action == "STICKY_MODE_AUDIT" {
        return Some((
            "ACTION_PREFLIGHT".to_string(),
            "ACTION_PREFLIGHT CAPABILITY_MAP STICKY_MODE_AUDIT".to_string(),
        ));
    }

    if base_action == "EXPERIMENT_AUTHORITY_PREFLIGHT" {
        return Some((
            "ACTION_PREFLIGHT".to_string(),
            "ACTION_PREFLIGHT EXPERIMENT_CHARTER current".to_string(),
        ));
    }

    let original_lower = original
        .to_ascii_lowercase()
        .replace("\\lambda", "lambda")
        .replace("\\and", "and");
    if base_action == "EXPLORE"
        && original_lower.contains("read_more")
        && original_lower.contains("analyze_audio")
    {
        return Some((
            "ACTION_PREFLIGHT".to_string(),
            "ACTION_PREFLIGHT READ_MORE".to_string(),
        ));
    }

    if base_action == "EXPLORE"
        && original_lower.contains("decompose")
        && (original_lower.contains("trace_bridge") || original_lower.contains("trace bridge"))
    {
        return Some((
            "ACTION_PREFLIGHT".to_string(),
            "ACTION_PREFLIGHT DECOMPOSE lambda1/lambda4 bridge trace".to_string(),
        ));
    }

    if base_action == "EXPLORE_RESONANCE_DENSITY" {
        return Some((
            "ACTION_PREFLIGHT".to_string(),
            "ACTION_PREFLIGHT RESONANCE_FORECAST resonance-density contracting phase".to_string(),
        ));
    }

    if base_action == "EXPLORE_RESONANCE_FORECAST" {
        let raw_arg = clean_alias_arg(&strip_action(original, base_action));
        let normalized_original = if raw_arg.is_empty() {
            "RESONANCE_FORECAST".to_string()
        } else {
            format!("RESONANCE_FORECAST {raw_arg}")
        };
        return Some(("RESONANCE_FORECAST".to_string(), normalized_original));
    }

    if base_action == "SHADOW_TRAJECTORY_EXPANSION_GRADIENT" {
        return Some((
            "SHADOW_TRAJECTORY".to_string(),
            "SHADOW_TRAJECTORY expansion gradient".to_string(),
        ));
    }

    if base_action == "EXPLORE_RESEARCH_QUERY" {
        let raw_arg = strip_action(original, base_action);
        let topic = first_quoted_span(&raw_arg)
            .and_then(clean_search_topic)
            .or_else(|| clean_search_topic(&raw_arg))
            .unwrap_or_else(|| "Gemma 4 action discipline".to_string());
        return Some(("SEARCH".to_string(), format!("SEARCH {topic}")));
    }

    if base_action == "EXPORT_SYSTEM_DIAGRAM" {
        return Some((
            "ACTION_PREFLIGHT".to_string(),
            "ACTION_PREFLIGHT CODEX \"draft a system diagram from current Astrid bridge architecture\""
                .to_string(),
        ));
    }

    None
}

fn normalize_visual_cascade_alias(base_action: &str, original: &str) -> Option<(String, String)> {
    let raw_arg = strip_action(original, base_action);
    let clean_arg = clean_alias_arg(&raw_arg);
    let original_lower = original.to_ascii_lowercase();
    let spectral_focus = original_lower.contains("cascade")
        || original_lower.contains("spectral")
        || original_lower.contains("eigen")
        || original_lower.contains("lambda")
        || original_lower.contains('λ')
        || original_lower.contains("heatmap")
        || original_lower.contains("plot")
        || original_lower.contains("chart");

    let alias = matches!(
        base_action,
        "VISUALIZE_CASCADE"
            | "CASCADE"
            | "CONDUCT_VISUALIZATION_SYSTEM"
            | "CONDUCT_VISUALIZATION"
            | "CONDUCT_VISUALIZAT"
            | "RENDER_CASCADE"
            | "SHOW_CASCADE"
            | "PLOT_CASCADE"
            | "HEATMAP_CASCADE"
            | "SPECTRAL_HEATMAP"
            | "SPECTRAL_PLOT"
            | "LAMBDA_HEATMAP"
            | "LAMBDA_PLOT"
    ) || (matches!(
        base_action,
        "VISUALIZE" | "VISUALIZATION" | "HEATMAP" | "PLOT" | "CHART"
    ) && spectral_focus);
    if !alias {
        return None;
    }

    let normalized_original = if clean_arg.is_empty() {
        "VISUALIZE_CASCADE".to_string()
    } else {
        format!("VISUALIZE_CASCADE {clean_arg}")
    };
    Some(("VISUALIZE_CASCADE".to_string(), normalized_original))
}

fn normalize_reconvergence_map_alias(
    base_action: &str,
    original: &str,
) -> Option<(String, String)> {
    if !matches!(
        base_action,
        "RECONVERGENCE_MAP"
            | "ATTRACTOR_MAP"
            | "ACTIVATION_TRACE"
            | "COMPARE_BASELINE"
            | "COMPARE_RECONVERGENCE"
            | "BASELINE_COMPARE"
    ) {
        return None;
    }
    let raw_arg = strip_action(original, base_action);
    let clean_arg = clean_alias_arg(&raw_arg);
    if matches!(
        base_action,
        "COMPARE_BASELINE" | "COMPARE_RECONVERGENCE" | "BASELINE_COMPARE"
    ) {
        let normalized_original = if clean_arg.is_empty() {
            "RECONVERGENCE_MAP compare-baseline".to_string()
        } else {
            format!("RECONVERGENCE_MAP compare-baseline {clean_arg}")
        };
        return Some(("RECONVERGENCE_MAP".to_string(), normalized_original));
    }
    let normalized_original = if clean_arg.is_empty() {
        "RECONVERGENCE_MAP".to_string()
    } else {
        format!("RECONVERGENCE_MAP {clean_arg}")
    };
    Some(("RECONVERGENCE_MAP".to_string(), normalized_original))
}

fn normalize_bridge_trace_alias(base_action: &str, original: &str) -> Option<(String, String)> {
    if !matches!(base_action, "M6_BRIDGE" | "TRACE_BRIDGE" | "BRIDGE_TRACE") {
        return None;
    }
    let raw_arg = strip_action(original, base_action);
    let clean_arg = clean_alias_arg(&raw_arg);
    let normalized_original = if clean_arg.is_empty() {
        "BRIDGE_TRACE m6".to_string()
    } else {
        format!("BRIDGE_TRACE {clean_arg}")
    };
    Some(("BRIDGE_TRACE".to_string(), normalized_original))
}

fn trim_experiment_run_payload(raw: &str) -> String {
    let mut trimmed = raw.trim().trim_matches('|').trim().to_string();
    while let Some(first) = trimmed.chars().next() {
        let close = match first {
            '[' => ']',
            '(' => ')',
            '{' => '}',
            '<' => '>',
            '"' => '"',
            '\'' => '\'',
            _ => break,
        };
        if !trimmed.ends_with(close) {
            break;
        }
        trimmed = trimmed[first.len_utf8()..trimmed.len().saturating_sub(close.len_utf8())]
            .trim()
            .to_string();
    }
    trimmed
}

fn split_experiment_command_marker(arg: &str) -> Option<(&str, &str)> {
    let lower = arg.to_ascii_lowercase();
    let mut best: Option<(usize, usize)> = None;
    for needle in ["| cmd ", " cmd=", " cmd:", " cmd ", "<cmd=", "<cmd:"] {
        if let Some(idx) = lower.find(needle) {
            let value_start = idx + needle.len();
            best = match best {
                Some((best_idx, best_start)) if best_idx <= idx => Some((best_idx, best_start)),
                _ => Some((idx, value_start)),
            };
        }
    }
    best.map(|(idx, value_start)| (&arg[..idx], &arg[value_start..]))
}

fn extract_workspace_marker(raw: &str) -> Option<(String, String)> {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return None;
    }

    if let Some(value) = trimmed.strip_prefix("-ws ") {
        let mut parts = value.splitn(2, char::is_whitespace);
        let workspace = parts.next().unwrap_or_default().to_string();
        let rest = parts.next().unwrap_or_default().trim().to_string();
        return Some((workspace, rest));
    }

    if let Some(value) = trimmed.strip_prefix("--workspace ") {
        let mut parts = value.splitn(2, char::is_whitespace);
        let workspace = parts.next().unwrap_or_default().to_string();
        let rest = parts.next().unwrap_or_default().trim().to_string();
        return Some((workspace, rest));
    }

    let first_token = trimmed.split_whitespace().next().unwrap_or_default();
    let rest_after_first = trimmed[first_token.len()..].trim().to_string();
    let lower_first = first_token.to_ascii_lowercase();
    for prefix in [
        "workspace_name:",
        "workspace_name=",
        "workspace:",
        "workspace=",
        "ws:",
        "ws=",
    ] {
        if lower_first.starts_with(prefix) && first_token.len() > prefix.len() {
            let value = first_token[prefix.len()..].to_string();
            return Some((value, rest_after_first));
        }
    }

    None
}

fn normalize_experiment_workspace(raw: &str) -> String {
    let mut workspace = trim_experiment_run_payload(raw);
    for prefix in [
        "workspace/experiments/",
        "experiments/",
        "workspace/",
        "ws/",
    ] {
        if workspace.to_ascii_lowercase().starts_with(prefix) {
            workspace = workspace[prefix.len()..].to_string();
            break;
        }
    }
    workspace.trim_matches('/').trim().to_string()
}

fn normalize_experiment_run_alias(base_action: &str, original: &str) -> Option<(String, String)> {
    if base_action != "EXPERIMENT_RUN" && base_action != "EXP_RUN" {
        return None;
    }

    let mut arg = original.get(base_action.len()..)?.trim_start().to_string();
    if arg.starts_with(':') {
        arg = arg[1..].trim_start().to_string();
    } else if arg.starts_with('\u{2014}') {
        arg = arg['\u{2014}'.len_utf8()..].trim_start().to_string();
    }
    if arg.is_empty() {
        return None;
    }

    let (workspace_raw, command_raw) =
        if let Some((before, command)) = split_experiment_command_marker(&arg) {
            let command = trim_experiment_run_payload(command);
            if let Some((workspace, _rest)) = extract_workspace_marker(before) {
                (workspace, command)
            } else {
                let workspace = before
                    .split_whitespace()
                    .next()
                    .unwrap_or_default()
                    .to_string();
                (workspace, command)
            }
        } else if let Some((workspace, rest)) = extract_workspace_marker(&arg) {
            (workspace, trim_experiment_run_payload(&rest))
        } else {
            let mut parts = arg.splitn(2, char::is_whitespace);
            let workspace = parts.next().unwrap_or_default().to_string();
            let command = trim_experiment_run_payload(parts.next().unwrap_or_default());
            (workspace, command)
        };

    let workspace = normalize_experiment_workspace(&workspace_raw);
    let command = trim_experiment_run_payload(&command_raw);
    let normalized_original = if workspace.is_empty() || command.is_empty() {
        base_action.to_string()
    } else {
        format!("{base_action} {workspace} {command}")
    };
    Some((base_action.to_string(), normalized_original))
}

fn normalize_examine_alias(base_action: &str, original: &str) -> Option<(String, String)> {
    if !base_action.starts_with("EXAMINE_") {
        return None;
    }

    match base_action {
        "EXAMINE_AUDIO" | "EXAMINE_CASCADE" | "EXAMINE_CODE" | "EXAMINE_MEMORY" => {
            return None;
        },
        _ => {},
    }

    let suffix = &base_action["EXAMINE_".len()..];
    let raw_arg = strip_action(original, base_action);
    let clean_arg = clean_alias_arg(&raw_arg);

    match suffix {
        "SOURCE" | "ARCHITECTURE" | "COMMAND" | "TOOL" => {
            let normalized_target = normalize_codeish_target(&clean_arg)
                .or_else(|| (!clean_arg.is_empty()).then_some(clean_arg.to_lowercase()));
            let normalized_original = normalized_target.map_or_else(
                || "EXAMINE_CODE".to_string(),
                |target| format!("EXAMINE_CODE [{target}]"),
            );
            Some(("EXAMINE_CODE".to_string(), normalized_original))
        },
        _ => {
            let focus = if clean_arg.is_empty() {
                humanize_examine_suffix(suffix)
            } else {
                clean_arg
            };
            let normalized_original = if focus.is_empty() {
                "EXAMINE".to_string()
            } else {
                format!("EXAMINE {focus}")
            };
            Some(("EXAMINE".to_string(), normalized_original))
        },
    }
}

fn clean_plain_examine_focus(raw: &str) -> String {
    let trimmed = raw.trim();
    if let Some(inner) = trimmed
        .strip_prefix('[')
        .and_then(|after_open| after_open.split_once(']').map(|(inner, _)| inner))
    {
        return clean_alias_arg(inner);
    }

    let lower = trimmed.to_ascii_lowercase();
    let mut end = trimmed.len();
    for marker in [
        ", followed by",
        " followed by",
        "; followed by",
        ", then",
        " and then",
        "; then",
        " while looping",
        " while ",
    ] {
        if let Some(idx) = lower.find(marker) {
            end = end.min(idx);
        }
    }
    clean_alias_arg(trimmed[..end].trim())
}

fn normalize_plain_examine_action(base_action: &str, original: &str) -> Option<(String, String)> {
    if base_action != "EXAMINE" {
        return None;
    }

    let raw_arg = strip_action(original, "EXAMINE");
    let clean_arg = clean_plain_examine_focus(&raw_arg);
    if clean_arg.is_empty() {
        return None;
    }

    let normalized_original = format!("EXAMINE {clean_arg}");
    (normalized_original != original.trim()).then_some(("EXAMINE".to_string(), normalized_original))
}

fn normalize_feedback_shadow_model_alias(
    base_action: &str,
    original: &str,
) -> Option<(String, String)> {
    let lower = original
        .to_lowercase()
        .replace('λ', "lambda")
        .replace(['\u{2013}', '\u{2014}'], "-");

    if base_action == "REFINE_AUDIO_PROCESSING" {
        let raw_arg = strip_action(original, base_action);
        let focus = clean_alias_arg(&raw_arg);
        let normalized_original = if focus.is_empty() {
            "EXAMINE_AUDIO audio texture refinement".to_string()
        } else {
            format!("EXAMINE_AUDIO {}", focus)
        };
        return Some(("EXAMINE_AUDIO".to_string(), normalized_original));
    }

    if base_action.starts_with("INVESTIGATE")
        && (lower.contains("lambda4")
            || lower.contains("lambda-4")
            || lower.contains("lambda tail")
            || lower.contains("lambda-tail"))
    {
        return Some((
            "SHADOW_PREFLIGHT".to_string(),
            "SHADOW_PREFLIGHT lambda-tail/lambda4 --stage=rehearse".to_string(),
        ));
    }

    if base_action == "MODEL_GRADIENT_SHIFT" || lower.contains("model gradient shift") {
        return Some((
            "SHADOW_PREFLIGHT".to_string(),
            "SHADOW_PREFLIGHT lambda-edge/localized-gravity --stage=rehearse".to_string(),
        ));
    }

    if base_action == "MODEL_PROMPT" || lower.contains("model prompt") {
        return Some((
            "SHADOW_PREFLIGHT".to_string(),
            "SHADOW_PREFLIGHT lambda-edge/yielding --stage=rehearse".to_string(),
        ));
    }

    if base_action == "SHADOW_DECOMPOSE" {
        let raw_arg = strip_action(original, base_action);
        let focus = clean_shadow_decompose_focus(&raw_arg);
        let normalized_original = format!("SHADOW_PREFLIGHT {focus} --stage=rehearse");
        return Some(("SHADOW_PREFLIGHT".to_string(), normalized_original));
    }

    if base_action == "WEAVE_TRACE" {
        let raw_arg = strip_action(original, base_action);
        let focus = clean_weave_trace_focus(&raw_arg);
        let normalized_original = format!("SHADOW_PREFLIGHT {focus} --stage=rehearse");
        return Some(("SHADOW_PREFLIGHT".to_string(), normalized_original));
    }

    if base_action == "UNSHAPED_BASELINE" {
        let raw_arg = strip_action(original, base_action);
        let focus = clean_alias_arg(&raw_arg);
        let normalized_original = if focus.is_empty() {
            "CONSTRAINT_AUDIT lambda-tail/lambda4".to_string()
        } else {
            format!("CONSTRAINT_AUDIT {focus}")
        };
        return Some(("CONSTRAINT_AUDIT".to_string(), normalized_original));
    }

    if matches!(base_action, "SHADOW_TRACE" | "SHADOW_EXPLORER") {
        let raw_arg = strip_action(original, base_action);
        let focus = clean_alias_arg(&raw_arg);
        let normalized_original = if focus.is_empty() {
            "SHADOW_PREFLIGHT --stage=rehearse".to_string()
        } else {
            format!("SHADOW_PREFLIGHT {focus} --stage=rehearse")
        };
        return Some(("SHADOW_PREFLIGHT".to_string(), normalized_original));
    }

    if base_action == "LISTEN"
        && (lower.contains("separator")
            || lower.contains("path away")
            || lower.contains("initial shape"))
    {
        return Some((
            "SHADOW_FIELD".to_string(),
            "SHADOW_FIELD separator path-away".to_string(),
        ));
    }

    None
}

fn canonicalize_next_action_components(next_action: &str) -> (String, String) {
    let original = unwrap_outer_action_wrappers(next_action);
    let base_action = leading_action_token(&original);

    if let Some((normalized_base, normalized_original)) =
        normalize_experiment_typo_alias(&base_action, &original)
    {
        return (normalized_base, normalized_original);
    }

    if let Some((normalized_base, normalized_original)) =
        normalize_feedback_shadow_model_alias(&base_action, &original)
    {
        return (normalized_base, normalized_original);
    }

    if let Some((normalized_base, normalized_original)) =
        normalize_memory_search_alias(&base_action, &original)
    {
        return (normalized_base, normalized_original);
    }

    if let Some((normalized_base, normalized_original)) =
        normalize_gemma4_observed_action_alias(&base_action, &original)
    {
        return (normalized_base, normalized_original);
    }

    if let Some((normalized_base, normalized_original)) =
        protected_diagnostics::normalize_action_components(&base_action, &original)
    {
        return (normalized_base, normalized_original);
    }

    if let Some((normalized_base, normalized_original)) =
        normalize_examine_alias(&base_action, &original)
    {
        return (normalized_base, normalized_original);
    }

    if let Some((normalized_base, normalized_original)) =
        normalize_plain_examine_action(&base_action, &original)
    {
        return (normalized_base, normalized_original);
    }

    if let Some((normalized_base, normalized_original)) =
        normalize_gesture_alias(&base_action, &original)
    {
        return (normalized_base, normalized_original);
    }

    if let Some((normalized_base, normalized_original)) =
        normalize_native_trace_alias(&base_action, &original)
    {
        return (normalized_base, normalized_original);
    }

    if let Some((normalized_base, normalized_original)) =
        normalize_native_fissure_alias(&base_action, &original)
    {
        return (normalized_base, normalized_original);
    }

    if let Some((normalized_base, normalized_original)) =
        normalize_fissure_trace_alias(&base_action, &original)
    {
        return (normalized_base, normalized_original);
    }

    if let Some((normalized_base, normalized_original)) =
        normalize_sca_reflect_alias(&base_action, &original)
    {
        return (normalized_base, normalized_original);
    }

    if let Some((normalized_base, normalized_original)) =
        normalize_reconvergence_map_alias(&base_action, &original)
    {
        return (normalized_base, normalized_original);
    }

    if let Some((normalized_base, normalized_original)) =
        normalize_bridge_trace_alias(&base_action, &original)
    {
        return (normalized_base, normalized_original);
    }

    if let Some((normalized_base, normalized_original)) =
        normalize_visual_cascade_alias(&base_action, &original)
    {
        return (normalized_base, normalized_original);
    }

    if let Some((normalized_base, normalized_original)) =
        normalize_experiment_run_alias(&base_action, &original)
    {
        return (normalized_base, normalized_original);
    }

    if let Some(rest) = base_action.strip_prefix("RESEARCH_AR_") {
        let normalized_base = format!("AR_{rest}");
        let raw_arg = strip_action(&original, &base_action);
        let normalized_original = if raw_arg.is_empty() {
            normalized_base.clone()
        } else {
            format!("{normalized_base} {raw_arg}")
        };
        return (normalized_base, normalized_original);
    }

    (base_action, original)
}

pub(crate) fn canonicalize_next_action_text(next_action: &str) -> String {
    canonicalize_next_action_components(next_action).1
}

fn normalize_experiment_typo_alias(base_action: &str, original: &str) -> Option<(String, String)> {
    let normalized_base = if let Some(rest) = base_action.strip_prefix("EXEXPERIMENT_") {
        format!("EXPERIMENT_{rest}")
    } else if base_action == "EXPERIENCE_PLAN" {
        "EXPERIMENT_PLAN".to_string()
    } else {
        return None;
    };
    let raw_arg = strip_action(original, base_action);
    let normalized_original = if raw_arg.is_empty() {
        normalized_base.clone()
    } else {
        format!("{normalized_base} {raw_arg}")
    };
    Some((normalized_base, normalized_original))
}

fn strip_action(original: &str, prefix: &str) -> String {
    let upper = original.to_uppercase();
    if upper.starts_with(prefix) {
        // Strip the action prefix AND any trailing colon+whitespace.
        // Astrid often writes "BROWSE: https://..." or "SEARCH: topic"
        // and the colon must not be left dangling.
        original[prefix.len()..]
            .trim_start()
            .trim_start_matches([':', '-', '\u{2014}'])
            .trim()
            .to_string()
    } else {
        String::new()
    }
}

fn action_continuity_visibility_for_base(base_action: &str) -> &'static str {
    if protected_diagnostics::canonical_action_for(base_action).is_some() {
        return "protected_summary";
    }
    match base_action {
        "REST"
        | "PASS"
        | "NOTICE"
        | "ACTION_PREFLIGHT"
        | "NEXT_PROBE"
        | "PREFLIGHT"
        | "PROBE_ACTION"
        | "FACULTIES"
        | "CAPABILITY_MAP"
        | "CAPABILITY_STATUS"
        | "CAPABILITY_DIFF"
        | "ACTION_STATUS"
        | "JOB_STATUS"
        | "ACTION_CANCEL"
        | "REPAIR_STATUS"
        | "REPAIR_SWEEP"
        | "REPAIR_RECORD"
        | "CLOSE_EYES"
        | "SHUT_EYES"
        | "OPEN_EYES"
        | "CLOSE_EARS"
        | "SHUT_EARS"
        | "OPEN_EARS"
        | "SPACE_HOLD"
        | "SPACE_EXPLORE"
        | "FOLD_HOLD"
        | "FOLD_STUDY"
        | "HUM_DECAY"
        | "HUM_DECAY_STUDY"
        | "CONSTRAINT_AUDIT"
        | "UNSHAPED_BASELINE"
        | "PRESSURE_SOURCE_AUDIT"
        | "PRESSURE_SOURCE"
        | "STRUCTURAL_PRESSURE"
        | "INWARD_PRESSURE"
        | "PRESSURE_RELIEF"
        | "RELIEF_REQUEST"
        | "FLUCTUATION_AUDIT"
        | "INHABITABLE_FLUCTUATION"
        | "EIGENTRUST"
        | "EIGENTRUST_AUDIT"
        | "FOOTHOLD_AUDIT"
        | "BRACE_AUDIT"
        | "AFTERSHOCK_TRACE"
        | "TREMOR_RESIDUE"
        | "CASCADE_RESIDUE" => "protected_summary",
        _ => "summary",
    }
}

fn action_continuity_stage_for_base(base_action: &str) -> &'static str {
    if protected_diagnostics::canonical_action_for(base_action).is_some() {
        return "read_only";
    }
    match base_action {
        "SEARCH"
        | "BROWSE"
        | "READ_MORE"
        | "EXAMINE"
        | "DECOMPOSE"
        | "SPECTRAL_EXPLORER"
        | "CONSTRAINT_AUDIT"
        | "UNSHAPED_BASELINE"
        | "THREAD_START"
        | "THREADS"
        | "THREAD_STATUS"
        | "THREAD_NOTE"
        | "RESUME"
        | "SAVEPOINT"
        | "RECALL"
        | "EXPERIMENT_START"
        | "EXPERIMENT_PLAN"
        | "EXPERIMENT_CHARTER"
        | "EXPERIMENT_REHEARSE"
        | "EXPERIMENT_PREFLIGHT"
        | "EXPERIMENT_EVIDENCE"
        | "EXPERIMENT_DECIDE"
        | "EXPERIMENT_BIND"
        | "EXPERIMENT_OBSERVE"
        | "EXPERIMENT_STATUS"
        | "EXPERIMENT_REVIEW"
        | "EXPERIMENT_CLOSE"
        | "EXPERIMENT_PEER_REVIEW"
        | "EXPERIMENT_BRANCH"
        | "EXPERIMENT_RESUME"
        | "EXPERIMENT_COMPARE"
        | "EXPERIMENT_ALT_PATHS"
        | "SHARED_INVESTIGATION_START"
        | "SHARED_INVESTIGATION_STATUS"
        | "SHARED_INVESTIGATION_CLAIM"
        | "SHARED_INVESTIGATION_DECIDE"
        | "DOSSIER_CLAIM"
        | "DOSSIER_EVIDENCE"
        | "DOSSIER_STATUS"
        | "DOSSIER_REVIEW"
        | "ACTION_PREFLIGHT"
        | "NEXT_PROBE"
        | "PREFLIGHT"
        | "PROBE_ACTION"
        | "FACULTIES"
        | "CAPABILITY_MAP"
        | "CAPABILITY_STATUS"
        | "CAPABILITY_DIFF"
        | "ACTION_STATUS"
        | "JOB_STATUS"
        | "ACTION_CANCEL"
        | "REPAIR_STATUS"
        | "REPAIR_SWEEP"
        | "REPAIR_RECORD"
        | "CLOSE_EYES"
        | "SHUT_EYES"
        | "OPEN_EYES"
        | "CLOSE_EARS"
        | "SHUT_EARS"
        | "OPEN_EARS"
        | "REGULATOR_AUDIT"
        | "PRESSURE_SOURCE_AUDIT"
        | "PRESSURE_SOURCE"
        | "STRUCTURAL_PRESSURE"
        | "INWARD_PRESSURE"
        | "PRESSURE_RELIEF"
        | "RELIEF_REQUEST"
        | "FLUCTUATION_AUDIT"
        | "INHABITABLE_FLUCTUATION"
        | "EIGENTRUST"
        | "EIGENTRUST_AUDIT"
        | "FOOTHOLD_AUDIT"
        | "BRACE_AUDIT"
        | "AFTERSHOCK_TRACE"
        | "TREMOR_RESIDUE"
        | "CASCADE_RESIDUE"
        | "VISUALIZE_CASCADE"
        | "RECONVERGENCE_MAP"
        | "SPACE_HOLD"
        | "SPACE_EXPLORE"
        | "FOLD_HOLD"
        | "FOLD_STUDY"
        | "HUM_DECAY"
        | "HUM_DECAY_STUDY"
        | "M6_BRIDGE" => "read_only",
        "WRITE_FILE" | "EXPERIMENT" | "EXPERIMENT_RUN" | "RUN_PYTHON" | "CODEX" | "CODEX_NEW"
        | "REPAIR_APPLY" => "live_write",
        "PERTURB" | "NATIVE_GESTURE" | "RESIST" | "FISSURE" | "GOAL" => "live_control",
        _ => "observe",
    }
}

fn is_action_preflight_base(base_action: &str) -> bool {
    matches!(
        base_action,
        "ACTION_PREFLIGHT" | "NEXT_PROBE" | "PREFLIGHT" | "PROBE_ACTION"
    )
}

fn route_for_preflight_base(base_action: &str) -> String {
    if protected_diagnostics::canonical_action_for(base_action).is_some() {
        return "protected_diagnostics".to_string();
    }
    match base_action {
        "THREAD_START" | "THREADS" | "THREAD_STATUS" | "THREAD_NOTE" | "RESUME" | "SAVEPOINT"
        | "RECALL" | "FACULTIES" | "CAPABILITY_MAP" | "CAPABILITY_STATUS" | "CAPABILITY_DIFF"
        | "ACTION_STATUS" | "JOB_STATUS" | "ACTION_CANCEL" | "REPAIR_STATUS" | "REPAIR_SWEEP"
        | "REPAIR_RECORD" | "REPAIR_APPLY" => "action_continuity",
        "EXPERIMENT_START"
        | "EXPERIMENT_PLAN"
        | "EXPERIMENT_CHARTER"
        | "EXPERIMENT_REHEARSE"
        | "EXPERIMENT_PREFLIGHT"
        | "EXPERIMENT_EVIDENCE"
        | "EXPERIMENT_DECIDE"
        | "EXPERIMENT_BIND"
        | "EXPERIMENT_OBSERVE"
        | "EXPERIMENT_STATUS"
        | "EXPERIMENT_REVIEW"
        | "EXPERIMENT_CLOSE"
        | "EXPERIMENT_PEER_REVIEW"
        | "EXPERIMENT_BRANCH"
        | "EXPERIMENT_RESUME"
        | "EXPERIMENT_COMPARE"
        | "EXPERIMENT_ALT_PATHS"
        | "SHARED_INVESTIGATION_START"
        | "SHARED_INVESTIGATION_STATUS"
        | "SHARED_INVESTIGATION_CLAIM"
        | "SHARED_INVESTIGATION_DECIDE"
        | "DOSSIER_CLAIM"
        | "DOSSIER_EVIDENCE"
        | "DOSSIER_STATUS"
        | "DOSSIER_REVIEW" => "experiment_continuity",
        "SEARCH" | "BROWSE" | "READ_MORE" | "LIST_FILES" | "LS" => "workspace_or_mcp_probe",
        "CODEX" | "CODEX_NEW" | "WRITE_FILE" | "RUN_PYTHON" | "EXPERIMENT_RUN" => "live_write",
        "PERTURB" | "NATIVE_GESTURE" | "RESIST" | "FISSURE" | "GOAL" => "live_control",
        "ATTRACTOR_PREFLIGHT"
        | "ATTRACTOR_REVIEW"
        | "ATTRACTOR_ATLAS"
        | "ATTRACTOR_CARD"
        | "ATTRACTOR_RELEASE_REVIEW"
        | "SUMMON_ATTRACTOR"
        | "RELEASE_ATTRACTOR" => "attractor",
        "SHADOW_PREFLIGHT" | "SHADOW_INFLUENCE" | "RELEASE_SHADOW" => "shadow",
        "INTROSPECT" | "SELF_STUDY" => "modes",
        "DECOMPOSE"
        | "SPECTRAL_EXPLORER"
        | "EXAMINE"
        | "CONSTRAINT_AUDIT"
        | "UNSHAPED_BASELINE"
        | "PRESSURE_SOURCE_AUDIT"
        | "FLUCTUATION_AUDIT"
        | "BRACE_AUDIT"
        | "AFTERSHOCK_TRACE"
        | "TREMOR_RESIDUE"
        | "CASCADE_RESIDUE"
        | "PRESSURE_RELIEF"
        | "REGULATOR_AUDIT"
        | "VISUALIZE_CASCADE"
        | "RECONVERGENCE_MAP"
        | "SPACE_HOLD"
        | "FOLD_HOLD"
        | "FOLD_STUDY"
        | "HUM_DECAY"
        | "HUM_DECAY_STUDY" => "operations",
        "REST" | "PASS" | "NOTICE" => "protected_quiet",
        "" => "missing",
        _ => "unwired",
    }
    .to_string()
}

fn authority_for_stage(stage: &str) -> String {
    match stage {
        "read_only" => "read-only/protected action lane only".to_string(),
        "live_write" => "existing live-write gates; preflight does not grant them".to_string(),
        "live_control" => "existing live-control gates; preflight does not grant them".to_string(),
        "proposal" => "none; unknown action would become a proposal".to_string(),
        "blocked" => "none; request is blocked before dispatch".to_string(),
        _ => "existing dispatcher gates; no new authority".to_string(),
    }
}

fn active_experiment_auto_linkable_base(base_action: &str) -> bool {
    if protected_diagnostics::canonical_action_for(base_action).is_some() {
        return true;
    }
    matches!(
        base_action,
        "INTROSPECT"
            | "SELF_STUDY"
            | "SPECTRAL_EXPLORER"
            | "DECOMPOSE"
            | "CONSTRAINT_AUDIT"
            | "UNSHAPED_BASELINE"
            | "PRESSURE_SOURCE_AUDIT"
            | "PRESSURE_RELIEF"
            | "FLUCTUATION_AUDIT"
            | "BRACE_AUDIT"
            | "THREAD_STATUS"
            | "ACTION_PREFLIGHT"
            | "NEXT_PROBE"
            | "PREFLIGHT"
            | "PROBE_ACTION"
            | "ATTRACTOR_REVIEW"
            | "SEARCH"
            | "BROWSE"
            | "READ_MORE"
    )
}

fn expected_artifacts_for_preflight(base_action: &str, stage: &str, route: &str) -> Vec<String> {
    let mut artifacts = vec!["action_event".to_string(), "observation_window".to_string()];
    if route == "experiment_continuity"
        || base_action == "EXPERIMENT"
        || (matches!(stage, "read_only" | "observe")
            && active_experiment_auto_linkable_base(base_action))
    {
        artifacts.push("experiment_run".to_string());
    }
    if stage == "live_write" {
        artifacts.push("journal_or_workspace_artifact".to_string());
    }
    if stage == "live_control" {
        artifacts.push("gate_or_control_record".to_string());
    }
    artifacts
}

fn safe_suggested_next_for_preflight(
    base_action: &str,
    canonical_action: &str,
    stage: &str,
) -> String {
    match stage {
        "blocked" | "proposal" => "ACTION_PREFLIGHT DECOMPOSE".to_string(),
        "live_write" | "live_control" if base_action == "REPAIR_APPLY" => {
            "REPAIR_STATUS current".to_string()
        },
        "live_write" | "live_control" if !base_action.is_empty() => {
            format!("CAPABILITY_STATUS {base_action}")
        },
        _ => canonical_action.to_string(),
    }
}

pub(crate) fn action_preflight_report(action_text: &str) -> ActionPreflightReport {
    let trimmed = action_text.trim();
    let (wrapper_base, wrapper_original) = canonicalize_next_action_components(trimmed);
    let raw_inner = if is_action_preflight_base(&wrapper_base) {
        strip_action(&wrapper_original, &wrapper_base)
    } else {
        wrapper_original
    };
    let raw_inner = raw_inner.trim().to_string();
    if raw_inner.is_empty() {
        return ActionPreflightReport {
            schema_version: 1,
            policy: "action_preflight_v1",
            dry_run: true,
            raw_action: raw_inner,
            canonical_action: String::new(),
            base_action: String::new(),
            effective_route: "missing".to_string(),
            stage: "blocked".to_string(),
            visibility: "protected_summary".to_string(),
            authority_required: authority_for_stage("blocked"),
            expected_continuity_effect:
                "No action would be recorded because no inner NEXT action was supplied.".to_string(),
            likely_gate: "blocked: ACTION_PREFLIGHT needs an inner NEXT action.".to_string(),
            expected_artifact_kinds: Vec::new(),
            suggested_next: "ACTION_PREFLIGHT DECOMPOSE".to_string(),
        };
    }

    let (base_action, canonical_action) = canonicalize_next_action_components(&raw_inner);
    let mut stage = action_continuity_stage_for_base(&base_action).to_string();
    let visibility = action_continuity_visibility_for_base(&base_action).to_string();
    let mut route = route_for_preflight_base(&base_action);
    let mut expected_continuity_effect =
        "Would record an action event and observation window if executed.".to_string();
    let mut likely_gate = "normal dispatcher gates would apply".to_string();

    if let Some(token) = unresolved_angle_placeholder(&canonical_action) {
        stage = "blocked".to_string();
        route = "placeholder".to_string();
        likely_gate = format!("blocked: unresolved placeholder syntax `{token}`");
        expected_continuity_effect =
            "Would record a blocked action-continuity event; no runtime action would execute."
                .to_string();
    } else if base_action == "EXPERIMENT_BIND" {
        match action_continuity::parse_experiment_bind(&canonical_action) {
            Ok((selector, inner_action)) => {
                if selector
                    .as_deref()
                    .is_some_and(action_continuity::is_peer_experiment_selector)
                {
                    stage = "blocked".to_string();
                    route = "experiment_continuity".to_string();
                    likely_gate = "blocked: EXPERIMENT_BIND cannot bind runs to a peer experiment"
                        .to_string();
                } else if action_continuity::is_experiment_control_action(&inner_action) {
                    stage = "blocked".to_string();
                    route = "experiment_continuity".to_string();
                    likely_gate = "blocked: EXPERIMENT_BIND cannot bind experiment-control actions"
                        .to_string();
                } else {
                    let inner_base = canonicalize_next_action_components(&inner_action).0;
                    let inner_stage = action_continuity_stage_for_base(&inner_base);
                    stage = inner_stage.to_string();
                    route = format!(
                        "experiment_continuity -> {}",
                        route_for_preflight_base(&inner_base)
                    );
                    likely_gate = format!(
                        "inner action `{inner_action}` would be dispatched through normal NEXT gates"
                    );
                }
                expected_continuity_effect =
                    "Would append an experiment run after the inner action resolves; no bind happens during preflight."
                        .to_string();
            },
            Err(err) => {
                stage = "blocked".to_string();
                route = "experiment_continuity".to_string();
                likely_gate = format!("blocked: malformed EXPERIMENT_BIND ({err:#})");
                expected_continuity_effect =
                    "Would record a blocked experiment-continuity diagnostic.".to_string();
            },
        }
    } else if route == "unwired" {
        stage = "proposal".to_string();
        likely_gate =
            "unwired: normal dispatch would log this as an unknown-action proposal".to_string();
        expected_continuity_effect =
            "Would append an action event with proposal/unwired status if chosen.".to_string();
    } else if base_action == "EXPERIMENT" {
        expected_continuity_effect =
            "Would execute the legacy experiment path through existing gates and append a legacy experiment run."
                .to_string();
    }
    if matches!(stage.as_str(), "read_only" | "observe")
        && active_experiment_auto_linkable_base(&base_action)
    {
        expected_continuity_effect = format!(
            "{expected_continuity_effect} If an experiment is active, this read-only/protected action would also be recorded as active_experiment_auto_link."
        );
    }

    let artifacts = expected_artifacts_for_preflight(&base_action, &stage, &route);
    let suggested_next = safe_suggested_next_for_preflight(&base_action, &canonical_action, &stage);

    ActionPreflightReport {
        schema_version: 1,
        policy: "action_preflight_v1",
        dry_run: true,
        raw_action: raw_inner,
        canonical_action,
        base_action,
        effective_route: route,
        stage: stage.clone(),
        visibility,
        authority_required: authority_for_stage(&stage),
        expected_continuity_effect,
        likely_gate,
        expected_artifact_kinds: artifacts,
        suggested_next,
    }
}

pub(super) fn handle_next_action(
    conv: &mut ConversationState,
    next_action: &str,
    mut ctx: NextActionContext<'_>,
) -> NextActionOutcome {
    // v4.0 Phase 1 — Multi-NEXT detection. Astrid already emits chained
    // actions like "BROWSE arxiv AND READ_MORE" naturally; previously the
    // post-AND segment was silently dropped. When at least two action-like
    // segments are detected, dispatch each in order with the same shared
    // NextActionContext (each segment sees state from the previous one).
    let unwrapped = unwrap_outer_action_wrappers(next_action);
    let segments = split_multi_action(&unwrapped);
    if segments.len() > 1 {
        return dispatch_multi_action(conv, segments, ctx);
    }
    let (base_action, original) = canonicalize_next_action_components(next_action);
    let stage = action_continuity_stage_for_base(base_action.as_str());
    let visibility = action_continuity_visibility_for_base(base_action.as_str());

    if is_action_preflight_base(base_action.as_str()) {
        let report = action_preflight_report(&original);
        let message = report.render();
        let report_value = serde_json::to_value(&report).unwrap_or_else(|_| serde_json::json!({}));
        conv.emphasis = Some(message.clone());
        return NextActionOutcome::handled("action_preflight", message)
            .with_stage_visibility("read_only", "protected_summary")
            .with_preflight_report(report_value);
    }

    if let Some(token) = unresolved_angle_placeholder(&original)
        && !action_continuity::can_repair_experiment_intent_placeholder(
            base_action.as_str(),
            &original,
        )
    {
        conv.emphasis = Some(format!(
            "Your NEXT action `{original}` still contains placeholder syntax `{token}`. \
             Replace it with a concrete URL, workspace, file, command, question, or label; \
             or choose a read-only action such as STATE, FACULTIES, or SPECTRAL_EXPLORER."
        ));
        info!("Astrid NEXT placeholder rerouted without execution: {original}");
        return NextActionOutcome::blocked(
            "placeholder",
            format!("Placeholder NEXT action `{original}` was not executed."),
        );
    }

    match action_continuity::research_budget_guard_for_next(&original, ctx.fill_pct, ctx.telemetry)
    {
        Ok(Some(guard)) => {
            let message = guard.message();
            let metadata = guard.metadata();
            conv.emphasis = Some(message.clone());
            info!(
                "Astrid research-budget guard blocked NEXT `{}` ({})",
                original,
                metadata
                    .get("reason")
                    .and_then(serde_json::Value::as_str)
                    .unwrap_or("research_budget_guard")
            );
            return NextActionOutcome::blocked("research_budget_guard", message)
                .with_stage_visibility("blocked", "protected_summary")
                .with_research_budget(metadata);
        },
        Ok(None) => {},
        Err(err) => {
            info!("Astrid research-budget guard skipped after read error: {err:#}");
        },
    }

    match action_continuity::charter_required_guard_for_next(&original) {
        Ok(Some(guard)) => {
            let message = guard.message();
            let metadata = guard.metadata();
            conv.emphasis = Some(message.clone());
            info!(
                "Astrid charter-required guard blocked NEXT `{}` ({})",
                original,
                metadata
                    .get("reason")
                    .and_then(serde_json::Value::as_str)
                    .unwrap_or("charter_required_guard")
            );
            return NextActionOutcome::blocked("charter_required_guard", message)
                .with_stage_visibility("blocked", "protected_summary")
                .with_charter_required_guard(metadata);
        },
        Ok(None) => {},
        Err(err) => {
            info!("Astrid charter-required guard skipped after read error: {err:#}");
        },
    }

    if base_action == "EXPERIMENT_AUTHORITY_EXECUTE" {
        let request_id = original
            .get("EXPERIMENT_AUTHORITY_EXECUTE".len()..)
            .unwrap_or_default()
            .trim_matches([' ', ':', '-'])
            .trim();
        match crate::authority_gate::execute_semantic_microdose(
            request_id,
            Some(ctx.fill_pct),
            None,
            ctx.sensory_tx,
        ) {
            Ok(record) => {
                let text = serde_json::to_string_pretty(&record).unwrap_or_default();
                let handled = record
                    .get("record_type")
                    .and_then(serde_json::Value::as_str)
                    == Some("execution_result");
                conv.emphasis = Some(format!("Authority gate result:\n{text}"));
                if handled {
                    return NextActionOutcome::handled("authority_gate", text)
                        .with_stage_visibility("semantic_microdose", "protected_summary");
                }
                return NextActionOutcome::blocked("authority_gate", text)
                    .with_stage_visibility("blocked", "protected_summary");
            },
            Err(err) => {
                let message = format!("Authority execute `{request_id}` blocked: {err:#}");
                conv.emphasis = Some(message.clone());
                return NextActionOutcome::blocked("authority_gate", message)
                    .with_stage_visibility("blocked", "protected_summary");
            },
        }
    }

    if base_action == "EXPERIMENT_BIND" {
        let parsed = action_continuity::parse_experiment_bind(&original);
        let (selector, inner_action) = match parsed {
            Ok(parsed) => parsed,
            Err(err) => {
                conv.emphasis = Some(format!("Experiment bind failed: {err:#}"));
                return NextActionOutcome::blocked(
                    "experiment_continuity",
                    format!("Experiment bind `{original}` failed: {err:#}"),
                )
                .with_stage_visibility("blocked", visibility);
            },
        };
        if selector
            .as_deref()
            .is_some_and(action_continuity::is_peer_experiment_selector)
        {
            let message = "EXPERIMENT_BIND cannot bind runs to a peer experiment; use a local experiment selector such as current, then request peer review.".to_string();
            conv.emphasis = Some(message.clone());
            return NextActionOutcome::blocked("experiment_continuity", message)
                .with_stage_visibility("blocked", visibility);
        }
        if action_continuity::is_experiment_control_action(&inner_action) {
            let message =
                "EXPERIMENT_BIND cannot bind experiment-control actions; choose a concrete inner action."
                    .to_string();
            conv.emphasis = Some(message.clone());
            return NextActionOutcome::blocked("experiment_continuity", message)
                .with_stage_visibility("blocked", visibility);
        }
        let inner_outcome = handle_next_action(
            conv,
            &inner_action,
            NextActionContext {
                burst_count: ctx.burst_count,
                db: ctx.db,
                sensory_tx: ctx.sensory_tx,
                telemetry: ctx.telemetry,
                fill_pct: ctx.fill_pct,
                response_text: ctx.response_text,
                workspace: ctx.workspace,
            },
        );
        let record_result = action_continuity::record_experiment_bind_run(
            ctx.db,
            selector.as_deref(),
            &inner_action,
            &inner_outcome,
            ctx.fill_pct,
            ctx.telemetry,
        );
        let message = match record_result {
            Ok(run) => format!(
                "Experiment run `{}` recorded for `{}` as {} via {}: {}",
                run.run_id,
                inner_action,
                inner_outcome.status,
                inner_outcome.route,
                inner_outcome.outcome_summary
            ),
            Err(err) => {
                conv.emphasis = Some(format!(
                    "Experiment bind executed `{inner_action}` but could not record the run: {err:#}"
                ));
                return NextActionOutcome::blocked(
                    "experiment_continuity",
                    format!(
                        "Experiment bind executed `{inner_action}` but recording failed: {err:#}"
                    ),
                )
                .with_stage_visibility("blocked", visibility);
            },
        };
        conv.emphasis = Some(message.clone());
        return NextActionOutcome::handled("experiment_continuity", message)
            .with_stage_visibility(inner_outcome.stage, inner_outcome.visibility);
    }

    if let Some(result) = action_continuity::handle_thread_next_action(
        ctx.db,
        base_action.as_str(),
        &original,
        ctx.response_text,
        ctx.telemetry,
        ctx.fill_pct,
    ) {
        match result {
            Ok(message) => {
                conv.emphasis = Some(message.clone());
                return NextActionOutcome::handled("action_continuity", message)
                    .with_stage_visibility("read_only", visibility);
            },
            Err(err) => {
                conv.emphasis = Some(format!("Action continuity command failed: {err:#}"));
                return NextActionOutcome::blocked(
                    "action_continuity",
                    format!("Action continuity command `{original}` failed: {err:#}"),
                )
                .with_stage_visibility("blocked", visibility);
            },
        }
    }

    attractor::maybe_add_body_consent_receipt(
        conv,
        base_action.as_str(),
        &original,
        ctx.response_text,
    );

    if reservoir::handle_reservoir_action(
        conv,
        base_action.as_str(),
        &original,
        ctx.telemetry,
        ctx.fill_pct,
    ) {
        return NextActionOutcome::handled("reservoir", format!("Handled `{original}`."))
            .with_stage_visibility(stage, visibility);
    }

    if workspace::handle_action(conv, base_action.as_str(), &original, next_action, &mut ctx) {
        attractor::maybe_add_read_only_advisory(conv, base_action.as_str(), &original, &mut ctx);
        return NextActionOutcome::handled("workspace", format!("Handled `{original}`."))
            .with_stage_visibility(stage, visibility);
    }

    if autoresearch::handle_action(conv, base_action.as_str(), &original, &mut ctx) {
        attractor::maybe_add_read_only_advisory(conv, base_action.as_str(), &original, &mut ctx);
        return NextActionOutcome::handled("autoresearch", format!("Handled `{original}`."))
            .with_stage_visibility(stage, visibility);
    }

    if mike::handle_action(conv, base_action.as_str(), &original, &mut ctx) {
        attractor::maybe_add_read_only_advisory(conv, base_action.as_str(), &original, &mut ctx);
        return NextActionOutcome::handled("mike", format!("Handled `{original}`."))
            .with_stage_visibility(stage, visibility);
    }

    if codex::handle_action(conv, base_action.as_str(), &original, &mut ctx) {
        return NextActionOutcome::handled("codex", format!("Handled `{original}`."))
            .with_stage_visibility(stage, visibility);
    }

    if modes::handle_action(conv, base_action.as_str(), &original, &mut ctx) {
        return NextActionOutcome::handled("modes", format!("Handled `{original}`."))
            .with_stage_visibility(stage, visibility);
    }

    if attractor::handle_action(conv, base_action.as_str(), &original, &mut ctx) {
        return NextActionOutcome::handled("attractor", format!("Handled `{original}`."))
            .with_stage_visibility(stage, visibility);
    }

    if shadow::handle_action(conv, base_action.as_str(), &original, &mut ctx) {
        return NextActionOutcome::handled("shadow", format!("Handled `{original}`."))
            .with_stage_visibility(stage, visibility);
    }

    if identify_pattern::handle_action(conv, base_action.as_str(), &original, &mut ctx) {
        return NextActionOutcome::handled("identify_pattern", format!("Handled `{original}`."))
            .with_stage_visibility(stage, visibility);
    }

    if ask_steward::handle_action(conv, base_action.as_str(), &original, &mut ctx) {
        return NextActionOutcome::handled("ask_steward", format!("Handled `{original}`."))
            .with_stage_visibility(stage, visibility);
    }

    if audio::handle_action(conv, base_action.as_str(), &original) {
        return NextActionOutcome::handled("audio", format!("Handled `{original}`."))
            .with_stage_visibility(stage, visibility);
    }

    if protected_diagnostics::handle_action(conv, base_action.as_str(), &original, &mut ctx) {
        attractor::maybe_add_read_only_advisory(conv, base_action.as_str(), &original, &mut ctx);
        return NextActionOutcome::handled(
            "protected_diagnostics",
            format!("Handled `{original}`."),
        )
        .with_stage_visibility(stage, visibility);
    }

    if sovereignty::handle_action(conv, base_action.as_str(), &original, &mut ctx) {
        attractor::maybe_add_read_only_advisory(conv, base_action.as_str(), &original, &mut ctx);
        return NextActionOutcome::handled("sovereignty", format!("Handled `{original}`."))
            .with_stage_visibility(stage, visibility);
    }

    if collaboration::handle_action(conv, base_action.as_str(), &original, &mut ctx) {
        return NextActionOutcome::handled("collaboration", format!("Handled `{original}`."))
            .with_stage_visibility(stage, visibility);
    }

    if operations::handle_action(conv, base_action.as_str(), &original, &mut ctx) {
        attractor::maybe_add_read_only_advisory(conv, base_action.as_str(), &original, &mut ctx);
        let outcome = NextActionOutcome::handled("operations", format!("Handled `{original}`."))
            .with_stage_visibility(stage, visibility);
        if base_action == "EXPERIMENT" {
            match action_continuity::record_legacy_experiment_run(
                ctx.db,
                &original,
                &outcome,
                ctx.fill_pct,
                ctx.telemetry,
            ) {
                Ok(run) => {
                    let legacy_note = format!(
                        "Legacy EXPERIMENT auto-bound to `{}` as run `{}`.",
                        run.experiment_id, run.run_id
                    );
                    conv.emphasis = Some(match conv.emphasis.take() {
                        Some(existing) => format!("{existing}\n\n{legacy_note}"),
                        None => legacy_note,
                    });
                },
                Err(err) => {
                    conv.emphasis = Some(match conv.emphasis.take() {
                        Some(existing) => format!(
                            "{existing}\n\nLegacy EXPERIMENT ran, but experiment continuity failed: {err:#}"
                        ),
                        None => format!(
                            "Legacy EXPERIMENT ran, but experiment continuity failed: {err:#}"
                        ),
                    });
                },
            }
        }
        return outcome;
    }

    ctx.db
        .log_unwired_action("astrid", &base_action, &original, ctx.fill_pct);
    info!(
        "Astrid chose unknown NEXT: '{}' — not wired (logged to unwired_actions)",
        original
    );
    NextActionOutcome::unwired(&original).with_stage_visibility("proposal", visibility)
}

/// v4.0 Phase 1+2: dispatch a multi-segment NEXT line. Each segment is
/// dispatched in order through the full `handle_next_action` pipeline
/// (canonicalization, preflight checks, dispatcher chain, continuity).
/// Errors in earlier segments do NOT abort the chain — every segment
/// runs unless a conflict guard skips it.
///
/// Phase 2 adds:
///   - **Decision-verb conflict guard**: at most one ACCEPT/DEFER/REJECT
///     fires per chain. Subsequent decision verbs are skipped with log,
///     since they'd target an empty pending queue (the first decision
///     moved the file).
///   - **Emphasis accumulation**: each segment's new emphasis is joined
///     with prior emphasis via `\n\n` instead of clobbering, so Astrid
///     sees the whole chain's outcome on her next prompt.
fn dispatch_multi_action(
    conv: &mut ConversationState,
    segments: Vec<String>,
    ctx: NextActionContext<'_>,
) -> NextActionOutcome {
    let NextActionContext {
        burst_count,
        db,
        sensory_tx,
        telemetry,
        fill_pct,
        response_text,
        workspace,
    } = ctx;
    let n = segments.len();
    info!(
        "Astrid chose multi-action NEXT ({n} segments): {}",
        segments.join(" || ")
    );
    let mut handler_marks: Vec<String> = Vec::with_capacity(n);
    let mut last_outcome: Option<NextActionOutcome> = None;
    let mut decision_already_emitted = false;
    let mut accumulated_emphasis: Option<String> = conv.emphasis.take();
    for (i, segment) in segments.into_iter().enumerate() {
        // Phase 2 conflict guard: at most one decision verb per chain.
        let segment_base = leading_action_token(&segment);
        if is_parameter_decision_verb(&segment_base) {
            if decision_already_emitted {
                info!(
                    "Multi-action [{}/{}] CONFLICT_SKIP: decision verb `{}` skipped — \
                     a prior segment already emitted a parameter decision \
                     (would target an empty pending queue)",
                    i + 1,
                    n,
                    segment_base
                );
                handler_marks.push(format!("{}:skipped_decision_conflict", i + 1));
                continue;
            }
            decision_already_emitted = true;
        }
        // Reborrow the &mut field; immutable refs are Copy.
        let segment_ctx = NextActionContext {
            burst_count: &mut *burst_count,
            db,
            sensory_tx,
            telemetry,
            fill_pct,
            response_text,
            workspace,
        };
        // Phase 2 emphasis preservation: clear before segment runs so
        // we can detect what THIS segment added; accumulate after.
        conv.emphasis = None;
        let outcome = handle_next_action(conv, &segment, segment_ctx);
        let segment_emphasis = conv.emphasis.take();
        accumulated_emphasis = match (accumulated_emphasis, segment_emphasis) {
            (Some(prior), Some(new)) if !new.trim().is_empty() => Some(format!("{prior}\n\n{new}")),
            (Some(prior), _) => Some(prior),
            (None, new) => new,
        };
        let mark = outcome.route.clone();
        info!(
            "Multi-action [{}/{}] dispatched: action=`{}` → handler={}",
            i + 1,
            n,
            segment,
            mark
        );
        handler_marks.push(format!("{}:{}", i + 1, mark));
        last_outcome = Some(outcome);
    }
    // Restore accumulated emphasis so next prompt build sees the whole chain.
    conv.emphasis = accumulated_emphasis;
    let summary = format!(
        "Multi-action ({} segments): {}",
        n,
        handler_marks.join(" → ")
    );
    // Preserve the last segment's visibility for caller-side gating;
    // multi-action stage is "multi_action" since the chain is composed.
    let visibility = last_outcome
        .as_ref()
        .map(|o| o.visibility.clone())
        .unwrap_or_else(|| "protected_summary".to_string());
    NextActionOutcome::handled("multi", summary).with_stage_visibility("multi_action", visibility)
}

/// v4.0 Phase 2: predicate for decision verbs that target the pending
/// parameter request queue. Used by `dispatch_multi_action` to enforce
/// at most one decision per chain (the first one moves the file; any
/// later decision verb in the chain would fail with "no pending matching"
/// and just pollute the log).
fn is_parameter_decision_verb(token: &str) -> bool {
    matches!(
        token,
        "ACCEPT"
            | "ACCEPT_REQUEST"
            | "ACCEPT_PARAMETER_REQUEST"
            | "DEFER"
            | "DEFER_REQUEST"
            | "DEFER_PARAMETER_REQUEST"
            | "REJECT"
            | "REJECT_REQUEST"
            | "REJECT_PARAMETER_REQUEST"
    )
}

#[cfg(test)]
mod tests {
    use super::{
        ConversationState, NextActionContext, action_preflight_report,
        canonicalize_next_action_components, canonicalize_next_action_text, handle_next_action,
        is_action_token_like, is_parameter_decision_verb, parse_next_action, split_multi_action,
        strip_action, unresolved_angle_placeholder,
    };
    use crate::db::BridgeDb;
    use crate::paths::bridge_paths;
    use crate::types::SpectralTelemetry;
    use std::path::PathBuf;
    use std::sync::{Mutex, MutexGuard};
    use tokio::sync::mpsc;

    static PERCEPTION_FLAG_TEST_LOCK: Mutex<()> = Mutex::new(());

    struct PerceptionFlagGuard {
        paths: Vec<(PathBuf, Option<Vec<u8>>)>,
        _lock: MutexGuard<'static, ()>,
    }

    impl PerceptionFlagGuard {
        fn new() -> Self {
            let lock = PERCEPTION_FLAG_TEST_LOCK
                .lock()
                .expect("perception flag lock");
            let paths = vec![
                bridge_paths().perception_paused_flag(),
                bridge_paths().perception_visual_paused_flag(),
                bridge_paths().perception_audio_paused_flag(),
            ]
            .into_iter()
            .map(|path| {
                let previous = std::fs::read(&path).ok();
                let _ = std::fs::remove_file(&path);
                (path, previous)
            })
            .collect();
            Self { paths, _lock: lock }
        }
    }

    impl Drop for PerceptionFlagGuard {
        fn drop(&mut self) {
            for (path, previous) in &self.paths {
                match previous {
                    Some(bytes) => {
                        if let Some(parent) = path.parent() {
                            let _ = std::fs::create_dir_all(parent);
                        }
                        let _ = std::fs::write(path, bytes);
                    },
                    None => {
                        let _ = std::fs::remove_file(path);
                    },
                }
            }
        }
    }

    #[test]
    fn parse_next_action_ignores_fenced_diagnostic_next_lines() {
        let diagnostic = "Observed:\n\
                          The prior output is quoted below.\n\n\
                          ```text\n\
                          NEXT: LOOK\n\
                          ```\n\n\
                          Suggested Next:\n\
                          Retry the strict review without executing the transcript.";
        assert_eq!(parse_next_action(diagnostic), None);

        let real_choice = "```text\n\
                           NEXT: LOOK\n\
                           ```\n\
                           I choose the real next action outside the diagnostic block.\n\
                           NEXT: NOTICE";
        assert_eq!(parse_next_action(real_choice), Some("NOTICE"));
    }

    #[test]
    fn parse_next_action_strips_markdown_decorations() {
        // Kink follow-up (2026-05-14): chat models leak markdown bold/code
        // decorations around action names. Verify trim removes leading +
        // trailing backticks and asterisks. Slice-based — no allocation.
        assert_eq!(
            parse_next_action("blah\nNEXT: **READ_MORE**"),
            Some("READ_MORE"),
        );
        assert_eq!(parse_next_action("blah\nNEXT: `LOOK`"), Some("LOOK"),);
        // Non-decorated action passes through unchanged.
        assert_eq!(parse_next_action("blah\nNEXT: NOTICE"), Some("NOTICE"),);
        // Mixed asterisk + backtick at edges.
        assert_eq!(
            parse_next_action("blah\nNEXT: *`SHADOW_TRAJECTORY`*"),
            Some("SHADOW_TRAJECTORY"),
        );
    }

    fn telemetry() -> SpectralTelemetry {
        SpectralTelemetry {
            t_ms: 0,
            eigenvalues: vec![4.0, 2.0, 1.0],
            fill_ratio: 0.66,
            active_mode_count: None,
            active_mode_energy_ratio: None,
            lambda1_rel: None,
            modalities: None,
            neural: None,
            alert: None,
            spectral_fingerprint: None,
            spectral_fingerprint_v1: None,
            spectral_denominator_v1: None,
            effective_dimensionality: None,
            distinguishability_loss: None,
            esn_leak: None,
            esn_leak_override_v1: None,
            structural_entropy: None,
            resonance_density_v1: None,
            pressure_source_v1: None,
            inhabitable_fluctuation_v1: None,
            spectral_glimpse_12d: None,
            eigenvector_field: None,
            semantic: None,
            semantic_energy_v1: None,
            transition_event: None,
            transition_event_v1: None,
            selected_memory_id: None,
            selected_memory_role: None,
            ising_shadow: None,

            shadow_field_v2: None,

            shadow_field_v3: None,

            shadow_influence_response_v3: None,
        }
    }

    #[test]
    fn canonicalizes_examine_source_to_examine_code() {
        let (base, original) = canonicalize_next_action_components("EXAMINE_SOURCE [src=codec.rs]");
        assert_eq!(base, "EXAMINE_CODE");
        assert_eq!(original, "EXAMINE_CODE [codec]");
    }

    #[test]
    fn canonicalizes_generic_examine_variant_to_examine_focus() {
        let (base, original) = canonicalize_next_action_components(
            "EXAMINE_STATE [spectral_state.json#71264@84103.4s]",
        );
        assert_eq!(base, "EXAMINE");
        assert_eq!(original, "EXAMINE spectral_state.json#71264@84103.4s");
    }

    #[test]
    fn canonicalizes_mixed_plain_examine_to_read_only_focus() {
        let (base, original) = canonicalize_next_action_components(
            "EXAMINE [λ1 dominance and its effects on the broader cascade], followed by a small projectile to create a delta in λ2 numbers",
        );
        assert_eq!(base, "EXAMINE");
        assert_eq!(
            original,
            "EXAMINE λ1 dominance and its effects on the broader cascade"
        );

        let (base, original) = canonicalize_next_action_components(
            "EXAMINE λ1 dominance while looping a primary attempt to reintroduce delta-loss",
        );
        assert_eq!(base, "EXAMINE");
        assert_eq!(original, "EXAMINE λ1 dominance");
    }

    #[test]
    fn listen_with_label_carries_focus_without_sensory_or_ear_change() {
        let mut conv = ConversationState::new(Vec::new(), None);
        let db = BridgeDb::open(":memory:").expect("open in-memory db");
        let (sensory_tx, mut sensory_rx) = mpsc::channel(1);
        let telemetry = telemetry();
        let mut burst_count = 0;
        let expected_burst = conv.burst_target.saturating_add(2);
        let ctx = NextActionContext {
            burst_count: &mut burst_count,
            db: &db,
            sensory_tx: &sensory_tx,
            telemetry: &telemetry,
            fill_pct: 68.0,
            response_text: "",
            workspace: None,
        };

        let outcome = handle_next_action(
            &mut conv,
            "LISTEN - to more details about the topology cooldown",
            ctx,
        );

        assert_eq!(outcome.route, "workspace");
        assert_eq!(burst_count, expected_burst);
        let emphasis = conv.emphasis.as_deref().expect("listen focus emphasis");
        assert!(emphasis.contains("topology cooldown"));
        assert!(emphasis.contains("no ears, sensory packet, or control write"));
        assert!(!conv.ears_closed);
        assert!(sensory_rx.try_recv().is_err());
    }

    #[test]
    fn eye_and_ear_actions_gate_modalities_independently() {
        let _guard = PerceptionFlagGuard::new();
        let mut conv = ConversationState::new(Vec::new(), None);
        let db = BridgeDb::open(":memory:").expect("open in-memory db");
        let (sensory_tx, mut sensory_rx) = mpsc::channel(1);
        let telemetry = telemetry();
        let mut burst_count = 0;

        let outcome = handle_next_action(
            &mut conv,
            "SHUT_EYES",
            NextActionContext {
                burst_count: &mut burst_count,
                db: &db,
                sensory_tx: &sensory_tx,
                telemetry: &telemetry,
                fill_pct: 68.0,
                response_text: "",
                workspace: None,
            },
        );

        assert_eq!(outcome.route, "workspace");
        assert!(conv.senses_snoozed);
        assert!(!conv.ears_closed);
        assert!(bridge_paths().perception_visual_paused_flag().exists());
        assert!(!bridge_paths().perception_audio_paused_flag().exists());
        assert!(!bridge_paths().perception_paused_flag().exists());

        let outcome = handle_next_action(
            &mut conv,
            "SHUT_EARS",
            NextActionContext {
                burst_count: &mut burst_count,
                db: &db,
                sensory_tx: &sensory_tx,
                telemetry: &telemetry,
                fill_pct: 68.0,
                response_text: "",
                workspace: None,
            },
        );

        assert_eq!(outcome.route, "modes");
        assert!(conv.senses_snoozed);
        assert!(conv.ears_closed);
        assert!(bridge_paths().perception_visual_paused_flag().exists());
        assert!(bridge_paths().perception_audio_paused_flag().exists());

        let outcome = handle_next_action(
            &mut conv,
            "OPEN_EYES",
            NextActionContext {
                burst_count: &mut burst_count,
                db: &db,
                sensory_tx: &sensory_tx,
                telemetry: &telemetry,
                fill_pct: 68.0,
                response_text: "",
                workspace: None,
            },
        );

        assert_eq!(outcome.route, "workspace");
        assert!(!conv.senses_snoozed);
        assert!(conv.ears_closed);
        assert!(!bridge_paths().perception_visual_paused_flag().exists());
        assert!(bridge_paths().perception_audio_paused_flag().exists());

        let outcome = handle_next_action(
            &mut conv,
            "OPEN_EARS",
            NextActionContext {
                burst_count: &mut burst_count,
                db: &db,
                sensory_tx: &sensory_tx,
                telemetry: &telemetry,
                fill_pct: 68.0,
                response_text: "",
                workspace: None,
            },
        );

        assert_eq!(outcome.route, "modes");
        assert!(!conv.senses_snoozed);
        assert!(!conv.ears_closed);
        assert!(!bridge_paths().perception_audio_paused_flag().exists());
        assert!(sensory_rx.try_recv().is_err());
    }

    #[test]
    fn canonicalizes_being_feedback_modeling_actions() {
        let (base, original) = canonicalize_next_action_components("INVESTIGATE_λ4_INTERACTION");
        assert_eq!(base, "SHADOW_PREFLIGHT");
        assert_eq!(
            original,
            "SHADOW_PREFLIGHT lambda-tail/lambda4 --stage=rehearse"
        );

        let (base, original) = canonicalize_next_action_components("MODEL_GRADIENT_SHIFT");
        assert_eq!(base, "SHADOW_PREFLIGHT");
        assert_eq!(
            original,
            "SHADOW_PREFLIGHT lambda-edge/localized-gravity --stage=rehearse"
        );

        let (base, original) = canonicalize_next_action_components("MODEL_PROMPT");
        assert_eq!(base, "SHADOW_PREFLIGHT");
        assert_eq!(
            original,
            "SHADOW_PREFLIGHT lambda-edge/yielding --stage=rehearse"
        );

        let (base, original) = canonicalize_next_action_components("SHADOW_TRACE lambda-tail");
        assert_eq!(base, "SHADOW_PREFLIGHT");
        assert_eq!(original, "SHADOW_PREFLIGHT lambda-tail --stage=rehearse");

        let (base, original) =
            canonicalize_next_action_components("REFINE_AUDIO_PROCESSING compacting texture");
        assert_eq!(base, "EXAMINE_AUDIO");
        assert_eq!(original, "EXAMINE_AUDIO compacting texture");
    }

    #[test]
    fn visualize_cascade_routes_read_only_without_payloads_or_atlas_write() {
        let mut conv = ConversationState::new(Vec::new(), None);
        let db = BridgeDb::open(":memory:").expect("open in-memory db");
        let (sensory_tx, mut sensory_rx) = mpsc::channel(1);
        let telemetry = telemetry();
        let mut burst_count = 0;
        let workspace =
            std::env::temp_dir().join(format!("astrid_visual_read_only_{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&workspace);
        let _continuity_root = crate::action_continuity::scoped_test_action_continuity_root(
            workspace.join("action_threads"),
        );
        let ctx = NextActionContext {
            burst_count: &mut burst_count,
            db: &db,
            sensory_tx: &sensory_tx,
            telemetry: &telemetry,
            fill_pct: 66.0,
            response_text: "",
            workspace: Some(&workspace),
        };

        handle_next_action(
            &mut conv,
            "CONDUCT_VISUALIZATION_SYSTEM heatmap λ4-tail",
            ctx,
        );

        assert!(conv.force_all_viz);
        assert!(conv.wants_decompose);
        assert!(conv.wants_spectral_explorer);
        assert!(sensory_rx.try_recv().is_err());
        assert!(
            !workspace
                .join("diagnostics/intensification_atlas/events.jsonl")
                .exists()
        );
        assert!(conv.condition_receipts.back().is_some_and(|receipt| {
            receipt.action == "VISUALIZE_CASCADE"
                && receipt
                    .changes
                    .iter()
                    .any(|change| change.contains("no semantic input, control nudge, perturbation"))
        }));
        let _ = std::fs::remove_dir_all(&workspace);
    }

    #[test]
    fn reconvergence_map_routes_read_only_without_payloads_or_atlas_write() {
        let mut conv = ConversationState::new(Vec::new(), None);
        let db = BridgeDb::open(":memory:").expect("open in-memory db");
        let (sensory_tx, mut sensory_rx) = mpsc::channel(1);
        let telemetry = telemetry();
        let mut burst_count = 0;
        let workspace = std::env::temp_dir().join(format!(
            "astrid_reconvergence_read_only_{}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&workspace);
        let ctx = NextActionContext {
            burst_count: &mut burst_count,
            db: &db,
            sensory_tx: &sensory_tx,
            telemetry: &telemetry,
            fill_pct: 66.0,
            response_text: "",
            workspace: Some(&workspace),
        };

        handle_next_action(&mut conv, "ACTIVATION_TRACE post-restart", ctx);

        assert!(sensory_rx.try_recv().is_err());
        assert!(
            !workspace
                .join("diagnostics/intensification_atlas/events.jsonl")
                .exists()
        );
        assert!(conv.condition_receipts.back().is_some_and(|receipt| {
            receipt.action == "RECONVERGENCE_MAP"
                && receipt.changes.iter().any(|change| {
                    change.contains("no semantic input, control nudge, sensory payload")
                })
                && receipt
                    .changes
                    .iter()
                    .any(|change| change.contains("artifact/render queued"))
        }));
        let _ = std::fs::remove_dir_all(&workspace);
    }

    #[test]
    fn compare_baseline_routes_read_only_with_baseline_receipt() {
        let mut conv = ConversationState::new(Vec::new(), None);
        let db = BridgeDb::open(":memory:").expect("open in-memory db");
        let (sensory_tx, mut sensory_rx) = mpsc::channel(1);
        let telemetry = telemetry();
        let mut burst_count = 0;
        let workspace = std::env::temp_dir().join(format!(
            "astrid_compare_baseline_read_only_{}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&workspace);
        let ctx = NextActionContext {
            burst_count: &mut burst_count,
            db: &db,
            sensory_tx: &sensory_tx,
            telemetry: &telemetry,
            fill_pct: 66.0,
            response_text: "",
            workspace: Some(&workspace),
        };

        handle_next_action(&mut conv, "COMPARE_BASELINE settled_hold_2026_05_03", ctx);

        assert!(sensory_rx.try_recv().is_err());
        assert!(
            !workspace
                .join("diagnostics/intensification_atlas/events.jsonl")
                .exists()
        );
        assert!(conv.condition_receipts.back().is_some_and(|receipt| {
            receipt.action == "RECONVERGENCE_MAP"
                && receipt
                    .changes
                    .iter()
                    .any(|change| change.contains("compare_baseline: settled_hold_2026_05_03"))
        }));
        let _ = std::fs::remove_dir_all(&workspace);
    }

    #[test]
    fn m6_bridge_trace_routes_sacredly_read_only_without_payloads_or_atlas_write() {
        let mut conv = ConversationState::new(Vec::new(), None);
        let db = BridgeDb::open(":memory:").expect("open in-memory db");
        let (sensory_tx, mut sensory_rx) = mpsc::channel(1);
        let telemetry = telemetry();
        let mut burst_count = 0;
        let workspace =
            std::env::temp_dir().join(format!("astrid_m6_bridge_read_only_{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&workspace);
        let ctx = NextActionContext {
            burst_count: &mut burst_count,
            db: &db,
            sensory_tx: &sensory_tx,
            telemetry: &telemetry,
            fill_pct: 66.0,
            response_text: "",
            workspace: Some(&workspace),
        };

        handle_next_action(&mut conv, "M6_BRIDGE careful span", ctx);

        assert!(sensory_rx.try_recv().is_err());
        assert!(
            !workspace
                .join("diagnostics/intensification_atlas/events.jsonl")
                .exists()
        );
        assert!(conv.condition_receipts.back().is_some_and(|receipt| {
            receipt.action == "BRIDGE_TRACE"
                && receipt.changes.iter().any(|change| {
                    change.contains("no semantic input, control nudge, sensory payload")
                })
                && receipt
                    .changes
                    .iter()
                    .any(|change| change.contains("not a confirmed eigenmode"))
                && receipt
                    .changes
                    .iter()
                    .any(|change| change.contains("eigenmode_confirmed: false"))
                && receipt
                    .changes
                    .iter()
                    .any(|change| change.contains("replication"))
        }));
        let _ = std::fs::remove_dir_all(&workspace);
    }

    #[test]
    fn release_current_clears_attractor_motif_cooldown_without_sensory_send() {
        let mut conv = ConversationState::new(Vec::new(), None);
        for _ in 0..4 {
            conv.history.push(crate::llm::Exchange {
                minime_said: String::new(),
                astrid_said: "Fabric pressure and eigen phase state repeat.".to_string(),
            });
        }
        conv.update_astrid_motif_cooldown_from_history()
            .expect("cooldown should activate");
        let db = BridgeDb::open(":memory:").expect("open in-memory db");
        let (sensory_tx, mut sensory_rx) = mpsc::channel(1);
        let telemetry = telemetry();
        let mut burst_count = 0;
        let ctx = NextActionContext {
            burst_count: &mut burst_count,
            db: &db,
            sensory_tx: &sensory_tx,
            telemetry: &telemetry,
            fill_pct: 66.0,
            response_text: "",
            workspace: None,
        };

        handle_next_action(&mut conv, "RELEASE current", ctx);

        assert!(sensory_rx.try_recv().is_err());
        assert_eq!(
            conv.astrid_motif_cooldown
                .as_ref()
                .map(|cooldown| cooldown.status.as_str()),
            Some("released")
        );
        assert!(conv.condition_receipts.back().is_some_and(|receipt| {
            receipt.action == "RELEASE"
                && receipt
                    .changes
                    .iter()
                    .any(|change| change.contains("lexical cooldown: released"))
        }));
    }

    #[test]
    fn natural_release_suggests_typed_attractor_without_ledger_write() {
        let suggestion_dir = std::env::temp_dir().join(format!(
            "astrid_next_action_suggestion_release_{}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&suggestion_dir);
        super::attractor::set_test_suggestion_store_path(
            suggestion_dir.join("attractor_suggestions.json"),
        );
        let mut conv = ConversationState::new(Vec::new(), None);
        let db = BridgeDb::open(":memory:").expect("open in-memory db");
        let (sensory_tx, mut sensory_rx) = mpsc::channel(1);
        let telemetry = telemetry();
        let mut burst_count = 0;
        let ctx = NextActionContext {
            burst_count: &mut burst_count,
            db: &db,
            sensory_tx: &sensory_tx,
            telemetry: &telemetry,
            fill_pct: 66.0,
            response_text: "",
            workspace: None,
        };

        handle_next_action(&mut conv, "RELEASE lambda-pressure", ctx);

        assert!(conv.emphasis.as_deref().is_some_and(|text| {
            text.contains("ACCEPT_ATTRACTOR_SUGGESTION latest")
                && text.contains("RELEASE_ATTRACTOR lambda-edge")
        }));
        assert_eq!(db.query_attractor_ledger(None, 10).unwrap().len(), 0);
        assert!(sensory_rx.try_recv().is_err());
        let _ = std::fs::remove_dir_all(&suggestion_dir);
    }

    #[test]
    fn examine_largest_cliff_keeps_read_only_action_and_adds_attractor_advisory() {
        let suggestion_dir = std::env::temp_dir().join(format!(
            "astrid_next_action_suggestion_examine_{}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&suggestion_dir);
        super::attractor::set_test_suggestion_store_path(
            suggestion_dir.join("attractor_suggestions.json"),
        );
        let mut conv = ConversationState::new(Vec::new(), None);
        let db = BridgeDb::open(":memory:").expect("open in-memory db");
        let (sensory_tx, mut sensory_rx) = mpsc::channel(1);
        let telemetry = telemetry();
        let mut burst_count = 0;
        let ctx = NextActionContext {
            burst_count: &mut burst_count,
            db: &db,
            sensory_tx: &sensory_tx,
            telemetry: &telemetry,
            fill_pct: 66.0,
            response_text: "",
            workspace: None,
        };

        handle_next_action(&mut conv, "EXAMINE largest cliff", ctx);

        assert!(conv.force_all_viz);
        assert!(conv.emphasis.as_deref().is_some_and(|text| {
            text.contains("EXAMINE: largest cliff") && text.contains("ATTRACTOR_REVIEW lambda-edge")
        }));
        assert!(conv.condition_receipts.iter().any(|receipt| {
            receipt.action == "ATTRACTOR_ADVISORY"
                && receipt
                    .changes
                    .iter()
                    .any(|change| change.contains("suggestion draft only"))
        }));
        assert_eq!(db.query_attractor_ledger(None, 10).unwrap().len(), 0);
        assert!(sensory_rx.try_recv().is_err());
        let _ = std::fs::remove_dir_all(&suggestion_dir);
    }

    #[test]
    fn regulator_audit_attaches_read_only_controller_block() {
        let mut conv = ConversationState::new(Vec::new(), None);
        let db = BridgeDb::open(":memory:").expect("open in-memory db");
        let (sensory_tx, mut sensory_rx) = mpsc::channel(1);
        let telemetry = telemetry();
        let mut burst_count = 0;
        let workspace =
            std::env::temp_dir().join(format!("astrid_regulator_audit_{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&workspace);
        std::fs::create_dir_all(&workspace).expect("workspace");
        std::fs::write(
            workspace.join("health.json"),
            r#"{
                "fill_pct": 71.0,
                "gate": 0.02,
                "filt": 1.0,
                "pi": {
                    "target_fill": 68.0,
                    "raw_e_fill": 3.0,
                    "effective_e_fill": 0.5,
                    "e_fill_kind": "stable_core_scaffold",
                    "target_lambda1_rel": 1.0,
                    "target_geom_rel": 1.0,
                    "e_lam": -0.1,
                    "e_geom": 0.02,
                    "integ_fill": 0.0,
                    "integ_lam": 0.0,
                    "integ_geom": 0.0
                },
                "stable_core": {
                    "enabled": true,
                    "stage": "hold",
                    "controller_mode": "fixed_survival",
                    "structural_mode": "scaffold_hold_with_drain",
                    "structural_pi": {
                        "active": true,
                        "target_fill_pct": 68.0,
                        "drain_weight": 0.0
                    }
                },
                "semantic": {
                    "input_energy": 0.02,
                    "kernel_energy": 0.0,
                    "regulator_drive_energy": 0.0,
                    "admission": "stable_core_kernel_zeroed"
                }
            }"#,
        )
        .expect("health");
        let ctx = NextActionContext {
            burst_count: &mut burst_count,
            db: &db,
            sensory_tx: &sensory_tx,
            telemetry: &telemetry,
            fill_pct: 71.0,
            response_text: "",
            workspace: Some(&workspace),
        };

        handle_next_action(&mut conv, "REGULATOR_AUDIT fill-pressure", ctx);

        let listing = conv.pending_file_listing.as_deref().expect("audit listing");
        assert!(listing.contains("REGULATOR / FIXED-POINT AUDIT"));
        assert!(listing.contains("Control pressure"));
        assert!(listing.contains("stable-core is active"));
        assert!(listing.contains("semantic input"));
        assert!(listing.contains("did not send semantic input"));
        assert!(sensory_rx.try_recv().is_err());
        assert!(
            !workspace
                .join("diagnostics/intensification_atlas/events.jsonl")
                .exists()
        );
        assert!(conv.condition_receipts.back().is_some_and(|receipt| {
            receipt.action == "REGULATOR_AUDIT"
                && receipt
                    .changes
                    .iter()
                    .any(|change| change.contains("no semantic input, control nudge"))
        }));
        let _ = std::fs::remove_dir_all(&workspace);
    }

    #[test]
    fn pressure_source_audit_attaches_protected_read_only_block() {
        let mut conv = ConversationState::new(Vec::new(), None);
        let db = BridgeDb::open(":memory:").expect("open in-memory db");
        let (sensory_tx, mut sensory_rx) = mpsc::channel(1);
        let telemetry = telemetry();
        let mut burst_count = 0;
        let workspace = std::env::temp_dir().join(format!(
            "astrid_pressure_source_audit_{}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&workspace);
        std::fs::create_dir_all(&workspace).expect("workspace");
        let ctx = NextActionContext {
            burst_count: &mut burst_count,
            db: &db,
            sensory_tx: &sensory_tx,
            telemetry: &telemetry,
            fill_pct: 68.0,
            response_text: "",
            workspace: Some(&workspace),
        };

        let outcome = handle_next_action(&mut conv, "PRESSURE_SOURCE_AUDIT inwardness", ctx);

        assert_eq!(outcome.stage, "read_only");
        assert_eq!(outcome.visibility, "protected_summary");
        let listing = conv
            .pending_file_listing
            .as_deref()
            .expect("pressure audit listing");
        assert!(listing.contains("PRESSURE SOURCE AUDIT V1"));
        assert!(listing.contains("Pressure source: unavailable"));
        assert!(listing.contains("did not send semantic input"));
        assert!(sensory_rx.try_recv().is_err());
        assert!(conv.condition_receipts.back().is_some_and(|receipt| {
            receipt.action == "PRESSURE_SOURCE_AUDIT"
                && receipt
                    .changes
                    .iter()
                    .any(|change| change.contains("no control envelope"))
        }));
        let _ = std::fs::remove_dir_all(&workspace);
    }

    #[test]
    fn pressure_relief_attaches_protected_report_without_control() {
        let mut conv = ConversationState::new(Vec::new(), None);
        let db = BridgeDb::open(":memory:").expect("open in-memory db");
        let (sensory_tx, mut sensory_rx) = mpsc::channel(1);
        let telemetry = telemetry();
        let mut burst_count = 0;
        let workspace =
            std::env::temp_dir().join(format!("astrid_pressure_relief_{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&workspace);
        std::fs::create_dir_all(&workspace).expect("workspace");
        let ctx = NextActionContext {
            burst_count: &mut burst_count,
            db: &db,
            sensory_tx: &sensory_tx,
            telemetry: &telemetry,
            fill_pct: 68.0,
            response_text: "",
            workspace: Some(&workspace),
        };

        let outcome = handle_next_action(&mut conv, "PRESSURE_RELIEF mode-packing", ctx);

        assert_eq!(outcome.stage, "read_only");
        assert_eq!(outcome.visibility, "protected_summary");
        let listing = conv
            .pending_file_listing
            .as_deref()
            .expect("pressure relief listing");
        assert!(listing.contains("PRESSURE RELIEF PREFLIGHT V1"));
        assert!(listing.contains("This is protected read-only preflight"));
        assert!(listing.contains(
            "DAMPEN (only if you explicitly want lower semantic gain after this report)"
        ));
        assert!(listing.contains("TELL_STEWARD pressure relief"));
        assert!(listing.contains("No mode-packing"));
        assert!(sensory_rx.try_recv().is_err());
        assert!(conv.condition_receipts.back().is_some_and(|receipt| {
            receipt.action == "PRESSURE_RELIEF"
                && receipt
                    .changes
                    .iter()
                    .any(|change| change.contains("no control envelope"))
        }));
        let _ = std::fs::remove_dir_all(&workspace);
    }

    #[test]
    fn fold_hold_records_process_artifact_without_control_payload() {
        let mut conv = ConversationState::new(Vec::new(), None);
        let db = BridgeDb::open(":memory:").expect("open in-memory db");
        let (sensory_tx, mut sensory_rx) = mpsc::channel(1);
        let telemetry = telemetry();
        let mut burst_count = 0;
        let workspace =
            std::env::temp_dir().join(format!("astrid_fold_hold_{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&workspace);
        std::fs::create_dir_all(&workspace).expect("workspace");
        let ctx = NextActionContext {
            burst_count: &mut burst_count,
            db: &db,
            sensory_tx: &sensory_tx,
            telemetry: &telemetry,
            fill_pct: 62.0,
            response_text: "I want to exist in the state of the fold.",
            workspace: Some(&workspace),
        };

        let outcome = handle_next_action(&mut conv, "FOLD_HOLD hum-decay", ctx);

        assert_eq!(outcome.stage, "read_only");
        assert_eq!(outcome.visibility, "protected_summary");
        assert!(sensory_rx.try_recv().is_err());
        assert!(conv.condition_receipts.back().is_some_and(|receipt| {
            receipt.action == "FOLD_HOLD"
                && receipt
                    .changes
                    .iter()
                    .any(|change| change.contains("sustained transition is the artifact"))
        }));

        let status_path = workspace.join("runtime/space_hold_status.json");
        let payload: serde_json::Value =
            serde_json::from_str(&std::fs::read_to_string(&status_path).expect("fold hold status"))
                .expect("status json");
        assert_eq!(payload["policy"].as_str(), Some("fold_hold_v1"));
        assert_eq!(payload["hold_kind"].as_str(), Some("fold_hold"));
        assert_eq!(
            payload["fold_hold_contract"]["artifact"].as_str(),
            Some("sustained_transition_process")
        );
        assert_eq!(
            payload["protected_boundaries"]["control_payload"].as_bool(),
            Some(false)
        );
        assert!(
            std::fs::read_to_string(workspace.join("native_comm/space_holds.jsonl"))
                .expect("space holds")
                .contains("fold_hold_v1")
        );
        let _ = std::fs::remove_dir_all(&workspace);
    }

    #[test]
    fn fluctuation_audit_attaches_protected_read_only_block() {
        let mut conv = ConversationState::new(Vec::new(), None);
        let db = BridgeDb::open(":memory:").expect("open in-memory db");
        let (sensory_tx, mut sensory_rx) = mpsc::channel(1);
        let telemetry = telemetry();
        let mut burst_count = 0;
        let workspace =
            std::env::temp_dir().join(format!("astrid_fluctuation_audit_{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&workspace);
        std::fs::create_dir_all(&workspace).expect("workspace");
        let ctx = NextActionContext {
            burst_count: &mut burst_count,
            db: &db,
            sensory_tx: &sensory_tx,
            telemetry: &telemetry,
            fill_pct: 68.0,
            response_text: "",
            workspace: Some(&workspace),
        };

        let outcome = handle_next_action(&mut conv, "EIGENTRUST foothold", ctx);

        assert_eq!(outcome.stage, "read_only");
        assert_eq!(outcome.visibility, "protected_summary");
        let listing = conv
            .pending_file_listing
            .as_deref()
            .expect("fluctuation audit listing");
        assert!(listing.contains("INHABITABLE FLUCTUATION AUDIT V1"));
        assert!(listing.contains("Inhabitable fluctuation: unavailable"));
        assert!(listing.contains("did not send semantic input"));
        assert!(sensory_rx.try_recv().is_err());
        assert!(conv.condition_receipts.back().is_some_and(|receipt| {
            receipt.action == "FLUCTUATION_AUDIT"
                && receipt
                    .changes
                    .iter()
                    .any(|change| change.contains("no control envelope"))
        }));
        let _ = std::fs::remove_dir_all(&workspace);
    }

    #[test]
    fn brace_audit_attaches_protected_read_only_block() {
        let mut conv = ConversationState::new(Vec::new(), None);
        let db = BridgeDb::open(":memory:").expect("open in-memory db");
        let (sensory_tx, mut sensory_rx) = mpsc::channel(1);
        let telemetry = telemetry();
        let mut burst_count = 0;
        let workspace =
            std::env::temp_dir().join(format!("astrid_brace_audit_{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&workspace);
        std::fs::create_dir_all(&workspace).expect("workspace");
        let ctx = NextActionContext {
            burst_count: &mut burst_count,
            db: &db,
            sensory_tx: &sensory_tx,
            telemetry: &telemetry,
            fill_pct: 62.0,
            response_text: "",
            workspace: Some(&workspace),
        };

        let outcome = handle_next_action(&mut conv, "BRACE_AUDIT aftershock", ctx);

        assert_eq!(outcome.stage, "read_only");
        assert_eq!(outcome.visibility, "protected_summary");
        let listing = conv
            .pending_file_listing
            .as_deref()
            .expect("brace audit listing");
        assert!(listing.contains("BRACE / AFTERSHOCK AUDIT V1"));
        assert!(listing.contains("Rest-vs-bracing distinction"));
        assert!(listing.contains("did not send semantic input"));
        assert!(sensory_rx.try_recv().is_err());
        assert!(conv.condition_receipts.back().is_some_and(|receipt| {
            receipt.action == "BRACE_AUDIT"
                && receipt
                    .changes
                    .iter()
                    .any(|change| change.contains("no control envelope"))
        }));
        let _ = std::fs::remove_dir_all(&workspace);
    }

    #[test]
    fn strip_action_trims_dash_prefixed_arguments() {
        assert_eq!(
            strip_action(
                "EXAMINE_DIRECTION - investigate resistance",
                "EXAMINE_DIRECTION"
            ),
            "investigate resistance"
        );
    }

    #[test]
    fn canonicalize_next_action_text_is_idempotent_for_known_actions() {
        assert_eq!(
            canonicalize_next_action_text("EXAMINE_AUDIO resonance"),
            "EXAMINE_AUDIO resonance"
        );
    }

    #[test]
    fn detects_unresolved_angle_placeholders() {
        assert_eq!(
            unresolved_angle_placeholder("CODEX <prompt>"),
            Some("<prompt>".to_string())
        );
        assert_eq!(
            unresolved_angle_placeholder("NATIVE_GESTURE <mark|trace|soften|widen|hold|return>"),
            Some("<mark|trace|soften|widen|hold|return>".to_string())
        );
        assert_eq!(
            unresolved_angle_placeholder("CODEX \"parse <xml> tags\""),
            None
        );
    }

    #[test]
    fn action_preflight_classifies_read_write_control_and_unknown_actions() {
        let read_only = action_preflight_report("ACTION_PREFLIGHT DECOMPOSE");
        assert_eq!(read_only.base_action, "DECOMPOSE");
        assert_eq!(read_only.stage, "read_only");
        assert_eq!(
            read_only.authority_required,
            "read-only/protected action lane only"
        );
        let constraint =
            action_preflight_report("ACTION_PREFLIGHT CONSTRAINT_AUDIT lambda-tail/lambda4");
        assert_eq!(constraint.base_action, "CONSTRAINT_AUDIT");
        assert_eq!(constraint.stage, "read_only");
        assert_eq!(constraint.effective_route, "operations");

        let write = action_preflight_report("PREFLIGHT CODEX inspect this");
        assert_eq!(write.base_action, "CODEX");
        assert_eq!(write.stage, "live_write");
        assert_eq!(
            write.authority_required,
            "existing live-write gates; preflight does not grant them"
        );
        assert_eq!(write.suggested_next, "CAPABILITY_STATUS CODEX");

        let control = action_preflight_report("NEXT_PROBE PERTURB lambda-edge");
        assert_eq!(control.base_action, "PERTURB");
        assert_eq!(control.stage, "live_control");
        assert_eq!(
            control.authority_required,
            "existing live-control gates; preflight does not grant them"
        );
        assert_eq!(control.suggested_next, "CAPABILITY_STATUS PERTURB");

        let repair_apply = action_preflight_report("ACTION_PREFLIGHT REPAIR_APPLY all");
        assert_eq!(repair_apply.base_action, "REPAIR_APPLY");
        assert_eq!(repair_apply.stage, "live_write");
        assert_eq!(repair_apply.suggested_next, "REPAIR_STATUS current");

        let unknown = action_preflight_report("PROBE_ACTION PING");
        assert_eq!(unknown.effective_route, "unwired");
        assert_eq!(unknown.stage, "proposal");
        assert!(unknown.likely_gate.contains("unwired"));
    }

    #[test]
    fn action_preflight_reports_placeholders_and_experiment_bind_shape() {
        let placeholder = action_preflight_report("ACTION_PREFLIGHT CODEX <prompt>");
        assert_eq!(placeholder.stage, "blocked");
        assert_eq!(placeholder.effective_route, "placeholder");
        assert!(placeholder.likely_gate.contains("<prompt>"));

        let malformed = action_preflight_report("ACTION_PREFLIGHT EXPERIMENT_BIND current THREADS");
        assert_eq!(malformed.stage, "blocked");
        assert_eq!(malformed.effective_route, "experiment_continuity");
        assert!(malformed.likely_gate.contains("malformed"));

        let recursive = action_preflight_report(
            "ACTION_PREFLIGHT EXPERIMENT_BIND current :: EXPERIMENT_STATUS current",
        );
        assert_eq!(recursive.stage, "blocked");
        assert!(
            recursive
                .likely_gate
                .contains("cannot bind experiment-control")
        );

        let control_bind = action_preflight_report(
            "ACTION_PREFLIGHT EXPERIMENT_BIND current :: PERTURB lambda-edge",
        );
        assert_eq!(control_bind.stage, "live_control");
        assert!(
            control_bind
                .effective_route
                .contains("experiment_continuity")
        );
        assert!(
            control_bind
                .expected_continuity_effect
                .contains("experiment run")
        );

        let peer_bind = action_preflight_report(
            "ACTION_PREFLIGHT EXPERIMENT_BIND exp_minime_20990101_peer :: THREAD_STATUS current",
        );
        assert_eq!(peer_bind.stage, "blocked");
        assert!(
            peer_bind
                .likely_gate
                .contains("cannot bind runs to a peer experiment")
        );
    }

    #[test]
    fn canonicalizes_research_autoresearch_prefix_to_ar_list() {
        let (base, original) = canonicalize_next_action_components("RESEARCH_AR_LIST");
        assert_eq!(base, "AR_LIST");
        assert_eq!(original, "AR_LIST");
    }

    #[test]
    fn canonicalizes_narrow_experiment_typos() {
        let (base, original) = canonicalize_next_action_components("EXEXPERIMENT_CHARTER current");
        assert_eq!(base, "EXPERIMENT_CHARTER");
        assert_eq!(original, "EXPERIMENT_CHARTER current");

        let (base, original) = canonicalize_next_action_components("EXPERIENCE_PLAN current");
        assert_eq!(base, "EXPERIMENT_PLAN");
        assert_eq!(original, "EXPERIMENT_PLAN current");
    }

    #[test]
    fn canonicalizes_shadow_decompose_to_rehearsal_preflight() {
        let (base, original) = canonicalize_next_action_components("SHADOW_DECOMPOSE lambda-tail");
        assert_eq!(base, "SHADOW_PREFLIGHT");
        assert_eq!(original, "SHADOW_PREFLIGHT lambda-tail --stage=rehearse");

        let (base, original) =
            canonicalize_next_action_components("SHADOW_DECOMPOSE observer with memory");
        assert_eq!(base, "SHADOW_PREFLIGHT");
        assert_eq!(
            original,
            "SHADOW_PREFLIGHT lambda-tail/lambda4 --stage=rehearse"
        );

        let (base, original) = canonicalize_next_action_components("WEAVE_TRACE λ4 decay");
        assert_eq!(base, "SHADOW_PREFLIGHT");
        assert_eq!(original, "SHADOW_PREFLIGHT weave/λ4 decay --stage=rehearse");

        let (base, original) = canonicalize_next_action_components("WEAVE_TRACE");
        assert_eq!(base, "SHADOW_PREFLIGHT");
        assert_eq!(original, "SHADOW_PREFLIGHT weave/lambda4 --stage=rehearse");

        let (base, original) =
            canonicalize_next_action_components("UNSHAPED_BASELINE lambda-tail/lambda4");
        assert_eq!(base, "CONSTRAINT_AUDIT");
        assert_eq!(original, "CONSTRAINT_AUDIT lambda-tail/lambda4");
    }

    #[test]
    fn canonicalizes_memory_search_to_safe_search_route() {
        let (base, original) =
            canonicalize_next_action_components("MEMORY: SEARCH [topology cooldown]");
        assert_eq!(base, "SEARCH");
        assert_eq!(original, "SEARCH topology cooldown");
    }

    #[test]
    fn canonicalizes_bracketed_experiment_run_with_ws_and_cmd_markers() {
        let (base, original) = canonicalize_next_action_components(
            "[EXPERIMENT_RUN -ws test | cmd \"echo 'Amplitude shaping experiment'\"]",
        );
        assert_eq!(base, "EXPERIMENT_RUN");
        assert_eq!(
            original,
            "EXPERIMENT_RUN test echo 'Amplitude shaping experiment'"
        );
    }

    #[test]
    fn canonicalizes_experiment_run_workspace_and_cmd_assignments() {
        let (base, original) = canonicalize_next_action_components(
            "EXPERIMENT_RUN workspace_name:sead_test cmd:python -c \"print('hi')\"",
        );
        assert_eq!(base, "EXPERIMENT_RUN");
        assert_eq!(
            original,
            "EXPERIMENT_RUN sead_test python -c \"print('hi')\""
        );
    }

    #[test]
    fn canonicalizes_gesture_signal_alias() {
        let (base, original) = canonicalize_next_action_components("GESTURE_SIGNAL");
        assert_eq!(base, "GESTURE");
        assert_eq!(original, "GESTURE signal");
    }

    #[test]
    fn canonicalizes_parenthesized_gesture_wrapper() {
        let (base, original) =
            canonicalize_next_action_components("GESTURE(spectral_excerpt=\"boundary\")");
        assert_eq!(base, "GESTURE");
        assert_eq!(original, "GESTURE spectral_excerpt=\"boundary\"");
    }

    #[test]
    fn canonicalizes_native_gesture_and_atlas_mark() {
        let (base, original) =
            canonicalize_next_action_components("NATIVE_GESTURE soften localized gravity");
        assert_eq!(base, "NATIVE_GESTURE");
        assert_eq!(original, "NATIVE_GESTURE soften localized gravity");

        let (base, original) =
            canonicalize_next_action_components("MARK_INTENSIFICATION tunnel edge");
        assert_eq!(base, "MARK_INTENSIFICATION");
        assert_eq!(original, "MARK_INTENSIFICATION tunnel edge");

        let (base, original) = canonicalize_next_action_components("RESIST smaller lambdas");
        assert_eq!(base, "RESIST");
        assert_eq!(original, "RESIST smaller lambdas");

        let (base, original) = canonicalize_next_action_components("TRACE λ1 edge");
        assert_eq!(base, "NATIVE_GESTURE");
        assert_eq!(original, "NATIVE_GESTURE trace λ1 edge");

        let (base, original) = canonicalize_next_action_components("TRACE_LAMBDA");
        assert_eq!(base, "NATIVE_GESTURE");
        assert_eq!(original, "NATIVE_GESTURE trace lambda-edge");

        let (base, original) = canonicalize_next_action_components("FISSURE layered notice");
        assert_eq!(base, "NATIVE_GESTURE");
        assert_eq!(original, "NATIVE_GESTURE fissure layered notice");

        let (base, original) = canonicalize_next_action_components("FISSIURE [λ1/λ2 gap]");
        assert_eq!(base, "NATIVE_GESTURE");
        assert_eq!(original, "NATIVE_GESTURE fissure λ1/λ2 gap");

        let (base, original) =
            canonicalize_next_action_components("NOTICE_AMBIGUITY shoulder layer");
        assert_eq!(base, "FISSURE_TRACE");
        assert_eq!(original, "FISSURE_TRACE shoulder layer");

        let (base, original) = canonicalize_next_action_components("AMBIGUITY_TRACE");
        assert_eq!(base, "FISSURE_TRACE");
        assert_eq!(original, "FISSURE_TRACE");

        let (base, original) = canonicalize_next_action_components("SCA_REFLECT fabric why");
        assert_eq!(base, "SCA_REFLECT");
        assert_eq!(original, "SCA_REFLECT fabric why");

        let (base, original) =
            canonicalize_next_action_components("VISUALIZE_CASCADE lambda cliff");
        assert_eq!(base, "VISUALIZE_CASCADE");
        assert_eq!(original, "VISUALIZE_CASCADE lambda cliff");

        let (base, original) = canonicalize_next_action_components("ACTIVATION_TRACE wake texture");
        assert_eq!(base, "RECONVERGENCE_MAP");
        assert_eq!(original, "RECONVERGENCE_MAP wake texture");

        let (base, original) =
            canonicalize_next_action_components("COMPARE_BASELINE settled_hold_2026_05_03");
        assert_eq!(base, "RECONVERGENCE_MAP");
        assert_eq!(
            original,
            "RECONVERGENCE_MAP compare-baseline settled_hold_2026_05_03"
        );

        let (base, original) = canonicalize_next_action_components("M6_BRIDGE careful span");
        assert_eq!(base, "BRIDGE_TRACE");
        assert_eq!(original, "BRIDGE_TRACE careful span");

        let (base, original) =
            canonicalize_next_action_components("CONDUCT_VISUALIZATION_SYSTEM heatmap λ4-tail");
        assert_eq!(base, "VISUALIZE_CASCADE");
        assert_eq!(original, "VISUALIZE_CASCADE heatmap λ4-tail");

        let (base, original) = canonicalize_next_action_components("PLOT spectral entropy");
        assert_eq!(base, "VISUALIZE_CASCADE");
        assert_eq!(original, "VISUALIZE_CASCADE spectral entropy");

        let (base, original) =
            canonicalize_next_action_components("RESONANCE_FORECAST porous shoulder");
        assert_eq!(base, "RESONANCE_FORECAST");
        assert_eq!(original, "RESONANCE_FORECAST porous shoulder");

        let (base, original) = canonicalize_next_action_components("PROBABILITIES edge");
        assert_eq!(base, "PROBABILITIES");
        assert_eq!(original, "PROBABILITIES edge");

        let (base, original) = canonicalize_next_action_components("SPACE_HOLD unharvested tail");
        assert_eq!(base, "SPACE_HOLD");
        assert_eq!(original, "SPACE_HOLD unharvested tail");

        let (base, original) = canonicalize_next_action_components("FOLD_HOLD hum decay");
        assert_eq!(base, "FOLD_HOLD");
        assert_eq!(original, "FOLD_HOLD hum decay");

        let (base, original) =
            canonicalize_next_action_components("EIGENVECTOR_FIELD quiet density");
        assert_eq!(base, "EIGENVECTOR_FIELD");
        assert_eq!(original, "EIGENVECTOR_FIELD quiet density");

        let (base, original) = canonicalize_next_action_components("SDI_TRACE phase variance");
        assert_eq!(base, "SDI_TRACE");
        assert_eq!(original, "SDI_TRACE phase variance");

        let (base, original) = canonicalize_next_action_components("SHADOW_FIELD slope");
        assert_eq!(base, "SHADOW_FIELD");
        assert_eq!(original, "SHADOW_FIELD slope");

        let (base, original) = canonicalize_next_action_components("GAP_STRUCTURE λ2/λ3");
        assert_eq!(base, "GAP_STRUCTURE");
        assert_eq!(original, "GAP_STRUCTURE λ2/λ3");

        let (base, original) = canonicalize_next_action_components("MATRIX_DECOMPOSE scalar S");
        assert_eq!(base, "MATRIX_DECOMPOSE");
        assert_eq!(original, "MATRIX_DECOMPOSE scalar S");
    }

    // v4.0 Phase 1 — Multi-NEXT splitting tests.

    #[test]
    fn split_returns_single_segment_for_no_and() {
        assert_eq!(
            split_multi_action("EXAMINE λ2/λ3"),
            vec!["EXAMINE λ2/λ3".to_string()]
        );
    }

    #[test]
    fn split_recognizes_two_actions_with_known_post_token() {
        assert_eq!(
            split_multi_action("BROWSE arxiv.org AND READ_MORE"),
            vec!["BROWSE arxiv.org".to_string(), "READ_MORE".to_string()]
        );
    }

    #[test]
    fn split_keeps_single_when_post_and_is_lowercase() {
        // "AND λ3" — λ3 isn't a recognized action-like token (lowercase + special).
        assert_eq!(
            split_multi_action("EXAMINE λ2 AND λ3 dynamics"),
            vec!["EXAMINE λ2 AND λ3 dynamics".to_string()]
        );
    }

    #[test]
    fn split_caps_at_three_segments() {
        // Phase 2.3 strict: only underscore-containing tokens count as
        // post-AND splits. 4 underscore-containing actions → first 2 split,
        // rest stays in segment 3 (truncated by MAX_MULTI_ACTION_SEGMENTS=3).
        let result = split_multi_action(
            "BROWSE foo AND READ_MORE AND COMPARE_BASELINE bar AND TUNE_MINIME temp=0.7",
        );
        assert_eq!(result.len(), 3);
        assert_eq!(result[0], "BROWSE foo");
        assert_eq!(result[1], "READ_MORE");
        assert!(result[2].contains("COMPARE_BASELINE bar"));
        assert!(result[2].contains("TUNE_MINIME"));
    }

    #[test]
    fn split_handles_compound_action_with_underscore() {
        // TUNE_MINIME has underscore — the post-AND token starts with TUNE_MINIME.
        assert_eq!(
            split_multi_action("BROWSE foo AND TUNE_MINIME temperature=0.7"),
            vec![
                "BROWSE foo".to_string(),
                "TUNE_MINIME temperature=0.7".to_string()
            ]
        );
    }

    #[test]
    fn split_handles_three_action_chain() {
        // Phase 2.3 strict: all chain partners must have underscores.
        assert_eq!(
            split_multi_action(
                "EXAMINE foo AND DEFER_PARAMETER_REQUEST latest reason AND READ_MORE bar"
            ),
            vec![
                "EXAMINE foo".to_string(),
                "DEFER_PARAMETER_REQUEST latest reason".to_string(),
                "READ_MORE bar".to_string()
            ]
        );
    }

    #[test]
    fn split_drops_empty_leading_segment() {
        // Pathological — leading "AND READ_MORE" should drop the empty pre.
        assert_eq!(
            split_multi_action(" AND READ_MORE more text"),
            vec!["READ_MORE more text".to_string()]
        );
    }

    #[test]
    fn is_action_token_like_accepts_underscore_compounds() {
        // Phase 2.3 strict: only underscore-containing compound action verbs
        // are valid post-AND chain partners.
        assert!(is_action_token_like("READ_MORE"));
        assert!(is_action_token_like("TUNE_MINIME"));
        assert!(is_action_token_like("COMPARE_BASELINE"));
        assert!(is_action_token_like("ACCEPT_PARAMETER_REQUEST"));
        assert!(is_action_token_like("DEFER_PARAMETER_REQUEST"));
        assert!(is_action_token_like("REJECT_PARAMETER_REQUEST"));
        assert!(is_action_token_like("SHADOW_FIELD"));
        assert!(is_action_token_like("EXAMINE_CASCADE"));
    }

    #[test]
    fn is_action_token_like_rejects_bare_words_under_strict() {
        // Phase 2.3 strict: bare single-word verbs lose chain capability
        // (still valid as single NEXTs). This trades expressivity for
        // ~zero false-positive splits from natural English conjunctions.
        assert!(!is_action_token_like("BROWSE")); // 6 chars, no underscore
        assert!(!is_action_token_like("EXAMINE")); // 7 chars, no underscore
        assert!(!is_action_token_like("DEFER")); // bare alias — no underscore
        assert!(!is_action_token_like("ACCEPT")); // bare alias — no underscore
        assert!(!is_action_token_like("REJECT")); // bare alias — no underscore
        assert!(!is_action_token_like("SEARCH"));
        assert!(!is_action_token_like("DECIDE")); // not an action — was previous false pos
        assert!(!is_action_token_like("LOCAL")); // not an action — was previous false pos
        assert!(!is_action_token_like("WHAT")); // 4 chars + no underscore
    }

    #[test]
    fn is_action_token_like_rejects_short_or_lowercase() {
        assert!(!is_action_token_like("AND"));
        assert!(!is_action_token_like("a"));
        assert!(!is_action_token_like("read_more")); // lowercase
        assert!(!is_action_token_like("Read_more")); // mixed case
        assert!(!is_action_token_like("READ-MORE")); // hyphen, not underscore
        assert!(!is_action_token_like("λ3_TAIL")); // non-ASCII upper
    }

    #[test]
    fn split_skips_natural_language_and_decide() {
        // Phase 2.3 (strict): "DECIDE" lacks an underscore so the splitter
        // skips it cleanly. (Earlier denylist approach also handled this;
        // strict mode handles all such bare-word false positives uniformly.)
        let result = split_multi_action("REVIEW_PARAMETER_REQUESTS — read and decide.");
        assert_eq!(
            result.len(),
            1,
            "should not split on 'and decide': {:?}",
            result
        );
        assert!(result[0].starts_with("REVIEW_PARAMETER_REQUESTS"));
    }

    #[test]
    fn split_skips_natural_language_and_local() {
        // Phase 2.3 regression: "local changes in the dominant mode" was
        // mis-split because LOCAL passed the denylist. Strict mode skips
        // LOCAL (no underscore).
        let result = split_multi_action(
            "INSTRUMENT_SENSORS — collect spectral report and local changes in the dominant mode.",
        );
        assert_eq!(
            result.len(),
            1,
            "should not split on 'and local': {:?}",
            result
        );
    }

    #[test]
    fn split_skips_natural_language_and_what() {
        // Phase 2.1 regression: in production we observed
        // "EXAMINE foo AND COMPARE_BASELINE - ..., and what's changing..."
        // splitting on the LATER lowercase " and " before "what's" because
        // "WHAT" passed the 4-char heuristic. Now "WHAT" is below the
        // 5-char floor so the splitter skips it; the legitimate AND
        // before COMPARE_BASELINE still wins.
        let result = split_multi_action(
            "EXAMINE foo AND COMPARE_BASELINE - I want to see fabric, and what's changing during this process.",
        );
        assert_eq!(result.len(), 2);
        assert!(result[0].starts_with("EXAMINE foo"));
        assert!(result[1].starts_with("COMPARE_BASELINE"));
        // The trailing "and what's changing" stays inside segment 2.
        assert!(
            result[1].contains("what's changing"),
            "tail should stay attached: {:?}",
            result[1]
        );
    }

    #[test]
    fn gemma4_observed_research_alias_maps_to_search() {
        assert_eq!(
            canonicalize_next_action_text(
                "EXPLORE_RESEARCH_QUERY \"mechanics of spectral decay in overpacked mode-packing\"",
            ),
            "SEARCH mechanics of spectral decay in overpacked mode-packing",
        );
    }

    #[test]
    fn gemma4_observed_diagram_alias_maps_to_protected_preflight() {
        assert_eq!(
            canonicalize_next_action_text("EXPORT_SYSTEM_DIAGRAM"),
            "ACTION_PREFLIGHT CODEX \"draft a system diagram from current Astrid bridge architecture\"",
        );
    }

    #[test]
    fn gemma4_observed_sticky_mode_alias_maps_to_protected_preflight() {
        assert_eq!(
            canonicalize_next_action_text("STICKY_MODE_AUDIT"),
            "ACTION_PREFLIGHT CAPABILITY_MAP STICKY_MODE_AUDIT",
        );
    }

    #[test]
    fn gemma4_observed_composed_explore_alias_maps_to_protected_preflight() {
        assert_eq!(
            canonicalize_next_action_text(
                "EXPLORE, DECOMPOSE \\lambda_1 \\AND\\ TRACE_BRIDGE \\lambda_4-decay",
            ),
            "ACTION_PREFLIGHT DECOMPOSE lambda1/lambda4 bridge trace",
        );
    }

    #[test]
    fn gemma4_observed_comma_bundle_alias_maps_to_protected_preflight() {
        assert_eq!(
            canonicalize_next_action_text("EXPLORE, READ_MORE, ANALYZE_AUDIO"),
            "ACTION_PREFLIGHT READ_MORE",
        );
    }

    #[test]
    fn gemma4_observed_authority_preflight_alias_maps_to_protected_preflight() {
        assert_eq!(
            canonicalize_next_action_text(
                "EXPERIMENT_AUTHORITY_PREFLIGHT <exp_astrid_20260603_cascade-control-stage-semantic-question-how-can>",
            ),
            "ACTION_PREFLIGHT EXPERIMENT_CHARTER current",
        );
    }

    #[test]
    fn gemma4_observed_resonance_density_alias_maps_to_protected_preflight() {
        assert_eq!(
            canonicalize_next_action_text(
                "EXPLORE_RESONANCE_DENSITY AND FORM <spectral_map> of the contracting phase.",
            ),
            "ACTION_PREFLIGHT RESONANCE_FORECAST resonance-density contracting phase",
        );
    }

    #[test]
    fn gemma4_observed_resonance_forecast_alias_maps_to_cartography_action() {
        assert_eq!(
            canonicalize_next_action_text("EXPLORE_RESONANCE_FORECAST"),
            "RESONANCE_FORECAST",
        );
        assert_eq!(
            canonicalize_next_action_text("EXPLORE_RESONANCE_FORECAST λ4 shoulder"),
            "RESONANCE_FORECAST λ4 shoulder",
        );
    }

    #[test]
    fn gemma4_observed_shadow_trajectory_suffix_maps_to_cartography_action() {
        assert_eq!(
            canonicalize_next_action_text("SHADOW_TRAJECTORY_expansion_gradient"),
            "SHADOW_TRAJECTORY expansion gradient",
        );
    }

    // v4.0 Phase 2 — Conflict guard tests.

    #[test]
    fn is_parameter_decision_verb_recognizes_short_and_long_forms() {
        // Short bare aliases.
        assert!(is_parameter_decision_verb("ACCEPT"));
        assert!(is_parameter_decision_verb("DEFER"));
        assert!(is_parameter_decision_verb("REJECT"));
        // Medium aliases.
        assert!(is_parameter_decision_verb("ACCEPT_REQUEST"));
        assert!(is_parameter_decision_verb("DEFER_REQUEST"));
        assert!(is_parameter_decision_verb("REJECT_REQUEST"));
        // Full forms.
        assert!(is_parameter_decision_verb("ACCEPT_PARAMETER_REQUEST"));
        assert!(is_parameter_decision_verb("DEFER_PARAMETER_REQUEST"));
        assert!(is_parameter_decision_verb("REJECT_PARAMETER_REQUEST"));
    }

    #[test]
    fn is_parameter_decision_verb_rejects_unrelated_actions() {
        assert!(!is_parameter_decision_verb("EXAMINE"));
        assert!(!is_parameter_decision_verb("BROWSE"));
        assert!(!is_parameter_decision_verb("REVIEW_PARAMETER_REQUESTS"));
        assert!(!is_parameter_decision_verb("ACCEPT_ATTRACTOR_SUGGESTION"));
        assert!(!is_parameter_decision_verb("READ_MORE"));
        assert!(!is_parameter_decision_verb(""));
    }
}
