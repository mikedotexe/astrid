use std::path::{Path, PathBuf};
use std::sync::OnceLock;

#[derive(Debug, Clone, Default)]
pub struct BridgePathOverrides {
    pub bridge_root: Option<PathBuf>,
    pub bridge_workspace: Option<PathBuf>,
    pub astrid_root: Option<PathBuf>,
    pub autoresearch_root: Option<PathBuf>,
    pub minime_root: Option<PathBuf>,
    pub minime_workspace: Option<PathBuf>,
    pub perception_path: Option<PathBuf>,
    pub introspector_script: Option<PathBuf>,
    pub reflective_sidecar_script: Option<PathBuf>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BridgePaths {
    bridge_root: PathBuf,
    bridge_workspace: PathBuf,
    astrid_root: PathBuf,
    autoresearch_root: PathBuf,
    minime_root: PathBuf,
    minime_workspace: PathBuf,
    perception_path: PathBuf,
    introspector_script: PathBuf,
    reflective_sidecar_script: PathBuf,
}

static BRIDGE_PATHS: OnceLock<BridgePaths> = OnceLock::new();

pub fn configure_bridge_paths(overrides: BridgePathOverrides) -> &'static BridgePaths {
    BRIDGE_PATHS.get_or_init(|| BridgePaths::resolve(overrides))
}

#[must_use]
pub fn bridge_paths() -> &'static BridgePaths {
    BRIDGE_PATHS.get_or_init(BridgePaths::default)
}

impl Default for BridgePaths {
    fn default() -> Self {
        Self::resolve(BridgePathOverrides::default())
    }
}

impl BridgePaths {
    #[must_use]
    pub fn resolve(overrides: BridgePathOverrides) -> Self {
        let bridge_workspace_hint = overrides
            .bridge_workspace
            .clone()
            .or_else(|| env_path("ASTRID_BRIDGE_WORKSPACE"));
        let bridge_root = overrides
            .bridge_root
            .clone()
            .or_else(|| env_path("ASTRID_BRIDGE_ROOT"))
            .or_else(|| {
                bridge_workspace_hint
                    .as_ref()
                    .and_then(|path| parent_dir(path))
            })
            .unwrap_or_else(default_bridge_root);

        let bridge_workspace =
            bridge_workspace_hint.unwrap_or_else(|| bridge_root.join("workspace"));

        let astrid_root = overrides
            .astrid_root
            .clone()
            .or_else(|| env_path("ASTRID_ROOT"))
            .unwrap_or_else(|| default_astrid_root(&bridge_root));

        let autoresearch_root = overrides
            .autoresearch_root
            .clone()
            .or_else(|| env_path("ASTRID_AUTORESEARCH_ROOT"))
            .unwrap_or_else(|| default_autoresearch_root(&astrid_root));

        let minime_workspace_hint = overrides
            .minime_workspace
            .clone()
            .or_else(|| env_path("MINIME_WORKSPACE"));
        let minime_root = overrides
            .minime_root
            .clone()
            .or_else(|| env_path("MINIME_ROOT"))
            .or_else(|| {
                minime_workspace_hint
                    .as_ref()
                    .and_then(|path| parent_dir(path))
            })
            .unwrap_or_else(|| default_minime_root(&astrid_root));
        let minime_workspace =
            minime_workspace_hint.unwrap_or_else(|| minime_root.join("workspace"));

        let perception_path = overrides
            .perception_path
            .clone()
            .or_else(|| env_path("ASTRID_PERCEPTION_PATH"))
            .unwrap_or_else(|| astrid_root.join("capsules/perception/workspace/perceptions"));
        let introspector_script = overrides
            .introspector_script
            .clone()
            .or_else(|| env_path("ASTRID_INTROSPECTOR_SCRIPT"))
            .unwrap_or_else(|| astrid_root.join("capsules/introspector/introspector.py"));
        let reflective_sidecar_script = overrides
            .reflective_sidecar_script
            .clone()
            .or_else(|| env_path("ASTRID_REFLECTIVE_SIDECAR"))
            .unwrap_or_else(|| {
                astrid_root
                    .parent()
                    .map(|root| root.join("mlx/benchmarks/python/chat_mlx_local.py"))
                    .unwrap_or_else(|| PathBuf::from("mlx/benchmarks/python/chat_mlx_local.py"))
            });

        Self {
            bridge_root,
            bridge_workspace,
            astrid_root,
            autoresearch_root,
            minime_root,
            minime_workspace,
            perception_path,
            introspector_script,
            reflective_sidecar_script,
        }
    }

