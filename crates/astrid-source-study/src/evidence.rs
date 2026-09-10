use serde::{Deserialize, Serialize};

/// Describes the input offered this turn, never the accuracy of its response.
#[derive(Clone, Copy, Debug, Default, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum InputKind {
    SourcePage,
    PrivateWriting,
    SourceSession,
    Relationships,
    Questions,
    RuntimeTrace,
    Map,
    Search,
    EndOfFile,
    Recovery,
    /// Older retained navigation offers have no explicit kind. Do not infer one
    /// from their prose or rewrite their already-bound input.
    #[default]
    Legacy,
}

impl InputKind {
    #[must_use]
    pub fn scope(self) -> &'static str {
        match self {
            Self::PrivateWriting => {
                "Private writing: your developing draft and recalled evidence, not newly supplied source or verified facts. This writing is saved locally; sharing is a separate choice."
            },
            Self::SourceSession => {
                "Study session: two or three numbered local source pages are supplied together. Each has its own revision and byte interval; one completed response is required before any bookmark advances."
            },
            Self::Relationships => {
                "Symbol relationships: lexical navigation candidates only, not resolved calls or proof of execution. OPEN a result for source context."
            },
            Self::Questions => {
                "Your study questions: authored context and status choices, not new source or verified conclusions."
            },
            Self::RuntimeTrace => {
                "Runtime record view: selected fields from your retained local execution records, observed at read time. Missing links remain unknown; this view executes no action and supplies no new source."
            },
            Self::SourcePage => {
                "Source page: numbered local source is supplied below. Only its stated revision and interval are shown this turn."
            },
            Self::Map => {
                "Map: navigation and delivery history only. No new source page is supplied this turn."
            },
            Self::Search => {
                "Search: matching excerpts and navigation only. These snippets are not a complete source page; OPEN a result to inspect its context and revision."
            },
            Self::EndOfFile => {
                "End of file: no new source bytes are supplied this turn. OPEN deliberately rereads; MAP chooses another source."
            },
            Self::Recovery => {
                "Recovery map: the requested source was not supplied. No new source page is supplied this turn."
            },
            Self::Legacy => {
                "Older retained input: its kind was not recorded. Consult its exact input; delivery does not verify response claims."
            },
        }
    }
}
