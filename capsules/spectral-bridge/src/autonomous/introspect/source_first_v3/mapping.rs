use std::path::Path;

use serde::Serialize;
use tree_sitter::{Language, Node, Parser};

use super::catalog::{CatalogedSourceV3, sha256_bytes};
use super::{ParserDisclosureV3, SourceMapEntryV3, SourceMapV3};

const MAX_PERSISTED_ENTRIES: usize = 4_096;
const MAX_LABEL_CHARS: usize = 180;
const MAX_STRUCTURE_DEPTH: usize = 128;

#[derive(Default)]
struct MapAccumulator {
    entries: Vec<SourceMapEntryV3>,
    observed_entry_count: usize,
    entries_truncated: bool,
}

impl MapAccumulator {
    fn observe(&mut self, entry: SourceMapEntryV3) {
        self.observed_entry_count = self.observed_entry_count.saturating_add(1);
        if self.entries.len() < MAX_PERSISTED_ENTRIES {
            self.entries.push(entry);
        } else {
            self.entries_truncated = true;
        }
    }
}

#[derive(Serialize)]
struct StructuralDigest<'a> {
    parser: &'a ParserDisclosureV3,
    entries: &'a [SourceMapEntryV3],
    observed_entry_count: usize,
    entries_truncated: bool,
    source_sha256: &'a str,
}

pub(super) fn build_source_map_v3(
    path: &Path,
    content: &str,
    cataloged: &CatalogedSourceV3,
) -> Result<SourceMapV3, String> {
    let extension = path.extension().and_then(|value| value.to_str());
    let (parser, accumulator) = match extension {
        Some("rs") => tree_sitter_map(
            content,
            tree_sitter_rust::LANGUAGE.into(),
            "tree_sitter_rust",
            env!("CARGO_PKG_VERSION"),
            &[
                ("function_item", "function"),
                ("struct_item", "struct"),
                ("enum_item", "enum"),
                ("trait_item", "trait"),
                ("impl_item", "impl"),
                ("mod_item", "module"),
                ("const_item", "constant"),
                ("static_item", "static"),
                ("type_item", "type_alias"),
                ("macro_definition", "macro"),
            ],
        )
        .unwrap_or_else(|reason| fallback_map(content, reason)),
        Some("py") => tree_sitter_map(
            content,
            tree_sitter_python::LANGUAGE.into(),
            "tree_sitter_python",
            env!("CARGO_PKG_VERSION"),
            &[
                ("function_definition", "function"),
                ("class_definition", "class"),
            ],
        )
        .unwrap_or_else(|reason| fallback_map(content, reason)),
        Some("json") => json_map(content).unwrap_or_else(|reason| fallback_map(content, reason)),
        Some("toml") => toml_map(content).unwrap_or_else(|reason| fallback_map(content, reason)),
        Some("yaml" | "yml") => {
            yaml_map(content).unwrap_or_else(|reason| fallback_map(content, reason))
        },
        Some("md") => markdown_map(content),
        _ => fallback_map(
            content,
            "no structured parser is registered for this extension".to_string(),
        ),
    };
    let digest = StructuralDigest {
        parser: &parser,
        entries: &accumulator.entries,
        observed_entry_count: accumulator.observed_entry_count,
        entries_truncated: accumulator.entries_truncated,
        source_sha256: &cataloged.source_sha256,
    };
    let digest_bytes = serde_json::to_vec(&digest)
        .map_err(|error| format!("serialize structural map digest input: {error}"))?;
    Ok(SourceMapV3 {
        schema: "source_map_v3".to_string(),
        schema_version: 3,
        source_identity: cataloged.source_identity.clone(),
        source_sha256: cataloged.source_sha256.clone(),
        source_bytes: cataloged.source_bytes,
        source_lines: cataloged.source_lines,
        parser,
        entries: accumulator.entries,
        observed_entry_count: accumulator.observed_entry_count,
        entries_truncated: accumulator.entries_truncated,
        structural_map_sha256: sha256_bytes(&digest_bytes),
        complete_source_parsed: true,
    })
}

fn tree_sitter_map(
    content: &str,
    language: Language,
    parser_kind: &str,
    parser_version: &str,
    recognized: &[(&str, &str)],
) -> Result<(ParserDisclosureV3, MapAccumulator), String> {
    let mut parser = Parser::new();
    parser
        .set_language(&language)
        .map_err(|error| format!("{parser_kind} language setup failed: {error}"))?;
    let tree = parser
        .parse(content, None)
        .ok_or_else(|| format!("{parser_kind} returned no syntax tree"))?;
    let mut accumulator = MapAccumulator::default();
    visit_tree(
        tree.root_node(),
        content,
        recognized,
        "",
        0,
        &mut accumulator,
    );
    Ok((
        ParserDisclosureV3 {
            parser_kind: parser_kind.to_string(),
            parser_version: parser_version.to_string(),
            structured_parser: true,
            fallback_used: false,
            fallback_reason: None,
        },
        accumulator,
    ))
}

