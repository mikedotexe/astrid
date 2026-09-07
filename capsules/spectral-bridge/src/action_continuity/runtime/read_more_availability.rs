/// Advisory availability of bare `READ_MORE` under the research-budget policy.
/// This does not reserve a budget action or establish that source content exists;
/// dispatch must still assess the actual selected command and current state.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ResearchBudgetReadMoreAvailability {
    Allowed,
    Blocked {
        reason: String,
        suggested_next: String,
    },
    Unknown,
}

impl ActionContinuityStore {
    /// Read the current policy without creating, refreshing, or appending state.
    /// Missing index means no active thread; corrupt or unreadable required state
    /// is unknown. Unlike dispatch's assessment, this never records a denial.
    #[must_use]
    pub fn research_budget_read_more_availability(&self) -> ResearchBudgetReadMoreAvailability {
        self.read_more_availability_read_only()
            .unwrap_or(ResearchBudgetReadMoreAvailability::Unknown)
    }

    fn read_more_availability_read_only(&self) -> Result<ResearchBudgetReadMoreAvailability> {
        // current_thread/read_thread can create directories and refresh persisted
        // projections. Prompt preparation must not call either reader.
        let index_raw = match fs::read_to_string(self.index_path()) {
            Ok(raw) => raw,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                return Ok(ResearchBudgetReadMoreAvailability::Allowed);
            },
            Err(error) => return Err(error.into()),
        };
        let index: ContinuityIndex = serde_json::from_str(&index_raw)?;
        if index.schema_version != SCHEMA_VERSION {
            return Err(anyhow!("Unsupported continuity index schema"));
        }
        let Some(thread_id) = index.active_thread_id else {
            return Ok(ResearchBudgetReadMoreAvailability::Allowed);
        };
        let mut components = Path::new(&thread_id).components();
        if !matches!(components.next(), Some(std::path::Component::Normal(_)))
            || components.next().is_some()
        {
            return Err(anyhow!("Invalid current thread identity"));
        }
        let thread: ResearchThread = serde_json::from_str(&fs::read_to_string(
            self.thread_dir(&thread_id).join("thread.json"),
        )?)?;
        if thread.thread_id != thread_id {
            return Err(anyhow!("Current thread identity does not match its record"));
        }
        let Some(experiment_id) = thread.active_experiment_id.as_deref().or_else(|| {
            thread
                .experiment_summary
                .as_ref()
                .and_then(|summary| summary.get("experiment_id"))
                .and_then(Value::as_str)
        }) else {
            return Ok(ResearchBudgetReadMoreAvailability::Allowed);
        };
        // Resolve the explicit experiment without the existing readers' silent
        // read/parse fallbacks. An incomplete append cannot become "allowed".
        let experiments_raw = fs::read_to_string(self.experiments_path(&thread_id))?;
        let experiments = experiments_raw
            .lines()
            .filter(|line| !line.trim().is_empty())
            .map(serde_json::from_str::<ExperimentRecord>)
            .collect::<std::result::Result<Vec<_>, _>>()?;
        if !experiments
            .iter()
            .any(|experiment| experiment.experiment_id == experiment_id)
        {
            return Err(anyhow!("Current experiment record is unavailable"));
        }
        // Dispatch's selector also accepts a title substring, even when the
        // thread carries an explicit ID. A later title can shadow that ID.
        // Check its latest-record ordering without adopting an ambiguous target
        // or changing the dispatcher's existing selection behavior.
        let selector = normalize_experiment_selector(experiment_id);
        if selector != experiment_id || selector.is_empty() || selector == "current" {
            return Err(anyhow!("Current experiment selector is ambiguous"));
        }
        let lower = selector.to_ascii_lowercase();
        let mut seen = HashSet::new();
        let resolved = experiments
            .iter()
            .rev()
            .filter(|record| seen.insert(record.experiment_id.as_str()))
            .find(|record| {
                record.experiment_id == selector
                    || record.title.to_ascii_lowercase().contains(&lower)
            });
        if resolved.map(|record| record.experiment_id.as_str()) != Some(experiment_id) {
            return Err(anyhow!("Dispatch resolves a different experiment"));
        }
        let authority_raw = fs::read_to_string(self.authority_gate_path(&thread_id))?;
        let rows = authority_raw
            .lines()
            .filter(|line| !line.trim().is_empty())
            .map(serde_json::from_str::<Value>)
            .collect::<std::result::Result<Vec<_>, _>>()?;
        if rows.iter().any(|row| !row.is_object()) {
            return Err(anyhow!("Invalid authority record"));
        }
        // These are dispatch's existing expiry, closure, scope and debit-count
        // predicates, plus its shared missing-budget and duplicate guidance.
        let Some(budget) = active_research_budget_from_rows(&rows, experiment_id) else {
            return Ok(ResearchBudgetReadMoreAvailability::Blocked {
                reason: guards::BudgetReason::NoActiveReadOnlyBudget
                    .as_str()
                    .to_string(),
                suggested_next: research_budget_missing_guidance(&rows).0,
            });
        };
        let budget_id = budget
            .get("budget_id")
            .and_then(Value::as_str)
            .unwrap_or_default();
        if let Some(suggested_next) = research_budget_duplicate_review_next(
            &rows,
            budget_id,
            &normalized_research_budget_target("READ_MORE"),
        ) {
            return Ok(ResearchBudgetReadMoreAvailability::Blocked {
                reason: guards::BudgetReason::DuplicateReviewRequired
                    .as_str()
                    .to_string(),
                suggested_next,
            });
        }
        Ok(ResearchBudgetReadMoreAvailability::Allowed)
    }
}

