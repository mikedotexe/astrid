//! Focus references share the reader/draft transaction lock. No lock spans inference.
use super::{DeliveryReceipt, Reader, atomic_write, completion_text};
use crate::focus::{FocusState, Operation, Owner, Target, parse_start};
use anyhow::{Context as _, Result, bail, ensure};
use serde::{Deserialize, Serialize};
use std::{collections::BTreeMap, fs};

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(tag = "op", rename_all = "snake_case")]
pub enum Request {
    Status,
    /// Host-only recovery of a verified eligible choice, never public status.
    Next,
    /// One durable provider invocation claim; retries do not grant it again.
    Claim {
        job_id: String,
    },
    Command {
        action: String,
    },
    Admit {
        job_id: String,
        input_id: String,
        action: String,
    },
    Complete {
        job_id: String,
        input_id: String,
    },
    Failed {
        job_id: String,
    },
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct Response {
    pub schema_version: u32,
    pub owner: Owner,
    pub revision: u64,
    pub protected: bool,
    pub target: Option<Target>,
    pub target_revision: Option<String>,
    pub admitted: u8,
    pub remaining: u8,
    pub deadline_ms: Option<u64>,
    pub pending_job: Option<String>,
    pub return_command: Option<String>,
    pub reason: Option<String>,
    pub window_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub next_action: Option<String>,
    #[serde(default)]
    pub invocation_granted: bool,
    pub pending_input_id: Option<String>,
}

#[derive(Serialize, Deserialize)]
struct Checkpoint {
    focus: FocusState,
    admissions: BTreeMap<String, String>,
    requests: BTreeMap<String, String>,
    #[serde(default)]
    completed_input: Option<String>,
    #[serde(default)]
    claimed_jobs: std::collections::BTreeSet<String>,
}

impl Reader {
    fn activity_owner(&self) -> Result<Owner> {
        match self.runtime.as_ref().map(|r| r.being.as_str()) {
            Some("astrid") => Ok(Owner::Astrid),
            Some("minime") => Ok(Owner::Minime),
            _ => bail!("activity operations require a configured owner"),
        }
    }

    fn target_revision(&self, target: &Target) -> Result<String> {
        target.validate()?;
        match target {
            Target::Write(id) => crate::writing::Writer::new(self.directory.join("writing"))
                .target_revision_locked(id),
            Target::Question(id) => self.load()?.questions.target_revision(id),
        }
    }

    fn activity_view(&self, state: &FocusState, now_ms: u64) -> Result<Response> {
        let mut response = view(state, now_ms);
        if let Some(target) = &state.parked {
            response.return_command = Some(return_command(
                target,
                &self.target_revision(target)?,
                state.revision,
            ));
        }
        Ok(response)
    }

    fn select_target(&self, target: &Target, park: bool) -> Result<()> {
        match target {
            Target::Write(id) => {
                crate::writing::Writer::new(self.directory.join("writing")).select_locked(id, park)
            },
            Target::Question(id) => {
                let mut state = self.load()?;
                state.apply_question(if park {
                    crate::questions::QuestionCommand::Park(id.clone())
                } else {
                    crate::questions::QuestionCommand::Focus(id.clone())
                })?;
                if !park {
                    super::cursors::ReadingCursor::capture(&state)
                        .validate_position(&self.catalog)?;
                }
                self.save(&state)
            },
        }
    }

    fn activity_receipt(&self, id: &str) -> Result<DeliveryReceipt> {
        if id.starts_with("writing-") {
            return crate::writing::Writer::new(self.directory.join("writing")).receipt_locked(id);
        }
        let state = self.load()?;
        state
            .receipts
            .get(id)
            .or_else(|| state.last_navigation.as_ref().filter(|r| r.page_id == id))
            .cloned()
            .context("study delivery not committed")
    }

