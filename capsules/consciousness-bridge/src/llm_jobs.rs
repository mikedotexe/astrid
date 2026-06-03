//! Durable status records for long local LLM calls.

use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::PathBuf;

use anyhow::{Context, Result, anyhow};
use serde::{Deserialize, Serialize};
use serde_json::json;

use crate::paths::bridge_paths;

const SCHEMA_VERSION: u32 = 1;
const SYSTEM: &str = "astrid";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LlmJob {
    pub schema_version: u32,
    pub job_id: String,
    pub system: String,
    pub action_id: Option<String>,
    pub thread_id: Option<String>,
    pub action_text: String,
    pub call_kind: String,
    pub status: String,
    pub created_at: String,
    pub started_at: Option<String>,
    pub finished_at: Option<String>,
    pub timeout_s: u64,
    pub validation_contract: String,
    pub next_policy: String,
    pub prompt_path: String,
    pub result_path: String,
    pub artifact_refs: Vec<serde_json::Value>,
    pub error: Option<String>,
    pub summary: String,
    #[serde(default = "current_pid")]
    pub worker_pid: u32,
}

#[derive(Debug, Clone)]
pub struct LlmJobStore {
    jobs_dir: PathBuf,
    index_path: PathBuf,
    status_path: PathBuf,
}

impl LlmJobStore {
    #[must_use]
    pub fn for_astrid_workspace() -> Self {
        let root = bridge_paths().bridge_workspace().join("llm_jobs");
        Self {
            jobs_dir: root.join("jobs"),
            index_path: root.join("index.json"),
            status_path: bridge_paths()
                .bridge_workspace()
                .join("runtime/llm_jobs_status.json"),
        }
    }

    pub fn start_call(
        &self,
        call_kind: &str,
        prompt: &str,
        timeout_s: u64,
        validation_contract: &str,
        next_policy: &str,
    ) -> Result<LlmJob> {
        self.ensure_dirs()?;
        let job_id = self.unique_job_id(call_kind);
        let job_dir = self.jobs_dir.join(&job_id);
        fs::create_dir_all(&job_dir)?;
        let prompt_path = job_dir.join("prompt.txt");
        let result_path = job_dir.join("result.txt");
        fs::write(&prompt_path, prompt)?;
        fs::write(job_dir.join("events.jsonl"), "")?;
        let now = now();
        let job = LlmJob {
            schema_version: SCHEMA_VERSION,
            job_id,
            system: SYSTEM.to_string(),
            action_id: None,
            thread_id: None,
            action_text: call_kind.to_string(),
            call_kind: call_kind.to_string(),
            status: "running".to_string(),
            created_at: now.clone(),
            started_at: Some(now.clone()),
            finished_at: None,
            timeout_s,
            validation_contract: validation_contract.to_string(),
            next_policy: next_policy.to_string(),
            prompt_path: prompt_path.display().to_string(),
            result_path: result_path.display().to_string(),
            artifact_refs: Vec::new(),
            error: None,
            summary: format!("Running {call_kind} LLM call."),
            worker_pid: current_pid(),
        };
        self.write_job(&job)?;
        self.append_event(&job.job_id, "running", &job.summary, None)?;
        self.update_index(&job)?;
        self.write_runtime_status()?;
        Ok(job)
    }

    pub fn finish_call(
        &self,
        job_id: &str,
        status: &str,
        result: Option<&str>,
        summary: &str,
        error: Option<&str>,
    ) -> Result<LlmJob> {
        let mut job = self
            .read_job(job_id)?
            .ok_or_else(|| anyhow!("No LLM job matched `{job_id}`"))?;
        if is_terminal(&job.status) {
            self.append_event(
                job_id,
                "late_result_ignored",
                &format!(
                    "Late `{status}` result ignored because job is already `{}`.",
                    job.status
                ),
                None,
            )?;
            self.write_runtime_status()?;
            return Ok(job);
        }
        let status = if matches!(
            status,
            "completed" | "thin_output" | "timeout" | "failed" | "canceled"
        ) {
            status
        } else {
            "failed"
        };
        let status =
            if job.status == "cancel_requested" && matches!(status, "completed" | "thin_output") {
                "canceled"
            } else {
                status
            };
        if let Some(result) = result
            && status != "canceled"
        {
            fs::write(&job.result_path, result)?;
        }
        job.status = status.to_string();
        job.finished_at = Some(now());
        job.summary = summary.to_string();
        job.error = error.map(str::to_string);
        self.write_job(&job)?;
        self.append_event(job_id, status, summary, error)?;
        self.update_index(&job)?;
        self.write_runtime_status()?;
        Ok(job)
    }

