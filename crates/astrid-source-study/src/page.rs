use crate::{MAX_PAGE_BYTES, Source, digest};
use anyhow::{Context as _, Result, bail};
use serde::{Deserialize, Serialize};
use std::fs;

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
        let mut end = start.clone();
        let mut body = String::new();
        let allowance = budget
            .saturating_sub(if budget < MAX_PAGE_BYTES { 900 } else { 1500 })
            .saturating_sub(source.id.len());
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
        let eof = end.byte == text.len();
        let id = digest(format!(
            "{}:{}:{}:{}",
            source.id, revision.sha256, start.byte, end.byte
        ));
        let rendered = format!(
            "SOURCE {}\nRevision sha256:{}; {} bytes; {} lines. Local checkout source; deployment and understanding are not established.\nPage {}\nExact source bytes {}..{}; line fragments retain their line number.\n\n{}\n{}\nNavigation: SELF_STUDY CONTINUE | SELF_STUDY MAP | SELF_STUDY FIND <literal text> | SELF_STUDY OPEN repository/path <line>\n",
            source.id,
            revision.sha256,
            revision.bytes,
            revision.lines,
            id,
            start.byte,
            end.byte,
            body,
            if eof {
                "End of file."
            } else {
                "More source follows; CONTINUE resumes at the exact next byte after verified delivery."
            }
        );
        if rendered.len() > budget {
            bail!("source page exceeds the protected delivery budget");
        }
        Ok(Self {
            id,
            question_id: None,
            source: source.id.clone(),
            revision,
            start,
            end,
            eof,
            text: rendered,
        })
    }
}
