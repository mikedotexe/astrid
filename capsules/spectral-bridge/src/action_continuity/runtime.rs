//! File-first action/thread continuity for Astrid.
//!
//! The JSON/JSONL files under `workspace/action_threads/` are authoritative.
//! SQLite rows are mirrors for querying and dashboards.

use std::collections::{HashMap, HashSet, VecDeque};
use std::ffi::OsStr;
use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use anyhow::{Context, Result, anyhow};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

use crate::continuity_control_plane::{
    AUTHORITY_BUDGET_MAX_SENDS, LOCAL_RESEARCH_MAX_ACTIONS, LOCAL_RESEARCH_TTL_SECS,
    LOOP_CONSEQUENCE_MAX_SENDS, LOOP_RESEARCH_MAX_ACTIONS, LOOP_TTL_SECS,
    STEWARD_RESEARCH_MAX_ACTIONS, authority_budget_request_scaffold, build_control_plane_v1,
    command_palette_text as control_plane_command_palette_text, control_plane_text,
    default_local_research_budget_request_scaffold, default_owned_loop_request_scaffold,
    local_research_budget_request_scaffold, owned_loop_request_scaffold,
    research_budget_accept_guidance,
};
use crate::db::{BridgeDb, unix_now};
use crate::paths::bridge_paths;
use crate::types::SpectralTelemetry;

#[path = "guards.rs"]
mod guards;
#[path = "ids.rs"]
mod ids;
#[path = "paths.rs"]
mod paths;
#[path = "persistence.rs"]
mod persistence;
pub use guards::{CharterReason, CharterRequiredGuardAssessment, ResearchBudgetGuardAssessment};

include!("runtime/core.rs");
include!("runtime/prompt_projection.rs");
include!("runtime/command_dispatch.rs");
include!("runtime/experiment_projection.rs");
include!("runtime/authority.rs");
include!("runtime/conveyor.rs");
include!("runtime/experiment_evidence.rs");
include!("runtime/shared_investigation.rs");
include!("runtime/workbench_projection.rs");
include!("runtime/persistence_helpers.rs");
include!("runtime/guards.rs");
include!("runtime/native_continuity.rs");
include!("runtime/charter_projection.rs");
include!("runtime/spectral_projection.rs");

#[cfg(test)]
#[path = "tests.rs"]
mod tests;
