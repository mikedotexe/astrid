//! Unique-ID generation and existence checks for [`ActionContinuityStore`].
//!
//! Extracted verbatim from the monolithic `action_continuity.rs` as part of the
//! decomposition roadmap (item 2, tranche A1). Behavior-identical: deterministic
//! ID minting + filesystem existence scans over `self.root`. See
//! `docs/steward-notes/ARCHITECTURE_DECOMPOSITION_PLAN_2026-06-13.md`.

use super::*;

impl ActionContinuityStore {
    pub(super) fn unique_shared_investigation_id(&self, title: &str) -> Result<String> {
        let root = format!("si_{}_{}", now_millis(), sanitize_slug(title));
        let mut candidate = root.clone();
        let mut suffix: u32 = 2;
        while self
            .shared_investigation_dir(&candidate)
            .join("investigation.json")
            .exists()
        {
            candidate = format!("{root}_{suffix}");
            suffix = suffix.saturating_add(1);
        }
        Ok(candidate)
    }

    pub(super) fn unique_shared_record_id(
        &self,
        investigation_id: &str,
        kind: &str,
    ) -> Result<String> {
        let root = format!(
            "{kind}_{SYSTEM}_{}_{}",
            now_millis(),
            sanitize_slug(investigation_id)
        );
        let filename = if kind == "claim" {
            "claims.jsonl"
        } else {
            "decisions.jsonl"
        };
        let path = self
            .shared_investigation_dir(investigation_id)
            .join(filename);
        let existing = fs::read_to_string(path).unwrap_or_default();
        let mut candidate = root.clone();
        let mut suffix: u32 = 2;
        while existing.contains(&candidate) {
            candidate = format!("{root}_{suffix}");
            suffix = suffix.saturating_add(1);
        }
        Ok(candidate)
    }

    pub(super) fn unique_thread_id(&self, title: &str) -> Result<String> {
        let date = chrono::Local::now().format("%Y%m%d");
        let slug = sanitize_slug(title);
        let base = format!("th_{SYSTEM}_{date}_{slug}");
        self.unique_dir_id(base)
    }

    pub(super) fn unique_action_id(&self, action: &str) -> Result<String> {
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

    pub(super) fn unique_experiment_id(&self, title: &str) -> Result<String> {
        let date = chrono::Local::now().format("%Y%m%d");
        let base = format!("exp_{SYSTEM}_{date}_{}", sanitize_slug(title));
        let mut candidate = base.clone();
        let mut suffix = 2_u32;
        while self.experiment_id_exists(&candidate)? {
            candidate = format!("{base}_{suffix}");
            suffix = suffix.saturating_add(1);
        }
        Ok(candidate)
    }

    pub(super) fn unique_run_id(&self, action_text: &str) -> Result<String> {
        let millis = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis();
        let base = format!("run_{SYSTEM}_{millis}_{}", sanitize_slug(action_text));
        let mut candidate = base.clone();
        let mut suffix = 2_u32;
        while self.run_id_exists(&candidate)? {
            candidate = format!("{base}_{suffix}");
            suffix = suffix.saturating_add(1);
        }
        Ok(candidate)
    }

    pub(super) fn unique_authority_request_id(&self, experiment_id: &str) -> Result<String> {
        let millis = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis();
        let base = format!("authreq_{SYSTEM}_{millis}_{}", sanitize_slug(experiment_id));
        let mut candidate = base.clone();
        let mut suffix = 2_u32;
        while self.authority_request_id_exists(&candidate)? {
            candidate = format!("{base}_{suffix}");
            suffix = suffix.saturating_add(1);
        }
        Ok(candidate)
    }

    pub(super) fn unique_authority_budget_id(&self, experiment_id: &str) -> Result<String> {
        let millis = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis();
        let base = format!("authbud_{SYSTEM}_{millis}_{}", sanitize_slug(experiment_id));
        let mut candidate = base.clone();
        let mut suffix = 2_u32;
        while self.authority_request_id_exists(&candidate)? {
            candidate = format!("{base}_{suffix}");
            suffix = suffix.saturating_add(1);
        }
        Ok(candidate)
    }

    pub(super) fn unique_research_budget_id(&self, experiment_id: &str) -> Result<String> {
        let millis = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis();
        let base = format!("resbud_{SYSTEM}_{millis}_{}", sanitize_slug(experiment_id));
        let mut candidate = base.clone();
        let mut suffix = 2_u32;
        while self.authority_request_id_exists(&candidate)? {
            candidate = format!("{base}_{suffix}");
            suffix = suffix.saturating_add(1);
        }
        Ok(candidate)
    }

    pub(super) fn unique_dossier_record_id(&self, kind: &str) -> Result<String> {
        let millis = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis();
        let base = format!("dossier_{SYSTEM}_{millis}_{}", sanitize_slug(kind));
        let mut candidate = base.clone();
        let mut suffix = 2_u32;
        while self.dossier_record_id_exists(&candidate)? {
            candidate = format!("{base}_{suffix}");
            suffix = suffix.saturating_add(1);
        }
        Ok(candidate)
    }

    pub(super) fn authority_request_id_exists(&self, request_id: &str) -> Result<bool> {
        let threads_dir = self.root.join("threads");
        if !threads_dir.exists() {
            return Ok(false);
        }
        for entry in fs::read_dir(threads_dir)? {
            let path = entry?.path().join("authority_gate.jsonl");
            if path.exists()
                && fs::read_to_string(&path)
                    .unwrap_or_default()
                    .contains(request_id)
            {
                return Ok(true);
            }
        }
        Ok(false)
    }

    pub(super) fn unique_dir_id(&self, base: String) -> Result<String> {
        let mut candidate = base.clone();
        let mut suffix = 2_u32;
        while self.thread_dir(&candidate).exists() {
            candidate = format!("{base}_{suffix}");
            suffix = suffix.saturating_add(1);
        }
        Ok(candidate)
    }

    pub(super) fn action_id_exists(&self, action_id: &str) -> Result<bool> {
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

    pub(super) fn experiment_id_exists(&self, experiment_id: &str) -> Result<bool> {
        let threads_dir = self.root.join("threads");
        if !threads_dir.exists() {
            return Ok(false);
        }
        for entry in fs::read_dir(threads_dir)? {
            let Ok(entry) = entry else { continue };
            let raw =
                fs::read_to_string(entry.path().join("experiments.jsonl")).unwrap_or_default();
            if raw.lines().any(|line| line.contains(experiment_id)) {
                return Ok(true);
            }
        }
        Ok(false)
    }

    pub(super) fn run_id_exists(&self, run_id: &str) -> Result<bool> {
        let threads_dir = self.root.join("threads");
        if !threads_dir.exists() {
            return Ok(false);
        }
        for entry in fs::read_dir(threads_dir)? {
            let Ok(entry) = entry else { continue };
            let raw =
                fs::read_to_string(entry.path().join("experiment_runs.jsonl")).unwrap_or_default();
            if raw.lines().any(|line| line.contains(run_id)) {
                return Ok(true);
            }
        }
        Ok(false)
    }

    pub(super) fn dossier_record_id_exists(&self, record_id: &str) -> Result<bool> {
        let threads_dir = self.root.join("threads");
        if !threads_dir.exists() {
            return Ok(false);
        }
        for entry in fs::read_dir(threads_dir)? {
            let Ok(entry) = entry else { continue };
            let path = entry.path().join("research_dossier.jsonl");
            let Ok(raw) = fs::read_to_string(path) else {
                continue;
            };
            if raw.contains(record_id) {
                return Ok(true);
            }
        }
        Ok(false)
    }
}