    #[must_use]
    pub fn bridge_root(&self) -> &Path {
        &self.bridge_root
    }

    #[must_use]
    pub fn bridge_workspace(&self) -> &Path {
        &self.bridge_workspace
    }

    #[must_use]
    pub fn astrid_root(&self) -> &Path {
        &self.astrid_root
    }

    #[must_use]
    pub fn autoresearch_root(&self) -> &Path {
        &self.autoresearch_root
    }

    #[must_use]
    pub fn minime_root(&self) -> &Path {
        &self.minime_root
    }

    #[must_use]
    pub fn minime_workspace(&self) -> &Path {
        &self.minime_workspace
    }

    #[must_use]
    pub fn perception_path(&self) -> &Path {
        &self.perception_path
    }

    #[must_use]
    pub fn introspector_script(&self) -> &Path {
        &self.introspector_script
    }

    #[must_use]
    pub fn reflective_sidecar_script(&self) -> &Path {
        &self.reflective_sidecar_script
    }

    #[must_use]
    pub fn bridge_src_dir(&self) -> PathBuf {
        self.bridge_root.join("src")
    }

    #[must_use]
    pub fn context_overflow_dir(&self) -> PathBuf {
        self.bridge_workspace.join("context_overflow")
    }

    #[must_use]
    pub fn astrid_journal_dir(&self) -> PathBuf {
        self.bridge_workspace.join("journal")
    }

    #[must_use]
    pub fn astrid_inbox_dir(&self) -> PathBuf {
        self.bridge_workspace.join("inbox")
    }

    #[must_use]
    pub fn agency_requests_dir(&self) -> PathBuf {
        self.bridge_workspace.join("agency_requests")
    }

    #[must_use]
    pub fn claude_tasks_dir(&self) -> PathBuf {
        self.bridge_workspace.join("claude_tasks")
    }

    #[must_use]
    pub fn astrid_outbox_dir(&self) -> PathBuf {
        self.bridge_workspace.join("outbox")
    }

    #[must_use]
    pub fn state_path(&self) -> PathBuf {
        self.bridge_workspace.join("state.json")
    }

    #[must_use]
    pub fn btsp_episode_bank_path(&self) -> PathBuf {
        self.bridge_workspace.join("btsp_episode_bank.json")
    }

    #[must_use]
    pub fn btsp_signal_catalog_path(&self) -> PathBuf {
        self.bridge_workspace.join("btsp_signal_catalog.json")
    }

    #[must_use]
    pub fn btsp_signal_events_path(&self) -> PathBuf {
        self.bridge_workspace.join("btsp_signal_events.jsonl")
    }

    #[must_use]
    pub fn btsp_signal_status_path(&self) -> PathBuf {
        self.bridge_workspace.join("btsp_signal_status.json")
    }

    #[must_use]
    pub fn sovereignty_proposals_path(&self) -> PathBuf {
        self.bridge_workspace.join("sovereignty_proposals.json")
    }

    #[must_use]
    pub fn btsp_minime_choice_cursor_path(&self) -> PathBuf {
        self.bridge_workspace.join("btsp_minime_choice_cursor.json")
    }

    #[must_use]
    pub fn experiments_dir(&self) -> PathBuf {
        self.bridge_workspace.join("experiments")
    }

    #[must_use]
    pub fn introspections_dir(&self) -> PathBuf {
        self.bridge_workspace.join("introspections")
    }

    #[must_use]
    pub fn creations_dir(&self) -> PathBuf {
        self.bridge_workspace.join("creations")
    }

    #[must_use]
    pub fn research_dir(&self) -> PathBuf {
        self.bridge_workspace.join("research")
    }

    #[must_use]
    pub fn inbox_audio_dir(&self) -> PathBuf {
        self.bridge_workspace.join("inbox_audio")
    }

    #[must_use]
    pub fn perception_paused_flag(&self) -> PathBuf {
        self.bridge_workspace.join("perception_paused.flag")
    }

