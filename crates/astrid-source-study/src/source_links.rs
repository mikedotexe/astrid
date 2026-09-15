//! Bounded same-file syntax candidates, never a resolved call graph.
use crate::source_structure::Outline;
use std::collections::{BTreeMap, BTreeSet};

pub(crate) fn render(
    outline: &Outline,
    source: &str,
    start: usize,
    end: usize,
    max_bytes: usize,
) -> String {
    if !outline.complete || start == end {
        return String::new();
    }
    let mut candidates = Vec::new();
    let mut seen = BTreeSet::new();
    let mut definitions_by_name = BTreeMap::<_, Vec<_>>::new();
    let mut references_by_name = BTreeMap::<_, Vec<_>>::new();
    for definition in &outline.declarations {
        definitions_by_name
            .entry(definition.name.as_str())
            .or_default()
            .push(definition);
    }
    for reference in &outline.references {
        references_by_name
            .entry(reference.name.as_str())
            .or_default()
            .push(reference);
    }
    // Prefer definitions referenced by the supplied page, particularly useful
    // when a test calls an implementation above its own source interval.
    for reference in &outline.references {
        if candidates.len() >= 2 {
            break;
        }
        if reference.start_byte < start || reference.end_byte > end {
            continue;
        }
        for definition in definitions_by_name
            .get(reference.name.as_str())
            .into_iter()
            .flatten()
        {
            if definition.test_context.is_none()
                && !(start <= definition.name_start_byte && definition.name_end_byte <= end)
                && seen.insert(definition.start_line)
            {
                candidates.push((
                    definition.start_line,
                    "definition candidate",
                    definition.qualified_name.as_str(),
                ));
                if candidates.len() >= 2 {
                    break;
                }
            }
        }
    }
    // Reading a definition may leave its calling conditions unanswered. Offer a
    // syntactic reference in another declaration without claiming name binding.
    for definition in &outline.declarations {
        if candidates.len() >= 2 {
            break;
        }
        if definition.name_start_byte < start || definition.name_end_byte > end {
            continue;
        }
        for reference in references_by_name
            .get(definition.name.as_str())
            .into_iter()
            .flatten()
        {
            if (start <= reference.start_byte && reference.end_byte <= end)
                || (definition.start_byte <= reference.start_byte
                    && reference.start_byte < definition.end_byte)
            {
                continue;
            }
            if let Some(caller) = outline.enclosing(reference.start_byte).last()
                && caller.test_context.is_none()
                && seen.insert(reference.line)
            {
                candidates.push((
                    reference.line,
                    "reference candidate in",
                    caller.qualified_name.as_str(),
                ));
                if candidates.len() >= 2 {
                    break;
                }
            }
        }
    }
    let heading = "\nRELATED SOURCE LOCATIONS — same revision; unsupplied syntax candidates, not resolved calls or runtime proof. Tests show tested examples; inspect implementation for broader claims.\n";
    let mut output = String::new();
    for (line, role, name) in candidates.into_iter().take(2) {
        let label: String = name
            .chars()
            .take(72)
            .map(|c| if c.is_control() { ' ' } else { c })
            .collect();
        let row = format!("{role} {label}: SELF_STUDY OPEN {source} {line}\n");
        let prefix = if output.is_empty() { heading } else { "" };
        if output
            .len()
            .saturating_add(prefix.len())
            .saturating_add(row.len())
            <= max_bytes
        {
            output.push_str(prefix);
            output.push_str(&row);
        }
    }
    output
}