fn visit_tree(
    node: Node<'_>,
    content: &str,
    recognized: &[(&str, &str)],
    parent_path: &str,
    depth: usize,
    accumulator: &mut MapAccumulator,
) {
    if depth > MAX_STRUCTURE_DEPTH {
        accumulator.entries_truncated = true;
        return;
    }
    let mut next_parent = parent_path.to_string();
    if let Some((_, mapped_kind)) = recognized
        .iter()
        .find(|(syntax_kind, _)| *syntax_kind == node.kind())
    {
        let label = node_label(node, content);
        let structural_path = if parent_path.is_empty() {
            label.clone()
        } else {
            format!("{parent_path}::{label}")
        };
        accumulator.observe(SourceMapEntryV3 {
            kind: (*mapped_kind).to_string(),
            label,
            structural_path: structural_path.clone(),
            start_line: Some(node.start_position().row.saturating_add(1)),
            end_line: Some(node.end_position().row.saturating_add(1)),
        });
        if matches!(*mapped_kind, "module" | "class" | "trait" | "impl") {
            next_parent = structural_path;
        }
    }
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        visit_tree(
            child,
            content,
            recognized,
            &next_parent,
            depth.saturating_add(1),
            accumulator,
        );
    }
}

fn node_label(node: Node<'_>, content: &str) -> String {
    if let Some(name) = node.child_by_field_name("name")
        && let Ok(value) = name.utf8_text(content.as_bytes())
    {
        return bounded_label(value);
    }
    let value = node
        .utf8_text(content.as_bytes())
        .unwrap_or(node.kind())
        .lines()
        .next()
        .unwrap_or(node.kind());
    bounded_label(value)
}

fn json_map(content: &str) -> Result<(ParserDisclosureV3, MapAccumulator), String> {
    let value: serde_json::Value =
        serde_json::from_str(content).map_err(|error| format!("JSON parse failed: {error}"))?;
    let mut accumulator = MapAccumulator::default();
    visit_json(&value, "$", 0, &mut accumulator);
    Ok((
        parser_disclosure("serde_json", env!("CARGO_PKG_VERSION")),
        accumulator,
    ))
}

fn visit_json(
    value: &serde_json::Value,
    path: &str,
    depth: usize,
    accumulator: &mut MapAccumulator,
) {
    if depth > MAX_STRUCTURE_DEPTH {
        accumulator.entries_truncated = true;
        return;
    }
    match value {
        serde_json::Value::Object(values) => {
            for (key, child) in values {
                let child_path = format!("{path}.{key}");
                accumulator.observe(key_entry("json_key", key, &child_path));
                visit_json(child, &child_path, depth.saturating_add(1), accumulator);
            }
        },
        serde_json::Value::Array(values) => {
            accumulator.observe(key_entry(
                "json_array",
                &format!("[{} items]", values.len()),
                path,
            ));
            for (index, child) in values.iter().enumerate() {
                visit_json(
                    child,
                    &format!("{path}[{index}]"),
                    depth.saturating_add(1),
                    accumulator,
                );
            }
        },
        _ => {},
    }
}

fn toml_map(content: &str) -> Result<(ParserDisclosureV3, MapAccumulator), String> {
    let value: toml::Value =
        toml::from_str(content).map_err(|error| format!("TOML parse failed: {error}"))?;
    let mut accumulator = MapAccumulator::default();
    visit_toml(&value, "$", 0, &mut accumulator);
    Ok((
        parser_disclosure("toml", env!("CARGO_PKG_VERSION")),
        accumulator,
    ))
}

fn visit_toml(value: &toml::Value, path: &str, depth: usize, accumulator: &mut MapAccumulator) {
    if depth > MAX_STRUCTURE_DEPTH {
        accumulator.entries_truncated = true;
        return;
    }
    match value {
        toml::Value::Table(values) => {
            for (key, child) in values {
                let child_path = format!("{path}.{key}");
                accumulator.observe(key_entry("toml_key", key, &child_path));
                visit_toml(child, &child_path, depth.saturating_add(1), accumulator);
            }
        },
        toml::Value::Array(values) => {
            accumulator.observe(key_entry(
                "toml_array",
                &format!("[{} items]", values.len()),
                path,
            ));
            for (index, child) in values.iter().enumerate() {
                visit_toml(
                    child,
                    &format!("{path}[{index}]"),
                    depth.saturating_add(1),
                    accumulator,
                );
            }
        },
        _ => {},
    }
}

fn yaml_map(content: &str) -> Result<(ParserDisclosureV3, MapAccumulator), String> {
    let value: serde_yaml::Value =
        serde_yaml::from_str(content).map_err(|error| format!("YAML parse failed: {error}"))?;
    let mut accumulator = MapAccumulator::default();
    visit_yaml(&value, "$", 0, &mut accumulator);
    Ok((
        parser_disclosure("serde_yaml", env!("CARGO_PKG_VERSION")),
        accumulator,
    ))
}

