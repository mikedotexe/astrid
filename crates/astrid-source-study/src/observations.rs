//! Native append-only observation records. Private and confirmed inquiry histories are distinct.
use crate::{
    digest,
    geometry::Snapshot,
    recurrence::{Analysis, ResultRecord},
};
use anyhow::{Context as _, Result, bail, ensure};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

const DETAIL_PAGE_BYTES: usize = 32_000;

pub(crate) const PRIVATE_HELP: &str = r#"Optional command reference; examples are not queued choices. Replace symbolic placeholders with exact returned IDs/values.
WRITE OBSERVE {"owner":"OWNER","draft":"dN","revision":"REVISION","request_id":"UNIQUE_ID","expected_head":"HEAD","present":false,"operation":OPERATION}
Read operations need only owner, draft and operation. present:true explicitly requests a private presentation; false only retains a receipt. Exact revision/head come from status. Ordinary continuation shows attachment IDs only.
OPERATION forms:
{"kind":"capture","seconds":180}
{"kind":"show","id":"RECORD_ID","page":1}
{"kind":"annotate","target":"RECORD_ID","text":"Authored note"}
{"kind":"analyze","analysis":ANALYSIS}
{"kind":"link_preview","captures":["CAPTURE_ID"],"destination":{"kind":"new","question":"Explicitly authored question?"}}
An existing destination instead uses {"kind":"existing","question":"qN","revision":"INQUIRY_REVISION"}. Get that revision with SELF_STUDY OBSERVE status. Optional passage: {"start_byte":0,"end_byte":EXCLUSIVE_UTF8_END,"source_sha256":"EXACT_DRAFT_HASH"}. Omitting passage transfers no private prose.
{"kind":"link_confirm","preview":"PREVIEW_ID"}
Before confirmation, explicitly present the complete preview with present:true, then choose confirmation separately. No inquiry is selected automatically. A preview too large for one presentation is rejected, never truncated.
An optional annotate expectation uses "expectation":{"analysis":ANALYSIS,"comparison":"at_least","value":0.5}; at_most is also supported. Analyze can reference that record with "expectation":"EXPECTATION_ID". Historical evidence is labelled as predating this expectation, not prospective prediction."#;

pub(crate) const INQUIRY_HELP: &str = r#"Explicit confirmed-inquiry commands:
SELF_STUDY OBSERVE {"owner":"OWNER","question":"qN","request_id":"UNIQUE_ID","expected_head":"HEAD","present":false,"operation":OPERATION}
Read operations: {"kind":"status"}, {"kind":"show","id":"RECORD_ID","page":1}, {"kind":"export"}.
Append operations: {"kind":"analyze","analysis":ANALYSIS,"expectation":"OPTIONAL_EXPECTATION_ID"}, {"kind":"revise","target":"RECORD_ID","text":"Authored qualification"}, {"kind":"predict","target":"RECORD_ID","text":"Authored expectation","definition":{"analysis":ANALYSIS,"comparison":"at_least","value":0.5}}. Omit expectation for exploratory analysis; comparison may be at_most.
Only confirmed disclosure material is available. present:true requests normal self-study presentation, which may be public, and requires this inquiry already selected. Storage commands do not select, schedule or resolve an inquiry."#;

