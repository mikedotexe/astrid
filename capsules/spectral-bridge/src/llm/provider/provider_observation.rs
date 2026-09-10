// Steward-only evidence. No prompt text, acceptance authority, or live control.
// A dispatch receipt survives process loss; Drop records cancellation during a
// graceful future drop. Missing terminal receipts are unknown, never successes.
const PROVIDER_OBSERVATION_SCHEMA: &str = "provider_attempt_observation_v1";
const PROVIDER_RAW_MAX_BYTES: usize = 262_144;
const PROVIDER_SPOOL_MAX_BYTES: u64 = 268_435_456;
const PROVIDER_RAW_SPOOL_MAX_BYTES: u64 = 67_108_864;
const PROVIDER_SPOOL_MAX_FILES: usize = 50_000;

#[derive(Clone)]
struct ProviderObservationContext {
    store: Option<std::sync::Arc<ProviderObservationStore>>,
    generation_id: Option<String>,
    logical_attempt_index: Option<u32>,
    attempts: std::sync::Arc<std::sync::Mutex<Vec<ProviderAttemptLink>>>,
}

#[derive(Debug, Clone, Serialize)]
struct ProviderAttemptLink {
    attempt_id: String,
    outcome: &'static str,
    input_availability: &'static str,
    recording_status: &'static str,
    reported_model: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
struct ProviderGenerationObservation {
    schema: &'static str,
    status: &'static str,
    attempts: Vec<ProviderAttemptLink>,
    decision_recording_status: &'static str,
    accepted_output_sha256: Option<String>,
}

impl ProviderObservationContext {
    fn configured(generation_id: Option<&str>, logical_attempt_index: Option<u32>) -> Self {
        static STORE: std::sync::OnceLock<Option<std::sync::Arc<ProviderObservationStore>>> =
            std::sync::OnceLock::new();
        let store = STORE
            .get_or_init(|| {
                // Explicit opt-in, sampled once per process. Unit tests inject a
                // private store and can never enable the host's evidence writer.
                if cfg!(test)
                    || !matches!(
                        std::env::var("ASTRID_PROVIDER_OBSERVATION").ok().as_deref(),
                        Some("1" | "on" | "true")
                    )
                {
                    return None;
                }
                let root = std::env::var_os("ASTRID_PROVIDER_OBSERVATION_DIR")
                    .filter(|value| !value.is_empty())
                    .map(std::path::PathBuf::from)
                    .unwrap_or_else(|| {
                        bridge_paths()
                            .bridge_workspace()
                            .join("provider_observations")
                    });
                Some(std::sync::Arc::new(ProviderObservationStore::new(root)))
            })
            .clone();
        Self {
            store,
            generation_id: generation_id.map(str::to_string),
            logical_attempt_index,
            attempts: std::sync::Arc::default(),
        }
    }