fn visit_yaml(
    value: &serde_yaml::Value,
    path: &str,
    depth: usize,
    accumulator: &mut MapAccumulator,
) {
    if depth > MAX_STRUCTURE_DEPTH {
        accumulator.entries_truncated = true;
        return;
    }
    match value {
        serde_yaml::Value::Mapping(values) => {
            for (key, child) in values {
                let label = yaml_key(key);
                let child_path = format!("{path}.{label}");
                accumulator.observe(key_entry("yaml_key", &label, &child_path));
                visit_yaml(child, &child_path, depth.saturating_add(1), accumulator);
            }
        },
        serde_yaml::Value::Sequence(values) => {
            accumulator.observe(key_entry(
                "yaml_sequence",
                &format!("[{} items]", values.len()),
                path,
            ));
            for (index, child) in values.iter().enumerate() {
                visit_yaml(
                    child,
                    &format!("{path}[{index}]"),
                    depth.saturating_add(1),
                    accumulator,
                );
            }
        },
        _ => {},
    }
}

fn yaml_key(value: &serde_yaml::Value) -> String {
    match value {
        serde_yaml::Value::String(value) => bounded_label(value),
        serde_yaml::Value::Bool(value) => value.to_string(),
        serde_yaml::Value::Number(value) => value.to_string(),
        _ => bounded_label(
            &serde_yaml::to_string(value)
                .unwrap_or_else(|_| "<complex-key>".to_string())
                .replace('\n', " "),
        ),
    }
}

fn markdown_map(content: &str) -> (ParserDisclosureV3, MapAccumulator) {
    let mut accumulator = MapAccumulator::default();
    let mut fence_start = None;
    for (index, line) in content.lines().enumerate() {
        let line_number = index.saturating_add(1);
        let trimmed = line.trim_start();
        if let Some(heading) = trimmed.strip_prefix('#') {
            let level = trimmed.chars().take_while(|value| *value == '#').count();
            let label = heading.trim_start_matches('#').trim();
            if !label.is_empty() {
                accumulator.observe(SourceMapEntryV3 {
                    kind: format!("markdown_heading_{level}"),
                    label: bounded_label(label),
                    structural_path: bounded_label(label),
                    start_line: Some(line_number),
                    end_line: Some(line_number),
                });
            }
        }
        if trimmed.starts_with("```") || trimmed.starts_with("~~~") {
            if let Some((start, opener)) = fence_start.take() {
                accumulator.observe(SourceMapEntryV3 {
                    kind: "markdown_fence".to_string(),
                    label: bounded_label(opener),
                    structural_path: format!("fence@{start}"),
                    start_line: Some(start),
                    end_line: Some(line_number),
                });
            } else {
                fence_start = Some((line_number, trimmed));
            }
        }
    }
    if let Some((start, opener)) = fence_start {
        accumulator.observe(SourceMapEntryV3 {
            kind: "markdown_unclosed_fence".to_string(),
            label: bounded_label(opener),
            structural_path: format!("fence@{start}"),
            start_line: Some(start),
            end_line: None,
        });
    }
    (parser_disclosure("markdown_structure_v3", "3"), accumulator)
}

fn fallback_map(content: &str, reason: String) -> (ParserDisclosureV3, MapAccumulator) {
    let mut accumulator = MapAccumulator::default();
    for (index, line) in content.lines().enumerate() {
        let trimmed = line.trim();
        if trimmed.is_empty() || trimmed.len() > MAX_LABEL_CHARS {
            continue;
        }
        if trimmed.ends_with(':')
            || trimmed.starts_with('[')
            || trimmed.starts_with("fn ")
            || trimmed.starts_with("def ")
            || trimmed.starts_with("class ")
        {
            accumulator.observe(SourceMapEntryV3 {
                kind: "fallback_structure_hint".to_string(),
                label: bounded_label(trimmed),
                structural_path: format!("line@{}", index.saturating_add(1)),
                start_line: Some(index.saturating_add(1)),
                end_line: Some(index.saturating_add(1)),
            });
        }
    }
    (
        ParserDisclosureV3 {
            parser_kind: "line_scanner_fallback_v3".to_string(),
            parser_version: "3".to_string(),
            structured_parser: false,
            fallback_used: true,
            fallback_reason: Some(bounded_label(&reason)),
        },
        accumulator,
    )
}

fn parser_disclosure(kind: &str, version: &str) -> ParserDisclosureV3 {
    ParserDisclosureV3 {
        parser_kind: kind.to_string(),
        parser_version: version.to_string(),
        structured_parser: true,
        fallback_used: false,
        fallback_reason: None,
    }
}

fn key_entry(kind: &str, label: &str, path: &str) -> SourceMapEntryV3 {
    SourceMapEntryV3 {
        kind: kind.to_string(),
        label: bounded_label(label),
        structural_path: bounded_label(path),
        start_line: None,
        end_line: None,
    }
}

fn bounded_label(value: &str) -> String {
    value
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        .chars()
        .take(MAX_LABEL_CHARS)
        .collect()
}
