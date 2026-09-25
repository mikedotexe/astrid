//! A changed checkout needs an explicit new selection, not a cursor transplant.
use crate::{Page, SourceRevision};
use std::fmt::{self, Write as _};

#[derive(Debug)]
pub(crate) struct SourceRevisionChanged {
    pub source: String,
    pub previous_sha256: String,
    pub current: SourceRevision,
}

impl fmt::Display for SourceRevisionChanged {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "source changed since the last page: {}", self.source)
    }
}

impl std::error::Error for SourceRevisionChanged {}

impl SourceRevisionChanged {
    pub(crate) fn render(&self, bookmark: &Page) -> String {
        let mut text = format!(
            "SOURCE REVISION CHANGED\nSource: {}\nSaved revision sha256:{}\nObserved checkout revision sha256:{} ({} bytes; {} lines).\nSaved next position: line {}, byte {}, in the OLD revision only. Its location in the changed source is unknown.\n\nYour bookmark, supplied passages and authored notes remain retained. No pending source page exists for this target. CONTINUE or RESUME of this bookmark cannot advance while the revision differs.\n\nAvailable explicit choices (none executed):\nSELF_STUDY OPEN {} 1\n",
            self.source,
            self.previous_sha256,
            self.current.sha256,
            self.current.bytes,
            self.current.lines,
            bookmark.end.line,
            bookmark.end.byte,
            self.source
        );
        for location in bookmark.source_locations.iter().rev().take(2) {
            if !location.name.is_empty()
                && location.name.len() <= 128
                && location
                    .name
                    .chars()
                    .all(|c| c.is_alphanumeric() || c == '_')
            {
                let _ = writeln!(text, "SELF_STUDY FIND {}", location.name);
            }
        }
        text.push_str("Search terms above, when present, are names from the previously supplied page, not verified locations in the changed source. OPEN starts a new revision from its explicitly selected line; it does not certify continuity with the old offset.\nSELF_STUDY MAP\nSELF_STUDY NOTE\nINTROSPECT\nREST\nYou may choose a new source position, inspect retained notes, do something else, or stop. No selection is automatic; delivery of this recovery does not advance source coverage or revise the inquiry.\n");
        text
    }
}
