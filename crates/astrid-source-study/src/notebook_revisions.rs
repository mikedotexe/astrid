//! Voluntary note revision. Old accounts and exact supplied anchors survive.
use super::{Entry, Notebook};
use crate::{Page, notebook_findings::Anchor};
use serde::{Deserialize, Serialize};

const MAX_CHANGES: usize = 64;

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct NoteChange {
    previous: Option<Entry>,
    replacement: Option<Entry>,
    counterevidence: Option<Anchor>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Revision {
    prior: String,
    text: String,
    source: String,
    line: usize,
}

impl Notebook {
    pub(crate) fn validate_notes(&self) -> anyhow::Result<()> {
        anyhow::ensure!(
            self.note_history.len() <= MAX_CHANGES,
            "note history exceeds its bound; checkpoint preserved"
        );
        for changes in self.note_history.windows(2) {
            anyhow::ensure!(
                changes[0].replacement == changes[1].previous,
                "note revision chain differs; checkpoint preserved"
            );
        }
        if let Some(last) = self.note_history.last() {
            anyhow::ensure!(
                last.replacement == self.note,
                "current note differs from revision history; checkpoint preserved"
            );
        }
        Ok(())
    }
    pub(super) fn change_note(&mut self, replacement: Option<Entry>, anchor: Option<Anchor>) {
        if anchor.is_none()
            && self.note.as_ref().map(|e| &e.text) == replacement.as_ref().map(|e| &e.text)
        {
            return;
        }
        if self.note_history.len() >= MAX_CHANGES {
            self.note_feedback = Some("Not saved: 64 note changes retained; nothing evicted. The prior note remains unchanged and this response remains in its delivery receipt.".into());
            return;
        }
        self.note_history.push(NoteChange {
            previous: self.note.clone(),
            replacement: replacement.clone(),
            counterevidence: anchor,
        });
        self.note = replacement;
        self.note_feedback = Some("Saved authored note change with prior account retained. This does not verify a claim or resolve an inquiry. SELF_STUDY NOTE opens the history.".into());
    }

    pub(super) fn revise_note(
        &mut self,
        value: &str,
        entry: &impl Fn(&str, usize) -> Entry,
        pages: &[Page],
    ) {
        let result = (|| -> anyhow::Result<_> {
            let revision: Revision = serde_json::from_str(value)?;
            anyhow::ensure!(
                self.note
                    .as_ref()
                    .is_some_and(|note| note.response_sha256 == revision.prior),
                "prior note identity differs; reopen SELF_STUDY NOTE"
            );
            anyhow::ensure!(
                !revision.text.trim().is_empty() && revision.text.len() <= 1600,
                "revision needs 1..1600 bytes of authored text"
            );
            let anchor =
                crate::notebook_findings::current_anchor(&revision.source, revision.line, pages)?;
            Ok((entry(&revision.text, 1600), anchor))
        })();
        match result {
            Ok((replacement, anchor)) => self.change_note(Some(replacement), Some(anchor)),
            Err(error) => {
                self.note_feedback = Some(format!(
                    "Revision not saved: {error}. Prior account unchanged."
                ));
            },
        }
    }

    pub(crate) fn note_view(&self, page: usize) -> anyhow::Result<String> {
        anyhow::ensure!(page > 0, "note history pages start at 1");
        let start = page.saturating_sub(1).saturating_mul(2);
        anyhow::ensure!(
            start < self.note_history.len().max(1),
            "note history page unavailable"
        );
        let changes = self
            .note_history
            .iter()
            .rev()
            .skip(start)
            .take(2)
            .collect::<Vec<_>>();
        let value = serde_json::json!({
            "scope":"Owner's authored note and revisions; source anchors establish supplied fragments, not correctness. No inquiry selected, resolved, or scheduled.",
            "current_note":self.note,
            "changes_newest_first":changes,
            "total_changes":self.note_history.len(),
            "page":page,
            "more":start.saturating_add(changes.len()) < self.note_history.len(),
            "revision_command":"STUDY_REVISE: {\"prior\":\"current note response_sha256\",\"text\":\"your revision\",\"source\":\"repository/path\",\"line\":1}",
            "evidence_requirement":"The cited numbered line must be supplied in the revision turn. OPEN or SESSION can supply it. Old source references are not current evidence.",
            "navigation":"SELF_STUDY NOTE --page N"
        });
        Ok(serde_json::to_string_pretty(&value)?)
    }
}
