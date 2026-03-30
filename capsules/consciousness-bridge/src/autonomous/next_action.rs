mod audio;
mod mike;
mod modes;
mod operations;
mod sovereignty;
mod workspace;

use tokio::sync::mpsc;
use tracing::info;

use super::{ConversationState, Mode, list_directory, save_astrid_journal, truncate_str};
use crate::db::BridgeDb;
use crate::paths::bridge_paths;
use crate::types::{SensoryMsg, SpectralTelemetry};

use super::reservoir;

pub(super) struct NextActionContext<'a> {
    pub burst_count: &'a mut u32,
    pub db: &'a BridgeDb,
    pub sensory_tx: &'a mpsc::Sender<SensoryMsg>,
    pub telemetry: &'a SpectralTelemetry,
    pub fill_pct: f32,
    pub response_text: &'a str,
}

/// Parse NEXT: action from Astrid's response.
pub(crate) fn parse_next_action(text: &str) -> Option<&str> {
    for line in text.lines().rev() {
        let trimmed = line.trim();
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
            return Some(clean.trim());
        }
    }
    None
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
        .trim_end_matches(|c: char| matches!(c, '.' | ',' | ';' | ':'))
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
        .trim_start_matches(|c: char| matches!(c, '-' | '\u{2014}' | ':'))
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

fn strip_action(original: &str, prefix: &str) -> String {
    let upper = original.to_uppercase();
    if upper.starts_with(prefix) {
        // Strip the action prefix AND any trailing colon+whitespace.
        // Astrid often writes "BROWSE: https://..." or "SEARCH: topic"
        // and the colon must not be left dangling.
        original[prefix.len()..]
            .trim_start_matches(':')
            .trim()
            .to_string()
    } else {
        String::new()
    }
}

pub(super) fn handle_next_action(
    conv: &mut ConversationState,
    next_action: &str,
    mut ctx: NextActionContext<'_>,
) {
    let original = next_action.trim().to_string();
    let base_action = original
        .split(|c: char| c.is_whitespace() || c == '\u{2014}' || c == '-' || c == '<' || c == ':')
        .next()
        .unwrap_or_default()
        .to_uppercase();

    if reservoir::handle_reservoir_action(
        conv,
        base_action.as_str(),
        &original,
        ctx.telemetry,
        ctx.fill_pct,
    ) {
        return;
    }

    if workspace::handle_action(conv, base_action.as_str(), &original, next_action, &mut ctx) {
        return;
    }

    if mike::handle_action(conv, base_action.as_str(), &original, &mut ctx) {
        return;
    }

    if modes::handle_action(conv, base_action.as_str(), &original, &mut ctx) {
        return;
    }

    if audio::handle_action(conv, base_action.as_str(), &original) {
        return;
    }

    if sovereignty::handle_action(conv, base_action.as_str(), &original, &mut ctx) {
        return;
    }

    if operations::handle_action(conv, base_action.as_str(), &original, &mut ctx) {
        return;
    }

    info!("Astrid chose unknown NEXT: '{}' — not wired", original);
}
