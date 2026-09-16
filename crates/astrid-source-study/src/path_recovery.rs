//! Exact catalog candidates for a failed path; never an implicit source open.
use crate::Catalog;
use std::cmp::Reverse;
use std::collections::BTreeSet;
use std::path::Path;

impl Catalog {
    pub(crate) fn path_candidates(&self, requested: &str, directory: bool) -> Vec<String> {
        self.catalog_candidates(requested, directory)
            .into_iter()
            .filter(|id| id != requested.trim_end_matches('/'))
            .map(|id| {
                if directory {
                    format!("SELF_STUDY MAP {id}")
                } else {
                    format!("SELF_STUDY OPEN {id} 1")
                }
            })
            .collect()
    }

    fn catalog_candidates(&self, requested: &str, directory: bool) -> Vec<String> {
        let requested = requested.strip_suffix('/').unwrap_or(requested);
        if !safe_reference(requested) {
            return Vec::new();
        }
        let parts: Vec<_> = requested.split('/').collect();
        let repository = parts.first().filter(|id| self.roots.contains_key(**id));
        // Bare names can recover across repositories. A repository-qualified
        // request must never silently broaden to a different installation root.
        if parts.len() > 1 && repository.is_none() {
            return Vec::new();
        }
        let Ok(sources) = self.sources() else {
            return Vec::new();
        };
        let mut available = BTreeSet::new();
        for source in sources {
            if directory {
                let mut parent = source.id.as_str();
                while let Some((next, _)) = parent.rsplit_once('/') {
                    if !next.contains('/') {
                        break;
                    }
                    available.insert(next.to_owned());
                    parent = next;
                }
            } else {
                available.insert(source.id);
            }
        }
        let normalized = requested.replace('_', "-");
        let leaf = parts.last().copied().unwrap_or("");
        let mut candidates: Vec<_> = available
            .into_iter()
            .filter(|id| {
                let candidate: Vec<_> = id.split('/').collect();
                let candidate_leaf = candidate.last().copied().unwrap_or("");
                let same_name = candidate_leaf.replace('_', "-") == leaf.replace('_', "-");
                let same_stem = !directory
                    && !leaf.contains('.')
                    && Path::new(candidate_leaf)
                        .file_stem()
                        .and_then(|stem| stem.to_str())
                        .is_some_and(|stem| stem.replace('_', "-") == leaf.replace('_', "-"));
                safe_reference(id)
                    && repository.is_none_or(|id| candidate.first() == Some(id))
                    && (same_name || same_stem)
            })
            .collect();
        candidates.sort_by_key(|id| {
            let candidate: Vec<_> = id.split('/').collect();
            let suffix = candidate
                .iter()
                .rev()
                .zip(parts.iter().rev())
                .take_while(|(a, b)| a == b)
                .count();
            let prefix = candidate
                .iter()
                .zip(parts.iter())
                .take_while(|(a, b)| a == b)
                .count();
            (
                Reverse(id.replace('_', "-") == normalized),
                Reverse(prefix),
                Reverse(suffix),
                candidate.len().abs_diff(parts.len()),
                id.clone(),
            )
        });
        candidates.into_iter().take(3).collect()
    }
}

fn safe_reference(requested: &str) -> bool {
    !requested.is_empty()
        && requested.len() <= 500
        && requested
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || b"_./-".contains(&byte))
        && requested.split('/').all(|part| {
            !part.is_empty()
                && !matches!(part, "." | "..")
                && (!part.starts_with('.') || matches!(part, ".cargo" | ".github"))
                && !matches!(
                    part.to_ascii_lowercase().as_str(),
                    "workspace"
                        | "workspaces"
                        | "journal"
                        | "journals"
                        | "generations"
                        | "drafts"
                        | "private"
                        | "target"
                        | "node_modules"
                        | "__pycache__"
                        | "venv"
                        | "dist"
                        | "backups"
                        | "releases"
                        | "secrets.json"
                        | "tokens.json"
                        | "credentials.json"
                )
        })
}
