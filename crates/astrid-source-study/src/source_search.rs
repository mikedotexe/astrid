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

    pub(crate) fn label(self) -> &'static str {
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
    definitions: Vec<(String, usize, String)>,
    consumers: Vec<(String, CallContext)>,
}

impl SearchReport {
    pub(crate) fn comparison(&self) -> String {
        let [(source, line, name)] = self.definitions.as_slice() else {
            return if self.definitions.len() > 1 {
                "Multiple parsed definition candidates; ambiguity unresolved. No caller/definition comparison selected. Inspect the qualified contexts at the exact locations below.\n\n".into()
            } else {
                String::new()
            };
        };
        if self.bounded || !valid_location(source, *line) {
            return String::new();
        }
        let distinct = |(path, call): &&(String, CallContext)| {
            (path != source || call.line != *line) && valid_location(path, call.line)
        };
        let Some((caller_source, call)) = self
            .consumers
            .iter()
            .filter(distinct)
            .find(|(path, _)| path == source)
            .or_else(|| self.consumers.iter().find(distinct))
        else {
            return String::new();
        };
        let mut targets = vec![(source.clone(), *line), (caller_source.clone(), call.line)];
        if let Some(condition) = call.condition_line
            && !targets.contains(&(caller_source.clone(), condition))
        {
            targets.push((caller_source.clone(), condition));
        }
        let Some(session) = comparison_session(&targets) else {
            return String::new();
        };
        format!(
            "Optional caller/definition comparison (syntax candidates, binding unverified; ambiguity unresolved): {session}\nDefinition syntax: {}. Callee syntax: {}.{}\nInspect arguments, assignments and conditions: return, retry, skipped cleanup, and log-and-continue differ. These locations are candidates, not verified call/data flow, reachability, deployment or causality. No verdict has been inferred; selecting SESSION moves the reading position to its last page.\n\n",
            context_label(name),
            context_label(&call.expression),
            if targets.len() == 3 {
                " The third location is enclosing branch/loop syntax, not a verified execution condition."
            } else {
                ""
            }
        )
    }
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
        if exact && crate::relationships::qualified_symbol(query) {
            self.collect_qualified_lines(source, role, text, query);
            return;
        }
        let rust = source.path.extension().is_some_and(|ext| ext == "rs");
        let outline = (exact && role == Role::Implementation && text.contains(query))
            .then(|| crate::source_structure::Outline::parse(&source.id, text));
        let calls = outline.as_ref().map_or_else(BTreeMap::new, |outline| {
            call_contexts(&source.id, text, outline)
        });
        if let Some(outline) = &outline {
            self.collect_comparison(&source.id, query, outline, &calls);
        }
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
            let consumer = outline.as_ref().is_some_and(|outline| {
                outline.complete
                    && outline.references.iter().any(|reference| {
                        reference.line == number
                            && reference.name == query
                            && !reference.test_context
                            && (reference.usage == "match"
                                || calls.contains_key(&reference.start_byte))
                    })
            });
            let kind = if consumer && role == Role::Implementation {
                3
            } else if exact
                && words.windows(2).any(|w| {
                    matches!(
                        w[0],
                        "fn" | "struct" | "enum" | "trait" | "type" | "class" | "def" | "function"
                    ) && w[1] == query
                })
            {
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
            let context = outline.as_ref().map_or_else(String::new, |outline| {
                result_context(outline, &calls, query, number)
            });
            self.rows.entry((role, kind)).or_default().push(format!(
                "[{}] SELF_STUDY OPEN {} {number} — line {number}: {}{context}",
                role.label(),
                source.id,
                line_excerpt(line, query)
            ));
            self.hits = self.hits.saturating_add(1);
            if self.hits >= MAX_HITS {
                self.bounded = true;
                break;
            }
        }
    }

    fn collect_qualified_lines(
        &mut self,
        source: &crate::Source,
        role: Role,
        text: &str,
        query: &str,
    ) {
        let rust = source.path.extension().is_some_and(|ext| ext == "rs");
        let outline = (role == Role::Implementation
            && text.contains(crate::relationships::terminal_symbol(query)))
        .then(|| crate::source_structure::Outline::parse(&source.id, text));
        let mut candidates = BTreeMap::new();
        if let Some(outline) = &outline {
            let calls = call_contexts(&source.id, text, outline);
            self.collect_comparison(&source.id, query, outline, &calls);
            for declaration in &outline.declarations {
                if declaration.test_context.is_none()
                    && let Some(rank) = definition_rank(&source.id, query, declaration, outline)
                {
                    candidates.insert(
                        declaration.name_line,
                        (
                            if rank == 0 { 0 } else { 5 },
                            format!(
                                "declaration {}",
                                context_label(
                                    &declaration_path(&source.id, declaration, outline)
                                        .unwrap_or_else(|| declaration.qualified_name.clone())
                                )
                            ),
                        ),
                    );
                }
            }
            for reference in &outline.references {
                if !reference.test_context
                    && let Some(call) = calls.get(&reference.start_byte)
                    && let Some(rank) = call_rank(query, call)
                {
                    let kind = if rank == 0 { 3 } else { 4 };
                    if candidates.get(&reference.line).is_none_or(|(previous, _)| {
                        result_priority(kind) < result_priority(*previous)
                    }) {
                        candidates.insert(
                            reference.line,
                            (kind, format!("callee {}", context_label(&call.expression))),
                        );
                    }
                }
            }
        }
        let mut test_remainder = role == Role::Test;
        for (offset, line) in text.lines().enumerate() {
            if rust && line.trim_start().starts_with("#[cfg(test)]") {
                test_remainder = true;
            }
            let number = offset.saturating_add(1);
            let candidate = candidates.get(&number);
            if candidate.is_none() && !literal_path_match(line, query) {
                continue;
            }
            let role = if test_remainder { Role::Test } else { role };
            let count = self.counts.entry(role).or_default();
            count.lines = count.lines.saturating_add(1);
            count.rust_lines = count.rust_lines.saturating_add(usize::from(rust));
            let (kind, context) = candidate.map_or((2, String::new()), |(kind, label)| {
                (
                    *kind,
                    format!(" [syntax context: {label}; binding unresolved]"),
                )
            });
            self.rows.entry((role, kind)).or_default().push(format!(
                "[{}] SELF_STUDY OPEN {} {number} — line {number}: {}{context}",
                role.label(),
                source.id,
                line_excerpt(line, crate::relationships::terminal_symbol(query)),
            ));
            self.hits = self.hits.saturating_add(1);
            if self.hits >= MAX_HITS {
                self.bounded = true;
                break;
            }
        }
    }

    fn collect_comparison(
        &mut self,
        source: &str,
        query: &str,
        outline: &crate::source_structure::Outline,
        calls: &BTreeMap<usize, CallContext>,
    ) {
        if !outline.complete {
            return;
        }
        for definition in &outline.declarations {
            if definition_rank(source, query, definition, outline).is_some()
                && definition.test_context.is_none()
                && self.definitions.len() < 17
            {
                self.definitions.push((
                    source.into(),
                    definition.start_line,
                    if crate::relationships::qualified_symbol(query) {
                        declaration_path(source, definition, outline)
                            .unwrap_or_else(|| definition.qualified_name.clone())
                    } else {
                        definition.qualified_name.clone()
                    },
                ));
            }
        }
        for reference in &outline.references {
            if reference.name == crate::relationships::terminal_symbol(query)
                && !reference.test_context
                && reference.usage == "call"
                && self.consumers.len() < 16
                && let Some(call) = calls.get(&reference.start_byte)
                && call_rank(query, call).is_some()
            {
                self.consumers.push((source.into(), call.clone()));
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
        let mut groups = self.rows.into_iter().collect::<Vec<_>>();
        groups.sort_by_key(|((role, kind), _)| (*role, result_priority(*kind)));
        let mut additional_consumers = Vec::new();
        for ((role, kind), mut rows) in groups {
            let label = match (role, kind) {
                (Role::Implementation, 3 | 4) => {
                    "Call / match sites (parsed syntax; name binding and execution unverified)"
                },
                (Role::Implementation, 0 | 5) => {
                    "Definition candidates (declaration-like text; inspect the surrounding source)"
                },
                (Role::Implementation, 1) => "Implementation blocks (lexical candidates)",
                (Role::Test, _) => {
                    "Test occurrences (including fixture strings; purpose requires surrounding source)"
                },
                _ => "Other references",
            };
            // Keep a bounded use-site preview ahead of definitions. A heavily
            // used symbol must not push every definition off the first page.
            if role == Role::Implementation && matches!(kind, 3 | 4) && rows.len() > 3 {
                additional_consumers.extend(rows.split_off(3));
            }
            lines.push(format!("{} — {label}", role.label()));
            lines.extend(rows);
        }
        if !additional_consumers.is_empty() {
            lines.push("Other references — additional parsed call / match sites (name binding and execution unverified)".into());
            lines.extend(additional_consumers);
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
            definitions: Vec::new(),
            consumers: Vec::new(),
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

#[derive(Clone)]
pub(crate) struct CallContext {
    pub expression: String,
    pub line: usize,
    pub start_byte: usize,
    pub end_byte: usize,
    condition_line: Option<usize>,
}

/// Retain callee syntax, not just each identifier inside it. The existing
/// outline gates language, parse size, traversal limits and opaque regions.
pub(crate) fn call_contexts(
    source: &str,
    text: &str,
    outline: &crate::source_structure::Outline,
) -> BTreeMap<usize, CallContext> {
    let mut calls = BTreeMap::new();
    if !outline.complete {
        return calls;
    }
    let language = match Path::new(source).extension().and_then(|ext| ext.to_str()) {
        Some("rs") => tree_sitter_rust::LANGUAGE.into(),
        Some("py") => tree_sitter_python::LANGUAGE.into(),
        _ => return calls,
    };
    let mut parser = tree_sitter::Parser::new();
    if parser.set_language(&language).is_err() {
        return calls;
    }
    let Some(tree) = parser.parse(text, None) else {
        return calls;
    };
    for reference in outline.references.iter().filter(|r| r.usage == "call") {
        let Some(mut node) = tree
            .root_node()
            .named_descendant_for_byte_range(reference.start_byte, reference.end_byte)
        else {
            continue;
        };
        for _ in 0..12 {
            let Some(parent) = node.parent() else { break };
            node = parent;
            if !matches!(node.kind(), "call_expression" | "call") {
                continue;
            }
            if let Some(callee) = node.child_by_field_name("function") {
                let mut terminal = callee;
                for _ in 0..12 {
                    let field = match terminal.kind() {
                        "generic_function" => "function",
                        "field_expression" => "field",
                        "attribute" => "attribute",
                        "scoped_identifier" => "name",
                        _ => break,
                    };
                    let Some(child) = terminal.child_by_field_name(field) else {
                        break;
                    };
                    terminal = child;
                }
                // Receiver/path components are context, not separate callees.
                // Oversized expressions remain reachable via ordinary results.
                if terminal.start_byte() == reference.start_byte
                    && terminal.end_byte() == reference.end_byte
                    && callee.end_byte().saturating_sub(callee.start_byte()) <= 512
                {
                    calls.insert(
                        reference.start_byte,
                        CallContext {
                            expression: text[callee.byte_range()].into(),
                            line: callee.start_position().row.saturating_add(1),
                            start_byte: callee.start_byte(),
                            end_byte: callee.end_byte(),
                            condition_line: enclosing_condition(node),
                        },
                    );
                }
            }
            break;
        }
    }
    calls
}

fn enclosing_condition(mut node: tree_sitter::Node<'_>) -> Option<usize> {
    for _ in 0..128 {
        node = node.parent()?;
        match node.kind() {
            "if_expression" | "if_statement" | "elif_clause" | "while_expression"
            | "while_statement" | "for_expression" | "for_statement" | "match_expression"
            | "match_statement" | "match_arm" => {
                return Some(node.start_position().row.saturating_add(1));
            },
            "function_item" | "function_definition" | "closure_expression" | "lambda" => break,
            _ => {},
        }
    }
    None
}

pub(crate) fn context_label(value: &str) -> String {
    let mut chars = value.chars().map(|c| if c.is_control() { ' ' } else { c });
    let mut label: String = chars.by_ref().take(160).collect();
    if chars.next().is_some() {
        label.push_str(" [truncated; use OPEN for full syntax]");
    }
    label
}

pub(crate) fn valid_location(source: &str, line: usize) -> bool {
    source.len() <= 512 && !source.chars().any(char::is_control) && line > 0
}

fn comparison_session(targets: &[(String, usize)]) -> Option<String> {
    if targets
        .iter()
        .any(|(source, line)| !valid_location(source, *line))
    {
        return None;
    }
    let command = format!(
        "SELF_STUDY SESSION {}",
        targets
            .iter()
            .map(|(source, line)| format!("OPEN {source} {line}"))
            .collect::<Vec<_>>()
            .join(" | ")
    );
    matches!(crate::Command::parse(&command), Ok(crate::Command::Session { targets: parsed }) if parsed == targets)
        .then_some(command)
}

fn result_context(
    outline: &crate::source_structure::Outline,
    calls: &BTreeMap<usize, CallContext>,
    query: &str,
    line: usize,
) -> String {
    let labels = outline
        .references
        .iter()
        .filter(|r| r.name == query && r.line == line)
        .filter_map(|r| {
            calls
                .get(&r.start_byte)
                .map(|call| format!("callee {}", context_label(&call.expression)))
        })
        .chain(
            outline
                .declarations
                .iter()
                .filter(|d| d.name == query && d.name_line == line)
                .map(|d| format!("declaration {}", context_label(&d.qualified_name))),
        )
        .take(2)
        .collect::<Vec<_>>();
    if labels.is_empty() {
        String::new()
    } else {
        format!(
            " [syntax context: {}; binding unresolved]",
            labels.join("; ")
        )
    }
}

fn result_priority(kind: u8) -> u8 {
    match kind {
        3 => 0,
        0 => 1,
        4 => 2,
        5 => 3,
        1 => 4,
        _ => 5,
    }
}

fn line_excerpt<'a>(line: &'a str, query: &str) -> &'a str {
    let at = line.find(query).unwrap_or(0);
    let from = line.floor_char_boundary(at.saturating_sub(50));
    let to = line.floor_char_boundary(
        at.saturating_add(query.len())
            .saturating_add(120)
            .min(line.len()),
    );
    &line[from..to]
}

fn literal_path_match(line: &str, query: &str) -> bool {
    line.split(|c: char| !c.is_ascii_alphanumeric() && !matches!(c, '_' | ':' | '.'))
        .any(|path| path == query)
}

fn path_rank(query: &str, candidate: &str, suffix: bool) -> Option<u8> {
    if candidate == query {
        return Some(0);
    }
    let separator = if query.contains("::") { "::" } else { "." };
    (suffix && candidate.ends_with(&format!("{separator}{query}"))).then_some(1)
}

fn call_rank(query: &str, call: &CallContext) -> Option<u8> {
    if !crate::relationships::qualified_symbol(query) {
        return Some(0);
    }
    let path: String = call
        .expression
        .chars()
        .filter(|c| !c.is_ascii_whitespace())
        .collect();
    crate::relationships::valid_symbol(&path)
        .then(|| path_rank(query, &path, query.contains("::")))
        .flatten()
}

fn definition_rank(
    source: &str,
    query: &str,
    declaration: &crate::source_structure::Declaration,
    outline: &crate::source_structure::Outline,
) -> Option<u8> {
    if !crate::relationships::qualified_symbol(query) {
        return (declaration.name == query).then_some(0);
    }
    let path = declaration_path(source, declaration, outline)?;
    path_rank(query, &path, true)
}

/// A path assembled only from explicit enclosing syntax. Do not substitute
/// receiver types, Self, imports, traits-for-types or generic impl targets.
pub(crate) fn declaration_path(
    source: &str,
    declaration: &crate::source_structure::Declaration,
    outline: &crate::source_structure::Outline,
) -> Option<String> {
    let separator = match Path::new(source).extension().and_then(|ext| ext.to_str()) {
        Some("rs") => "::",
        Some("py") => ".",
        _ => return None,
    };
    let mut parts = Vec::new();
    for scope in outline.enclosing(declaration.name_start_byte) {
        let name = if scope.kind == "impl" {
            scope.name.strip_prefix("impl ")?
        } else {
            &scope.name
        };
        if !crate::relationships::valid_symbol(name)
            || (separator == "::" && name.contains('.'))
            || (separator == "." && name.contains(':'))
        {
            return None;
        }
        parts.push(name);
    }
    let path = parts.join(separator);
    crate::relationships::valid_symbol(&path).then_some(path)
}

pub(crate) fn path_role(id: &str) -> Role {
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
