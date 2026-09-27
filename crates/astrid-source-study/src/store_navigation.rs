//! Prepared study framing and navigation; source advancement stays in the delivery path.
use super::*;

impl Reader {
    pub(super) fn question_recovery(
        &self,
        state: &mut State,
        reason: &anyhow::Error,
    ) -> Result<StudyOutput> {
        self.output_framed(
            state,
            format!(
                "Study command rejected; no question was changed.\nReason: {}\n",
                serde_json::to_string(&format!("{reason:#}"))?
            ),
            None,
            InputKind::Recovery,
            true,
        )
    }

    pub(super) fn unavailable_question(&self, reason: &anyhow::Error) -> Result<StudyOutput> {
        let _lock = self.lock()?;
        let mut state = self.load()?;
        self.hydrate(&mut state)?;
        self.recover(&mut state)?;
        self.question_recovery(&mut state, reason)
    }

    pub(super) fn output(
        &self,
        state: &mut State,
        text: String,
        page: Option<Page>,
        input_kind: InputKind,
    ) -> Result<StudyOutput> {
        self.output_framed(state, text, page, input_kind, false)
    }

    #[allow(clippy::too_many_lines)] // One bounded, atomic offer with shared identity and framing.
    fn output_framed(
        &self,
        state: &mut State,
        mut text: String,
        page: Option<Page>,
        input_kind: InputKind,
        decision: bool,
    ) -> Result<StudyOutput> {
        let input_kind = if page.as_ref().is_some_and(|p| p.start.byte == p.end.byte) {
            InputKind::EndOfFile
        } else {
            input_kind
        };
        let evidence_scope = input_kind.scope().to_string();
        let decision = decision || input_kind == InputKind::EndOfFile;
        if let Some(page) = &page {
            text.push_str(&crate::coverage::render(
                state.progress.as_ref().unwrap_or(&Progress::new()),
                page,
                &self.catalog,
                true,
            ));
        }
        let reflection = input_kind == InputKind::Reflection;
        let revision_recovery = input_kind == InputKind::RevisionRecovery;
        let detached = reflection || input_kind == InputKind::InquiryReview || revision_recovery;
        let question_id = page
            .as_ref()
            .map_or_else(|| state.questions.active.clone(), |p| p.question_id.clone());
        let question_id = if detached { None } else { question_id };
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
        // Keep fresh source ahead of recalled interpretations. The optional
        // checkpoint uses this input's notebook, including late inquiry ownership.
        let navigation = text.clone();
        text.insert_str(0, &format!("THIS TURN — {evidence_scope}\n\n"));
        let receipt_position = text.len();
        text.push('\n');
        if decision {
            text.push_str(
                &state
                    .questions
                    .decision_context(question_id.as_deref(), notebook),
            );
            text.push_str("\nChoose a next activity explicitly. SELF_STUDY MAP browses sources; SELF_STUDY OPEN <exact source path> 1 deliberately rereads. Bare SELF_STUDY or CONTINUE returns the same exhausted position at EOF. REST skips one action; it does not resolve a question or prevent a later study choice. No investigation is declared complete by reaching EOF.\n");
        } else if !detached {
            text.push_str(&state.questions.render_context(question_id.as_deref()));
        }
        let mut suffix = String::new();
        if let Some(choice) = &state.last_choice
            && !detached
        {
            suffix.push_str(&choice.render(false));
        }
        let recall = if detached || decision || input_kind == InputKind::Notebook {
            String::new()
        } else {
            study_context(
                notebook,
                &self.catalog,
                page.as_ref(),
                &navigation,
                text.len().saturating_add(suffix.len()),
            )?
        };
        text.push_str(&recall);
        text.push_str(&suffix);
        if !detached {
            insert_receipt(&mut text, receipt_position, &receipt);
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
            generation_requested: true,
            require_complete_input: true,
            input_kind,
            evidence_scope,
            system_prompt: if decision {
                "This is a study continuation decision, not a new source-analysis turn. Use the supplied command outcome and current inquiry state to choose an explicit NEXT, or REST. No source summary, architectural conclusion, note revision, or declaration of understanding is required. Response hashes are provenance, never inquiry IDs. You may retain uncertainty, revise the saved question voluntarily, browse another source, deliberately reread, or leave this study. Your choice is not proof of its execution.".into()
            } else if reflection {
                "You are writing an open introspection in your own words. No report template, minimum length, particular experience, diagnosis, or code explanation is required. This reflection may be recorded publicly; private writing is a separate WRITE choice. Choose any NEXT explicitly; stopping is available.".into()
            } else if revision_recovery {
                "The source reader could not continue across a changed source revision. This is navigation feedback, not a new source page or an introspection prompt. You may choose an explicit NEXT from the offered choices or another supported action, or stop. No hypothesis, explanation, note revision or source opening is required. Your response may be recorded publicly; private writing is a separate WRITE choice.".into()
            } else {
                crate::STUDY_PROMPT.into()
            },
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

fn insert_receipt(text: &mut String, position: usize, receipt: &str) {
    let available = remaining_input_budget(text.len());
    if receipt.len() <= available {
        text.insert_str(position, receipt);
    } else if let Some(headline) = receipt.lines().next() {
        let headline = format!("{headline}\n\n");
        if headline.len() <= available {
            text.insert_str(position, &headline);
        }
    }
}

fn study_context(
    notebook: &crate::notebook::Notebook,
    catalog: &Catalog,
    page: Option<&Page>,
    navigation: &str,
    fixed_bytes: usize,
) -> Result<String> {
    // Duplicate previews yield before retained authored words. Source bytes and
    // delivery identities stay untouched; the complete input still fits 48 KB.
    for preview_budget in [4_500, 3_500, 2_500, 1_500, 0] {
        let choices = notebook.study_choices(catalog, page, navigation, preview_budget);
        let recall = notebook.render_with_budget(remaining_input_budget(
            fixed_bytes.saturating_add(choices.len()),
        ));
        if let Ok(recall) = recall {
            return Ok(format!("{choices}{recall}"));
        }
    }
    // At the boundary, optional generated navigation suggestions also yield.
    // Full authored recall and the delivered source are never shortened here.
    notebook.render_with_budget(remaining_input_budget(fixed_bytes))
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