    pub fn request_cancel(&self, selector: Option<&str>) -> Result<LlmJob> {
        self.expire_timed_out_jobs()?;
        let mut job = self
            .resolve(selector)?
            .ok_or_else(|| anyhow!("No LLM job matched `{}`", selector.unwrap_or("latest")))?;
        if job.status == "running" {
            job.status = "cancel_requested".to_string();
            job.summary =
                "Cancel requested; running LLM call will be discarded when it returns.".to_string();
            self.write_job(&job)?;
            self.append_event(&job.job_id, "cancel_requested", &job.summary, None)?;
            self.update_index(&job)?;
            self.write_runtime_status()?;
            return Ok(job);
        }
        if job.status == "queued" {
            return self.finish_call(
                &job.job_id,
                "canceled",
                None,
                "Canceled before worker start.",
                None,
            );
        }
        Ok(job)
    }

    pub fn status_text(&self, selector: Option<&str>) -> Result<String> {
        self.expire_timed_out_jobs()?;
        let Some(job) = self.resolve(selector)? else {
            return Ok(format!(
                "No LLM job matched `{}`.",
                selector.unwrap_or("latest")
            ));
        };
        Ok(format!(
            "LLM job `{}` [{}]\nAction: {}\nCall kind: {}\nAction id: {}\nThread id: {}\nElapsed: {}\nValidation: {}\nNEXT policy: {}\nSummary: {}",
            job.job_id,
            job.status,
            job.action_text,
            job.call_kind,
            job.action_id.as_deref().unwrap_or("(pending)"),
            job.thread_id.as_deref().unwrap_or("(none)"),
            elapsed_text(&job),
            job.validation_contract,
            job.next_policy,
            job.summary,
        ))
    }

    pub fn active_summary(&self) -> Option<String> {
        let _ = self.expire_timed_out_jobs();
        self.list_jobs(20).ok().and_then(|jobs| {
            jobs.into_iter()
                .rev()
                .find(|job| is_active(&job.status))
                .map(|job| {
                    format!(
                        "LLM job running: {} ({}, elapsed {}). Use ACTION_STATUS latest.",
                        job.action_text,
                        job.job_id,
                        elapsed_text(&job)
                    )
                })
        })
    }

    pub fn write_runtime_status(&self) -> Result<()> {
        self.ensure_dirs_no_recover()?;
        self.expire_timed_out_jobs()?;
        let jobs = self.list_jobs(12)?;
        let active = jobs
            .iter()
            .filter(|job| is_active(&job.status))
            .map(compact_job)
            .collect::<Vec<_>>();
        let recent = jobs
            .iter()
            .rev()
            .take(8)
            .map(compact_job)
            .collect::<Vec<_>>();
        fs::write(
            &self.status_path,
            serde_json::to_string_pretty(&json!({
                "schema_version": SCHEMA_VERSION,
                "system": SYSTEM,
                "updated_at": now(),
                "active_count": active.len(),
                "latest_job_id": jobs.last().map(|job| job.job_id.clone()),
                "active_jobs": active,
                "recent_jobs": recent,
            }))?,
        )?;
        Ok(())
    }

    fn resolve(&self, selector: Option<&str>) -> Result<Option<LlmJob>> {
        self.ensure_dirs()?;
        let selector = selector.unwrap_or("latest").trim();
        if selector.is_empty() || selector == "latest" {
            let index = self.read_index()?;
            return index
                .get("latest_job_id")
                .and_then(serde_json::Value::as_str)
                .map(|job_id| self.read_job(job_id))
                .transpose()
                .map(Option::flatten);
        }
        if selector.starts_with("job_") {
            return self.read_job(selector);
        }
        for job in self.list_jobs(50)?.into_iter().rev() {
            if job.action_id.as_deref() == Some(selector) || job.action_text.contains(selector) {
                return Ok(Some(job));
            }
        }
        Ok(None)
    }

