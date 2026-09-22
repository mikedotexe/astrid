use super::*;
use crate::observations::{Disclosure, Entry, ObservationRequest as Request, Operation};
use anyhow::ensure;

fn now() -> Result<u64> {
    Ok(u64::try_from(
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)?
            .as_millis(),
    )?)
}
impl Reader {
    #[allow(clippy::too_many_lines)] // One allowlisted private transaction, including its two-checkpoint confirmation boundary.
    pub(super) fn prepare_observation(&self, json: &str, action: &str) -> Result<StudyOutput> {
        let _lock = self.lock()?;
        ensure!(
            crate::preparation::in_transaction(),
            "observation operations require prepare_once transaction"
        );
        let request: Request = serde_json::from_str(json)?;
        let owner = &self.runtime.as_ref().context("host owner required")?.being;
        ensure!(*owner == request.owner, "observation owner mismatch");
        let writer = crate::writing::Writer::new(self.directory.join("writing"));
        let (revision, prose, mut history) = writer.observation_context(&request.draft)?;
        history.check_owner(owner)?;
        let fingerprint = digest(serde_json::to_vec(&request)?);
        if let Some(record) = history.retry(&request.request_id, &fingerprint)? {
            let text = format!(
                "Already committed {}. Head {}. Revision {}. Explicit show opens details.",
                record.id,
                history.head(),
                revision
            );
            return writer.observation_output(text, false, action, None);
        }
        let mut state = self.load()?;
        let mut presented_preview = None;
        let text = match &request.operation {
            Operation::Status => format!(
                "Draft {} revision {}\nObservation head {}\n{}\nFour captures and 64 records; nothing evicted. No parked reminders.\n{}\n{}\n{}",
                request.draft,
                revision,
                history.head(),
                history.compact(),
                crate::observations::PRIVATE_HELP,
                crate::observations::ANALYSIS_HELP,
                crate::recurrence::LIMITS
            ),
            Operation::Show { id, page } => {
                if matches!(history.record(id)?.entry()?, Entry::Preview { .. }) {
                    presented_preview = Some(id.clone());
                }
                history.show_page(id, *page)?
            },
            operation => {
                ensure!(
                    request.revision == revision,
                    "stale draft revision; inspect and reselect explicitly"
                );
                ensure!(
                    request.expected_head == history.head(),
                    "stale observation head; inspect and reselect explicitly"
                );
                ensure!(
                    history.records.len() < 64,
                    "64 observation records retained; no eviction"
                );
                let entry = match operation {
                    Operation::Capture { seconds } => {
                        let root = self
                            .catalog
                            .roots
                            .get("minime")
                            .context("trusted Minime root required")?;
                        Entry::Capture {
                            snapshot: crate::geometry::Snapshot::capture_observation(
                                root,
                                *seconds,
                                now()?,
                            )?,
                            attachment_revision: revision.clone(),
                        }
                    },
                    Operation::Annotate {
                        target,
                        text,
                        expectation,
                    } => {
                        if let Some(definition) = expectation {
                            Entry::Expectation {
                                target: target.clone(),
                                text: text.clone(),
                                definition: definition.clone(),
                                authored_at_unix_ms: now()?,
                            }
                        } else {
                            Entry::Annotation {
                                target: target.clone(),
                                text: text.clone(),
                            }
                        }
                    },
                    Operation::Analyze {
                        analysis,
                        expectation,
                    } => history.analysis_entry(analysis, expectation.as_deref(), now()?)?,
                    Operation::LinkPreview {
                        captures,
                        destination,
                        passage,
                    } => {
                        ensure!(
                            (1..=4).contains(&captures.len()),
                            "select one through four captures"
                        );
                        let question = state.questions.observation_destination(destination)?;
                        let selected_passage = passage
                            .as_ref()
                            .map(|selection| -> Result<String> {
                                ensure!(
                                    digest(&prose) == selection.source_sha256,
                                    "passage source changed; preview again"
                                );
                                ensure!(
                                    selection.start_byte < selection.end_byte,
                                    "select a nonempty passage"
                                );
                                let exact = prose
                                    .get(selection.start_byte..selection.end_byte)
                                    .context(
                                        "passage must use exact UTF-8 boundaries within this draft",
                                    )?;
                                crate::observations::text(exact, 24_000)?;
                                Ok(exact.into())
                            })
                            .transpose()?;
                        let selected = captures
                            .iter()
                            .map(|id| Ok((id.clone(), history.snapshot(id)?)))
                            .collect::<Result<BTreeMap<_, _>>>()?;
                        ensure!(
                            selected.len() == captures.len(),
                            "duplicate selected capture"
                        );
                        Entry::Preview {
                            destination: destination.clone(),
                            draft_revision: revision.clone(),
                            disclosure: Disclosure {
                                question,
                                captures: selected,
                                selected_passage,
                            },
                        }
                    },
                    Operation::LinkConfirm { preview } => {
                        writer.verify_preview_delivery(preview)?;
                        let Entry::Preview {
                            destination,
                            draft_revision,
                            disclosure,
                        } = history.record(preview)?.entry()?
                        else {
                            bail!("preview ID required")
                        };
                        ensure!(
                            draft_revision == revision,
                            "preview source changed; preview again"
                        );
                        ensure!(
                            state.questions.observation_destination(&destination)?
                                == disclosure.question,
                            "preview destination changed; preview again"
                        );
                        let (question, inquiry_record) = state.questions.confirm_observation(
                            owner,
                            &request.request_id,
                            &destination,
                            disclosure,
                        )?;
                        // Both checkpoints publish through the same durable preparation redo.
                        self.save(&state)?;
                        Entry::Confirmed {
                            preview: preview.clone(),
                            question,
                            inquiry_record,
                        }
                    },
                    _ => unreachable!("reads handled above"),
                };
                let id = history.append(owner, &request.request_id, &fingerprint, entry, true)?;
                if matches!(operation, Operation::LinkPreview { .. }) {
                    presented_preview = Some(id.clone());
                }
                writer.save_observations(&request.draft, &revision, history.clone())?;
                if request.present {
                    history.show_page(&id, 1)?
                } else if let Entry::Confirmed {
                    question,
                    inquiry_record,
                    ..
                } = history.record(&id)?.entry()?
                {
                    format!(
                        "Private confirmation {id}. Inquiry {question}; disclosed record {inquiry_record}. No generation or inquiry selection requested."
                    )
                } else {
                    format!(
                        "Private observation operation saved as {id}. Head {}. No generation, inquiry selection or public entry requested. Explicit show opens details.",
                        history.head()
                    )
                }
            },
        };
        if !request.present {
            self.finish_observation_metadata(action)?;
        }
        writer.observation_output(
            format!("{text}\nDraft source SHA256 {}", digest(&prose)),
            request.present,
            action,
            presented_preview,
        )
    }

