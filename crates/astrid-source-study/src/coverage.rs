//! Delivery coverage beside the reading position; preparation is never credit.
use crate::progress::{Progress, SourceProgress};
use crate::{Catalog, Page, digest};
use std::{fmt::Write as _, fs};

/// This is reference metadata, not another source interval. For a final offered
/// page, show remaining gaps conditionally without mutating durable progress.
pub(crate) fn render(progress: &Progress, page: &Page, catalog: &Catalog, offered: bool) -> String {
    render_pages(
        progress,
        page,
        catalog,
        if offered {
            std::slice::from_ref(page)
        } else {
            &[]
        },
    )
}

pub(crate) fn render_pages(
    progress: &Progress,
    page: &Page,
    catalog: &Catalog,
    offers: &[Page],
) -> String {
    let previous = progress
        .get(&page.source)
        .filter(|p| p.revision == page.revision);
    let prior = previous.map_or_else(
        || "No verified delivery recorded for this revision; understanding is not asserted".into(),
        SourceProgress::label,
    );
    let mut text = format!("\nREADING COVERAGE — verified before this response: {prior}.\n");
    if !page.eof {
        text.push_str("The offered page is not counted until its complete delivery is verified.\n");
        return text;
    }
    text.push_str("End of file is a reading position, not a claim that all source was delivered or understood.\n");
    let mut projected = Progress::new();
    if let Some(previous) = previous {
        projected.insert(page.source.clone(), previous.clone());
    }
    for offered in offers
        .iter()
        .filter(|offered| offered.source == page.source && offered.revision == page.revision)
    {
        crate::progress::record(&mut projected, offered);
    }
    let empty = SourceProgress {
        revision: page.revision.clone(),
        ranges: Vec::new(),
    };
    let coverage = projected.get(&page.source).unwrap_or(&empty);
    let gaps = coverage.gaps();
    let condition = if offers.is_empty() {
        "At the current verified coverage"
    } else if offers.len() == 1 {
        "If this offered page is verified"
    } else {
        "If this whole offered session is verified"
    };
    if gaps.is_empty() {
        let _ = writeln!(
            text,
            "{condition}, all bytes of this revision are covered. That still does not establish understanding."
        );
        return text;
    }
    let shown = gaps
        .iter()
        .take(3)
        .map(|(start, end)| format!("{start}..{end}"))
        .collect::<Vec<_>>()
        .join(", ");
    let more = if gaps.len() > 3 { ", …" } else { "" };
    let _ = writeln!(
        text,
        "{condition}, these source bytes remain without verified delivery: {shown}{more}."
    );
    let source = catalog.resolve(&page.source).ok();
    let bytes = source
        .and_then(|source| fs::read(source.path).ok())
        .filter(|bytes| {
            bytes.len() == page.revision.bytes && digest(bytes) == page.revision.sha256
        });
    if let Some(bytes) = bytes {
        for (start, end) in gaps.iter().take(3) {
            let line = bytes[..*start].split(|byte| *byte == b'\n').count();
            let _ = writeln!(
                text,
                "Optional missing-region read, bytes {start}..{end}: SELF_STUDY OPEN {} {line}",
                page.source
            );
        }
        text.push_str("OPEN starts at the containing line and may reread its earlier fragment. You may also browse, reread, or leave this study.\n");
    } else {
        text.push_str("The current file could not be matched to this revision for gap-opening links. The retained coverage remains tied to its stated revision.\n");
    }
    text
}
