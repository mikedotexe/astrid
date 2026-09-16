//! Bounded source hints from question provenance, never a global basename guess.
use crate::Catalog;
use std::fmt::Write as _;

/// Resolve at most two optional choices without enumerating or reading source.
/// A saved question's source stays its anchor during unrelated browsing. Exact
/// paths in the question can still deliberately cross repository boundaries.
pub(crate) fn render_choices(
    catalog: &Catalog,
    question: &str,
    terms: &[&str],
    question_origin: Option<&str>,
    fallback_sources: &[&str],
) -> (String, bool) {
    let mut out = String::new();
    let mut opened = Vec::new();
    let mut unresolved = 0_usize;
    for reference in terms.iter().copied().filter(|term| source_reference(term)) {
        let exact = reference
            .split_once('/')
            .is_some_and(|(repository, _)| catalog.roots.contains_key(repository))
            .then(|| catalog.resolve(reference).ok())
            .flatten();
        let candidate = exact
            .map(|source| (source.id, "Source path written in your question"))
            .or_else(|| {
                contextual_source(
                    catalog,
                    question,
                    terms,
                    reference,
                    question_origin,
                    fallback_sources,
                )
            });
        if let Some((source, basis)) = candidate {
            if !opened.contains(&source) {
                let _ = writeln!(
                    out,
                    "{basis} (catalog path verified; relevance unverified; no source bytes supplied until chosen): SELF_STUDY OPEN {source} 1"
                );
                opened.push(source);
            }
        } else if unresolved < 2 {
            let _ = writeln!(
                out,
                "Source reference `{reference}` has no verified contextual match. A filename alone does not identify a repository or implementation; choose an exact source path through SELF_STUDY MAP. All catalog sources remain available."
            );
            unresolved = unresolved.saturating_add(1);
        }
        if opened.len() == 2 {
            break;
        }
    }
    (out, !opened.is_empty())
}

fn contextual_source(
    catalog: &Catalog,
    question: &str,
    terms: &[&str],
    reference: &str,
    question_origin: Option<&str>,
    fallback_sources: &[&str],
) -> Option<(String, &'static str)> {
    // A missing explicit repository path is never repaired into another source.
    if reference
        .split_once('/')
        .is_some_and(|(repo, _)| catalog.roots.contains_key(repo))
    {
        return None;
    }
    let mentioned_repositories = catalog
        .roots
        .keys()
        .filter(|repo| {
            question
                .split(|c: char| !c.is_ascii_alphanumeric() && c != '-')
                .any(|word| word.eq_ignore_ascii_case(repo))
        })
        .collect::<Vec<_>>();
    let origin = [question_origin.unwrap_or_default()];
    let contexts = if question_origin.is_some() {
        &origin[..]
    } else {
        fallback_sources
    };
    let mut best: Option<(usize, String)> = None;
    for context in contexts.iter().take(8) {
        let Some((repository, _)) = context.split_once('/') else {
            continue;
        };
        if !mentioned_repositories.is_empty()
            && !mentioned_repositories
                .iter()
                .any(|repo| repo.as_str() == repository)
        {
            continue;
        }
        let Some((parent, _)) = context.rsplit_once('/') else {
            continue;
        };
        let Some(source) = catalog.resolve(&format!("{parent}/{reference}")).ok() else {
            continue;
        };
        let score = path_symbol_score(&source.id, terms);
        if best.as_ref().is_none_or(|(previous, _)| score > *previous) {
            best = Some((score, source.id));
        }
    }
    best.map(|(_, source)| {
        (
            source,
            if question_origin.is_some() {
                "Filename candidate beside the source where your question was saved"
            } else {
                "Filename candidate from recent source context (lexical path hints only)"
            },
        )
    })
}

/// A lexical tie-breaker among already known contexts, not a symbol index or a
/// claim that this file defines the named symbol. Keep input order for ties.
fn path_symbol_score(source: &str, terms: &[&str]) -> usize {
    terms
        .iter()
        .filter(|term| {
            term.len() >= 3
                && !source_reference(term)
                && source
                    .split(|c: char| !c.is_ascii_alphanumeric())
                    .any(|part| part.eq_ignore_ascii_case(term))
        })
        .count()
}

pub(crate) fn source_reference(value: &str) -> bool {
    !value
        .bytes()
        .any(|byte| byte.is_ascii_whitespace() || matches!(byte, b'|' | b'"' | b'\'' | b'<' | b'>'))
        && (value.contains('/')
            || [".rs", ".py", ".md", ".toml", ".json", ".txt"]
                .iter()
                .any(|suffix| value.ends_with(suffix)))
}
