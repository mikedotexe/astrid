use crate::{MAX_PAGE_BYTES, Source, digest};
use anyhow::{Context as _, Result, bail};
use serde::{Deserialize, Serialize};
use std::fs;

/// A declaration name actually included in the numbered, delivered source.
/// This records syntax, not execution, correctness, or an authored conclusion.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct SourceLocation {
    pub line: usize,
    pub name: String,
    pub kind: String,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct SourceRevision {
    pub sha256: String,
    pub bytes: usize,
    pub lines: usize,
}
#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct Position {
    /// Byte cursor in the source revision; a page's end cursor is exclusive.
    pub byte: usize,
    /// One-based line containing the cursor, possibly the line after the last
    /// delivered row when that row ended with a newline.
    pub line: usize,
}
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct Page {
    pub id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub question_id: Option<String>,
    pub source: String,
    pub revision: SourceRevision,
    pub start: Position,
    pub end: Position,
    pub eof: bool,
    pub text: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub source_locations: Vec<SourceLocation>,
}

impl Page {
    pub(crate) fn read(
        source: &Source,
        start: Option<&Position>,
        line: usize,
        expected: Option<&str>,
    ) -> Result<Self> {
        Self::read_with_budget(source, start, line, expected, MAX_PAGE_BYTES)
    }

    pub(crate) fn read_with_budget(
        source: &Source,
        start: Option<&Position>,
        line: usize,
        expected: Option<&str>,
        budget: usize,
    ) -> Result<Self> {
        // A generous hard ceiling avoids accidental allocation of generated data.
        // Files below it remain fully navigable, including the large Python driver.
        if fs::metadata(&source.path)?.len() > 64 * 1024 * 1024 {
            bail!("source exceeds the 64 MiB text-file limit; catalog entry remains visible");
        }
        let text = fs::read_to_string(&source.path).context("source must be UTF-8 text")?;
        let revision = SourceRevision {
            sha256: digest(&text),
            bytes: text.len(),
            lines: text.lines().count(),
        };
        if expected.is_some_and(|hash| hash != revision.sha256) {
            bail!(
                "source changed since the last page; choose SELF_STUDY OPEN {} <line> to start a new revision",
                source.id
            );
        }
        let start = start_position(&text, start, line)?;
        let outline = crate::source_structure::Outline::parse(&source.id, &text);
        let scope = outline.scope_text(
            &source.id,
            &text,
            start.byte,
            if budget < MAX_PAGE_BYTES { 550 } else { 850 },
        );
        let links_budget = if budget < MAX_PAGE_BYTES { 400 } else { 700 };
        let footer =
            "More source follows; CONTINUE resumes at the exact next byte after verified delivery.";
        let navigation = "Navigation: SELF_STUDY CONTINUE | SELF_STUDY MAP | SELF_STUDY FIND <literal text> | SELF_STUDY OPEN repository/path <line>\n";
        // Reserve actual metadata bytes, a maximum-length range end, and the
        // optional source links before choosing the immutable source interval.
        let header = |id: &str, end: usize, line_interval: &str| {
            format!(
                "SOURCE {}\nRevision sha256:{}; {} bytes; {} lines. Local checkout source; deployment and understanding are not established.\nPage {}\nExact source bytes {}..{} (end exclusive). {}\n\n{}\n",
                source.id,
                revision.sha256,
                revision.bytes,
                revision.lines,
                id,
                start.byte,
                end,
                line_interval,
                scope
            )
        };
        // Both line bounds fit within the extent even when a saved cursor is
        // inside a line. Reserve the longest fragment wording before paging.
        let max_line_interval = format!(
            "Delivered source lines {}–{} (inclusive; partial lines explicitly marked). Declaration spans below are separate metadata.",
            start.line,
            revision.lines.max(start.line)
        );
        let overhead = header(&"0".repeat(64), text.len(), &max_line_interval)
            .len()
            .saturating_add(footer.len())
            .saturating_add(navigation.len())
            .saturating_add(links_budget)
            .saturating_add(4);
        let (end, body) = numbered_source(&text, &start, budget.saturating_sub(overhead));
        let eof = end.byte == text.len();
        if end.byte == start.byte && !eof {
            bail!(
                "source identity and scope leave no room in this page budget; open this source separately"
            );
        }
        let id = digest(format!(
            "{}:{}:{}:{}",
            source.id, revision.sha256, start.byte, end.byte
        ));
        let end_scope = outline.page_end_text(start.byte, end.byte, links_budget.min(300));
        let links = crate::source_links::render(
            &outline,
            &source.id,
            start.byte,
            end.byte,
            links_budget.saturating_sub(end_scope.len()),
        );
        let rendered = format!(
            "{}{body}\n{}{end_scope}\n{links}{navigation}",
            header(&id, end.byte, &delivered_lines(&text, &start, &end)),
            if eof { "End of file." } else { footer }
        );
        if rendered.len() > budget {
            bail!("source page exceeds the protected delivery budget");
        }
        let source_locations = delivered_locations(&outline, start.byte, end.byte);
        Ok(Self {
            id,
            question_id: None,
            source: source.id.clone(),
            revision,
            start,
            end,
            eof,
            text: rendered,
            source_locations,
        })
    }
}

