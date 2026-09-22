//! Being-authored inquiries. Status is a choice, never an inferred understanding score.
use crate::Page;
use crate::notebook::Notebook;
use anyhow::{Context as _, Result, bail};
use serde::{Deserialize, Serialize};
use std::{collections::BTreeMap, fmt::Write as _};

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub enum QuestionCommand {
    List { page: usize },
    New(String),
    Focus(String),
    Home,
    Park(String),
    Resolve { id: String, finding: String },
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
            "NEW" if !rest.trim().is_empty() && rest.len() <= 350 => Self::New(rest.trim().into()),
            "HOME" => Self::Home,
            "PARK" if valid_id(rest) => Self::Park(rest.into()),
            "RESOLVE" => {
                let (id, finding) = rest.split_once(' ').unwrap_or((rest, ""));
                if !valid_id(id) || finding.len() > 700 {
                    bail!("use QUESTION RESOLVE qN [finding, up to 700 bytes]");
                }
                Self::Resolve {
                    id: id.into(),
                    finding: finding.trim().into(),
                }
            },
            id if rest.is_empty() && valid_id(id) => Self::Focus(id.into()),
            _ => bail!(
                "use QUESTION, QUESTION NEW <question, up to 350 bytes>, QUESTION qN, QUESTION HOME, QUESTION PARK qN, or QUESTION RESOLVE qN [finding]"
            ),
        })
    }
}
fn valid_id(s: &str) -> bool {
    s.strip_prefix('q')
        .is_some_and(|n| !n.is_empty() && n.bytes().all(|b| b.is_ascii_digit()))
}

#[derive(Default, Serialize, Deserialize)]
pub(crate) struct Questions {
    pub(crate) active: Option<String>,
    next: u64,
    entries: BTreeMap<String, Inquiry>,
    unthreaded: Option<Notebook>,
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
}
#[derive(Serialize, Deserialize)]
struct Reference {
    source: String,
    revision: String,
    line: usize,
}
impl Questions {
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
        }
        Ok(())
    }

    pub(crate) fn target_revision(&self, id: &str) -> Result<String> {
        let inquiry = self
            .entries
            .get(id)
            .context("question not found; use SELF_STUDY QUESTION")?;
        // Selection/parking status is not a revision of the authored account.
        let authored = crate::digest(serde_json::to_vec(&(
            &inquiry.question,
            &inquiry.finding,
            &inquiry.notebook,
            &inquiry.sources,
        ))?);
        // An empty additive family must not invalidate existing focus/return references.
        if inquiry.geometry.records.is_empty() {
            return Ok(authored);
        }
        Ok(crate::digest(serde_json::to_vec(&(
            authored,
            &inquiry.geometry,
        ))?))
    }
    pub(crate) fn apply(
        &mut self,
        command: QuestionCommand,
        notebook: &mut Notebook,
    ) -> Result<String> {
        // Validate before changing focus, so an invalid command cannot lose context.
        match &command {
            QuestionCommand::Focus(id)
            | QuestionCommand::Park(id)
            | QuestionCommand::Resolve { id, .. } => {
                self.entries
                    .get(id)
                    .context("question not found; use SELF_STUDY QUESTION")?;
            },
            QuestionCommand::New(_) if self.entries.len() >= 32 => {
                bail!("32 questions retained; select an existing question to continue it")
            },
            _ => {},
        }
        match command {
            QuestionCommand::List { page } => return self.render_list(page),
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
                    },
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
                self.active = Some(id);
            },
            QuestionCommand::Home => {
                if self.active.take().is_some() {
                    *notebook = self.unthreaded.clone().unwrap_or_default();
                }
            },
            QuestionCommand::Park(id) => {
                self.entries
                    .get_mut(&id)
                    .context("question not found")?
                    .status = "parked".into();
                if self.active.as_ref() == Some(&id) {
                    self.active = None;
                    *notebook = self.unthreaded.clone().unwrap_or_default();
                }
            },
            QuestionCommand::Resolve { id, finding } => {
                let inquiry = self.entries.get_mut(&id).context("question not found")?;
                inquiry.status = "resolved by you".into();
                inquiry.finding = finding;
                if self.active.as_ref() == Some(&id) {
                    self.active = None;
                    *notebook = self.unthreaded.clone().unwrap_or_default();
                }
            },
        }
        self.render_list(1)
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
            global.record_pages(response, text, pages);
        }
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
            "\nACTIVE STUDY QUESTION {id} — your inquiry, not a verified conclusion.\nSaved inquiry question: {}\nYou may pursue, revise, resolve or park it. Browsing elsewhere is allowed.\nOptional: SELF_STUDY QUESTION PARK {id} | SELF_STUDY QUESTION HOME\nPARK records a pause; HOME returns to unthreaded browsing. Neither declares an answer.\n",
            q.question
        );
        if !q.finding.is_empty() {
            let excerpt = &q.finding[..q.finding.floor_char_boundary(q.finding.len().min(200))];
            let suffix = if excerpt.len() < q.finding.len() {
                " [excerpt; QUESTION lists the full finding]"
            } else {
                ""
            };
            let _ = writeln!(out, "Your saved finding: {excerpt}{suffix}");
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
    fn render_list(&self, page: usize) -> Result<String> {
        let mut rows=vec!["Your study questions. Select qN to restore its notebook and saved reading position. NEW starts a question; PARK leaves it available; RESOLVE records your conclusion, without verifying it. HOME returns to unthreaded browsing. Historical questions without a saved position require an explicit source selection.".into()];
        rows.extend(self.entries.iter().map(|(id, q)| {
            format!(
                "SELF_STUDY QUESTION {id} — {}{} — {}",
                q.status,
                if self.active.as_ref() == Some(id) {
                    " (active)"
                } else {
                    ""
                },
                q.question
            )
        }));
        if let Some(id) = &self.active {
            rows.push(format!("Optional chosen geometry observations: SELF_STUDY GEOMETRY {{\"question\":\"{id}\",\"operation\":{{\"kind\":\"status\"}}}}. No automatic capture or experiment."));
        }
        for (id, q) in &self.entries {
            if !q.finding.is_empty() {
                rows.push(format!(
                    "{id} — your saved finding (not independently verified): {}",
                    q.finding
                ));
            }
        }
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
