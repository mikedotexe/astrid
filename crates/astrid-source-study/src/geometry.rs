//! Explicit, question-owned observations. No inference, input injection or control endpoints.
#![allow(clippy::arithmetic_side_effects)] // Bounded finite coordinates and checked recorder clocks.
use anyhow::{Context as _, Result, bail, ensure};
use serde::{Deserialize, Serialize};
use std::path::Path;

pub(crate) const FORMAT: &str = "question-geometry-v1";
pub(crate) const RECIPE: &str = "mean-state-rms-distance-v1";

#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct GeometryRequest {
    pub question: String,
    #[serde(default)]
    pub request_id: String,
    #[serde(default)]
    pub expected_head: String,
    pub operation: Operation,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub(crate) enum Operation {
    Status,
    Show {
        id: String,
    },
    Export,
    Capture {
        seconds: u64,
        note: String,
    },
    Predict {
        baseline: String,
        maximum_rms_distance: f64,
        expectation: String,
    },
    Compare {
        prediction: String,
        observation: String,
    },
    Revise {
        target: String,
        text: String,
    },
}

#[derive(Default, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct History {
    pub owner: Option<String>,
    pub records: Vec<Record>,
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct Record {
    pub id: String,
    pub previous: String,
    pub request_id: String,
    pub request_sha256: String,
    /// Hash exact UTF-8 bytes, not a re-serialized floating-point representation.
    pub body_json: String,
}

#[derive(Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub(crate) enum Entry {
    Capture {
        snapshot: Snapshot,
        note: String,
    },
    Prediction {
        baseline: String,
        maximum_rms_distance: f64,
        expectation: String,
    },
    Comparison {
        prediction: String,
        observation: String,
        recipe: String,
        rms_distance: f64,
        threshold_met: bool,
    },
    Revision {
        target: String,
        text: String,
    },
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct Snapshot {
    pub source_sha256: String,
    pub captured_at_unix_ms: u64,
    pub requested_seconds: u64,
    pub source: String,
    pub scope: String,
    pub identity: String,
    pub frames: Vec<Frame>,
}

#[derive(Clone, Serialize, Deserialize)]
pub(crate) struct Frame {
    pub t_ms: u64,
    pub wall_clock_unix_ms: u64,
    pub activations: Vec<f64>,
}

#[derive(Deserialize)]
struct Trace {
    policy: String,
    reservoir_dim: usize,
    sample_interval_ms: u64,
    retained_secs: u64,
    updated_at_unix_ms: u64,
    frames: Vec<TraceFrame>,
}

#[derive(Deserialize)]
struct TraceFrame {
    #[serde(flatten)]
    frame: Frame,
    summary: Summary,
}
#[derive(Deserialize)]
struct Summary {
    finite_fraction: f64,
}

impl Snapshot {
    pub fn capture(root: &Path, seconds: u64, now: u64) -> Result<Self> {
        use std::io::Read as _;
        ensure!((1..=60).contains(&seconds), "choose 1 through 60 seconds");
        let root = root.canonicalize()?;
        let path = root.join("workspace/runtime/esn_activation_trace_v1.json");
        ensure!(
            path.canonicalize()? == path,
            "activation source must not traverse symlinks"
        );
        let file = std::fs::File::open(&path)?;
        ensure!(
            file.metadata()?.is_file(),
            "activation source must be a regular file"
        );
        let mut bytes = Vec::new();
        file.take(2_097_153).read_to_end(&mut bytes)?;
        ensure!(bytes.len() <= 2_097_152, "activation source exceeds 2 MiB");
        let trace: Trace = serde_json::from_slice(&bytes)?;
        ensure!(
            trace.policy == "esn_activation_trace_v1"
                && trace.reservoir_dim == 128
                && trace.sample_interval_ms == 1000
                && trace.retained_secs == 180,
            "unsupported activation recorder policy"
        );
        ensure!(
            !trace.frames.is_empty() && trace.frames.len() <= 180,
            "invalid recorder frame count"
        );
        ensure!(
            trace
                .frames
                .iter()
                .all(|f| f.summary.finite_fraction.to_bits() == 1.0_f64.to_bits()),
            "sanitized/nonfinite source is not measured geometry"
        );
        let frames: Vec<_> = trace.frames.into_iter().map(|f| f.frame).collect();
        validate_frames(&frames)?;
        let last = frames.last().context("empty recorder")?;
        ensure!(
            trace.updated_at_unix_ms == last.wall_clock_unix_ms,
            "source update clock mismatch"
        );
        ensure!(
            now >= last.wall_clock_unix_ms && now - last.wall_clock_unix_ms <= 12_000,
            "source is stale or ahead of host clock; no fresh capture recorded"
        );
        let cutoff = last.t_ms.saturating_sub(seconds.saturating_mul(1000));
        let snapshot = Self {
            source_sha256: crate::digest(&bytes),
            captured_at_unix_ms: now,
            requested_seconds: seconds,
            source: "minime/workspace/runtime/esn_activation_trace_v1.json".into(),
            scope: "native_esn_128_activations".into(),
            identity: "boot_and_node_layout_unverified".into(),
            frames: frames.into_iter().filter(|f| f.t_ms >= cutoff).collect(),
        };
        snapshot.validate()?;
        Ok(snapshot)
    }