#[cfg(test)]
mod read_more_availability_tests {
    use super::*;
    use std::collections::BTreeMap;

    fn fixture() -> (
        tempfile::TempDir,
        ActionContinuityStore,
        ResearchThread,
        ExperimentRecord,
    ) {
        let directory = tempfile::tempdir().unwrap();
        let store = ActionContinuityStore::new(directory.path());
        let thread = store
            .create_thread(None, "Read-only budget fixture", None)
            .unwrap();
        let experiment = store
            .start_experiment(None, "Reading", "Can the next page be read?")
            .unwrap();
        // Starting an experiment does not create the authority ledger. This
        // fixture models a readable ledger containing no approvals; separate
        // cases remove it to verify missing state remains unknown.
        fs::write(store.authority_gate_path(&thread.thread_id), b"").unwrap();
        (directory, store, thread, experiment)
    }

    fn snapshot(root: &Path) -> BTreeMap<PathBuf, (bool, Vec<u8>, SystemTime)> {
        fn walk(
            root: &Path,
            path: &Path,
            result: &mut BTreeMap<PathBuf, (bool, Vec<u8>, SystemTime)>,
        ) {
            let metadata = fs::metadata(path).unwrap();
            result.insert(
                path.strip_prefix(root).unwrap().to_path_buf(),
                (
                    metadata.is_dir(),
                    if metadata.is_file() {
                        fs::read(path).unwrap()
                    } else {
                        Vec::new()
                    },
                    metadata.modified().unwrap(),
                ),
            );
            if metadata.is_dir() {
                for entry in fs::read_dir(path).unwrap() {
                    walk(root, &entry.unwrap().path(), result);
                }
            }
        }
        let mut result = BTreeMap::new();
        walk(root, root, &mut result);
        result
    }

    fn unchanged_preview(store: &ActionContinuityStore) -> ResearchBudgetReadMoreAvailability {
        let before = snapshot(store.root());
        let result = store.research_budget_read_more_availability();
        assert_eq!(
            snapshot(store.root()),
            before,
            "preview changed persisted state"
        );
        result
    }

    fn approval(experiment: &ExperimentRecord) -> Value {
        json!({"record_schema":"research_budget_v1", "record_type":"research_budget_approval",
            "budget_id":"resbud_preview", "experiment_id":experiment.experiment_id,
            "scope":"read_only_research", "status":"active", "max_actions":5,
            "expires_at_unix_s":u64::MAX})
    }

    fn set_rows(store: &ActionContinuityStore, thread: &ResearchThread, rows: &[Value]) {
        let raw = rows
            .iter()
            .map(|row| format!("{row}\n"))
            .collect::<String>();
        fs::write(store.authority_gate_path(&thread.thread_id), raw).unwrap();
    }

