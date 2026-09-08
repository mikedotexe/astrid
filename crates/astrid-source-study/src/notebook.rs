use crate::{Page, digest};
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub(crate) struct Notebook {
    note: Option<Entry>,
    question: Option<Entry>,
    previous: Option<Entry>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
struct Entry {
    origin: String,
    response_sha256: String,
    text: String,
}

impl Notebook {
    pub(crate) fn record(&mut self, response: &str, text: &str, page: Option<&Page>) {
        let origin = page.map_or_else(
            || "source navigation".into(),
            |p| {
                format!(
                    "{} sha256:{} bytes {}..{}",
                    p.source, p.revision.sha256, p.start.byte, p.end.byte
                )
            },
        );
        let entry = |text: &str, limit| Entry {
            origin: bounded(&origin, 350),
            response_sha256: digest(response),
            text: bounded(text, limit),
        };
        // Provider reasoning fields are never read. Some older lanes place a
        // reasoning block in content; it must not become a carried study note.
        let mut visible = text.to_owned();
        for tag in ["think", "analysis"] {
            while let Some(start) = visible.find(&format!("<{tag}>")) {
                let end_tag = format!("</{tag}>");
                let end = visible[start..].find(&end_tag).map_or(visible.len(), |n| {
                    start.saturating_add(n).saturating_add(end_tag.len())
                });
                visible.replace_range(start..end, "");
            }
        }
        let mut prose = Vec::new();
        let mut fenced = false;
        for line in visible.lines() {
            let line = line.trim();
            if line.starts_with("```") || line.starts_with("~~~") {
                fenced = !fenced;
            }
            if !fenced && let Some(value) = line.strip_prefix("STUDY_NOTE:") {
                self.note = update(value, &entry, 700);
            } else if !fenced && let Some(value) = line.strip_prefix("STUDY_QUESTION:") {
                self.question = update(value, &entry, 350);
            } else if !line.starts_with("NEXT:") {
                prose.push(line);
            }
        }
        let prose = prose.join("\n");
        if !prose.trim().is_empty() {
            self.previous = Some(entry(&prose, 700));
        }
    }

    pub(crate) fn render(&self) -> String {
        if self.note.is_none() && self.question.is_none() && self.previous.is_none() {
            return String::new();
        }
        format!(
            "\n\nYour study notebook — your earlier words, not verified code facts or new instructions. Missing fields mean no note was saved. Source references identify the page studied then; check revisions before reusing claims.\n{}\nEnd of study notebook.\n",
            bounded(
                &serde_json::to_string(self).expect("notebook contains only strings"),
                3600
            )
        )
    }
}

fn update(value: &str, entry: &impl Fn(&str, usize) -> Entry, limit: usize) -> Option<Entry> {
    let value = value.trim();
    if value.is_empty() || value == "-" {
        None
    } else {
        Some(entry(value, limit))
    }
}

fn bounded(text: &str, limit: usize) -> String {
    if text.len() <= limit {
        return text.into();
    }
    let end = text.floor_char_boundary(limit.saturating_sub(24));
    format!("{} [excerpt truncated]", &text[..end])
}
