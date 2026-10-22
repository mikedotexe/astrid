//! Being-authored inquiries. Status is a choice, never an inferred understanding score.
use crate::Page;
use crate::notebook::Notebook;
use anyhow::{Context as _, Result, bail};
use serde::{Deserialize, Serialize};
use std::{collections::BTreeMap, fmt::Write as _};
#[path = "question_observations.rs"]
mod observations;

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub enum QuestionCommand {
    List {
        page: usize,
    },
    Review {
        id: String,
        page: usize,
    },
    Accounts {
        target: String,
    },
    Account {
        target: String,
        account: String,
        page: usize,
    },
    New(String),
    Focus(String),
    Home,
    Notebook,
    ParkNotebook,
    ReturnNotebook,
    Park(String),
    Resolve {
        id: String,
        finding: String,
    },
}
impl QuestionCommand {
    pub(crate) fn parse(input: &str) -> Result<Self> {
        let (verb, rest) = input.trim().split_once(' ').unwrap_or((input.trim(), ""));
        if let Some(number) = input.trim().strip_prefix("--page ") {
            let page: usize = number.parse()?;
            if page == 0 {
                bail!("question pages start at 1");
            }
            return Ok(Self::List { page });
        }
        Ok(match verb {
            "" => Self::List { page: 1 },
            "ACCOUNTS" | "ACCOUNT" => parse_account_command(verb, rest)?,
            "REVIEW" => {
                let mut parts = rest.split_whitespace();
                let id = parts.next().unwrap_or_default();
                let page = match parts.next() {
                    None => 1,
                    Some("--page") => parts.next().context("review page required")?.parse()?,
                    _ => bail!("use NEXT: SELF_STUDY QUESTION REVIEW qN [--page N]"),
                };
                anyhow::ensure!(
                    valid_id(id) && page > 0 && parts.next().is_none(),
                    "use NEXT: SELF_STUDY QUESTION REVIEW qN [--page N], pages start at 1"
                );
                Self::Review {
                    id: id.into(),
                    page,
                }
            },
            "NEW" if !rest.trim().is_empty() && rest.len() <= 350 => Self::New(rest.trim().into()),
            "HOME" if rest.is_empty() => Self::Home,
            "NOTEBOOK" if rest.is_empty() => Self::Notebook,
            "PARK" if rest == "NOTEBOOK" => Self::ParkNotebook,
            "RETURN" if rest == "NOTEBOOK" => Self::ReturnNotebook,
            "PARK" if valid_id(rest) => Self::Park(rest.into()),
            "RESOLVE" => {
                let (id, finding) = rest.split_once(' ').unwrap_or((rest, ""));
                validate_resolution(id, finding)?;
                Self::Resolve {
                    id: id.into(),
                    finding: finding.trim().into(),
                }
            },
            id if rest.is_empty() && valid_id(id) => Self::Focus(id.into()),
            _ => bail!(
                "choose a complete final line: NEXT: SELF_STUDY QUESTION, NEXT: SELF_STUDY QUESTION REVIEW qN [--page N], NEXT: SELF_STUDY QUESTION NEW <question, up to 350 bytes>, NEXT: SELF_STUDY QUESTION qN, NEXT: SELF_STUDY QUESTION HOME, NEXT: SELF_STUDY QUESTION PARK qN, NEXT: SELF_STUDY QUESTION RESOLVE qN (optionally append your own finding, up to 700 bytes), NEXT: SELF_STUDY QUESTION NOTEBOOK, NEXT: SELF_STUDY QUESTION PARK NOTEBOOK, or NEXT: SELF_STUDY QUESTION RETURN NOTEBOOK"
            ),
        })
    }
}
fn parse_account_command(verb: &str, rest: &str) -> Result<QuestionCommand> {
    let mut parts = rest.split_whitespace();
    let target = parts.next().unwrap_or_default();
    anyhow::ensure!(
        target == "NOTEBOOK" || valid_id(target),
        "use an exact qN or NOTEBOOK account target"
    );
    if verb == "ACCOUNTS" {
        anyhow::ensure!(
            parts.next().is_none(),
            "use SELF_STUDY QUESTION ACCOUNTS qN or NOTEBOOK"
        );
        return Ok(QuestionCommand::Accounts {
            target: target.into(),
        });
    }
    let account = parts.next().unwrap_or_default();
    anyhow::ensure!(
        account.len() == 64 && account.bytes().all(|c| c.is_ascii_hexdigit()),
        "use the exact account_sha256 from SELF_STUDY QUESTION ACCOUNTS {target}"
    );
    let page = match parts.next() {
        None => 1,
        Some("--page") => parts
            .next()
            .context("account page required")?
            .parse::<usize>()?,
        _ => bail!("use SELF_STUDY QUESTION ACCOUNT {target} {account} [--page N]"),
    };
    anyhow::ensure!(
        page > 0 && parts.next().is_none(),
        "account pages start at 1; no extra arguments"
    );
    Ok(QuestionCommand::Account {
        target: target.into(),
        account: account.into(),
        page,
    })
}
fn valid_id(s: &str) -> bool {
    s.strip_prefix('q')
        .is_some_and(|n| !n.is_empty() && n.bytes().all(|b| b.is_ascii_digit()))
}

