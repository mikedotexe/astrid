//! Shared, non-executing interpretation of response command lines and delivery evidence.
use crate::{digest, store::completion_text};
use anyhow::Result;
use serde::{Deserialize, Serialize};

/// Indices of unquoted top-level lines. Unclosed fences remain data through EOF.
#[must_use]
pub fn eligible_choice_line_indices(text: &str) -> Vec<usize> {
    let mut eligible = Vec::new();
    let mut fence: Option<(char, usize)> = None;
    let mut hidden_tag = None;
    for (index, line) in text.lines().enumerate() {
        if hidden_tag.is_some() {
            scan_internal_blocks(line, &mut hidden_tag);
            continue;
        }
        let indentation = &line[..line.len().saturating_sub(line.trim_start().len())];
        if line.starts_with("    ") || indentation.contains('\t') {
            continue;
        }
        let trimmed = line.trim();
        let marker = trimmed.chars().next().filter(|c| matches!(c, '`' | '~'));
        let marker = marker.and_then(|c| {
            let count = trimmed.chars().take_while(|v| *v == c).count();
            (count >= 3).then_some((c, count))
        });
        if let Some((opening, length)) = fence {
            if let Some((closing, count)) = marker
                && closing == opening
                && count >= length
                && trimmed[count..].trim().is_empty()
            {
                fence = None;
            }
            continue;
        }
        if let Some(marker) = marker {
            fence = Some(marker);
            continue;
        }
        if !trimmed.is_empty()
            && !trimmed.starts_with(['>', '"', '\'', '`', '“', '‘'])
            && !scan_internal_blocks(line, &mut hidden_tag)
        {
            eligible.push(index);
        }
    }
    eligible
}

// These finite metadata tags already belong to provider-visible-output cleanup.
// A whole delimiter-bearing line is data; quoted/fenced examples never open a block.
fn scan_internal_blocks(mut line: &str, hidden: &mut Option<&'static str>) -> bool {
    const TAGS: [&str; 6] = [
        "think",
        "analysis",
        "thinking",
        "Thinking",
        "writing_mode",
        "denial_record",
    ];
    let mut observed = hidden.is_some();
    loop {
        if let Some(tag) = *hidden {
            let closing = format!("</{tag}>");
            let Some(index) = line.find(&closing) else {
                return true;
            };
            line = &line[index.saturating_add(closing.len())..];
            *hidden = None;
        } else {
            let first = TAGS
                .iter()
                .filter_map(|tag| line.find(&format!("<{tag}>")).map(|index| (index, *tag)))
                .min_by_key(|(index, _)| *index);
            let Some((index, tag)) = first else {
                return observed;
            };
            let opening_length = tag.len().saturating_add(2);
            line = &line[index.saturating_add(opening_length)..];
            *hidden = Some(tag);
            observed = true;
        }
    }
}

/// Eligible lines in original order, preserving their exact bytes.
#[must_use]
pub fn eligible_lines(text: &str) -> Vec<&str> {
    let lines: Vec<_> = text.lines().collect();
    eligible_choice_line_indices(text)
        .into_iter()
        .map(|i| lines[i])
        .collect()
}

/// Last explicit NEXT, not a command selected from an earlier narrative mention.
#[must_use]
pub fn final_explicit_next(text: &str) -> Option<&str> {
    eligible_lines(text).into_iter().rev().find_map(|line| {
        let line = line.trim_start();
        line.get(..5)
            .filter(|prefix| prefix.eq_ignore_ascii_case("NEXT:"))?;
        Some(line[5..].trim_start())
    })
}

fn source_command(text: &str) -> bool {
    text.strip_prefix("SELF_STUDY ").is_some_and(|rest| {
        matches!(
            rest.split_whitespace().next(),
            Some("MAP" | "FIND" | "OPEN" | "RESUME" | "CONTINUE" | "RELATE" | "SESSION" | "TRACE")
        )
    })
}

/// Only the actual final nonempty line can use the existing bare read-only affordance.
#[must_use]
pub fn final_bare_source_command(text: &str) -> Option<&str> {
    let lines: Vec<_> = text.lines().collect();
    let (index, last) = lines
        .iter()
        .enumerate()
        .rfind(|(_, line)| !line.trim().is_empty())?;
    let last = last.trim();
    (eligible_choice_line_indices(text).contains(&index) && source_command(last)).then_some(last)
}

#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct ChoiceFeedback {
    pub selected_next: Option<String>,
    pub selection_kind: Option<String>,
    pub earlier_source_command: Option<String>,
    pub recovery_commands: Vec<String>,
    pub explanation: Option<String>,
}

