//! Prepared study framing and navigation; source advancement stays in the delivery path.
use super::*;

impl Reader {
    pub(super) fn output(
        &self,
        state: &mut State,
        mut text: String,
        page: Option<Page>,
        input_kind: InputKind,
    ) -> Result<StudyOutput> {
        let input_kind = if page.as_ref().is_some_and(|p| p.start.byte == p.end.byte) {
            InputKind::EndOfFile
        } else {
            input_kind
        };
        let evidence_scope = input_kind.scope().to_string();
        if let Some(page) = &page {
            text.push_str(&crate::coverage::render(
                state.progress.as_ref().unwrap_or(&Progress::new()),
                page,
                &self.catalog,
                true,
            ));
        }
        let question_id = page
            .as_ref()
            .map_or_else(|| state.questions.active.clone(), |p| p.question_id.clone());
        let notebook = state
            .questions
            .notebook_for(question_id.as_deref(), &state.notebook);
        let key = crate::navigation_history::question_key(
            question_id.as_deref(),
            notebook.question_text(),
        );
        let navigation_offer = crate::navigation_history::NavigationOffer::new(
            key.clone(),
            input_kind,
            &text,
            &self.catalog,
        );
        let receipt = state
            .navigation_history
            .render(&key, input_kind, &self.catalog);
        text.insert_str(
            0,
            &notebook.study_choices(&self.catalog, page.as_ref(), &text),
        );
        text.insert_str(0, &format!("THIS TURN — {evidence_scope}\n\n"));
        let receipt_position = text.len();
        let mut suffix = state.questions.render_context(question_id.as_deref());
        if let Some(choice) = &state.last_choice {
            suffix.push_str(&choice.render(false));
        }
        let notebook_budget = remaining_input_budget(text.len().saturating_add(suffix.len()));
        text.push_str(&notebook.render_with_budget(notebook_budget)?);
        text.push_str(&suffix);
        let available = remaining_input_budget(text.len());
        if receipt.len() <= available {
            text.insert_str(receipt_position, &receipt);
        } else if let Some(headline) = receipt.lines().next() {
            let headline = format!("{headline}\n\n");
            if headline.len() <= available {
                text.insert_str(receipt_position, &headline);
            }
        }
        if text
            .len()
            .saturating_add(crate::STUDY_PROMPT.len())
            .saturating_add(32)
            > crate::MAX_INPUT_BYTES
        {
            bail!("complete study input exceeds the shared provider budget; bookmark unchanged");
        }
        let mut output = StudyOutput {
            require_complete_input: true,
            input_kind,
            evidence_scope,
            system_prompt: crate::STUDY_PROMPT.into(),
            input_budget_bytes: crate::MAX_INPUT_BYTES,
            context_tokens: crate::CONTEXT_TOKENS,
            text,
            question_id,
            page,
            session_pages: Vec::new(),
            navigation_id: None,
        };
        if output.page.is_none() {
            // A deliberately intervening navigation can carry a new question or
            // finding. Keep the pending source bytes, but refresh its framing on
            // resume. Retain the old complete input for late delivery verification
            // and choice provenance. Uninterrupted retries stay byte-exact.
            state.pending_page_context_stale = true;
            state.sequence = state
                .sequence
                .checked_add(1)
                .context("source-study sequence exhausted")?;
            output.navigation_id = Some(digest(format!(
                "navigation:{}:{}",
                state.sequence, output.text
            )));
            state.pending_navigation = Some(output.clone());
        } else {
            state.pending_page_context_stale = false;
            state.pending_page_output = Some(output.clone());
        }
        remember_offer(state, &output, navigation_offer);
        self.save(state)?;
        Ok(output)
    }

    pub(super) fn map(&self, state: &State, topic: &str, page: usize) -> Result<String> {
        let progress = state
            .progress
            .as_ref()
            .context("study progress not loaded")?;
        self.catalog
            .map(topic, page, progress, state.current.as_deref())
    }

    pub(super) fn prepare_map_view(
        &self,
        state: &mut State,
        topic: &str,
        page: usize,
        recursive: bool,
    ) -> Result<StudyOutput> {
        let result = if recursive {
            let progress = state
                .progress
                .as_ref()
                .context("study progress not loaded")?;
            self.catalog
                .list(topic, page, progress, state.current.as_deref())
        } else {
            self.map(state, topic, page)
        };
        match result {
            Ok(text) => self.output(state, text, None, InputKind::Map),
            Err(error) => self.recovery_with_candidates(
                state,
                &error,
                &self.catalog.path_candidates(topic, true),
            ),
        }
    }
}

fn remaining_input_budget(text_bytes: usize) -> usize {
    crate::MAX_INPUT_BYTES.saturating_sub(
        text_bytes
            .saturating_add(crate::STUDY_PROMPT.len())
            .saturating_add(32),
    )
}

fn remember_offer(
    state: &mut State,
    output: &StudyOutput,
    navigation_offer: crate::navigation_history::NavigationOffer,
) {
    let pending_ids: Vec<_> = state
        .pending_page_output
        .iter()
        .chain(state.pending_navigation.iter())
        .chain(state.pending_session.iter())
        .filter_map(|o| {
            o.page
                .as_ref()
                .map(|p| p.id.clone())
                .or_else(|| o.navigation_id.clone())
        })
        .collect();
    state
        .choice_sequences
        .retain(|id, _| pending_ids.contains(id));
    state
        .navigation_offers
        .retain(|id, _| pending_ids.contains(id));
    if let Some(id) = output
        .page
        .as_ref()
        .map(|p| p.id.clone())
        .or_else(|| output.navigation_id.clone())
    {
        state.navigation_offers.insert(id.clone(), navigation_offer);
        state.choice_sequences.insert(id, state.sequence);
    }
}