    fn delivered_next(&self, id: &str, target: &Target) -> Result<Option<String>> {
        let receipt = self.activity_receipt(id)?;
        let bytes = fs::read(&receipt.artifact_path)?;
        ensure!(
            crate::digest(&bytes) == receipt.artifact_sha256,
            "delivery artifact changed"
        );
        let artifact: serde_json::Value = serde_json::from_slice(&bytes)?;
        let response = artifact["response_json"]
            .as_str()
            .context("retained response missing")?;
        ensure!(
            crate::digest(response) == receipt.response_sha256,
            "response identity mismatch"
        );
        let text = completion_text(response)?;
        Ok(crate::response_choice::final_explicit_next(&text)
            .map(str::trim)
            .map(|a| {
                if matches!(target, Target::Write(_)) && a.eq_ignore_ascii_case("CONTINUE") {
                    "WRITE CONTINUE".into()
                } else {
                    a.into()
                }
            }))
    }

    fn validate_activity_input(&self, id: &str, target: &Target, action: &str) -> Result<()> {
        match target {
            Target::Write(draft) => crate::writing::Writer::new(self.directory.join("writing"))
                .validate_pending_locked(draft, id, action)?,
            Target::Question(question) => {
                let state = self.load_for_delivery(id)?;
                let identity = crate::digest(serde_json::to_vec(&crate::Command::parse(action)?)?);
                ensure!(
                    state.prepared_actions.get(id) == Some(&identity),
                    "study input does not match the prepared action; explicitly reselect legacy inputs"
                );
                ensure!(
                    state.cursor_owner.as_deref() == Some(question),
                    "input belongs to another question"
                );
                ensure!(
                    super::cursors::ReadingCursor::capture(&state).offers(id),
                    "input is not pending"
                );
                for page in state
                    .pending
                    .iter()
                    .chain(state.pending_session.iter().flat_map(|s| &s.session_pages))
                {
                    let source = self.catalog.resolve(&page.source)?;
                    ensure!(
                        crate::digest(fs::read(&source.path)?) == page.revision.sha256,
                        "source changed; explicitly reselect it before generating"
                    );
                }
            },
        }
        Ok(())
    }

