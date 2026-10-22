//! Explicit owner-scoped research admission in the native authority log.
//!
//! The lock covers validation and durable append, never a host action or inference.
use super::*;
use std::cell::RefCell;
use std::marker::PhantomData;
use std::rc::Rc;

pub const RESEARCH_TRANSACTION_VERSION: u32 = 1;

thread_local! {
    static LOCKS: RefCell<HashMap<PathBuf, (fs::File, usize)>> = RefCell::new(HashMap::new());
}

pub(crate) struct OwnerTransaction {
    path: PathBuf,
    // A process lock must be released on the thread that entered it.
    _local: PhantomData<Rc<()>>,
}

pub(crate) fn research_owner_transaction(root: &Path) -> Result<OwnerTransaction> {
    OwnerTransaction::enter(root)
}

impl OwnerTransaction {
    pub(super) fn enter(root: &Path) -> Result<Self> {
        fs::create_dir_all(root)?;
        let path = fs::canonicalize(root)?.join(".research-transaction.lock");
        LOCKS.with(|locks| -> Result<()> {
            let mut locks = locks.borrow_mut();
            if let Some((_, count)) = locks.get_mut(&path) {
                *count = count.checked_add(1).context("transaction depth exceeded")?;
                return Ok(());
            }
            anyhow::ensure!(!path.is_symlink(), "research lock is a symlink");
            let file = OpenOptions::new()
                .create(true)
                .append(true)
                .read(true)
                .open(&path)?;
            file.lock()?;
            locks.insert(path.clone(), (file, 1));
            Ok(())
        })?;
        Ok(Self {
            path,
            _local: PhantomData,
        })
    }
}

impl Drop for OwnerTransaction {
    fn drop(&mut self) {
        LOCKS.with(|locks| {
            let mut locks = locks.borrow_mut();
            if let Some((_, count)) = locks.get_mut(&self.path) {
                *count = count.saturating_sub(1);
                if *count == 0 {
                    locks.remove(&self.path);
                }
            }
        });
    }
}

fn strict_history_text(path: &Path) -> Result<Option<String>> {
    let metadata = match fs::symlink_metadata(path) {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(error) => return Err(error.into()),
    };
    anyhow::ensure!(
        metadata.is_file(),
        "history is not a regular file; preserved"
    );
    let raw = fs::read_to_string(path)?;
    anyhow::ensure!(
        raw.is_empty() || raw.ends_with('\n'),
        "incomplete authority history preserved"
    );
    Ok(Some(raw))
}

pub(crate) fn strict_rows(path: &Path) -> Result<Vec<Value>> {
    let Some(raw) = strict_history_text(path)? else {
        return Ok(Vec::new());
    };
    raw.lines()
        .filter(|line| !line.trim().is_empty())
        .map(|line| {
            let row = research_json::parse(line)?;
            anyhow::ensure!(row.is_object(), "invalid authority history preserved");
            anyhow::ensure!(
                row.get("research_transaction_version")
                    .is_none_or(|version| version == 1),
                "unsupported research transaction history preserved"
            );
            Ok(row)
        })
        .collect()
}

