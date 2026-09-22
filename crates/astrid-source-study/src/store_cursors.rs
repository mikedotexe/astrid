//! Per-question selection; global delivery evidence is never restored from a snapshot.
use super::{Reader, State, StudyOutput};
use crate::{Page, questions::QuestionCommand};
use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

#[derive(Clone, Default, Serialize, Deserialize)]
pub(super) struct ReadingCursor {
    current: Option<String>,
    bookmarks: BTreeMap<String, Page>,
    pending: Option<Page>,
    pending_page_output: Option<StudyOutput>,
    pending_page_context_stale: bool,
    pending_session: Option<StudyOutput>,
    pending_navigation: Option<StudyOutput>,
}

impl ReadingCursor {
    pub(super) fn validate_position(&self, catalog: &crate::Catalog) -> Result<()> {
        for page in self
            .pending
            .iter()
            .chain(self.current.as_ref().and_then(|s| self.bookmarks.get(s)))
            .chain(self.pending_session.iter().flat_map(|s| &s.session_pages))
        {
            let source = catalog.resolve(&page.source)?;
            anyhow::ensure!(
                crate::digest(std::fs::read(&source.path)?) == page.revision.sha256,
                "reading source changed; explicitly OPEN the source again before returning"
            );
        }
        Ok(())
    }
    pub(super) fn capture(state: &State) -> Self {
        Self {
            current: state.current.clone(),
            bookmarks: state.bookmarks.clone(),
            pending: state.pending.clone(),
            pending_page_output: state.pending_page_output.clone(),
            pending_page_context_stale: state.pending_page_context_stale,
            pending_session: state.pending_session.clone(),
            pending_navigation: state.pending_navigation.clone(),
        }
    }
    fn restore(&self, state: &mut State) {
        state.current.clone_from(&self.current);
        state.bookmarks.clone_from(&self.bookmarks);
        state.pending.clone_from(&self.pending);
        state
            .pending_page_output
            .clone_from(&self.pending_page_output);
        state.pending_page_context_stale = self.pending_page_context_stale;
        state.pending_session.clone_from(&self.pending_session);
        state
            .pending_navigation
            .clone_from(&self.pending_navigation);
    }
    pub(super) fn offers(&self, id: &str) -> bool {
        self.pending.as_ref().is_some_and(|p| p.id == id)
            || self
                .pending_session
                .as_ref()
                .into_iter()
                .chain(self.pending_navigation.as_ref())
                .any(|p| p.navigation_id.as_deref() == Some(id))
    }
}

impl State {
    pub(super) fn migrate_cursors(&mut self) {
        // Attribute only positions that already carry their owning question.
        // Old global progress cannot reconstruct every historical inquiry.
        for page in self.bookmarks.values() {
            let cursor = self
                .reading_contexts
                .entry(page.question_id.clone().unwrap_or_default())
                .or_default();
            cursor.bookmarks.insert(page.source.clone(), page.clone());
            if self.current.as_ref() == Some(&page.source) {
                cursor.current.clone_from(&self.current);
            }
        }
        if let Some(page) = &self.pending {
            let cursor = self
                .reading_contexts
                .entry(page.question_id.clone().unwrap_or_default())
                .or_default();
            cursor.pending.clone_from(&self.pending);
            cursor
                .pending_page_output
                .clone_from(&self.pending_page_output);
            cursor.pending_page_context_stale = self.pending_page_context_stale;
        }
        for (output, session) in [
            (self.pending_session.as_ref(), true),
            (self.pending_navigation.as_ref(), false),
        ] {
            if let Some(output) = output {
                let cursor = self
                    .reading_contexts
                    .entry(output.question_id.clone().unwrap_or_default())
                    .or_default();
                if session {
                    cursor.pending_session = Some(output.clone());
                } else {
                    cursor.pending_navigation = Some(output.clone());
                }
            }
        }
        self.select_cursor(self.questions.active.clone());
    }
    fn retain_cursor(&mut self) {
        self.reading_contexts.insert(
            self.cursor_owner.clone().unwrap_or_default(),
            ReadingCursor::capture(self),
        );
    }
    fn select_cursor(&mut self, owner: Option<String>) {
        let cursor = self
            .reading_contexts
            .get(owner.as_deref().unwrap_or_default())
            .cloned()
            .unwrap_or_default();
        cursor.restore(self);
        self.cursor_owner = owner;
    }
    pub(super) fn apply_question(&mut self, command: QuestionCommand) -> Result<String> {
        self.retain_cursor();
        let text = self.questions.apply(command, &mut self.notebook)?;
        if self.cursor_owner != self.questions.active {
            self.select_cursor(self.questions.active.clone());
        }
        Ok(text)
    }
    pub(super) fn retain_and_align_cursor(&mut self) {
        self.retain_cursor();
        if self.cursor_owner != self.questions.active {
            self.select_cursor(self.questions.active.clone());
        }
    }
}

impl Reader {
    pub(super) fn load_for_delivery(&self, id: &str) -> Result<State> {
        let mut state = self.load()?;
        if !ReadingCursor::capture(&state).offers(id) {
            let owner = state
                .reading_contexts
                .iter()
                .find(|(_, cursor)| cursor.offers(id))
                .map(|(owner, _)| owner.clone());
            if let Some(owner) = owner {
                state.retain_cursor();
                state.select_cursor((!owner.is_empty()).then_some(owner));
            }
        }
        Ok(state)
    }
}