    fn read_job(&self, job_id: &str) -> Result<Option<LlmJob>> {
        let path = self.jobs_dir.join(job_id).join("job.json");
        if !path.exists() {
            return Ok(None);
        }
        Ok(Some(serde_json::from_str(&fs::read_to_string(path)?)?))
    }

    fn list_jobs(&self, limit: usize) -> Result<Vec<LlmJob>> {
        self.ensure_dirs_no_recover()?;
        let mut jobs = Vec::new();
        for entry in fs::read_dir(&self.jobs_dir)? {
            let Ok(entry) = entry else { continue };
            let path = entry.path().join("job.json");
            if !path.exists() {
                continue;
            }
            if let Ok(job) = serde_json::from_str::<LlmJob>(&fs::read_to_string(path)?) {
                jobs.push(job);
            }
        }
        jobs.sort_by(|a, b| a.created_at.cmp(&b.created_at));
        if jobs.len() > limit {
            Ok(jobs.split_off(jobs.len().saturating_sub(limit)))
        } else {
            Ok(jobs)
        }
    }

    fn ensure_dirs(&self) -> Result<()> {
        self.ensure_dirs_no_recover()?;
        self.recover_stale_running_jobs()?;
        self.expire_timed_out_jobs()?;
        Ok(())
    }

    fn ensure_dirs_no_recover(&self) -> Result<()> {
        fs::create_dir_all(&self.jobs_dir)?;
        if let Some(parent) = self.status_path.parent() {
            fs::create_dir_all(parent)?;
        }
        if !self.index_path.exists() {
            fs::write(
                &self.index_path,
                serde_json::to_string_pretty(&json!({
                    "schema_version": SCHEMA_VERSION,
                    "system": SYSTEM,
                    "latest_job_id": null,
                    "recent_jobs": [],
                    "updated_at": now(),
                }))?,
            )?;
        }
        Ok(())
    }

    fn recover_stale_running_jobs(&self) -> Result<()> {
        for mut job in self.list_jobs(100)? {
            if !is_active(&job.status) {
                continue;
            }
            if job.worker_pid == current_pid() {
                continue;
            }
            let result_path = PathBuf::from(&job.result_path);
            if result_path.exists() && result_path.metadata().is_ok_and(|meta| meta.len() > 0) {
                continue;
            }
            job.status = "failed".to_string();
            job.finished_at = Some(now());
            job.error = Some("worker_restarted_before_completion".to_string());
            job.summary = "Worker restarted before completion; result was not written.".to_string();
            self.write_job(&job)?;
            self.append_event(&job.job_id, "failed", &job.summary, job.error.as_deref())?;
        }
        Ok(())
    }

    fn expire_timed_out_jobs(&self) -> Result<()> {
        self.ensure_dirs_no_recover()?;
        let now_dt = chrono::Utc::now();
        for mut job in self.list_jobs(100)? {
            if !is_active(&job.status) || job.timeout_s == 0 {
                continue;
            }
            let start = job.started_at.as_ref().unwrap_or(&job.created_at);
            let Ok(start_dt) = chrono::DateTime::parse_from_rfc3339(start) else {
                continue;
            };
            let elapsed = now_dt.signed_duration_since(start_dt.with_timezone(&chrono::Utc));
            let Ok(timeout_s) = i64::try_from(job.timeout_s) else {
                continue;
            };
            if elapsed.num_seconds() <= timeout_s {
                continue;
            }
            job.status = "timeout".to_string();
            job.finished_at = Some(now());
            job.error = Some("llm_job_timeout".to_string());
            job.summary = format!(
                "Timed out after {}s; any late worker result will be ignored.",
                job.timeout_s
            );
            self.write_job(&job)?;
            self.append_event(&job.job_id, "timeout", &job.summary, job.error.as_deref())?;
            self.update_index(&job)?;
        }
        Ok(())
    }

    fn update_index(&self, job: &LlmJob) -> Result<()> {
        let mut index = self.read_index()?;
        let mut recent = index
            .get("recent_jobs")
            .and_then(serde_json::Value::as_array)
            .cloned()
            .unwrap_or_default()
            .into_iter()
            .filter(|value| value.as_str() != Some(job.job_id.as_str()))
            .collect::<Vec<_>>();
        recent.push(json!(job.job_id));
        if recent.len() > 30 {
            recent = recent.split_off(recent.len().saturating_sub(30));
        }
        index["latest_job_id"] = json!(job.job_id);
        index["recent_jobs"] = json!(recent);
        index["updated_at"] = json!(now());
        fs::write(&self.index_path, serde_json::to_string_pretty(&index)?)?;
        Ok(())
    }

