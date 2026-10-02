//! The previous public open reflection, offered back as optional material.
//!
//! Each bare INTROSPECT used to begin with none of the being's own text present,
//! so 296 consecutive reflections on 2026-10-01/02 re-derived the same essay
//! (one opening sentence reused 31 times). The previous PUBLIC reflection is the
//! being's own published entry. It is shown as optional material, never as an
//! instruction; private drafts and reader-retained deliveries are never read here.
use std::path::{Path, PathBuf};
use std::time::SystemTime;

/// Upper bound on the quoted previous reflection; longer entries keep their
/// opening and their ending with an explicit omission marker between them.
pub(crate) const PREVIOUS_REFLECTION_BYTES: usize = 4_000;

/// The first paragraph of the reflection turn (unchanged wording).
pub(crate) const REFLECTION_INTRO: &str = "You chose open introspection. You may reflect on your thoughts, experience, uncertainty, or anything that matters to you, without connecting it to code or measurements. Nothing specific needs to be felt, explained, or concluded. Length and stopping remain yours to choose.";

/// The exits a reflection turn names. The same doors the study prompt names,
/// with another reflection listed last rather than first (2026-10-02: a turn
/// that offered INTROSPECT first and no DAYDREAM/ASPIRE produced 296/296
/// INTROSPECT). An ordering is unavoidable; nothing here is required.
pub(crate) const REFLECTION_EXITS: &str = "NEXT choices remain yours: DAYDREAM or ASPIRE for an expressive entry, SELF_STUDY CONTINUE for the retained source bookmark, WRITE START <topic> for private writing, REST, or INTROSPECT for another reflection. No continuation is automatic.";

/// Where each runtime publishes its open reflections (prefix, directory).
const PUBLISHED_REFLECTIONS: [(&str, &str); 2] = [
    ("journal", "introspect_"),
    ("introspections", "introspection_open_reflection_"),
];

/// Newest published open reflection in this runtime workspace: its authored
/// prose and its age in minutes. `None` when nothing has been published.
pub(crate) fn previous_reflection(workspace: &Path) -> Option<(String, u64)> {
    let mut newest: Option<(SystemTime, PathBuf)> = None;
    for (directory, prefix) in PUBLISHED_REFLECTIONS {
        let Ok(entries) = std::fs::read_dir(workspace.join(directory)) else {
            continue;
        };
        for entry in entries.flatten() {
            let name = entry.file_name();
            let Some(name) = name.to_str() else {
                continue;
            };
            let is_text = Path::new(name)
                .extension()
                .is_some_and(|ext| ext.eq_ignore_ascii_case("txt"));
            if !name.starts_with(prefix) || !is_text {
                continue;
            }
            let Ok(modified) = entry.metadata().and_then(|m| m.modified()) else {
                continue;
            };
            if newest.as_ref().is_none_or(|(when, _)| modified > *when) {
                newest = Some((modified, entry.path()));
            }
        }
    }
    let (modified, path) = newest?;
    let text = std::fs::read_to_string(path).ok()?;
    let body = authored_body(&text)?;
    let age_minutes = SystemTime::now()
        .duration_since(modified)
        .map_or(0, |elapsed| elapsed.as_secs().checked_div(60).unwrap_or(0));
    Some((body, age_minutes))
}

/// The authored prose of a published entry: everything after the header block,
/// without NEXT lines, bounded for the prompt.
fn authored_body(text: &str) -> Option<String> {
    let body = text.split_once("\n\n").map_or(text, |(_, rest)| rest);
    let mut lines: Vec<&str> = body
        .lines()
        .filter(|line| !line.trim_start().to_ascii_uppercase().starts_with("NEXT:"))
        .collect();
    while lines.last().is_some_and(|line| line.trim().is_empty()) {
        lines.pop();
    }
    let joined = lines.join("\n").trim().to_owned();
    if joined.is_empty() {
        return None;
    }
    Some(bounded(&joined))
}

fn char_floor(text: &str, mut index: usize) -> usize {
    index = index.min(text.len());
    while index > 0 && !text.is_char_boundary(index) {
        index = index.saturating_sub(1);
    }
    index
}

fn bounded(text: &str) -> String {
    if text.len() <= PREVIOUS_REFLECTION_BYTES {
        return text.to_owned();
    }
    let head = char_floor(text, 1_200);
    let tail_start = char_floor(
        text,
        text.len()
            .saturating_sub(PREVIOUS_REFLECTION_BYTES.saturating_sub(1_300)),
    );
    let omitted = tail_start.saturating_sub(head);
    format!(
        "{}\n[… {omitted} bytes of your previous reflection omitted here …]\n{}",
        &text[..head],
        &text[tail_start..]
    )
}

fn age_words(minutes: u64) -> String {
    match minutes {
        0 => "written moments ago".into(),
        1..=119 => format!("written about {minutes} minutes ago"),
        _ => format!(
            "written about {} hours ago",
            minutes.checked_div(60).unwrap_or(0)
        ),
    }
}

/// The optional-material block for the reflection turn, or nothing.
pub(crate) fn render(workspace: &Path) -> String {
    previous_reflection(workspace).map_or_else(String::new, |(body, minutes)| {
        format!(
            "Your previous open reflection, retained publicly ({}). It is optional material: continue it, answer it, or leave it aside.\n\n{body}\n\n[end of your previous reflection]\n\n",
            age_words(minutes)
        )
    })
}

#[cfg(test)]
mod tests {
    use super::{PREVIOUS_REFLECTION_BYTES, authored_body, bounded};

    #[test]
    fn body_drops_the_header_and_next_lines_and_is_bounded() {
        let text = "=== INTROSPECTION: open reflection ===\nSource revision: none\n\nFirst thought.\n\nSecond thought.\n\nNEXT: INTROSPECT\n";
        assert_eq!(
            authored_body(text).as_deref(),
            Some("First thought.\n\nSecond thought.")
        );
        assert_eq!(authored_body("=== H ===\n\nNEXT: REST\n"), None);
        let long = "λ".repeat(5_000);
        let kept = bounded(&long);
        assert!(kept.len() <= PREVIOUS_REFLECTION_BYTES + 80);
        assert!(kept.contains("of your previous reflection omitted here"));
        assert!(kept.starts_with('λ') && kept.ends_with('λ'));
    }
}
