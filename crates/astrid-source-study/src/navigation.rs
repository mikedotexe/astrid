use crate::progress::Progress;
use crate::{Catalog, MAX_PAGE_BYTES};
use anyhow::{Result, bail};
use std::fmt::Write as _;

impl Catalog {
    pub(crate) fn map(
        &self,
        topic: &str,
        page: usize,
        progress: &Progress,
        current: Option<&str>,
    ) -> Result<String> {
        let topic = topic.trim_end_matches('/');
        let mut lines = Vec::new();
        let mut scoped = false;
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
                lines.push(self.classified_entry(id, progress));
            }
            lines.push("These are entry points. Browse their directories for the surrounding implementation:".into());
            let directories = component
                .sources
                .iter()
                .filter_map(|id| id.rsplit_once('/').map(|(directory, _)| directory))
                .collect::<std::collections::BTreeSet<_>>();
            for directory in directories {
                lines.push(format!("SELF_STUDY MAP {directory}"));
            }
        } else {
            scoped = true;
            lines = self.directory_entries(topic, progress)?;
        }
        self.catalog_page("MAP", topic, page, lines, progress, current, scoped)
    }

    pub(crate) fn list(
        &self,
        topic: &str,
        page: usize,
        progress: &Progress,
        current: Option<&str>,
    ) -> Result<String> {
        let topic = topic.trim_end_matches('/');
        let prefix = format!("{topic}/");
        let lines = self
            .sources()?
            .iter()
            .filter(|source| source.id == topic || source.id.starts_with(&prefix))
            .map(|source| self.classified_entry(&source.id, progress))
            .collect::<Vec<_>>();
        if lines.is_empty() {
            bail!("no catalog entries for {topic}; use SELF_STUDY MAP");
        }
        self.catalog_page("LIST", topic, page, lines, progress, current, true)
    }

    fn directory_entries(&self, topic: &str, progress: &Progress) -> Result<Vec<String>> {
        use std::collections::BTreeMap;
        let prefix = format!("{topic}/");
        let mut directories: BTreeMap<String, BTreeMap<crate::source_search::Role, usize>> =
            BTreeMap::new();
        let mut entries = Vec::new();
        for source in self.sources()? {
            let remainder = if source.id == topic {
                ""
            } else if let Some(remainder) = source.id.strip_prefix(&prefix) {
                remainder
            } else {
                continue;
            };
            let role = crate::source_search::path_role(&source.id);
            if let Some((child, _)) = remainder.split_once('/') {
                let counts = directories.entry(format!("{prefix}{child}")).or_default();
                let count = counts.entry(role).or_default();
                *count = count.saturating_add(1);
            } else {
                entries.push((
                    role_priority(role),
                    1,
                    source.id.clone(),
                    self.classified_entry(&source.id, progress),
                ));
            }
        }
        for (directory, counts) in directories {
            let total = counts
                .values()
                .fold(0_usize, |sum, count| sum.saturating_add(*count));
            let rank = counts
                .keys()
                .map(|role| role_priority(*role))
                .min()
                .unwrap_or(5);
            let roles = counts
                .iter()
                .map(|(role, count)| format!("{}: {count}", role.label()))
                .collect::<Vec<_>>()
                .join("; ");
            let text = format!(
                "Directory {directory} [{total} catalog files; {roles}]\nSELF_STUDY MAP {directory}"
            );
            entries.push((rank, 0, directory, text));
        }
        if entries.is_empty() {
            bail!("no catalog entries for {topic}; use SELF_STUDY MAP");
        }
        entries.sort_by(|a, b| (&a.0, &a.1, &a.2).cmp(&(&b.0, &b.1, &b.2)));
        Ok(entries.into_iter().map(|entry| entry.3).collect())
    }

    fn classified_entry(&self, id: &str, progress: &Progress) -> String {
        format!(
            "[{}] {}",
            crate::source_search::path_role(id).label(),
            self.study_entry(id, progress)
        )
    }

    #[allow(clippy::too_many_arguments)]
    fn catalog_page(
        &self,
        verb: &str,
        topic: &str,
        page: usize,
        lines: Vec<String>,
        progress: &Progress,
        current: Option<&str>,
        scoped: bool,
    ) -> Result<String> {
        let command = format!("SELF_STUDY {verb} {topic}").trim_end().to_string();
        let kind = if verb == "MAP" { "map" } else { "list" };
        let mut header = if verb == "MAP" {
            "Shared system map — the same source catalog for Astrid and Minime.\nCompact maps show immediate directories and files; component maps offer direct entry points.\n".to_string()
        } else {
            "Recursive source list — every available catalog file under the chosen scope, including historical material.\n".to_string()
        };
        header.push_str("Path-based labels describe file roles, not verified behavior. A directory is not source code; OPEN or RESUME reads a file. CONTINUE resumes the saved source bookmark, not the next map/list page.\n");
        if let Some(current) = current {
            writeln!(
                header,
                "Saved source bookmark (not source shown this turn): {}",
                self.study_entry(current, progress)
            )?;
        }
        let parent = topic.rsplit_once('/').map_or("", |(parent, _)| parent);
        let mut choices = format!("Parent/system map: SELF_STUDY MAP {parent}")
            .trim_end()
            .to_string();
        if scoped {
            write!(
                choices,
                "\nCompact scope: SELF_STUDY MAP {topic}\nRecursive scope: SELF_STUDY LIST {topic}"
            )?;
        }
        choices.push_str("\nChoose any exact OPEN/RESUME above to read source directly, or browse, reread, change the question, or stop. No source bookmark advanced.");
        let budget = MAX_PAGE_BYTES
            .saturating_sub(header.len())
            .saturating_sub(choices.len())
            .saturating_sub(command.len())
            .saturating_sub(200);
        let pages = bounded_pages(lines, budget)?;
        let Some(body) = page.checked_sub(1).and_then(|index| pages.get(index)) else {
            let mut recovery = format!(
                "Requested {kind} page {page} is past the end ({} pages). Start this view: {command} --page 1.",
                pages.len()
            );
            if verb == "MAP" && scoped {
                write!(
                    recovery,
                    " Compact maps now group immediate directories and files. For the former recursive catalog use SELF_STUDY LIST {topic} --page {page}; its page boundaries may differ. Compact scope: SELF_STUDY MAP {topic}."
                )?;
            }
            bail!("{recovery}");
        };
        let mut text = header;
        text.push_str(body);
        write!(text, "\nNavigation page {page}/{}. ", pages.len())?;
        if page < pages.len() {
            writeln!(
                text,
                "Next {kind} page: {command} --page {}",
                page.saturating_add(1)
            )?;
        } else {
            writeln!(text, "End of {kind}; no further {kind} page.")?;
        }
        text.push_str(&choices);
        Ok(text)
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
        let report = self.search(query, false)?;
        let header = report.header(&format!(
            "Literal source search: {query}. Path and content matches; line numbers are one-based."
        ));
        paginate_with_header(
            &header,
            report.lines(query, false),
            &format!("SELF_STUDY FIND {query}"),
            page,
        )
    }
}

