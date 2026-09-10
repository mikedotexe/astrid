//! Exact catalog candidates for a failed path; never an implicit source open.
use crate::Catalog;
use std::cmp::Reverse;
use std::collections::BTreeSet;
use std::path::{Component, Path};

impl Catalog {
    pub(crate) fn path_candidates(&self, requested: &str, directory: bool) -> Vec<String> {
        let requested = requested.trim_end_matches('/');
        let parts: Vec<_> = requested.split('/').collect();
        let Some(repository) = parts.first() else {
            return Vec::new();
        };
        if parts.len() < 2
            || !self.roots.contains_key(*repository)
            || !Path::new(requested)
                .components()
                .all(|c| matches!(c, Component::Normal(_)))
        {
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
        let mut candidates: Vec<_> = available
            .into_iter()
            .filter(|id| {
                let candidate: Vec<_> = id.split('/').collect();
                id != requested
                    && id.len() <= 500
                    && !id.chars().any(char::is_whitespace)
                    && candidate.first() == parts.first()
                    && candidate.last().map(|p| p.replace('_', "-"))
                        == parts.last().map(|p| p.replace('_', "-"))
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
        candidates
            .into_iter()
            .take(3)
            .map(|id| {
                if directory {
                    format!("SELF_STUDY MAP {id}")
                } else {
                    format!("SELF_STUDY OPEN {id} 1")
                }
            })
            .collect()
    }
}
