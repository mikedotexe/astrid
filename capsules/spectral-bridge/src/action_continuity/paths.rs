//! Filesystem path builders for [`ActionContinuityStore`].
//!
//! Extracted verbatim from the monolithic `action_continuity.rs` as part of the
//! decomposition roadmap (item 2, tranche A1). Behavior-identical: every method
//! derives its path purely from `self.root` (plus `bridge_paths()` for the
//! shared-collaborations root). See `docs/steward-notes/ARCHITECTURE_DECOMPOSITION_PLAN_2026-06-13.md`.

use super::*;

impl ActionContinuityStore {
    pub(super) fn experiments_path(&self, thread_id: &str) -> PathBuf {
        self.thread_dir(thread_id).join("experiments.jsonl")
    }

    pub(super) fn experiment_runs_path(&self, thread_id: &str) -> PathBuf {
        self.thread_dir(thread_id).join("experiment_runs.jsonl")
    }

    pub(super) fn authority_gate_path(&self, thread_id: &str) -> PathBuf {
        self.thread_dir(thread_id).join("authority_gate.jsonl")
    }

    pub(super) fn dossier_path(&self, thread_id: &str) -> PathBuf {
        self.thread_dir(thread_id).join("research_dossier.jsonl")
    }

    pub(super) fn being_memory_path(&self, thread_id: &str) -> PathBuf {
        self.thread_dir(thread_id).join("being_memory.jsonl")
    }

    pub(super) fn continuity_sessions_path(&self, thread_id: &str) -> PathBuf {
        self.thread_dir(thread_id).join("continuity_sessions.jsonl")
    }

    pub(super) fn shared_investigation_root(&self) -> PathBuf {
        let production_root = bridge_paths().bridge_workspace().join("action_threads");
        if self.root == production_root {
            bridge_paths()
                .shared_collaborations_dir()
                .join("shared_investigations")
        } else {
            self.root.join("shared_investigations")
        }
    }

    pub(super) fn shared_investigation_dir(&self, investigation_id: &str) -> PathBuf {
        self.shared_investigation_root().join(investigation_id)
    }

    pub(super) fn index_path(&self) -> PathBuf {
        self.root.join("index.json")
    }

    pub(super) fn proposals_path(&self) -> PathBuf {
        self.root.join("proposals.jsonl")
    }

    pub(super) fn thread_dir(&self, thread_id: &str) -> PathBuf {
        self.root.join("threads").join(thread_id)
    }
}
