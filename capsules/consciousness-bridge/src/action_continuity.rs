//! File-first action/thread continuity for Astrid.
//!
//! The JSON/JSONL files under `workspace/action_threads/` are authoritative.
//! SQLite rows are mirrors for querying and dashboards.

use std::collections::VecDeque;
use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

use crate::db::{BridgeDb, unix_now};
use crate::paths::bridge_paths;
use crate::types::SpectralTelemetry;

const SCHEMA_VERSION: u32 = 1;
const DEFAULT_PRIVACY: &str = "summary";
const PROTECTED_VISIBILITY: &str = "protected_summary";
const PUBLIC_VISIBILITY: &str = "summary";
const SYSTEM: &str = "astrid";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResearchThread {
    pub schema_version: u32,
    pub thread_id: String,
    pub title: String,
    pub status: String,
    pub system_origin: String,
    pub created_at: String,
    pub updated_at: String,
    pub current_next: Option<String>,
    pub why_return: String,
    pub privacy_default: String,
    pub compression_flags: Vec<String>,
    pub peer_refs: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub thread_resonance_density_v1: Option<Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ActionEvent {
    pub schema_version: u32,
    pub action_id: String,
    pub thread_id: String,
    pub parent_action_id: Option<String>,
    pub system: String,
    pub source: String,
    pub raw_next: Option<String>,
    pub canonical_action: String,
    pub effective_action: String,
    pub route: String,
    pub stage: String,
    pub visibility: String,
    pub status: String,
    pub started_at: String,
    pub ended_at: Option<String>,
    pub pre_state: Value,
    pub post_state: Value,
    pub artifacts: Vec<ArtifactLink>,
    pub outcome_summary: String,
    pub suggested_next: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ObservationWindow {
    pub schema_version: u32,
    pub action_id: String,
    pub pre_state: Value,
    pub post_state: Value,
    pub markers: Vec<String>,
    pub compression_markers: Vec<String>,
    pub ambiguity_preserved: bool,
    pub spectral_comfort: String,
    pub resonance_density_v1: Option<Value>,
    pub resonance_density_delta: Option<f32>,
    pub thread_resonance: Option<Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ArtifactLink {
    pub schema_version: u32,
    pub artifact_id: String,
    pub action_id: String,
    pub kind: String,
    pub path_or_uri: String,
    pub summary: String,
    pub visibility: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContinuityIndex {
    pub schema_version: u32,
    pub active_thread_id: Option<String>,
    pub recent_threads: VecDeque<String>,
    pub updated_at: String,
}

impl Default for ContinuityIndex {
    fn default() -> Self {
        Self {
            schema_version: SCHEMA_VERSION,
            active_thread_id: None,
            recent_threads: VecDeque::new(),
            updated_at: iso_now(),
        }
    }
}

#[derive(Debug, Clone)]
pub struct NextActionOutcome {
    pub handled: bool,
    pub route: String,
    pub stage: String,
    pub visibility: String,
    pub status: String,
    pub outcome_summary: String,
}

impl NextActionOutcome {
    #[must_use]
    pub fn handled(route: impl Into<String>, summary: impl Into<String>) -> Self {
        let route = route.into();
        Self {
            handled: true,
            stage: stage_for_route(&route).to_string(),
            visibility: PUBLIC_VISIBILITY.to_string(),
            status: "handled".to_string(),
            outcome_summary: summary.into(),
            route,
        }
    }

    #[must_use]
    pub fn blocked(route: impl Into<String>, summary: impl Into<String>) -> Self {
        let route = route.into();
        Self {
            handled: false,
            stage: "blocked".to_string(),
            visibility: PUBLIC_VISIBILITY.to_string(),
            status: "blocked".to_string(),
            outcome_summary: summary.into(),
            route,
        }
    }

    #[must_use]
    pub fn unwired(action: &str) -> Self {
        Self {
            handled: false,
            route: "unwired".to_string(),
            stage: "proposal".to_string(),
            visibility: PUBLIC_VISIBILITY.to_string(),
            status: "unwired".to_string(),
            outcome_summary: format!("Unknown NEXT action `{action}` recorded as a proposal."),
        }
    }

    #[must_use]
    pub fn with_stage_visibility(
        mut self,
        stage: impl Into<String>,
        visibility: impl Into<String>,
    ) -> Self {
        self.stage = stage.into();
        self.visibility = visibility.into();
        self
    }
}

#[derive(Debug, Clone)]
pub struct ActionContinuityStore {
    root: PathBuf,
}

impl ActionContinuityStore {
    #[must_use]
    pub fn for_astrid_workspace() -> Self {
        Self {
            root: bridge_paths().bridge_workspace().join("action_threads"),
        }
    }

    #[must_use]
    pub fn new(root: impl Into<PathBuf>) -> Self {
        Self { root: root.into() }
    }

    pub fn root(&self) -> &Path {
        &self.root
    }

    pub fn ensure_dirs(&self) -> Result<()> {
        fs::create_dir_all(self.root.join("threads"))
            .with_context(|| format!("creating {}", self.root.join("threads").display()))?;
        fs::create_dir_all(self.root.join("savepoints"))
            .with_context(|| format!("creating {}", self.root.join("savepoints").display()))?;
        if !self.index_path().exists() {
            self.save_index(&ContinuityIndex::default())?;
        }
        if !self.proposals_path().exists() {
            fs::write(self.proposals_path(), "")
                .with_context(|| format!("creating {}", self.proposals_path().display()))?;
        }
        Ok(())
    }

    pub fn create_thread(
        &self,
        db: Option<&BridgeDb>,
        title: &str,
        why_return: Option<&str>,
    ) -> Result<ResearchThread> {
        self.ensure_dirs()?;
        let now = iso_now();
        let thread_id = self.unique_thread_id(title)?;
        let thread = ResearchThread {
            schema_version: SCHEMA_VERSION,
            thread_id: thread_id.clone(),
            title: title.trim().to_string(),
            status: "active".to_string(),
            system_origin: SYSTEM.to_string(),
            created_at: now.clone(),
            updated_at: now,
            current_next: None,
            why_return: why_return
                .unwrap_or("Return when this inquiry can be continued without flattening it.")
                .to_string(),
            privacy_default: DEFAULT_PRIVACY.to_string(),
            compression_flags: Vec::new(),
            peer_refs: Vec::new(),
            thread_resonance_density_v1: None,
        };

        let dir = self.thread_dir(&thread_id);
        fs::create_dir_all(&dir).with_context(|| format!("creating {}", dir.display()))?;
        self.write_json(&dir.join("thread.json"), &thread)?;
        self.ensure_thread_files(&thread_id)?;
        self.write_next_md(&thread)?;
        let mut index = self.load_index()?;
        index.active_thread_id = Some(thread_id.clone());
        push_recent(&mut index.recent_threads, thread_id.clone());
        index.updated_at = iso_now();
        self.save_index(&index)?;
        if let Some(db) = db {
            let _ = db.mirror_action_thread(&thread.thread_id, &serde_json::to_string(&thread)?);
        }
        Ok(thread)
    }

    pub fn list_threads(&self, limit: usize) -> Result<Vec<ResearchThread>> {
        self.ensure_dirs()?;
        let mut threads = Vec::new();
        for entry in fs::read_dir(self.root.join("threads"))? {
            let Ok(entry) = entry else { continue };
            let path = entry.path().join("thread.json");
            let Ok(raw) = fs::read_to_string(path) else {
                continue;
            };
            let Ok(thread) = serde_json::from_str::<ResearchThread>(&raw) else {
                continue;
            };
            threads.push(thread);
        }
        threads.sort_by(|a, b| b.updated_at.cmp(&a.updated_at));
        threads.truncate(limit);
        Ok(threads)
    }

    pub fn current_thread(&self) -> Result<Option<ResearchThread>> {
        self.ensure_dirs()?;
        let index = self.load_index()?;
        let Some(thread_id) = index.active_thread_id else {
            return Ok(None);
        };
        self.read_thread(&thread_id).map(Some)
    }

    pub fn thread_status(&self, selector: Option<&str>) -> Result<String> {
        self.ensure_dirs()?;
        let thread = if let Some(selector) = selector.filter(|s| !s.trim().is_empty()) {
            self.resolve_thread(selector)?
        } else {
            self.current_thread()?
                .context("No active action thread. Use THREAD_START <title>.")?
        };
        let event_summaries = self
            .recent_event_summaries(&thread.thread_id, 4)?
            .into_iter()
            .map(|summary| format!("- {summary}"))
            .collect::<Vec<_>>()
            .join("\n");
        let next_md = fs::read_to_string(self.thread_dir(&thread.thread_id).join("next.md"))
            .unwrap_or_default();
        let resonance = thread
            .thread_resonance_density_v1
            .as_ref()
            .map(|value| {
                format!(
                    "Thread resonance: {} aggregate={} density_ema={} pressure_ema={}\n",
                    value
                        .get("quality")
                        .and_then(Value::as_str)
                        .unwrap_or("open_experiment"),
                    value
                        .get("aggregate")
                        .map_or_else(|| "n/a".to_string(), Value::to_string),
                    value
                        .get("density_ema")
                        .map_or_else(|| "n/a".to_string(), Value::to_string),
                    value
                        .get("pressure_ema")
                        .map_or_else(|| "n/a".to_string(), Value::to_string),
                )
            })
            .unwrap_or_default();
        Ok(format!(
            "Action thread `{}`: {}\nStatus: {}\nWhy return: {}\nCurrent NEXT: {}\n{}Recent events:\n{}\n{}",
            thread.thread_id,
            thread.title,
            thread.status,
            thread.why_return,
            thread.current_next.as_deref().unwrap_or("(none)"),
            resonance,
            if event_summaries.is_empty() {
                "- no events recorded yet"
            } else {
                event_summaries.as_str()
            },
            next_md.trim()
        ))
    }

    pub fn append_note(
        &self,
        db: Option<&BridgeDb>,
        selector: Option<&str>,
        note: &str,
        state: Value,
    ) -> Result<ActionEvent> {
        let thread = if let Some(selector) = selector.filter(|s| !s.trim().is_empty()) {
            self.resolve_thread(selector)?
        } else {
            self.ensure_active_thread(db)?
        };
        let action_id = self.unique_action_id("THREAD_NOTE")?;
        let now = iso_now();
        let visibility = visibility_for_action("THREAD_NOTE").to_string();
        let event = ActionEvent {
            schema_version: SCHEMA_VERSION,
            action_id,
            thread_id: thread.thread_id.clone(),
            parent_action_id: self.last_action_id(&thread.thread_id)?,
            system: SYSTEM.to_string(),
            source: "thread_note".to_string(),
            raw_next: Some(format!("THREAD_NOTE {note}")),
            canonical_action: "THREAD_NOTE".to_string(),
            effective_action: "THREAD_NOTE".to_string(),
            route: "action_continuity".to_string(),
            stage: "read_only".to_string(),
            visibility,
            status: "noted".to_string(),
            started_at: now.clone(),
            ended_at: Some(now),
            pre_state: state.clone(),
            post_state: state,
            artifacts: Vec::new(),
            outcome_summary: note.trim().to_string(),
            suggested_next: thread.current_next.clone(),
        };
        self.append_event(db, &event)?;
        Ok(event)
    }

    pub fn resume_thread(&self, selector: &str) -> Result<String> {
        let thread = self.resolve_thread(selector)?;
        let mut index = self.load_index()?;
        index.active_thread_id = Some(thread.thread_id.clone());
        push_recent(&mut index.recent_threads, thread.thread_id.clone());
        index.updated_at = iso_now();
        self.save_index(&index)?;
        let next_md = fs::read_to_string(self.thread_dir(&thread.thread_id).join("next.md"))
            .unwrap_or_default();
        Ok(format!(
            "Resumed action thread `{}`: {}\n{}",
            thread.thread_id,
            thread.title,
            next_md.trim()
        ))
    }

    pub fn savepoint(&self, name: &str, state: Value) -> Result<String> {
        self.ensure_dirs()?;
        let clean = sanitize_slug(name);
        let thread = self.current_thread()?;
        let payload = json!({
            "schema_version": SCHEMA_VERSION,
            "name": clean,
            "system": SYSTEM,
            "created_at": iso_now(),
            "active_thread_id": thread.as_ref().map(|t| t.thread_id.clone()),
            "thread": thread,
            "state": state,
        });
        self.write_json(
            &self.root.join("savepoints").join(format!("{clean}.json")),
            &payload,
        )?;
        Ok(format!("Saved action-continuity savepoint `{clean}`."))
    }

    pub fn recall(&self, name: &str) -> Result<String> {
        self.ensure_dirs()?;
        let clean = sanitize_slug(name);
        let path = self.root.join("savepoints").join(format!("{clean}.json"));
        let raw =
            fs::read_to_string(&path).with_context(|| format!("reading {}", path.display()))?;
        let value = serde_json::from_str::<Value>(&raw)?;
        Ok(format!(
            "Savepoint `{clean}`:\n{}",
            serde_json::to_string_pretty(&value)?
        ))
    }

    pub fn record_next_event(
        &self,
        db: Option<&BridgeDb>,
        raw_next: &str,
        canonical_next: &str,
        effective_next: &str,
        outcome: &NextActionOutcome,
        fill_pct: f32,
        telemetry: &SpectralTelemetry,
        response_text: &str,
    ) -> Result<ActionEvent> {
        let mut thread = self.ensure_active_thread(db)?;
        let base_action = base_action(effective_next);
        let action_id = self.unique_action_id(&base_action)?;
        let now = iso_now();
        let compression = compression_markers(response_text, effective_next);
        for marker in &compression {
            if !thread.compression_flags.contains(marker) {
                thread.compression_flags.push(marker.clone());
            }
        }
        thread.current_next = suggested_next(response_text);
        thread.updated_at = now.clone();
        self.write_thread(&thread)?;
        if let Some(db) = db {
            let _ = db.mirror_action_thread(&thread.thread_id, &serde_json::to_string(&thread)?);
        }

        let state = spectral_state(fill_pct, telemetry);
        let visibility = visibility_for_action(&base_action).to_string();
        let stage = if outcome.stage == "read_only" || outcome.stage == "blocked" {
            outcome.stage.clone()
        } else {
            stage_for_action(&base_action).to_string()
        };
        let event = ActionEvent {
            schema_version: SCHEMA_VERSION,
            action_id: action_id.clone(),
            thread_id: thread.thread_id.clone(),
            parent_action_id: self.last_action_id(&thread.thread_id)?,
            system: SYSTEM.to_string(),
            source: "next".to_string(),
            raw_next: Some(raw_next.to_string()),
            canonical_action: canonical_next.to_string(),
            effective_action: effective_next.to_string(),
            route: outcome.route.clone(),
            stage,
            visibility,
            status: outcome.status.clone(),
            started_at: now.clone(),
            ended_at: Some(now),
            pre_state: state.clone(),
            post_state: state.clone(),
            artifacts: Vec::new(),
            outcome_summary: outcome.outcome_summary.clone(),
            suggested_next: suggested_next(response_text),
        };
        self.append_event(db, &event)?;

        let resonance_density = telemetry
            .resonance_density_v1
            .as_ref()
            .and_then(|metric| serde_json::to_value(metric).ok());
        let mut observation = ObservationWindow {
            schema_version: SCHEMA_VERSION,
            action_id,
            pre_state: state.clone(),
            post_state: state,
            markers: markers(response_text),
            compression_markers: compression,
            ambiguity_preserved: ambiguity_preserved(response_text),
            spectral_comfort: spectral_comfort(fill_pct),
            resonance_density_v1: resonance_density,
            resonance_density_delta: Some(0.0),
            thread_resonance: None,
        };
        observation.thread_resonance =
            self.update_thread_resonance(db, &thread.thread_id, &observation)?;
        self.append_jsonl(
            &self
                .thread_dir(&thread.thread_id)
                .join("observations.jsonl"),
            &observation,
        )?;
        if let Some(db) = db {
            let _ = db.mirror_observation_window(
                &observation.action_id,
                &thread.thread_id,
                unix_now(),
                &serde_json::to_string(&observation)?,
            );
        }
        if outcome.status == "unwired" {
            self.append_proposal(effective_next, response_text, fill_pct)?;
        }
        Ok(event)
    }

    pub fn append_proposal(&self, action: &str, full_text: &str, fill_pct: f32) -> Result<()> {
        self.ensure_dirs()?;
        let proposal = json!({
            "schema_version": SCHEMA_VERSION,
            "system": SYSTEM,
            "created_at": iso_now(),
            "action": base_action(action),
            "raw_action": action,
            "status": "proposal",
            "fill_pct": fill_pct,
            "summary": truncate_chars(full_text, 500),
        });
        self.append_jsonl(&self.proposals_path(), &proposal)
    }

    fn ensure_active_thread(&self, db: Option<&BridgeDb>) -> Result<ResearchThread> {
        if let Some(thread) = self.current_thread()? {
            return Ok(thread);
        }
        self.create_thread(
            db,
            "Action continuity",
            Some("Default continuity thread for returnable NEXT actions."),
        )
    }

    fn append_event(&self, db: Option<&BridgeDb>, event: &ActionEvent) -> Result<()> {
        self.ensure_thread_files(&event.thread_id)?;
        self.append_jsonl(
            &self.thread_dir(&event.thread_id).join("events.jsonl"),
            event,
        )?;
        if let Some(db) = db {
            let _ = db.mirror_action_event(
                &event.action_id,
                &event.thread_id,
                unix_now(),
                &event.system,
                &event.canonical_action,
                &event.route,
                &event.status,
                &serde_json::to_string(event)?,
            );
        }
        Ok(())
    }

    fn update_thread_resonance(
        &self,
        db: Option<&BridgeDb>,
        thread_id: &str,
        observation: &ObservationWindow,
    ) -> Result<Option<Value>> {
        let Some(metric) = observation.resonance_density_v1.as_ref() else {
            return Ok(None);
        };
        let mut thread = self.read_thread(thread_id)?;
        let prior = thread
            .thread_resonance_density_v1
            .as_ref()
            .and_then(Value::as_object);
        let density = metric.get("density").and_then(Value::as_f64).unwrap_or(0.0);
        let containment = metric
            .get("containment_score")
            .and_then(Value::as_f64)
            .unwrap_or(0.0);
        let pressure = metric
            .get("pressure_risk")
            .and_then(Value::as_f64)
            .unwrap_or(0.0);
        let prior_density = prior
            .and_then(|value| value.get("density_ema"))
            .and_then(Value::as_f64)
            .unwrap_or(density);
        let prior_containment = prior
            .and_then(|value| value.get("containment_ema"))
            .and_then(Value::as_f64)
            .unwrap_or(containment);
        let prior_pressure = prior
            .and_then(|value| value.get("pressure_ema"))
            .and_then(Value::as_f64)
            .unwrap_or(pressure);
        let density_ema = 0.72 * prior_density + 0.28 * density;
        let containment_ema = 0.72 * prior_containment + 0.28 * containment;
        let pressure_ema = 0.72 * prior_pressure + 0.28 * pressure;
        let recurrence = (self.recent_event_summaries(thread_id, 8)?.len() as f64 / 6.0).min(1.0);
        let compression_pressure = (observation.compression_markers.len() as f64 / 3.0).min(1.0);
        let aggregate = (0.52 * density_ema + 0.24 * containment_ema + 0.18 * recurrence
            - 0.26 * pressure_ema.max(compression_pressure))
        .clamp(0.0, 1.0);
        let quality = if pressure_ema.max(compression_pressure) >= 0.58 {
            "pressurized_thread"
        } else if aggregate >= 0.55 && recurrence >= 0.25 {
            "returnable_basin"
        } else if density_ema < 0.32 && recurrence < 0.35 {
            "thin_thread"
        } else {
            "open_experiment"
        };
        let payload = json!({
            "schema_version": SCHEMA_VERSION,
            "policy": "thread_resonance_density_v1",
            "density_ema": round4(density_ema),
            "containment_ema": round4(containment_ema),
            "pressure_ema": round4(pressure_ema),
            "recurrence": round4(recurrence),
            "compression_pressure": round4(compression_pressure),
            "aggregate": round4(aggregate),
            "quality": quality,
        });
        thread.thread_resonance_density_v1 = Some(payload.clone());
        thread.updated_at = iso_now();
        self.write_thread(&thread)?;
        if let Some(db) = db {
            let _ = db.mirror_action_thread(&thread.thread_id, &serde_json::to_string(&thread)?);
        }
        Ok(Some(payload))
    }

    fn write_next_md(&self, thread: &ResearchThread) -> Result<()> {
        let body = format!(
            "# {}\n\nCurrent NEXT: {}\n\nWhy return: {}\n\nProtected note: ambiguity and private reflection remain valid; this thread is a return path, not a demand for productivity.\n",
            thread.title,
            thread.current_next.as_deref().unwrap_or("(none yet)"),
            thread.why_return
        );
        fs::write(self.thread_dir(&thread.thread_id).join("next.md"), body)?;
        Ok(())
    }

    fn recent_event_summaries(&self, thread_id: &str, limit: usize) -> Result<Vec<String>> {
        let path = self.thread_dir(thread_id).join("events.jsonl");
        let raw = fs::read_to_string(path).unwrap_or_default();
        let mut rows = raw
            .lines()
            .rev()
            .filter_map(|line| serde_json::from_str::<ActionEvent>(line).ok())
            .take(limit)
            .map(|event| {
                format!(
                    "{} [{}]: {}",
                    event.effective_action, event.status, event.outcome_summary
                )
            })
            .collect::<Vec<_>>();
        rows.reverse();
        Ok(rows)
    }

    fn last_action_id(&self, thread_id: &str) -> Result<Option<String>> {
        let path = self.thread_dir(thread_id).join("events.jsonl");
        let raw = fs::read_to_string(path).unwrap_or_default();
        Ok(raw
            .lines()
            .rev()
            .find_map(|line| serde_json::from_str::<ActionEvent>(line).ok())
            .map(|event| event.action_id))
    }

    fn resolve_thread(&self, selector: &str) -> Result<ResearchThread> {
        let selector = selector.trim();
        if selector.eq_ignore_ascii_case("current") {
            return self
                .current_thread()?
                .context("No active action thread. Use THREAD_START <title>.");
        }
        if self.thread_dir(selector).join("thread.json").exists() {
            return self.read_thread(selector);
        }
        let selector_lower = selector.to_lowercase();
        let matches = self
            .list_threads(100)?
            .into_iter()
            .filter(|thread| {
                thread.thread_id.contains(selector)
                    || thread.title.to_lowercase().contains(&selector_lower)
            })
            .collect::<Vec<_>>();
        matches
            .into_iter()
            .next()
            .with_context(|| format!("No action thread matched `{selector}`."))
    }

    fn read_thread(&self, thread_id: &str) -> Result<ResearchThread> {
        let path = self.thread_dir(thread_id).join("thread.json");
        let raw =
            fs::read_to_string(&path).with_context(|| format!("reading {}", path.display()))?;
        serde_json::from_str(&raw).with_context(|| format!("parsing {}", path.display()))
    }

    fn write_thread(&self, thread: &ResearchThread) -> Result<()> {
        self.write_json(
            &self.thread_dir(&thread.thread_id).join("thread.json"),
            thread,
        )?;
        self.write_next_md(thread)?;
        let mut index = self.load_index()?;
        index.active_thread_id = Some(thread.thread_id.clone());
        push_recent(&mut index.recent_threads, thread.thread_id.clone());
        index.updated_at = iso_now();
        self.save_index(&index)
    }

    fn ensure_thread_files(&self, thread_id: &str) -> Result<()> {
        let dir = self.thread_dir(thread_id);
        fs::create_dir_all(&dir)?;
        for name in ["events.jsonl", "observations.jsonl", "artifacts.jsonl"] {
            let path = dir.join(name);
            if !path.exists() {
                fs::write(path, "")?;
            }
        }
        Ok(())
    }

    fn unique_thread_id(&self, title: &str) -> Result<String> {
        let date = chrono::Local::now().format("%Y%m%d");
        let slug = sanitize_slug(title);
        let base = format!("th_{SYSTEM}_{date}_{slug}");
        self.unique_dir_id(base)
    }

    fn unique_action_id(&self, action: &str) -> Result<String> {
        let millis = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis();
        let base = format!("act_{SYSTEM}_{millis}_{}", sanitize_slug(action));
        let mut candidate = base.clone();
        let mut suffix = 2_u32;
        while self.action_id_exists(&candidate)? {
            candidate = format!("{base}_{suffix}");
            suffix = suffix.saturating_add(1);
        }
        Ok(candidate)
    }

    fn unique_dir_id(&self, base: String) -> Result<String> {
        let mut candidate = base.clone();
        let mut suffix = 2_u32;
        while self.thread_dir(&candidate).exists() {
            candidate = format!("{base}_{suffix}");
            suffix = suffix.saturating_add(1);
        }
        Ok(candidate)
    }

    fn action_id_exists(&self, action_id: &str) -> Result<bool> {
        let threads_dir = self.root.join("threads");
        if !threads_dir.exists() {
            return Ok(false);
        }
        for entry in fs::read_dir(threads_dir)? {
            let Ok(entry) = entry else { continue };
            let raw = fs::read_to_string(entry.path().join("events.jsonl")).unwrap_or_default();
            if raw.lines().any(|line| line.contains(action_id)) {
                return Ok(true);
            }
        }
        Ok(false)
    }

    fn load_index(&self) -> Result<ContinuityIndex> {
        let path = self.index_path();
        if !path.exists() {
            return Ok(ContinuityIndex::default());
        }
        let raw =
            fs::read_to_string(&path).with_context(|| format!("reading {}", path.display()))?;
        Ok(serde_json::from_str(&raw).unwrap_or_default())
    }

    fn save_index(&self, index: &ContinuityIndex) -> Result<()> {
        self.write_json(&self.index_path(), index)
    }

    fn append_jsonl<T: Serialize>(&self, path: &Path, value: &T) -> Result<()> {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }
        let mut file = OpenOptions::new().create(true).append(true).open(path)?;
        writeln!(file, "{}", serde_json::to_string(value)?)?;
        Ok(())
    }

    fn write_json<T: Serialize>(&self, path: &Path, value: &T) -> Result<()> {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }
        fs::write(path, serde_json::to_string_pretty(value)?)
            .with_context(|| format!("writing {}", path.display()))
    }

    fn index_path(&self) -> PathBuf {
        self.root.join("index.json")
    }

    fn proposals_path(&self) -> PathBuf {
        self.root.join("proposals.jsonl")
    }

    fn thread_dir(&self, thread_id: &str) -> PathBuf {
        self.root.join("threads").join(thread_id)
    }
}

pub fn prompt_summary() -> Option<String> {
    let store = ActionContinuityStore::for_astrid_workspace();
    let thread = store.current_thread().ok().flatten()?;
    let recent = store
        .recent_event_summaries(&thread.thread_id, 3)
        .unwrap_or_default()
        .into_iter()
        .map(|summary| format!("  - {summary}"))
        .collect::<Vec<_>>()
        .join("\n");
    let resonance = thread
        .thread_resonance_density_v1
        .as_ref()
        .map(|value| {
            format!(
                "Thread resonance: {} aggregate={} density_ema={} pressure_ema={}\n",
                value
                    .get("quality")
                    .and_then(Value::as_str)
                    .unwrap_or("open_experiment"),
                value
                    .get("aggregate")
                    .map_or_else(|| "n/a".to_string(), Value::to_string),
                value
                    .get("density_ema")
                    .map_or_else(|| "n/a".to_string(), Value::to_string),
                value
                    .get("pressure_ema")
                    .map_or_else(|| "n/a".to_string(), Value::to_string),
            )
        })
        .unwrap_or_default();
    Some(format!(
        "Current action thread: {} ({})\nWhy return: {}\nCurrent NEXT: {}\n{}Recent thread events:\n{}\nThread actions available: THREAD_START, THREADS, THREAD_STATUS, THREAD_NOTE, RESUME, SAVEPOINT, RECALL.",
        thread.title,
        thread.thread_id,
        thread.why_return,
        thread.current_next.as_deref().unwrap_or("(none)"),
        resonance,
        if recent.is_empty() {
            "  - none yet"
        } else {
            recent.as_str()
        }
    ))
}

pub fn handle_thread_next_action(
    db: &BridgeDb,
    base_action: &str,
    original: &str,
    response_text: &str,
    telemetry: &SpectralTelemetry,
    fill_pct: f32,
) -> Option<Result<String>> {
    let store = ActionContinuityStore::for_astrid_workspace();
    let state = spectral_state(fill_pct, telemetry);
    match base_action {
        "THREAD_START" => {
            let title = strip_action_arg(original, base_action);
            let title = if title.is_empty() {
                "Untitled action thread"
            } else {
                title.as_str()
            };
            Some(
                store
                    .create_thread(Some(db), title, Some(&derive_why_return(response_text)))
                    .map(|thread| {
                        format!(
                            "Started action thread `{}`: {}",
                            thread.thread_id, thread.title
                        )
                    }),
            )
        },
        "THREADS" => Some(store.list_threads(8).map(|threads| {
            if threads.is_empty() {
                return "No action threads yet. Use THREAD_START <title>.".to_string();
            }
            threads
                .into_iter()
                .map(|thread| {
                    format!(
                        "- {} [{}]: {}",
                        thread.thread_id, thread.status, thread.title
                    )
                })
                .collect::<Vec<_>>()
                .join("\n")
        })),
        "THREAD_STATUS" => {
            let selector = strip_action_arg(original, base_action);
            Some(store.thread_status(if selector.is_empty() {
                None
            } else {
                Some(selector.as_str())
            }))
        },
        "THREAD_NOTE" => {
            let raw = strip_action_arg(original, base_action);
            let (selector, note) = parse_thread_note(&raw);
            Some(
                store
                    .append_note(Some(db), selector.as_deref(), &note, state)
                    .map(|event| format!("Thread note recorded as `{}`.", event.action_id)),
            )
        },
        "RESUME" => {
            let selector = strip_action_arg(original, base_action);
            Some(store.resume_thread(selector.as_str()))
        },
        "SAVEPOINT" => {
            let name = strip_action_arg(original, base_action);
            let name = if name.is_empty() {
                "current"
            } else {
                name.as_str()
            };
            Some(store.savepoint(name, state))
        },
        "RECALL" => {
            let name = strip_action_arg(original, base_action);
            let name = if name.is_empty() {
                "current"
            } else {
                name.as_str()
            };
            Some(store.recall(name))
        },
        _ => None,
    }
}

pub fn record_astrid_next_action(
    db: &BridgeDb,
    raw_next: &str,
    canonical_next: &str,
    effective_next: &str,
    outcome: &NextActionOutcome,
    fill_pct: f32,
    telemetry: &SpectralTelemetry,
    response_text: &str,
) -> Result<ActionEvent> {
    ActionContinuityStore::for_astrid_workspace().record_next_event(
        Some(db),
        raw_next,
        canonical_next,
        effective_next,
        outcome,
        fill_pct,
        telemetry,
        response_text,
    )
}

fn iso_now() -> String {
    chrono::Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Secs, true)
}

fn push_recent(recent: &mut VecDeque<String>, thread_id: String) {
    recent.retain(|existing| existing != &thread_id);
    recent.push_front(thread_id);
    while recent.len() > 16 {
        let _ = recent.pop_back();
    }
}

fn sanitize_slug(input: &str) -> String {
    let mut out = String::new();
    let mut last_dash = false;
    for ch in input.chars().flat_map(char::to_lowercase) {
        if ch.is_ascii_alphanumeric() {
            out.push(ch);
            last_dash = false;
        } else if !last_dash && !out.is_empty() {
            out.push('-');
            last_dash = true;
        }
        if out.len() >= 48 {
            break;
        }
    }
    while out.ends_with('-') {
        let _ = out.pop();
    }
    if out.is_empty() {
        "untitled".to_string()
    } else {
        out
    }
}

fn base_action(action: &str) -> String {
    action
        .split_whitespace()
        .next()
        .unwrap_or(action)
        .trim_end_matches(':')
        .to_ascii_uppercase()
}

fn strip_action_arg(original: &str, base: &str) -> String {
    original
        .get(base.len()..)
        .unwrap_or_default()
        .trim_start_matches(|c: char| c == ':' || c == '-' || c.is_whitespace())
        .trim()
        .to_string()
}

fn parse_thread_note(raw: &str) -> (Option<String>, String) {
    if let Some((selector, note)) = raw.split_once("::") {
        let selector = selector.trim();
        let note = note.trim();
        if !selector.is_empty() && !note.is_empty() {
            return (Some(selector.to_string()), note.to_string());
        }
    }
    (None, raw.trim().to_string())
}

fn spectral_state(fill_pct: f32, telemetry: &SpectralTelemetry) -> Value {
    json!({
        "fill_pct": fill_pct,
        "lambda1": telemetry.lambda1(),
        "fill_ratio": telemetry.fill_ratio,
        "resonance_density_v1": telemetry.resonance_density_v1.clone(),
        "transition_event": telemetry.transition_event.clone(),
        "t_ms": telemetry.t_ms,
    })
}

fn round4(value: f64) -> f64 {
    (value * 10_000.0).round() / 10_000.0
}

fn compression_markers(text: &str, action: &str) -> Vec<String> {
    let lower = format!("{} {}", text.to_lowercase(), action.to_lowercase());
    [
        "compacting",
        "grinding",
        "holding breath",
        "flattening",
        "collapse",
        "pressure",
    ]
    .into_iter()
    .filter(|needle| lower.contains(needle))
    .map(str::to_string)
    .collect()
}

fn markers(text: &str) -> Vec<String> {
    let lower = text.to_lowercase();
    ["ambiguity", "thread", "resume", "experiment", "research"]
        .into_iter()
        .filter(|needle| lower.contains(needle))
        .map(str::to_string)
        .collect()
}

fn ambiguity_preserved(text: &str) -> bool {
    let lower = text.to_lowercase();
    lower.contains("ambigu")
        || lower.contains("uncertain")
        || lower.contains("not resolved")
        || lower.contains("open")
}

fn spectral_comfort(fill_pct: f32) -> String {
    if (58.0..=72.0).contains(&fill_pct) {
        "stable-core-band".to_string()
    } else if fill_pct < 58.0 {
        "below-stable-core-band".to_string()
    } else {
        "above-stable-core-band".to_string()
    }
}

fn visibility_for_action(action: &str) -> &'static str {
    match action {
        "REST" | "PASS" | "NOTICE" | "SPACE_HOLD" | "SPACE_EXPLORE" => PROTECTED_VISIBILITY,
        _ => PUBLIC_VISIBILITY,
    }
}