    /// Host-only activity operations. Results contain references, not private prose.
    /// # Errors
    /// Invalid ownership, revision, delivery or storage fails closed.
    #[allow(clippy::too_many_lines)] // One transaction table keeps validation, native selection and commit ordered under one lock.
    pub fn activity(
        &self,
        request_id: &str,
        expected_revision: Option<u64>,
        now_ms: u64,
        request: Request,
    ) -> Result<Response> {
        let _lock = self.lock()?;
        let owner = self.activity_owner()?;
        let path = self.directory.join("activity-focus-v1.json");
        let marker = self.directory.join("activity-transition-v1.json");
        let mut saved: Checkpoint = if path.exists() {
            serde_json::from_slice(&fs::read(&path)?)
                .context("activity checkpoint corrupt; retained unchanged")?
        } else {
            Checkpoint {
                focus: FocusState::new(owner.clone()),
                admissions: BTreeMap::new(),
                requests: BTreeMap::new(),
                completed_input: None,
                claimed_jobs: std::collections::BTreeSet::new(),
            }
        };
        saved.focus.validate(&owner)?;
        if marker.exists() {
            let bytes = fs::read(&marker)?;
            let _: serde_json::Value = serde_json::from_slice(&bytes)
                .context("corrupt interrupted transition; retained")?;
            saved.focus.apply(
                &owner,
                &format!("recovery-{}", crate::digest(bytes)),
                now_ms,
                Operation::End,
            )?;
            atomic_write(&path, &serde_json::to_vec_pretty(&saved)?)?;
            fs::remove_file(&marker)?;
            fs::File::open(&self.directory)?.sync_all()?;
            bail!(
                "interrupted activity transition released priority; native work retained; inspect ACTIVITY_STATUS"
            );
        }
        // Persist clock observations before validating the requested operation.
        // Even a rejected admission must not resurrect priority after rollback.
        if saved.focus.window.as_ref().is_some_and(|w| {
            w.ended.is_none() && (now_ms < w.last_seen_ms || now_ms >= w.deadline_ms)
        }) {
            saved.focus.apply(
                &owner,
                &format!("clock-{}-{now_ms}", saved.focus.revision),
                now_ms,
                Operation::ObserveClock,
            )?;
            atomic_write(&path, &serde_json::to_vec_pretty(&saved)?)?;
        }
        if let Some(window) = &mut saved.focus.window
            && window.ended.is_none()
            && now_ms > window.last_seen_ms
        {
            // This watermark does not change the selected-work revision or budget.
            window.last_seen_ms = now_ms;
            atomic_write(&path, &serde_json::to_vec_pretty(&saved)?)?;
        }
        if matches!(request, Request::Status | Request::Next)
            || matches!(&request, Request::Command { action } if action == "ACTIVITY_STATUS")
        {
            let mut result = self.activity_view(&saved.focus, now_ms)?;
            result.pending_input_id = saved
                .focus
                .window
                .as_ref()
                .and_then(|w| w.running_job.as_ref())
                .and_then(|id| saved.admissions.get(id))
                .cloned();
            if matches!(request, Request::Next) && result.protected && result.pending_job.is_none()
            {
                let window = saved
                    .focus
                    .window
                    .as_ref()
                    .context("missing focus window")?;
                ensure!(
                    self.target_revision(&window.target)? == window.target_revision,
                    "selected work changed; explicit reselection required"
                );
                let next = if window.admitted == 0 {
                    Some(window.target.presentation())
                } else {
                    self.delivered_next(
                        saved
                            .completed_input
                            .as_deref()
                            .context("continuation receipt missing")?,
                        &window.target,
                    )?
                };
                ensure!(
                    next.as_ref().map(crate::digest) == window.next,
                    "continuation identity changed"
                );
                result.next_action = next;
            }
            return Ok(result);
        }
        ensure!(
            !request_id.is_empty() && request_id.len() <= 128,
            "bounded request ID required"
        );
        // A retry may query a newer revision after losing its response. The ID
        // binds its payload; revision guards only the first successful commit.
        let fingerprint = crate::digest(serde_json::to_vec(&request)?);
        if let Some(prior) = saved.requests.get(request_id) {
            ensure!(
                prior == &fingerprint,
                "conflicting activity operation retry"
            );
            return self.activity_view(&saved.focus, now_ms);
        }
        ensure!(
            expected_revision == Some(saved.focus.revision),
            "stale activity revision; inspect ACTIVITY_STATUS"
        );
        ensure!(
            saved.requests.len() < 65_536,
            "activity operation history full; retained unchanged"
        );
        let mut native_change = None;
        let mut invocation_granted = false;
        let op = match request {
            Request::Claim { job_id } => {
                ensure!(
                    saved
                        .focus
                        .window
                        .as_ref()
                        .and_then(|w| w.running_job.as_ref())
                        == Some(&job_id),
                    "provider claim does not match admitted job"
                );
                invocation_granted = saved.claimed_jobs.insert(job_id);
                Operation::ObserveClock
            },
            Request::Command { action } => match action.as_str() {
                "END_ACTIVITY_FOCUS" | "CHECK_MAILBOX" | "CHECK_MAILBOX LARGE" => Operation::End,
                "PARK_ACTIVITY" => {
                    let target = saved
                        .focus
                        .window
                        .as_ref()
                        .context("no selected activity")?
                        .target
                        .clone();
                    native_change = Some((target, true));
                    Operation::Park
                },
                command if command.starts_with("RETURN_ACTIVITY ") => {
                    let target = saved
                        .focus
                        .parked
                        .as_ref()
                        .context("no parked activity")?
                        .clone();
                    let revision = self.target_revision(&target)?;
                    ensure!(
                        command == return_command(&target, &revision, saved.focus.revision),
                        "stale return command; inspect ACTIVITY_STATUS"
                    );
                    native_change = Some((target, false));
                    Operation::Return {
                        expected_revision: saved.focus.revision,
                    }
                },
                command => {
                    let (target, turns) = parse_start(command)?;
                    let target_revision = self.target_revision(&target)?;
                    native_change = Some((target.clone(), false));
                    saved.completed_input = None;
                    Operation::Start {
                        target,
                        target_revision,
                        turns,
                    }
                },
            },
            Request::Admit {
                job_id,
                input_id,
                action,
            } => {
                let target = &saved.focus.window.as_ref().context("no focus")?.target;
                self.validate_activity_input(&input_id, target, &action)?;
                let revision = self.target_revision(target)?;
                if let Some(previous) = saved.admissions.get(&job_id) {
                    ensure!(previous == &input_id, "conflicting input admission");
                }
                saved.admissions.insert(job_id.clone(), input_id);
                Operation::Admit {
                    job_id,
                    action,
                    target_revision: revision,
                }
            },
            Request::Complete { job_id, input_id } => {
                ensure!(
                    saved.admissions.get(&job_id) == Some(&input_id),
                    "completion input was not admitted"
                );
                let target = &saved.focus.window.as_ref().context("no focus")?.target;
                let next = self.delivered_next(&input_id, target)?;
                saved.completed_input = Some(input_id);
                Operation::Complete {
                    job_id,
                    next,
                    target_revision: self.target_revision(target)?,
                }
            },
            Request::Failed { job_id } => {
                let target = &saved.focus.window.as_ref().context("no focus")?.target;
                Operation::Complete {
                    job_id,
                    next: None,
                    target_revision: self.target_revision(target)?,
                }
            },
            Request::Status | Request::Next => unreachable!(),
        };
        saved.focus.apply(&owner, request_id, now_ms, op)?;
        if let Some((target, park)) = native_change {
            // Recovery never guesses which half completed. The marker has only
            // identities; canonical prose and accepted deliveries stay native.
            atomic_write(
                &marker,
                &serde_json::to_vec(&serde_json::json!({"request_id":request_id,"target":target}))?,
            )?;
            self.select_target(&target, park)?;
        }
        saved.requests.insert(request_id.into(), fingerprint);
        atomic_write(&path, &serde_json::to_vec_pretty(&saved)?)?;
        if marker.exists() {
            fs::remove_file(&marker)?;
            fs::File::open(&self.directory)?.sync_all()?;
        }
        let mut result = self.activity_view(&saved.focus, now_ms)?;
        result.invocation_granted = invocation_granted;
        Ok(result)
    }
}

fn return_command(target: &Target, revision: &str, activity_revision: u64) -> String {
    let (kind, id) = match target {
        Target::Write(id) => ("WRITE", id),
        Target::Question(id) => ("QUESTION", id),
    };
    format!("RETURN_ACTIVITY {kind} {id} {revision} {activity_revision}")
}
fn view(state: &FocusState, now_ms: u64) -> Response {
    let w = state.window.as_ref();
    Response {
        schema_version: 1,
        owner: state.owner.clone(),
        revision: state.revision,
        protected: state.protected(now_ms),
        target: w.map(|w| w.target.clone()),
        target_revision: w.map(|w| w.target_revision.clone()),
        admitted: w.map_or(0, |w| w.admitted),
        remaining: w.map_or(0, |w| w.limit.saturating_sub(w.admitted)),
        deadline_ms: w.map(|w| w.deadline_ms),
        pending_job: w.and_then(|w| w.running_job.clone()),
        return_command: None,
        reason: w.and_then(|w| w.ended.clone()),
        window_id: w.map(|w| w.id.clone()),
        next_action: None,
        invocation_granted: false,
        pending_input_id: None,
    }
}
