//! Bounded syntax context from the exact source revision, never runtime evidence.
//!
//! Reuses the bridge's tree-sitter grammars. Parsing is deliberately conservative:
//! an erroneous/incomplete tree produces no declarations, and macro token trees,
//! strings, and comments never become definition/reference candidates.
use std::{fmt::Write as _, path::Path};
use tree_sitter::{Node, Parser};

const MAX_PARSE_BYTES: usize = 2 * 1024 * 1024;
const MAX_NODES: usize = 150_000;
const MAX_DECLARATIONS: usize = 4_096;
const MAX_REFERENCES: usize = 16_384;
const MAX_DEPTH: usize = 128;

#[derive(Clone, Debug)]
pub(crate) struct Declaration {
    pub name: String,
    pub qualified_name: String,
    pub kind: &'static str,
    pub start_byte: usize,
    pub end_byte: usize,
    pub start_line: usize,
    pub end_line: usize,
    pub test_context: Option<String>,
    pub name_start_byte: usize,
    pub name_end_byte: usize,
    pub name_line: usize,
    pub review_only_line: Option<usize>,
}

#[derive(Clone, Debug)]
pub(crate) struct Reference {
    pub name: String,
    pub start_byte: usize,
    pub end_byte: usize,
    pub line: usize,
    pub usage: &'static str,
    pub test_context: bool,
}

struct Region {
    start: usize,
    end: usize,
    label: &'static str,
}

pub(crate) struct Outline {
    pub status: &'static str,
    pub complete: bool,
    pub parser: Option<&'static str>,
    pub declarations: Vec<Declaration>,
    pub references: Vec<Reference>,
    regions: Vec<Region>,
    visited: usize,
}

impl Outline {
    pub(crate) fn parse(source_id: &str, text: &str) -> Self {
        let mut outline = Self {
            status: "unsupported source language; enclosing scope unknown",
            complete: false,
            parser: None,
            declarations: Vec::new(),
            references: Vec::new(),
            regions: Vec::new(),
            visited: 0,
        };
        let (language, parser_name) =
            match Path::new(source_id).extension().and_then(|e| e.to_str()) {
                Some("rs") => (tree_sitter_rust::LANGUAGE.into(), "tree-sitter-rust"),
                Some("py") => (tree_sitter_python::LANGUAGE.into(), "tree-sitter-python"),
                _ => return outline,
            };
        outline.parser = Some(parser_name);
        if text.len() > MAX_PARSE_BYTES {
            outline.status = "2 MiB syntax inspection limit; enclosing scope unknown";
            return outline;
        }
        let mut parser = Parser::new();
        if parser.set_language(&language).is_err() {
            outline.status = "syntax parser unavailable; enclosing scope unknown";
            return outline;
        }
        let Some(tree) = parser.parse(text, None) else {
            outline.status = "syntax parser returned no tree; enclosing scope unknown";
            return outline;
        };
        if tree.root_node().has_error() {
            outline.status = "syntax parse contains errors; enclosing scope unknown";
            return outline;
        }
        outline.complete = true;
        let path_context = test_path_context(source_id);
        outline.visit(tree.root_node(), text, "", path_context.as_deref(), 0);
        if outline.complete {
            outline.status = "parsed syntax; macros unexpanded and runtime use unverified";
        } else {
            outline.status = "syntax traversal limit; enclosing scope unknown";
            outline.declarations.clear();
            outline.references.clear();
            outline.regions.clear();
        }
        outline
    }

    pub(crate) fn enclosing(&self, byte: usize) -> Vec<&Declaration> {
        let mut scopes: Vec<_> = self
            .declarations
            .iter()
            .filter(|d| d.start_byte <= byte && byte < d.end_byte)
            .collect();
        scopes.sort_by_key(|d| (d.start_byte, std::cmp::Reverse(d.end_byte)));
        scopes
    }

