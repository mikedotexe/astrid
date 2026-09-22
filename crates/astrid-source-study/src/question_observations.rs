use super::*;
use crate::observations::{Destination, Disclosure, Entry, History};
use anyhow::ensure;

impl Questions {
    pub(crate) fn observation_destination(&self, destination: &Destination) -> Result<String> {
        match destination {
            Destination::Existing { question, revision } => {
                ensure!(
                    self.target_revision(question)? == *revision,
                    "inquiry changed; preview again"
                );
                Ok(self
                    .entries
                    .get(question)
                    .context("existing inquiry required")?
                    .question
                    .clone())
            },
            Destination::New { question } => {
                crate::observations::text(question, 350)?;
                ensure!(
                    self.entries.len() < 32,
                    "32 inquiries retained; no eviction"
                );
                Ok(question.clone())
            },
        }
    }
    pub(crate) fn observations(&mut self, id: &str) -> Result<(&str, &mut History)> {
        let inquiry = self
            .entries
            .get_mut(id)
            .context("existing owner-scoped inquiry required")?;
        Ok((&inquiry.question, &mut inquiry.observations))
    }
    pub(crate) fn confirm_observation(
        &mut self,
        owner: &str,
        request_id: &str,
        destination: &Destination,
        payload: Disclosure,
    ) -> Result<(String, String)> {
        ensure!(
            self.observation_destination(destination)? == payload.question,
            "destination differs from preview"
        );
        let id = match destination {
            Destination::Existing { question, .. } => question.clone(),
            Destination::New { question } => {
                self.next = self.next.checked_add(1).context("question IDs exhausted")?;
                let id = format!("q{}", self.next);
                self.entries.insert(
                    id.clone(),
                    Inquiry {
                        question: question.clone(),
                        status: "open".into(),
                        finding: String::new(),
                        notebook: Notebook::default(),
                        sources: Vec::new(),
                        geometry: crate::geometry::History::default(),
                        observations: History::default(),
                    },
                );
                id
            },
        };
        let history = &mut self
            .entries
            .get_mut(&id)
            .context("inquiry missing")?
            .observations;
        // Caller-chosen operation IDs and private request fingerprints are not disclosure material.
        let public_request = format!("disclosure-{}", crate::digest(request_id));
        let public_fingerprint = crate::digest(serde_json::to_vec(&payload)?);
        let record = history.append(
            owner,
            &public_request,
            &public_fingerprint,
            Entry::Disclosure { payload },
            false,
        )?;
        Ok((id, record))
    }
    pub(crate) fn observation_export(&self, owner: &str, id: &str) -> Result<Vec<u8>> {
        let inquiry = self.entries.get(id).context("inquiry missing")?;
        inquiry.observations.validate(false)?;
        inquiry.observations.check_owner(owner)?;
        inquiry.geometry.validate()?;
        inquiry.geometry.check_owner(owner)?;
        let body_json = serde_json::to_string(
            &serde_json::json!({"format":"inquiry-observations-v2","owner":owner,"question_id":id,"question":inquiry.question,"geometry_v1":inquiry.geometry,"observations":inquiry.observations,"limits":crate::recurrence::LIMITS}),
        )?;
        Ok(serde_json::to_vec_pretty(
            &serde_json::json!({"format":"inquiry-observations-v2","body_sha256":crate::digest(&body_json),"body_json":body_json}),
        )?)
    }
    pub(crate) fn has_observations(&self, id: &str) -> bool {
        self.entries
            .get(id)
            .is_some_and(|q| !q.observations.records.is_empty())
    }
}