// Recognize only syntax markers from the command reference, not the quality,
// uncertainty or truth of authored findings. Existing records remain readable.
fn is_finding_template(finding: &str) -> bool {
    let finding = finding.trim().strip_suffix('.').unwrap_or(finding.trim());
    ["[finding]", "[finding, up to 700 bytes]", "<finding>"]
        .iter()
        .any(|marker| finding.eq_ignore_ascii_case(marker))
}

fn validate_resolution(id: &str, finding: &str) -> Result<()> {
    anyhow::ensure!(
        valid_id(id) && finding.len() <= 700,
        "use NEXT: SELF_STUDY QUESTION RESOLVE qN; optionally append your own finding, up to 700 bytes"
    );
    anyhow::ensure!(
        !is_finding_template(finding),
        "RESOLVE received a literal template marker, not an authored finding. No closure was applied. To close without a finding, choose NEXT: SELF_STUDY QUESTION RESOLVE {id}. Otherwise append your own words (up to 700 bytes). You may instead inspect NEXT: SELF_STUDY QUESTION REVIEW {id}, keep reading, park, or stop. No text is inferred from surrounding prose."
    );
    Ok(())
}

fn finding_limitation(finding: &str) -> Option<&'static str> {
    is_finding_template(finding).then_some(
        "The saved finding is a literal template marker. Historical status and text are retained; this is not a substantive finding or verified answer. No automatic reopening or correction was performed.",
    )
}