    fn read_index(&self) -> Result<serde_json::Value> {
        self.ensure_dirs_no_recover()?;
        Ok(serde_json::from_str(&fs::read_to_string(
            &self.index_path,
        )?)?)
    }

    fn write_job(&self, job: &LlmJob) -> Result<()> {
        fs::write(
            self.jobs_dir.join(&job.job_id).join("job.json"),
            serde_json::to_string_pretty(job)?,
        )?;
        Ok(())
    }

    fn append_event(
        &self,
        job_id: &str,
        event: &str,
        summary: &str,
        error: Option<&str>,
    ) -> Result<()> {
        let path = self.jobs_dir.join(job_id).join("events.jsonl");
        let mut file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(path)
            .context("open llm job events")?;
        writeln!(
            file,
            "{}",
            serde_json::to_string(&json!({
                "event": event,
                "at": now(),
                "summary": summary,
                "error": error,
            }))?
        )?;
        Ok(())
    }

    fn unique_job_id(&self, call_kind: &str) -> String {
        let millis = chrono::Utc::now().timestamp_millis();
        let slug = slug(call_kind);
        let root = format!("job_{SYSTEM}_{millis}_{slug}");
        let mut candidate = root.clone();
        let mut suffix = 2_u32;
        while self.jobs_dir.join(&candidate).exists() {
            candidate = format!("{root}_{suffix}");
            suffix = suffix.saturating_add(1);
        }
        candidate
    }
}

pub fn start_call(
    call_kind: &str,
    prompt: &str,
    timeout_s: u64,
    validation_contract: &str,
    next_policy: &str,
) -> Option<LlmJob> {
    LlmJobStore::for_astrid_workspace()
        .start_call(
            call_kind,
            prompt,
            timeout_s,
            validation_contract,
            next_policy,
        )
        .ok()
}

pub fn finish_call(
    job: Option<&LlmJob>,
    status: &str,
    result: Option<&str>,
    summary: &str,
    error: Option<&str>,
) -> Option<LlmJob> {
    if let Some(job) = job {
        return LlmJobStore::for_astrid_workspace()
            .finish_call(&job.job_id, status, result, summary, error)
            .ok();
    }
    None
}

pub fn status_text(selector: Option<&str>) -> Result<String> {
    LlmJobStore::for_astrid_workspace().status_text(selector)
}

pub fn cancel(selector: Option<&str>) -> Result<String> {
    let job = LlmJobStore::for_astrid_workspace().request_cancel(selector)?;
    Ok(format!(
        "LLM job `{}` is now `{}`.\n{}",
        job.job_id, job.status, job.summary
    ))
}

pub fn active_prompt_summary() -> Option<String> {
    LlmJobStore::for_astrid_workspace().active_summary()
}

pub fn runtime_status() -> Option<serde_json::Value> {
    if cfg!(test) {
        return None;
    }
    let store = LlmJobStore::for_astrid_workspace();
    let _ = store.write_runtime_status();
    fs::read_to_string(&store.status_path)
        .ok()
        .and_then(|raw| serde_json::from_str(&raw).ok())
}

fn compact_job(job: &LlmJob) -> serde_json::Value {
    json!({
        "job_id": job.job_id,
        "action_id": job.action_id,
        "thread_id": job.thread_id,
        "action_text": job.action_text,
        "call_kind": job.call_kind,
        "status": job.status,
        "created_at": job.created_at,
        "started_at": job.started_at,
        "finished_at": job.finished_at,
        "elapsed": elapsed_text(job),
        "summary": job.summary,
        "worker_pid": job.worker_pid,
    })
}

fn elapsed_text(job: &LlmJob) -> String {
    let start = job.started_at.as_ref().unwrap_or(&job.created_at);
    let Ok(start) = chrono::DateTime::parse_from_rfc3339(start) else {
        return "unknown".to_string();
    };
    let end = job
        .finished_at
        .as_deref()
        .and_then(|value| chrono::DateTime::parse_from_rfc3339(value).ok())
        .unwrap_or_else(|| chrono::Utc::now().into());
    let secs = end.signed_duration_since(start).num_seconds().max(0);
    format!("{secs}s")
}

