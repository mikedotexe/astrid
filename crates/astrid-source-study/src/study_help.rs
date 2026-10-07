//! Explicit reference, kept out of ordinary source-page prompts.
use super::State;
use crate::Catalog;
#[path = "verification_context.rs"]
mod verification;

pub(super) fn render(topic: &str, state: &State, catalog: &Catalog) -> String {
    let body = match topic {
        "navigation" => include_str!("../help-navigation.txt"),
        "verification" => include_str!("../help-verification.txt"),
        "notebook" => include_str!("../help-notebook.txt"),
        "attention" => include_str!("../help-attention.txt"),
        _ => {
            "SELF_STUDY HELP navigation: maps, literal search, related symbols, chosen source sessions and delivery traces.\nSELF_STUDY HELP verification: inspect arguments, definitions and consumers together; distinguish comments and calculations from effects.\nSELF_STUDY HELP notebook: authored questions, notes, contrary evidence, revisions and explicit return.\nSELF_STUDY HELP attention: private writing, bounded focus, parking and stopping.\nOpening help never selects an inquiry or supplies a source page."
        },
    };
    let context = if topic == "verification" {
        verification::render(state, catalog)
    } else {
        String::new()
    };
    format!(
        "STUDY COMMAND REFERENCE\n{context}{body}\n\nCommands above are reference only. Choose an action on a final NEXT: line, or stop; no example is automatically executed."
    )
}