#[derive(Serialize, Deserialize)]
pub(crate) struct Questions {
    pub(crate) active: Option<String>,
    next: u64,
    entries: BTreeMap<String, Inquiry>,
    unthreaded: Option<Notebook>,
    /// Presentation selection only, not an authored status or conclusion.
    #[serde(default = "default_quiet")]
    pub(crate) unthreaded_quiet: bool,
}
fn default_quiet() -> bool {
    true
}
impl Default for Questions {
    fn default() -> Self {
        Self {
            active: None,
            next: 0,
            entries: BTreeMap::new(),
            unthreaded: None,
            unthreaded_quiet: true,
        }
    }
}
#[derive(Serialize, Deserialize)]
struct Inquiry {
    question: String,
    status: String,
    finding: String,
    notebook: Notebook,
    sources: Vec<Reference>,
    #[serde(default)]
    geometry: crate::geometry::History,
    #[serde(default)]
    observations: crate::observations::History,
}
#[derive(Serialize, Deserialize)]
struct Reference {
    source: String,
    revision: String,
    line: usize,
}
impl Questions {
    pub(crate) fn notebook_view(&self, global: &Notebook) -> Result<String> {
        let notebook = self.notebook_for(None, global);
        Ok(serde_json::to_string_pretty(&serde_json::json!({
            "scope": "Explicit inspection of the retained unthreaded notebook question, not newly supplied source or verified conclusions. Inspection does not select it or change reading positions.",
            "question": notebook.question_view(),
            "quiet": self.unthreaded_quiet,
            "return_command": "SELF_STUDY QUESTION RETURN NOTEBOOK",
            "park_command": "SELF_STUDY QUESTION PARK NOTEBOOK",
            "accounts_command": "SELF_STUDY QUESTION ACCOUNTS NOTEBOOK",
            "position": "Return selects the retained unthreaded reading position, including any deliberate browsing since parking. It does not rewind or open a source automatically. Changed source bytes still require explicit reselection."
        }))?)
    }
    pub(crate) fn review(&self, id: &str, page: usize) -> Result<String> {
        let inquiry = self
            .entries
            .get(id)
            .context("question not found; use NEXT: SELF_STUDY QUESTION")?;
        let mut note: serde_json::Value = serde_json::from_str(&inquiry.notebook.note_view(page)?)?;
        note["navigation"] = format!("SELF_STUDY QUESTION REVIEW {id} --page N").into();
        Ok(serde_json::to_string_pretty(&serde_json::json!({
            "scope": "Explicit inquiry review. Historical authored accounts and supplied-source references only; not current source, verified understanding, or complete experimental history. Review does not select, reopen, resolve, or revise this inquiry. Its response cannot change saved notes; choose the inquiry and source explicitly to revise.",
            "id": id,
            "authored_revision": self.target_revision(id)?,
            "question": inquiry.question,
            "status": inquiry.status,
            "finding": inquiry.finding,
            "finding_limitation": finding_limitation(&inquiry.finding),
            "note_history": note,
            "historical_source_references": inquiry.sources,
            "select_command": format!("SELF_STUDY QUESTION {id}"),
            "accounts_command": format!("SELF_STUDY QUESTION ACCOUNTS {id}"),
            "experimental_records": "Geometry and confirmed observations remain separately inspectable; this view does not export them."
        }))?)
    }
    pub(crate) fn inspect_accounts(
        &self,
        command: &QuestionCommand,
        global: &Notebook,
    ) -> Result<Option<String>> {
        let (target, account, page) = match command {
            QuestionCommand::Accounts { target } => (target, None, 1),
            QuestionCommand::Account {
                target,
                account,
                page,
            } => (target, Some(account.as_str()), *page),
            _ => return Ok(None),
        };
        // Typed callers must not bypass the same explicit target and identity checks.
        anyhow::ensure!(
            target == "NOTEBOOK" || valid_id(target),
            "invalid account target"
        );
        if let Some(account) = account {
            anyhow::ensure!(
                account.len() == 64 && account.bytes().all(|c| c.is_ascii_hexdigit()),
                "invalid account identity"
            );
        }
        let notebook = if target == "NOTEBOOK" {
            if self.active.is_some() {
                self.unthreaded
                    .as_ref()
                    .context("unthreaded account store unavailable")?
            } else {
                global
            }
        } else {
            &self
                .entries
                .get(target)
                .context("question not found; no other inquiry inspected")?
                .notebook
        };
        Ok(Some(notebook.accounts_view(target, account, page)?))
    }
    pub(crate) fn geometry(&mut self, id: &str) -> Result<(&str, &mut crate::geometry::History)> {
        let inquiry = self
            .entries
            .get_mut(id)
            .context("existing question required")?;
        Ok((&inquiry.question, &mut inquiry.geometry))
    }

    pub(crate) fn validate_geometry(&self) -> Result<()> {
        for inquiry in self.entries.values() {
            inquiry.geometry.validate()?;
            inquiry.observations.validate(false)?;
        }
        Ok(())
    }