    fn validate(&self) -> Result<()> {
        ensure!(
            (1..=60).contains(&self.requested_seconds) && (1..=61).contains(&self.frames.len()),
            "invalid frozen interval bounds"
        );
        ensure!(
            self.scope == "native_esn_128_activations"
                && self.identity == "boot_and_node_layout_unverified"
                && self.source == "minime/workspace/runtime/esn_activation_trace_v1.json"
                && valid_hash(&self.source_sha256),
            "unsupported geometry provenance"
        );
        validate_frames(&self.frames)?;
        let first = &self.frames[0];
        let last = self.frames.last().context("empty interval")?;
        ensure!(
            last.t_ms - first.t_ms <= self.requested_seconds * 1000,
            "frozen interval exceeds request"
        );
        ensure!(
            self.captured_at_unix_ms >= last.wall_clock_unix_ms
                && self.captured_at_unix_ms - last.wall_clock_unix_ms <= 12_000,
            "invalid capture clock"
        );
        Ok(())
    }

    fn means(&self) -> Vec<f64> {
        #[allow(clippy::cast_precision_loss)]
        let count = self.frames.len() as f64;
        (0..128)
            .map(|i| self.frames.iter().map(|f| f.activations[i]).sum::<f64>() / count)
            .collect()
    }

    pub fn summary(&self) -> String {
        let first = &self.frames[0];
        let last = &self.frames[self.frames.len() - 1];
        let gaps: Vec<_> = self
            .frames
            .windows(2)
            .filter(|p| p[1].t_ms - p[0].t_ms > 1000)
            .map(|p| format!("{}..{}", p[0].t_ms, p[1].t_ms))
            .collect();
        format!(
            "{} exact 128-node frames; engine interval {}..{} ms ({} ms); wall interval {}..{} ms; requested {} s. Gaps over nominal 1000 ms: {:?}. Source SHA256 {}. Boot/node layout unverified. No interpolation or covariance eigenvectors.",
            self.frames.len(),
            first.t_ms,
            last.t_ms,
            last.t_ms - first.t_ms,
            first.wall_clock_unix_ms,
            last.wall_clock_unix_ms,
            self.requested_seconds,
            gaps,
            self.source_sha256
        )
    }
}

fn validate_frames(frames: &[Frame]) -> Result<()> {
    for frame in frames {
        ensure!(
            frame.activations.len() == 128
                && frame
                    .activations
                    .iter()
                    .all(|x| x.is_finite() && (-1.0..=1.0).contains(x)),
            "invalid 128-node activation vector"
        );
    }
    for pair in frames.windows(2) {
        ensure!(
            pair[1].t_ms > pair[0].t_ms
                && pair[1].t_ms - pair[0].t_ms >= 1000
                && pair[1].wall_clock_unix_ms > pair[0].wall_clock_unix_ms,
            "duplicate, reversed or underspaced source clocks"
        );
    }
    Ok(())
}

fn valid_hash(s: &str) -> bool {
    s.len() == 64 && s.bytes().all(|b| b.is_ascii_hexdigit())
}
fn prose(s: &str) -> Result<()> {
    ensure!(
        !s.trim().is_empty() && s.len() <= 2000,
        "authored text must be 1..2000 bytes; no truncation"
    );
    Ok(())
}