fn delivered_locations(
    outline: &crate::source_structure::Outline,
    start: usize,
    end: usize,
) -> Vec<SourceLocation> {
    outline
        .declarations
        .iter()
        .filter(|declaration| {
            declaration.name_start_byte >= start
                && declaration.name_end_byte <= end
                && declaration.name_start_byte < declaration.name_end_byte
        })
        .take(64)
        .map(|declaration| SourceLocation {
            line: declaration.name_line,
            name: declaration.name.clone(),
            kind: declaration.test_context.as_ref().map_or_else(
                || declaration.kind.into(),
                |context| format!("{}; {context}", declaration.kind),
            ),
        })
        .collect()
}

fn start_position(text: &str, start: Option<&Position>, line: usize) -> Result<Position> {
    let start = if let Some(start) = start {
        start.clone()
    } else {
        let byte = if line <= 1 {
            0
        } else {
            text.match_indices('\n')
                .nth(line.saturating_sub(2))
                .map(|(offset, _)| offset.saturating_add(1))
                .context("requested line is past the end of this source")?
        };
        Position {
            byte,
            line: line.max(1),
        }
    };
    if start.byte > text.len() || !text.is_char_boundary(start.byte) {
        bail!("invalid saved source position");
    }
    Ok(start)
}

fn numbered_source(text: &str, start: &Position, allowance: usize) -> (Position, String) {
    let mut end = start.clone();
    let mut body = String::new();
    while end.byte < text.len() && body.len() < allowance {
        let rest = &text[end.byte..];
        let prefix = format!("{:>6} | ", end.line);
        let wanted = rest.find('\n').map_or(rest.len(), |n| n.saturating_add(1));
        let began_before_page =
            end.byte > 0 && text.as_bytes()[end.byte.saturating_sub(1)] != b'\n';
        let complete_marker = if began_before_page {
            fragment_marker(end.line, true, false)
        } else {
            String::new()
        };
        let available = allowance.saturating_sub(body.len());
        let whole_size = prefix
            .len()
            .saturating_add(wanted)
            .saturating_add(usize::from(!rest[..wanted].ends_with('\n')))
            .saturating_add(complete_marker.len());
        let (take, marker) = if whole_size <= available {
            (wanted, complete_marker)
        } else if !body.is_empty() {
            // Leave an ordinary next line intact for the next page. Filling
            // spare bytes is not worth turning a function name into a suffix.
            break;
        } else {
            // A line longer than this page's entire body allowance must still
            // be reachable. Mark bounded fragments outside the source gutter.
            let marker = fragment_marker(end.line, began_before_page, true);
            let room = available
                .saturating_sub(prefix.len())
                .saturating_sub(marker.len())
                .saturating_sub(1);
            (rest.floor_char_boundary(wanted.min(room)), marker)
        };
        if take == 0 {
            break;
        }
        let fragment = &rest[..take];
        body.push_str(&marker);
        body.push_str(&prefix);
        body.push_str(fragment);
        if !fragment.ends_with('\n') {
            body.push('\n');
        }
        end.byte = end.byte.saturating_add(take);
        if fragment.ends_with('\n') {
            end.line = end.line.saturating_add(1);
        }
    }
    (end, body)
}

fn fragment_marker(line: usize, began_before_page: bool, continues: bool) -> String {
    format!(
        "[Partial source line {line}; {}; {}. This row is not a complete line.]\n",
        if began_before_page {
            "began before this page"
        } else {
            "begins here"
        },
        if continues {
            "continues on next page"
        } else {
            "ends here"
        }
    )
}

