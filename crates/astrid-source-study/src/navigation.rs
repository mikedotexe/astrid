use crate::progress::Progress;
use crate::{Catalog, MAX_PAGE_BYTES};
use anyhow::{Result, bail};
use std::fmt::Write as _;
use std::fs::File;
use std::io::{BufRead as _, BufReader};

impl Catalog {
    pub(crate) fn map(
        &self,
        topic: &str,
        page: usize,
        progress: &Progress,
        current: Option<&str>,
    ) -> Result<String> {
        let mut lines =
            vec!["Shared system map — the same source catalog for Astrid and Minime.".into()];
        if let Some(current) = current {
            lines.push(format!(
                "Current source: {}",
                self.study_entry(current, progress)
            ));
        }
        if topic.is_empty() {
            for component in &self.components {
                lines.push(format!(
                    "{} — {}. {} entry points have delivery history. SELF_STUDY MAP {}",
                    component.id,
                    component.title,
                    component
                        .sources
                        .iter()
                        .filter(|id| progress.contains_key(*id))
                        .count(),
                    component.id
                ));
            }
            for (id, root) in &self.roots {
                lines.push(format!(
                    "Repository {id}: {}. SELF_STUDY MAP {id}",
                    if root.is_dir() {
                        "available"
                    } else {
                        "unavailable locally"
                    }
                ));
            }
            lines.push("Includes implementation, tests, manifests, interfaces, shaders and architecture docs. Excludes runtime/private artifacts, credentials and generated build output. Source is the local checkout, not proof of deployed behavior.".into());
        } else if let Some(component) = self.components.iter().find(|c| c.id == topic) {
            lines.push(component.title.clone());
            for id in &component.sources {
                lines.push(self.study_entry(id, progress));
            }
        } else {
            let sources = self.sources()?;
            let mut found = false;
            for source in sources.iter().filter(|s| {
                s.id == topic
                    || s.id
                        .starts_with(&format!("{}/", topic.trim_end_matches('/')))
            }) {
                found = true;
                lines.push(self.study_entry(&source.id, progress));
            }
            if !found {
                bail!("no catalog entries for {topic}; use SELF_STUDY MAP");
            }
        }
        paginate(lines, &format!("SELF_STUDY MAP {topic}"), page)
    }

    fn study_entry(&self, id: &str, progress: &Progress) -> String {
        let Ok(source) = self.resolve(id) else {
            return format!("{id} [unavailable — use FIND to locate a moved implementation]");
        };
        let Some(item) = progress.get(id) else {
            return format!("SELF_STUDY OPEN {id} 1 [Not delivered]");
        };
        let unchanged = std::fs::metadata(&source.path).is_ok_and(|m| m.len() <= 64 * 1024 * 1024)
            && std::fs::read(&source.path)
                .is_ok_and(|bytes| crate::digest(bytes) == item.revision.sha256);
        if !unchanged {
            return format!(
                "SELF_STUDY OPEN {id} 1 [Source changed or unreadable; previous revision: {}]",
                item.label()
            );
        }
        if item.complete() {
            format!(
                "SELF_STUDY OPEN {id} 1 [Deliberate reread — {}]",
                item.label()
            )
        } else if item
            .ranges
            .last()
            .is_some_and(|r| r.1 == item.revision.bytes)
        {
            format!(
                "SELF_STUDY OPEN {id} 1 [Read missing earlier bytes — {}]",
                item.label()
            )
        } else {
            format!("SELF_STUDY RESUME {id} [Resume — {}]", item.label())
        }
    }

    pub(crate) fn find(&self, query: &str, page: usize) -> Result<String> {
        let mut lines = vec![format!(
            "Literal source search: {query}. Path and content matches; line numbers are one-based."
        )];
        let mut unreadable = 0usize;
        for source in self.sources()? {
            if source.id.contains(query) {
                lines.push(format!("SELF_STUDY OPEN {} 1 [path match]", source.id));
            }
            let Ok(file) = File::open(&source.path) else {
                unreadable = unreadable.saturating_add(1);
                continue;
            };
            for (offset, line) in BufReader::new(file).lines().enumerate() {
                let Ok(line) = line else {
                    unreadable = unreadable.saturating_add(1);
                    break;
                };
                if let Some(at) = line.find(query) {
                    let from = line.floor_char_boundary(at.saturating_sub(50));
                    let to = line.floor_char_boundary(
                        at.saturating_add(query.len())
                            .saturating_add(120)
                            .min(line.len()),
                    );
                    lines.push(format!(
                        "SELF_STUDY OPEN {} {} — {}",
                        source.id,
                        offset.saturating_add(1),
                        &line[from..to]
                    ));
                }
            }
        }
        if lines.len() == 1 {
            lines.push(format!("No matches for the exact literal query {query:?}. Punctuation is part of the query; try a shorter identifier or SELF_STUDY MAP. No source page was delivered."));
        }
        lines.push(format!("{unreadable} unreadable/non-UTF-8 files skipped. Search lists matches; it does not mark source as delivered."));
        paginate(lines, &format!("SELF_STUDY FIND {query}"), page)
    }
}

fn paginate(lines: Vec<String>, command: &str, page: usize) -> Result<String> {
    let mut pages = vec![String::new()];
    let budget = MAX_PAGE_BYTES
        .saturating_sub(command.len())
        .saturating_sub(300);
    for line in lines {
        let line = &line[..line.floor_char_boundary(budget.min(line.len()))];
        let current = pages.last_mut().expect("one page");
        if current.len().saturating_add(line.len()).saturating_add(1) > budget {
            pages.push(String::new());
        }
        let current = pages.last_mut().expect("one page");
        current.push_str(line);
        current.push('\n');
    }
    let mut text = pages
        .get(page.saturating_sub(1))
        .cloned()
        .ok_or_else(|| anyhow::anyhow!("navigation page is past the end"))?;
    write!(text, "\nNavigation page {page}/{}.", pages.len())?;
    if page < pages.len() {
        write!(text, " Next: {command} --page {}", page.saturating_add(1))?;
    }
    Ok(text)
}