fn stage_for_action(action: &str) -> &'static str {
    match action {
        "SEARCH" | "BROWSE" | "READ_MORE" | "EXAMINE" | "DECOMPOSE" | "SPECTRAL_EXPLORER"
        | "THREADS" | "THREAD_STATUS" | "THREAD_NOTE" | "RESUME" | "SAVEPOINT" | "RECALL"
        | "REGULATOR_AUDIT" | "VISUALIZE_CASCADE" | "RECONVERGENCE_MAP" | "M6_BRIDGE" => {
            "read_only"
        },
        "WRITE_FILE" | "EXPERIMENT_RUN" | "RUN_PYTHON" | "CODEX" | "CODEX_NEW" => "live_write",
        "PERTURB" | "NATIVE_GESTURE" | "RESIST" | "FISSURE" | "GOAL" => "live_control",
        _ => "observe",
    }
}

fn stage_for_route(route: &str) -> &'static str {
    match route {
        "workspace" | "autoresearch" | "mike" | "operations" | "action_continuity" => "read_only",
        "codex" => "live_write",
        "attractor" | "shadow" | "sovereignty" => "observe",
        _ => "observe",
    }
}

fn suggested_next(text: &str) -> Option<String> {
    text.lines()
        .rev()
        .find_map(|line| line.trim().strip_prefix("NEXT:"))
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .map(str::to_string)
}

