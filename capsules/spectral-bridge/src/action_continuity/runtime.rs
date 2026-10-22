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

#[path = "runtime/experiment_comparison.rs"]
mod experiment_comparison;
#[path = "guards.rs"]
mod guards;
#[path = "ids.rs"]
mod ids;
#[path = "runtime/language_diagnostics.rs"]
mod language_diagnostics;
#[path = "paths.rs"]
mod paths;
#[path = "persistence.rs"]
mod persistence;
#[path = "research_json.rs"]
mod research_json;
#[path = "research_routes.rs"]
mod research_routes;
#[path = "research_transactions.rs"]
mod research_transactions;
pub(crate) use research_routes::{ResearchWebRequest, research_url_arg, research_web_request};
pub(crate) use research_transactions::research_owner_transaction;
pub(crate) use research_transactions::strict_rows as read_native_authority_records;
pub use research_transactions::{
    RESEARCH_TRANSACTION_VERSION, ResearchReservation, append_native_authority_record,
    research_binding_requires_budget, research_transaction_compatibility,
};
#[cfg(test)]
#[path = "research_transaction_tests.rs"]
mod research_transaction_tests;
#[path = "runtime/retained_inspection.rs"]
mod retained_inspection;
#[path = "runtime/return_support.rs"]
mod return_support;
#[path = "runtime/save_receipts.rs"]
mod save_receipts;
pub use guards::{CharterReason, CharterRequiredGuardAssessment, ResearchBudgetGuardAssessment};

include!("runtime/core.rs");
include!("runtime/session_lifecycle.rs");
include!("runtime/session_contract.rs");
#[path = "runtime/reader_bookmark_io.rs"]
mod reader_bookmark_io;
#[path = "runtime/reader_bookmark_types.rs"]
mod reader_bookmark_types;
pub use reader_bookmark_types::*;
include!("runtime/reader_bookmarks.rs");
include!("runtime/self_study_contract.rs");
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

#[cfg(test)]
#[path = "session_contract_tests.rs"]
mod session_contract_tests;

#[cfg(test)]
#[path = "reader_bookmark_tests.rs"]
mod reader_bookmark_tests;

#[cfg(test)]
#[path = "self_study_contract_tests.rs"]
mod self_study_contract_tests;

#[cfg(test)]
#[path = "return_support_tests.rs"]
mod return_support_tests;

#[cfg(test)]
#[path = "consumer_failure_tests.rs"]
mod consumer_failure_tests;

#[cfg(test)]
#[path = "exploration_scope_tests.rs"]
mod exploration_scope_tests;

#[cfg(test)]
#[path = "language_diagnostic_tests.rs"]
mod language_diagnostic_tests;
