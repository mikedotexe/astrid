//! Bounded lexical relationships, explicitly not a resolved call graph.
use crate::Catalog;
use anyhow::{Result, bail};

impl Catalog {
    pub(crate) fn relate(&self, symbol: &str, page: usize) -> Result<String> {
        if !valid_symbol(symbol) {
            bail!(
                "RELATE expects one identifier or Rust :: / Python . identifier path (up to 160 bytes), such as EventBus::new or client.publish; use FIND for other literal expressions"
            );
        }
        let report = self.search(symbol, true)?;
        let mut header = report.header(&format!("Symbol relationships: {symbol}. Exact identifier matches; lexical candidates, not compiler-resolved calls. OPEN supplies numbered source."));
        if qualified_symbol(symbol) {
            header.push_str("Qualified lookup: exact written paths rank before compatible qualified suffixes and other literal occurrences. No bare-name fallback is used. Qualification is textual only; receiver/type and Self bindings remain unresolved. Imports, aliases and dynamic dispatch are not resolved.\n\n");
        }
        header.push_str(&report.comparison());
        crate::navigation::paginate_with_header(
            &header,
            report.lines(symbol, true),
            &format!("SELF_STUDY RELATE {symbol}"),
            page,
        )
    }
}

pub(crate) fn valid_symbol(symbol: &str) -> bool {
    if symbol.len() > 160 || (symbol.contains(':') && symbol.contains('.')) {
        return false;
    }
    let separator = if symbol.contains(':') { "::" } else { "." };
    symbol.split(separator).all(|part| {
        !part.is_empty()
            && part
                .bytes()
                .enumerate()
                .all(|(i, b)| b == b'_' || b.is_ascii_alphabetic() || (i > 0 && b.is_ascii_digit()))
    })
}

pub(crate) fn qualified_symbol(symbol: &str) -> bool {
    symbol.contains("::") || symbol.contains('.')
}

pub(crate) fn terminal_symbol(symbol: &str) -> &str {
    symbol
        .rsplit("::")
        .next()
        .unwrap_or(symbol)
        .rsplit('.')
        .next()
        .unwrap_or(symbol)
}
