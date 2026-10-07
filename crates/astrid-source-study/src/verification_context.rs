//! Optional navigation derived from the selected cursor, not a new delivery.
use super::super::State;
use crate::{Catalog, digest, source_structure::Outline};
use std::{collections::BTreeSet, fmt::Write as _, fs::File, io::Read as _};

const MAX_SOURCE_BYTES: u64 = 2 * 1024 * 1024;

pub(super) fn render(state: &State, catalog: &Catalog) -> String {
    let context = state.questions.active.as_deref().unwrap_or("unthreaded");
    let mut out = format!(
        "CURRENT VERIFICATION CONTEXT: {context}. Reference only; no new source is supplied or executed.\n"
    );
    let Some(page) = state
        .current
        .as_ref()
        .and_then(|id| state.bookmarks.get(id))
    else {
        out.push_str("No saved source position in this context. Select a source using SELF_STUDY MAP or SELF_STUDY FIND <literal text>; no other inquiry's cursor is used.\n\n");
        return out;
    };
    let _ = writeln!(
        out,
        "Saved interval: {} bytes {}..{}, sha256={}; next byte={}, EOF={}.",
        page.source, page.start.byte, page.end.byte, page.revision.sha256, page.end.byte, page.eof
    );
    if state.pending.is_some() || state.pending_session.is_some() {
        out.push_str("A separate source offer is pending; this help does not acknowledge it or count it as delivered.\n");
    }
    let bytes = catalog.resolve(&page.source).ok().and_then(|source| {
        let file = File::open(source.path).ok()?;
        if file.metadata().ok()?.len() > MAX_SOURCE_BYTES {
            return None;
        }
        let mut bytes = Vec::new();
        file.take(MAX_SOURCE_BYTES.saturating_add(1))
            .read_to_end(&mut bytes)
            .ok()?;
        (bytes.len() == page.revision.bytes && digest(&bytes) == page.revision.sha256)
            .then_some(bytes)
    });
    let Some(text) = bytes.and_then(|bytes| String::from_utf8(bytes).ok()) else {
        out.push_str("Current source is missing, changed, unreadable or beyond the 2 MiB inspection bound. No symbol candidates derived; saved coverage remains revision-bound. SELF_STUDY MAP can locate current sources for explicit reselection.\n\n");
        return out;
    };
    let outline = Outline::parse(&page.source, &text);
    let _ = writeln!(out, "Syntax inspection: {}.", outline.status);
    // Prefer call names actually in the saved interval, then declarations there.
    // A bare-name search does not establish qualification or receiver binding.
    let references = outline
        .references
        .iter()
        .filter(|r| {
            r.usage == "call" && r.start_byte >= page.start.byte && r.end_byte <= page.end.byte
        })
        .map(|r| (&r.name, r.line));
    let declarations = outline
        .declarations
        .iter()
        .filter(|d| d.name_start_byte >= page.start.byte && d.name_end_byte <= page.end.byte)
        .map(|d| (&d.name, d.name_line));
    let mut seen = BTreeSet::new();
    for (name, line) in references
        .chain(declarations)
        .filter(|(name, _)| crate::relationships::valid_symbol(name))
        .filter(|(name, _)| seen.insert((*name).clone()))
        .take(4)
    {
        let _ = writeln!(
            out,
            "Name candidate at saved line {line}: SELF_STUDY RELATE {name}"
        );
    }
    out.push_str("These are bounded name-search choices, not resolved consumers or evidence of deployment. RELATE retrieves further candidates explicitly; FIND can inspect assignments and conditions. No command above is selected automatically.\n\n");
    out
}