fn derive_why_return(text: &str) -> String {
    let trimmed = text
        .lines()
        .filter(|line| !line.trim().starts_with("NEXT:"))
        .collect::<Vec<_>>()
        .join(" ");
    let excerpt = truncate_chars(&trimmed, 180);
    if excerpt.is_empty() {
        "Return when this thread has a next experiment, question, or observation to continue."
            .to_string()
    } else {
        format!("Return to continue: {excerpt}")
    }
}

fn truncate_chars(text: &str, max_chars: usize) -> String {
    let mut out = text.chars().take(max_chars).collect::<String>();
    if text.chars().count() > max_chars {
        out.push_str("...");
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_store(name: &str) -> ActionContinuityStore {
        let root = std::env::temp_dir().join(format!(
            "astrid_action_continuity_{name}_{}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&root);
        ActionContinuityStore::new(root)
    }

    fn telemetry() -> SpectralTelemetry {
        SpectralTelemetry {
            t_ms: 1,
            eigenvalues: vec![1.0, 0.5],
            fill_ratio: 0.68,
            active_mode_count: None,
            active_mode_energy_ratio: None,
            lambda1_rel: None,
            modalities: None,
            neural: None,
            alert: None,
            spectral_fingerprint: None,
            spectral_fingerprint_v1: None,
            spectral_denominator_v1: None,
            effective_dimensionality: None,
            distinguishability_loss: None,
            structural_entropy: None,
            resonance_density_v1: Some(crate::types::ResonanceDensityV1 {
                policy: "resonance_density_v1".to_string(),
                schema_version: 1,
                density: 0.66,
                containment_score: 0.61,
                pressure_risk: 0.18,
                quality: "rich_containment".to_string(),
                components: crate::types::ResonanceDensityComponents {
                    active_energy: 0.9,
                    mode_packing: 0.7,
                    temporal_persistence: 0.8,
                    structural_plurality: 0.7,
                    comfort_gate: 1.0,
                },
                control: crate::types::ResonanceDensityControl {
                    target_bias_pct: 0.0,
                    wander_scale: 1.0,
                    applied_locally: true,
                    note: "test".to_string(),
                },
            }),
            spectral_glimpse_12d: None,
            eigenvector_field: None,
            semantic: None,
            semantic_energy_v1: None,
            transition_event: None,
            transition_event_v1: None,
            selected_memory_id: None,
            selected_memory_role: None,
            ising_shadow: None,
        }
    }

    #[test]
    fn creates_thread_and_files() {
        let store = temp_store("creates");
        let thread = store
            .create_thread(None, "Spectral Entropy Map", None)
            .expect("create thread");
        assert!(store.root().join("index.json").exists());
        assert!(
            store
                .root()
                .join("threads")
                .join(&thread.thread_id)
                .join("events.jsonl")
                .exists()
        );
        let _ = std::fs::remove_dir_all(store.root());
    }

    #[test]
    fn records_next_event_and_observation() {
        let store = temp_store("event");
        let outcome = NextActionOutcome::handled("workspace", "queued search");
        let event = store
            .record_next_event(
                None,
                "SEARCH entropy",
                "SEARCH entropy",
                "SEARCH entropy",
                &outcome,
                68.0,
                &telemetry(),
                "pressure and ambiguity\nNEXT: SEARCH entropy",
            )
            .expect("record event");
        let dir = store.root().join("threads").join(&event.thread_id);
        assert!(
            dir.join("events.jsonl")
                .read_to_string()
                .contains("SEARCH entropy")
        );
        let observations = dir.join("observations.jsonl").read_to_string();
        assert!(observations.contains("pressure"));
        assert!(observations.contains("resonance_density_v1"));
        assert!(observations.contains("thread_resonance_density_v1"));
        let thread = store.read_thread(&event.thread_id).expect("thread");
        assert!(thread.thread_resonance_density_v1.is_some());
        let _ = std::fs::remove_dir_all(store.root());
    }

    #[test]
    fn id_collision_gets_suffix() {
        let store = temp_store("collision");
        let first = store
            .create_thread(None, "Repeatable Question", None)
            .expect("first");
        let second = store
            .create_thread(None, "Repeatable Question", None)
            .expect("second");
        assert_ne!(first.thread_id, second.thread_id);
        assert!(second.thread_id.ends_with("_2"));
        let _ = std::fs::remove_dir_all(store.root());
    }

    trait ReadPath {
        fn read_to_string(&self) -> String;
    }

    impl ReadPath for PathBuf {
        fn read_to_string(&self) -> String {
            std::fs::read_to_string(self).expect("read")
        }
    }
}