    /// Metadata lies outside the numbered byte interval; commands are atomic.
    pub(crate) fn scope_text(
        &self,
        source_id: &str,
        text: &str,
        byte: usize,
        budget: usize,
    ) -> String {
        let mut output = format!(
            "SOURCE SCOPE at page start — {}; origin: same SOURCE and sha256 above. Syntax only; runtime behavior unverified.\n",
            self.parser.unwrap_or("no syntax parser")
        );
        if !self.complete {
            output.push_str(self.status);
            output.push_str(". Source remains readable; no declaration guessed.\n");
            return output;
        }
        // An OPEN at an indented declaration still belongs to that declaration.
        let indent = text[byte..]
            .chars()
            .take_while(|c| matches!(c, ' ' | '\t' | '\r'))
            .map(char::len_utf8)
            .sum::<usize>();
        let anchor = byte.saturating_add(indent);
        let scopes = self.enclosing(anchor);
        if scopes.is_empty() {
            output.push_str("No enclosing declaration at this position (file-level syntax). Runtime use is not established.\n");
        } else {
            for declaration in scopes.iter().rev().take(2) {
                let name = bounded_label(&declaration.qualified_name, 80);
                let test_context = declaration
                    .test_context
                    .as_deref()
                    .unwrap_or("no test marker found");
                let row = format!(
                    "{} {} (lines {}–{}; {}). Enclosing declaration: SELF_STUDY OPEN {} {}\n",
                    declaration.kind,
                    name,
                    declaration.start_line,
                    declaration.end_line,
                    test_context,
                    source_id,
                    declaration.start_line
                );
                if output.len().saturating_add(row.len()).saturating_add(100) <= budget {
                    output.push_str(&row);
                } else {
                    output
                        .push_str("Additional enclosing declaration omitted by metadata budget.\n");
                    break;
                }
            }
        }
        if let Some(region) = self
            .regions
            .iter()
            .find(|r| r.start <= anchor && anchor < r.end)
        {
            let _ = writeln!(
                output,
                "Page begins inside {}; text there is not a declaration.",
                region.label
            );
        }
        output
    }

    /// A delivered page can stop inside a different declaration from the one
    /// named at its start. Its last line is not that declaration's closing line.
    pub(crate) fn page_end_text(&self, start: usize, end: usize, budget: usize) -> String {
        if !self.complete || start == end {
            return String::new();
        }
        let scopes = self.enclosing(end.saturating_sub(1));
        let Some(declaration) = scopes
            .last()
            .filter(|declaration| end < declaration.end_byte)
        else {
            return String::new();
        };
        let row = format!(
            "\nPAGE END SCOPE — {} {} continues beyond this page (declaration lines {}–{}). Syntax span is metadata, not delivered coverage.\n",
            declaration.kind,
            bounded_label(&declaration.qualified_name, 80),
            declaration.start_line,
            declaration.end_line
        );
        if row.len() <= budget {
            row
        } else {
            "\nPAGE END SCOPE — enclosing declaration continues beyond this page; its closing source is not delivered here.\n".into()
        }
    }

