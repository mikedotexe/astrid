//! Same-revision evidence, never a claim about deployment or symbol resolution.
use crate::source_structure::Outline;
use std::fmt::Write as _;

pub(crate) fn render(outline: &Outline, source: &str, byte: usize, budget: usize) -> String {
    let mut out = String::from("SOURCE USE EVIDENCE — current file only; runtime use unknown.\n");
    if !outline.complete {
        out.push_str("Syntax coverage unavailable; call/test scope unknown.\n");
        return out;
    }
    // OPEN commonly points at a declaration's indentation or its doc comment.
    let declaration = outline
        .enclosing(byte)
        .last()
        .copied()
        .or_else(|| outline.declarations.iter().find(|d| d.start_byte >= byte));
    let Some(declaration) = declaration else {
        out.push_str("No declaration selected at this position; use scope unknown.\n");
        return out;
    };
    let test = declaration.test_context.is_some();
    let calls = outline
        .references
        .iter()
        .filter(|r| {
            r.name == declaration.name
                && r.usage == "call"
                && !(declaration.start_byte <= r.start_byte && r.start_byte < declaration.end_byte)
        })
        .collect::<Vec<_>>();
    let non_test = calls.iter().filter(|r| !r.test_context).count();
    let test_calls = calls.len().saturating_sub(non_test);
    let label = if test {
        "test context"
    } else if declaration.review_only_line.is_some() {
        "declared review-only"
    } else if non_test > 0 {
        "non-test call candidates"
    } else {
        "unknown use"
    };
    let name: String = declaration.name.chars().take(80).collect();
    let _ = writeln!(
        out,
        "{name}: {label}; {non_test} non-test and {test_calls} test call sites with the same name in this file. Name binding and other files are unchecked."
    );
    if let Some(line) = declaration.review_only_line {
        let _ = writeln!(
            out,
            "Review-only is the source author's declared intent at {source}:{line}, not an enforcement or runtime assertion."
        );
    }
    if test_calls > 0 && non_test == 0 {
        out.push_str(
            "Only test calls found here does not establish test-only use across the repository.\n",
        );
    }
    if out.len() <= budget {
        out
    } else {
        "SOURCE USE EVIDENCE — metadata omitted by budget; runtime use unknown.\n".into()
    }
}
