//! Only host-configured, Being-local execution records. No database or command execution.
use anyhow::{Context as _, Result, bail};
use serde_json::{Value, json};
use std::{
    fs,
    path::{Path, PathBuf},
};

pub(crate) struct RuntimeRecords {
    pub(crate) workspace: PathBuf,
    pub(crate) being: String,
}
impl RuntimeRecords {
    pub(crate) fn view(&self, target: &str) -> Result<String> {
        if !matches!(self.being.as_str(), "astrid" | "minime") {
            bail!("unknown trace owner");
        }
        let job_prefix = format!("job_{}_", self.being);
        let action_prefix = format!("act_{}_", self.being);
        if target != "LAST"
            && !(safe_id(target)
                && (target.starts_with(&job_prefix) || target.starts_with(&action_prefix)))
        {
            bail!("TRACE accepts LAST or one of your own job_/act_ identifiers, never a path");
        }
        let index = read_json(&self.workspace, &self.workspace.join("llm_jobs/index.json"))?;
        let mut ids = index["recent_jobs"]
            .as_array()
            .context("job index missing recent jobs")?
            .iter()
            .filter_map(Value::as_str)
            .take(256)
            .map(str::to_owned)
            .collect::<Vec<_>>();
        if target.starts_with(&job_prefix) {
            ids = vec![target.into()];
        }
        let mut jobs = Vec::new();
        for id in ids {
            if !safe_id(&id) || !id.starts_with(&job_prefix) {
                continue;
            }
            let path = self
                .workspace
                .join("llm_jobs/jobs")
                .join(&id)
                .join("job.json");
            let Ok(job) = read_json(&self.workspace, &path) else {
                continue;
            };
            if job["job_id"] != id || job["system"] != self.being {
                continue;
            }
            let action = job["action_text"].as_str().unwrap_or("");
            if (target == "LAST"
                && action.starts_with("SELF_STUDY")
                && !action.starts_with("SELF_STUDY TRACE"))
                || target == id
                || job["action_id"] == target
            {
                jobs.push((path, job));
            }
        }
        jobs.sort_by(|a, b| a.1["created_at"].as_str().cmp(&b.1["created_at"].as_str()));
        let Some((path, job)) = jobs.pop() else {
            return Ok("No matching retained study execution in the bounded recent index. No action or source delivery is inferred. Use TRACE job_<your-being>_<exact-id> for an older known job.".into());
        };
        let selected = select(&job, JOB_FIELDS);
        let mut artifacts = Vec::new();
        let mut execution = None;
        if let Some(refs) = job["artifact_refs"].as_array() {
            for item in refs.iter().take(12) {
                artifacts.push(select(item, &["kind", "artifact_id", "path_or_uri"]));
                if item["kind"] == "action_manifest"
                    && let Some(name) = item["path_or_uri"].as_str()
                {
                    let candidate = Path::new(name);
                    if candidate.starts_with(self.workspace.join("actions"))
                        && let Ok(record) = read_json(&self.workspace, candidate)
                        && record["action_id"] == job["action_id"]
                    {
                        execution = Some(select(
                            &record,
                            &[
                                "action_id",
                                "parent_action_id",
                                "canonical_action",
                                "effective_action",
                                "route",
                                "status",
                                "started_at",
                                "ended_at",
                                "outcome_summary",
                            ],
                        ));
                    }
                }
            }
        }
        let view = json!({"owner":self.being,"record":path,"parsed_record_sha256":crate::digest(serde_json::to_vec(&job)?),"job":selected,"linked_action_manifest":execution,"artifacts":artifacts,
            "limits":"Job status and linked manifest fields are recorded outcomes, not proof of delivery or understanding. No missing route, provider input or causal link is invented. Use exact source delivery receipts to establish received bytes. This is a bounded read, not an atomic runtime snapshot."});
        Ok(serde_json::to_string_pretty(&view)?)
    }
}
fn safe_id(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 200
        && value
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'_' | b'-'))
}
fn read_json(root: &Path, path: &Path) -> Result<Value> {
    let root = fs::canonicalize(root)?;
    let canonical = fs::canonicalize(path)?;
    if !canonical.starts_with(&root)
        || fs::symlink_metadata(path)?.file_type().is_symlink()
        || fs::metadata(&canonical)?.len() > 1024 * 1024
    {
        bail!("runtime record outside owner or size limit");
    }
    let before = fs::metadata(&canonical)?;
    let bytes = fs::read(&canonical)?;
    let after = fs::metadata(&canonical)?;
    if before.len() != after.len() || before.modified()? != after.modified()? {
        bail!("runtime record changed during read");
    }
    Ok(serde_json::from_slice(&bytes)?)
}
fn select(value: &Value, keys: &[&str]) -> Value {
    let mut result = serde_json::Map::new();
    for key in keys {
        if let Some(v) = value.get(*key) {
            let v = match v {
                Value::String(s) => Value::String(s.chars().take(400).collect()),
                Value::Object(_) | Value::Array(_) => {
                    Value::String("structured details omitted; consult the retained record".into())
                },
                _ => v.clone(),
            };
            result.insert((*key).into(), v);
        }
    }
    Value::Object(result)
}

const JOB_FIELDS: &[&str] = &[
    "job_id",
    "action_id",
    "action_text",
    "system",
    "status",
    "worker_status",
    "worker_pid",
    "created_at",
    "started_at",
    "finished_at",
    "timeout_s",
    "error",
    "outcome",
    "summary",
];
