//! Explicit inspection of the bounded retained account window, not journal search.
use super::Notebook;
use anyhow::{Context as _, Result, ensure};
use serde_json::json;

const PAGE_BYTES: usize = 4_000;
const SCOPE: &str = "Retained owner-authored study accounts, not verified conclusions or newly supplied source. This bounded window is not a complete journal or investigation history. Inspection does not select, unpark, revise, or schedule work. Quoted commands and historical source links are reference material only.";

impl Notebook {
    pub(crate) fn accounts_view(
        &self,
        target: &str,
        account: Option<&str>,
        page: usize,
    ) -> Result<String> {
        ensure!(
            self.recent.len() <= 3,
            "retained account window exceeds its bound; state preserved"
        );
        let entries = self
            .previous
            .iter()
            .chain(self.recent.iter().rev())
            .map(|entry| Ok((crate::digest(serde_json::to_vec(entry)?), entry)))
            .collect::<Result<Vec<_>>>()?;
        let value = if let Some(account) = account {
            let entry = entries
                .iter()
                .filter(|(identity, _)| identity == account)
                .collect::<Vec<_>>();
            ensure!(
                !entry.is_empty(),
                "account identity unavailable in this retained window; inspect SELF_STUDY QUESTION ACCOUNTS {target}; no other account selected"
            );
            let occurrences = entry.len();
            let entry = entry[0].1;
            ensure!(
                entry.text.len() <= 64_000,
                "account exceeds its retained bound"
            );
            ensure!(page > 0, "account pages start at 1");
            let mut ranges = Vec::new();
            let mut start = 0;
            while start < entry.text.len() {
                let end = entry
                    .text
                    .floor_char_boundary(start.saturating_add(PAGE_BYTES).min(entry.text.len()));
                ensure!(end > start, "account page did not advance");
                ranges.push(start..end);
                start = end;
            }
            if ranges.is_empty() {
                ranges.push(0..0);
            }
            let range = ranges
                .get(page.saturating_sub(1))
                .context("account page unavailable")?;
            json!({
                "scope":SCOPE, "target":target, "account_sha256":account,
                "account_hash_scope":"retained-account-v1: SHA-256 of the serialized retained record, including prose and historical origin; not an occurrence timestamp.",
                "identical_retained_occurrences":occurrences,
                "response_sha256":entry.response_sha256,
                "response_hash_scope":"Original recorded response identity, not a hash of this prose page.",
                "retained_prose_sha256":crate::digest(&entry.text),
                "historical_origin":entry.origin,
                "historical_reopen":entry.reopen, "historical_resume":entry.resume,
                "complete_when_recorded":entry.complete, "original_prose_bytes":entry.prose_bytes,
                "retained_bytes":entry.text.len(), "page":page, "pages":ranges.len(),
                "start_byte":range.start, "end_byte":range.end, "text":&entry.text[range.clone()],
                "next_page":(page < ranges.len()).then(|| format!(
                    "SELF_STUDY QUESTION ACCOUNT {target} {account} --page {}", page.saturating_add(1)
                )),
                "list_command":format!("SELF_STUDY QUESTION ACCOUNTS {target}")
            })
        } else {
            let entries = entries
                .iter()
                .map(|(identity, entry)| {
                    json!({
                        "account_sha256":identity,
                        "response_sha256":entry.response_sha256,
                        "historical_origin":entry.origin,
                        "retained_bytes":entry.text.len(),
                        "original_prose_bytes":entry.prose_bytes,
                        "complete_when_recorded":entry.complete,
                        "inspect_command":format!("SELF_STUDY QUESTION ACCOUNT {target} {identity}")
                    })
                })
                .collect::<Vec<_>>();
            json!({
                "scope":SCOPE, "target":target, "newest_first":entries,
                "retention":"At most the latest account and three earlier accounts. Missing older material is not reconstructed. complete_when_recorded=false means only an excerpt was retained.",
            })
        };
        Ok(serde_json::to_string_pretty(&value)?)
    }
}