    fn visit(
        &mut self,
        node: Node<'_>,
        text: &str,
        parent: &str,
        inherited_context: Option<&str>,
        depth: usize,
    ) {
        if !self.complete {
            return;
        }
        self.visited = self.visited.saturating_add(1);
        if depth > MAX_DEPTH || self.visited > MAX_NODES {
            self.complete = false;
            return;
        }
        if let Some(label) = opaque_region(node.kind()) {
            self.regions.push(Region {
                start: node.start_byte(),
                end: node.end_byte(),
                label: if label == "a comment"
                    && node
                        .utf8_text(text.as_bytes())
                        .unwrap_or_default()
                        .contains("Historical being report")
                {
                    "historical being-report commentary (attributed earlier account)"
                } else {
                    label
                },
            });
            return;
        }
        let mut qualified = parent.to_owned();
        let test_marker = rust_test_marker(node, text).or_else(|| python_test_marker(node, text));
        let test_context = test_marker.as_deref().or(inherited_context);
        if let Some(kind) = declaration_kind(node.kind()) {
            if self.declarations.len() >= MAX_DECLARATIONS {
                self.complete = false;
                return;
            }
            let name = declaration_name(node, text);
            let full_name = if parent.is_empty() {
                name.clone()
            } else {
                format!("{parent}::{name}")
            };
            // This path is a display label, never a symbol identity. Bound it
            // at every nesting level so deeply nested names cannot amplify
            // the parsed source into an unbounded metadata allocation.
            qualified = bounded_label(&full_name, 256);
            self.declarations.push(Declaration {
                name,
                qualified_name: qualified.clone(),
                kind,
                start_byte: node.start_byte(),
                end_byte: node.end_byte(),
                start_line: node.start_position().row.saturating_add(1),
                end_line: node
                    .end_position()
                    .row
                    .saturating_add(usize::from(node.end_position().column > 0))
                    .max(1),
                test_context: test_context.map(str::to_owned),
                name_start_byte: node
                    .child_by_field_name("name")
                    .map_or(node.start_byte(), |n| n.start_byte()),
                name_end_byte: node
                    .child_by_field_name("name")
                    .map_or(node.start_byte(), |n| n.end_byte()),
                name_line: node
                    .child_by_field_name("name")
                    .map_or(node.start_position().row, |n| n.start_position().row)
                    .saturating_add(1),
                review_only_line: review_only_marker(node, text),
            });
        } else if is_reference(node) {
            if self.references.len() >= MAX_REFERENCES {
                self.complete = false;
                return;
            }
            self.references.push(Reference {
                name: node
                    .utf8_text(text.as_bytes())
                    .unwrap_or_default()
                    .to_owned(),
                start_byte: node.start_byte(),
                end_byte: node.end_byte(),
                line: node.start_position().row.saturating_add(1),
                usage: reference_usage(node),
                test_context: test_context.is_some(),
            });
        }
        let mut cursor = node.walk();
        for child in node.named_children(&mut cursor) {
            self.visit(
                child,
                text,
                &qualified,
                test_context,
                depth.saturating_add(1),
            );
        }
    }
}

fn review_only_marker(node: Node<'_>, text: &str) -> Option<usize> {
    let mut previous = node.prev_named_sibling();
    for _ in 0..32 {
        let item = previous?;
        if !matches!(
            item.kind(),
            "line_comment" | "block_comment" | "comment" | "attribute_item"
        ) {
            break;
        }
        if matches!(item.kind(), "line_comment" | "block_comment" | "comment")
            && item
                .utf8_text(text.as_bytes())
                .ok()?
                .contains("Source-study scope: review-only.")
        {
            return Some(item.start_position().row.saturating_add(1));
        }
        previous = item.prev_named_sibling();
    }
    None
}

fn reference_usage(node: Node<'_>) -> &'static str {
    let mut ancestor = node.parent();
    for _ in 0..12 {
        let Some(parent) = ancestor else { break };
        if matches!(parent.kind(), "call_expression" | "call") {
            return if parent
                .child_by_field_name("function")
                .is_some_and(|callee| {
                    callee.start_byte() <= node.start_byte() && node.end_byte() <= callee.end_byte()
                }) {
                "call"
            } else {
                "reference"
            };
        }
        if parent.kind() == "match_arm" {
            return if parent
                .child_by_field_name("pattern")
                .is_some_and(|pattern| {
                    pattern.start_byte() <= node.start_byte()
                        && node.end_byte() <= pattern.end_byte()
                }) {
                "match"
            } else {
                "reference"
            };
        }
        if declaration_kind(parent.kind()).is_some() {
            break;
        }
        ancestor = parent.parent();
    }
    "reference"
}

fn declaration_kind(kind: &str) -> Option<&'static str> {
    match kind {
        "function_item" | "function_signature_item" | "function_definition" => Some("function"),
        "struct_item" => Some("struct"),
        "enum_item" => Some("enum"),
        "trait_item" => Some("trait"),
        "impl_item" => Some("impl"),
        "mod_item" => Some("module"),
        "const_item" => Some("constant"),
        "static_item" => Some("static"),
        "type_item" | "associated_type" => Some("type"),
        "class_definition" => Some("class"),
        _ => None,
    }
}

