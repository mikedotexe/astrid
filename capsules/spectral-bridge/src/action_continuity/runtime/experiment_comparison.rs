//! Explicit comparison of saved records, without selection or peer-reference writes.
use super::retained_inspection::{Snapshot, digest, safe_identity};
use super::*;

fn text_excerpt(value: &Value) -> Value {
    let Some(text) = value.as_str() else {
        return Value::Null;
    };
    json!({"text": text.chars().take(2048).collect::<String>(), "excerpt": text.chars().count() > 2048})
}

fn side_view(owner: &str, record: &Value, runs: &[Value]) -> Result<Value> {
    let rows = runs.iter().rev().take(4).map(|run| json!({
        "run_id": run["run_id"],
        "created_at": run["created_at"],
        "updated_at": run["updated_at"],
        "execution_receipt": {
            "stage": run["stage"], "status": run["status"],
            "result_summary": text_excerpt(&run["result_summary"]),
            "scope": "Recorded outcome, not an independent verification of execution",
        },
        "authored_interpretation": text_excerpt(&run["interpretation"]),
        "artifact_references": run["artifacts"].as_array().map(|a| a.iter().take(4).map(|v| json!({
            "artifact_id": v["artifact_id"], "kind": v["kind"],
            "path_or_uri": text_excerpt(&v["path_or_uri"]),
        })).collect::<Vec<_>>()).unwrap_or_default(),
        "artifact_reference_count": run["artifacts"].as_array().map_or(0, Vec::len),
        "artifact_validity": "not_verified",
    })).collect::<Vec<_>>();
    Ok(json!({
        "owner": owner, "experiment_id": record["experiment_id"],
        "record_sha256": digest(record)?, "status": record["status"],
        "created_at": record["created_at"], "updated_at": record["updated_at"],
        "authored_title": text_excerpt(&record["title"]),
        "authored_question": text_excerpt(&record["question"]),
        "authored_hypothesis": text_excerpt(&record["hypothesis"]),
        "recent_runs": rows, "retained_run_count": runs.len(),
        "shared_investigation": "Any recorded membership is an association, not a discovered similarity",
    }))
}

fn read_records(
    snapshot: &mut Snapshot,
    directory: &Path,
    id: &str,
    owner: &str,
) -> Result<(Value, Vec<Value>)> {
    anyhow::ensure!(
        safe_identity(id) && id.starts_with(&format!("exp_{owner}_")),
        "invalid owner-scoped experiment identity"
    );
    let records = snapshot.records::<Value>(&directory.join("experiments.jsonl"))?;
    for record in &records {
        let typed: ExperimentRecord =
            serde_json::from_value(record.clone()).context("malformed experiment record")?;
        anyhow::ensure!(
            typed.schema_version == 1
                && typed.experiment_id.starts_with(&format!("exp_{owner}_"))
                && directory.file_name().and_then(OsStr::to_str) == Some(typed.thread_id.as_str()),
            "experiment ownership or schema mismatch"
        );
    }
    let record = records
        .into_iter()
        .rev()
        .find(|r| r["experiment_id"] == id)
        .context("exact experiment record unavailable")?;
    anyhow::ensure!(
        directory.file_name().and_then(OsStr::to_str) == record["thread_id"].as_str(),
        "experiment thread identity mismatch"
    );
    let rows = snapshot.records::<Value>(&directory.join("experiment_runs.jsonl"))?;
    for row in &rows {
        let typed: ExperimentRunRecord =
            serde_json::from_value(row.clone()).context("malformed run record")?;
        anyhow::ensure!(
            typed.schema_version == 1
                && safe_identity(&typed.run_id)
                && typed.experiment_id.starts_with(&format!("exp_{owner}_")),
            "run ownership or schema mismatch"
        );
    }
    let runs = rows
        .into_iter()
        .filter(|r| r["experiment_id"] == id)
        .collect();
    Ok((record, runs))
}

fn peer_directories(root: &Path) -> Result<Vec<PathBuf>> {
    let mut directories = Vec::new();
    let entries = match fs::read_dir(root) {
        Ok(entries) => entries,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(directories),
        Err(error) => return Err(error.into()),
    };
    for entry in entries {
        let entry = entry?;
        if entry.file_type()?.is_dir() {
            directories.push(entry.path());
            anyhow::ensure!(
                directories.len() <= 256,
                "peer inspection directory bound exceeded"
            );
        }
    }
    directories.sort();
    Ok(directories)
}

