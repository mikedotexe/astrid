//! Durable two-hour schedule for the immutable steward's 15-minute poll timer.

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::config::Config;
use crate::util::{atomic_private_write, canonical_json, read_stable_regular, unix_seconds};
use crate::{Error, Result};

const SCHEMA_V1: &str = "astrid.edge.steward_helper.schedule.v1";
const SCHEMA: &str = "astrid.edge.steward_helper.schedule.v2";
const COMPLETION_PLAN_SCHEMA: &str = "astrid.edge.steward_helper.schedule_completion.v1";
pub(crate) const INTERVAL_SECONDS: u64 = 2 * 60 * 60;

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct ScheduleState {
    schema: String,
    next_due_at_unix_seconds: u64,
    pending_due_at_unix_seconds: Option<u64>,
    last_completed_at_unix_seconds: Option<u64>,
    last_model_started_at_unix_seconds: Option<u64>,
    next_model_eligible_at_unix_seconds: u64,
    completed_count: u64,
    model_start_count: u64,
    #[serde(skip)]
    migrated_from_v1: bool,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
struct ScheduleStateV1 {
    schema: String,
    next_due_at_unix_seconds: u64,
    pending_due_at_unix_seconds: Option<u64>,
    last_completed_at_unix_seconds: Option<u64>,
    completed_count: u64,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct CompletionPlan {
    schema: String,
    due_nonce: String,
    planned_at_unix_seconds: u64,
    scheduled_due_at_unix_seconds: u64,
    next_due_at_unix_seconds: u64,
    coalesces_pending_slot: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Decision {
    NotDue {
        next_due_at_unix_seconds: u64,
    },
    ModelFloor {
        nonce: String,
        next_model_eligible_at_unix_seconds: u64,
    },
    Due {
        nonce: String,
        automatic: bool,
    },
}

pub fn decide(config: &Config, requested: Option<String>) -> Result<Decision> {
    if let Some(nonce) = requested {
        return Ok(Decision::Due {
            nonce,
            automatic: false,
        });
    }
    decide_at(&schedule_path(config), unix_seconds())
}

pub fn complete(config: &Config, nonce: &str, automatic: bool) -> Result<()> {
    if !automatic {
        return Ok(());
    }
    complete_with_plan_at(
        &schedule_path(config),
        Some(&completion_plan_path(config)),
        nonce,
        unix_seconds(),
    )
}

/// Persist and return the exact automatic cadence boundary that authored
/// projections must publish. Completion later consumes this same plan, so a
/// delayed finalization or crash recovery cannot make the owner-visible
/// receipt disagree with the immutable scheduler.
pub fn prepare_completion_projection(config: &Config, nonce: &str) -> Result<u64> {
    let now = unix_seconds();
    let schedule_path = schedule_path(config);
    prepare_completion_at(&schedule_path, &completion_plan_path(config), nonce, now)
}

/// Return the currently scheduled cadence boundary without consuming or
/// preparing it. Evidence integrations use this only for truthful reporting.
pub fn next_due_at(config: &Config) -> Result<u64> {
    Ok(load(&schedule_path(config), unix_seconds())?.next_due_at_unix_seconds)
}

/// Atomically consume one automatic model-start slot immediately before the
/// first provider write. A failed, malformed, or partial response leaves the
/// original due nonce pending, but cannot begin another model call for two
/// hours. Exact prepared-transaction recovery never calls this function.
pub fn begin_model_attempt(config: &Config, nonce: &str, automatic: bool) -> Result<()> {
    if !automatic {
        return Ok(());
    }
    begin_model_attempt_at(&schedule_path(config), nonce, unix_seconds())
}

fn schedule_path(config: &Config) -> PathBuf {
    config.state_root.join("schedule.json")
}

fn completion_plan_path(config: &Config) -> PathBuf {
    config.state_root.join("schedule-completion.json")
}

fn initial(now: u64) -> ScheduleState {
    ScheduleState {
        schema: SCHEMA.to_owned(),
        next_due_at_unix_seconds: now,
        pending_due_at_unix_seconds: None,
        last_completed_at_unix_seconds: None,
        last_model_started_at_unix_seconds: None,
        next_model_eligible_at_unix_seconds: 0,
        completed_count: 0,
        model_start_count: 0,
        migrated_from_v1: false,
    }
}

fn load(path: &Path, now: u64) -> Result<ScheduleState> {
    if !path.exists() {
        return Ok(initial(now));
    }
    let bytes = read_stable_regular(path, 16 * 1024)?;
    let schema = serde_json::from_slice::<serde_json::Value>(&bytes)?
        .get("schema")
        .and_then(serde_json::Value::as_str)
        .ok_or_else(|| Error::new("scheduled reflection state schema is absent"))?
        .to_owned();
    let state = if schema == SCHEMA {
        serde_json::from_slice(&bytes)?
    } else if schema == SCHEMA_V1 {
        let legacy: ScheduleStateV1 = serde_json::from_slice(&bytes)?;
        if legacy.schema != SCHEMA_V1 {
            return Err(Error::new("scheduled reflection v1 state is invalid"));
        }
        // A pending v1 slot may already have reached the provider before this
        // floor existed. Conservatively start one migration floor now instead
        // of risking an immediate duplicate model call.
        let migrated_start = legacy.pending_due_at_unix_seconds.map(|_| now);
        ScheduleState {
            schema: SCHEMA.to_owned(),
            next_due_at_unix_seconds: legacy.next_due_at_unix_seconds,
            pending_due_at_unix_seconds: legacy.pending_due_at_unix_seconds,
            last_completed_at_unix_seconds: legacy.last_completed_at_unix_seconds,
            last_model_started_at_unix_seconds: migrated_start,
            next_model_eligible_at_unix_seconds: migrated_start
                .map_or(0, |started| started.saturating_add(INTERVAL_SECONDS)),
            completed_count: legacy.completed_count,
            model_start_count: u64::from(migrated_start.is_some()),
            migrated_from_v1: true,
        }
    } else {
        return Err(Error::new("scheduled reflection state schema is invalid"));
    };
    if state.schema != SCHEMA
        || state.next_due_at_unix_seconds == 0
        || state.next_due_at_unix_seconds > now.saturating_add(INTERVAL_SECONDS)
        || state
            .pending_due_at_unix_seconds
            .is_some_and(|pending| pending != state.next_due_at_unix_seconds || pending > now)
        || state
            .last_completed_at_unix_seconds
            .is_some_and(|completed| completed > now.saturating_add(60))
        || state
            .last_model_started_at_unix_seconds
            .is_some_and(|started| started > now.saturating_add(60))
        || match state.last_model_started_at_unix_seconds {
            Some(started) => {
                state.next_model_eligible_at_unix_seconds
                    != started.saturating_add(INTERVAL_SECONDS)
                    || state.model_start_count == 0
            },
            None => state.next_model_eligible_at_unix_seconds != 0 || state.model_start_count != 0,
        }
    {
        return Err(Error::new("scheduled reflection state is invalid"));
    }
    Ok(state)
}

fn persist(path: &Path, state: &ScheduleState) -> Result<()> {
    atomic_private_write(path, &canonical_json(state)?)
}

fn decide_at(path: &Path, now: u64) -> Result<Decision> {
    let mut state = load(path, now)?;
    // Persist an additive v1 migration even when the slot is not yet due. This
    // makes the next immutable root admission evaluate the same v2 floor.
    if state.migrated_from_v1 {
        persist(path, &state)?;
        state.migrated_from_v1 = false;
    }
    if now < state.next_due_at_unix_seconds {
        return Ok(Decision::NotDue {
            next_due_at_unix_seconds: state.next_due_at_unix_seconds,
        });
    }
    let pending = state
        .pending_due_at_unix_seconds
        .unwrap_or(state.next_due_at_unix_seconds);
    state.pending_due_at_unix_seconds = Some(pending);
    persist(path, &state)?;
    if now < state.next_model_eligible_at_unix_seconds {
        return Ok(Decision::ModelFloor {
            nonce: format!("due-{pending}"),
            next_model_eligible_at_unix_seconds: state.next_model_eligible_at_unix_seconds,
        });
    }
    Ok(Decision::Due {
        nonce: format!("due-{pending}"),
        automatic: true,
    })
}

fn begin_model_attempt_at(path: &Path, nonce: &str, now: u64) -> Result<()> {
    let mut state = load(path, now)?;
    let expected = state
        .pending_due_at_unix_seconds
        .ok_or_else(|| Error::new("model attempt has no pending scheduled slot"))?;
    if nonce != format!("due-{expected}")
        || now < state.next_due_at_unix_seconds
        || now < state.next_model_eligible_at_unix_seconds
    {
        return Err(Error::new(
            "model attempt does not hold the exact eligible scheduled slot",
        ));
    }
    state.last_model_started_at_unix_seconds = Some(now);
    state.next_model_eligible_at_unix_seconds = now.saturating_add(INTERVAL_SECONDS);
    state.model_start_count = state.model_start_count.saturating_add(1);
    persist(path, &state)
}

fn coalesced_next_due(expected: u64, completed_at: u64) -> u64 {
    let elapsed = completed_at.saturating_sub(expected);
    let elapsed_intervals = elapsed / INTERVAL_SECONDS;
    let intervals_to_next = elapsed_intervals.saturating_add(1);
    expected.saturating_add(INTERVAL_SECONDS.saturating_mul(intervals_to_next))
}

fn prepare_completion_at(
    schedule_path: &Path,
    plan_path: &Path,
    nonce: &str,
    now: u64,
) -> Result<u64> {
    let state = load(schedule_path, now)?;
    if plan_path.exists() || plan_path.is_symlink() {
        let prior: CompletionPlan =
            serde_json::from_slice(&read_stable_regular(plan_path, 16 * 1024)?)?;
        validate_completion_plan(&prior, now)?;
        if prior.due_nonce == nonce {
            return Ok(prior.next_due_at_unix_seconds);
        }
    }
    let coalesces_pending_slot = state
        .pending_due_at_unix_seconds
        .is_some_and(|pending| nonce == format!("due-{pending}"));
    let scheduled_due_at_unix_seconds = state.next_due_at_unix_seconds;
    let plan = CompletionPlan {
        schema: COMPLETION_PLAN_SCHEMA.to_owned(),
        due_nonce: nonce.to_owned(),
        planned_at_unix_seconds: now,
        scheduled_due_at_unix_seconds,
        next_due_at_unix_seconds: if coalesces_pending_slot {
            coalesced_next_due(scheduled_due_at_unix_seconds, now)
        } else {
            scheduled_due_at_unix_seconds
        },
        coalesces_pending_slot,
    };
    atomic_private_write(plan_path, &canonical_json(&plan)?)?;
    Ok(plan.next_due_at_unix_seconds)
}

fn validate_completion_plan(plan: &CompletionPlan, now: u64) -> Result<()> {
    let expected = plan
        .due_nonce
        .strip_prefix("due-")
        .and_then(|value| value.parse::<u64>().ok())
        .ok_or_else(|| Error::new("schedule completion plan due nonce is invalid"))?;
    if plan.schema != COMPLETION_PLAN_SCHEMA
        || plan.planned_at_unix_seconds > now.saturating_add(60)
        || plan.scheduled_due_at_unix_seconds == 0
        || plan.next_due_at_unix_seconds
            != if plan.coalesces_pending_slot {
                coalesced_next_due(
                    plan.scheduled_due_at_unix_seconds,
                    plan.planned_at_unix_seconds,
                )
            } else {
                plan.scheduled_due_at_unix_seconds
            }
        || (plan.coalesces_pending_slot && expected != plan.scheduled_due_at_unix_seconds)
    {
        return Err(Error::new("schedule completion plan is invalid"));
    }
    Ok(())
}

#[cfg(test)]
fn complete_at(path: &Path, nonce: &str, now: u64) -> Result<()> {
    complete_with_plan_at(path, None, nonce, now)
}

fn complete_with_plan_at(
    path: &Path,
    plan_path: Option<&Path>,
    nonce: &str,
    now: u64,
) -> Result<()> {
    let mut state = load(path, now)?;
    let expected = state
        .pending_due_at_unix_seconds
        .ok_or_else(|| Error::new("scheduled reflection completion has no pending slot"))?;
    if nonce != format!("due-{expected}") {
        return Err(Error::new(
            "scheduled reflection completion does not bind its pending slot",
        ));
    }
    // Preserve the original two-hour cadence. A delayed or long-running
    // reflection consumes every elapsed boundary into one coalesced slot and
    // advances to the first original boundary strictly after completion.
    // Scheduling from `now` here would make generation latency permanently
    // drift the cadence.
    let next_due = if let Some(plan_path) = plan_path
        && (plan_path.exists() || plan_path.is_symlink())
    {
        let plan: CompletionPlan =
            serde_json::from_slice(&read_stable_regular(plan_path, 16 * 1024)?)?;
        validate_completion_plan(&plan, now)?;
        if plan.due_nonce == nonce && plan.coalesces_pending_slot {
            plan.next_due_at_unix_seconds
        } else {
            coalesced_next_due(expected, now)
        }
    } else {
        coalesced_next_due(expected, now)
    };
    state.pending_due_at_unix_seconds = None;
    state.last_completed_at_unix_seconds = Some(now);
    state.next_due_at_unix_seconds = next_due;
    state.completed_count = state.completed_count.saturating_add(1);
    persist(path, &state)
}

#[cfg(test)]
mod tests {
    use super::{
        Decision, begin_model_attempt_at, complete_at, complete_with_plan_at, decide_at,
        prepare_completion_at,
    };

    fn private_temporary_directory() -> tempfile::TempDir {
        tempfile::tempdir_in(std::env::current_dir().unwrap()).unwrap()
    }

    #[test]
    fn due_slots_defer_without_catchup_and_keep_a_two_hour_floor() {
        let temporary = private_temporary_directory();
        let path = temporary.path().join("schedule.json");
        assert_eq!(
            decide_at(&path, 10_000).unwrap(),
            Decision::Due {
                nonce: "due-10000".to_owned(),
                automatic: true,
            }
        );
        assert_eq!(
            decide_at(&path, 20_000).unwrap(),
            Decision::Due {
                nonce: "due-10000".to_owned(),
                automatic: true,
            }
        );
        begin_model_attempt_at(&path, "due-10000", 20_000).unwrap();
        complete_at(&path, "due-10000", 20_000).unwrap();
        assert_eq!(
            decide_at(&path, 24_399).unwrap(),
            Decision::NotDue {
                next_due_at_unix_seconds: 24_400,
            }
        );
        assert_eq!(
            decide_at(&path, 40_000).unwrap(),
            Decision::Due {
                nonce: "due-24400".to_owned(),
                automatic: true,
            }
        );
    }

    #[test]
    fn completion_requires_the_exact_pending_nonce() {
        let temporary = private_temporary_directory();
        let path = temporary.path().join("schedule.json");
        let _ = decide_at(&path, 10_000).unwrap();
        assert!(complete_at(&path, "due-10001", 10_001).is_err());
    }

    #[test]
    fn many_missed_poll_slots_coalesce_into_the_original_pending_due() {
        let temporary = private_temporary_directory();
        let path = temporary.path().join("schedule.json");
        assert_eq!(
            decide_at(&path, 10_000).unwrap(),
            Decision::Due {
                nonce: "due-10000".to_owned(),
                automatic: true,
            }
        );
        assert_eq!(
            decide_at(&path, 100_000).unwrap(),
            Decision::Due {
                nonce: "due-10000".to_owned(),
                automatic: true,
            }
        );
        begin_model_attempt_at(&path, "due-10000", 100_000).unwrap();
        complete_at(&path, "due-10000", 100_000).unwrap();
        assert_eq!(
            decide_at(&path, 103_599).unwrap(),
            Decision::NotDue {
                next_due_at_unix_seconds: 103_600,
            }
        );
    }

    #[test]
    fn delayed_completion_publishes_and_commits_one_persisted_coalesced_boundary() {
        let temporary = private_temporary_directory();
        let schedule = temporary.path().join("schedule.json");
        let plan = temporary.path().join("schedule-completion.json");
        assert!(matches!(
            decide_at(&schedule, 10_000).unwrap(),
            Decision::Due { .. }
        ));

        // A reflection that finishes 10,000 seconds after its original slot
        // must retain the original cadence (24,400), not report 27,200.
        let published = prepare_completion_at(&schedule, &plan, "due-10000", 20_000).unwrap();
        assert_eq!(published, 24_400);

        // Final schedule persistence may happen later than projection. It must
        // consume the exact persisted plan rather than recompute from 40,000.
        complete_with_plan_at(&schedule, Some(&plan), "due-10000", 40_000).unwrap();
        let state: serde_json::Value =
            serde_json::from_slice(&std::fs::read(schedule).unwrap()).unwrap();
        assert_eq!(state["next_due_at_unix_seconds"], published);
        assert_ne!(state["next_due_at_unix_seconds"], 47_200);
    }

    #[test]
    fn failed_model_start_keeps_due_nonce_but_enforces_two_hour_floor() {
        let temporary = private_temporary_directory();
        let path = temporary.path().join("schedule.json");
        let due = decide_at(&path, 10_000).unwrap();
        assert!(matches!(due, Decision::Due { .. }));
        begin_model_attempt_at(&path, "due-10000", 10_010).unwrap();
        assert_eq!(
            decide_at(&path, 17_209).unwrap(),
            Decision::ModelFloor {
                nonce: "due-10000".to_owned(),
                next_model_eligible_at_unix_seconds: 17_210,
            }
        );
        assert_eq!(
            decide_at(&path, 17_210).unwrap(),
            Decision::Due {
                nonce: "due-10000".to_owned(),
                automatic: true,
            }
        );
    }

    #[test]
    fn repeated_or_tampered_model_start_is_rejected() {
        let temporary = private_temporary_directory();
        let path = temporary.path().join("schedule.json");
        let _ = decide_at(&path, 10_000).unwrap();
        begin_model_attempt_at(&path, "due-10000", 10_001).unwrap();
        assert!(begin_model_attempt_at(&path, "due-10000", 10_002).is_err());
        assert!(begin_model_attempt_at(&path, "due-9999", 17_201).is_err());

        let mut value: serde_json::Value =
            serde_json::from_slice(&std::fs::read(&path).unwrap()).unwrap();
        value["next_model_eligible_at_unix_seconds"] = serde_json::json!(10_002);
        std::fs::write(&path, serde_json::to_vec(&value).unwrap()).unwrap();
        assert!(decide_at(&path, 10_002).is_err());
    }

    #[test]
    fn pending_v1_state_migrates_with_a_conservative_restart_floor() {
        let temporary = private_temporary_directory();
        let path = temporary.path().join("schedule.json");
        std::fs::write(
            &path,
            br#"{"schema":"astrid.edge.steward_helper.schedule.v1","next_due_at_unix_seconds":10000,"pending_due_at_unix_seconds":10000,"last_completed_at_unix_seconds":null,"completed_count":0}"#,
        )
        .unwrap();
        assert_eq!(
            decide_at(&path, 10_100).unwrap(),
            Decision::ModelFloor {
                nonce: "due-10000".to_owned(),
                next_model_eligible_at_unix_seconds: 17_300,
            }
        );
        let state = std::fs::read_to_string(path).unwrap();
        assert!(state.contains("astrid.edge.steward_helper.schedule.v2"));
    }

    #[test]
    fn prepared_authored_recovery_completes_without_consuming_a_model_start() {
        let temporary = private_temporary_directory();
        let path = temporary.path().join("schedule.json");
        let _ = decide_at(&path, 10_000).unwrap();
        // Recovery finalizes an already-prepared exact response and therefore
        // calls completion directly without beginning another model attempt.
        complete_at(&path, "due-10000", 10_100).unwrap();
        let value: serde_json::Value =
            serde_json::from_slice(&std::fs::read(path).unwrap()).unwrap();
        assert_eq!(value["model_start_count"], 0);
        assert_eq!(
            value["last_model_started_at_unix_seconds"],
            serde_json::Value::Null
        );
        assert_eq!(value["next_due_at_unix_seconds"], 17_200);
    }

    #[test]
    fn long_run_advances_to_first_original_boundary_after_completion() {
        let temporary = private_temporary_directory();
        let path = temporary.path().join("schedule.json");
        let _ = decide_at(&path, 10_000).unwrap();
        begin_model_attempt_at(&path, "due-10000", 10_001).unwrap();
        complete_at(&path, "due-10000", 25_000).unwrap();
        assert_eq!(
            decide_at(&path, 31_599).unwrap(),
            Decision::NotDue {
                next_due_at_unix_seconds: 31_600,
            }
        );
        assert_eq!(
            decide_at(&path, 31_600).unwrap(),
            Decision::Due {
                nonce: "due-31600".to_owned(),
                automatic: true,
            }
        );
    }

    #[test]
    fn completion_exactly_on_boundary_skips_that_elapsed_boundary() {
        let temporary = private_temporary_directory();
        let path = temporary.path().join("schedule.json");
        let _ = decide_at(&path, 10_000).unwrap();
        complete_at(&path, "due-10000", 17_200).unwrap();
        assert_eq!(
            decide_at(&path, 24_399).unwrap(),
            Decision::NotDue {
                next_due_at_unix_seconds: 24_400,
            }
        );
    }
}