    fn finish_observation_metadata(&self, action: &str) -> Result<()> {
        if !crate::preparation::exists(&self.directory.join("activity-focus-v1.json")) {
            return Ok(());
        }
        let at = now()?;
        let status = self.activity("observation-status", None, at, ActivityRequest::Status)?;
        if status.protected {
            // Storage creates no new authored NEXT. End priority without spending or restoring a slot.
            let id = format!(
                "observation-stop-{}",
                digest(serde_json::to_vec(&(&status.window_id, action))?)
            );
            self.activity(
                &id,
                Some(status.revision),
                at,
                ActivityRequest::Command {
                    action: "END_ACTIVITY_FOCUS".into(),
                },
            )?;
        }
        Ok(())
    }

    pub(super) fn prepare_inquiry_observation(&self, json: &str) -> Result<StudyOutput> {
        let _lock = self.lock()?;
        ensure!(
            crate::preparation::in_transaction(),
            "inquiry observations require prepare_once transaction"
        );
        let request: InquiryRequest = serde_json::from_str(json)?;
        let owner = &self.runtime.as_ref().context("host owner required")?.being;
        ensure!(*owner == request.owner, "inquiry owner mismatch");
        let mut state = self.load()?;
        ensure!(
            !request.present || state.questions.active.as_deref() == Some(&request.question),
            "select this inquiry before requesting a public presentation"
        );
        let fingerprint = digest(serde_json::to_vec(&request)?);
        let revision = state.questions.target_revision(&request.question)?;
        let (question, history) = state.questions.observations(&request.question)?;
        history.check_owner(owner)?;
        history.validate(false)?;
        let text = if let Some(record) = history.retry(&request.request_id, &fingerprint)? {
            history.show_page(&record.id, 1)?
        } else {
            match &request.operation {
                InquiryOperation::Status => format!(
                    "Confirmed observation history for {}: {}\nRevision {revision}\nHead {}\n{}\n{}\n{}",
                    request.question,
                    question,
                    history.head(),
                    history.compact(),
                    crate::observations::INQUIRY_HELP,
                    crate::observations::ANALYSIS_HELP
                ),
                InquiryOperation::Show { id, page } => history.show_page(id, *page)?,
                InquiryOperation::Export => {
                    let bytes = state
                        .questions
                        .observation_export(owner, &request.question)?;
                    let path = self
                        .directory
                        .join("geometry-exports")
                        .join(format!("{}.json", digest(&bytes)));
                    atomic_write(&path, &bytes)?;
                    format!(
                        "Explicit inquiry-observations-v2 export: {} SHA256 {}. Includes confirmed inquiry material and legacy geometry, never private draft history.",
                        path.display(),
                        digest(&bytes)
                    )
                },
                operation => {
                    ensure!(
                        request.expected_head == history.head(),
                        "stale inquiry observation head"
                    );
                    let entry = match operation {
                        InquiryOperation::Analyze {
                            analysis,
                            expectation,
                        } => history.analysis_entry(analysis, expectation.as_deref(), now()?)?,
                        InquiryOperation::Predict {
                            target,
                            text,
                            definition,
                        } => Entry::Expectation {
                            target: target.clone(),
                            text: text.clone(),
                            definition: definition.clone(),
                            authored_at_unix_ms: now()?,
                        },
                        InquiryOperation::Revise { target, text } => Entry::Annotation {
                            target: target.clone(),
                            text: text.clone(),
                        },
                        _ => unreachable!("reads handled above"),
                    };
                    let id =
                        history.append(owner, &request.request_id, &fingerprint, entry, false)?;
                    history.show_page(&id, 1)?
                },
            }
        };
        self.save(&state)?;
        if !request.present {
            self.finish_observation_metadata(json)?;
            return Ok(StudyOutput {
                generation_requested: false,
                input_kind: InputKind::Geometry,
                evidence_scope: crate::recurrence::LIMITS.into(),
                require_complete_input: true,
                system_prompt: crate::STUDY_PROMPT.into(),
                input_budget_bytes: crate::MAX_INPUT_BYTES,
                context_tokens: crate::CONTEXT_TOKENS,
                text,
                page: None,
                session_pages: Vec::new(),
                question_id: Some(request.question),
                navigation_id: None,
            });
        }
        // No question selection or cursor change; public presentation is explicit.
        let mut output = self.output(&mut state, text, None, InputKind::Geometry)?;
        output.generation_requested = request.present;
        Ok(output)
    }
}
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct InquiryRequest {
    owner: String,
    question: String,
    #[serde(default)]
    request_id: String,
    #[serde(default)]
    expected_head: String,
    #[serde(default)]
    present: bool,
    operation: InquiryOperation,
}

/// Determine presentation intent using the same typed grammar as execution.
/// # Errors
/// Malformed observation requests fail privately before model admission.
pub fn observation_presentation_requested(action: &str) -> Result<Option<bool>> {
    if let Some(json) = action.trim().strip_prefix("WRITE OBSERVE ") {
        return Ok(Some(serde_json::from_str::<Request>(json)?.present));
    }
    if let Some(json) = action.trim().strip_prefix("SELF_STUDY OBSERVE ") {
        return Ok(Some(serde_json::from_str::<InquiryRequest>(json)?.present));
    }
    Ok(None)
}
#[derive(Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
enum InquiryOperation {
    Status,
    Show {
        id: String,
        #[serde(default = "crate::observations::first_page")]
        page: usize,
    },
    Export,
    Analyze {
        analysis: crate::recurrence::Analysis,
        #[serde(default)]
        expectation: Option<String>,
    },
    Predict {
        target: String,
        text: String,
        definition: crate::observation_expectations::Expectation,
    },
    Revise {
        target: String,
        text: String,
    },
}