    pub(crate) fn target_revision(&self, id: &str) -> Result<String> {
        let inquiry = self
            .entries
            .get(id)
            .context("question not found; use NEXT: SELF_STUDY QUESTION")?;
        // Selection/parking status is not a revision of the authored account.
        let authored = crate::digest(serde_json::to_vec(&(
            &inquiry.question,
            &inquiry.finding,
            &inquiry.notebook,
            &inquiry.sources,
        ))?);
        // An empty additive family must not invalidate existing focus/return references.
        let prior = if inquiry.geometry.records.is_empty() {
            authored
        } else {
            crate::digest(serde_json::to_vec(&(authored, &inquiry.geometry))?)
        };
        if inquiry.observations.records.is_empty() {
            return Ok(prior);
        }
        Ok(crate::digest(serde_json::to_vec(&(
            prior,
            &inquiry.observations,
        ))?))
    }
    fn validate_command(&self, command: &QuestionCommand, notebook: &Notebook) -> Result<()> {
        if let QuestionCommand::Resolve { id, finding } = command {
            // Typed callers must obey the same boundary as the text parser.
            validate_resolution(id, finding)?;
        }
        match command {
            QuestionCommand::Focus(id)
            | QuestionCommand::Park(id)
            | QuestionCommand::Resolve { id, .. } => {
                self.entries
                    .get(id)
                    .context("question not found; use NEXT: SELF_STUDY QUESTION")?;
            },
            QuestionCommand::New(_) if self.entries.len() >= 32 => {
                bail!("32 questions retained; select an existing question to continue it")
            },
            QuestionCommand::ReturnNotebook | QuestionCommand::ParkNotebook => {
                anyhow::ensure!(
                    self.notebook_for(None, notebook).question_text().is_some(),
                    "no unthreaded notebook question is retained; inspect SELF_STUDY QUESTION NOTEBOOK"
                );
            },
            _ => {},
        }
        Ok(())
    }

    pub(crate) fn apply(
        &mut self,
        command: QuestionCommand,
        notebook: &mut Notebook,
    ) -> Result<String> {
        // Validate before changing focus, so an invalid command cannot lose context.
        self.validate_command(&command, notebook)?;
        let mut transition: String;
        match command {
            QuestionCommand::List { page } => return self.render_list(page),
            QuestionCommand::Review { id, page } => return self.review(&id, page),
            QuestionCommand::Notebook => return self.notebook_view(notebook),
            command @ (QuestionCommand::Accounts { .. } | QuestionCommand::Account { .. }) => {
                return self
                    .inspect_accounts(&command, notebook)?
                    .context("account inspection unavailable");
            },
            QuestionCommand::ParkNotebook => {
                self.unthreaded_quiet = true;
                transition = "Unthreaded notebook parked quietly. Exact words and reading positions are retained; no numbered inquiry was changed. Explicit inspection: NEXT: SELF_STUDY QUESTION NOTEBOOK. Explicit return: NEXT: SELF_STUDY QUESTION RETURN NOTEBOOK.\n".into();
            },
            QuestionCommand::ReturnNotebook => {
                if self.active.take().is_some() {
                    *notebook = self.unthreaded.clone().unwrap_or_default();
                }
                self.unthreaded_quiet = false;
                transition = "Selected the retained unthreaded notebook and its current reading position. No source was opened or rewound, and no inquiry was resolved.\n".into();
            },
            QuestionCommand::New(question) => {
                self.next = self.next.checked_add(1).context("question IDs exhausted")?;
                let id = format!("q{}", self.next);
                if self.active.is_none() {
                    self.unthreaded = Some(notebook.clone());
                }
                let fresh = Notebook::default();
                self.entries.insert(
                    id.clone(),
                    Inquiry {
                        question,
                        status: "open".into(),
                        finding: String::new(),
                        notebook: fresh.clone(),
                        sources: Vec::new(),
                        geometry: crate::geometry::History::default(),
                        observations: crate::observations::History::default(),
                    },
                );
                transition = format!(
                    "Created and selected {id}. No other inquiry or finding was replayed.\n"
                );
                self.active = Some(id);
                *notebook = fresh;
            },
            QuestionCommand::Focus(id) => {
                if self.active.is_none() {
                    self.unthreaded = Some(notebook.clone());
                }
                let inquiry = self.entries.get_mut(&id).context("question not found")?;
                inquiry.status = "open".into();
                *notebook = inquiry.notebook.clone();
                transition =
                    format!("Selected {id} and its retained notebook and reading position.\n");
                self.active = Some(id);
            },
            QuestionCommand::Home => {
                self.unthreaded_quiet = true;
                if let Some(id) = self.active.take() {
                    *notebook = self.unthreaded.clone().unwrap_or_default();
                    transition = format!(
                        "Left numbered inquiry {id}; its authored status is unchanged. Returned to unthreaded browsing.\n"
                    );
                } else {
                    transition =
                        "Already in unthreaded browsing; HOME did not change selection.\n".into();
                }
            },
            QuestionCommand::Park(id) => {
                self.entries
                    .get_mut(&id)
                    .context("question not found")?
                    .status = "parked".into();
                if self.active.as_ref() == Some(&id) {
                    self.unthreaded_quiet = true;
                    self.active = None;
                    *notebook = self.unthreaded.clone().unwrap_or_default();
                }
                transition = format!(
                    "Marked {id} parked; its question, findings and reading position remain available.\n"
                );
            },
            QuestionCommand::Resolve { id, finding } => {
                transition = self.resolve_inquiry(&id, finding, notebook)?;
            },
        }
        if !transition.is_empty() {
            // Report the chosen operation without replaying every old finding as
            // fresh study material. Listing/review remain explicit choices.
            transition.push_str("Saved inquiries remain available through NEXT: SELF_STUDY QUESTION or NEXT: SELF_STUDY QUESTION REVIEW qN. No new source was supplied by this operation.\n");
            return Ok(transition);
        }
        self.render_list(1)
    }

