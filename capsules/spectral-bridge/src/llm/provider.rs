//! Provider, rendering, and fallback implementation for Astrid's LLM lane.
//!
//! Astrid reads minime's latest journal entry and spectral state, then
//! generates a genuine response via a local LLM. Dialogue prefers the coupled
//! generation server (Gemma 4 12B on port 8090), but falls back to
//! Ollama when that dedicated lane is unavailable so Astrid does not collapse
//! into static canned fallback lines.

use std::collections::HashMap;

use serde::{Deserialize, Serialize};
use sha2::{Digest as _, Sha256};
use tracing::{debug, warn};

use crate::paths::bridge_paths;
use crate::prompt_budget::PromptBudgetReport;

include!("provider/configuration.rs");
include!("provider/prompt_contracts.rs");
include!("provider/transport.rs");
include!("provider/writing_profile.rs");
include!("provider/protected_delivery.rs");
include!("provider/afterimages.rs");
include!("provider/runtime_feedback.rs");
include!("provider/provider_observation.rs");
include!("provider/generation_controls.rs");
include!("provider/provider_observation_store.rs");
include!("provider/provider_observation_tests.rs");
include!("provider/provider_execution.rs");
include!("provider/dialogue_context.rs");
include!("provider/fallback_budget.rs");
include!("provider/fallback_mapping.rs");
include!("provider/fallback_weights.rs");
include!("provider/fallback_dynamics.rs");
include!("provider/fallback_trajectory.rs");
include!("provider/fallback_evidence.rs");
include!("provider/fallback_gradient.rs");
include!("provider/fallback_rendering.rs");
include!("provider/fallback_contracts.rs");
include!("provider/diagnostic_persistence.rs");
include!("provider/generation_record.rs");
include!("provider/own_body.rs");
include!("provider/dialogue_runtime.rs");
include!("provider/dialogue_generation.rs");
include!("provider/research.rs");
include!("provider/embeddings.rs");
include!("provider/witness.rs");
include!("provider/fallback_contract_tests.rs");
include!("provider/generation_record_tests.rs");
include!("provider/witness_tests.rs");
include!("provider/generative_actions.rs");
include!("provider/source_study.rs");
include!("provider/tests.rs");
include!("provider/context_retrieval_tests.rs");
include!("provider/context_blocks.rs");
include!("provider/protected_delivery_tests.rs");
include!("provider/runtime_feedback_tests.rs");
include!("provider/afterimage_feedback_tests.rs");
include!("provider/dialogue_feedback_tests.rs");
