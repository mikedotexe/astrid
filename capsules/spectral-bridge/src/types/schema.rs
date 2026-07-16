//! Canonical schemas and compatibility projections for the spectral bridge.
//!
//! These types define the wire format for all IPC topics in the
//! `consciousness.v1.*` namespace and map directly to minime's
//! `WebSocket` protocols.
//!
//! Many types are defined now but consumed in later phases (MCP tools,
//! WASM component). Allow dead code until then.
#![allow(dead_code)]

use std::{collections::BTreeMap, path::PathBuf};

use serde::{Deserialize, Serialize};

use crate::lambda_edge::LambdaEdgePerceptionV1;
use crate::lambda_tail::LambdaTailTelemetryV1;
use crate::sticky_mode::StickyModeAuditV1;

include!("schema/clamp_provenance.rs");
include!("schema/experience_delta.rs");
include!("schema/resonance_stability.rs");
include!("schema/transport_evidence.rs");
include!("schema/texture_evidence.rs");
include!("schema/inhabitable_fluctuation.rs");
include!("schema/telemetry.rs");
include!("schema/sensory.rs");
include!("schema/bridge_status.rs");
include!("schema/status_enums.rs");
include!("schema/events_and_attractors.rs");
include!("schema/control_and_chimera.rs");
include!("schema/tests.rs");