    fn matches_dispatch(
        store: &ActionContinuityStore,
        preview: &ResearchBudgetReadMoreAvailability,
    ) {
        let telemetry =
            serde_json::from_value(json!({"t_ms":1,"eigenvalues":[1.0,0.5],"fill_ratio":0.68}))
                .unwrap();
        let dispatched = store
            .research_budget_guard_assessment("READ_MORE", 68.0, &telemetry)
            .unwrap();
        match (preview, dispatched) {
            (ResearchBudgetReadMoreAvailability::Allowed, None) => {},
            (
                ResearchBudgetReadMoreAvailability::Blocked {
                    reason,
                    suggested_next,
                },
                Some(assessment),
            ) => {
                assert_eq!(reason.as_str(), assessment.reason.as_str());
                assert_eq!(suggested_next, &assessment.suggested_next);
            },
            pair => panic!("preview/dispatch disagreement: {pair:?}"),
        }
    }

    #[test]
    fn read_more_availability_does_not_create_missing_store_or_active_thread() {
        let directory = tempfile::tempdir().unwrap();
        let missing = directory.path().join("never-created");
        let store = ActionContinuityStore::new(&missing);
        assert_eq!(
            store.research_budget_read_more_availability(),
            ResearchBudgetReadMoreAvailability::Allowed
        );
        assert!(!missing.exists());
        fs::create_dir(&missing).unwrap();
        fs::write(
            store.index_path(),
            serde_json::to_vec(&ContinuityIndex::default()).unwrap(),
        )
        .unwrap();
        assert_eq!(
            unchanged_preview(&store),
            ResearchBudgetReadMoreAvailability::Allowed
        );
        assert!(!missing.join("threads").exists());
    }

    #[test]
    fn read_more_availability_no_active_experiment_does_not_refresh_thread() {
        let directory = tempfile::tempdir().unwrap();
        let store = ActionContinuityStore::new(directory.path());
        store.create_thread(None, "No experiment", None).unwrap();
        assert_eq!(
            unchanged_preview(&store),
            ResearchBudgetReadMoreAvailability::Allowed
        );
    }

    #[test]
    fn read_more_availability_missing_budget_matches_dispatch_without_recording_denial() {
        let (_directory, store, thread, _) = fixture();
        let preview = unchanged_preview(&store);
        assert_eq!(
            preview,
            ResearchBudgetReadMoreAvailability::Blocked {
                reason: "no_active_read_only_research_budget".into(),
                suggested_next: "EXPERIMENT_RESEARCH_BUDGET_ACCEPT latest".into(),
            }
        );
        assert!(
            !fs::read_to_string(store.authority_gate_path(&thread.thread_id))
                .unwrap()
                .contains("research_budget_blocked")
        );
        matches_dispatch(&store, &preview);
        assert!(
            fs::read_to_string(store.authority_gate_path(&thread.thread_id))
                .unwrap()
                .contains("research_budget_blocked")
        );
    }

    #[test]
    fn read_more_availability_existing_request_uses_shared_status_guidance() {
        let (_directory, store, thread, experiment) = fixture();
        set_rows(
            &store,
            &thread,
            &[
                json!({"record_schema":"research_budget_v1", "record_type":"research_budget_request",
            "budget_id":"resbud_pending", "experiment_id":experiment.experiment_id, "status":"pending_steward_approval"}),
            ],
        );
        let preview = unchanged_preview(&store);
        assert_eq!(
            preview,
            ResearchBudgetReadMoreAvailability::Blocked {
                reason: "no_active_read_only_research_budget".into(),
                suggested_next: "EXPERIMENT_RESEARCH_BUDGET_STATUS resbud_pending".into(),
            }
        );
        matches_dispatch(&store, &preview);
    }

    #[test]
    fn read_more_availability_active_expired_closed_exhausted_and_other_budget_match_dispatch() {
        for state in [
            "active",
            "expired",
            "closed",
            "exhausted",
            "other_experiment",
            "other_scope",
        ] {
            let (_directory, store, thread, experiment) = fixture();
            let mut budget = approval(&experiment);
            match state {
                "expired" => budget["expires_at_unix_s"] = json!(1),
                "exhausted" => budget["max_actions"] = json!(2),
                "other_experiment" => budget["experiment_id"] = json!("exp_other"),
                "other_scope" => budget["scope"] = json!("mutating"),
                _ => {},
            }
            let mut rows = vec![budget];
            if state == "exhausted" {
                for _ in 0..2 {
                    rows.push(json!({"record_schema":"research_budget_v1", "record_type":"research_budget_debit",
                        "budget_id":"resbud_preview", "normalized_target":"a different page"}));
                }
            }
            if state == "closed" {
                rows.push(json!({"record_schema":"research_budget_v1", "record_type":"research_budget_closed", "budget_id":"resbud_preview"}));
            }
            set_rows(&store, &thread, &rows);
            let preview = unchanged_preview(&store);
            assert_eq!(
                matches!(preview, ResearchBudgetReadMoreAvailability::Allowed),
                state == "active",
                "{state}"
            );
            matches_dispatch(&store, &preview);
        }
    }

