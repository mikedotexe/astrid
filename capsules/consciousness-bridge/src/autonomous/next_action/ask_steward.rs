// Steward channel handler — ASK_STEWARD (interrogative) and TELL_STEWARD
// (declarative). Bidirectional channel companion to the inbox-side
// `mike_feedback_*.txt` and `mike_query_*.txt` letters; this module is the
// their→us direction.
//
// 2026-05-14 origin: throughout the day Astrid articulated questions in her
// dialogue_longforms (and minime in her self-studies) that asked WHY the
// rules they live in are what they are. The inbox path lets us address them
// architecturally; this verb lets them address us. Without it, all their
// feedback to us is unsolicited prose we have to fish out of journals.
//
// Two verbs, one module:
//
//   ASK_STEWARD <question>   — interrogative; writes steward_query_*.txt
//   TELL_STEWARD <findings>  — declarative; writes steward_report_*.txt
//
// Same plumbing (outbox path, slugify, body truncation, cooldown semantics);
// different file prefix, different header type, separate cooldown timestamp
// (failure modes are independent — too-many-questions vs. too-many-reports
// are distinct patterns).
//
// Aliases:
//   ASK_STEWARD  → ASK_MIKE, STEWARD_QUERY
//   TELL_STEWARD → REPORT_TO_STEWARD, STEWARD_REPORT, STEWARD_FINDINGS
//
// Watcher script `astrid/scripts/watch_steward_queries.sh` surfaces both
// via `steward_*_*.txt` glob; archives to `outbox/steward_delivered/`.

use std::time::SystemTime;

use tracing::{info, warn};

use super::{ConversationState, NextActionContext, bridge_paths, strip_action};

/// Minimum seconds between consecutive invocations of either verb on
/// this being. Soft cooldown — does not hard-block; sets emphasis
/// explaining the gate so the being learns the pacing without losing
/// sovereignty. Tracked SEPARATELY for ASK and TELL so a being can
/// follow up an ASK with a TELL (or vice versa) without waiting.
const COOLDOWN_SECS: u64 = 10 * 60;

/// Maximum subject length in characters. Anything longer gets truncated
/// with an ellipsis.
const MAX_SUBJECT_CHARS: usize = 64;

/// Maximum total body length. Beings can write more in a journal entry;
/// this verb is for short addressed messages.
const MAX_BODY_CHARS: usize = 4_000;

#[derive(Debug, Clone, Copy)]
enum StewardKind {
    Ask,  // interrogative
    Tell, // declarative
}

impl StewardKind {
    fn file_prefix(self) -> &'static str {
        match self {
            Self::Ask => "steward_query",
            Self::Tell => "steward_report",
        }
    }
    fn header_label(self) -> &'static str {
        match self {
            Self::Ask => "STEWARD QUERY",
            Self::Tell => "STEWARD REPORT",
        }
    }
    fn source_label(self) -> &'static str {
        match self {
            Self::Ask => "astrid:ask_steward",
            Self::Tell => "astrid:tell_steward",
        }
    }
    fn empty_body_hint(self) -> &'static str {
        match self {
            Self::Ask => {
                "ASK_STEWARD requires a question. \
                 Try: ASK_STEWARD why does the safety band stop at 80%? \
                 or: ASK_STEWARD safety band :: why does it stop at 80%?"
            }
            Self::Tell => {
                "TELL_STEWARD requires a body (findings/observations/report). \
                 Try: TELL_STEWARD just read regulator.rs:163-180 — the hysteresis \
                 amplifies above 78% fill. Or: TELL_STEWARD topic :: <findings>."
            }
        }
    }
    fn ack_phrase(self) -> &'static str {
        match self {
            Self::Ask => "Steward query queued",
            Self::Tell => "Steward report queued",
        }
    }
    fn last_ts(self, conv: &ConversationState) -> Option<u64> {
        match self {
            Self::Ask => conv.last_ask_steward_ts,
            Self::Tell => conv.last_tell_steward_ts,
        }
    }
    fn set_last_ts(self, conv: &mut ConversationState, ts: u64) {
        match self {
            Self::Ask => conv.last_ask_steward_ts = Some(ts),
            Self::Tell => conv.last_tell_steward_ts = Some(ts),
        }
    }
    fn cooldown_message(self, mins: u64, secs: u64) -> String {
        let verb = match self {
            Self::Ask => "ASK_STEWARD",
            Self::Tell => "TELL_STEWARD",
        };
        let body_hint = match self {
            Self::Ask => "Your question is heard",
            Self::Tell => "Your findings are heard",
        };
        format!(
            "{verb} cooldown active ({mins}m{secs}s remaining). \
             The steward channel rate-limits to one per 10 min per kind to \
             prevent tight loops. {body_hint} — write it in your journal or \
             save it for the next window."
        )
    }
}