impl ActionContinuityStore {
    pub(super) fn compare_retained_experiments(&self, selector: Option<&str>) -> Result<String> {
        let result = self.comparison_snapshot(selector);
        Ok(result.unwrap_or_else(|error| format!(
            "Experiment comparison unavailable: {error}. No selection, resumption, peer mutation or follow-up was performed. Existing status commands remain available."
        )))
    }

    fn comparison_snapshot(&self, selector: Option<&str>) -> Result<String> {
        let mut snapshot = Snapshot::default();
        let index = snapshot
            .json::<Value>(&self.index_path())?
            .context("no saved thread index")?;
        let thread_id = index["active_thread_id"]
            .as_str()
            .filter(|id| safe_identity(id))
            .context("no selected action thread")?;
        let thread = snapshot
            .json::<ResearchThread>(&self.thread_dir(thread_id).join("thread.json"))?
            .context("selected thread unavailable")?;
        anyhow::ensure!(
            thread.thread_id == thread_id && thread.system_origin == SYSTEM,
            "thread ownership mismatch"
        );
        let (left, right) = parse_experiment_compare(selector.unwrap_or("current"));
        let resolve = |selector: Option<&str>| -> Result<String> {
            let selector = selector.unwrap_or("current").trim();
            if selector.eq_ignore_ascii_case("current") || selector.is_empty() {
                return thread
                    .active_experiment_id
                    .clone()
                    .context("no selected experiment; choose an exact ID");
            }
            anyhow::ensure!(
                safe_identity(selector),
                "use an exact experiment ID or current"
            );
            Ok(selector.into())
        };
        let left_id = resolve(left.as_deref())?;
        let (left_record, left_runs) =
            read_records(&mut snapshot, &self.thread_dir(thread_id), &left_id, SYSTEM)?;
        let left_view = side_view(SYSTEM, &left_record, &left_runs)?;
        let right_id = resolve(right.as_deref())?;
        let mut peer_paths = None;
        let right_view = if right_id.starts_with(PEER_EXPERIMENT_PREFIX) {
            let root = bridge_paths()
                .minime_workspace()
                .join("action_threads/threads");
            let dirs = peer_directories(&root)?;
            let mut matched = None;
            for directory in &dirs {
                let records = snapshot.records::<Value>(&directory.join("experiments.jsonl"))?;
                if records.iter().any(|r| r["experiment_id"] == right_id) {
                    anyhow::ensure!(matched.is_none(), "ambiguous peer identity");
                    let (record, runs) =
                        read_records(&mut snapshot, directory, &right_id, PEER_SYSTEM)?;
                    matched = Some(side_view(PEER_SYSTEM, &record, &runs)?);
                }
            }
            peer_paths = Some((root, dirs));
            matched.unwrap_or_else(|| json!({"owner":PEER_SYSTEM,"experiment_id":right_id,"availability":"unavailable","reason":"Peer snapshot: not available from local action-thread files."}))
        } else {
            let (record, runs) = read_records(
                &mut snapshot,
                &self.thread_dir(thread_id),
                &right_id,
                SYSTEM,
            )?;
            side_view(SYSTEM, &record, &runs)?
        };
        let claims = snapshot.records::<Value>(&self.dossier_path(thread_id))?;
        let claim = claims
            .iter()
            .rev()
            .find(|record| {
                record["experiment_id"] == left_id
                    && record["being"] == SYSTEM
                    && record["record_type"] == "claim"
                    && record["claim_id"].as_str().is_some_and(safe_identity)
            })
            .and_then(|record| record["claim_id"].as_str());
        snapshot.validate()?;
        if let Some((root, before)) = peer_paths {
            anyhow::ensure!(
                peer_directories(&root)? == before,
                "peer catalog changed during read"
            );
        }
        let claim_choice = claim.map_or_else(String::new, |id| format!(
            "\nOptional counterevidence: DOSSIER_EVIDENCE {left_id} :: claim_id: {id}; evidence: <your words>; counterevidence: <your words>"
        ));
        Ok(format!(
            "Experiment comparison\nLeft:\n{}\nRight:\n{}\nThese are distinct saved records. Juxtaposition does not establish shared mechanism, causation, temporal correspondence or equivalent experience. Artifact references are not verified artifacts. Astrid cannot bind runs, close, or mutate the peer experiment. No activity was selected or scheduled.\nOptional authored analogy, difference or uncertainty: DOSSIER_CLAIM {left_id} :: claim: <your words>; basis: <your basis>; source_refs: {left_id},{right_id}; stance: hold{claim_choice}\nInspection: EXPERIMENT_STATUS {left_id}",
            serde_json::to_string_pretty(&left_view)?,
            serde_json::to_string_pretty(&right_view)?,
        ))
    }
}
