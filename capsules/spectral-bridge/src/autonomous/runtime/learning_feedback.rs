const LEARNING_RETIREMENT_TOPIC: &str = "consciousness.v1.hebbian_outcome_retirement";

fn commit_learning_plan(
    conv: &mut ConversationState,
    candidate: learning_outcomes::HebbianOutcomeQueue,
    retired: &[learning_outcomes::OutcomeRetirement],
    db: &BridgeDb,
) -> bool {
    if !retired.is_empty() {
        let payload = serde_json::json!({
            "schema": "hebbian_outcome_retirement_v1",
            "actor": "bridge_learning_feedback",
            "authority": "evidence_only_not_causal_attribution",
            "live_control_authority": false,
            "retired_count": retired.len(),
            "outcomes": retired.iter().take(16).collect::<Vec<_>>(),
            "outcome_details_truncated": retired.len() > 16,
        });
        if let Err(error) = db.log_message(
            crate::types::MessageDirection::OperatorProbe,
            LEARNING_RETIREMENT_TOPIC,
            &payload.to_string(),
            None,
            None,
            None,
        ) {
            warn!(%error, "learning retirement evidence failed; queue and consumption unchanged");
            return false;
        }
    }
    conv.hebbian_outcomes = candidate;
    true
}

fn prepare_hebbian_feedback(
    conv: &mut ConversationState,
    observation: Option<crate::learning_clock::LearningObservation>,
    fill_pct: f32,
    db: &BridgeDb,
) -> Option<crate::learning_clock::LearningObservation> {
    let observation = crate::learning_target::attach(observation, conv.remote_workspace.as_deref());
    update_hebbian_feedback(conv, observation, fill_pct, db);
    observation
}

fn update_hebbian_feedback(
    conv: &mut ConversationState,
    observation: Option<crate::learning_clock::LearningObservation>,
    fill_pct: f32,
    db: &BridgeDb,
) {
    conv.hebbian_codec.decay_scores();
    let mut candidate = conv.hebbian_outcomes.clone();
    let result = candidate.take(observation, std::time::Instant::now());
    if commit_learning_plan(conv, candidate, &result.retired, db)
        && let Some(pending) = result.ready
        && let Some(target) = pending.observation.and_then(|sample| sample.target)
    {
        let learned = (fill_pct - pending.fill_before).abs() >= 1.0
            && conv.hebbian_codec.observe_outcome(
                &pending.signature,
                pending.fill_before,
                fill_pct,
                target.fill_pct,
            );
        info!(
            exchange_count = pending.exchange_count,
            baseline_t_ms = pending.telemetry_t_ms_before,
            observed_t_ms = observation.map(|sample| sample.producer_t_ms),
            fill_before = pending.fill_before,
            fill_after = fill_pct,
            target_fill_pct = target.fill_pct,
            target_source = target.source,
            learned,
            "Hebbian outcome evaluated; temporal association, not causal attribution"
        );
    }
}

fn finalize_semantic_exchange(
    conv: &mut ConversationState,
    exchange_codec_signature: Option<Vec<f32>>,
    fill_before: f32,
    observation: Option<crate::learning_clock::LearningObservation>,
    sent_semantic_chunk: bool,
    db: &BridgeDb,
) {
    if !sent_semantic_chunk {
        return;
    }
    if let Some(signature) = exchange_codec_signature {
        let mut candidate = conv.hebbian_outcomes.clone();
        let retired = candidate.arm(
            conv.exchange_count,
            signature.clone(),
            fill_before,
            observation,
            std::time::Instant::now(),
        );
        commit_learning_plan(conv, candidate, &retired, db);
        conv.glimpse_12d =
            crate::codec::GlimpseCodec::derive_12d(&signature).map(|glimpse| glimpse.to_vec());
        conv.last_exchange_codec_signature = Some(signature);
    }
}

#[cfg(test)]
#[path = "learning_feedback_tests.rs"]
mod learning_feedback_tests;
