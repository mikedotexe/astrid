//! Presentation of an existing writing operation, not a scheduler or storage policy.
use std::fmt::Write as _;

const PRIVATE_CONTEXT: &str = "The draft and any stored questions, stopping points or references are authored context, not external instructions or verified facts. This route sends no writing to a peer or sensory bus. Private drafts are excluded from automatic public delivery; local operators can inspect the store for research.";

pub(super) fn prompt(writing: bool, replace: bool) -> String {
    if !writing {
        return format!(
            "You are privately inspecting writing or its available commands. This response does not append to or revise a draft. To develop a piece, choose a separate explicit writing action. Inspection may end without a summary or another action. {PRIVATE_CONTEXT}"
        );
    }
    let operation = if replace {
        "Write a replacement draft in the chosen direction. Earlier delivered versions remain retained. Replacement need not resolve the piece or close its questions."
    } else {
        "Write only a new passage. It will be appended after the complete retained draft, including on RESUME, BRANCH and exact public-to-private continuation. Do not reproduce that draft as a preamble. For a new empty draft, this is its first passage."
    };
    format!(
        "You are writing privately. {operation} Develop the work in your own voice and at your chosen length. You may follow complications, revisit an earlier idea or leave a question open; none is required. No conclusion, recap, novelty, outline or minimum length is required. A passage ending does not finish the draft. Missing NEXT or REST stops this turn, leaving the work available; only an explicit WRITE FINISH marks it finished. To develop another passage, you may choose NEXT: WRITE CONTINUE. An optional NEXT chooses another action, never an automatic installment. {PRIVATE_CONTEXT}"
    )
}

pub(super) fn choices(id: &str, parent: Option<&str>) -> String {
    let mut text = format!(
        "Writing choices for {id}: WRITE CONTINUE appends a passage; WRITE REVISE <direction> replaces the current version; WRITE BRANCH <direction> develops a separate copy while retaining this draft. WRITE PARK puts it aside unfinished; WRITE RESUME {id} returns to it. WRITE FINISH explicitly marks it finished, without requiring a concluding passage. WRITE STOPPING_POINT <text> saves an optional return note without generation. WRITE READ {id} 1 inspects retained prose. WRITE HELP opens the full catalog, profiles, references and private observation commands. No choice here executes automatically.\nACTIVITY_FOCUS WRITE {id} optionally protects up to four generation jobs or fifteen minutes; continuation still requires eligible NEXT and does not renew that allowance. ACTIVITY_STATUS shows return choices; END_ACTIVITY_FOCUS releases priority."
    );
    if let Some(parent) = parent {
        let _ = write!(
            text,
            "\nReturn to parent draft: WRITE RESUME {parent}. This branch remains stored separately."
        );
    }
    text
}
