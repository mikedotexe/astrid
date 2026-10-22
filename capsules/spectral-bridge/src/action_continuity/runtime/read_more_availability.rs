/// Quota availability of bare READ_MORE, not proof of a saved source or delivery.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ResearchBudgetReadMoreAvailability {
    Allowed,
    Blocked { reason: String, suggested_next: String },
    Unknown,
}

impl ActionContinuityStore {
    /// Bare continuation has no explicit experiment binding. Source ownership,
    /// revision and pending-work checks remain with the reader at dispatch.
    /// Historical quota state is not an input and is never refreshed here.
    #[must_use]
    pub fn research_budget_read_more_availability(&self) -> ResearchBudgetReadMoreAvailability {
        ResearchBudgetReadMoreAvailability::Allowed
    }
}

#[cfg(test)]
mod read_more_availability_tests {
    use super::*;

    #[test]
    fn unbound_continuation_never_reads_or_changes_experiment_quota_state() {
        let directory = tempfile::tempdir().unwrap();
        let root = directory.path().join("absent");
        let store = ActionContinuityStore::new(&root);
        assert_eq!(store.research_budget_read_more_availability(), ResearchBudgetReadMoreAvailability::Allowed);
        assert!(!root.exists());
        let thread = store.create_thread(None, "Synthetic continuation", None).unwrap();
        let experiment = store.start_experiment(None, "Reading", "Can another page remain voluntary?").unwrap();
        let gate = store.authority_gate_path(&thread.thread_id);
        for state in ["active", "expired", "held", "closed", "exhausted", "duplicate_target", "other_owner", "corrupt"] {
            let bytes = if state == "corrupt" {
                b"{incomplete".to_vec()
            } else {
                format!("{}\n", json!({"record_schema":"research_budget_v1",
                    "record_type":"research_budget_approval", "status":state,
                    "experiment_id":experiment.experiment_id, "budget_id":"synthetic",
                    "max_actions":0, "expires_at_unix_s":1})).into_bytes()
            };
            fs::write(&gate, &bytes).unwrap();
            let before = fs::metadata(&gate).unwrap().modified().unwrap();
            assert_eq!(store.research_budget_read_more_availability(), ResearchBudgetReadMoreAvailability::Allowed, "{state}");
            assert_eq!(fs::read(&gate).unwrap(), bytes);
            assert_eq!(fs::metadata(&gate).unwrap().modified().unwrap(), before);
            let telemetry = serde_json::from_value(json!({"t_ms":1,"eigenvalues":[1.0,0.5],"fill_ratio":0.68})).unwrap();
            assert!(store.research_budget_guard_assessment("READ_MORE", 68.0, &telemetry).unwrap().is_none());
        }
    }
}
