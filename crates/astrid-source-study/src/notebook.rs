use crate::notebook_findings::Findings;
use crate::question_sources::source_reference;
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
    #[serde(default, skip_serializing_if = "Findings::is_empty")]
    source_findings: Findings,
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
    pub(crate) fn study_choices(
        &self,
        catalog: &Catalog,
        page: Option<&Page>,
        navigation: &str,
    ) -> String {
        let mut out = self.checkpoint_context();
        let mut has_question_source = false;
        if let Some(question) = self.question_text() {
            let quoted = inquiry_terms(question);
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
            let (source_choices, has_source) = crate::question_sources::render_choices(
                catalog,
                question,
                &quoted,
                self.question.as_ref().and_then(source_from_reopen),
                &context_sources,
            );
            out.push_str(&source_choices);
            has_question_source = has_source;
            for term in quoted
                .into_iter()
                .filter(|term| {
                    !source_reference(term) && (identifier(term) || dotted_literal(term))
                })
                .take(2)
            {
                append_lookup_choice(&mut out, navigation, term);
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
        if targets.len() == 2 && !has_question_source {
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

    /// Render existing authored state together without synthesizing conclusions,
    /// classifying their truth, or changing the inquiry's status or scheduling.
    fn checkpoint_context(&self) -> String {
        const MAX_CHECKPOINT_BYTES: usize = 6_000;
        const FOOTER: &str = "You can distinguish what you have established from what remains an assumption, including places already checked. Your findings remain yours to retain, qualify or revise.\n";
        if self.question.is_none() && self.note.is_none() && !self.source_findings.has_authored() {
            return String::new();
        }
        let mut out = String::from(
            "OPTIONAL STUDY CHECK-IN — fresh source or navigation is above; recalled words are below. You may take stock, keep reading, revise or park a question, or choose another activity. No answer or change of direction is required.\n",
        );
        if let Some(question) = &self.question {
            let _ = writeln!(
                out,
                "YOUR CURRENT QUESTION — {}\nThis is your saved inquiry, not evidence that its premise is true. STUDY_QUESTION: can revise it; - clears this notebook field without resolving an inquiry.",
                question.text,
            );
        }
        if let Some(note) = &self.note {
            let preview = format!(
                "YOUR SAVED NOTE — recalled account, not independently verified: {:?}\nRecorded with {}. This origin does not establish the note's claims. STUDY_NOTE: replaces it in your own words; - clears it.",
                note.text, note.origin,
            );
            // Full authored words remain protected in the notebook. Omit an
            // overlarge duplicate preview as a whole, never a misleading prefix.
            if out
                .len()
                .saturating_add(preview.len())
                .saturating_add(FOOTER.len())
                .saturating_add(500)
                <= MAX_CHECKPOINT_BYTES
            {
                out.push_str(&preview);
                out.push('\n');
            } else {
                out.push_str("YOUR SAVED NOTE — retained whole in the notebook below; the duplicate preview is omitted for input space. It remains your unverified account.\n");
            }
        }
        if self.source_findings.has_authored() {
            out.push_str(&self.source_findings.render_authored(
                MAX_CHECKPOINT_BYTES.saturating_sub(out.len().saturating_add(FOOTER.len())),
            ));
        } else {
            out.push_str("No source-linked finding is saved in this notebook. Your prose can remain freeform; STUDY_FINDING: can optionally keep a concrete conclusion beside a supplied source line.\n");
        }
        out.push_str(FOOTER);
        out
    }

    pub(crate) fn record(&mut self, response: &str, text: &str, page: Option<&Page>) {
        self.record_pages(response, text, page.map_or(&[], std::slice::from_ref));
    }

    /// The caller verifies complete delivery before recording any source or words.
    /// All pages of a verified session can support independently authored findings.
    pub(crate) fn record_pages(&mut self, response: &str, text: &str, pages: &[Page]) {
        self.source_findings.record(response, text, pages);
        let page = pages.last();
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
        let mut prose = Vec::new();
        let eligible = crate::response_choice::eligible_choice_line_indices(text);
        for (index, original) in text.lines().enumerate() {
            let line = original.trim();
            let directive = eligible.binary_search(&index).is_ok();
            if directive && let Some(value) = line.strip_prefix("STUDY_NOTE:") {
                self.note = update(value, &entry, 1600);
            } else if directive && let Some(value) = line.strip_prefix("STUDY_QUESTION:") {
                // Repeating an unchanged question while browsing elsewhere must
                // not transfer its source provenance to the detour. A deliberate
                // rewording or clear remains a new authored update.
                if self.question_text() != Some(value.trim()) {
                    self.question = update(value, &entry, 500);
                }
            } else if directive && crate::notebook_findings::is_directive(line) {
                // The source-findings receipt reports accepted and rejected updates.
            } else if !directive || !line.starts_with("NEXT:") {
                prose.push(original);
            }
        }
        let mut prose = prose.join("\n");
        // Provider reasoning fields are never read. Some older lanes place a
        // reasoning block in content; it must not become a carried study note.
        // Determine directive eligibility before cleanup, so removing an inline
        // metadata block cannot promote the rest of that line into a command.
        for tag in [
            "think",
            "analysis",
            "thinking",
            "Thinking",
            "writing_mode",
            "denial_record",
        ] {
            while let Some(start) = prose.find(&format!("<{tag}>")) {
                let end_tag = format!("</{tag}>");
                let end = prose[start..].find(&end_tag).map_or(prose.len(), |n| {
                    start.saturating_add(n).saturating_add(end_tag.len())
                });
                prose.replace_range(start..end, "");
            }
        }
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

    #[cfg(test)]
    pub(crate) fn render(&self) -> String {
        self.render_with_budget(usize::MAX)
            .expect("recorded notebook fields fit the default rendering budget")
    }

    /// Bound the complete carried notebook, including framing and valid JSON.
    /// Rendering never edits the durable notebook. Oldest whole accounts are
    /// omitted before the latest account becomes an explicitly marked excerpt.
    pub(crate) fn render_with_budget(&self, max_total_bytes: usize) -> anyhow::Result<String> {
        const HEADER: &str = "\n\nRECALLED ACCOUNT — your study notebook contains recent visible responses and your saved findings, not source supplied this turn or verified code facts. Recent accounts are oldest first; previous is the latest. complete=false marks an excerpt, never a full answer. It may contain mistakes or truncated context. Source references identify the input behind the earlier account; they do not validate its symbols, line claims or conclusions. A reopen link marks the page behind that account; the question's link is where it was asked, not a known answer location. Reopen checks current source; resume continues the bookmark. Missing fields mean no note was saved. source_findings.authored contains your unverified conclusions; each anchor preserves an actually supplied numbered line fragment, its original revision and page identity. supplied_locations are bounded lexical source-location recall, not answers or code supplied this turn. A fragment may omit surrounding scope. OPEN checks current checkout, which may differ from the saved revision.\n";
        const FOOTER: &str = "\nEnd of study notebook.\n";
        if self.note.is_none()
            && self.question.is_none()
            && self.previous.is_none()
            && self.source_findings.is_empty()
        {
            return Ok(String::new());
        }
        let Some(json_budget) =
            max_total_bytes.checked_sub(HEADER.len().saturating_add(FOOTER.len()))
        else {
            anyhow::bail!(
                "study notebook framing exceeds the remaining input budget; notebook unchanged"
            );
        };
        let json_budget = json_budget.min(32_000);
        let mut view = self.clone();
        // Bound the serialized value without ever cutting JSON syntax or an exact path.
        loop {
            let serialized = serde_json::to_string(&view)?;
            if serialized.len() <= json_budget {
                return Ok(format!("{HEADER}{serialized}{FOOTER}"));
            }
            if !view.recent.is_empty() {
                view.recent.remove(0);
                continue;
            }
            if view.source_findings.omit_oldest_location() {
                continue;
            }
            if let Some(previous) = &mut view.previous {
                if previous.text.len() > 64 {
                    previous.text = bounded(&previous.text, (previous.text.len() / 2).max(64));
                    previous.complete = false;
                    continue;
                }
                if previous.reopen.is_some() || previous.resume.is_some() {
                    previous.reopen = None;
                    previous.resume = None;
                    continue;
                }
                if previous.origin.len() > 100 {
                    previous.origin = bounded(&previous.origin, 100);
                    continue;
                }
            }
            // Preserve authored findings, the chosen note and question in full. If even their
            // minimum framing cannot fit, report failure rather than silently
            // replacing them with null or clipping serialized JSON.
            anyhow::bail!(
                "saved study findings, note, question and minimum account exceed the remaining input budget; notebook unchanged"
            );
        }
    }
}

fn source_from_reopen(entry: &Entry) -> Option<&str> {
    let rest = entry.reopen.as_deref()?.strip_prefix("SELF_STUDY OPEN ")?;
    let (source, line) = rest.rsplit_once(' ')?;
    line.parse::<usize>().ok()?;
    Some(source)
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

fn append_lookup_choice(out: &mut String, navigation: &str, symbol: &str) {
    if navigation.starts_with(&format!("Symbol relationships: {symbol}."))
        || navigation.starts_with(&format!("Literal source search: {symbol}."))
    {
        let _ = writeln!(
            out,
            "This turn already supplies lexical results for the question's identifier {symbol}. Inspect their roles and numbered source before treating a match as evidence; you remain free to reread or change direction."
        );
    } else {
        let operation = if dotted_literal(symbol) {
            "FIND"
        } else {
            "RELATE"
        };
        let _ = writeln!(
            out,
            "Optional lexical lookup for a name in your question (existence and meaning unverified): SELF_STUDY {operation} {symbol}"
        );
    }
}

/// Bounded lexical mentions for optional navigation, never verified claims.
/// Quotes are not command quoting: returned terms contain no whitespace or
/// command delimiters, and literal FIND choices use the exact unquoted term.
pub(crate) fn inquiry_terms(text: &str) -> Vec<&str> {
    let text = &text[..text.floor_char_boundary(8192)];
    let mut terms = Vec::new();
    let mut quote: Option<(char, usize)> = None;
    let mut previous = None;
    for (index, character) in text.char_indices() {
        if let Some((delimiter, start)) = quote {
            if character == delimiter {
                let value = &text[start..index];
                if technical_term(value) && !terms.contains(&value) {
                    terms.push(value);
                    if terms.len() == 8 {
                        return terms;
                    }
                }
                quote = None;
            }
        } else if matches!(character, '`' | '"' | '\'')
            && !previous.is_some_and(char::is_alphanumeric)
        {
            quote = Some((character, index.saturating_add(character.len_utf8())));
        }
        previous = Some(character);
    }
    // Event names need not be quoted. Restrict bare detection to multiple
    // dotted segments so ordinary sentence punctuation does not become a name.
    for term in text.split(|c: char| !c.is_ascii_alphanumeric() && !"_./-".contains(c)) {
        let term = term.trim_end_matches('.');
        if term.matches('.').count() >= 2 && dotted_literal(term) && !terms.contains(&term) {
            terms.push(term);
            if terms.len() == 8 {
                break;
            }
        }
    }
    terms
}

fn technical_term(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 300
        && !value.starts_with(['.', '/', '-'])
        && !value.ends_with(['.', '/'])
        && !value.contains("..")
        && value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || b"_./-".contains(&byte))
}

fn identifier(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 160
        && value.bytes().enumerate().all(|(index, byte)| {
            byte == b'_' || byte.is_ascii_alphabetic() || (index > 0 && byte.is_ascii_digit())
        })
}

fn dotted_literal(value: &str) -> bool {
    technical_term(value)
        && value.len() <= 160
        && value.contains('.')
        && !value.contains('/')
        && value.split('.').all(|part| !part.is_empty())
}

#[cfg(test)]
mod budget_tests {
    use super::Notebook;

    #[test]
    fn a_tight_budget_preserves_notes_and_marks_excerpts_without_mutating_storage() {
        let mut notebook = Notebook::default();
        let response = format!(
            "STUDY_NOTE: Keep this finding.\nSTUDY_QUESTION: Keep this question.\n{}END",
            "🦀".repeat(1800)
        );
        notebook.record("first", &response, None);
        let stored = serde_json::to_value(&notebook).unwrap();
        let full = notebook.render();
        let compact = notebook.render_with_budget(2400).unwrap();
        assert!(compact.len() <= 2400);
        assert!(compact.len() < full.len());
        assert!(compact.contains("Keep this finding."));
        assert!(compact.contains("Keep this question."));
        assert!(compact.contains("excerpt truncated; middle omitted"));
        assert!(compact.contains("\"complete\":false"));
        let body = compact.split_once('\n').unwrap().1;
        let json = body.split_once('\n').unwrap().1.split_once('\n').unwrap().1;
        let json = json.strip_suffix("\nEnd of study notebook.\n").unwrap();
        let _: serde_json::Value = serde_json::from_str(json).unwrap();
        assert_eq!(serde_json::to_value(&notebook).unwrap(), stored);
        assert!(notebook.render_with_budget(100).is_err());
        assert_eq!(serde_json::to_value(&notebook).unwrap(), stored);
        assert_eq!(Notebook::default().render_with_budget(0).unwrap(), "");
    }

    #[test]
    fn a_tight_budget_omits_location_recall_but_preserves_the_authored_finding() {
        let page: crate::Page = serde_json::from_value(serde_json::json!({
            "id":"fixture", "source":"astrid/crates/demo/src/lib.rs",
            "revision":{"sha256":"frozen-revision", "bytes":22, "lines":1},
            "start":{"byte":0, "line":1}, "end":{"byte":22, "line":2}, "eof":true,
            "text":"     1 | pub fn candidate() {}\n",
            "source_locations":[{"line":1,"name":"candidate","kind":"function"}]
        }))
        .unwrap();
        let mut notebook = Notebook::default();
        notebook.record(
            "fixture-response",
            &format!("STUDY_NOTE: Keep this note.\nSTUDY_QUESTION: Keep this question.\nSTUDY_FINDING: astrid/crates/demo/src/lib.rs:1 | {}", "A".repeat(600)),
            Some(&page),
        );
        let stored = serde_json::to_value(&notebook).unwrap();
        let limit = notebook.render().len().saturating_sub(128);
        let compact = notebook.render_with_budget(limit).unwrap();
        assert!(compact.len() <= limit);
        assert!(compact.contains("\"omitted_locations_for_input_budget\":1"));
        assert!(compact.contains(&"A".repeat(600)));
        assert!(compact.contains("Keep this note."));
        assert!(compact.contains("Keep this question."));
        assert_eq!(serde_json::to_value(&notebook).unwrap(), stored);
        assert!(notebook.render_with_budget(100).is_err());
        assert_eq!(serde_json::to_value(&notebook).unwrap(), stored);
    }
}
