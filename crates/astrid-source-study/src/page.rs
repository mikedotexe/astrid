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
    pub byte: usize,
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
        let header = |id: &str, end: usize| {
            format!(
                "SOURCE {}\nRevision sha256:{}; {} bytes; {} lines. Local checkout source; deployment and understanding are not established.\nPage {}\nExact source bytes {}..{}; line fragments retain their line number.\n\n{}\n",
                source.id,
                revision.sha256,
                revision.bytes,
                revision.lines,
                id,
                start.byte,
                end,
                scope
            )
        };
        let overhead = header(&"0".repeat(64), text.len())
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
        let links =
            crate::source_links::render(&outline, &source.id, start.byte, end.byte, links_budget);
        let rendered = format!(
            "{}{body}\n{}\n{links}{navigation}",
            header(&id, end.byte),
            if eof { "End of file." } else { footer }
        );
        if rendered.len() > budget {
            bail!("source page exceeds the protected delivery budget");
        }
        let source_locations = outline
            .declarations
            .iter()
            .filter(|declaration| {
                declaration.name_start_byte >= start.byte
                    && declaration.name_end_byte <= end.byte
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
            .collect();
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
        let room = allowance
            .saturating_sub(body.len())
            .saturating_sub(prefix.len())
            .saturating_sub(1);
        let wanted = rest.find('\n').map_or(rest.len(), |n| n.saturating_add(1));
        let take = rest.floor_char_boundary(wanted.min(room));
        if take == 0 {
            break;
        }
        let fragment = &rest[..take];
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