fn now() -> String {
    chrono::Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Millis, true)
}

fn current_pid() -> u32 {
    std::process::id()
}

fn is_active(status: &str) -> bool {
    matches!(status, "running" | "cancel_requested" | "queued")
}

fn is_terminal(status: &str) -> bool {
    matches!(
        status,
        "completed" | "thin_output" | "timeout" | "failed" | "canceled"
    )
}

fn slug(text: &str) -> String {
    let mut out = String::new();
    let mut last_dash = false;
    for ch in text.to_ascii_lowercase().chars() {
        if ch.is_ascii_alphanumeric() {
            out.push(ch);
            last_dash = false;
        } else if !last_dash {
            out.push('-');
            last_dash = true;
        }
        if out.len() >= 48 {
            break;
        }
    }
    out.trim_matches('-').to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_store(name: &str) -> LlmJobStore {
        let root = std::env::temp_dir().join(format!(
            "astrid_llm_jobs_test_{}_{}",
            name,
            chrono::Utc::now().timestamp_nanos_opt().unwrap_or_default()
        ));
        LlmJobStore {
            jobs_dir: root.join("jobs"),
            index_path: root.join("index.json"),
            status_path: root.join("runtime/llm_jobs_status.json"),
        }
    }

    #[test]
    fn status_text_for_missing_job_degrades() {
        let text = temp_store("missing")
            .status_text(Some("job_missing"))
            .expect("status text");
        assert!(text.contains("No LLM job matched"));
    }

    #[test]
    fn job_lifecycle_writes_status_and_result() {
        let store = temp_store("lifecycle");
        let job = store
            .start_call(
                "introspect",
                "Read source",
                120,
                "strict_introspection_v1",
                "accepted",
            )
            .expect("start job");
        let status = store.status_text(Some("latest")).expect("status text");
        assert!(status.contains("introspect"));
        assert!(status.contains("strict_introspection_v1"));

        let completed = store
            .finish_call(&job.job_id, "completed", Some("Observed\n"), "done", None)
            .expect("finish job");
        assert_eq!(completed.status, "completed");
        assert!(PathBuf::from(completed.result_path).exists());
        let runtime = fs::read_to_string(store.status_path).expect("runtime status");
        assert!(runtime.contains(&job.job_id));
    }

    #[test]
    fn canceled_running_job_discards_late_success_result() {
        let store = temp_store("cancel_late_success");
        let job = store
            .start_call(
                "daydream",
                "prompt",
                90,
                "action_finalizer",
                "finalizer_owned",
            )
            .expect("start job");
        let canceled = store.request_cancel(Some(&job.job_id)).expect("cancel");
        assert_eq!(canceled.status, "cancel_requested");

        let finished = store
            .finish_call(
                &job.job_id,
                "completed",
                Some("late result"),
                "completed after cancellation",
                None,
            )
            .expect("finish job");
        assert_eq!(finished.status, "canceled");
        let result_path = PathBuf::from(finished.result_path);
        assert!(
            !result_path.exists()
                || fs::read_to_string(result_path)
                    .unwrap_or_default()
                    .is_empty()
        );
    }

    #[test]
    fn running_job_times_out_and_ignores_late_result() {
        let store = temp_store("timeout_late_success");
        let mut job = store
            .start_call(
                "introspect",
                "prompt",
                1,
                "strict_introspection_v1",
                "accepted_strict_review_only",
            )
            .expect("start job");
        job.started_at = Some("2026-05-10T00:00:00.000Z".to_string());
        store.write_job(&job).expect("write stale job");

        store.write_runtime_status().expect("runtime status");
        let timed_out = store
            .read_job(&job.job_id)
            .expect("read job")
            .expect("job present");
        assert_eq!(timed_out.status, "timeout");
        assert_eq!(timed_out.error.as_deref(), Some("llm_job_timeout"));

        let late = store
            .finish_call(&job.job_id, "completed", Some("late result"), "late", None)
            .expect("late finish");
        assert_eq!(late.status, "timeout");
        let result_path = PathBuf::from(late.result_path);
        assert!(
            !result_path.exists()
                || fs::read_to_string(result_path)
                    .unwrap_or_default()
                    .is_empty()
        );
    }
}