    fn finish_dialogue(&self, accepted: Option<&str>) -> ProviderGenerationObservation {
        let attempts = self
            .attempts
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .clone();
        let mut result = ProviderGenerationObservation {
            schema: PROVIDER_OBSERVATION_SCHEMA,
            status: if self.store.is_some() {
                "enabled"
            } else {
                "disabled"
            },
            attempts,
            decision_recording_status: "disabled",
            accepted_output_sha256: accepted.map(generation_sha256_hex),
        };
        if let Some(store) = &self.store {
            let decision_id = format!("decision-{}", generation_record_id());
            let payload = serde_json::json!({
                "schema": PROVIDER_OBSERVATION_SCHEMA, "stage": "dialogue_decision",
                "generation_id": self.generation_id,
                "logical_attempt_index": self.logical_attempt_index,
                "attempts": result.attempts,
                "decision": if accepted.is_some() { "accepted" } else { "not_accepted" },
                "accepted_output_sha256": result.accepted_output_sha256,
                "created_at_unix_ms": generation_unix_ms(),
                "persistence_contract": "complete_immutable_file_is_write_receipt",
            });
            result.decision_recording_status = if store.write_event(&decision_id, &payload) {
                "recorded"
            } else {
                "recording_failed"
            };
        }
        result
    }
}

struct ProviderAttemptObserver {
    context: ProviderObservationContext,
    envelope: serde_json::Value,
    link: ProviderAttemptLink,
    started: std::time::Instant,
}

impl ProviderAttemptObserver {
    fn begin(
        label: &str,
        provider: &'static str,
        configured_model: &str,
        request_bytes: &[u8],
        context: Option<&ProviderObservationContext>,
    ) -> Option<Self> {
        let context = context
            .cloned()
            .unwrap_or_else(|| ProviderObservationContext::configured(None, None));
        let store = context.store.as_ref()?;
        let attempt_id = format!("provider-{}", generation_record_id());
        let release = provider_observation_release();
        let envelope = serde_json::json!({
            "schema": PROVIDER_OBSERVATION_SCHEMA,
            "stage": "dispatch_started",
            "dispatch_semantics": "client_send_entered_not_proof_of_server_acceptance",
            "attempt_id": attempt_id,
            "generation_id": context.generation_id,
            "logical_attempt_index": context.logical_attempt_index,
            "label": label, "provider": provider,
            "configured_model": configured_model.chars().take(256).collect::<String>(),
            "configured_model_sha256": generation_sha256_hex(configured_model),
            "configured_model_truncated": configured_model.chars().count() > 256,
            "reported_model": null,
            "request_sha256": hex::encode(Sha256::digest(request_bytes)),
            "generation_controls": provider_control_request(request_bytes),
            "raw_response_stage": "parsed_message_content_before_cleanup",
            "input_availability": "response_unavailable",
            "marker_observed_total": null,
            "outcome": "cancelled_or_abandoned",
            "release_before": release,
            "created_at_unix_ms": generation_unix_ms(),
            "pid": std::process::id(),
            "persistence_contract": "complete_immutable_file_is_write_receipt",
        });
        let recorded = store.write_event(&format!("{attempt_id}-dispatch"), &envelope);
        Some(Self {
            context,
            envelope,
            link: ProviderAttemptLink {
                attempt_id,
                outcome: "cancelled_or_abandoned",
                input_availability: "response_unavailable",
                recording_status: if recorded {
                    "recorded"
                } else {
                    "recording_failed"
                },
                reported_model: None,
            },
            started: std::time::Instant::now(),
        })
    }

    fn requested_controls(
        observer: &mut Option<Self>,
        temperature: f32,
        max_tokens: u32,
        timeout_secs: u64,
    ) {
        if let Some(observer) = observer {
            observer.envelope["generation_controls"]["requested"] = serde_json::json!({
                "temperature": temperature, "max_tokens": max_tokens, "timeout_secs": timeout_secs,
                "source": "provider_caller_before_adapter_policy",
            });
        }
    }

    fn outcome(observer: &mut Option<Self>, outcome: &'static str) {
        if let Some(observer) = observer {
            observer.link.outcome = outcome;
        }
    }

    fn body(observer: &mut Option<Self>, body: &str, model: Option<&str>) {
        if let Some(observer) = observer {
            observer.envelope["generation_controls"]["server_reported"] =
                provider_control_response(body);
            observer.envelope["http_body_sha256"] = generation_sha256_hex(body).into();
            observer.envelope["http_body_bytes"] = body.len().into();
            observer.link.reported_model = model
                .filter(|model| !model.trim().is_empty())
                .map(|model| model.chars().take(256).collect());
        }
    }

    fn model(observer: &mut Option<Self>, model: Option<&str>) {
        if let Some(observer) = observer {
            let model = model.filter(|model| !model.trim().is_empty());
            observer.envelope["reported_model_sha256"] =
                serde_json::json!(model.map(generation_sha256_hex));
            let bounded = model.filter(|model| model.chars().count() <= 256);
            observer.link.reported_model = bounded.map(str::to_string);
            observer.envelope["reported_model_status"] = if bounded.is_some() {
                "reported_exact"
            } else if model.is_some() {
                "too_long_hash_only"
            } else {
                "unreported"
            }
            .into();
        }
    }