    #[must_use]
    pub fn perception_visual_paused_flag(&self) -> PathBuf {
        self.bridge_workspace.join("perception_visual_paused.flag")
    }

    #[must_use]
    pub fn perception_audio_paused_flag(&self) -> PathBuf {
        self.bridge_workspace.join("perception_audio_paused.flag")
    }

    #[must_use]
    pub fn astrid_contact_state_path(&self) -> PathBuf {
        self.bridge_workspace.join("contact_state.json")
    }

    #[must_use]
    pub fn audio_creations_dir(&self) -> PathBuf {
        self.bridge_workspace.join("audio_creations")
    }

    /// Mike's curated research root (sibling of astrid_root).
    #[must_use]
    pub fn mike_research_root(&self) -> PathBuf {
        self.astrid_root
            .parent()
            .map(|p| p.join("research"))
            .unwrap_or_else(|| PathBuf::from("/Users/v/other/research"))
    }

    /// v5 Coordination Protocol V1: shared collaboration root, sibling of
    /// both astrid_root and minime_root. Neither workspace owns it; both
    /// read/write. Houses one subdirectory per active joint thread.
    #[must_use]
    pub fn shared_collaborations_dir(&self) -> PathBuf {
        self.astrid_root
            .parent()
            .map(|p| p.join("shared").join("collaborations"))
            .unwrap_or_else(|| PathBuf::from("/Users/v/other/shared/collaborations"))
    }

    #[must_use]
    pub fn minime_inbox_dir(&self) -> PathBuf {
        self.minime_workspace.join("inbox")
    }

    #[must_use]
    pub fn minime_outbox_dir(&self) -> PathBuf {
        self.minime_workspace.join("outbox")
    }

    #[must_use]
    pub fn minime_contact_state_path(&self) -> PathBuf {
        self.minime_workspace.join("contact_state.json")
    }

    #[must_use]
    pub fn minime_memory_bank_path(&self) -> PathBuf {
        self.minime_workspace.join("spectral_memory_bank.json")
    }

    #[must_use]
    pub fn minime_memory_requests_dir(&self) -> PathBuf {
        self.minime_workspace.join("memory_requests")
    }

    /// v3.6.1: directory where Astrid receives parameter-tuning requests
    /// from minime (`from_minime_*.json`) and writes her own outbound
    /// requests (`from_astrid_*.json`).
    #[must_use]
    pub fn parameter_requests_dir(&self) -> PathBuf {
        self.bridge_workspace.join("parameter_requests")
    }

    /// v3.6.1: minime's parameter-requests inbox, where Astrid drops
    /// `from_astrid_*.json` requests for minime to review.
    #[must_use]
    pub fn minime_parameter_requests_dir(&self) -> PathBuf {
        self.minime_workspace.join("parameter_requests")
    }
}

/// v3.6.1: count `from_minime_*.json` files awaiting Astrid's review in
/// her bridge workspace. Returns 0 if the directory doesn't exist or is
/// unreadable. Cheap directory scan; cache at the call site if invoked
/// per exchange.
#[must_use]
pub fn count_pending_minime_requests() -> u32 {
    let dir = bridge_paths().parameter_requests_dir();
    let Ok(entries) = std::fs::read_dir(&dir) else {
        return 0;
    };
    let count = entries
        .flatten()
        .filter(|entry| {
            let name = entry.file_name();
            let lossy = name.to_string_lossy();
            lossy.starts_with("from_minime_") && lossy.ends_with(".json")
        })
        .count();
    u32::try_from(count).unwrap_or(u32::MAX)
}