pub(crate) fn paginate(lines: Vec<String>, command: &str, page: usize) -> Result<String> {
    paginate_with_header("", lines, command, page)
}

pub(crate) fn paginate_with_header(
    header: &str,
    lines: Vec<String>,
    command: &str,
    page: usize,
) -> Result<String> {
    let mut pages = vec![String::new()];
    let budget = MAX_PAGE_BYTES
        .saturating_sub(command.len())
        .saturating_sub(header.len())
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
    text.insert_str(0, header);
    write!(text, "\nNavigation page {page}/{}.", pages.len())?;
    if page < pages.len() {
        write!(text, " Next: {command} --page {}", page.saturating_add(1))?;
    }
    text.push('\n');
    Ok(text)
}

fn role_priority(role: crate::source_search::Role) -> u8 {
    use crate::source_search::Role;
    match role {
        Role::Implementation => 0,
        Role::Configuration => 1,
        Role::Test => 2,
        Role::Documentation => 3,
        Role::History => 4,
    }
}

fn bounded_pages(lines: Vec<String>, budget: usize) -> Result<Vec<String>> {
    let mut pages = vec![String::new()];
    for line in lines {
        if line.len().saturating_add(1) > budget {
            bail!(
                "navigation entry exceeds the page allowance; choose a narrower directory with SELF_STUDY MAP <repository/directory>; commands were not shortened"
            );
        }
        let current = pages.last().expect("one page");
        if current.len().saturating_add(line.len()).saturating_add(1) > budget {
            pages.push(String::new());
        }
        let current = pages.last_mut().expect("one page");
        current.push_str(&line);
        current.push('\n');
    }
    Ok(pages)
}
