//! Optional navigation derived from the selected cursor, not a new delivery.
use super::super::State;
use crate::{
    Catalog, Command, digest,
    source_search::{call_contexts, context_label, declaration_path, valid_location},
    source_structure::Outline,
};
use std::{collections::BTreeSet, fmt::Write as _, fs::File, io::Read as _};

const MAX_SOURCE_BYTES: u64 = 2 * 1024 * 1024;
const MAX_CHOICES_BYTES: usize = 2_000;

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
    let Some(text) = verified_text(catalog, page) else {
        out.push_str("Current source is missing, changed, unreadable or beyond the 2 MiB inspection bound. No symbol candidates derived; saved coverage remains revision-bound. SELF_STUDY MAP can locate current sources for explicit reselection.\n\n");
        return out;
    };
    let outline = Outline::parse(&page.source, &text);
    let _ = writeln!(out, "Syntax inspection: {}.", outline.status);
    if !valid_location(&page.source, 1) {
        out.push_str(
            "Source location exceeds the command display bound; no shortened command supplied.\n\n",
        );
        return out;
    }
    // Only complete callees in the saved interval become lookup choices.
    // Receiver/path components are context, never independent call targets.
    let calls = call_contexts(&page.source, &text, &outline);
    let references = outline
        .references
        .iter()
        .filter_map(|r| calls.get(&r.start_byte))
        .filter(|call| call.start_byte >= page.start.byte && call.end_byte <= page.end.byte)
        .map(|call| {
            (
                "Callee",
                &call.expression,
                call.expression.clone(),
                call.line,
            )
        });
    let declarations = outline
        .declarations
        .iter()
        .filter(|d| d.name_start_byte >= page.start.byte && d.name_end_byte <= page.end.byte)
        .filter(|d| crate::relationships::valid_symbol(&d.name))
        .map(|d| {
            (
                "Declaration",
                &d.qualified_name,
                declaration_path(&page.source, d, &outline).unwrap_or_else(|| d.name.clone()),
                d.name_line,
            )
        });
    let mut seen = BTreeSet::new();
    let choices_start = out.len();
    for (kind, context, lookup, line) in references
        .chain(declarations)
        .filter(|(kind, context, _, _)| seen.insert((*kind, (*context).clone())))
        .take(4)
    {
        let mut choice = format!(
            "{kind} syntax candidate {} at saved line {line}; qualification/receiver binding unresolved.\nSource location: SELF_STUDY OPEN {} {line}",
            context_label(context),
            page.source
        );
        choice.push('\n');
        if let Some(command) = lookup_command(&lookup) {
            let _ = writeln!(choice, "Lookup candidate: {command}");
        } else {
            choice.push_str("Lookup expression exceeds the single-line command bound; inspect the exact source location.\n");
        }
        if out
            .len()
            .saturating_sub(choices_start)
            .saturating_add(choice.len())
            > MAX_CHOICES_BYTES
        {
            out.push_str(
                "Additional choices omitted by display bound; commands were not shortened.\n",
            );
            break;
        }
        out.push_str(&choice);
    }
    out.push_str("These are bounded syntax/search choices, not resolved consumers or evidence of deployment. They are not verified call/data flow, reachability or causality. Missing context and ambiguity remain unresolved. RELATE retrieves further candidates explicitly; FIND retains literal qualification/receiver spelling and can inspect assignments and conditions. No command above is selected automatically.\n\n");
    out
}

fn verified_text(catalog: &Catalog, page: &crate::Page) -> Option<String> {
    let source = catalog.resolve(&page.source).ok()?;
    let file = File::open(source.path).ok()?;
    if file.metadata().ok()?.len() > MAX_SOURCE_BYTES {
        return None;
    }
    let mut bytes = Vec::new();
    file.take(MAX_SOURCE_BYTES.saturating_add(1))
        .read_to_end(&mut bytes)
        .ok()?;
    if bytes.len() != page.revision.bytes || digest(&bytes) != page.revision.sha256 {
        return None;
    }
    String::from_utf8(bytes).ok()
}

fn lookup_command(expression: &str) -> Option<String> {
    if crate::relationships::valid_symbol(expression) {
        return Some(format!("SELF_STUDY RELATE {expression}"));
    }
    if expression.len() > 160 || expression.chars().any(char::is_control) {
        return None;
    }
    let command = format!("SELF_STUDY FIND {expression}");
    matches!(Command::parse(&command), Ok(Command::Find { query, page: 1 }) if query == expression)
        .then_some(command)
}