    fn normalized(
        observer: &mut Option<Self>,
        raw: &str,
        normalized: &ProviderOutputNormalizationV1,
    ) {
        if let Some(observer) = observer {
            observer.envelope["raw_response_sha256"] = generation_sha256_hex(raw).into();
            observer.envelope["raw_response_bytes"] = raw.len().into();
            observer.envelope["normalized_output_sha256"] =
                generation_sha256_hex(&normalized.text).into();
            observer.envelope["normalized_output_stage"] = "marker_cleanup_then_trim".into();
            let count = normalized
                .cleanup_report
                .as_ref()
                .map_or(0, |report| report.observed_total);
            observer.envelope["marker_observed_total"] = count.into();
            observer.envelope["cleanup_report"] =
                serde_json::to_value(&normalized.cleanup_report).unwrap_or_default();
            observer.link.input_availability = if count == 0 {
                "observed_no_markers_raw_not_retained"
            } else if raw.len() > PROVIDER_RAW_MAX_BYTES {
                "input_not_retained_size_limit"
            } else {
                let store = observer
                    .context
                    .store
                    .as_ref()
                    .expect("enabled observer has store");
                match store.write_raw(raw) {
                    Ok(path) => {
                        observer.envelope["raw_artifact"] = path.into();
                        "retained_exact"
                    },
                    Err(ProviderObservationWriteError::Quota) => "input_not_retained_quota",
                    Err(ProviderObservationWriteError::Io) => {
                        observer.link.recording_status = "recording_failed";
                        "input_not_retained_recording_failed"
                    },
                }
            };
        }
    }

    fn returned(observer: &mut Option<Self>, text: &str) {
        if let Some(observer) = observer {
            observer.link.outcome = "provider_returned";
            observer.envelope["provider_output_sha256"] = generation_sha256_hex(text).into();
            observer.envelope["provider_output_stage"] =
                "after_provider_cleanup_and_quality_filters".into();
        }
    }
}

impl Drop for ProviderAttemptObserver {
    fn drop(&mut self) {
        self.envelope["stage"] = "provider_outcome".into();
        self.envelope["outcome"] = self.link.outcome.into();
        self.envelope["input_availability"] = self.link.input_availability.into();
        self.envelope["reported_model"] = serde_json::json!(self.link.reported_model);
        self.envelope["earlier_recording_status"] = self.link.recording_status.into();
        self.envelope["release_after"] = provider_observation_release();
        self.envelope["elapsed_ms"] = u64::try_from(self.started.elapsed().as_millis())
            .unwrap_or(u64::MAX)
            .into();
        let store = self
            .context
            .store
            .as_ref()
            .expect("enabled observer has store");
        if !store.write_event(&format!("{}-outcome", self.link.attempt_id), &self.envelope) {
            self.link.recording_status = "recording_failed";
        }
        self.context
            .attempts
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .push(self.link.clone());
    }
}

fn provider_observation_release() -> serde_json::Value {
    // Reuse the startup manifest-to-executable verification. A mutable build
    // file or git HEAD alone never earns an activation claim. Legacy launches
    // without a pinned manifest stay explicitly unknown. No per-request binary
    // hashing, live file reads, or deploy-authority inference.
    crate::deployment::verification_receipt().map_or_else(
        || serde_json::json!({"status": "activation_unknown"}),
        |receipt| {
            serde_json::json!({
                "status": "startup_verified_process_binding",
                "manifest_sha256": receipt["manifest_sha256"],
                "binary_sha256": receipt["binary_sha256"],
                "deployment_identity": receipt["deployment_identity"],
                "verification_scope": "immutable_startup_binding_not_current_service_health",
            })
        },
    )
}