fn delivered_lines(text: &str, start: &Position, end: &Position) -> String {
    if start.byte == end.byte {
        return "No source lines delivered (empty byte interval).".into();
    }
    let last_byte = text.as_bytes()[end.byte.saturating_sub(1)];
    let last_line = end.line.saturating_sub(usize::from(last_byte == b'\n'));
    let partial_start = start.byte > 0 && text.as_bytes()[start.byte.saturating_sub(1)] != b'\n';
    let partial_end = end.byte < text.len() && last_byte != b'\n';
    format!(
        "Delivered source lines {}–{last_line} (inclusive; {}). Declaration spans below are separate metadata.",
        start.line,
        if partial_start || partial_end {
            "partial lines explicitly marked"
        } else {
            "complete lines"
        }
    )
}

#[cfg(test)]
mod boundary_tests {
    use super::*;

    #[test]
    fn ordinary_lines_wait_for_a_fresh_page_instead_of_splitting_a_symbol() {
        let source =
            "first line\nlet entropy_mult = semantic_context_persistence_multiplier(pressure);\n";
        let start = Position { byte: 0, line: 1 };
        let (end, body) = numbered_source(source, &start, 80);
        assert_eq!(end, Position { byte: 11, line: 2 });
        assert_eq!(body, "     1 | first line\n");
        let (last, next) = numbered_source(source, &end, 80);
        assert_eq!(last.byte, source.len());
        assert!(next.contains("semantic_context_persistence_multiplier"));
        assert!(!next.contains("Partial source line"));
        assert_eq!(
            delivered_lines(source, &end, &last),
            "Delivered source lines 2–2 (inclusive; complete lines). Declaration spans below are separate metadata."
        );
    }

    #[test]
    fn old_mid_line_cursor_resumes_without_rewinding_or_hiding_the_fragment() {
        let source = "before\nlet entropy_mult = semantic_context_persistence_multiplier(pressure);\nafter\n";
        let byte = source.find("antic_context").unwrap();
        let start = Position { byte, line: 2 };
        let (end, body) = numbered_source(source, &start, 400);
        assert_eq!(end.byte, source.len());
        assert!(body.contains("Partial source line 2; began before this page; ends here"));
        assert!(body.contains("     2 | antic_context_persistence_multiplier(pressure);\n"));
        assert!(!body.contains("     2 | semantic_context"));
        assert!(delivered_lines(source, &start, &end).contains("partial lines explicitly marked"));
    }

    #[test]
    fn oversized_utf8_line_walks_exact_bytes_with_marked_first_middle_and_last_fragments() {
        let source = format!("{}\nlast", "🦀".repeat(500));
        let mut start = Position { byte: 0, line: 1 };
        let mut saw_middle = false;
        let mut saw_end = false;
        while start.byte < source.len() {
            let (end, body) = numbered_source(&source, &start, 300);
            assert!(end.byte > start.byte);
            assert!(body.len() <= 300);
            assert!(source.is_char_boundary(end.byte));
            assert!(body.contains("Partial source line 1"));
            if start.byte == 0 {
                assert!(body.contains("begins here; continues on next page"));
            } else if end.line == 1 {
                saw_middle = true;
                assert!(body.contains("began before this page; continues on next page"));
            } else {
                saw_end = true;
                assert!(body.contains("began before this page; ends here"));
                assert!(body.contains("     2 | last\n"));
            }
            start = end;
        }
        assert!(saw_middle && saw_end);
    }

    #[test]
    fn empty_final_newline_missing_newline_and_crlf_line_intervals_are_exact() {
        for (source, last_line, cursor_line) in [
            ("", 0, 1),
            ("\n", 1, 2),
            ("a\nb", 2, 2),
            ("a\nb\n", 2, 3),
            ("a\r\nb\r\n", 2, 3),
        ] {
            let start = Position { byte: 0, line: 1 };
            let (end, body) = numbered_source(source, &start, 400);
            assert_eq!(end.byte, source.len());
            assert_eq!(end.line, cursor_line);
            let label = delivered_lines(source, &start, &end);
            if source.is_empty() {
                assert!(body.is_empty());
                assert!(label.contains("No source lines delivered"));
            } else {
                assert!(label.contains(&format!("lines 1–{last_line}")));
                assert!(label.contains("complete lines"));
            }
            if source.contains("\r\n") {
                assert_eq!(body, "     1 | a\r\n     2 | b\r\n");
            }
        }
    }

    #[test]
    fn insufficient_fragment_budget_never_returns_an_unlabelled_fragment() {
        let source = "x".repeat(2_000);
        let start = Position { byte: 0, line: 1 };
        let (end, body) = numbered_source(&source, &start, 40);
        assert_eq!(end, start);
        assert!(body.is_empty());
    }
}
