use super::*;
use crate::observations::History;

impl Writer {
    pub(crate) fn observation_context(&self, id: &str) -> Result<(String, String, History)> {
        let _lock = self.lock()?;
        let state = self.load()?;
        let draft = state
            .drafts
            .get(id)
            .context("existing owner-scoped draft required")?;
        Ok((
            self.target_revision_locked(id)?,
            draft.parts.join("\n\n"),
            draft.observations.clone(),
        ))
    }
    pub(crate) fn save_observations(
        &self,
        id: &str,
        revision: &str,
        history: History,
    ) -> Result<()> {
        let _lock = self.lock()?;
        anyhow::ensure!(
            self.target_revision_locked(id)? == revision,
            "draft changed; explicit reselection required"
        );
        history.validate(true)?;
        let mut state = self.load()?;
        state
            .drafts
            .get_mut(id)
            .context("draft missing")?
            .observations = history;
        self.save(&state)
    }
    pub(crate) fn observation_output(
        &self,
        text: String,
        present: bool,
        action: &str,
        preview: Option<String>,
    ) -> Result<StudyOutput> {
        anyhow::ensure!(
            text.len().saturating_add(PROMPT.len()).saturating_add(32) <= crate::MAX_INPUT_BYTES,
            "observation presentation exceeds input limit; evidence preserved"
        );
        let mut state = self.load()?;
        state.sequence = state
            .sequence
            .checked_add(1)
            .context("writing sequence exhausted")?;
        let output=StudyOutput {generation_requested:present,input_kind:InputKind::PrivateWriting,
            evidence_scope:"Private observation receipt. No public delivery or inferred intent. Numerical descriptions are not felt-state conclusions.".into(),
            require_complete_input:true,system_prompt:PROMPT.into(),input_budget_bytes:crate::MAX_INPUT_BYTES,
            context_tokens:crate::CONTEXT_TOKENS,text,page:None,session_pages:Vec::new(),question_id:None,
            navigation_id:Some(format!("writing-{}",state.sequence))};
        if present {
            if let Some(previous) = state.pending.take() {
                state.retained_pending.insert(
                    previous
                        .output
                        .navigation_id
                        .clone()
                        .context("pending writing ID")?,
                    previous,
                );
            }
            let request: crate::observations::ObservationRequest = serde_json::from_str(
                action
                    .trim()
                    .strip_prefix("WRITE OBSERVE ")
                    .context("observation action required")?,
            )?;
            state.pending = Some(Pending {
                output: output.clone(),
                draft: None,
                revision: 0,
                replace: false,
                selected_action_sha256: Some(digest(action)),
                observation_draft: Some(request.draft),
                observation_preview: preview,
            });
        }
        self.save(&state)?;
        Ok(output)
    }

    pub(crate) fn verify_preview_delivery(&self, preview: &str) -> Result<()> {
        let state = self.load()?;
        let id=state.preview_deliveries.get(preview).context("preview must first be shown with present:true and verified input delivery; then confirm explicitly")?;
        let receipt = state.receipts.get(id).context("preview receipt missing")?;
        let bytes = crate::preparation::read(&receipt.artifact_path)?;
        anyhow::ensure!(
            digest(&bytes) == receipt.artifact_sha256,
            "preview delivery artifact changed"
        );
        let artifact: serde_json::Value = serde_json::from_slice(&bytes)?;
        anyhow::ensure!(
            artifact["observation_preview"].as_str() == Some(preview),
            "preview delivery identity mismatch"
        );
        let output: StudyOutput = serde_json::from_value(artifact["output"].clone())?;
        let request = artifact["request_json"]
            .as_str()
            .context("preview input missing")?;
        let response = artifact["response_json"]
            .as_str()
            .context("preview response missing")?;
        anyhow::ensure!(
            digest(request) == receipt.request_sha256
                && digest(response) == receipt.response_sha256,
            "preview wire hashes differ"
        );
        output.verify_delivery(request, response)
    }
}
