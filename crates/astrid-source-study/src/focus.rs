//! Bounded priority for an explicitly selected activity, never control authority.
use anyhow::{Context as _, Result, bail, ensure};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

pub const MAX_JOBS: u8 = 4;
pub const WINDOW_MS: u64 = 15 * 60 * 1000;
pub const GUIDANCE: &str = "ACTIVITY_FOCUS WRITE dN [turns N] or ACTIVITY_FOCUS QUESTION qN [turns N] protects existing work for up to four generation jobs or fifteen minutes. Only an explicit eligible NEXT continues it. ACTIVITY_STATUS supplies exact return commands. PARK_ACTIVITY keeps work quiet; RETURN_ACTIVITY restores it without renewing protection. END_ACTIVITY_FOCUS releases priority. CHECK_MAILBOX ends protection and restores ordinary receive policy.";

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Owner {
    Astrid,
    Minime,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", content = "id", rename_all = "snake_case")]
pub enum Target {
    Write(String),
    Question(String),
}
impl Target {
    /// # Errors
    /// Rejects anything other than one canonical native draft/question ID.
    pub fn validate(&self) -> Result<()> {
        let (id, prefix) = match self {
            Self::Write(id) => (id, 'd'),
            Self::Question(id) => (id, 'q'),
        };
        let number = id.strip_prefix(prefix).context("invalid activity target")?;
        ensure!(
            number.parse::<u64>().is_ok_and(|n| n > 0)
                && !number.starts_with('0')
                && number.bytes().all(|c| c.is_ascii_digit()),
            "invalid activity target"
        );
        Ok(())
    }
    #[must_use]
    pub fn presentation(&self) -> String {
        match self {
            Self::Write(id) => format!("WRITE RESUME {id}"),
            Self::Question(_) => "SELF_STUDY CONTINUE".into(),
        }
    }
    #[must_use]
    pub fn permits(&self, action: &str) -> bool {
        let words: Vec<_> = action.split_whitespace().collect();
        match self {
            Self::Write(id) => {
                matches!(
                    words.as_slice(),
                    ["WRITE", "CONTINUE"] | ["WRITE", "REVISE", ..]
                ) || action
                    .trim()
                    .strip_prefix("WRITE OBSERVE ")
                    .and_then(|json| {
                        serde_json::from_str::<crate::observations::ObservationRequest>(json).ok()
                    })
                    .is_some_and(|request| request.draft == *id)
            },
            Self::Question(_) => {
                words.first() == Some(&"SELF_STUDY") && !matches!(words.get(1), Some(&"QUESTION"))
            },
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Window {
    pub id: String,
    pub target: Target,
    pub target_revision: String,
    pub created_ms: u64,
    pub deadline_ms: u64,
    pub last_seen_ms: u64,
    pub limit: u8,
    pub admitted: u8,
    /// Hash only: exact authored commands stay in native delivery artifacts.
    pub next: Option<String>,
    pub running_job: Option<String>,
    pub ended: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct FocusState {
    pub schema_version: u32,
    pub owner: Owner,
    pub window: Option<Window>,
    pub parked: Option<Target>,
    pub revision: u64,
    requests: BTreeMap<String, String>,
    jobs: BTreeMap<String, String>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "op", rename_all = "snake_case")]
pub enum Operation {
    Start {
        target: Target,
        target_revision: String,
        turns: u8,
    },
    Admit {
        job_id: String,
        action: String,
        target_revision: String,
    },
    Complete {
        job_id: String,
        next: Option<String>,
        target_revision: String,
    },
    End,
    Park,
    Return {
        expected_revision: u64,
    },
    ObserveClock,
}

impl FocusState {
    #[must_use]
    pub fn new(owner: Owner) -> Self {
        Self {
            schema_version: 1,
            owner,
            window: None,
            parked: None,
            revision: 0,
            requests: BTreeMap::new(),
            jobs: BTreeMap::new(),
        }
    }

    /// # Errors
    /// Rejects other owners, unsupported versions and invalid clocks/budgets.
    pub fn validate(&self, owner: &Owner) -> Result<()> {
        ensure!(
            self.schema_version == 1 && &self.owner == owner,
            "unsupported or cross-owner activity checkpoint"
        );
        if let Some(w) = &self.window {
            w.target.validate()?;
            ensure!(
                (1..=MAX_JOBS).contains(&w.limit)
                    && w.admitted <= w.limit
                    && w.created_ms.checked_add(WINDOW_MS) == Some(w.deadline_ms)
                    && w.last_seen_ms >= w.created_ms,
                "invalid focus budget or clock"
            );
        }
        if let Some(target) = &self.parked {
            target.validate()?;
        }
        Ok(())
    }

    /// Queries do not renew the window. Backward clocks remove priority.
    #[must_use]
    pub fn protected(&self, now_ms: u64) -> bool {
        self.window.as_ref().is_some_and(|w| {
            w.ended.is_none()
                && now_ms >= w.last_seen_ms
                && now_ms < w.deadline_ms
                && (w.running_job.is_some() || (w.admitted < w.limit && w.next.is_some()))
        })
    }

    /// Apply to a copy, so validation errors never partly change in-memory state.
    /// # Errors
    /// Rejects conflicting retries, stale work and unselected generations.
    pub fn apply(
        &mut self,
        owner: &Owner,
        request_id: &str,
        now_ms: u64,
        op: Operation,
    ) -> Result<()> {
        self.validate(owner)?;
        ensure!(
            !request_id.is_empty() && request_id.len() <= 128,
            "bounded request ID required"
        );
        let fingerprint = crate::digest(serde_json::to_vec(&op)?);
        if let Some(previous) = self.requests.get(request_id) {
            ensure!(previous == &fingerprint, "conflicting activity retry");
            return Ok(());
        }
        ensure!(
            self.requests.len() < 65_536,
            "activity request history capacity reached; retained unchanged"
        );
        let mut next = self.clone();
        next.transition(request_id, now_ms, op)?;
        next.revision = next
            .revision
            .checked_add(1)
            .context("activity revision exhausted")?;
        next.requests.insert(request_id.into(), fingerprint);
        next.validate(owner)?;
        *self = next;
        Ok(())
    }

    #[allow(clippy::too_many_lines)] // Exhaustive transition table keeps budget and selection invariants visible together.
    fn transition(&mut self, request_id: &str, now_ms: u64, op: Operation) -> Result<()> {
        if let Some(w) = &mut self.window {
            if now_ms < w.last_seen_ms {
                w.ended = Some("unverifiable_clock".into());
            } else {
                w.last_seen_ms = now_ms;
                if now_ms >= w.deadline_ms {
                    w.ended = Some("deadline".into());
                }
            }
        }
        match op {
            Operation::ObserveClock => {},
            Operation::Start {
                target,
                target_revision,
                turns,
            } => {
                target.validate()?;
                ensure!((1..=MAX_JOBS).contains(&turns), "turns must be 1 through 4");
                ensure!(
                    !target_revision.is_empty(),
                    "verified target revision required"
                );
                ensure!(
                    !self.protected(now_ms),
                    "end the existing focus before starting another"
                );
                ensure!(
                    self.window.as_ref().is_none_or(|w| w.running_job.is_none()),
                    "accepted work is still running"
                );
                self.window = Some(Window {
                    id: request_id.into(),
                    next: Some(crate::digest(target.presentation())),
                    target,
                    target_revision,
                    created_ms: now_ms,
                    last_seen_ms: now_ms,
                    deadline_ms: now_ms.checked_add(WINDOW_MS).context("deadline overflow")?,
                    limit: turns,
                    admitted: 0,
                    running_job: None,
                    ended: None,
                });
            },
            Operation::Admit {
                job_id,
                action,
                target_revision,
            } => {
                ensure!(
                    !job_id.is_empty() && job_id.len() <= 128,
                    "bounded job ID required"
                );
                let window_id = &self.window.as_ref().context("no focus")?.id;
                let fingerprint =
                    crate::digest(serde_json::to_vec(&(window_id, &action, &target_revision))?);
                if let Some(prior) = self.jobs.get(&job_id) {
                    ensure!(prior == &fingerprint, "conflicting generation admission");
                    return Ok(());
                }
                ensure!(
                    self.protected(now_ms),
                    "focus is not eligible for admission"
                );
                let w = self.window.as_mut().context("no focus")?;
                ensure!(w.running_job.is_none(), "one generation at a time");
                ensure!(
                    w.target_revision == target_revision,
                    "selected work changed; explicit return required"
                );
                ensure!(
                    w.next.as_deref() == Some(crate::digest(&action).as_str()),
                    "generation was not explicitly selected"
                );
                w.admitted = w.admitted.checked_add(1).context("job budget exhausted")?;
                w.running_job = Some(job_id.clone());
                w.next = None;
                self.jobs.insert(job_id, fingerprint);
            },
            Operation::Complete {
                job_id,
                next,
                target_revision,
            } => {
                let w = self.window.as_mut().context("no focus")?;
                ensure!(
                    w.running_job.as_deref() == Some(&job_id),
                    "completion does not match the accepted job"
                );
                ensure!(
                    !target_revision.is_empty(),
                    "verified completion revision required"
                );
                w.running_job = None;
                w.target_revision = target_revision;
                w.next = next
                    .filter(|choice| w.target.permits(choice))
                    .map(crate::digest);
                if w.next.is_none() {
                    w.ended.get_or_insert_with(|| "no_eligible_next".into());
                }
                if w.admitted == w.limit {
                    w.ended.get_or_insert_with(|| "job_budget".into());
                }
            },
            Operation::End | Operation::Park => {
                if let Some(w) = &mut self.window {
                    if matches!(op, Operation::Park) {
                        self.parked = Some(w.target.clone());
                    }
                    w.ended = Some(
                        if matches!(op, Operation::Park) {
                            "parked"
                        } else {
                            "ended"
                        }
                        .into(),
                    );
                    w.next = None;
                }
            },
            Operation::Return { expected_revision } => {
                ensure!(
                    expected_revision == self.revision,
                    "stale return revision; inspect ACTIVITY_STATUS"
                );
                ensure!(self.parked.is_some(), "no parked activity");
                // Return selects native context through the adapter, never renews priority.
                if let Some(w) = &mut self.window {
                    ensure!(w.running_job.is_none(), "accepted work is still running");
                    w.ended = Some("explicit_return_without_focus".into());
                    w.next = None;
                }
            },
        }
        Ok(())
    }
}

/// Shared parsing; the legacy metabolic FOCUS command is deliberately not an alias.
/// # Errors
/// Rejects invalid targets, extra arguments and budgets outside one through four.
pub fn parse_start(action: &str) -> Result<(Target, u8)> {
    let words: Vec<_> = action.split_whitespace().collect();
    let (kind, id, turns) = match words.as_slice() {
        ["ACTIVITY_FOCUS", kind, id] => (*kind, *id, MAX_JOBS),
        ["ACTIVITY_FOCUS", kind, id, "turns", n] => (*kind, *id, n.parse::<u8>()?),
        _ => bail!("use ACTIVITY_FOCUS WRITE dN [turns N] or ACTIVITY_FOCUS QUESTION qN [turns N]"),
    };
    ensure!((1..=MAX_JOBS).contains(&turns), "turns must be 1 through 4");
    let target = match kind {
        "WRITE" => Target::Write(id.into()),
        "QUESTION" => Target::Question(id.into()),
        _ => bail!("focus target must be WRITE or QUESTION"),
    };
    target.validate()?;
    Ok((target, turns))
}