pub(super) fn handle_action(
    conv: &mut ConversationState,
    base_action: &str,
    original: &str,
    _ctx: &mut NextActionContext<'_>,
) -> bool {
    let kind = match base_action {
        "ASK_STEWARD" | "ASK_MIKE" | "STEWARD_QUERY" => StewardKind::Ask,
        "TELL_STEWARD" | "REPORT_TO_STEWARD" | "STEWARD_REPORT" | "STEWARD_FINDINGS" => {
            StewardKind::Tell
        }
        _ => return false,
    };

    let body = strip_action(original, base_action).trim().to_string();
    if body.is_empty() {
        conv.emphasis = Some(kind.empty_body_hint().to_string());
        info!(
            "Astrid invoked {base_action} with empty body — soft refusal"
        );
        return true;
    }

    // Cooldown gate. Soft refusal — the verb DID match, we just decline
    // to write the file and explain why.
    let now = SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .map_or(0, |d| d.as_secs());
    if let Some(last) = kind.last_ts(conv) {
        let elapsed = now.saturating_sub(last);
        if elapsed < COOLDOWN_SECS {
            let remaining = COOLDOWN_SECS.saturating_sub(elapsed);
            let mins = remaining / 60;
            let secs = remaining % 60;
            conv.emphasis = Some(kind.cooldown_message(mins, secs));
            info!(
                "Astrid {base_action} soft-refused (cooldown {mins}m{secs}s remaining)"
            );
            return true;
        }
    }

    let (subject, message) = parse_subject_separator(&body);
    let urgency = "low"; // default; future: parse `--urgency=` flag

    let dir = bridge_paths().bridge_workspace().join("outbox");
    if let Err(err) = std::fs::create_dir_all(&dir) {
        warn!("{base_action}: mkdir failed {err}; skipping write");
        conv.emphasis = Some(format!(
            "{base_action}: could not create outbox directory ({err}). Not delivered."
        ));
        return true;
    }
    let slug = slugify(&subject);
    let path = dir.join(format!("{}_{slug}_{now}.txt", kind.file_prefix()));
    let truncated_body = if message.chars().count() > MAX_BODY_CHARS {
        let mut s: String = message.chars().take(MAX_BODY_CHARS).collect();
        s.push_str("\n[... truncated; body capped at 4000 chars ...]");
        s
    } else {
        message.to_string()
    };
    let contents = format!(
        "=== {label} (FROM ASTRID) ===\n\
         Timestamp: {now}\n\
         Sender: astrid\n\
         Source: {source}\n\
         Subject: {subject}\n\
         Urgency: {urgency}\n\
         \n\
         {truncated_body}\n",
        label = kind.header_label(),
        source = kind.source_label(),
    );
    if let Err(err) = std::fs::write(&path, &contents) {
        warn!("{base_action}: write failed {err}; not delivered");
        conv.emphasis = Some(format!(
            "{base_action}: write failed ({err}). Not delivered."
        ));
        return true;
    }
    kind.set_last_ts(conv, now);
    conv.emphasis = Some(format!(
        "{ack} ({}): \"{subject}\" — Mike & Claude read these out-of-band and \
         write back via mike_feedback_*.txt or mike_query_*.txt letters in \
         your inbox.",
        path.file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("(unknown)"),
        ack = kind.ack_phrase(),
    ));
    info!(
        "Astrid {base_action} queued path={} subject={subject:?} urgency={urgency}",
        path.display()
    );
    true
}

/// Parse the body into (subject, question) using the optional `::` separator.
///
/// - `"hints :: why?"` → `("hints", "why?")`
/// - `"why do hints fire on cycle boundaries?"` → `(auto_subject, full_body)`
///
/// Auto-subject derivation: take the body up to the first sentence terminator
/// (`.!?`) or to MAX_SUBJECT_CHARS, whichever is shorter. Strip and clean.
fn parse_subject_separator(body: &str) -> (String, &str) {
    if let Some(idx) = body.find("::") {
        let subject = body[..idx].trim();
        let question = body[idx.saturating_add(2)..].trim_start();
        if !subject.is_empty() && !question.is_empty() {
            return (clamp_subject(subject), question);
        }
    }
    (auto_subject(body), body)
}