    fn resolve_inquiry(
        &mut self,
        id: &str,
        finding: String,
        notebook: &mut Notebook,
    ) -> Result<String> {
        let finding_outcome = if finding.is_empty() {
            "without a finding"
        } else {
            "and retained your finding"
        };
        let inquiry = self.entries.get_mut(id).context("question not found")?;
        inquiry.status = "resolved by you".into();
        inquiry.finding = finding;
        if self.active.as_deref() == Some(id) {
            self.unthreaded_quiet = true;
            self.active = None;
            *notebook = self.unthreaded.clone().unwrap_or_default();
        }
        Ok(format!(
            "Marked {id} resolved by you {finding_outcome}. This records your conclusion, not verified understanding.\n"
        ))
    }

    pub(crate) fn record(
        &mut self,
        id: Option<&str>,
        global: &mut Notebook,
        response: &str,
        text: &str,
        pages: &[Page],
    ) {
        if let Some(id) = id {
            let Some(inquiry) = self.entries.get_mut(id) else {
                return;
            };
            inquiry.notebook.record_pages(response, text, pages);
            if let Some(question) = inquiry.notebook.question_text() {
                inquiry.question = question.into();
            }
            for page in pages {
                inquiry
                    .sources
                    .retain(|r| r.source != page.source || r.line != page.start.line);
                inquiry.sources.push(Reference {
                    source: page.source.clone(),
                    revision: page.revision.sha256.clone(),
                    line: page.start.line,
                });
                if inquiry.sources.len() > 6 {
                    inquiry.sources.remove(0);
                }
            }
            if self.active.as_deref() == Some(id) {
                *global = inquiry.notebook.clone();
            }
        } else if self.active.is_some() {
            self.unthreaded
                .get_or_insert_with(Notebook::default)
                .record_pages(response, text, pages);
        } else {
            let prior = global.question_text().map(str::to_owned);
            global.record_pages(response, text, pages);
            // Only a newly authored question selects this context. Repeating
            // an unchanged saved field cannot undo an explicit quiet choice.
            if global.question_text().is_some() && global.question_text() != prior.as_deref() {
                self.unthreaded_quiet = false;
            }
        }
    }
    pub(crate) fn validate_notes(&self) -> anyhow::Result<()> {
        for inquiry in self.entries.values() {
            inquiry.notebook.validate_notes()?;
        }
        if let Some(home) = &self.unthreaded {
            home.validate_notes()?;
        }
        Ok(())
    }
    pub(crate) fn notebook_for<'a>(
        &'a self,
        id: Option<&str>,
        global: &'a Notebook,
    ) -> &'a Notebook {
        if let Some(id) = id {
            self.entries.get(id).map_or(global, |q| &q.notebook)
        } else if self.active.is_some() {
            self.unthreaded.as_ref().unwrap_or(global)
        } else {
            global
        }
    }
    pub(crate) fn render_context(&self, id: Option<&str>) -> String {
        let Some((id, q)) = id.and_then(|id| self.entries.get(id).map(|q| (id, q))) else {
            return String::new();
        };
        let mut out = format!(
            "\nACTIVE STUDY QUESTION {id} — your inquiry, not a verified conclusion.\nSaved inquiry question: {}\nYou may pursue, revise, resolve or park it. Browsing elsewhere is allowed.\nOptional final action line: NEXT: SELF_STUDY QUESTION PARK {id} | NEXT: SELF_STUDY QUESTION HOME\nPARK records a pause; HOME returns to unthreaded browsing. Neither declares an answer.\n",
            q.question
        );
        let _ = writeln!(
            out,
            "Optional authored closure without a finding: NEXT: SELF_STUDY QUESTION RESOLVE {id}\nYou may append your own finding (up to 700 bytes); do not copy syntax placeholders. Closure records your judgment, not verified understanding. No conclusion is required to keep reading, park or stop."
        );
        if !q.finding.is_empty() {
            let excerpt = &q.finding[..q.finding.floor_char_boundary(q.finding.len().min(200))];
            let suffix = if excerpt.len() < q.finding.len() {
                " [excerpt; NEXT: SELF_STUDY QUESTION REVIEW qN opens the full finding]"
            } else {
                ""
            };
            let _ = writeln!(out, "Your saved finding: {excerpt}{suffix}");
            if let Some(limitation) = finding_limitation(&q.finding) {
                let _ = writeln!(out, "{limitation}");
            }
        }
        for reference in q.sources.iter().rev() {
            let line = format!(
                "Earlier delivery: SELF_STUDY OPEN {} {} (revision {})\n",
                reference.source, reference.line, reference.revision
            );
            if out.len().saturating_add(line.len()) > 900 {
                break;
            }
            out.push_str(&line);
        }
        out
    }

    pub(crate) fn decision_context(&self, id: Option<&str>, notebook: &Notebook) -> String {
        let mut text = if let Some((id, inquiry)) =
            id.and_then(|id| self.entries.get(id).map(|inquiry| (id, inquiry)))
        {
            format!(
                "Selected inquiry {id}; authored status: {}.\nSaved inquiry question: {}\nYou may choose a final action line: NEXT: SELF_STUDY QUESTION PARK {id}, NEXT: SELF_STUDY QUESTION HOME, or NEXT: SELF_STUDY QUESTION RESOLVE {id}\nRESOLVE without extra text closes without a finding; optionally append your own words (up to 700 bytes), not syntax placeholders. Closure records your judgment, not verified understanding. NEXT: SELF_STUDY QUESTION REVIEW {id} explicitly reviews its saved account.\n",
                inquiry.status,
                serde_json::to_string(&inquiry.question).unwrap_or_default()
            )
        } else {
            let mut text = String::from(
                "No numbered inquiry is selected. You are in unthreaded browsing. A saved notebook question is a separate field, not an addressable qN inquiry. HOME leaves that field intact; it does not select a new source or end self-study scheduling.\n",
            );
            if let Some(question) = notebook.question_text() {
                if self.unthreaded_quiet {
                    text.push_str("Notebook commands remain available on request: NEXT: SELF_STUDY QUESTION NOTEBOOK inspects; NEXT: SELF_STUDY QUESTION RETURN NOTEBOOK selects. Inspection does not select a question.\n");
                } else {
                    let _ = writeln!(
                        text,
                        "Saved notebook question: {}\nNEXT: SELF_STUDY QUESTION PARK NOTEBOOK retains it quietly. You may revise it with a separate top-level response line STUDY_QUESTION: <your words>, or clear that field with STUDY_QUESTION: -. Do not put these notebook directives after NEXT:. Clearing does not declare an answer or resolve a numbered inquiry.",
                        serde_json::to_string(question).unwrap_or_default()
                    );
                }
            }
            text
        };
        text.push_str("NEXT: SELF_STUDY QUESTION lists existing inquiry IDs. NEXT: SELF_STUDY QUESTION NEW <your question> explicitly creates a numbered inquiry. RESOLVE and PARK require an existing qN; response_sha256 and source hashes are not inquiry IDs. No ID is inferred from a saved question.\n");
        text
    }
    fn render_list(&self, page: usize) -> Result<String> {
        let mut rows=vec!["Your study questions. Choose a complete final action line. NEXT: SELF_STUDY QUESTION qN restores its notebook and saved reading position. NEXT: SELF_STUDY QUESTION REVIEW qN [--page N] inspects an inquiry without selecting or revising it. NEXT: SELF_STUDY QUESTION NEW <question> starts a question; NEXT: SELF_STUDY QUESTION PARK qN leaves it available. NEXT: SELF_STUDY QUESTION RESOLVE qN closes without a finding; optionally append your own finding (up to 700 bytes), not syntax placeholders. Closure records your judgment, not verified understanding. NEXT: SELF_STUDY QUESTION HOME returns to unthreaded browsing. Historical questions without a saved position require an explicit source selection.".into()];
        rows.extend(self.entries.iter().map(|(id, q)| {
            format!(
                "NEXT: SELF_STUDY QUESTION {id} — {}{} — {}{}",
                q.status,
                if self.active.as_ref() == Some(id) {
                    " (active)"
                } else {
                    ""
                },
                q.question,
                if is_finding_template(&q.finding) {
                    " (saved finding is a template marker; REVIEW retains its exact text)"
                } else {
                    ""
                }
            )
        }));
        if let Some(id) = &self.active {
            rows.push(format!("Optional chosen geometry observations: SELF_STUDY GEOMETRY {{\"question\":\"{id}\",\"operation\":{{\"kind\":\"status\"}}}}. No automatic capture or experiment."));
        }
        rows.push("Retained unthreaded question: NEXT: SELF_STUDY QUESTION NOTEBOOK inspects without selecting. NEXT: SELF_STUDY QUESTION RETURN NOTEBOOK selects it; NEXT: SELF_STUDY QUESTION PARK NOTEBOOK keeps it quiet. Findings remain available through QUESTION REVIEW qN, not automatically repeated in this index.".into());
        crate::navigation::paginate(rows, "SELF_STUDY QUESTION", page)
    }
}

#[cfg(test)]
mod geometry_migration_tests {
    use super::*;
    #[test]
    fn empty_geometry_preserves_preexisting_question_revision() {
        let mut questions = Questions::default();
        questions
            .apply(
                QuestionCommand::New("Existing inquiry".into()),
                &mut Notebook::default(),
            )
            .unwrap();
        let inquiry = &questions.entries["q1"];
        let original = crate::digest(
            serde_json::to_vec(&(
                &inquiry.question,
                &inquiry.finding,
                &inquiry.notebook,
                &inquiry.sources,
            ))
            .unwrap(),
        );
        let mut legacy = serde_json::to_value(&questions).unwrap();
        legacy["entries"]["q1"]
            .as_object_mut()
            .unwrap()
            .remove("geometry");
        let migrated: Questions = serde_json::from_value(legacy).unwrap();
        assert_eq!(migrated.target_revision("q1").unwrap(), original);
        assert_eq!(questions.target_revision("q1").unwrap(), original);
    }
}
