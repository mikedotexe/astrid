//! Shared data types for the Astrid secure agent runtime.
//!
//! This crate provides the canonical definitions for:
//! - IPC payload schemas (cross-boundary messaging between WASM guests and host)
//! - LLM message, tool, and streaming types
//! - Kernel management API request/response types
//!
//! It has minimal dependencies (serde, uuid) and is WASM-compatible, making it
//! suitable for use in both the kernel runtime and user-space capsule SDKs.

#![deny(unsafe_code)]
#![deny(missing_docs)]
#![deny(clippy::all)]
#![deny(unreachable_pub)]
#![deny(clippy::unwrap_used)]
#![cfg_attr(test, allow(clippy::unwrap_used))]

pub mod agency_corridor;
pub mod authority;
pub mod ipc;
pub mod kernel;
pub mod llm;

pub use agency_corridor::{
    AgencyCorridorActionV1, AgencyCorridorPacketV1, AgencyCorridorPacketV2,
    AgencyCorridorReceiptV1, AgencyCorridorReceiptV2, AgencyCorridorStateV1,
    AgencyProgramReceiptKindV1, AgencyProgramReceiptV1, AgencyWorkProgramStatusV1,
    AgencyWorkProgramV1, AutonomousWorkQueueV1, AutonomyEscalatorStepV1, AutonomyLeaseStateV1,
    AutonomyLeaseV1, AutonomyPrioritySignalV1, CanaryCriteriaProposalV1, ClosureObjectionV1,
    ClosureReopenPolicyV1, ClosureReopenRefV1, EvidencePortfolioV1, QuarantinedPatchBundleV1,
    SafeLabCandidateV1, ScopedSelfObservationRequestV1, SourcePrepProposalV1,
};
pub use authority::{
    AuthorityBoundaryPacketV1, AuthorityBoundaryPacketV2, AuthorityClass, AuthorityGateStateV1,
    AuthorityLifecycleEvaluationV2, AuthorityLifecycleReceiptKindV2, AuthorityLifecycleReceiptV2,
    AuthorityLifecycleStateV2, AuthorityReceiptRefV1, ExperienceDeltaRefV2, RedactionProfileV2,
    ReplayCandidateV1, ReplayResultClassificationV2, ReplayResultV2, RolloutAbortContractV2,
    ScopedApprovalKindV2, ScopedApprovalV2, TelemetryConditionV2,
};
pub use ipc::{IpcMessage, IpcPayload, OnboardingField, OnboardingFieldType, SelectionOption};
pub use kernel::{
    CapsuleMetadataEntry, CommandInfo, DaemonStatus, KernelRequest, KernelResponse,
    SYSTEM_SESSION_UUID,
};
pub use llm::{
    ContentPart, LlmResponse, LlmToolDefinition, Message, MessageContent, MessageRole, StopReason,
    StreamEvent, ToolCall, ToolCallResult, Usage,
};