    #[test]
    fn read_more_availability_duplicate_review_counts_bare_read_more_only() {
        let (_directory, store, thread, experiment) = fixture();
        let mut rows = vec![approval(&experiment)];
        for _ in 0..2 {
            rows.push(
                json!({"record_schema":"research_budget_v1", "record_type":"research_budget_debit",
                "budget_id":"resbud_preview", "normalized_target":"some other page"}),
            );
        }
        set_rows(&store, &thread, &rows);
        assert_eq!(
            unchanged_preview(&store),
            ResearchBudgetReadMoreAvailability::Allowed
        );
        rows[1]["normalized_target"] = json!("read_more");
        rows[2]["normalized_target"] = json!("read_more");
        set_rows(&store, &thread, &rows);
        let preview = unchanged_preview(&store);
        assert!(
            matches!(&preview, ResearchBudgetReadMoreAvailability::Blocked { reason, .. } if reason == "duplicate_query_or_url_review_required")
        );
        matches_dispatch(&store, &preview);
    }

    #[test]
    fn read_more_availability_shadowed_experiment_selector_is_unknown() {
        let (_directory, store, thread, experiment) = fixture();
        set_rows(&store, &thread, &[approval(&experiment)]);
        assert_eq!(
            unchanged_preview(&store),
            ResearchBudgetReadMoreAvailability::Allowed
        );
        let mut shadow = experiment.clone();
        shadow.experiment_id = "exp_title_shadow".into();
        shadow.title = format!("Follow-up to {}", experiment.experiment_id);
        store
            .append_jsonl(&store.experiments_path(&thread.thread_id), &shadow)
            .unwrap();
        assert_eq!(
            unchanged_preview(&store),
            ResearchBudgetReadMoreAvailability::Unknown
        );

        let telemetry =
            serde_json::from_value(json!({"t_ms":1,"eigenvalues":[1.0,0.5],"fill_ratio":0.68}))
                .unwrap();
        let assessment = store
            .research_budget_guard_assessment("READ_MORE", 68.0, &telemetry)
            .unwrap()
            .unwrap();
        assert_eq!(assessment.experiment_id, shadow.experiment_id);
        assert_eq!(
            assessment.reason.as_str(),
            "no_active_read_only_research_budget"
        );

        // Historical matching titles do not shadow once their latest record
        // changes: preview follows dispatch's latest-per-ID ordering.
        shadow.title = "An unrelated later question".into();
        store
            .append_jsonl(&store.experiments_path(&thread.thread_id), &shadow)
            .unwrap();
        let preview = unchanged_preview(&store);
        assert_eq!(preview, ResearchBudgetReadMoreAvailability::Allowed);
        matches_dispatch(&store, &preview);
    }

    #[test]
    fn read_more_availability_unreadable_or_corrupt_required_state_is_unknown_without_writes() {
        for target in [
            "index",
            "thread",
            "experiments",
            "authority",
            "missing_authority",
            "unreadable_authority",
        ] {
            let (_directory, store, thread, _) = fixture();
            let path = match target {
                "index" => store.index_path(),
                "thread" => store.thread_dir(&thread.thread_id).join("thread.json"),
                "experiments" => store.experiments_path(&thread.thread_id),
                _ => store.authority_gate_path(&thread.thread_id),
            };
            if target.starts_with("missing") || target.starts_with("unreadable") {
                fs::remove_file(&path).unwrap();
                if target.starts_with("unreadable") {
                    fs::create_dir(path).unwrap();
                }
            } else {
                fs::write(path, "{incomplete").unwrap();
            }
            assert_eq!(
                unchanged_preview(&store),
                ResearchBudgetReadMoreAvailability::Unknown,
                "{target}"
            );
        }
    }
}