fn declaration_name(node: Node<'_>, text: &str) -> String {
    if node.kind() == "impl_item" {
        let target = node
            .child_by_field_name("type")
            .and_then(|n| n.utf8_text(text.as_bytes()).ok())
            .unwrap_or("<unknown type>");
        return node
            .child_by_field_name("trait")
            .and_then(|n| n.utf8_text(text.as_bytes()).ok())
            .map_or_else(
                || format!("impl {target}"),
                |name| format!("impl {name} for {target}"),
            );
    }
    node.child_by_field_name("name")
        .and_then(|n| n.utf8_text(text.as_bytes()).ok())
        .unwrap_or("<unnamed>")
        .to_owned()
}

fn opaque_region(kind: &str) -> Option<&'static str> {
    match kind {
        "line_comment" | "block_comment" | "comment" => Some("a comment"),
        "string_literal"
        | "raw_string_literal"
        | "char_literal"
        | "string"
        | "concatenated_string" => Some("a string/character literal"),
        "macro_invocation"
        | "macro_definition"
        | "token_tree"
        | "attribute_item"
        | "inner_attribute_item"
        | "decorator" => Some("unexpanded macro/attribute syntax"),
        _ => None,
    }
}

fn is_reference(node: Node<'_>) -> bool {
    if !matches!(
        node.kind(),
        "identifier" | "type_identifier" | "field_identifier"
    ) {
        return false;
    }
    !node.parent().is_some_and(|parent| {
        declaration_kind(parent.kind()).is_some()
            && parent
                .child_by_field_name("name")
                .is_some_and(|name| name.id() == node.id())
    })
}

fn rust_test_marker(node: Node<'_>, text: &str) -> Option<String> {
    declaration_kind(node.kind())?;
    let mut previous = node.prev_named_sibling();
    while let Some(attribute) = previous {
        if matches!(attribute.kind(), "line_comment" | "block_comment") {
            previous = attribute.prev_named_sibling();
            continue;
        }
        if attribute.kind() != "attribute_item" {
            break;
        }
        let mut value = String::new();
        if !attribute_tokens(attribute, text, &mut value, 0) {
            previous = attribute.prev_named_sibling();
            continue;
        }
        if matches!(value.as_str(), "#[test]" | "#[cfg(test)]") {
            return Some(format!(
                "test syntax: {value} at line {}",
                attribute.start_position().row.saturating_add(1)
            ));
        }
        previous = attribute.prev_named_sibling();
    }
    None
}

fn attribute_tokens(node: Node<'_>, text: &str, output: &mut String, depth: usize) -> bool {
    if depth > MAX_DEPTH || output.len() > 1_024 {
        return false;
    }
    if matches!(node.kind(), "line_comment" | "block_comment") {
        return true;
    }
    if node.child_count() == 0 {
        if let Ok(token) = node.utf8_text(text.as_bytes()) {
            output.push_str(token);
        }
        return true;
    }
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        if !attribute_tokens(child, text, output, depth.saturating_add(1)) {
            return false;
        }
    }
    true
}

fn python_test_marker(node: Node<'_>, text: &str) -> Option<String> {
    if node.kind() != "function_definition" {
        return None;
    }
    let name = node
        .child_by_field_name("name")?
        .utf8_text(text.as_bytes())
        .ok()?;
    name.starts_with("test_")
        .then(|| "test naming convention; collection/execution unverified".into())
}

fn test_path_context(source_id: &str) -> Option<String> {
    source_id
        .split('/')
        .any(|part| matches!(part, "tests" | "test" | "fixtures"))
        .then(|| "test/fixture path; execution unverified".into())
}

fn bounded_label(value: &str, limit: usize) -> String {
    let mut chars = value.chars().map(|c| if c.is_control() { ' ' } else { c });
    let mut label: String = chars.by_ref().take(limit).collect();
    if chars.next().is_some() {
        label.push('…');
    }
    label
}
