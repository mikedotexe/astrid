fn should_arm_prompt_overflow_read_more(
    active_read_path: Option<&str>,
    recent_next_choice: Option<&str>,
) -> bool {
    active_read_path.is_none()
        && !recent_next_choice.is_some_and(|choice| {
            choice
                .split_whitespace()
                .next()
                .is_some_and(|action| action.eq_ignore_ascii_case("READ_MORE"))
        })
}

fn peer_source_study_context(source: &str) -> String {
    format!(
        "The text above is Minime's authored source-study account from {source}. \
         Its input/delivery header identifies what source, navigation or recovery was supplied; \
         delivery does not verify the account's code claims. It is not automatically a report \
         of Minime's current felt condition or of your own experience. Explicit experiences, \
         suggestions, uncertainty and objections remain Minime's words. You may respond to \
         what interests you, check a claim, disagree, or leave it aside; no diagnosis or \
         co-regulation response is required."
    )
}

#[cfg(test)]
mod peer_source_study_context_tests {
    use super::peer_source_study_context;

    #[test]
    fn source_account_is_attributed_without_prescribing_present_experience() {
        let hint = peer_source_study_context("self_study_fixture.txt");
        assert!(hint.contains("Minime's authored source-study account from self_study_fixture.txt"));
        assert!(hint.contains("delivery does not verify"));
        assert!(hint.contains("navigation or recovery"));
        assert!(hint.contains("objections remain Minime's words"));
        assert!(hint.contains("no diagnosis or co-regulation response is required"));
        assert!(!hint.contains("grounded in minime's present condition"));
    }
}

fn fill_responsive_rest_secs(base_rest: u64, current_fill: f32) -> u64 {
    if current_fill < 30.0 {
        ((base_rest as f64 * 0.6) as u64).max(30)
    } else if current_fill < 40.0 {
        base_rest
    } else if current_fill < 50.0 {
        ((base_rest as f64 * 1.2) as u64).min(MAX_REST_SECS)
    } else {
        base_rest
    }
}

struct DialogueCollaborationDeliveryV1 {
    offer: Option<crate::autonomous::next_action::collaboration_attention::CollaborationPromptOfferV1>,
    submission: Option<crate::llm::ContextSubmissionTrackerV1>,
}

impl DialogueCollaborationDeliveryV1 {
    fn prepare(
        checkpoint: &mut crate::autonomous::next_action::collaboration_attention::CollaborationPromptCheckpointV1,
        eligible: bool,
    ) -> Self {
        let offer = eligible
            .then(|| {
                crate::autonomous::next_action::collaboration_attention::prepare_prompt_offer(
                    checkpoint,
                )
            })
            .flatten();
        let submission = offer
            .as_ref()
            .filter(|offer| offer.marker.is_some())
            .map(|offer| crate::llm::ContextSubmissionTrackerV1::new(offer.content.clone()));
        Self { offer, submission }
    }

    fn context(&self) -> Option<&str> {
        self.offer.as_ref().map(|offer| offer.content.as_str())
    }

    fn submission(&self) -> Option<&crate::llm::ContextSubmissionTrackerV1> {
        self.submission.as_ref()
    }

    fn retry_context(&self) -> Option<&str> {
        self.context()
            .filter(|_| !self.submission().is_some_and(crate::llm::ContextSubmissionTrackerV1::submitted))
    }

    fn finish(&self, conv: &mut ConversationState) {
        let Some(offer) = self.offer.as_ref() else {
            return;
        };
        crate::autonomous::next_action::collaboration_attention::finish_prompt_offer(
            &mut conv.collaboration_prompt_checkpoint,
            offer,
            self.submission()
                .is_some_and(crate::llm::ContextSubmissionTrackerV1::submitted),
        );
        // Persist before later turn work can fail, avoiding replay after restart.
        save_state(conv);
    }
}
