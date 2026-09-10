//! Bounded lexical evidence with explicit file roles; not semantic indexing.
use crate::Catalog;
use anyhow::Result;
use std::{collections::BTreeMap, fs, path::Path};

const MAX_HITS: usize = 1500;
const MAX_BYTES: u64 = 128 * 1024 * 1024;

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub(crate) enum Role {
    Implementation,
    Test,
    History,
    Documentation,
    Configuration,
}

impl Role {
    const ALL: [Self; 5] = [
        Self::Implementation,
        Self::Test,
        Self::History,
        Self::Documentation,
        Self::Configuration,
    ];

    fn label(self) -> &'static str {
        match self {
            Self::Implementation => "Implementation text",
            Self::Test => "Test / fixture / example material",
            Self::History => "Historical commentary",
            Self::Documentation => "Other documentation",
            Self::Configuration => "Configuration / data / interfaces",
        }
    }
}

#[derive(Default)]
struct Counts {
    lines: usize,
    paths: usize,
    rust_lines: usize,
}

pub(crate) struct SearchReport {
    counts: BTreeMap<Role, Counts>,
    rows: BTreeMap<(Role, u8), Vec<String>>,
    nearby: BTreeMap<String, (usize, String)>,
    catalog_files: usize,
    read_files: usize,
    skipped: usize,
    bytes: u64,
    hits: usize,
    bounded: bool,
}

impl SearchReport {
    pub(crate) fn header(&self, title: &str) -> String {
        let mut lines = vec![title.to_owned(),
            "Evidence roles use catalog paths and conservative Rust test markers, not a compiler. Implementation text may contain comments or strings; declaration-like text is not proof of a declaration or runtime use. Test data does not establish a production counterpart. Historical commentary may repeat earlier claims; it is not independent implementation evidence.".into()];
        for role in Role::ALL {
            let empty = Counts::default();
            let count = self.counts.get(&role).unwrap_or(&empty);
            lines.push(format!(
                "{}: {} matching lines ({} Rust), {} path matches.",
                role.label(),
                count.lines,
                count.rust_lines,
                count.paths
            ));
        }
        if self
            .counts
            .get(&Role::Implementation)
            .is_none_or(|c| c.lines == 0)
        {
            lines.push("No implementation-text occurrence found in the scanned portion of this catalog. This does not prove a mechanism is absent or that an equivalent exists.".into());
        }
        lines.push(format!("Scope: current local catalog, {} listed files; {} UTF-8 files read; {} metadata bytes considered; {} skipped; scan limit reached: {}. Counts are matching lines, not distinct symbols or calls, and include all result pages. Skipped files and any unscanned remainder are unknown. Rust #[cfg(test)] marks a conservative test remainder. Local source is not proof of deployed behavior. No source bookmark advanced.", self.catalog_files, self.read_files, self.bytes, self.skipped, self.bounded));
        lines.join("\n") + "\n\n"
    }

    fn collect_lines(
        &mut self,
        source: &crate::Source,
        role: Role,
        text: &str,
        query: &str,
        exact: bool,
    ) {
        let rust = source.path.extension().is_some_and(|ext| ext == "rs");
        let mut test_remainder = role == Role::Test;
        for (offset, line) in text.lines().enumerate() {
            if rust && line.trim_start().starts_with("#[cfg(test)]") {
                test_remainder = true;
            }
            let role = if test_remainder { Role::Test } else { role };
            let number = offset.saturating_add(1);
            let words = line
                .split(|c: char| !c.is_ascii_alphanumeric() && c != '_')
                .filter(|s| !s.is_empty())
                .collect::<Vec<_>>();
            if exact && role == Role::Implementation {
                for word in &words {
                    if self.nearby.contains_key(*word) || self.nearby.len() >= 100 {
                        continue;
                    }
                    if let Some(distance) = spelling_distance(query, word) {
                        self.nearby.insert(
                            (*word).into(),
                            (distance, format!("SELF_STUDY OPEN {} {number}", source.id)),
                        );
                    }
                }
            }
            let found = if exact {
                words.contains(&query)
            } else {
                line.contains(query)
            };
            if !found {
                continue;
            }
            let kind = if exact
                && words.windows(2).any(|w| {
                    matches!(
                        w[0],
                        "fn" | "struct" | "enum" | "trait" | "type" | "class" | "def" | "function"
                    ) && w[1] == query
                }) {
                0
            } else if exact && words.first() == Some(&"impl") {
                1
            } else {
                2
            };
            let count = self.counts.entry(role).or_default();
            count.lines = count.lines.saturating_add(1);
            if rust {
                count.rust_lines = count.rust_lines.saturating_add(1);
            }
            let at = line.find(query).unwrap_or(0);
            let from = line.floor_char_boundary(at.saturating_sub(50));
            let to = line.floor_char_boundary(
                at.saturating_add(query.len())
                    .saturating_add(120)
                    .min(line.len()),
            );
            self.rows.entry((role, kind)).or_default().push(format!(
                "[{}] SELF_STUDY OPEN {} {number} — line {number}: {}",
                role.label(),
                source.id,
                &line[from..to]
            ));
            self.hits = self.hits.saturating_add(1);
            if self.hits >= MAX_HITS {
                self.bounded = true;
                break;
            }
        }
    }

