//! Bounded observations of verified navigation, never a redirect or a claim of learning.
use crate::{Catalog, Command, InputKind, digest};
use serde::{Deserialize, Serialize};
use std::fmt::Write as _;

const MAX_CANDIDATES: usize = 4;

#[derive(Clone, Default, Serialize, Deserialize)]
pub(crate) struct NavigationHistory {
    question_key: String,
    without_source: u32,
    candidates: Vec<Candidate>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    conflict: Option<SourceConflict>,
}

#[derive(Clone, Serialize, Deserialize)]
pub(crate) struct NavigationOffer {
    question_key: String,
    kind: InputKind,
    candidates: Vec<Candidate>,
}

#[derive(Clone, Serialize, Deserialize)]
struct Candidate {
    source: String,
    command: String,
    input_id: String,
}

#[derive(Clone, Serialize, Deserialize)]
struct SourceConflict {
    candidate: Candidate,
    selected_next: String,
}

impl Candidate {
    fn available(&self, catalog: &Catalog) -> bool {
        let Ok(Command::Open { source, line }) = Command::parse(&self.command) else {
            return false;
        };
        source == self.source
            && catalog
                .resolve(&source)
                .ok()
                .and_then(|source| crate::Page::read(&source, None, line, None).ok())
                .is_some_and(|page| page.start.byte < page.end.byte)
    }
}

pub(crate) fn question_key(id: Option<&str>, question: Option<&str>) -> String {
    digest(format!(
        "{}:{}",
        id.unwrap_or("home"),
        question.unwrap_or("")
    ))
}

impl NavigationOffer {
    pub(crate) fn new(key: String, kind: InputKind, text: &str, catalog: &Catalog) -> Self {
        // Only explicit source-opening choices in the actual offered navigation.
        // Recalled prose and runtime instructions are not interpreted as source evidence.
        let mut candidates = Vec::new();
        for line in text.lines() {
            let Some(command) = line
                .find("SELF_STUDY OPEN ")
                .or_else(|| line.find("SELF_STUDY RESUME "))
                .map(|n| &line[n..])
            else {
                continue;
            };
            // Search rows append quoted excerpts after the complete command.
            // Read only the rendered verb/path/line prefix, never that excerpt.
            let mut words = command.split_whitespace();
            words.next();
            let verb = words.next().unwrap_or("");
            let source = words.next().unwrap_or("");
            let command = match verb {
                "OPEN" => {
                    let Some(line) = words
                        .next()
                        .and_then(|s| s.parse::<usize>().ok())
                        .filter(|n| *n > 0)
                    else {
                        continue;
                    };
                    format!("SELF_STUDY OPEN {source} {line}")
                },
                "RESUME" => format!("SELF_STUDY RESUME {source}"),
                _ => continue,
            };
            let (source, line) = match Command::parse(&command) {
                Ok(Command::Open { source, line }) => (source, line),
                Ok(Command::Resume { source }) => (source, 1),
                _ => continue,
            };
            if source.len() > 400
                || source.chars().any(char::is_whitespace)
                || catalog.resolve(&source).is_err()
                || candidates.iter().any(|c: &Candidate| c.command == command)
            {
                continue;
            }
            candidates.push(Candidate {
                command: format!("SELF_STUDY OPEN {source} {line}"),
                source,
                input_id: String::new(),
            });
            // The navigation page itself is bounded. Retain enough listed choices
            // to recognize a named candidate, rather than scanning the whole catalog.
            if candidates.len() == 64 {
                break;
            }
        }
        Self {
            question_key: key,
            kind,
            candidates,
        }
    }
}

impl NavigationHistory {
    pub(crate) fn late_source(&mut self, offer: &NavigationOffer) {
        if self.question_key == offer.question_key
            && matches!(offer.kind, InputKind::SourcePage | InputKind::SourceSession)
        {
            self.without_source = 0;
            self.candidates.clear();
            self.conflict = None;
        }
    }