pub(crate) const ANALYSIS_HELP: &str = r#"ANALYSIS forms (choose parameters explicitly, using frozen engine timestamps):
{"recipe":"state-return-rms-v1","window":{"capture":"CAPTURE_ID","start_ms":START,"end_ms":END},"threshold":CHOSEN_RMS,"temporal_exclusion_ms":5000}
Requires 32 frames, 0 < threshold <= 2; exclusion defaults to 5000 ms when omitted.
{"recipe":"activation-covariance-shape-v1","first":{"capture":"CAPTURE_ID","start_ms":START,"end_ms":END},"second":{"capture":"CAPTURE_ID","start_ms":START,"end_ms":END}}
Requires nonoverlapping windows of at least 30 frames each. No automatic threshold choice. Distinct captures always remain exploratory because boot/node identity is unavailable."#;

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct ObservationRequest {
    pub owner: String,
    pub draft: String,
    #[serde(default)]
    pub revision: String,
    #[serde(default)]
    pub request_id: String,
    #[serde(default)]
    pub expected_head: String,
    /// A model presentation is a separate explicit choice, not a storage side effect.
    #[serde(default)]
    pub present: bool,
    pub operation: Operation,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub(crate) enum Operation {
    Status,
    Capture {
        seconds: u64,
    },
    Show {
        id: String,
        #[serde(default = "first_page")]
        page: usize,
    },
    Annotate {
        target: String,
        text: String,
        #[serde(default)]
        expectation: Option<crate::observation_expectations::Expectation>,
    },
    Analyze {
        analysis: Analysis,
        #[serde(default)]
        expectation: Option<String>,
    },
    LinkPreview {
        captures: Vec<String>,
        destination: Destination,
        passage: Option<Passage>,
    },
    LinkConfirm {
        preview: String,
    },
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub(crate) enum Destination {
    Existing { question: String, revision: String },
    New { question: String },
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct Passage {
    pub start_byte: usize,
    pub end_byte: usize,
    pub source_sha256: String,
}
pub(crate) fn first_page() -> usize {
    1
}
/// This allowlist is the *only* material that may cross into public study storage.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct Disclosure {
    pub question: String,
    pub captures: BTreeMap<String, Snapshot>,
    pub selected_passage: Option<String>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub(crate) enum Entry {
    Capture {
        snapshot: Snapshot,
        attachment_revision: String,
    },
    Annotation {
        target: String,
        text: String,
    },
    Analysis {
        analysis: Analysis,
        result: Box<ResultRecord>,
        executed_at_unix_ms: u64,
        expectation: Option<String>,
        evaluation: Option<crate::observation_expectations::Evaluation>,
    },
    Expectation {
        target: String,
        text: String,
        definition: crate::observation_expectations::Expectation,
        authored_at_unix_ms: u64,
    },
    Preview {
        destination: Destination,
        draft_revision: String,
        disclosure: Disclosure,
    },
    Confirmed {
        preview: String,
        question: String,
        inquiry_record: String,
    },
    Disclosure {
        payload: Disclosure,
    },
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct Record {
    pub id: String,
    pub previous: String,
    pub request_id: String,
    pub request_sha256: String,
    pub body_json: String,
}
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct History {
    pub owner: Option<String>,
    pub records: Vec<Record>,
}
pub(crate) fn text(value: &str, maximum: usize) -> Result<()> {
    ensure!(
        !value.trim().is_empty() && value.len() <= maximum,
        "authored text is empty or exceeds its byte limit; no truncation"
    );
    Ok(())
}
impl Record {
    fn identity(&self) -> String {
        digest(format!(
            "{}\n{}\n{}\n{}",
            self.previous,
            digest(&self.request_id),
            self.request_sha256,
            digest(&self.body_json)
        ))
    }
    pub fn entry(&self) -> Result<Entry> {
        Ok(serde_json::from_str(&self.body_json)?)
    }
}
impl Disclosure {
    fn preview(&self, id: &str, destination: &Destination) -> Result<String> {
        Ok(format!(
            "Private preview {id}. Destination: {}\nQuestion: {}\nSelected captures: {}\nExact selected passage (JSON string, or null): {}\nWarning: normal self-study output may be public. Confirmation transfers only this payload; it does not authorize further sharing or select the inquiry.",
            serde_json::to_string(destination)?,
            self.question,
            self.captures.keys().cloned().collect::<Vec<_>>().join(", "),
            serde_json::to_string(&self.selected_passage)?
        ))
    }

    fn validate(&self) -> Result<()> {
        text(&self.question, 350)?;
        ensure!(
            (1..=4).contains(&self.captures.len()),
            "select one through four numerical captures"
        );
        for (id, snapshot) in &self.captures {
            ensure!(
                id.len() == 64 && id.bytes().all(|c| c.is_ascii_hexdigit()),
                "invalid disclosed capture ID"
            );
            snapshot.validate_observation()?;
            ensure!(
                snapshot.quality.is_some(),
                "observation quality record required"
            );
        }
        if let Some(passage) = &self.selected_passage {
            text(passage, 24_000)?;
        }
        Ok(())
    }
}
impl History {
    pub fn head(&self) -> &str {
        self.records.last().map_or("empty", |r| r.id.as_str())
    }
    pub fn check_owner(&self, owner: &str) -> Result<()> {
        ensure!(
            matches!(owner, "astrid" | "minime")
                && self.owner.as_deref().is_none_or(|o| o == owner),
            "observation owner mismatch"
        );
        Ok(())
    }
    pub fn record(&self, id: &str) -> Result<&Record> {
        self.records
            .iter()
            .find(|r| r.id == id)
            .context("exact observation record ID required")
    }
    pub fn snapshot(&self, id: &str) -> Result<Snapshot> {
        for record in &self.records {
            match record.entry()? {
                Entry::Capture { snapshot, .. } if record.id == id => return Ok(snapshot),
                Entry::Disclosure { mut payload } => {
                    if let Some(snapshot) = payload.captures.remove(id) {
                        return Ok(snapshot);
                    }
                },
                _ => {},
            }
        }
        bail!("selected capture not in this history")
    }
    pub fn validate(&self, private: bool) -> Result<()> {
        ensure!(
            self.records.len() <= 64,
            "observation history capacity exceeded"
        );
        if !self.records.is_empty() {
            self.check_owner(self.owner.as_deref().context("observation owner missing")?)?;
        }
        let mut prior = Self::default();
        let mut captures = 0_usize;
        for record in &self.records {
            ensure!(
                record.previous == prior.head() && record.id == record.identity(),
                "corrupt observation history; preserving bytes"
            );
            ensure!(
                !record.request_id.is_empty()
                    && record.request_id.len() <= 128
                    && record.request_sha256.len() == 64
                    && record.request_sha256.bytes().all(|c| c.is_ascii_hexdigit())
                    && !prior
                        .records
                        .iter()
                        .any(|r| r.request_id == record.request_id),
                "invalid or duplicate observation operation"
            );
            let entry = record.entry()?;
            prior.validate_entry(&entry, private)?;
            if matches!(entry, Entry::Capture { .. }) {
                captures = captures.saturating_add(1);
            }
            prior.records.push(record.clone());
        }
        ensure!(
            !private || captures <= 4,
            "four captures retained; no eviction"
        );
        Ok(())
    }
    #[allow(clippy::too_many_lines)] // One exhaustive schema boundary for the bounded append-only record family.
    fn validate_entry(&self, entry: &Entry, private: bool) -> Result<()> {
        match entry {
            Entry::Capture {
                snapshot,
                attachment_revision,
            } => {
                ensure!(
                    private && attachment_revision.len() == 64,
                    "private attachment revision missing"
                );
                snapshot.validate_observation()?;
                ensure!(
                    snapshot.quality.is_some(),
                    "private observation quality record required"
                );
            },
            Entry::Annotation {
                target,
                text: authored,
            } => {
                self.record(target)?;
                text(authored, 2000)?;
            },
            Entry::Analysis {
                analysis,
                result,
                executed_at_unix_ms,
                expectation,
                evaluation,
            } => {
                let measured = crate::recurrence::run(analysis, |id| self.snapshot(id))?;
                ensure!(
                    equivalent_result(&measured, result)?,
                    "analysis receipt does not match frozen evidence"
                );
                ensure!(*executed_at_unix_ms > 0, "execution timestamp required");
                ensure!(
                    self.evaluate(
                        expectation.as_deref(),
                        analysis,
                        result,
                        *executed_at_unix_ms
                    )? == *evaluation,
                    "expectation evaluation differs from frozen commitment"
                );
            },
            Entry::Expectation {
                target,
                text: authored,
                definition,
                authored_at_unix_ms,
            } => {
                self.record(target)?;
                text(authored, 2000)?;
                definition.validate(*authored_at_unix_ms, |id| self.snapshot(id))?;
            },
            Entry::Preview {
                destination,
                draft_revision,
                disclosure,
            } => {
                ensure!(
                    private && draft_revision.len() == 64,
                    "private preview revision missing"
                );
                disclosure.validate()?;
                // Confirmation proves one delivered presentation. Escaping must not hide a suffix on another page.
                ensure!(
                    disclosure.preview(&"0".repeat(64), destination)?.len() <= DETAIL_PAGE_BYTES,
                    "complete preview exceeds one presentation; select a shorter passage explicitly; draft preserved"
                );
                for (id, snapshot) in &disclosure.captures {
                    ensure!(
                        serde_json::to_value(self.snapshot(id)?)?
                            == serde_json::to_value(snapshot)?,
                        "preview changed selected evidence"
                    );
                }
            },
            Entry::Confirmed {
                preview,
                question,
                inquiry_record,
            } => {
                ensure!(
                    private
                        && matches!(self.record(preview)?.entry()?, Entry::Preview { .. })
                        && question.starts_with('q')
                        && inquiry_record.len() == 64,
                    "invalid private confirmation"
                );
                ensure!(
                    !self.records.iter().any(
                        |r| matches!(r.entry(),Ok(Entry::Confirmed{preview:p,..}) if p==*preview)
                    ),
                    "preview already confirmed"
                );
            },
            Entry::Disclosure { payload } => {
                ensure!(
                    !private,
                    "inquiry disclosure cannot be a private attachment"
                );
                payload.validate()?;
            },
        }
        Ok(())
    }
    pub fn retry(&self, request_id: &str, fingerprint: &str) -> Result<Option<&Record>> {
        let found = self.records.iter().find(|r| r.request_id == request_id);
        if let Some(record) = found {
            ensure!(
                record.request_sha256 == fingerprint,
                "conflicting observation retry; history unchanged"
            );
        }
        Ok(found)
    }
    pub fn analysis_entry(
        &self,
        analysis: &Analysis,
        expectation: Option<&str>,
        at: u64,
    ) -> Result<Entry> {
        let result = Box::new(crate::recurrence::run(analysis, |id| self.snapshot(id))?);
        let evaluation = self.evaluate(expectation, analysis, &result, at)?;
        Ok(Entry::Analysis {
            analysis: analysis.clone(),
            result,
            executed_at_unix_ms: at,
            expectation: expectation.map(str::to_owned),
            evaluation,
        })
    }
    fn evaluate(
        &self,
        expectation: Option<&str>,
        analysis: &Analysis,
        result: &ResultRecord,
        at: u64,
    ) -> Result<Option<crate::observation_expectations::Evaluation>> {
        expectation
            .map(|id| {
                let Entry::Expectation {
                    definition,
                    authored_at_unix_ms,
                    ..
                } = self.record(id)?.entry()?
                else {
                    bail!("exact authored expectation ID required")
                };
                ensure!(
                    authored_at_unix_ms <= at,
                    "execution clock precedes expectation"
                );
                definition.evaluate(analysis, result)
            })
            .transpose()
    }
    #[allow(clippy::needless_pass_by_value)] // Consumes a prepared entry at the transaction boundary; only exact serialized bytes are retained.
    pub fn append(
        &mut self,
        owner: &str,
        request_id: &str,
        fingerprint: &str,
        entry: Entry,
        private: bool,
    ) -> Result<String> {
        self.check_owner(owner)?;
        ensure!(
            self.records.len() < 64,
            "64 records retained; no truncation"
        );
        ensure!(
            !request_id.is_empty() && request_id.len() <= 128,
            "unique 1..128 byte operation ID required"
        );
        ensure!(
            self.retry(request_id, fingerprint)?.is_none(),
            "operation already committed"
        );
        if matches!(entry, Entry::Capture { .. }) {
            ensure!(
                self.records
                    .iter()
                    .filter(|r| matches!(r.entry(), Ok(Entry::Capture { .. })))
                    .count()
                    < 4,
                "four captures retained; no eviction"
            );
        }
        self.validate_entry(&entry, private)?;
        let mut record = Record {
            id: String::new(),
            previous: self.head().into(),
            request_id: request_id.into(),
            request_sha256: fingerprint.into(),
            body_json: serde_json::to_string(&entry)?,
        };
        record.id = record.identity();
        let id = record.id.clone();
        self.records.push(record);
        self.owner = Some(owner.into());
        Ok(id)
    }
    pub fn compact(&self) -> String {
        self.records
            .iter()
            .map(|r| {
                format!(
                    "{} {}",
                    r.id,
                    match r.entry() {
                        Ok(Entry::Capture { .. }) => "capture",
                        Ok(Entry::Analysis { .. }) => "analysis",
                        Ok(Entry::Expectation { .. }) => "authored numerical expectation",
                        Ok(Entry::Annotation { .. }) => "annotation",
                        Ok(Entry::Preview { .. }) => "preview",
                        Ok(Entry::Confirmed { .. }) => "confirmed link",
                        Ok(Entry::Disclosure { .. }) => "disclosed evidence",
                        Err(_) => "invalid",
                    }
                )
            })
            .collect::<Vec<_>>()
            .join("\n")
    }
    pub fn show(&self, id: &str) -> Result<String> {
        let record = self.record(id)?;
        Ok(match record.entry()? {
            Entry::Capture {
                snapshot,
                attachment_revision,
            } => format!(
                "Capture {id} attached at {attachment_revision}. {}",
                snapshot.summary()
            ),
            Entry::Preview {
                destination,
                disclosure,
                ..
            } => disclosure.preview(id, &destination)?,
            Entry::Disclosure { payload } => format!(
                "Confirmed evidence for {}\n{}\nExplicit selected passage: {}",
                payload.question,
                payload
                    .captures
                    .iter()
                    .map(|(id, s)| format!("{id}: {}", s.summary()))
                    .collect::<Vec<_>>()
                    .join("\n"),
                serde_json::to_string(&payload.selected_passage)?
            ),
            _ => record.body_json.clone(),
        })
    }
    pub fn show_page(&self, id: &str, page: usize) -> Result<String> {
        let text = self.show(id)?;
        let mut rest = text.as_str();
        let mut pages = Vec::new();
        while !rest.is_empty() {
            let end = rest.floor_char_boundary(rest.len().min(DETAIL_PAGE_BYTES));
            pages.push(&rest[..end]);
            rest = &rest[end..];
        }
        let selected = page
            .checked_sub(1)
            .and_then(|i| pages.get(i))
            .context("observation page out of range")?;
        Ok(format!(
            "Record {id}, explicit detail page {page}/{} (UTF-8 fragment; all record bytes remain retained).\n{selected}\nUse the same show operation with page set explicitly to retrieve another page.",
            pages.len()
        ))
    }
    pub fn attachments(&self) -> String {
        self.records
            .iter()
            .filter(|r| matches!(r.entry(), Ok(Entry::Capture { .. })))
            .map(|r| r.id.as_str())
            .collect::<Vec<_>>()
            .join(", ")
    }
}
// JSON readers may round a final bit. Tolerate only numerical round-off, not changed structure.
fn equivalent_result(a: &ResultRecord, b: &ResultRecord) -> Result<bool> {
    fn compare(a: &serde_json::Value, b: &serde_json::Value) -> bool {
        match (a, b) {
            (serde_json::Value::Number(x), serde_json::Value::Number(y)) => {
                match (x.as_u64(), y.as_u64()) {
                    (Some(x), Some(y)) => x == y,
                    _ => x.as_f64().zip(y.as_f64()).is_some_and(|(x, y)| {
                        #[allow(clippy::arithmetic_side_effects)]
                        {
                            (x - y).abs() <= 1e-12
                        }
                    }),
                }
            },
            (serde_json::Value::Array(x), serde_json::Value::Array(y)) => {
                x.len() == y.len() && x.iter().zip(y).all(|(a, b)| compare(a, b))
            },
            (serde_json::Value::Object(x), serde_json::Value::Object(y)) => {
                x.len() == y.len()
                    && x.iter()
                        .all(|(k, v)| y.get(k).is_some_and(|b| compare(v, b)))
            },
            _ => a == b,
        }
    }
    Ok(compare(
        &serde_json::to_value(a)?,
        &serde_json::to_value(b)?,
    ))
}
