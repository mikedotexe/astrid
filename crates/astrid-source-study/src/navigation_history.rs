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
                || candidates.iter().any(|c: &Candidate| c.source == source)
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
        }
    }

    pub(crate) fn record(&mut self, offer: NavigationOffer, input_id: &str, response: &str) {
        if matches!(
            offer.kind,
            InputKind::Questions | InputKind::RuntimeTrace | InputKind::Legacy
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
            return;
        }
        self.without_source = self.without_source.saturating_add(1);
        let visible = crate::response_choice::eligible_lines(response).join("\n");
        let terms = crate::notebook::inquiry_terms(&visible);
        let named_sources = terms
            .iter()
            .filter_map(|term| {
                let mut matches = offer.candidates.iter().filter(|candidate| {
                    candidate.source == *term || candidate.source.ends_with(&format!("/{term}"))
                });
                let first = matches.next()?;
                matches.next().is_none().then(|| first.source.clone())
            })
            .collect::<Vec<_>>();
        for mut candidate in offer.candidates {
            let named = named_sources.contains(&candidate.source);
            if !named || self.candidates.iter().any(|c| c.source == candidate.source) {
                continue;
            }
            candidate.input_id = input_id.into();
            self.candidates.push(candidate);
            if self.candidates.len() > MAX_CANDIDATES {
                self.candidates.remove(0);
            }
        }
    }

    pub(crate) fn render(&self, key: &str, kind: InputKind, catalog: &Catalog) -> String {
        if self.question_key != key
            || self.without_source < 3
            || matches!(
                kind,
                InputKind::SourcePage
                    | InputKind::SourceSession
                    | InputKind::Questions
                    | InputKind::RuntimeTrace
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