// This is a launch/rollback guard input, not a claim that older adapters know
// how to refuse it. Release tooling must enforce it before activation.
pub(super) fn require_capability(root: &Path) -> Result<()> {
    require_capability_with_sync(root, |_, file| Ok(file.sync_all()?))
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ResearchRequirement {
    schema_version: u32,
    owner: String,
    required_research_transaction_version: u32,
}

/// Inspect the compatibility floor without creating, migrating or repairing state.
pub fn research_transaction_compatibility(root: &Path) -> Result<Value> {
    use std::io::Read as _;
    anyhow::ensure!(!root.is_symlink(), "research directory is a symlink");
    let path = root.join("research-transactions-required.json");
    let required = match fs::symlink_metadata(&path) {
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => 0,
        Err(error) => return Err(error.into()),
        Ok(metadata) => {
            anyhow::ensure!(
                metadata.is_file() && metadata.len() <= 4096,
                "invalid research capability requirement preserved"
            );
            let mut raw = String::new();
            fs::File::open(&path)?.take(4097).read_to_string(&mut raw)?;
            anyhow::ensure!(raw.len() <= 4096, "research requirement too large");
            let value = research_json::parse(&raw)?;
            let existing: ResearchRequirement = serde_json::from_value(value)?;
            anyhow::ensure!(
                existing.schema_version == 1
                    && existing.owner == SYSTEM
                    && existing.required_research_transaction_version >= 1
                    && existing.required_research_transaction_version
                        <= RESEARCH_TRANSACTION_VERSION,
                "research transaction downgrade refused; state preserved"
            );
            existing.required_research_transaction_version
        },
    };
    Ok(
        json!({"owner":SYSTEM,"supported_version":RESEARCH_TRANSACTION_VERSION,
        "required_version":required}),
    )
}

pub(super) fn require_capability_with_sync(
    root: &Path,
    mut sync: impl FnMut(&Path, &fs::File) -> Result<()>,
) -> Result<()> {
    use std::io::Read as _;
    anyhow::ensure!(
        !root.is_symlink(),
        "research requirement directory is a symlink"
    );
    let _lock = OwnerTransaction::enter(root)?;
    let path = root.join("research-transactions-required.json");
    let payload =
        json!({"schema_version": 1, "owner": SYSTEM, "required_research_transaction_version": 1});
    let metadata = match fs::symlink_metadata(&path) {
        Ok(metadata) => Some(metadata),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => None,
        Err(error) => return Err(error.into()),
    };
    let file = if let Some(metadata) = metadata {
        anyhow::ensure!(
            metadata.is_file() && metadata.len() <= 4096,
            "invalid research capability requirement preserved"
        );
        let file = fs::File::open(&path)?;
        let mut raw = Vec::new();
        (&file).take(4097).read_to_end(&mut raw)?;
        anyhow::ensure!(raw.len() <= 4096, "research requirement too large");
        let existing: ResearchRequirement = serde_json::from_slice(&raw)?;
        anyhow::ensure!(
            existing.schema_version == 1
                && existing.owner == SYSTEM
                && existing.required_research_transaction_version == RESEARCH_TRANSACTION_VERSION,
            "unsupported research capability requirement preserved"
        );
        file
    } else {
        let mut file = OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&path)?;
        writeln!(file, "{payload}")?;
        file
    };
    // An existing valid marker may be left by an uncertain earlier fsync.
    sync(&path, &file)?;
    sync(root, &fs::File::open(root)?)?;
    if let Some(parent) = root.parent() {
        let parent = if parent.as_os_str().is_empty() {
            Path::new(".")
        } else {
            parent
        };
        sync(parent, &fs::File::open(parent)?)?;
    }
    Ok(())
}

pub(super) fn reservations(rows: &[Value], budget_id: &str) -> usize {
    let mut latest = HashMap::new();
    for row in rows
        .iter()
        .filter(|row| row["budget_id"] == budget_id && row["research_transaction_version"] == 1)
    {
        if let Some(id) = row["operation_id"].as_str() {
            latest.insert(id, row);
        }
    }
    latest
        .values()
        .filter(|row| matches!(row["lifecycle"].as_str(), Some("admitted" | "queued")))
        .count()
}

