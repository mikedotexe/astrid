//! Optional Being-authored conclusions anchored only in verified supplied source.
//! An anchor proves delivery of a fragment, never the correctness of its interpretation.
use crate::{Page, digest};
use serde::{Deserialize, Serialize};
use std::fmt::Write as _;

const MAX_FINDINGS: usize = 6;
const MAX_LOCATIONS: usize = 6;
const MAX_WORDS_BYTES: usize = 600;
const MAX_FRAGMENT_BYTES: usize = 240;
const MAX_UPDATES: usize = 6;

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub(crate) struct Findings {
    authored: Vec<Finding>,
    supplied_locations: Vec<Anchor>,
    updates: Vec<String>,
    #[serde(default, skip_serializing_if = "is_zero")]
    omitted_locations_for_input_budget: usize,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
struct Finding {
    id: String,
    words: String,
    response_sha256: String,
    anchor: Anchor,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
struct Anchor {
    source: String,
    line: usize,
    revision_sha256: String,
    page_id: String,
    page_start_byte: usize,
    page_end_byte: usize,
    delivered_line_fragment: String,
    fragment_truncated: bool,
    reopen_current_checkout: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    lexical_location_kind: Option<String>,
}

pub(crate) fn is_directive(line: &str) -> bool {
    line.starts_with("STUDY_FINDING:") || line.starts_with("STUDY_FINDING_DROP:")
}

impl Findings {
    /// Keep the latest explicit update visible across ordinary study responses.
    /// Rendering only omits whole duplicate results; durable receipts stay intact.
    pub(crate) fn render_updates(&self, max_bytes: usize) -> String {
        let mut out = format!(
            "FINDING CAPACITY — {}/{} retained; {} available. Saving at an existing cited location replaces that finding; nothing is evicted automatically.\n",
            self.authored.len(),
            MAX_FINDINGS,
            MAX_FINDINGS.saturating_sub(self.authored.len()),
        );
        if self.updates.is_empty() {
            return out;
        }
        out.push_str("LATEST FINDING SAVE/REMOVE RESULTS — from your most recent explicit update, retained until another finding update. Newest result first. These report storage, not whether a conclusion is correct.\n");
        let omitted = |count| {
            format!(
                "{count} earlier result(s) remain whole in source_findings.updates in the full notebook below.\n"
            )
        };
        let omission_reserve = omitted(self.updates.len()).len();
        let mut shown = 0_usize;
        for (index, update) in self.updates.iter().enumerate().rev() {
            let row = format!("Result {}: {update}\n", index.saturating_add(1));
            if out
                .len()
                .saturating_add(row.len())
                .saturating_add(omission_reserve)
                > max_bytes
            {
                break;
            }
            out.push_str(&row);
            shown = shown.saturating_add(1);
        }
        if shown < self.updates.len() {
            out.push_str(&omitted(self.updates.len().saturating_sub(shown)));
        }
        out
    }

    /// A readable view of authored words beside their retained evidence. This
    /// never promotes an interpretation to fact or reads a newer source revision.
    pub(crate) fn render_authored(&self, max_bytes: usize) -> String {
        if self.authored.is_empty() {
            return String::new();
        }
        let mut out = String::from(
            "YOUR SOURCE-LINKED FINDINGS — your saved interpretations, not independently verified facts. These fragments were supplied earlier; surrounding code may matter and the current checkout may differ. Keep, revise or remove a finding as you choose.\n",
        );
        let omitted = |count| {
            format!(
                "{count} additional finding(s) remain whole in the full notebook below; this preview omits them to leave room for source and recent responses.\n"
            )
        };
        let omission_reserve = omitted(self.authored.len()).len();
        if out.len().saturating_add(omission_reserve) > max_bytes {
            let notice = omitted(self.authored.len());
            return if notice.len() <= max_bytes {
                notice
            } else {
                String::new()
            };
        }
        let mut shown = 0_usize;
        for finding in &self.authored {
            let anchor = &finding.anchor;
            let mut row = String::new();
            let _ = writeln!(
                row,
                "Your words: {:?}\nRetained fragment {}:{} (sha256:{}{}): {:?}\nReopen current source: {}\nOptional replacement (supply your own revised words): STUDY_FINDING: {}:{} | your revised words\nOptional removal: STUDY_FINDING_DROP: {}",
                finding.words,
                anchor.source,
                anchor.line,
                anchor.revision_sha256,
                if anchor.fragment_truncated {
                    "; fragment excerpt"
                } else {
                    ""
                },
                anchor.delivered_line_fragment,
                anchor.reopen_current_checkout,
                anchor.source,
                anchor.line,
                finding.id,
            );
            if out
                .len()
                .saturating_add(row.len())
                .saturating_add(omission_reserve)
                > max_bytes
            {
                // Preserve exact optional actions even when a duplicate of the
                // whole authored words and fragment would crowd out the notebook.
                row = format!(
                    "Finding at {}:{} — your whole words and retained fragment remain in the full notebook below; their duplicate preview is omitted for input space.\nReopen current source: {}\nOptional replacement (supply your own revised words): STUDY_FINDING: {}:{} | your revised words\nOptional removal: STUDY_FINDING_DROP: {}\n",
                    anchor.source,
                    anchor.line,
                    anchor.reopen_current_checkout,
                    anchor.source,
                    anchor.line,
                    finding.id,
                );
                if out
                    .len()
                    .saturating_add(row.len())
                    .saturating_add(omission_reserve)
                    > max_bytes
                {
                    break;
                }
            }
            out.push_str(&row);
            shown = shown.saturating_add(1);
        }
        if shown < self.authored.len() {
            out.push_str(&omitted(self.authored.len().saturating_sub(shown)));
        }
        out
    }

    pub(crate) fn has_authored(&self) -> bool {
        !self.authored.is_empty()
    }

    pub(crate) fn has_updates(&self) -> bool {
        !self.updates.is_empty()
    }

    pub(crate) fn is_empty(&self) -> bool {
        self.authored.is_empty()
            && self.supplied_locations.is_empty()
            && self.updates.is_empty()
            && self.omitted_locations_for_input_budget == 0
    }

    /// Only render-time recall may be omitted; authored findings stay durable and whole.
    pub(crate) fn omit_oldest_location(&mut self) -> bool {
        if self.supplied_locations.is_empty() {
            false
        } else {
            self.supplied_locations.remove(0);
            self.omitted_locations_for_input_budget =
                self.omitted_locations_for_input_budget.saturating_add(1);
            true
        }
    }

    pub(crate) fn record(&mut self, response: &str, text: &str, pages: &[Page]) {
        let lines = supplied_lines(pages);
        let mut count = 0_usize;
        for line in crate::response_choice::eligible_lines(text) {
            let line = line.trim();
            if !is_directive(line) {
                continue;
            }
            if count == 0 {
                self.updates.clear();
            }
            count = count.saturating_add(1);
            if count > MAX_UPDATES {
                self.updates.push("Further finding directives were not applied: at most six updates per response. Omitted findings remain unchanged.".into());
                break;
            }
            let feedback = if let Some(value) = line.strip_prefix("STUDY_FINDING:") {
                self.save(value.trim(), &digest(response), &lines)
            } else {
                self.remove(line.trim_start_matches("STUDY_FINDING_DROP:").trim())
            };
            self.updates.push(feedback);
        }
        // Observe only lexical locations actually present in numbered source.
        // This does not read files, seek unseen enclosing scope, or infer answers.
        for (page, line, text) in lines {
            let Some(location) = page
                .source_locations
                .iter()
                .find(|location| location.line == line)
            else {
                continue;
            };
            if !safe_source(&page.source) {
                continue;
            }
            let mut anchor = Anchor::from_line(page, line, text);
            anchor.lexical_location_kind = Some(format!(
                "{} candidate: {}; supplied syntax is not proof of behavior",
                &location.kind[..location
                    .kind
                    .floor_char_boundary(location.kind.len().min(80))],
                &location.name[..location
                    .name
                    .floor_char_boundary(location.name.len().min(120))],
            ));
            self.supplied_locations
                .retain(|old| old.source != anchor.source || old.line != anchor.line);
            self.supplied_locations.push(anchor);
            if self.supplied_locations.len() > MAX_LOCATIONS {
                self.supplied_locations.remove(0);
            }
        }
    }

    fn save(&mut self, value: &str, response_hash: &str, lines: &[(&Page, usize, &str)]) -> String {
        let Some((citation, words)) = value.split_once('|') else {
            return "Finding not saved. Use STUDY_FINDING: repository/path:line | your words (up to 600 bytes); cite a supplied numbered source line.".into();
        };
        let words = words.trim();
        let Some((source, line)) = parse_citation(citation.trim()) else {
            return "Finding not saved: citation needs an exact repository/path and positive line number; placeholders, traversal and malformed paths are not citations.".into();
        };
        if words.is_empty() || words.len() > MAX_WORDS_BYTES || words.chars().any(char::is_control)
        {
            return "Finding not saved: provide your words on one line, from 1 to 600 bytes. Your previous finding remains unchanged.".into();
        }
        let direct: Vec<_> = lines
            .iter()
            .filter(|(page, number, _)| page.source == source && *number == line)
            .collect();
        // Two selected fragments/revisions of one line require an explicit reread;
        // never silently choose which fragment an ambiguous citation meant.
        let anchor = match direct.as_slice() {
            [(page, number, fragment)] => Some(Anchor::from_line(page, *number, fragment)),
            [] => self
                .supplied_locations
                .iter()
                .chain(self.authored.iter().map(|finding| &finding.anchor))
                .find(|anchor| anchor.source == source && anchor.line == line)
                .cloned(),
            _ => {
                return format!(
                    "Finding not saved: multiple supplied fragments match this source line. To choose an unambiguous anchor, optionally reopen one page: SELF_STUDY OPEN {source} {line}"
                );
            },
        };
        let Some(anchor) = anchor else {
            return format!(
                "Finding not saved: that numbered line is not in this input's source pages or this inquiry's retained source anchors. Search/map mentions and recalled prose are not source anchors. To supply the line, optionally use: SELF_STUDY OPEN {source} {line}"
            );
        };
        let id = format!("f{}", digest(format!("{source}:{line}")));
        let existing = self.authored.iter().position(|finding| finding.id == id);
        if existing.is_none() && self.authored.len() >= MAX_FINDINGS {
            return "Finding not saved: six authored findings are retained in this inquiry. Nothing was evicted. You may keep them all, replace a finding at its existing cited location, or remove one using its exact optional command below before trying a new location again.".into();
        }
        let finding = Finding {
            id: id.clone(),
            words: words.into(),
            response_sha256: response_hash.into(),
            anchor,
        };
        if let Some(index) = existing {
            self.authored[index] = finding;
        } else {
            self.authored.push(finding);
        }
        let operation = if existing.is_some() {
            "Updated"
        } else {
            "Saved"
        };
        format!(
            "{operation} {id}: your authored conclusion beside a delivered source fragment; correctness is not verified."
        )
    }

    fn remove(&mut self, id: &str) -> String {
        let Some(index) = self.authored.iter().position(|finding| finding.id == id) else {
            return "No authored finding with that ID is currently retained in this inquiry; it may already have been removed. Source delivery history is unchanged. To remove another finding, use STUDY_FINDING_DROP: followed by its exact displayed ID.".into();
        };
        self.authored.remove(index);
        format!("Removed authored finding {id}; source delivery history is unchanged.")
    }
}

// Serde's skip_serializing_if callback receives a reference to the field.
#[allow(clippy::trivially_copy_pass_by_ref)]
fn is_zero(value: &usize) -> bool {
    *value == 0
}

impl Anchor {
    fn from_line(page: &Page, line: usize, fragment: &str) -> Self {
        Self {
            source: page.source.clone(),
            line,
            revision_sha256: page.revision.sha256.clone(),
            page_id: page.id.clone(),
            page_start_byte: page.start.byte,
            page_end_byte: page.end.byte,
            delivered_line_fragment: fragment
                [..fragment.floor_char_boundary(fragment.len().min(MAX_FRAGMENT_BYTES))]
                .into(),
            fragment_truncated: fragment.len() > MAX_FRAGMENT_BYTES,
            reopen_current_checkout: format!("SELF_STUDY OPEN {} {line}", page.source),
            lexical_location_kind: None,
        }
    }
}

fn parse_citation(citation: &str) -> Option<(&str, usize)> {
    let (source, line) = citation.rsplit_once(':')?;
    if !safe_source(source) || !line.bytes().all(|byte| byte.is_ascii_digit()) {
        return None;
    }
    let line: usize = line.parse().ok()?;
    (line > 0).then_some((source, line))
}

fn safe_source(source: &str) -> bool {
    source.len() <= 300
        && source.contains('/')
        && source
            .split('/')
            .all(|part| !part.is_empty() && part != "." && part != "..")
        && source
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || b"_./-".contains(&byte))
}

/// Page framing is excluded: source rows have the exact reader-produced prefix.
fn supplied_lines(pages: &[Page]) -> Vec<(&Page, usize, &str)> {
    pages
        .iter()
        .flat_map(|page| {
            page.text.lines().filter_map(move |text| {
                let (prefix, fragment) = text.split_once(" | ")?;
                let line: usize = prefix.trim().parse().ok()?;
                (prefix == format!("{line:>6}")
                    && page.start.byte < page.end.byte
                    && (page.start.line..=page.end.line).contains(&line))
                .then_some((page, line, fragment))
            })
        })
        .collect()
}
