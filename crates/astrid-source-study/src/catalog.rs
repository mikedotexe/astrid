use std::collections::BTreeMap;
use std::fs;
use std::path::{Component, Path, PathBuf};

use anyhow::{Context as _, Result, bail};
use globset::{GlobBuilder, GlobSet, GlobSetBuilder};
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Deserialize)]
pub struct Repository {
    pub id: String,
    pub directory: String,
    pub include: Vec<String>,
}

#[derive(Clone, Debug, Deserialize)]
pub(crate) struct StudyComponent {
    pub id: String,
    pub title: String,
    pub sources: Vec<String>,
}

#[derive(Deserialize)]
struct Manifest {
    version: u32,
    repositories: Vec<Repository>,
    components: Vec<StudyComponent>,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct Source {
    pub id: String,
    pub path: PathBuf,
}

#[derive(Clone)]
pub struct Catalog {
    pub version: u32,
    pub(crate) roots: BTreeMap<String, PathBuf>,
    rules: BTreeMap<String, GlobSet>,
    pub(crate) components: Vec<StudyComponent>,
}

impl Catalog {
    /// Installation wiring is supplied by the host adapter, never source prose.
    /// # Errors
    /// Returns an error for unknown repository IDs or invalid catalog rules.
    pub fn new(roots: BTreeMap<String, PathBuf>) -> Result<Self> {
        let manifest: Manifest = toml::from_str(include_str!("../catalog.toml"))?;
        let mut rules = BTreeMap::new();
        for repository in &manifest.repositories {
            let mut builder = GlobSetBuilder::new();
            for pattern in &repository.include {
                builder.add(GlobBuilder::new(pattern).literal_separator(true).build()?);
            }
            rules.insert(repository.id.clone(), builder.build()?);
        }
        if roots.keys().any(|id| !rules.contains_key(id)) {
            bail!("unknown repository ID in source-study installation");
        }
        Ok(Self {
            version: manifest.version,
            roots,
            rules,
            components: manifest.components,
        })
    }

    /// # Errors
    /// Returns an error if roots cannot identify the installation.
    pub fn installation(astrid: &Path, minime: &Path) -> Result<Self> {
        let parent = astrid.parent().context("Astrid root has no parent")?;
        let manifest: Manifest = toml::from_str(include_str!("../catalog.toml"))?;
        let mut roots = manifest
            .repositories
            .into_iter()
            .map(|repo| (repo.id, parent.join(repo.directory)))
            .collect::<BTreeMap<_, _>>();
        roots.insert("astrid".into(), astrid.to_path_buf());
        roots.insert("minime".into(), minime.to_path_buf());
        Self::new(roots)
    }

    /// A canonical ID always retains repository and relative path. Absolute
    /// compatibility paths resolve exactly; there is no basename fallback.
    /// # Errors
    /// Returns an error for missing, ambiguous, private, or out-of-scope paths.
    pub fn resolve(&self, requested: &str) -> Result<Source> {
        let target = alias(requested.trim());
        if Path::new(target).is_absolute() {
            let canonical = fs::canonicalize(target).context("source path unavailable")?;
            for (id, root) in &self.roots {
                if let Ok(root) = fs::canonicalize(root)
                    && let Ok(relative) = canonical.strip_prefix(&root)
                {
                    return self.resolve_id(id, relative);
                }
            }
            bail!("source is outside the shared source catalog");
        }
        if let Some((id, relative)) = target.split_once('/')
            && self.roots.contains_key(id)
        {
            return self.resolve_id(id, Path::new(relative));
        }
        let candidates = [
            ("astrid", PathBuf::from(target)),
            ("astrid", Path::new("capsules/spectral-bridge").join(target)),
            ("minime", PathBuf::from(target)),
        ]
        .into_iter()
        .filter_map(|(repo, path)| self.resolve_id(repo, &path).ok())
        .collect::<Vec<_>>();
        match candidates.as_slice() {
            [source] => Ok(source.clone()),
            [] => bail!("source not found; use SELF_STUDY FIND <text> or an exact repository/path"),
            _ => bail!("ambiguous relative path; choose an exact repository/path"),
        }
    }

    fn resolve_id(&self, repository: &str, relative: &Path) -> Result<Source> {
        if !relative
            .components()
            .all(|c| matches!(c, Component::Normal(_)))
        {
            bail!("source IDs require a plain repository-relative path");
        }
        let rule = self.rules.get(repository).context("unknown repository")?;
        let configured_root = self
            .roots
            .get(repository)
            .context("repository is not configured")?;
        if !source_path(relative) || !rule.is_match(relative) {
            bail!(
                "path is not in the shared source catalog (source only; private artifacts use their own reader)"
            );
        }
        let root = fs::canonicalize(configured_root).context("repository unavailable locally")?;
        let path = fs::canonicalize(root.join(relative)).context("source unavailable locally")?;
        let actual_relative = path
            .strip_prefix(&root)
            .context("source symlink leaves its repository")?;
        if !source_path(actual_relative) || !rule.is_match(actual_relative) {
            bail!("source symlink leaves the approved source areas");
        }
        if !path.is_file() {
            bail!("source target is not a file");
        }
        Ok(Source {
            id: format!(
                "{repository}/{}",
                actual_relative
                    .to_str()
                    .context("source path is not UTF-8")?
            ),
            path,
        })
    }