    pub(crate) fn record(&mut self, offer: NavigationOffer, input_id: &str, response: &str) {
        if matches!(
            offer.kind,
            InputKind::Questions
                | InputKind::Reflection
                | InputKind::Notebook
                | InputKind::InquiryReview
                | InputKind::Help
                | InputKind::RevisionRecovery
                | InputKind::RuntimeTrace
                | InputKind::Geometry
                | InputKind::Legacy
        ) {
            return;
        }
        if self.question_key != offer.question_key {
            *self = Self {
                question_key: offer.question_key,
                ..Self::default()
            };
        }
        if matches!(offer.kind, InputKind::SourcePage | InputKind::SourceSession) {
            self.without_source = 0;
            self.candidates.clear();
            self.conflict = None;
            return;
        }
        self.without_source = self.without_source.saturating_add(1);
        let feedback = crate::response_choice::inspect_response(response, false);
        if let Some(selected) = feedback.selected_next.as_deref()
            && matches!(Command::parse(selected), Ok(Command::Continue))
            && let Some(earlier) = feedback.earlier_source_command.as_deref()
            && let Some(candidate) = offer.candidates.iter().find(|c| c.command == earlier)
        {
            let mut candidate = candidate.clone();
            candidate.input_id = input_id.into();
            self.conflict = Some(SourceConflict {
                candidate,
                selected_next: selected.into(),
            });
        }
        let visible = crate::response_choice::eligible_lines(response).join("\n");
        let terms = crate::notebook::inquiry_terms(&visible);
        let named_sources = terms
            .iter()
            .filter_map(|term| {
                let mut matches = offer.candidates.iter().filter(|candidate| {
                    candidate.source == *term || candidate.source.ends_with(&format!("/{term}"))
                });
                let first = matches.next()?;
                matches
                    .all(|candidate| candidate.source == first.source)
                    .then(|| first.source.clone())
            })
            .collect::<Vec<_>>();
        for mut candidate in offer.candidates {
            let named = named_sources.contains(&candidate.source);
            if !named
                || self
                    .candidates
                    .iter()
                    .any(|c| c.command == candidate.command)
            {
                continue;
            }
            candidate.input_id = input_id.into();
            self.candidates.push(candidate);
            if self.candidates.len() > MAX_CANDIDATES {
                self.candidates.remove(0);
            }
        }
    }

    pub(crate) fn source_selection(&self, key: &str, catalog: &Catalog) -> String {
        if self.question_key != key {
            return String::new();
        }
        let conflict = self
            .conflict
            .as_ref()
            .filter(|c| c.candidate.available(catalog));
        let mut text = String::new();
        if let Some(conflict) = conflict {
            let _ = writeln!(
                text,
                "Retained verified choice conflict (input {}): your final choice was {:?}; the earlier {:?} was not selected. Neither this reference nor its suggestion executes it.",
                conflict.candidate.input_id, conflict.selected_next, conflict.candidate.command
            );
        }
        let mut shown = Vec::new();
        for candidate in conflict
            .into_iter()
            .map(|c| &c.candidate)
            .chain(self.candidates.iter())
        {
            if shown.contains(&candidate.command) || !candidate.available(catalog) {
                continue;
            }
            let _ = writeln!(
                text,
                "Previously offered location (input {}; current-checkout navigation hint, not delivered source or a bookmark): NEXT: {}",
                candidate.input_id, candidate.command
            );
            shown.push(candidate.command.clone());
            if shown.len() == MAX_CANDIDATES {
                break;
            }
        }
        text
    }

    pub(crate) fn render(&self, key: &str, kind: InputKind, catalog: &Catalog) -> String {
        if self.question_key != key
            || self.without_source < 3
            || matches!(
                kind,
                InputKind::SourcePage
                    | InputKind::SourceSession
                    | InputKind::Questions
                    | InputKind::Reflection
                    | InputKind::Notebook
                    | InputKind::InquiryReview
                    | InputKind::Help
                    | InputKind::RevisionRecovery
                    | InputKind::RuntimeTrace
                    | InputKind::Geometry
                    | InputKind::SourceSelectionNeeded
            )
        {
            return String::new();
        }
        let mut text = format!(
            "NAVIGATION RECEIPT — {} consecutive verified study inputs in this tracked inquiry supplied no numbered source page. Search excerpts can still be useful. This count starts with this reader's tracking or the current question; it is not a lifetime total, a judgment of understanding, or a limit.\n",
            self.without_source
        );
        let candidates = self
            .candidates
            .iter()
            .filter(|c| catalog.resolve(&c.source).is_ok())
            .collect::<Vec<_>>();
        for candidate in &candidates {
            let _ = writeln!(
                text,
                "Previously offered and named in your response (input {}; meaning unverified; current checkout): {}",
                candidate.input_id, candidate.command
            );
        }
        if let [first, second, ..] = candidates.as_slice() {
            let _ = writeln!(
                text,
                "Optional comparison of these candidates with your recalled account (their role remains to be checked): SELF_STUDY SESSION {} | {}",
                first.command.trim_start_matches("SELF_STUDY "),
                second.command.trim_start_matches("SELF_STUDY ")
            );
        }
        text.push_str("You may open or search a candidate, compare evidence, revise or keep your question, continue browsing, or leave the study. No command is changed and no note is rewritten.\n\n");
        text
    }
}
