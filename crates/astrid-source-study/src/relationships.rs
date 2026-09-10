//! Bounded lexical relationships, explicitly not a resolved call graph.
use crate::Catalog;
use anyhow::{Result, bail};

impl Catalog {
    pub(crate) fn relate(&self, symbol: &str, page: usize) -> Result<String> {
        if !valid_symbol(symbol) {
            bail!(
                "RELATE expects one exact identifier, such as EventDispatcher; use FIND for literal phrases"
            );
        }
        let report = self.search(symbol, true)?;
        let header = report.header(&format!("Symbol relationships: {symbol}. Exact identifier matches; lexical candidates, not compiler-resolved calls. OPEN supplies numbered source."));
        crate::navigation::paginate_with_header(
            &header,
            report.lines(symbol, true),
            &format!("SELF_STUDY RELATE {symbol}"),
            page,
        )
    }
}

pub(crate) fn valid_symbol(symbol: &str) -> bool {
    !symbol.is_empty()
        && symbol.len() <= 160
        && symbol
            .bytes()
            .enumerate()
            .all(|(i, b)| b == b'_' || b.is_ascii_alphabetic() || (i > 0 && b.is_ascii_digit()))
}