    pub(crate) fn lines(self, query: &str, exact: bool) -> Vec<String> {
        let mut lines = Vec::new();
        if self.hits == 0 {
            lines.push(if exact {
                format!("No exact identifier matches for {query}. Try FIND <shorter literal text> or MAP.")
            } else {
                format!("No matches for the exact literal query {query:?}. Punctuation is part of the query; try a shorter identifier or SELF_STUDY MAP. No source page was delivered.")
            });
        }
        if self
            .counts
            .get(&Role::Implementation)
            .is_none_or(|c| c.lines == 0)
            && !self.nearby.is_empty()
        {
            lines.push("Nearby spellings in implementation text (candidates only; spelling does not establish the role you intend, equivalence, or a declaration):".into());
            let mut candidates = self.nearby.into_iter().collect::<Vec<_>>();
            candidates.sort_by(|(a, (distance_a, _)), (b, (distance_b, _))| {
                (distance_a, a).cmp(&(distance_b, b))
            });
            for (word, (_, location)) in candidates.into_iter().take(3) {
                lines.push(format!("Candidate identifier text {word}: {location}"));
            }
        }
        for ((role, kind), rows) in self.rows {
            let label = match (role, kind) {
                (Role::Implementation, 0) => {
                    "Definition candidates (declaration-like text; inspect the surrounding source)"
                },
                (Role::Implementation, 1) => "Implementation blocks (lexical candidates)",
                (Role::Test, _) => {
                    "Test occurrences (including fixture strings; purpose requires surrounding source)"
                },
                _ => "Other references",
            };
            lines.push(format!("{} — {label}", role.label()));
            lines.extend(rows);
        }
        lines.push("OPEN any exact result to inspect numbered source and context. You may page through the remaining matches, reread, change the query, browse MAP, or stop; no replacement mechanism has been inferred.".into());
        lines
    }
}

impl Catalog {
    pub(crate) fn search(&self, query: &str, exact: bool) -> Result<SearchReport> {
        let sources = self.sources()?;
        let mut report = SearchReport {
            counts: BTreeMap::new(),
            rows: BTreeMap::new(),
            nearby: BTreeMap::new(),
            catalog_files: sources.len(),
            read_files: 0,
            skipped: 0,
            bytes: 0,
            hits: 0,
            bounded: false,
        };
        for source in sources {
            if report.hits >= MAX_HITS {
                report.bounded = true;
                break;
            }
            let role = path_role(&source.id);
            if !exact && source.id.contains(query) {
                let count = report.counts.entry(role).or_default();
                count.paths = count.paths.saturating_add(1);
                report.rows.entry((role, 2)).or_default().push(format!(
                    "[{}] SELF_STUDY OPEN {} 1 [path match]",
                    role.label(),
                    source.id
                ));
                report.hits = report.hits.saturating_add(1);
            }
            let Ok(meta) = fs::metadata(&source.path) else {
                report.skipped = report.skipped.saturating_add(1);
                continue;
            };
            if meta.len() > 64 * 1024 * 1024 {
                report.skipped = report.skipped.saturating_add(1);
                continue;
            }
            if report.bytes.saturating_add(meta.len()) > MAX_BYTES {
                report.bounded = true;
                break;
            }
            report.bytes = report.bytes.saturating_add(meta.len());
            let Ok(text) = fs::read_to_string(&source.path) else {
                report.skipped = report.skipped.saturating_add(1);
                continue;
            };
            report.read_files = report.read_files.saturating_add(1);
            report.collect_lines(&source, role, &text, query, exact);
        }
        Ok(report)
    }
}

fn path_role(id: &str) -> Role {
    let lower = id.to_ascii_lowercase();
    let path = Path::new(&lower);
    let name = path.file_name().and_then(|s| s.to_str()).unwrap_or("");
    if lower.split('/').any(|part| {
        matches!(
            part,
            "tests" | "test" | "fixtures" | "testdata" | "examples" | "benches" | "__tests__"
        )
    }) || matches!(name, "tests.rs" | "test.rs" | "conftest.py")
        || name.starts_with("test_")
        || name.ends_with("_test.rs")
        || name.ends_with("_tests.rs")
        || name.ends_with("_test.py")
        || name.contains(".test.")
        || name.contains(".spec.")
    {
        return Role::Test;
    }
    if lower.split('/').any(|part| {
        matches!(
            part,
            "steward-notes" | "md-claude-chapters" | "md-chapters" | "reports" | "analyses"
        )
    }) || name.starts_with("changelog")
        || name.contains("ledger")
    {
        return Role::History;
    }
    if lower.split('/').any(|part| part == "docs")
        || matches!(
            path.extension().and_then(|s| s.to_str()),
            Some("md" | "txt")
        )
    {
        return Role::Documentation;
    }
    if matches!(
        path.extension().and_then(|s| s.to_str()),
        Some(
            "rs" | "py"
                | "js"
                | "ts"
                | "tsx"
                | "jsx"
                | "swift"
                | "c"
                | "h"
                | "cpp"
                | "hpp"
                | "sh"
                | "bash"
                | "zsh"
                | "metal"
                | "wgsl"
                | "glsl"
                | "html"
                | "css"
        )
    ) {
        Role::Implementation
    } else {
        Role::Configuration
    }
}

// Small deterministic spelling hints, never semantic or caller resolution.
fn spelling_distance(query: &str, word: &str) -> Option<usize> {
    if query == word
        || query.len() < 6
        || word.len().abs_diff(query.len()) > 3
        || !word.starts_with(query.get(..3)?)
        || (query.contains('_')
            && query.rsplit_once('_').map(|(_, suffix)| suffix)
                != word.rsplit_once('_').map(|(_, suffix)| suffix))
    {
        return None;
    }
    let mut previous = (0..=word.len()).collect::<Vec<_>>();
    for (i, a) in query.bytes().enumerate() {
        let mut current = vec![i.saturating_add(1)];
        for (j, b) in word.bytes().enumerate() {
            current.push(
                previous[j.saturating_add(1)]
                    .saturating_add(1)
                    .min(current[j].saturating_add(1))
                    .min(previous[j].saturating_add(usize::from(a != b))),
            );
        }
        previous = current;
    }
    previous.last().copied().filter(|distance| *distance <= 3)
}
