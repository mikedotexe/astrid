use crate::{Catalog, Page, digest};
use serde::{Deserialize, Serialize};
use std::fmt::Write as _;

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub(crate) struct Notebook {
    note: Option<Entry>,
    question: Option<Entry>,
    previous: Option<Entry>,
    #[serde(default)]
    recent: Vec<Entry>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
struct Entry {
    origin: String,
    response_sha256: String,
    text: String,
    /// True only when all visible prose survived recording and rendering.
    #[serde(default)]
    complete: bool,
    #[serde(default)]
    prose_bytes: usize,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    reopen: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    resume: Option<String>,
}

impl Notebook {
    pub(crate) fn study_choices(&self, catalog: &Catalog, page: Option<&Page>) -> String {
        let mut out = String::new();
        let mut question_sources = Vec::new();
        if let Some(question) = self.question_text() {
            let _ = writeln!(
                out,
                "YOUR CURRENT QUESTION — {question}\nIf this reading changes your answer, you can save the finding with STUDY_NOTE: and revise STUDY_QUESTION: (or use - to clear it). These are optional; your prose can develop the answer freely."
            );
            let quoted = question
                .split('`')
                .skip(1)
                .step_by(2)
                .filter(|value| !value.is_empty() && value.len() <= 300)
                .collect::<Vec<_>>();
            let context_sources = page
                .map(|page| page.source.as_str())
                .into_iter()
                .chain(
                    self.question
                        .iter()
                        .chain(self.note.iter())
                        .chain(self.previous.iter())
                        .chain(self.recent.iter().rev())
                        .filter_map(source_from_reopen),
                )
                .collect::<Vec<_>>();
            for reference in quoted
                .iter()
                .copied()
                .filter(|value| source_reference(value))
            {
                let direct = catalog.resolve(reference).ok().into_iter();
                let sibling = context_sources.iter().filter_map(|context| {
                    let (parent, _) = context.rsplit_once('/')?;
                    catalog.resolve(&format!("{parent}/{reference}")).ok()
                });
                for source in direct.chain(sibling) {
                    if !question_sources.contains(&source.id) {
                        question_sources.push(source.id);
                    }
                    if question_sources.len() == 2 {
                        break;
                    }
                }
                if question_sources.len() == 2 {
                    break;
                }
            }
            for source in &question_sources {
                let _ = writeln!(
                    out,
                    "Open a source named in this question (exact catalog path; no source bytes are supplied until you choose it): SELF_STUDY OPEN {source} 1"
                );
            }
            for symbol in quoted
                .into_iter()
                .filter(|s| {
                    !s.is_empty()
                        && s.len() <= 160
                        && s.bytes().enumerate().all(|(i, b)| {
                            b == b'_' || b.is_ascii_alphabetic() || (i > 0 && b.is_ascii_digit())
                        })
                })
                .take(2)
            {
                let _ = writeln!(
                    out,
                    "Find this question's symbol: SELF_STUDY RELATE {symbol}"
                );
            }
        }
        let current = page.map(|p| format!("SELF_STUDY OPEN {} {}", p.source, p.start.line));
        let mut targets = Vec::new();
        for target in current.iter().chain(
            self.previous
                .iter()
                .chain(self.recent.iter().rev())
                .filter_map(|e| e.reopen.as_ref()),
        ) {
            if target.len() <= 300 && !targets.contains(target) {
                targets.push(target.clone());
            }
            if targets.len() == 2 {
                break;
            }
        }
        if targets.len() == 2 && question_sources.is_empty() {
            let _ = writeln!(
                out,
                "Compare recent source locations in one turn (current checkout, smaller pages): SELF_STUDY SESSION {} | {}",
                targets[0].trim_start_matches("SELF_STUDY "),
                targets[1].trim_start_matches("SELF_STUDY ")
            );
        }
        if !out.is_empty() {
            out.push('\n');
        }
        out
    }

    pub(crate) fn question_text(&self) -> Option<&str> {
        self.question.as_ref().map(|e| e.text.as_str())
    }

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
            complete: text.len() <= limit,
            prose_bytes: text.len(),
            reopen: page.map(|p| format!("SELF_STUDY OPEN {} {}", p.source, p.start.line)),
            resume: page.map(|p| format!("SELF_STUDY RESUME {}", p.source)),
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
        for original in visible.lines() {
            let line = original.trim();
            if line.starts_with("```") || line.starts_with("~~~") {
                fenced = !fenced;
            }
            if !fenced && let Some(value) = line.strip_prefix("STUDY_NOTE:") {
                self.note = update(value, &entry, 1600);
            } else if !fenced && let Some(value) = line.strip_prefix("STUDY_QUESTION:") {
                self.question = update(value, &entry, 500);
            } else if !line.starts_with("NEXT:") {
                prose.push(original);
            }
        }
        let prose = prose.join("\n");
        if !prose.trim().is_empty() {
            let latest = entry(prose.trim(), 64_000);
            if self
                .previous
                .as_ref()
                .is_none_or(|p| p.response_sha256 != latest.response_sha256)
            {
                if let Some(previous) = self.previous.take() {
                    self.recent.push(previous);
                    if self.recent.len() > 3 {
                        self.recent.remove(0);
                    }
                }
                self.previous = Some(latest);
            }
        }
    }

    pub(crate) fn render(&self) -> String {
        if self.note.is_none() && self.question.is_none() && self.previous.is_none() {
            return String::new();
        }
        let mut view = self.clone();
        // Bound the serialized value without ever cutting JSON syntax or an exact path.
        let serialized = loop {
            let rendered = serde_json::to_string(&view).expect("notebook strings serialize");
            if rendered.len() <= 32_000 {
                break rendered;
            }
            // Prefer complete recent answers. Drop the oldest whole account
            // before excerpting anything; always keep the current question/note.
            if !view.recent.is_empty() {
                view.recent.remove(0);
                continue;
            }
            let mut changed = false;
            for item in [&mut view.previous, &mut view.note, &mut view.question]
                .into_iter()
                .flatten()
            {
                if item.text.len() > 64 {
                    item.text = bounded(&item.text, (item.text.len() / 2).max(64));
                    item.complete = false;
                    changed = true;
                    break;
                }
                if item.reopen.is_some() || item.resume.is_some() {
                    item.reopen = None;
                    item.resume = None;
                    changed = true;
                    break;
                }
                if item.origin.len() > 100 {
                    item.origin = bounded(&item.origin, 100);
                    changed = true;
                    break;
                }
            }
            if !changed {
                break r#"{"note":null,"question":null,"previous":null}"#.into();
            }
        };
        format!(
            "\n\nRECALLED ACCOUNT — your study notebook contains recent visible responses and your saved findings, not source supplied this turn or verified code facts. Recent accounts are oldest first; previous is the latest. complete=false marks an excerpt, never a full answer. It may contain mistakes or truncated context. Source references identify the input behind the earlier account; they do not validate its symbols, line claims or conclusions. A reopen link marks the page behind that account; the question's link is where it was asked, not a known answer location. Reopen checks current source; resume continues the bookmark. Missing fields mean no note was saved.\n{serialized}\nEnd of study notebook.\n"
        )
    }
}

fn source_from_reopen(entry: &Entry) -> Option<&str> {
    let rest = entry.reopen.as_deref()?.strip_prefix("SELF_STUDY OPEN ")?;
    let (source, line) = rest.rsplit_once(' ')?;
    line.parse::<usize>().ok()?;
    Some(source)
}

fn source_reference(value: &str) -> bool {
    !value
        .bytes()
        .any(|byte| byte.is_ascii_whitespace() || matches!(byte, b'|' | b'"' | b'\'' | b'<' | b'>'))
        && (value.contains('/')
            || [".rs", ".py", ".md", ".toml", ".json", ".txt"]
                .iter()
                .any(|suffix| value.ends_with(suffix)))
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
    const MARKER: &str = "\n[excerpt truncated; middle omitted]\n";
    if text.len() <= limit {
        return text.into();
    }
    let room = limit.saturating_sub(MARKER.len());
    let head = text.floor_char_boundary(room / 2);
    let tail = text.ceil_char_boundary(text.len().saturating_sub(room.saturating_sub(head)));
    format!("{}{}{}", &text[..head], MARKER, &text[tail..])
}
