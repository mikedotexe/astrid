//! Explicit inspection of one inquiry's native cursor, never a selection or repair.
use super::ReadingCursor;
use crate::{Catalog, Page, digest};
use serde_json::{Value, json};
use std::{collections::BTreeMap, fs, io::Read as _};

impl ReadingCursor {
    pub(super) fn return_facts(&self, id: &str, catalog: &Catalog) -> Value {
        let saved = self.current.as_ref().and_then(|s| self.bookmarks.get(s));
        let pending = self.pending.iter().chain(
            self.pending_session
                .iter()
                .flat_map(|output| &output.session_pages),
        );
        let mut sources = BTreeMap::new();
        for page in saved.into_iter().chain(pending.clone()) {
            sources
                .entry(page.source.clone())
                .or_insert_with(|| current_revision(catalog, &page.source));
        }
        let matches = |page: &Page| {
            sources.get(&page.source).and_then(Option::as_ref) == Some(&page.revision.sha256)
        };
        let source_check = saved.into_iter().chain(pending.clone()).all(matches);
        let can_continue =
            source_check && (pending.clone().next().is_some() || saved.is_some_and(|p| !p.eof));
        let pending_facts = pending
            .map(|page| {
                let input_id = if self.pending.as_ref().is_some_and(|p| p.id == page.id) {
                    Some(page.id.as_str())
                } else {
                    self.pending_session
                        .as_ref()
                        .and_then(|o| o.navigation_id.as_deref())
                };
                let mut facts = page_facts(page, &sources);
                facts["input_id"] = json!(input_id);
                facts
            })
            .collect::<Vec<_>>();
        let reselect = sources
            .iter()
            .filter_map(|(source, current)| {
                current
                    .as_ref()
                    .map(|_| format!("SELF_STUDY OPEN {source} 1"))
            })
            .collect::<Vec<_>>();
        json!({
            "scope": "This inquiry's own retained position only. Source checks describe this inspection, not execution eligibility or restored understanding. No source bytes are supplied by this view. Other inquiries' positions are never borrowed.",
            "question_id": id,
            "saved_position": saved.map(|page| {
                let mut facts = page_facts(page, &sources);
                facts["next_byte_position"] = json!(page.end);
                facts["eof"] = json!(page.eof);
                facts
            }),
            "pending_source_delivery": pending_facts,
            "pending_delivery_scope": "Prepared source input retained, not proof of model delivery. Inspection does not regenerate or submit it.",
            "saved_sources_match_now": source_check,
            "select_command": format!("SELF_STUDY QUESTION {id}"),
            "selection_effect": "Selects this inquiry's notebook and cursor and reopens its authored status; does not open source or continue automatically.",
            "continue_after_selection": can_continue.then_some("SELF_STUDY CONTINUE"),
            "continuation": if !source_check { "source_reselection_required" }
                else if !pending_facts.is_empty() { "pending_source_delivery" }
                else if saved.is_some_and(|p| p.eof) { "saved_source_exhausted" }
                else if saved.is_some() { "saved_position" }
                else { "source_selection_required" },
            "reselect_after_selection": reselect,
            "browse_command": "SELF_STUDY MAP",
            "choices_scope": "Select this inquiry first to reselect source for it. OPEN 1 deliberately reads current bytes from the beginning; it does not repair the historical cursor. Missing or unreadable sources require choosing an available source through MAP. No return, note, explanation or further action is required."
        })
    }
}

fn page_facts(page: &Page, sources: &BTreeMap<String, Option<String>>) -> Value {
    let current = sources.get(&page.source).and_then(Option::as_ref);
    json!({
        "page_id": page.id,
        "source": page.source,
        "historical_source_revision": page.revision.sha256,
        "start": page.start,
        "end": page.end,
        "currently_verified_revision": current,
        "source_status": match current {
            Some(revision) if *revision == page.revision.sha256 => "verified_matching",
            Some(_) => "verified_changed",
            None => "unavailable"
        }
    })
}

// Recheck a bounded UTF-8 snapshot and exact catalog identity. Do not normalize a
// missing saved path into a different source or expose arbitrary filesystem errors.
fn current_revision(catalog: &Catalog, id: &str) -> Option<String> {
    let source = catalog.resolve(id).ok().filter(|source| source.id == id)?;
    let file = fs::File::open(&source.path).ok()?;
    let before = file.metadata().ok()?;
    let limit = 64 * 1024 * 1024;
    if !before.is_file() || before.len() > limit {
        return None;
    }
    let mut bytes = Vec::new();
    file.take(limit.saturating_add(1))
        .read_to_end(&mut bytes)
        .ok()?;
    let after_source = catalog.resolve(id).ok()?;
    let after = fs::metadata(&source.path).ok()?;
    if after_source.id != id
        || after_source.path != source.path
        || before.len() != after.len()
        || before.modified().ok()? != after.modified().ok()?
        || u64::try_from(bytes.len()).ok()? != before.len()
    {
        return None;
    }
    std::str::from_utf8(&bytes).ok()?;
    Some(digest(bytes))
}