impl Record {
    fn identity(&self) -> String {
        crate::digest(format!(
            "{}\n{}\n{}\n{}",
            self.previous,
            crate::digest(&self.request_id),
            self.request_sha256,
            crate::digest(&self.body_json)
        ))
    }
    fn entry(&self) -> Result<Entry> {
        Ok(serde_json::from_str(&self.body_json)?)
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
            "geometry owner mismatch"
        );
        Ok(())
    }
    pub fn validate(&self) -> Result<()> {
        ensure!(self.records.len() <= 64, "geometry history limit exceeded");
        if !self.records.is_empty() {
            self.check_owner(self.owner.as_deref().context("geometry owner missing")?)?;
        }
        let mut prior = Self::default();
        let mut captures = 0;
        for record in &self.records {
            ensure!(
                record.previous == prior.head()
                    && record.identity() == record.id
                    && valid_hash(&record.request_sha256),
                "corrupt geometry history; preserving bytes"
            );
            ensure!(
                !prior
                    .records
                    .iter()
                    .any(|r| r.request_id == record.request_id),
                "duplicate geometry operation ID"
            );
            let entry = record.entry()?;
            prior.validate_entry(&entry)?;
            if matches!(entry, Entry::Capture { .. }) {
                captures += 1;
            }
            // Copy exact record bytes; never rewrite authored entries during validation.
            prior
                .records
                .push(serde_json::from_value(serde_json::to_value(record)?)?);
        }
        ensure!(
            captures <= 4,
            "four frozen intervals per question; existing evidence retained"
        );
        Ok(())
    }
    fn validate_entry(&self, entry: &Entry) -> Result<()> {
        match entry {
            Entry::Capture { snapshot, note } => {
                snapshot.validate()?;
                prose(note)?;
            },
            Entry::Prediction {
                baseline,
                maximum_rms_distance,
                expectation,
            } => {
                self.snapshot(baseline)?;
                ensure!(
                    maximum_rms_distance.is_finite() && (0.0..=2.0).contains(maximum_rms_distance),
                    "RMS bound must be finite, 0..2"
                );
                prose(expectation)?;
            },
            Entry::Comparison {
                prediction,
                observation,
                recipe,
                rms_distance,
                threshold_met,
            } => {
                let (distance, matched) = self.compare(prediction, observation)?;
                ensure!(
                    recipe == RECIPE
                        && rms_distance.is_finite()
                        && (distance - rms_distance).abs() <= 1e-12
                        && *threshold_met == matched,
                    "comparison receipt does not match frozen evidence"
                );
            },
            Entry::Revision { target, text } => {
                self.record(target)?;
                prose(text)?;
            },
        }
        Ok(())
    }
    fn record(&self, id: &str) -> Result<&Record> {
        self.records
            .iter()
            .find(|r| r.id == id)
            .context("exact existing geometry record ID required")
    }
    fn snapshot(&self, id: &str) -> Result<Snapshot> {
        match self.record(id)?.entry()? {
            Entry::Capture { snapshot, .. } => Ok(snapshot),
            _ => bail!("capture ID required"),
        }
    }
    fn compare(&self, prediction: &str, observation: &str) -> Result<(f64, bool)> {
        let Entry::Prediction {
            baseline,
            maximum_rms_distance,
            ..
        } = self.record(prediction)?.entry()?
        else {
            bail!("prediction ID required")
        };
        let prediction_index = self
            .records
            .iter()
            .position(|r| r.id == prediction)
            .context("prediction missing")?;
        let observation_index = self
            .records
            .iter()
            .position(|r| r.id == observation)
            .context("observation missing")?;
        ensure!(
            observation_index > prediction_index,
            "prediction must precede the second capture, not merely comparison"
        );
        let a = self.snapshot(&baseline)?;
        let b = self.snapshot(observation)?;
        ensure!(
            a.frames.len() >= 2 && b.frames.len() >= 2,
            "insufficient: each interval needs at least two recorded states"
        );
        ensure!(
            b.frames[0].wall_clock_unix_ms
                > a.frames
                    .last()
                    .context("empty baseline")?
                    .wall_clock_unix_ms
                && b.frames[0].t_ms > a.frames.last().context("empty baseline")?.t_ms,
            "insufficient: intervals overlap or clocks restarted"
        );
        let distance = (a
            .means()
            .iter()
            .zip(b.means())
            .map(|(a, b)| (a - b).powi(2))
            .sum::<f64>()
            / 128.0)
            .sqrt();
        Ok((distance, distance <= maximum_rms_distance))
    }
    pub fn apply(
        &mut self,
        owner: &str,
        request: &GeometryRequest,
        root: &Path,
        now: u64,
    ) -> Result<String> {
        self.check_owner(owner)?;
        self.validate()?;
        let fingerprint = crate::digest(serde_json::to_vec(request)?);
        if let Some(record) = self
            .records
            .iter()
            .find(|r| r.request_id == request.request_id)
        {
            ensure!(
                record.request_sha256 == fingerprint,
                "conflicting geometry retry; history unchanged"
            );
            return self.show(&record.id);
        }
        ensure!(
            !request.request_id.is_empty() && request.request_id.len() <= 128,
            "mutation requires a 1..128 byte request_id"
        );
        ensure!(
            request.expected_head == self.head(),
            "stale geometry head; STATUS then explicitly retry"
        );
        ensure!(
            self.records.len() < 64,
            "64 records retained; history will not be truncated"
        );
        let entry = match &request.operation {
            Operation::Capture { seconds, note } => {
                ensure!(
                    self.records
                        .iter()
                        .filter(|r| matches!(r.entry(), Ok(Entry::Capture { .. })))
                        .count()
                        < 4,
                    "four intervals retained; no evidence discarded"
                );
                prose(note)?;
                Entry::Capture {
                    snapshot: Snapshot::capture(root, *seconds, now)?,
                    note: note.clone(),
                }
            },
            Operation::Predict {
                baseline,
                maximum_rms_distance,
                expectation,
            } => Entry::Prediction {
                baseline: baseline.clone(),
                maximum_rms_distance: *maximum_rms_distance,
                expectation: expectation.clone(),
            },
            Operation::Compare {
                prediction,
                observation,
            } => {
                let (rms_distance, threshold_met) = self.compare(prediction, observation)?;
                Entry::Comparison {
                    prediction: prediction.clone(),
                    observation: observation.clone(),
                    recipe: RECIPE.into(),
                    rms_distance,
                    threshold_met,
                }
            },
            Operation::Revise { target, text } => Entry::Revision {
                target: target.clone(),
                text: text.clone(),
            },
            _ => bail!("read-only operation is not a mutation"),
        };
        self.validate_entry(&entry)?;
        let mut record = Record {
            id: String::new(),
            previous: self.head().into(),
            request_id: request.request_id.clone(),
            request_sha256: fingerprint,
            body_json: serde_json::to_string(&entry)?,
        };
        record.id = record.identity();
        let id = record.id.clone();
        self.owner = Some(owner.into());
        self.records.push(record);
        self.show(&id)
    }
    pub fn show(&self, id: &str) -> Result<String> {
        let record = self.record(id)?;
        let detail = match record.entry()? {
            Entry::Capture { snapshot, note } => {
                format!("Authored note (verbatim): {note}\n{}", snapshot.summary())
            },
            _ => record.body_json.clone(),
        };
        Ok(format!(
            "Geometry record {}\n{}\nHead {}\nNumerical threshold only; boot/layout identity unknown, no causal or felt-state inference. Commitment before capture does not prove the result was previously unseen. Full vectors are retained for explicit EXPORT, not expanded into the prompt.",
            record.id,
            detail,
            self.head()
        ))
    }
    pub fn status(&self) -> String {
        let rows = self
            .records
            .iter()
            .map(|r| {
                format!(
                    "{} {}",
                    r.id,
                    r.entry().map_or("invalid", |e| match e {
                        Entry::Capture { .. } => "capture",
                        Entry::Prediction { .. } => "prediction",
                        Entry::Comparison { .. } => "comparison",
                        Entry::Revision { .. } => "revision",
                    })
                )
            })
            .collect::<Vec<_>>()
            .join("\n");
        format!(
            "Geometry head: {}\n{}\nExplicit SHOW/EXPORT only. Parking the question does not erase this history or trigger reminders.\nOperations are typed JSON after SELF_STUDY GEOMETRY with question qN and operation. Read operations: {{\"kind\":\"status\"}}, {{\"kind\":\"show\",\"id\":\"exact record ID\"}}, {{\"kind\":\"export\"}}. Mutations also require a unique request_id and expected_head equal to this head.\nCapture: {{\"kind\":\"capture\",\"seconds\":10,\"note\":\"your exact words\"}}. Predict: {{\"kind\":\"predict\",\"baseline\":\"capture ID\",\"maximum_rms_distance\":0.1,\"expectation\":\"your expectation\"}}. Compare after a later nonoverlapping capture: {{\"kind\":\"compare\",\"prediction\":\"prediction ID\",\"observation\":\"later capture ID\"}}. Revise: {{\"kind\":\"revise\",\"target\":\"earlier record ID\",\"text\":\"your qualification or challenge\"}}.\nComparison is the RMS difference of unweighted mean 128-node vectors, not a time-weighted or causal test; gaps and sampling selection can change it. Four intervals of at most 60 s and 64 records per question; overflow fails without erasure.",
            self.head(),
            rows
        )
    }
    pub fn export(&self, owner: &str, question_id: &str, question: &str) -> Result<Vec<u8>> {
        self.validate()?;
        self.check_owner(owner)?;
        let body_json = serde_json::to_string(
            &serde_json::json!({"format":FORMAT,"owner":owner,"question_id":question_id,"question":question,"history":self,
            "limits":"Observational coordinate comparison only. Boot and node-layout identity unverified. No causal, eigenmode or experiential claim. Hashes check byte integrity, not producer authenticity."}),
        )?;
        Ok(serde_json::to_vec_pretty(
            &serde_json::json!({"format":FORMAT,"body_sha256":crate::digest(&body_json),"body_json":body_json}),
        )?)
    }
}
