//! Bounded lexical relationships, explicitly not a resolved call graph.
use crate::Catalog;
use anyhow::{Result, bail};
use std::{collections::BTreeMap, fs};

impl Catalog {
    pub(crate) fn relate(&self, symbol: &str, page: usize) -> Result<String> {
        validate_symbol(symbol)?;
        let mut groups = BTreeMap::<u8, Vec<String>>::new();
        let (mut scanned, mut bytes, mut hits, mut skipped) = (0usize, 0u64, 0usize, 0usize);
        let mut bounded = false;
        for source in self.sources()? {
            let Ok(meta) = fs::metadata(&source.path) else {
                skipped = skipped.saturating_add(1);
                continue;
            };
            if meta.len() > 64 * 1024 * 1024 {
                skipped = skipped.saturating_add(1);
                continue;
            }
            if bytes.saturating_add(meta.len()) > 128 * 1024 * 1024 || hits >= 1500 {
                bounded = true;
                break;
            }
            bytes = bytes.saturating_add(meta.len());
            scanned = scanned.saturating_add(1);
            let Ok(text) = fs::read_to_string(&source.path) else {
                skipped = skipped.saturating_add(1);
                continue;
            };
            let mut test_region = source.id.contains("/tests/")
                || source.id.contains("/test_")
                || source.id.contains("_tests.");
            for (i, line) in text.lines().enumerate() {
                if line.contains("#[cfg(test)]") {
                    test_region = true;
                }
                let words = line
                    .split(|c: char| !c.is_ascii_alphanumeric() && c != '_')
                    .filter(|s| !s.is_empty())
                    .collect::<Vec<_>>();
                if !words.contains(&symbol) {
                    continue;
                }
                let definition = words.windows(2).any(|w| {
                    matches!(
                        w[0],
                        "fn" | "struct" | "enum" | "trait" | "type" | "class" | "def" | "function"
                    ) && w[1] == symbol
                });
                let implementation = words.first() == Some(&"impl");
                let group = if test_region {
                    2
                } else if definition {
                    0
                } else if implementation {
                    1
                } else {
                    3
                };
                let excerpt = line.trim();
                let excerpt = &excerpt[..excerpt.floor_char_boundary(excerpt.len().min(180))];
                groups.entry(group).or_default().push(format!(
                    "SELF_STUDY OPEN {} {} — line {}: {}",
                    source.id,
                    i.saturating_add(1),
                    i.saturating_add(1),
                    excerpt
                ));
                hits = hits.saturating_add(1);
                if hits >= 1500 {
                    bounded = true;
                    break;
                }
            }
        }
        let mut lines = vec![format!(
            "Symbol relationships: {symbol}. Exact identifier matches; lexical candidates, not compiler-resolved calls or proof of runtime use. OPEN supplies numbered source. Definitions precede implementation blocks, test occurrences and other references."
        )];
        for (key, rows) in groups {
            lines.push(
                [
                    "Definition candidates",
                    "Implementation blocks",
                    "Test occurrences (path or test-module marker)",
                    "Other references (may include calls, imports, comments or docs)",
                ][usize::from(key)]
                .into(),
            );
            lines.extend(rows);
        }
        if hits == 0 {
            lines.push(format!(
                "No exact identifier matches for {symbol}. Try FIND <shorter literal text> or MAP."
            ));
        }
        lines.push(format!("Scanned {scanned} files / {bytes} bytes; {skipped} skipped; {hits} matches. Scan limit reached: {bounded}. No source bookmark advanced."));
        crate::navigation::paginate(lines, &format!("SELF_STUDY RELATE {symbol}"), page)
    }
}

fn validate_symbol(symbol: &str) -> Result<()> {
    if symbol.is_empty()
        || symbol.len() > 160
        || !symbol
            .bytes()
            .enumerate()
            .all(|(i, b)| b == b'_' || b.is_ascii_alphabetic() || (i > 0 && b.is_ascii_digit()))
    {
        bail!(
            "RELATE expects one exact identifier, such as EventDispatcher; use FIND for literal phrases"
        );
    }
    Ok(())
}