pub(super) fn held(rows: &[Value], budget_id: &str) -> bool {
    rows.iter()
        .rev()
        .find(|row| row["budget_id"] == budget_id && row["record_type"] == "research_budget_review")
        .is_some_and(|row| matches!(row["outcome"].as_str(), Some("hold" | "close")))
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResearchReservation {
    pub(super) record: Value,
}

pub fn research_binding_requires_budget(action: &str) -> Result<bool> {
    let canonical = crate::autonomous::canonicalize_next_action_text(action);
    let base = base_action(&canonical);
    anyhow::ensure!(
        !action.contains(['\r', '\n', '\0']),
        "research requires one bounded command"
    );
    if matches!(
        base.as_str(),
        "THREADS"
            | "THREAD_STATUS"
            | "STATE"
            | "FACULTIES"
            | "ACTION_PREFLIGHT"
            | "PREFLIGHT"
            | "ACTION_CHECK"
    ) {
        return Ok(false);
    }
    validate_research_shape(action)?;
    if charter_guard_live_base(&base) || mutating_research_budget_base(&base) {
        // Binding records provenance; the existing consequential-action gate
        // still decides authority. A research allowance cannot grant it.
        return Ok(false);
    }
    anyhow::ensure!(
        read_only_research_budget_base(&base)
            || research_budget_projection_only_base(&base)
            || astrid_source_study::research_source_route(&canonical).is_some(),
        "bound action is not a verified read-only route"
    );
    Ok(true)
}

pub(super) fn validate_research_shape(action: &str) -> Result<()> {
    anyhow::ensure!(
        !action.contains(['\r', '\n', '\0']),
        "research requires one bounded command"
    );
    let words: Vec<_> = action.split_whitespace().collect();
    anyhow::ensure!(!words.is_empty(), "empty research action");
    if astrid_source_study::command_boundary::literal_arguments(action) {
        return Ok(());
    }
    for pair in words.windows(2) {
        let next = pair[1].to_ascii_uppercase();
        anyhow::ensure!(
            !(pair[0].eq_ignore_ascii_case("AND") && next.len() >= 5 && next.contains('_')),
            "compound research requires separately bound actions"
        );
    }
    Ok(())
}

/// Append-only entry point for other native authority-stream writers. It joins
/// the same owner transaction and rejects incomplete or unknown-version history.
pub fn append_native_authority_record(path: &Path, row: &Value) -> Result<()> {
    anyhow::ensure!(
        path.file_name() == Some(OsStr::new("authority_gate.jsonl")),
        "not an authority stream"
    );
    let thread = path.parent().context("missing thread directory")?;
    let threads = thread.parent().context("missing threads directory")?;
    anyhow::ensure!(
        threads.file_name() == Some(OsStr::new("threads")),
        "invalid authority path"
    );
    let root = threads.parent().context("missing owner directory")?;
    ActionContinuityStore::new(root).append_jsonl(path, row)
}

impl ResearchReservation {
    #[must_use]
    pub fn metadata(&self) -> Value {
        json!({"policy": "explicit_research_transaction_v1", "record_id": self.record["record_id"],
            "operation_id": self.record["operation_id"], "experiment_id": self.record["experiment_id"],
            "budget_id": self.record["budget_id"], "execution_scope": "host_action_dispatch",
            "input_delivered": "unknown"})
    }

    pub(crate) fn action(&self) -> Result<&str> {
        self.record["raw_action"]
            .as_str()
            .context("missing reserved action")
    }
}

fn validate_source_scope(budget: &Value, action: &str) -> Result<()> {
    let sources = budget["allowed_sources"]
        .as_array()
        .context("allowance source scope missing")?;
    anyhow::ensure!(
        !sources.is_empty()
            && sources
                .iter()
                .all(|source| matches!(source.as_str(), Some("web" | "local"))),
        "unsupported allowance source scope"
    );
    let base = base_action(&crate::autonomous::canonicalize_next_action_text(action));
    let required: &[&str] = if research_web_request(action)?.is_some() {
        &["web"]
    } else if base == "READ_MORE" {
        &["web", "local"]
    } else {
        &["local"]
    };
    anyhow::ensure!(
        required
            .iter()
            .all(|required| sources.iter().any(|source| source == required)),
        "action source is outside the exact allowance"
    );
    Ok(())
}

impl ActionContinuityStore {
    pub(super) fn strict_research_experiment(
        &self,
        thread_id: &str,
        experiment_id: &str,
    ) -> Result<ExperimentRecord> {
        anyhow::ensure!(
            !thread_id.is_empty()
                && thread_id.len() <= 512
                && thread_id
                    .bytes()
                    .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b'-'))
                && experiment_id.starts_with(LOCAL_EXPERIMENT_PREFIX),
            "invalid research owner identity"
        );
        let threads = self.root.join("threads");
        let directory = self.thread_dir(thread_id);
        for path in [&self.root, &threads, &directory] {
            anyhow::ensure!(
                fs::symlink_metadata(path)?.is_dir(),
                "linked or invalid research owner directory"
            );
        }
        let thread_path = directory.join("thread.json");
        anyhow::ensure!(
            fs::symlink_metadata(&thread_path)?.is_file(),
            "invalid research thread file"
        );
        let thread: ResearchThread = serde_json::from_str(&fs::read_to_string(thread_path)?)?;
        anyhow::ensure!(
            thread.schema_version == SCHEMA_VERSION
                && thread.thread_id == thread_id
                && thread.system_origin == SYSTEM,
            "research thread owner, identity or schema mismatch"
        );
        let raw = strict_history_text(&self.experiments_path(thread_id))?
            .context("missing experiment history")?;
        let mut latest = None;
        for line in raw.lines() {
            let record: ExperimentRecord = serde_json::from_value(research_json::parse(line)?)?;
            anyhow::ensure!(
                record.schema_version == SCHEMA_VERSION
                    && record.thread_id == thread_id
                    && record.experiment_id.starts_with(LOCAL_EXPERIMENT_PREFIX),
                "experiment history identity or schema mismatch"
            );
            if record.experiment_id == experiment_id {
                latest = Some(record);
            }
        }
        latest.context("missing experiment")
    }

    pub(crate) fn validate_research_reservation(
        &self,
        reservation: &ResearchReservation,
    ) -> Result<()> {
        let _lock = OwnerTransaction::enter(&self.root)?;
        let ticket = &reservation.record;
        let thread = ticket["thread_id"]
            .as_str()
            .context("missing research thread")?;
        anyhow::ensure!(
            !thread.is_empty()
                && thread.len() <= 512
                && thread
                    .bytes()
                    .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b'-')),
            "invalid research thread identity"
        );
        anyhow::ensure!(ticket["being"] == SYSTEM, "foreign research owner");
        let path = self.authority_gate_path(thread);
        anyhow::ensure!(
            !path
                .parent()
                .context("missing authority parent")?
                .is_symlink(),
            "linked research thread"
        );
        let rows = strict_rows(&path)?;
        anyhow::ensure!(
            rows.iter()
                .find(|row| row["operation_id"] == ticket["operation_id"])
                == Some(ticket),
            "reservation does not match native owner authority"
        );
        Ok(())
    }
    fn bound_research_context(
        &self,
        experiment_id: &str,
        action: &str,
    ) -> Result<(ResearchThread, ExperimentRecord, Vec<Value>, Value)> {
        anyhow::ensure!(
            research_binding_requires_budget(action)?,
            "metadata does not consume research allowance"
        );
        self.resolve_experiment_bind_target(Some(experiment_id))?;
        let index: ContinuityIndex = serde_json::from_str(&fs::read_to_string(self.index_path())?)?;
        let thread_id = index.active_thread_id.context("no selected thread")?;
        let thread: ResearchThread = serde_json::from_str(&fs::read_to_string(
            self.thread_dir(&thread_id).join("thread.json"),
        )?)?;
        let experiment = self.strict_research_experiment(&thread_id, experiment_id)?;
        anyhow::ensure!(
            !matches!(
                experiment.status.as_str(),
                "refused"
                    | "paused"
                    | "held"
                    | "hold"
                    | "closed"
                    | "complete"
                    | "completed"
                    | "abandoned"
            ),
            "experiment is held or complete"
        );
        // The allowlisted read-only route consumes its operands as data.
        // Executable compound actions were rejected by validate_research_shape.
        let rows = strict_rows(&self.authority_gate_path(&thread_id))?;
        let budget = active_research_budget_from_rows(&rows, experiment_id)
            .context("no active allowance for this exact experiment")?;
        anyhow::ensure!(
            budget["being"] == SYSTEM && budget["thread_id"] == thread_id,
            "allowance owner or thread mismatch"
        );
        validate_source_scope(&budget, action)?;
        Ok((thread, experiment, rows, budget))
    }

    pub fn research_preflight(&self, experiment_id: &str, action: &str) -> Result<Value> {
        let _lock = OwnerTransaction::enter(&self.root)?;
        let (_, _, _, budget) = self.bound_research_context(experiment_id, action)?;
        Ok(json!({"would_admit": true, "experiment_id": experiment_id,
            "budget_id": budget["budget_id"], "remaining_actions": budget["remaining_actions"],
            "repeat_policy": "counts_normally", "execution_started": false}))
    }

    pub fn reserve_research(
        &self,
        experiment_id: &str,
        action: &str,
        operation_id: &str,
        state: &Value,
    ) -> Result<ResearchReservation> {
        anyhow::ensure!(
            !operation_id.is_empty() && operation_id.len() <= 256,
            "bounded durable operation id required"
        );
        let _lock = OwnerTransaction::enter(&self.root)?;
        let (thread, experiment, rows, budget) =
            self.bound_research_context(experiment_id, action)?;
        // An identical retry is inspectable but never permission to dispatch twice.
        anyhow::ensure!(
            !rows.iter().any(|row| row["operation_id"] == operation_id),
            "operation already admitted; inspect receipt instead of replaying"
        );
        let budget_id = budget["budget_id"].as_str().context("missing budget id")?;
        let target = normalized_research_budget_target(action);
        let record = self.research_budget_record("research_budget_reservation", budget_id, &thread, &experiment, state,
            json!({"research_transaction_version": 1, "operation_id": operation_id, "lifecycle": "admitted",
                "raw_action": action, "normalized_target": target,
                "repeat_policy": "counts_normally"}));
        require_capability(&self.root)?;
        self.append_jsonl(&self.authority_gate_path(&thread.thread_id), &record)?;
        Ok(ResearchReservation { record })
    }

    pub fn transition_research(
        &self,
        reservation: &ResearchReservation,
        lifecycle: &str,
    ) -> Result<Value> {
        self.transition_research_scoped(reservation, lifecycle, None)
    }

    pub(crate) fn queue_saved_reading_research(
        &self,
        reservation: &ResearchReservation,
    ) -> Result<Value> {
        self.transition_research_scoped(reservation, "queued", Some("saved_text_generation_worker"))
    }

    pub(crate) fn queue_source_study_research(
        &self,
        reservation: &ResearchReservation,
    ) -> Result<Value> {
        self.transition_research_scoped(
            reservation,
            "queued",
            Some("source_study_generation_worker"),
        )
    }

    /// Recheck before reader preparation without spending the worker reservation.
    /// Callers hold the owner transaction across preparation; claim rechecks later.
    pub(crate) fn validate_research_start(&self, reservation: &ResearchReservation) -> Result<()> {
        let _lock = OwnerTransaction::enter(&self.root)?;
        self.validate_research_reservation(reservation)?;
        let ticket = &reservation.record;
        let rows = strict_rows(
            &self.authority_gate_path(ticket["thread_id"].as_str().context("missing thread")?),
        )?;
        let previous = rows
            .iter()
            .rev()
            .find(|row| row["operation_id"] == ticket["operation_id"])
            .context("missing reservation")?;
        anyhow::ensure!(
            matches!(previous["lifecycle"].as_str(), Some("admitted" | "queued")),
            "execution already started or uncertain; no automatic replay"
        );
        self.validate_research_start_context(reservation, &rows)
    }

    fn validate_research_start_context(
        &self,
        reservation: &ResearchReservation,
        rows: &[Value],
    ) -> Result<()> {
        let ticket = &reservation.record;
        let eligible: Vec<_> = rows
            .iter()
            .filter(|row| row["operation_id"] != ticket["operation_id"])
            .cloned()
            .collect();
        let experiment = ticket["experiment_id"]
            .as_str()
            .context("missing experiment")?;
        let active = active_research_budget_from_rows(&eligible, experiment)
            .filter(|active| active["budget_id"] == ticket["budget_id"])
            .context("allowance changed before dispatch")?;
        anyhow::ensure!(
            ["being", "thread_id", "experiment_id"]
                .iter()
                .all(|key| active[key] == ticket[key]),
            "allowance identity changed before dispatch"
        );
        validate_source_scope(
            &active,
            ticket["raw_action"].as_str().context("missing action")?,
        )?;
        // Worker authority remains the original ticket, not the current UI thread.
        anyhow::ensure!(ticket["being"] == SYSTEM, "foreign research owner");
        let current = self.strict_research_experiment(
            ticket["thread_id"].as_str().context("missing thread")?,
            experiment,
        )?;
        anyhow::ensure!(
            !matches!(
                current.status.as_str(),
                "refused"
                    | "paused"
                    | "held"
                    | "hold"
                    | "closed"
                    | "complete"
                    | "completed"
                    | "abandoned"
            ),
            "experiment held before execution"
        );
        Ok(())
    }

    fn transition_research_scoped(
        &self,
        reservation: &ResearchReservation,
        lifecycle: &str,
        scope: Option<&str>,
    ) -> Result<Value> {
        anyhow::ensure!(
            matches!(
                lifecycle,
                "queued"
                    | "execution_started"
                    | "completed"
                    | "failed"
                    | "blocked"
                    | "released_unstarted"
            ),
            "invalid lifecycle"
        );
        let _lock = OwnerTransaction::enter(&self.root)?;
        let ticket = &reservation.record;
        let path =
            self.authority_gate_path(ticket["thread_id"].as_str().context("missing thread")?);
        let rows = strict_rows(&path)?;
        let matching: Vec<_> = rows
            .iter()
            .filter(|row| row["operation_id"] == ticket["operation_id"])
            .collect();
        anyhow::ensure!(
            matching.first().copied() == Some(ticket),
            "reservation identity mismatch"
        );
        let previous = matching.last().context("missing reservation")?;
        if previous["lifecycle"] == lifecycle {
            return Ok((*previous).clone());
        }
        let expected = if lifecycle == "queued" {
            &["admitted"][..]
        } else if matches!(lifecycle, "execution_started" | "released_unstarted") {
            &["admitted", "queued"][..]
        } else {
            &["execution_started"][..]
        };
        anyhow::ensure!(
            previous["lifecycle"]
                .as_str()
                .is_some_and(|actual| expected.contains(&actual)),
            "cannot replay or refund uncertain execution"
        );
        if lifecycle == "execution_started" {
            self.validate_research_start_context(reservation, &rows)?;
        }
        let mut row = ticket.clone();
        row["record_type"] = json!(if lifecycle == "execution_started" {
            "research_budget_debit"
        } else {
            "research_budget_lifecycle"
        });
        row["record_id"] = json!(format!(
            "{}:{lifecycle}",
            ticket["record_id"].as_str().context("missing record id")?
        ));
        row["lifecycle"] = json!(lifecycle);
        row["execution_scope"] = json!(
            scope
                .or_else(|| previous["execution_scope"].as_str())
                .unwrap_or(if research_web_request(reservation.action()?)?.is_some() {
                    "web_fetch_and_generation_worker"
                } else {
                    "host_action_dispatch"
                })
        );
        row["input_delivered"] = json!("unknown");
        row["recorded_at"] = json!(chrono::Utc::now().to_rfc3339());
        self.append_jsonl(&path, &row)?;
        Ok(row)
    }

    pub(crate) fn claim_research_start(&self, reservation: &ResearchReservation) -> Result<()> {
        let _lock = OwnerTransaction::enter(&self.root)?;
        let ticket = &reservation.record;
        let rows = strict_rows(
            &self.authority_gate_path(ticket["thread_id"].as_str().context("missing thread")?),
        )?;
        let previous = rows
            .iter()
            .rev()
            .find(|row| row["operation_id"] == ticket["operation_id"])
            .context("missing reservation")?;
        anyhow::ensure!(
            matches!(previous["lifecycle"].as_str(), Some("admitted" | "queued")),
            "execution already started or uncertain; no automatic replay"
        );
        self.transition_research(reservation, "execution_started")?;
        Ok(())
    }
}