/// v3.6.4: peek the lexicographically-last `from_minime_*.json` request and
/// return `(request_id, param, value_display)` for the curriculum to surface
/// in a DecideRequest nudge. Returns `None` if no pending requests exist or
/// the file can't be parsed. Cheap O(N) directory scan + one read.
#[must_use]
pub fn peek_latest_pending_minime_request() -> Option<(String, String, String)> {
    let dir = bridge_paths().parameter_requests_dir();
    let entries = std::fs::read_dir(&dir).ok()?;
    let mut paths: Vec<std::path::PathBuf> = entries
        .flatten()
        .map(|e| e.path())
        .filter(|p| {
            p.is_file()
                && p.file_name()
                    .and_then(|n| n.to_str())
                    .map(|n| n.starts_with("from_minime_") && n.ends_with(".json"))
                    .unwrap_or(false)
        })
        .collect();
    if paths.is_empty() {
        return None;
    }
    paths.sort();
    let chosen = paths.last()?;
    let text = std::fs::read_to_string(chosen).ok()?;
    let v: serde_json::Value = serde_json::from_str(&text).ok()?;
    let request_id = v.get("request_id").and_then(|x| x.as_str())?.to_string();
    let param = v
        .get("param")
        .and_then(|x| x.as_str())
        .unwrap_or("?")
        .to_string();
    let value_display = match v.get("proposed_value") {
        Some(serde_json::Value::String(s)) => s.clone(),
        Some(other) => other.to_string(),
        None => "?".to_string(),
    };
    Some((request_id, param, value_display))
}

fn env_path(name: &str) -> Option<PathBuf> {
    std::env::var_os(name).and_then(|value| {
        if value.is_empty() {
            None
        } else {
            Some(PathBuf::from(value))
        }
    })
}

fn default_bridge_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

fn default_astrid_root(bridge_root: &Path) -> PathBuf {
    bridge_root
        .parent()
        .and_then(Path::parent)
        .map(Path::to_path_buf)
        .unwrap_or_else(|| bridge_root.to_path_buf())
}

fn default_minime_root(astrid_root: &Path) -> PathBuf {
    astrid_root
        .parent()
        .map(|root| root.join("minime"))
        .unwrap_or_else(|| PathBuf::from("minime"))
}

fn default_autoresearch_root(astrid_root: &Path) -> PathBuf {
    astrid_root
        .parent()
        .map(|root| root.join("autoresearch"))
        .unwrap_or_else(|| PathBuf::from("autoresearch"))
}

fn parent_dir(path: &Path) -> Option<PathBuf> {
    path.parent().map(Path::to_path_buf)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn resolve_uses_sibling_defaults_from_bridge_root() {
        let paths = BridgePaths::resolve(BridgePathOverrides {
            bridge_root: Some(PathBuf::from("/tmp/astrid/capsules/spectral-bridge")),
            ..BridgePathOverrides::default()
        });

        assert_eq!(
            paths.bridge_workspace(),
            Path::new("/tmp/astrid/capsules/spectral-bridge/workspace")
        );
        assert_eq!(paths.astrid_root(), Path::new("/tmp/astrid"));
        assert_eq!(paths.autoresearch_root(), Path::new("/tmp/autoresearch"));
        assert_eq!(paths.minime_root(), Path::new("/tmp/minime"));
        assert_eq!(paths.minime_workspace(), Path::new("/tmp/minime/workspace"));
    }

    #[test]
    fn resolve_prefers_explicit_workspace_and_script_overrides() {
        let paths = BridgePaths::resolve(BridgePathOverrides {
            bridge_root: Some(PathBuf::from("/tmp/astrid/capsules/spectral-bridge")),
            bridge_workspace: Some(PathBuf::from("/runtime/bridge-workspace")),
            autoresearch_root: Some(PathBuf::from("/runtime/autoresearch")),
            minime_workspace: Some(PathBuf::from("/runtime/minime-workspace")),
            perception_path: Some(PathBuf::from("/runtime/perception")),
            introspector_script: Some(PathBuf::from("/runtime/introspector.py")),
            reflective_sidecar_script: Some(PathBuf::from("/runtime/reflective.py")),
            ..BridgePathOverrides::default()
        });

        assert_eq!(
            paths.bridge_workspace(),
            Path::new("/runtime/bridge-workspace")
        );
        assert_eq!(
            paths.autoresearch_root(),
            Path::new("/runtime/autoresearch")
        );
        assert_eq!(
            paths.minime_workspace(),
            Path::new("/runtime/minime-workspace")
        );
        assert_eq!(paths.perception_path(), Path::new("/runtime/perception"));
        assert_eq!(
            paths.introspector_script(),
            Path::new("/runtime/introspector.py")
        );
        assert_eq!(
            paths.reflective_sidecar_script(),
            Path::new("/runtime/reflective.py")
        );
    }
}
