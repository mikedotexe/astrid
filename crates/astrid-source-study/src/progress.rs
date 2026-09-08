use crate::{Page, SourceRevision};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

pub(crate) type Progress = BTreeMap<String, SourceProgress>;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub(crate) struct SourceProgress {
    pub revision: SourceRevision,
    pub ranges: Vec<(usize, usize)>,
}

pub(crate) fn record(progress: &mut Progress, page: &Page) {
    let item = progress
        .entry(page.source.clone())
        .or_insert_with(|| SourceProgress {
            revision: page.revision.clone(),
            ranges: Vec::new(),
        });
    if item.revision != page.revision {
        item.revision = page.revision.clone();
        item.ranges.clear();
    }
    item.ranges.push((page.start.byte, page.end.byte));
    item.ranges.sort_unstable();
    let mut merged: Vec<(usize, usize)> = Vec::new();
    for &(start, end) in &item.ranges {
        if let Some(last) = merged.last_mut()
            && start <= last.1
        {
            last.1 = last.1.max(end);
        } else {
            merged.push((start, end));
        }
    }
    item.ranges = merged;
}

impl SourceProgress {
    pub(crate) fn label(&self) -> String {
        let complete = self.ranges.as_slice() == [(0, self.revision.bytes)];
        let ranges = self
            .ranges
            .iter()
            .take(4)
            .map(|(a, b)| format!("{a}..{b}"))
            .collect::<Vec<_>>()
            .join(", ");
        format!(
            "{}; delivered bytes {ranges}{} of {}; sha256:{}; understanding not asserted",
            if complete {
                "Complete delivery"
            } else {
                "Partial delivery"
            },
            if self.ranges.len() > 4 {
                ", … (more ranges retained)"
            } else {
                ""
            },
            self.revision.bytes,
            self.revision.sha256
        )
    }

    pub(crate) fn complete(&self) -> bool {
        self.ranges.as_slice() == [(0, self.revision.bytes)]
    }
}