fn retained_action(action: &str) -> String {
    if action.len() > 1024 {
        return "[choice exceeds receipt display limit; consult retained response]".into();
    }
    let parts: Vec<_> = action.split_whitespace().collect();
    if parts
        .first()
        .is_some_and(|s| s.trim_end_matches(':').eq_ignore_ascii_case("WRITE"))
    {
        let verb = parts.get(1).map(|word| word.to_ascii_uppercase());
        if let Some(verb) = verb.filter(|verb| {
            matches!(
                verb.as_str(),
                "HELP"
                    | "LIST"
                    | "PROFILE"
                    | "START"
                    | "BRANCH"
                    | "RESUME"
                    | "READ"
                    | "CONTINUE"
                    | "REVISE"
                    | "QUESTION"
                    | "EVIDENCE"
                    | "FINISH"
            )
        }) {
            return if parts.len() > 2 {
                format!("WRITE {verb} [arguments withheld; see retained response]")
            } else {
                action.into()
            };
        }
        return "WRITE [private arguments withheld]".into();
    }
    if parts.iter().any(|s| {
        s.trim_matches(['[', ']'])
            .trim_end_matches(':')
            .eq_ignore_ascii_case("WRITE")
    }) {
        return "WRITE [private arguments withheld]".into();
    }
    action.into()
}

/// Interpret only visible response command lines. Never queues, executes or replaces a choice.
#[must_use]
pub fn inspect_response(text: &str, private_writing: bool) -> ChoiceFeedback {
    let explicit = final_explicit_next(text).map(str::trim);
    let bare = explicit
        .is_none()
        .then(|| final_bare_source_command(text))
        .flatten();
    let selected = explicit.or(bare);
    let mut result = ChoiceFeedback {
        selected_next: selected.map(retained_action),
        selection_kind: explicit
            .map(|_| "explicit_next".into())
            .or_else(|| bare.map(|_| "bare_source".into())),
        ..ChoiceFeedback::default()
    };
    if explicit.is_some() {
        // Stop at the chosen NEXT line: later examples never imply a second choice.
        let lines = eligible_lines(text);
        let selected_index = lines.iter().rposition(|line| {
            line.trim()
                .get(..5)
                .is_some_and(|s| s.eq_ignore_ascii_case("NEXT:"))
        });
        if let Some(index) = selected_index {
            let earlier = lines[..index]
                .iter()
                .rev()
                .map(|line| line.trim())
                .find(|line| source_command(line) && Some(*line) != selected);
            if let Some(earlier) = earlier {
                result.earlier_source_command = Some(retained_action(earlier));
                result.explanation = Some("An earlier standalone source command differs from the final NEXT. The final NEXT remains the selected choice; the earlier command was not substituted. You may choose it explicitly next time.".into());
            }
        }
    }
    if selected.is_some_and(|action| action.eq_ignore_ascii_case("FINISH")) && private_writing {
        result.recovery_commands.push("WRITE FINISH".into());
        result.explanation = Some("FINISH is not recognized as a writing command. It does not mark the draft finished; use NEXT: WRITE FINISH to finish the active draft, or choose another action.".into());
    } else if selected.is_none() {
        let last = text
            .lines()
            .enumerate()
            .filter(|(_, line)| !line.trim().is_empty())
            .last();
        if let Some((_, last)) =
            last.filter(|(index, _)| eligible_choice_line_indices(text).contains(index))
        {
            let candidate = last.trim();
            if let Some(query) = candidate.strip_prefix("FIND ").map(str::trim)
                && !query.is_empty()
                && query.len() <= 600
                && !query.contains(['\n', '\r', '|'])
            {
                let command = format!("SELF_STUDY FIND {query}");
                result.recovery_commands.push(command);
                result.explanation = Some("No NEXT or complete SELF_STUDY choice was selected. FIND needs the SELF_STUDY prefix for local source reading; this is a recovery option, not an executed or queued search.".into());
            }
        }
    }
    result
}

/// Bounded reference retained only after the caller verifies complete delivery.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct ChoiceReceipt {
    pub input_id: String,
    pub request_sha256: String,
    pub response_sha256: String,
    pub feedback: ChoiceFeedback,
}

impl ChoiceReceipt {
    /// The caller must verify delivery before saving this observation.
    /// # Errors
    /// Returns an error if the provider response has no readable completion field.
    pub fn from_delivery(
        input_id: &str,
        request: &str,
        response: &str,
        private_writing: bool,
    ) -> Result<Self> {
        Ok(Self {
            input_id: input_id.into(),
            request_sha256: digest(request),
            response_sha256: digest(response),
            feedback: inspect_response(&completion_text(response)?, private_writing),
        })
    }

    /// Reference-only JSON keeps command examples out of executable-looking top-level lines.
    /// # Panics
    /// Panics if the string-only receipt cannot be serialized as JSON.
    #[must_use]
    pub fn render(&self, _private_writing: bool) -> String {
        let mut view = self.clone();
        {
            for text in [
                &mut view.feedback.selected_next,
                &mut view.feedback.earlier_source_command,
            ]
            .into_iter()
            .flatten()
            {
                *text = retained_action(text);
            }
            view.feedback.recovery_commands = view
                .feedback
                .recovery_commands
                .iter()
                .map(|s| retained_action(s))
                .collect();
        }
        format!(
            "\n\nPREVIOUS RESPONSE CHOICE — reference only. The input ID and request hash identify the previous delivered request. A selected response command is not proof of queueing, dispatch or completion. The source or draft supplied in THIS input identifies what was actually prepared now. Recovery options remain your choice.\n{}\nEnd of previous response choice.\n",
            serde_json::to_string(&view).expect("choice strings serialize")
        )
    }
}