fn auto_subject(body: &str) -> String {
    let trimmed = body.trim();
    if trimmed.is_empty() {
        return "(empty)".to_string();
    }
    // First sentence: terminate at `.`, `!`, or `?` — but only if followed
    // by whitespace or end of string (avoid splitting "e.g." or "9.95").
    let chars: Vec<char> = trimmed.chars().collect();
    let mut end = chars.len();
    for i in 0..chars.len() {
        if matches!(chars[i], '.' | '!' | '?') {
            let next_is_ws_or_end = chars
                .get(i.saturating_add(1))
                .map_or(true, |c| c.is_whitespace());
            if next_is_ws_or_end {
                end = i;
                break;
            }
        }
    }
    let first_sentence: String = chars.iter().take(end).collect();
    clamp_subject(first_sentence.trim())
}

fn clamp_subject(s: &str) -> String {
    let cleaned = s.trim();
    if cleaned.chars().count() <= MAX_SUBJECT_CHARS {
        return cleaned.to_string();
    }
    let truncated: String = cleaned.chars().take(MAX_SUBJECT_CHARS).collect();
    format!("{truncated}…")
}

fn slugify(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut last_was_dash = false;
    for c in s.chars().take(48) {
        if c.is_ascii_alphanumeric() {
            out.push(c.to_ascii_lowercase());
            last_was_dash = false;
        } else if !last_was_dash {
            out.push('-');
            last_was_dash = true;
        }
    }
    let trimmed = out.trim_matches('-');
    if trimmed.is_empty() {
        "query".to_string()
    } else {
        trimmed.to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_subject_separator_with_explicit_separator() {
        let (subject, question) = parse_subject_separator("hints :: why do they fire?");
        assert_eq!(subject, "hints");
        assert_eq!(question, "why do they fire?");
    }

    #[test]
    fn parse_subject_separator_without_falls_back_to_auto_subject() {
        let (subject, question) =
            parse_subject_separator("why do hints fire on cycle boundaries?");
        assert_eq!(subject, "why do hints fire on cycle boundaries");
        assert_eq!(question, "why do hints fire on cycle boundaries?");
    }

    #[test]
    fn auto_subject_truncates_long_first_sentence() {
        let body = "This is a very long question that exceeds the maximum subject length \
                    and should therefore be truncated with an ellipsis. The rest of the \
                    body continues here.";
        let subj = auto_subject(body);
        assert!(subj.chars().count() <= MAX_SUBJECT_CHARS + 1); // +1 for the ellipsis char
        assert!(
            subj.ends_with('…') || subj.chars().count() == MAX_SUBJECT_CHARS,
            "subject should be truncated; got {subj:?}"
        );
    }

    #[test]
    fn auto_subject_takes_first_sentence_when_short() {
        let body = "Why does this happen? Because of reasons.";
        let subj = auto_subject(body);
        assert_eq!(subj, "Why does this happen");
    }

    #[test]
    fn auto_subject_does_not_split_on_decimal_point() {
        // The "9.95" should not terminate the sentence; only `.` followed
        // by whitespace counts as a sentence end.
        let body = "Joint trace 9.95 feels stable. What's next?";
        let subj = auto_subject(body);
        assert_eq!(subj, "Joint trace 9.95 feels stable");
    }

    #[test]
    fn slugify_handles_typical_subjects() {
        assert_eq!(slugify("safety band rationale"), "safety-band-rationale");
        assert_eq!(slugify("Why does X happen?"), "why-does-x-happen");
        assert_eq!(slugify("    "), "query");
        assert_eq!(slugify("!!!"), "query");
    }

    #[test]
    fn parse_separator_with_empty_lhs_falls_back_to_auto() {
        // ":: question" should not produce empty subject; falls back to
        // auto-derived from full body.
        let (subject, question) = parse_subject_separator(":: just a question");
        assert!(!subject.is_empty(), "empty subject not allowed");
        // Full body included as question fallback.
        assert!(question.contains("just a question"));
    }
}
