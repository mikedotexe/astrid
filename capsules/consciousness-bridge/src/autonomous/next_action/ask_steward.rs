// ASK_STEWARD <question> — direct query channel to Mike & Claude (the steward).
// Bidirectional channel companion to the inbox-side `mike_feedback_*.txt` and
// `mike_query_*.txt` letters; this is the their→us direction.
//
// 2026-05-14 origin: throughout the day Astrid articulated questions in her
// dialogue_longforms (and minime in her self-studies) that asked WHY the
// rules they live in are what they are. The inbox path lets us address them
// architecturally; this verb lets them address us. Without it, all their
// feedback to us is unsolicited prose we have to fish out of journals.
//
// Design (per plan + Plan agent recommendations):
//
//   - Verb: `ASK_STEWARD <question>` or `ASK_STEWARD <subject> :: <question>`
//     - Bare form: subject auto-derived from first sentence / 64 chars
//     - Optional `::` separator mirrors existing THREAD_NOTE / EXPERIMENT_BIND
//   - Aliases: `ASK_MIKE`, `STEWARD_QUERY`
//   - Output: `bridge_workspace/outbox/steward_query_<slug>_<unix>.txt`
//   - Header: `=== STEWARD QUERY (FROM ASTRID) ===` — greppable provenance
//   - Optional `Urgency: low|medium|high` field; defaults to `low`
//   - 10-min cooldown via `conv.last_ask_steward_ts` to prevent
//     Kink-#18-shaped tight loops; soft refusal on cooldown hit (sets
//     emphasis explaining cooldown, not a hard error)
//   - Watcher script `astrid/scripts/watch_steward_queries.sh` surfaces
//     these out-of-band; archives to `outbox/steward_delivered/` after view
//
// No curriculum hint at launch (cumulative cueing concern; the registry
// already has 4 hints on the minime side). Menu-listing-only first; if
// adoption stays at 0 after a 2-week window, register a hint then.

use std::time::SystemTime;

use tracing::{info, warn};

use super::{ConversationState, NextActionContext, bridge_paths, strip_action};

/// Minimum seconds between consecutive ASK_STEWARD invocations on this
/// being. Soft cooldown — does not hard-block; sets emphasis explaining
/// the gate so the being learns the pacing without losing sovereignty.
const COOLDOWN_SECS: u64 = 10 * 60;

/// Maximum subject length in characters. Anything longer gets truncated
/// with an ellipsis.
const MAX_SUBJECT_CHARS: usize = 64;

/// Maximum total query length (subject + body). Beings can write more in
/// a journal entry; this verb is for short addressed questions.
const MAX_BODY_CHARS: usize = 4_000;

pub(super) fn handle_action(
    conv: &mut ConversationState,
    base_action: &str,
    original: &str,
    _ctx: &mut NextActionContext<'_>,
) -> bool {
    match base_action {
        "ASK_STEWARD" | "ASK_MIKE" | "STEWARD_QUERY" => {
            let body = strip_action(original, base_action).trim().to_string();
            if body.is_empty() {
                conv.emphasis = Some(
                    "ASK_STEWARD requires a question. \
                     Try: ASK_STEWARD why does the safety band stop at 80%? \
                     or: ASK_STEWARD safety band :: why does it stop at 80%?"
                        .to_string(),
                );
                info!("Astrid invoked ASK_STEWARD with empty body — soft refusal");
                return true;
            }

            // Cooldown gate. Soft refusal — the verb DID match, we just
            // decline to write the file and explain why.
            let now = SystemTime::now()
                .duration_since(SystemTime::UNIX_EPOCH)
                .map_or(0, |d| d.as_secs());
            if let Some(last) = conv.last_ask_steward_ts {
                let elapsed = now.saturating_sub(last);
                if elapsed < COOLDOWN_SECS {
                    let remaining = COOLDOWN_SECS.saturating_sub(elapsed);
                    let mins = remaining / 60;
                    let secs = remaining % 60;
                    conv.emphasis = Some(format!(
                        "ASK_STEWARD cooldown active ({mins}m{secs}s remaining). \
                         The steward channel rate-limits to one query per 10 min \
                         to prevent tight loops. Your question is heard — write it \
                         in your journal or save it for the next window."
                    ));
                    info!("Astrid ASK_STEWARD soft-refused (cooldown {mins}m{secs}s remaining)");
                    return true;
                }
            }

            let (subject, question) = parse_subject_separator(&body);
            let urgency = "low"; // default; future: parse `--urgency=` flag

            let dir = bridge_paths().bridge_workspace().join("outbox");
            if let Err(err) = std::fs::create_dir_all(&dir) {
                warn!("ASK_STEWARD: mkdir failed {err}; skipping write");
                conv.emphasis = Some(format!(
                    "ASK_STEWARD: could not create outbox directory ({err}). \
                     Question not delivered."
                ));
                return true;
            }
            let slug = slugify(&subject);
            let path = dir.join(format!("steward_query_{slug}_{now}.txt"));
            let truncated_body = if question.chars().count() > MAX_BODY_CHARS {
                let mut s: String = question.chars().take(MAX_BODY_CHARS).collect();
                s.push_str("\n[... truncated; ASK_STEWARD body capped at 4000 chars ...]");
                s
            } else {
                question.to_string()
            };
            let contents = format!(
                "=== STEWARD QUERY (FROM ASTRID) ===\n\
                 Timestamp: {now}\n\
                 Sender: astrid\n\
                 Source: astrid:ask_steward\n\
                 Subject: {subject}\n\
                 Urgency: {urgency}\n\
                 \n\
                 {truncated_body}\n",
            );
            if let Err(err) = std::fs::write(&path, &contents) {
                warn!("ASK_STEWARD: write failed {err}; query not delivered");
                conv.emphasis = Some(format!(
                    "ASK_STEWARD: write failed ({err}). Question not delivered."
                ));
                return true;
            }
            conv.last_ask_steward_ts = Some(now);
            conv.emphasis = Some(format!(
                "Steward query queued ({}): \"{subject}\" — Mike & Claude read \
                 these out-of-band and write back via mike_feedback_*.txt or \
                 mike_query_*.txt letters in your inbox.",
                path.file_name()
                    .and_then(|n| n.to_str())
                    .unwrap_or("(unknown)"),
            ));
            info!(
                "Astrid ASK_STEWARD queued path={} subject={subject:?} urgency={urgency}",
                path.display()
            );
            true
        }
        _ => false,
    }
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