    /// List all available catalog entries.
    /// # Errors
    /// Returns an error if an installed repository cannot be enumerated.
    pub fn sources(&self) -> Result<Vec<Source>> {
        let mut found = BTreeMap::new();
        for (id, root) in &self.roots {
            if !root.exists() {
                continue;
            }
            let mut directories = vec![root.clone()];
            while let Some(dir) = directories.pop() {
                for entry in
                    fs::read_dir(&dir).with_context(|| format!("list {}", dir.display()))?
                {
                    let entry = entry?;
                    let kind = entry.file_type()?;
                    let path = entry.path();
                    let relative = path.strip_prefix(root)?;
                    if blocked_path(relative) {
                        continue;
                    }
                    // Symlink directories are not traversed; explicit opens still
                    // resolve and validate their exact canonical destination.
                    if kind.is_dir() {
                        directories.push(path);
                    } else if kind.is_file()
                        && let Ok(source) = self.resolve_id(id, relative)
                    {
                        found.insert(source.id.clone(), source);
                    }
                }
            }
        }
        Ok(found.into_values().collect())
    }
}

fn blocked_path(path: &Path) -> bool {
    path.components().any(|part| {
        let name = part.as_os_str().to_string_lossy().to_ascii_lowercase();
        (name.starts_with('.') && !matches!(name.as_str(), ".cargo" | ".github"))
            || matches!(
                name.as_str(),
                "workspace"
                    | "workspaces"
                    | "target"
                    | "node_modules"
                    | "__pycache__"
                    | "venv"
                    | "dist"
                    | "backups"
                    | "releases"
            )
    })
}

fn source_path(path: &Path) -> bool {
    if blocked_path(path) {
        return false;
    }
    let name = path
        .file_name()
        .unwrap_or_default()
        .to_string_lossy()
        .to_ascii_lowercase();
    let stem = path
        .file_stem()
        .unwrap_or_default()
        .to_string_lossy()
        .to_ascii_lowercase();
    let configuration = path.extension().is_some_and(|ext| {
        ["json", "toml", "yaml", "yml", "ini", "txt"]
            .iter()
            .any(|kind| ext.eq_ignore_ascii_case(kind))
    });
    if configuration
        && matches!(
            stem.as_str(),
            "credentials" | "secrets" | "tokens" | "private_key"
        )
    {
        return false;
    }
    if path
        .extension()
        .is_some_and(|ext| ext.eq_ignore_ascii_case("pem") || ext.eq_ignore_ascii_case("key"))
        || matches!(
            name.as_str(),
            "tokens.json" | "secrets.json" | "credentials.json"
        )
    {
        return false;
    }
    if matches!(
        name.as_str(),
        "dockerfile" | "makefile" | "justfile" | "license"
    ) {
        return true;
    }
    matches!(
        path.extension()
            .and_then(|s| s.to_str())
            .unwrap_or("")
            .to_ascii_lowercase()
            .as_str(),
        "rs" | "py"
            | "md"
            | "toml"
            | "lock"
            | "yaml"
            | "yml"
            | "json"
            | "jsonl"
            | "txt"
            | "csv"
            | "sh"
            | "bash"
            | "zsh"
            | "plist"
            | "service"
            | "timer"
            | "socket"
            | "in"
            | "wit"
            | "metal"
            | "wgsl"
            | "glsl"
            | "js"
            | "ts"
            | "tsx"
            | "jsx"
            | "swift"
            | "c"
            | "h"
            | "cpp"
            | "hpp"
            | "html"
            | "css"
    )
}

fn alias(target: &str) -> &str {
    match target {
        "astrid:codec" => "astrid/capsules/spectral-bridge/src/codec/projection.rs",
        "astrid:autonomous" => {
            "astrid/capsules/spectral-bridge/src/autonomous/runtime/orchestration.rs"
        },
        "astrid:ws" => "astrid/capsules/spectral-bridge/src/ws/telemetry_port.rs",
        "astrid:types" => "astrid/capsules/spectral-bridge/src/types/schema/telemetry.rs",
        "astrid:llm" => "astrid/capsules/spectral-bridge/src/llm/provider/dialogue_runtime.rs",
        "minime:regulator" | "regulator" | "regulator.rs" => "minime/minime/src/regulator/core.rs",
        "minime:sensory_bus" | "sensory_bus.rs" => "minime/minime/src/sensory_bus.rs",
        "minime:esn" | "esn.rs" => "minime/minime/src/esn.rs",
        "minime:main(excerpt)" | "main.rs" => "minime/minime/src/runtime.rs",
        "minime:autonomous_agent" | "autonomous_agent.py" => "minime/minime_autonomy/runtime.py",
        "proposal:phase_transitions" => {
            "astrid/docs/steward-notes/AI_BEINGS_PHASE_TRANSITION_ARCHITECTURE.md"
        },
        "proposal:bidirectional_contact" => {
            "astrid/docs/steward-notes/AI_BEINGS_BIDIRECTIONAL_CONTACT_AND_CORRESPONDENCE_ARCHITECTURE.md"
        },
        "proposal:distance_contact_control" => {
            "astrid/docs/steward-notes/AI_BEINGS_DISTANCE_CONTACT_CONTAINMENT_CONTROL_AND_PARTICIPATION_AUDIT.md"
        },
        "proposal:12d_glimpse" => {
            "astrid/docs/steward-notes/AI_BEINGS_MULTI_SCALE_REPRESENTATION_AND_12D_GLIMPSE_AUDIT.md"
        },
        _ => target,
    }
}
