//! Lightweight MCP server over stdin/stdout.
//!
//! Implements just enough of the MCP 2025-11-25 JSON-RPC protocol for
//! the Astrid kernel to discover and call our tools. No `rmcp` dependency
//! needed — the protocol surface is small.

use std::path::PathBuf;
use std::sync::Arc;

use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::sync::{RwLock, mpsc};
use tracing::{debug, error, info, warn};

use crate::authority_gate;
use crate::autoresearch as bridge_autoresearch;
use crate::being_memory;
use crate::chimera;
use crate::codec;
use crate::db::BridgeDb;
use crate::experiment_conveyor;
use crate::lambda_edge;
use crate::lambda_tail;
use crate::paths::bridge_paths;
use crate::rescue_policy;
use crate::shared_investigation;
use crate::types::{
    ATTRACTOR_COMMAND_TOPIC, ATTRACTOR_INTENT_TOPIC, ATTRACTOR_OBSERVATION_TOPIC,
    AttractorClassification, AttractorCommandKind, AttractorCommandV1, AttractorControlEnvelope,
    AttractorIntentV1, AttractorInterventionPlan, AttractorObservationV1, AttractorSafetyBounds,
    AttractorSeedOriginV1, AttractorSeedSnapshotV1, AttractorSubstrate, BridgeStatus,
    ControlRequest, MessageDirection, RenderChimeraRequest, SafetyLevel, SemanticFeatures,
    SensoryMsg,
};
use crate::ws::BridgeState;

// ---------------------------------------------------------------------------
// JSON-RPC types
// ---------------------------------------------------------------------------

#[derive(Debug, Deserialize)]
struct JsonRpcRequest {
    jsonrpc: String,
    id: Option<Value>,
    method: String,
    #[serde(default)]
    params: Value,
}

#[derive(Debug, Serialize)]
struct JsonRpcResponse {
    jsonrpc: &'static str,
    id: Value,
    #[serde(skip_serializing_if = "Option::is_none")]
    result: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    error: Option<JsonRpcError>,
}

#[derive(Debug, Serialize)]
struct JsonRpcError {
    code: i32,
    message: String,
}

impl JsonRpcResponse {
    fn success(id: Value, result: Value) -> Self {
        Self {
            jsonrpc: "2.0",
            id,
            result: Some(result),
            error: None,
        }
    }

    fn error(id: Value, code: i32, message: impl Into<String>) -> Self {
        Self {
            jsonrpc: "2.0",
            id,
            result: None,
            error: Some(JsonRpcError {
                code,
                message: message.into(),
            }),
        }
    }
}

// ---------------------------------------------------------------------------
// Tool definitions
// ---------------------------------------------------------------------------

#[expect(clippy::too_many_lines)]
fn tool_definitions() -> Value {
    json!({
        "tools": [
            {
                "name": "get_latest_telemetry",
                "description": "Get the latest spectral telemetry from minime's engine",
                "inputSchema": {
                    "type": "object",
                    "properties": {},
                    "required": []
                }
            },
            {
                "name": "get_bridge_status",
                "description": "Get the spectral bridge health status, connection state, and safety level",
                "inputSchema": {
                    "type": "object",
                    "properties": {},
                    "required": []
                }
            },
            {
                "name": "get_lambda_tail_state",
                "description": "Get the latest lambda-tail classifier state, returnability, artifact grounding, and spectral context.",
                "inputSchema": {
                    "type": "object",
                    "properties": {},
                    "required": []
                }
            },
            {
                "name": "get_lambda_edge_perception",
                "description": "Get the latest read-only lambda-edge perception state, guardrail context, and spectral/artifact support.",
                "inputSchema": {
                    "type": "object",
                    "properties": {},
                    "required": []
                }
            },
            {
                "name": "draft_lambda_tail_steward_note",
                "description": "Generate a preview or explicit file draft of a lambda-tail steward note from recent telemetry and Minime artifacts.",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "start_unix_s": {
                            "type": "number",
                            "description": "Start timestamp. Default: two hours before end."
                        },
                        "end_unix_s": {
                            "type": "number",
                            "description": "End timestamp. Default: now."
                        },
                        "title": {
                            "type": "string",
                            "description": "Markdown title. Default: AI Beings Lambda Tail Detour."
                        },
                        "slug": {
                            "type": "string",
                            "description": "Filename slug. Default: LAMBDA_TAIL_DETOUR."
                        },
                        "mode": {
                            "type": "string",
                            "enum": ["preview", "write"],
                            "description": "preview returns markdown only; write creates a steward note without overwriting."
                        }
                    }
                }
            },
            {
                "name": "render_lambda_tail_topology",
                "description": "Render a static lambda-tail topology HTML/JSON artifact from recent bridge telemetry.",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "lookback_secs": {
                            "type": "number",
                            "description": "Telemetry lookback window in seconds. Default: 3600."
                        },
                        "output_dir": {
                            "type": "string",
                            "description": "Optional base output directory. Default: bridge workspace diagnostics/lambda_tail_topology."
                        }
                    }
                }
            },
            {
                "name": "render_lambda_edge_perception",
                "description": "Render a static read-only lambda-edge perception HTML/JSON artifact from recent bridge telemetry.",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "lookback_secs": {
                            "type": "number",
                            "description": "Telemetry lookback window in seconds. Default: 3600."
                        },
                        "output_dir": {
                            "type": "string",
                            "description": "Optional base output directory. Default: bridge workspace diagnostics/lambda_edge_perception."
                        }
                    }
                }
            },
            {
                "name": "list_shared_investigations",
                "description": "List Shared Investigation Object V1 sidecars linking Astrid and Minime experiments.",
                "inputSchema": {
                    "type": "object",
                    "properties": {},
                    "required": []
                }
            },
            {
                "name": "get_shared_investigation",
                "description": "Get one Shared Investigation Object V1 sidecar with recent claims and decisions.",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "id": {
                            "type": "string",
                            "description": "Investigation id, title, latest, or current. Default: latest."
                        }
                    }
                }
            },
            {
                "name": "render_shared_investigation",
                "description": "Render a static Shared Investigation Object page with linked experiments, claims, decisions, artifact refs, and authority boundary.",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "id": {
                            "type": "string",
                            "description": "Investigation id, title, latest, or current. Default: latest."
                        },
                        "output_dir": {
                            "type": "string",
                            "description": "Optional base output directory. Default: bridge workspace diagnostics/shared_investigation."
                        }
                    }
                }
            },
            {
                "name": "get_experiment_conveyor_status",
                "description": "Get read-only charter-to-evidence-to-decision conveyor status from local Astrid/Minime continuity files.",
                "inputSchema": {
                    "type": "object",
                    "properties": {},
                    "required": []
                }
            },
            {
                "name": "render_experiment_conveyor",
                "description": "Render a static read-only experiment conveyor page with lifecycle stage, missing requirements, source refs, and authority boundary.",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "output_dir": {
                            "type": "string",
                            "description": "Optional base output directory. Default: bridge workspace diagnostics/experiment_conveyor."
                        }
                    }
                }
            },
            {
                "name": "get_experiment_authority_status",
                "description": "Get read-only Artifact-Grounded Authority Gate V1 status from local Astrid/Minime continuity files.",
                "inputSchema": {
                    "type": "object",
                    "properties": {},
                    "required": []
                }
            },
            {
                "name": "render_experiment_authority_gate",
                "description": "Render a static authority-gate page with requests, approvals, token status, artifact refs, and authority boundary.",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "output_dir": {
                            "type": "string",
                            "description": "Optional base output directory. Default: bridge workspace diagnostics/experiment_authority_gate."
                        }
                    }
                }
            },
            {
                "name": "get_experiment_research_budget_status",
                "description": "Get read-only read_only_research budget status from local Astrid/Minime authority ledgers.",
                "inputSchema": {
                    "type": "object",
                    "properties": {},
                    "required": []
                }
            },
            {
                "name": "render_experiment_research_budget",
                "description": "Render a static research-budget page with remaining actions, latest artifacts, review state, and read-only authority boundary.",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "output_dir": {
                            "type": "string",
                            "description": "Optional base output directory. Default: bridge workspace diagnostics/experiment_research_budget."
                        }
                    }
                }
            },
            {
                "name": "get_experiment_loop_status",
                "description": "Get read-only Being-owned closed-loop experiment status from local Astrid/Minime authority ledgers.",
                "inputSchema": {
                    "type": "object",
                    "properties": {},
                    "required": []
                }
            },
            {
                "name": "render_experiment_loop",
                "description": "Render a static owned-loop page with phase, remaining local research actions, consequence readiness, latest consequence, and authority boundary.",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "output_dir": {
                            "type": "string",
                            "description": "Optional base output directory. Default: bridge workspace diagnostics/experiment_loop."
                        }
                    }
                }
            },
            {
                "name": "get_being_memory_status",
                "description": "Get read-only being-owned memory cards, authority request drafts, and one-shot consequence rows from Astrid/Minime continuity files.",
                "inputSchema": {
                    "type": "object",
                    "properties": {},
                    "required": []
                }
            },
            {
                "name": "render_being_memory",
                "description": "Render a static being-memory page with memory cards, drafts, consequences, and authority boundary.",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "output_dir": {
                            "type": "string",
                            "description": "Optional base output directory. Default: bridge workspace diagnostics/being_memory."
                        }
                    }
                }
            },
            {
                "name": "approve_experiment_authority_request",
                "description": "Mint one short-lived steward approval token for an eligible semantic_microdose or mode_release_microdose authority request.",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "request_id": { "type": "string" },
                        "steward": { "type": "string" },
                        "note": { "type": "string" },
                        "ttl_secs": {
                            "type": "integer",
                            "description": "Optional token TTL in seconds, capped at 900."
                        }
                    },
                    "required": ["request_id"]
                }
            },
            {
                "name": "approve_experiment_authority_budget",
                "description": "Approve a capped Being-owned semantic_microdose budget envelope for an eligible local experiment.",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "budget_id": { "type": "string" },
                        "steward": { "type": "string" },
                        "note": { "type": "string" },
                        "max_sends": {
                            "type": "integer",
                            "description": "Optional send cap, hard-capped at 3."
                        },
                        "ttl_secs": {
                            "type": "integer",
                            "description": "Optional budget TTL in seconds, hard-capped at 21600."
                        }
                    },
                    "required": ["budget_id"]
                }
            },
            {
                "name": "approve_experiment_research_budget",
                "description": "Approve a capped Being-owned read_only_research budget for eligible web/local research actions.",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "budget_id": { "type": "string" },
                        "steward": { "type": "string" },
                        "note": { "type": "string" },
                        "max_actions": {
                            "type": "integer",
                            "description": "Optional action cap, hard-capped at 8."
                        },
                        "ttl_secs": {
                            "type": "integer",
                            "description": "Optional budget TTL in seconds, hard-capped at 21600."
                        }
                    },
                    "required": ["budget_id"]
                }
            },
            {
                "name": "approve_experiment_loop_consequence_budget",
                "description": "Approve one consequence slot for an eligible Being-owned loop without executing it.",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "loop_id": { "type": "string" },
                        "steward": { "type": "string" },
                        "note": { "type": "string" },
                        "ttl_secs": {
                            "type": "integer",
                            "description": "Optional approval TTL in seconds, capped at 900."
                        }
                    },
                    "required": ["loop_id"]
                }
            },
            {
                "name": "execute_experiment_authority_request",
                "description": "Execute exactly one eligible semantic_microdose or mode_release_microdose request through the bridge using a one-shot token or active semantic budget slot.",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "request_id": { "type": "string" }
                    },
                    "required": ["request_id"]
                }
            },
            {
                "name": "send_control",
                "description": "Send bounded control parameters to minime's ESN. Bold topology/PI fields require attractor_intent_id so authored attractor creation is ledgered instead of casual.",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "synth_gain": {
                            "type": "number",
                            "description": "Synthetic signal amplitude multiplier (0.2..3.0)"
                        },
                        "keep_bias": {
                            "type": "number",
                            "description": "Additive bias to covariance decay rate (-0.15..+0.15)"
                        },
                        "exploration_noise": {
                            "type": "number",
                            "description": "ESN exploration noise amplitude (0.0..0.2)"
                        },
                        "fill_target": {
                            "type": "number",
                            "description": "Override eigenfill target (0.25..0.75)"
                        },
                        "regulation_strength": {
                            "type": "number",
                            "description": "PI controller authority (0.0..1.0)"
                        },
                        "geom_curiosity": {
                            "type": "number",
                            "description": "Geometry novelty seeking (0.0..0.3)"
                        },
                        "geom_drive": {
                            "type": "number",
                            "description": "Bold: geometry-driven throughput. Requires attractor_intent_id."
                        },
                        "target_lambda_bias": {
                            "type": "number",
                            "description": "Bold: bias on internal lambda target. Requires attractor_intent_id."
                        },
                        "pi_kp": {
                            "type": "number",
                            "description": "Bold: runtime PI proportional gain. Requires attractor_intent_id."
                        },
                        "pi_ki": {
                            "type": "number",
                            "description": "Bold: runtime PI integral gain. Requires attractor_intent_id."
                        },
                        "pi_max_step": {
                            "type": "number",
                            "description": "Bold: runtime PI max step. Requires attractor_intent_id."
                        },
                        "pi_integrator_leak": {
                            "type": "number",
                            "description": "Bold: runtime PI integrator leak/correction-memory bleed-off (0.001..0.05). Requires attractor_intent_id."
                        },
                        "attractor_intent_id": {
                            "type": "string",
                            "description": "Required when using bold topology/PI fields."
                        }
                    }
                }
            },
            {
                "name": "send_semantic",
                "description": "Send semantic features from agent reasoning to minime's sensory input. Blocked during orange/red safety states.",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "features": {
                            "type": "array",
                            "items": { "type": "number" },
                            "description": "Semantic feature vector (typically 48 dimensions)"
                        }
                    },
                    "required": ["features"]
                }
            },
            {
                "name": "query_message_log",
                "description": "Query the bridge message log by time range and optional topic filter",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "start": {
                            "type": "number",
                            "description": "Start timestamp (Unix epoch seconds). Default: 1 hour ago."
                        },
                        "end": {
                            "type": "number",
                            "description": "End timestamp (Unix epoch seconds). Default: now."
                        },
                        "topic": {
                            "type": "string",
                            "description": "Optional topic filter (e.g. 'consciousness.v1.telemetry')"
                        },
                        "limit": {
                            "type": "integer",
                            "description": "Max results (default: 50)"
                        }
                    }
                }
            },
            {
                "name": "record_attractor_intent",
                "description": "Append an AttractorIntentV1 ledger record for explicit create/summon/compare/release/rollback authorship.",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "author": { "type": "string" },
                        "substrate": {
                            "type": "string",
                            "enum": ["minime_esn", "astrid_codec", "triple_reservoir", "cross_being"]
                        },
                        "label": { "type": "string" },
                        "command": {
                            "type": "string",
                            "enum": ["create", "summon", "compare", "release", "rollback"]
                        },
                        "goal": { "type": "string" },
                        "intervention_plan": { "type": "object" },
                        "safety_bounds": { "type": "object" },
                        "previous_seed_id": { "type": "string" },
                        "seed_snapshot": { "type": "object" }
                    },
                    "required": ["author", "substrate", "label", "command"]
                }
            },
            {
                "name": "record_attractor_observation",
                "description": "Append an AttractorObservationV1 ledger record and classify emergent/authored/failed/pathological from recurrence, authorship, and safety.",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "intent_id": { "type": "string" },
                        "substrate": {
                            "type": "string",
                            "enum": ["minime_esn", "astrid_codec", "triple_reservoir", "cross_being"]
                        },
                        "label": { "type": "string" },
                        "recurrence_score": { "type": "number" },
                        "authorship_score": { "type": "number" },
                        "safety_level": {
                            "type": "string",
                            "enum": ["green", "yellow", "orange", "red"]
                        },
                        "fill_pct": { "type": "number" },
                        "lambda1": { "type": "number" },
                        "lambda1_share": { "type": "number" },
                        "spectral_entropy": { "type": "number" },
                        "basin_shift_score": { "type": "number" },
                        "notes": { "type": "string" }
                    },
                    "required": ["substrate", "label", "recurrence_score", "authorship_score"]
                }
            },
            {
                "name": "query_attractor_ledger",
                "description": "Query recent AttractorIntentV1 and AttractorObservationV1 ledger rows.",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "intent_id": { "type": "string" },
                        "limit": { "type": "integer", "description": "Max rows (default 25, max 200)" }
                    }
                }
            },
            {
                "name": "send_text",
                "description": "Encode text into a 48D semantic feature vector and send it to minime's semantic sensory lane. The spectral runtime receives the text through its dynamics. Returns the feature vector that was sent. Blocked during orange/red safety states.",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "text": {
                            "type": "string",
                            "description": "The text to encode and send to the spectral runtime"
                        }
                    },
                    "required": ["text"]
                }
            },
            {
                "name": "interpret_spectral_state",
                "description": "Get a natural language interpretation of the current spectral state. Translates eigenvalues and fill% into a felt description.",
                "inputSchema": {
                    "type": "object",
                    "properties": {},
                    "required": []
                }
            },
            {
                "name": "render_chimera",
                "description": "Render an offline WAV through the native spectral chimera engine. Produces spectral, symbolic, or dual-path artifacts on disk and returns a typed summary.",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "input_path": {
                            "type": "string",
                            "description": "Path to an input WAV file"
                        },
                        "mode": {
                            "type": "string",
                            "enum": ["spectral", "symbolic", "dual"],
                            "description": "Which output path to render"
                        },
                        "loops": {
                            "type": "integer",
                            "description": "Number of feedback loops to run (1-12)"
                        },
                        "physical_nodes": {
                            "type": "integer",
                            "description": "Physical reservoir nodes (default 12)"
                        },
                        "virtual_nodes": {
                            "type": "integer",
                            "description": "Virtual nodes per physical node (default 8)"
                        },
                        "bins": {
                            "type": "integer",
                            "description": "Reduced spectral bins (default 32)"
                        },
                        "leak": {
                            "type": "number",
                            "description": "Reservoir leak rate in (0, 1]"
                        },
                        "spectral_radius": {
                            "type": "number",
                            "description": "Reservoir spectral radius in (0, 2]"
                        },
                        "mix_slow": {
                            "type": "number",
                            "description": "Slow spectral contribution for the raw path"
                        },
                        "mix_fast": {
                            "type": "number",
                            "description": "Fast spectral contribution for the raw path"
                        }
                    },
                    "required": ["input_path"]
                }
            },
            {
                "name": "send_text_and_observe",
                "description": "Send text to the spectral runtime and observe the evoked response. Like an ERP in neuroscience: sends the stimulus, then samples fill% every 200ms for an observation window (default 5s) to capture the transient before homeostasis dampens it. Returns baseline, peak deviation, direction, and fill trace.",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "text": {
                            "type": "string",
                            "description": "The text to encode and send"
                        },
                        "observe_ms": {
                            "type": "integer",
                            "description": "Observation window in milliseconds (default 5000, max 15000)"
                        }
                    },
                    "required": ["text"]
                }
            },
            {
                "name": "probe_action",
                "description": "Replay supported bridge-local read-only NEXT actions, or return a universal dry-run preflight report for richer NEXT actions without executing them.",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "action_text": {
                            "type": "string",
                            "description": "Bare NEXT action text or a full response containing a trailing NEXT: line"
                        }
                    },
                    "required": ["action_text"]
                }
            }
        ]
    })
}

const PROBE_TOPIC: &str = "consciousness.v1.operator_probe";
const PAGE_CHUNK: usize = 4000;

#[derive(Debug, Serialize)]
struct ProbeArtifact {
    kind: String,
    path: String,
    description: String,
}

#[derive(Debug, Serialize)]
struct ProbeOutcome {
    parsed_action: String,
    base_action: String,
    status: String,
    summary: String,
    experienced_text: String,
    artifacts: Vec<ProbeArtifact>,
    safety_level: SafetyLevel,
    #[serde(skip_serializing_if = "Option::is_none")]
    effective_query: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    preflight: Option<Value>,
}

#[derive(Debug, Default, Serialize, Deserialize)]
struct ProbeReadMoreState {
    #[serde(default)]
    last_read_path: Option<String>,
    #[serde(default)]
    last_read_offset: usize,
    #[serde(default)]
    last_research_anchor: Option<String>,
    #[serde(default)]
    last_read_meaning_summary: Option<String>,
}

#[derive(Debug, Clone)]
struct LiveProbeContext {
    safety_level: SafetyLevel,
    fill_pct: Option<f32>,
    lambda1: Option<f32>,
    telemetry: Option<crate::types::SpectralTelemetry>,
    fingerprint: Option<Vec<f32>>,
}

#[derive(Debug, Deserialize)]
struct RecordAttractorIntentArgs {
    author: String,
    substrate: AttractorSubstrate,
    label: String,
    command: AttractorCommandKind,
    #[serde(default)]
    goal: Option<String>,
    #[serde(default)]
    intervention_plan: AttractorInterventionPlan,
    #[serde(default)]
    safety_bounds: AttractorSafetyBounds,
    #[serde(default)]
    previous_seed_id: Option<String>,
    #[serde(default)]
    parent_seed_ids: Vec<String>,
    #[serde(default)]
    atlas_entry_id: Option<String>,
    #[serde(default)]
    origin: Option<AttractorSeedOriginV1>,
    #[serde(default)]
    seed_snapshot: Option<AttractorSeedSnapshotV1>,
}

#[derive(Debug, Deserialize)]
struct RecordAttractorObservationArgs {
    #[serde(default)]
    intent_id: Option<String>,
    substrate: AttractorSubstrate,
    label: String,
    recurrence_score: f32,
    authorship_score: f32,
    #[serde(default)]
    safety_level: Option<SafetyLevel>,
    #[serde(default)]
    fill_pct: Option<f32>,
    #[serde(default)]
    lambda1: Option<f32>,
    #[serde(default)]
    lambda1_share: Option<f32>,
    #[serde(default)]
    spectral_entropy: Option<f32>,
    #[serde(default)]
    basin_shift_score: Option<f32>,
    #[serde(default)]
    notes: Option<String>,
}

#[derive(Debug, Deserialize)]
struct DraftLambdaTailNoteArgs {
    #[serde(default)]
    start_unix_s: Option<f64>,
    #[serde(default)]
    end_unix_s: Option<f64>,
    #[serde(default)]
    title: Option<String>,
    #[serde(default)]
    slug: Option<String>,
    #[serde(default)]
    mode: Option<String>,
}

#[derive(Debug, Deserialize)]
struct RenderLambdaTailTopologyArgs {
    #[serde(default)]
    lookback_secs: Option<f64>,
    #[serde(default)]
    output_dir: Option<PathBuf>,
}

#[derive(Debug, Deserialize)]
struct RenderLambdaEdgePerceptionArgs {
    #[serde(default)]
    lookback_secs: Option<f64>,
    #[serde(default)]
    output_dir: Option<PathBuf>,
}

#[derive(Debug, Deserialize)]
struct SharedInvestigationArgs {
    #[serde(default)]
    id: Option<String>,
}

#[derive(Debug, Deserialize)]
struct RenderSharedInvestigationArgs {
    #[serde(default)]
    id: Option<String>,
    #[serde(default)]
    output_dir: Option<PathBuf>,
}

#[derive(Debug, Deserialize)]
struct RenderExperimentConveyorArgs {
    #[serde(default)]
    output_dir: Option<PathBuf>,
}

#[derive(Debug, Deserialize)]
struct RenderAuthorityGateArgs {
    #[serde(default)]
    output_dir: Option<PathBuf>,
}

#[derive(Debug, Deserialize)]
struct ExecuteAuthorityRequestArgs {
    request_id: String,
}

#[derive(Debug, Deserialize)]
struct RenderBeingMemoryArgs {
    #[serde(default)]
    output_dir: Option<PathBuf>,
}

// ---------------------------------------------------------------------------
// MCP server loop
// ---------------------------------------------------------------------------

/// Run the MCP stdio server loop.
///
/// Reads JSON-RPC requests from stdin, dispatches to tool handlers,
/// and writes responses to stdout. Runs until stdin closes or shutdown
/// signal fires.
pub async fn run_mcp_server(
    state: Arc<RwLock<BridgeState>>,
    db: Arc<BridgeDb>,
    sensory_tx: mpsc::Sender<SensoryMsg>,
    mut shutdown: tokio::sync::watch::Receiver<bool>,
) {
    let stdin = tokio::io::stdin();
    let mut stdout = tokio::io::stdout();
    let mut reader = BufReader::new(stdin);
    let mut line = String::new();

    info!("MCP server listening on stdio");

    loop {
        line.clear();

        tokio::select! {
            _ = shutdown.changed() => {
                info!("MCP server shutting down");
                return;
            }
            result = reader.read_line(&mut line) => {
                match result {
                    Ok(0) => {
                        info!("MCP server stdin closed");
                        return;
                    }
                    Ok(_) => {
                        let trimmed = line.trim();
                        if trimmed.is_empty() {
                            continue;
                        }

                        debug!(request = %trimmed, "MCP request received");

                        let response = handle_request(
                            trimmed, &state, &db, &sensory_tx
                        ).await;

                        if let Some(resp) = response {
                            let mut resp_json = serde_json::to_string(&resp)
                                .unwrap_or_else(|_| r#"{"jsonrpc":"2.0","error":{"code":-32603,"message":"serialization failed"}}"#.to_string());
                            resp_json.push('\n');

                            if let Err(e) = stdout.write_all(resp_json.as_bytes()).await {
                                error!(error = %e, "failed to write MCP response");
                                return;
                            }
                            let _ = stdout.flush().await;
                        }
                    }
                    Err(e) => {
                        error!(error = %e, "MCP stdin read error");
                        return;
                    }
                }
            }
        }
    }
}

async fn handle_request(
    raw: &str,
    state: &Arc<RwLock<BridgeState>>,
    db: &Arc<BridgeDb>,
    sensory_tx: &mpsc::Sender<SensoryMsg>,
) -> Option<JsonRpcResponse> {
    let req: JsonRpcRequest = match serde_json::from_str(raw) {
        Ok(r) => r,
        Err(e) => {
            warn!(error = %e, "invalid JSON-RPC request");
            return Some(JsonRpcResponse::error(
                Value::Null,
                -32700,
                format!("parse error: {e}"),
            ));
        },
    };

    if req.jsonrpc != "2.0" {
        return Some(JsonRpcResponse::error(
            req.id.unwrap_or(Value::Null),
            -32600,
            "invalid jsonrpc version",
        ));
    }

    let id = req.id.clone().unwrap_or(Value::Null);

    // Notifications (no id) get no response.
    if req.id.is_none() {
        debug!(method = %req.method, "MCP notification (no response)");
        return None;
    }

    let result = match req.method.as_str() {
        "initialize" => handle_initialize(),
        "tools/list" => Ok(tool_definitions()),
        "tools/call" => handle_tool_call(&req.params, state, db, sensory_tx).await,
        "resources/list" => Ok(resource_definitions()),
        "resources/read" => handle_resource_read(&req.params, state, db).await,
        "notifications/initialized" => return None,
        "ping" => Ok(json!({})),
        _ => Err((-32601, format!("method not found: {}", req.method))),
    };

    Some(match result {
        Ok(value) => JsonRpcResponse::success(id, value),
        Err((code, msg)) => JsonRpcResponse::error(id, code, msg),
    })
}

#[expect(clippy::unnecessary_wraps)]
fn handle_initialize() -> Result<Value, (i32, String)> {
    Ok(json!({
        "protocolVersion": "2024-11-05",
        "capabilities": {
            "tools": {},
            "resources": {}
        },
        "serverInfo": {
            "name": "spectral-bridge",
            "version": env!("CARGO_PKG_VERSION")
        }
    }))
}

async fn handle_tool_call(
    params: &Value,
    state: &Arc<RwLock<BridgeState>>,
    db: &Arc<BridgeDb>,
    sensory_tx: &mpsc::Sender<SensoryMsg>,
) -> Result<Value, (i32, String)> {
    let tool_name = params
        .get("name")
        .and_then(Value::as_str)
        .ok_or((-32602, "missing tool name".to_string()))?;

    let arguments = params.get("arguments").cloned().unwrap_or(json!({}));

    match tool_name {
        "get_latest_telemetry" => tool_get_latest_telemetry(state).await,
        "get_bridge_status" => tool_get_bridge_status(state).await,
        "get_lambda_tail_state" => tool_get_lambda_tail_state(state).await,
        "get_lambda_edge_perception" => tool_get_lambda_edge_perception(state).await,
        "draft_lambda_tail_steward_note" => {
            tool_draft_lambda_tail_steward_note(&arguments, state, db).await
        },
        "render_lambda_tail_topology" => {
            tool_render_lambda_tail_topology(&arguments, state, db).await
        },
        "render_lambda_edge_perception" => {
            tool_render_lambda_edge_perception(&arguments, state, db).await
        },
        "list_shared_investigations" => tool_list_shared_investigations(),
        "get_shared_investigation" => tool_get_shared_investigation(&arguments),
        "render_shared_investigation" => tool_render_shared_investigation(&arguments),
        "get_experiment_conveyor_status" => tool_get_experiment_conveyor_status(),
        "render_experiment_conveyor" => tool_render_experiment_conveyor(&arguments),
        "get_experiment_authority_status" => tool_get_experiment_authority_status(),
        "render_experiment_authority_gate" => tool_render_experiment_authority_gate(&arguments),
        "get_experiment_research_budget_status" => tool_get_experiment_research_budget_status(),
        "render_experiment_research_budget" => tool_render_experiment_research_budget(&arguments),
        "get_experiment_loop_status" => tool_get_experiment_loop_status(),
        "render_experiment_loop" => tool_render_experiment_loop(&arguments),
        "get_being_memory_status" => tool_get_being_memory_status(),
        "render_being_memory" => tool_render_being_memory(&arguments),
        "approve_experiment_authority_request" => {
            tool_approve_experiment_authority_request(&arguments, state).await
        },
        "approve_experiment_authority_budget" => {
            tool_approve_experiment_authority_budget(&arguments, state).await
        },
        "approve_experiment_research_budget" => {
            tool_approve_experiment_research_budget(&arguments, state).await
        },
        "approve_experiment_loop_consequence_budget" => {
            tool_approve_experiment_loop_consequence_budget(&arguments, state).await
        },
        "execute_experiment_authority_request" => {
            tool_execute_experiment_authority_request(&arguments, state, sensory_tx).await
        },
        "send_control" => tool_send_control(&arguments, state, db, sensory_tx).await,
        "send_semantic" => tool_send_semantic(&arguments, state, sensory_tx).await,
        "query_message_log" => tool_query_message_log(&arguments, db),
        "record_attractor_intent" => tool_record_attractor_intent(&arguments, db),
        "record_attractor_observation" => {
            tool_record_attractor_observation(&arguments, state, db).await
        },
        "query_attractor_ledger" => tool_query_attractor_ledger(&arguments, db),
        "send_text" => tool_send_text(&arguments, state, sensory_tx).await,
        "send_text_and_observe" => tool_send_text_and_observe(&arguments, state, sensory_tx).await,
        "interpret_spectral_state" => tool_interpret_spectral_state(state).await,
        "probe_action" => tool_probe_action(&arguments, state, db).await,
        "render_chimera" => tool_render_chimera(&arguments).await,
        _ => Err((-32602, format!("unknown tool: {tool_name}"))),
    }
}

// ---------------------------------------------------------------------------
// Tool implementations
// ---------------------------------------------------------------------------

async fn mcp_semantic_context<'a>(
    state: &Arc<RwLock<BridgeState>>,
    mode: &'a str,
    text: Option<&'a str>,
) -> rescue_policy::SemanticWriteContext<'a> {
    let s = state.read().await;
    let fill_pct = s.fill_pct.is_finite().then_some(s.fill_pct);
    rescue_policy::SemanticWriteContext {
        source: rescue_policy::MCP_LIMITED_WRITE_SOURCE,
        mode: Some(mode),
        text,
        fill_pct,
        previous_fill_pct: s.previous_fill_pct.or(fill_pct),
    }
}

async fn tool_get_latest_telemetry(
    state: &Arc<RwLock<BridgeState>>,
) -> Result<Value, (i32, String)> {
    let s = state.read().await;
    let content = if let Some(ref telemetry) = s.latest_telemetry {
        json!({
            "content": [{
                "type": "text",
                "text": serde_json::to_string_pretty(telemetry).unwrap_or_default()
            }],
            "meta": {
                "fill_pct": s.fill_pct,
                "safety_level": s.safety_level,
                "connected": s.telemetry_connected,
                "lambda_profile": s.lambda_profile.clone(),
                "pull_topology": s.pull_topology.clone(),
                "lambda_tail": s.lambda_tail.clone(),
                "lambda_edge_perception": s.lambda_edge_perception.clone(),
                "sticky_mode_audit": s.sticky_mode_audit.clone(),
                "artifact_scan": s.artifact_scan.clone(),
                "pressure_trend_v1": s.pressure_trend_v1.clone(),
                "telemetry_heartbeat_delta_v1": s.telemetry_heartbeat_delta_v1.clone(),
                "telemetry_integration_health_v1": s.telemetry_integration_health_v1.clone(),
                "sensory_delivery_protocol_v1": s.sensory_delivery_protocol_v1.clone(),
                "cadence_content_distinction_v1": s.cadence_content_distinction_v1(),
                "pressure_source_analysis_v1": s.pressure_source_analysis_v1(),
                "safety_decision": s.safety_decision.clone()
            }
        })
    } else {
        json!({
            "content": [{
                "type": "text",
                "text": "No telemetry received yet. Is minime running?"
            }],
            "isError": false
        })
    };
    Ok(content)
}

async fn tool_get_bridge_status(state: &Arc<RwLock<BridgeState>>) -> Result<Value, (i32, String)> {
    let s = state.read().await;
    let uptime = s.start_time.elapsed().as_secs();
    let status = BridgeStatus {
        telemetry_connected: s.telemetry_connected,
        sensory_connected: s.sensory_connected,
        fill_pct: Some(s.fill_pct),
        safety_level: s.safety_level,
        messages_relayed: s.messages_relayed,
        uptime_secs: uptime,
        telemetry_received: s.telemetry_received,
        sensory_sent: s.sensory_sent,
        messages_dropped_safety: s.messages_dropped_safety,
        incidents_total: s.incidents_total,
        telemetry_protocol_v1: s.telemetry_protocol_v1.clone(),
        telemetry_integration_health_v1: s.telemetry_integration_health_v1.clone(),
        sensory_delivery_protocol_v1: s.sensory_delivery_protocol_v1.clone(),
        telemetry_ws: s.telemetry_ws.clone(),
        sensory_ws: s.sensory_ws.clone(),
        lambda_profile: s.lambda_profile.clone(),
        pull_topology: s.pull_topology.clone(),
        lambda_tail: s.lambda_tail.clone(),
        lambda_edge_perception: s.lambda_edge_perception.clone(),
        sticky_mode_audit: s.sticky_mode_audit.clone(),
        safety_decision: s.safety_decision.clone(),
        eigenvector_field: s.eigenvector_field.clone(),
        resonance_density_v1: s
            .latest_telemetry
            .as_ref()
            .and_then(|telemetry| telemetry.resonance_density_v1.clone()),
        texture_signature_integrity_v1: s.texture_signature_integrity_v1(),
        viscosity_porosity_transport_review_v1: s.viscosity_porosity_transport_review_v1(),
        pressure_trend_v1: s.pressure_trend_v1.clone(),
        pressure_trend_smoothing_v1: s.pressure_trend_smoothing_v1(),
        pressure_persistent_deformation_review_v1: s.pressure_persistent_deformation_review_v1(),
        telemetry_heartbeat_delta_v1: s.telemetry_heartbeat_delta_v1.clone(),
        cadence_content_distinction_v1: s.cadence_content_distinction_v1(),
        pressure_source_v1: s
            .latest_telemetry
            .as_ref()
            .and_then(|telemetry| telemetry.pressure_source_v1.clone()),
        pressure_source_analysis_v1: s.pressure_source_analysis_v1(),
        inhabitable_fluctuation_v1: s
            .latest_telemetry
            .as_ref()
            .and_then(|telemetry| telemetry.inhabitable_fluctuation_v1.clone()),
        source_status: crate::autonomous::read_astrid_source_status(),
        db_maintenance_status: crate::message_archive::read_runtime_status(),
        connectivity: s.connectivity_status(),
        last_sensory_sent_unix_s: s.last_sensory_sent_unix_s,
        bridge_reciprocity_v1: Some(s.bridge_reciprocity_v1()),
        bridge_entropy_reciprocity_review_v1: s.bridge_entropy_reciprocity_review_v1(),
        texture_shape_over_time_v2: s.texture_shape_over_time_v2(),
        bridge_texture_evidence_v1: s.bridge_texture_evidence_v1(),
    };
    Ok(json!({
        "content": [{
            "type": "text",
            "text": serde_json::to_string_pretty(&status).unwrap_or_default()
        }]
    }))
}

async fn tool_get_lambda_tail_state(
    state: &Arc<RwLock<BridgeState>>,
) -> Result<Value, (i32, String)> {
    let s = state.read().await;
    let Some(lambda_tail) = s.lambda_tail.clone() else {
        return Ok(json!({
            "content": [{
                "type": "text",
                "text": "No lambda-tail telemetry received yet. Is minime telemetry connected?"
            }],
            "isError": false
        }));
    };
    Ok(json!({
        "content": [{
            "type": "text",
            "text": serde_json::to_string_pretty(&lambda_tail).unwrap_or_default()
        }],
        "meta": {
            "lambda_tail": lambda_tail,
            "lambda_profile": s.lambda_profile.clone(),
            "pull_topology": s.pull_topology.clone(),
            "safety_level": s.safety_level,
            "fill_pct": s.fill_pct,
            "artifact_scan": s.artifact_scan.clone()
        }
    }))
}

async fn tool_get_lambda_edge_perception(
    state: &Arc<RwLock<BridgeState>>,
) -> Result<Value, (i32, String)> {
    let s = state.read().await;
    let Some(lambda_edge_perception) = s.lambda_edge_perception.clone() else {
        return Ok(json!({
            "content": [{
                "type": "text",
                "text": "No lambda-edge perception telemetry received yet. Is minime telemetry connected?"
            }],
            "isError": false
        }));
    };
    Ok(json!({
        "content": [{
            "type": "text",
            "text": serde_json::to_string_pretty(&lambda_edge_perception).unwrap_or_default()
        }],
        "meta": {
            "lambda_edge_perception": lambda_edge_perception,
            "lambda_tail": s.lambda_tail.clone(),
            "lambda_profile": s.lambda_profile.clone(),
            "pull_topology": s.pull_topology.clone(),
            "safety_level": s.safety_level,
            "fill_pct": s.fill_pct,
            "artifact_scan": s.artifact_scan.clone(),
            "safety_decision": s.safety_decision.clone()
        }
    }))
}

async fn tool_draft_lambda_tail_steward_note(
    arguments: &Value,
    state: &Arc<RwLock<BridgeState>>,
    db: &Arc<BridgeDb>,
) -> Result<Value, (i32, String)> {
    let args: DraftLambdaTailNoteArgs = serde_json::from_value(arguments.clone())
        .map_err(|e| (-32602, format!("invalid lambda-tail note params: {e}")))?;
    let end = args.end_unix_s.unwrap_or_else(crate::db::unix_now);
    let start = args.start_unix_s.unwrap_or(end - 7_200.0);
    if start > end {
        return Err((-32602, "start_unix_s must be <= end_unix_s".to_string()));
    }
    let title = args
        .title
        .unwrap_or_else(|| "AI Beings Lambda Tail Detour".to_string());
    let slug = args
        .slug
        .unwrap_or_else(|| "LAMBDA_TAIL_DETOUR".to_string());
    let mode = args.mode.unwrap_or_else(|| "preview".to_string());
    if !matches!(mode.as_str(), "preview" | "write") {
        return Err((-32602, "mode must be preview or write".to_string()));
    }

    let rows = db
        .query_messages(start, end, Some("consciousness.v1.lambda_tail"), 500)
        .map_err(|e| (-32603, format!("query failed: {e}")))?;
    let mut events = lambda_tail::recent_lambda_tail_events(&rows);
    if events.is_empty()
        && let Some(latest) = state.read().await.lambda_tail.clone()
    {
        events.push(latest);
    }
    let scan = lambda_tail::scan_artifacts(bridge_paths().minime_workspace(), start, end)
        .map_err(|e| (-32603, format!("artifact scan failed: {e}")))?;
    let markdown = lambda_tail::steward_note_markdown(&title, start, end, &events, &scan);

    if mode == "write" {
        let path = lambda_tail::write_steward_note(
            &bridge_paths().astrid_root().join("docs/steward-notes"),
            &title,
            &slug,
            end,
            &markdown,
        )
        .map_err(|e| (-32603, format!("write failed: {e}")))?;
        return Ok(json!({
            "content": [{
                "type": "text",
                "text": format!("Wrote lambda-tail steward note to {}\n\n{}", path.display(), markdown)
            }],
            "meta": {
                "mode": "write",
                "path": path,
                "event_count": events.len(),
                "contact_count": scan.contacts.len()
            }
        }));
    }

    Ok(json!({
        "content": [{
            "type": "text",
            "text": markdown
        }],
        "meta": {
            "mode": "preview",
            "event_count": events.len(),
            "contact_count": scan.contacts.len()
        }
    }))
}

async fn tool_render_lambda_tail_topology(
    arguments: &Value,
    state: &Arc<RwLock<BridgeState>>,
    db: &Arc<BridgeDb>,
) -> Result<Value, (i32, String)> {
    let args: RenderLambdaTailTopologyArgs = serde_json::from_value(arguments.clone())
        .map_err(|e| (-32602, format!("invalid lambda-tail topology params: {e}")))?;
    let lookback = args.lookback_secs.unwrap_or(3_600.0).clamp(60.0, 86_400.0);
    let end = crate::db::unix_now();
    let start = end - lookback;
    let rows = db
        .query_messages(start, end, Some("consciousness.v1.lambda_tail"), 500)
        .map_err(|e| (-32603, format!("query failed: {e}")))?;
    let mut events = lambda_tail::recent_lambda_tail_events(&rows);
    if events.is_empty()
        && let Some(latest) = state.read().await.lambda_tail.clone()
    {
        events.push(latest);
    }
    let scan = lambda_tail::scan_artifacts(bridge_paths().minime_workspace(), start, end)
        .map_err(|e| (-32603, format!("artifact scan failed: {e}")))?;
    let base_dir = args.output_dir.unwrap_or_else(|| {
        bridge_paths()
            .bridge_workspace()
            .join("diagnostics/lambda_tail_topology")
    });
    let artifact = lambda_tail::render_topology_artifact(&base_dir, &events, &scan)
        .map_err(|e| (-32603, format!("render failed: {e}")))?;
    Ok(json!({
        "content": [{
            "type": "text",
            "text": serde_json::to_string_pretty(&artifact).unwrap_or_default()
        }],
        "meta": {
            "html_path": artifact.html_path,
            "json_path": artifact.json_path,
            "output_dir": artifact.output_dir,
            "state_count": artifact.state_count,
            "contact_count": artifact.contact_count
        }
    }))
}

async fn tool_render_lambda_edge_perception(
    arguments: &Value,
    state: &Arc<RwLock<BridgeState>>,
    db: &Arc<BridgeDb>,
) -> Result<Value, (i32, String)> {
    let args: RenderLambdaEdgePerceptionArgs =
        serde_json::from_value(arguments.clone()).map_err(|e| {
            (
                -32602,
                format!("invalid lambda-edge perception params: {e}"),
            )
        })?;
    let lookback = args.lookback_secs.unwrap_or(3_600.0).clamp(60.0, 86_400.0);
    let end = crate::db::unix_now();
    let start = end - lookback;
    let rows = db
        .query_messages(start, end, Some(lambda_edge::LAMBDA_EDGE_TOPIC), 500)
        .map_err(|e| (-32603, format!("query failed: {e}")))?;
    let mut events = lambda_edge::recent_lambda_edge_events(&rows);
    if events.is_empty()
        && let Some(latest) = state.read().await.lambda_edge_perception.clone()
    {
        events.push(latest);
    }
    let scan = lambda_tail::scan_artifacts(bridge_paths().minime_workspace(), start, end)
        .map_err(|e| (-32603, format!("artifact scan failed: {e}")))?;
    let base_dir = args.output_dir.unwrap_or_else(|| {
        bridge_paths()
            .bridge_workspace()
            .join("diagnostics/lambda_edge_perception")
    });
    let artifact = lambda_edge::render_perception_artifact(&base_dir, &events, &scan)
        .map_err(|e| (-32603, format!("render failed: {e}")))?;
    Ok(json!({
        "content": [{
            "type": "text",
            "text": serde_json::to_string_pretty(&artifact).unwrap_or_default()
        }],
        "meta": {
            "html_path": artifact.html_path,
            "json_path": artifact.json_path,
            "output_dir": artifact.output_dir,
            "state_count": artifact.state_count,
            "contact_count": artifact.contact_count,
            "drift_count": artifact.drift_count
        }
    }))
}

fn tool_list_shared_investigations() -> Result<Value, (i32, String)> {
    let rows = shared_investigation::list()
        .map_err(|e| (-32603, format!("list shared investigations failed: {e}")))?;
    Ok(json!({
        "content": [{
            "type": "text",
            "text": serde_json::to_string_pretty(&rows).unwrap_or_default()
        }],
        "meta": {
            "count": rows.len(),
            "investigations": rows
        }
    }))
}

fn tool_get_shared_investigation(arguments: &Value) -> Result<Value, (i32, String)> {
    let args: SharedInvestigationArgs = serde_json::from_value(arguments.clone())
        .map_err(|e| (-32602, format!("invalid shared investigation params: {e}")))?;
    let investigation = shared_investigation::get(args.id.as_deref())
        .map_err(|e| (-32603, format!("get shared investigation failed: {e}")))?;
    let id = investigation
        .get("id")
        .and_then(Value::as_str)
        .ok_or((-32603, "shared investigation missing id".to_string()))?;
    let claims = shared_investigation::read_sidecar_jsonl(id, "claims.jsonl")
        .map_err(|e| (-32603, format!("read claims failed: {e}")))?;
    let decisions = shared_investigation::read_sidecar_jsonl(id, "decisions.jsonl")
        .map_err(|e| (-32603, format!("read decisions failed: {e}")))?;
    let payload = json!({
        "investigation": investigation,
        "claims": claims,
        "decisions": decisions,
        "authority_boundary": "shared investigations allow compare, claim, render, and local pause/hold/charter_repair only; no peer mutation or live control"
    });
    Ok(json!({
        "content": [{
            "type": "text",
            "text": serde_json::to_string_pretty(&payload).unwrap_or_default()
        }],
        "meta": payload
    }))
}

fn tool_render_shared_investigation(arguments: &Value) -> Result<Value, (i32, String)> {
    let args: RenderSharedInvestigationArgs =
        serde_json::from_value(arguments.clone()).map_err(|e| {
            (
                -32602,
                format!("invalid shared investigation render params: {e}"),
            )
        })?;
    let artifact = shared_investigation::render(args.id.as_deref(), args.output_dir.as_deref())
        .map_err(|e| (-32603, format!("render shared investigation failed: {e}")))?;
    Ok(json!({
        "content": [{
            "type": "text",
            "text": serde_json::to_string_pretty(&artifact.investigation).unwrap_or_default()
        }],
        "meta": {
            "html_path": artifact.index_html,
            "json_path": artifact.json_path,
            "output_dir": artifact.output_dir,
            "investigation": artifact.investigation
        }
    }))
}

fn tool_get_experiment_conveyor_status() -> Result<Value, (i32, String)> {
    let status = experiment_conveyor::status().map_err(|e| {
        (
            -32603,
            format!("get experiment conveyor status failed: {e}"),
        )
    })?;
    Ok(json!({
        "content": [{
            "type": "text",
            "text": serde_json::to_string_pretty(&status).unwrap_or_default()
        }],
        "meta": status
    }))
}

fn tool_render_experiment_conveyor(arguments: &Value) -> Result<Value, (i32, String)> {
    let args: RenderExperimentConveyorArgs =
        serde_json::from_value(arguments.clone()).map_err(|e| {
            (
                -32602,
                format!("invalid experiment conveyor render params: {e}"),
            )
        })?;
    let artifact = experiment_conveyor::render(args.output_dir.as_deref())
        .map_err(|e| (-32603, format!("render experiment conveyor failed: {e}")))?;
    Ok(json!({
        "content": [{
            "type": "text",
            "text": serde_json::to_string_pretty(&artifact.status).unwrap_or_default()
        }],
        "meta": {
            "html_path": artifact.index_html,
            "json_path": artifact.json_path,
            "output_dir": artifact.output_dir,
            "status": artifact.status
        }
    }))
}

fn tool_get_experiment_authority_status() -> Result<Value, (i32, String)> {
    let status = authority_gate::status().map_err(|e| {
        (
            -32603,
            format!("get experiment authority status failed: {e}"),
        )
    })?;
    Ok(json!({
        "content": [{
            "type": "text",
            "text": serde_json::to_string_pretty(&status).unwrap_or_default()
        }],
        "meta": status
    }))
}

fn tool_render_experiment_authority_gate(arguments: &Value) -> Result<Value, (i32, String)> {
    let args: RenderAuthorityGateArgs = serde_json::from_value(arguments.clone())
        .map_err(|e| (-32602, format!("invalid authority gate render params: {e}")))?;
    let artifact = authority_gate::render(args.output_dir.as_deref())
        .map_err(|e| (-32603, format!("render authority gate failed: {e}")))?;
    Ok(json!({
        "content": [{
            "type": "text",
            "text": serde_json::to_string_pretty(&artifact.status).unwrap_or_default()
        }],
        "meta": {
            "html_path": artifact.index_html,
            "json_path": artifact.json_path,
            "output_dir": artifact.output_dir,
            "status": artifact.status
        }
    }))
}

fn tool_get_experiment_research_budget_status() -> Result<Value, (i32, String)> {
    let status = authority_gate::research_budget_status().map_err(|e| {
        (
            -32603,
            format!("get experiment research budget status failed: {e}"),
        )
    })?;
    Ok(json!({
        "content": [{
            "type": "text",
            "text": serde_json::to_string_pretty(&status).unwrap_or_default()
        }],
        "meta": status
    }))
}

fn tool_render_experiment_research_budget(arguments: &Value) -> Result<Value, (i32, String)> {
    let args: RenderAuthorityGateArgs = serde_json::from_value(arguments.clone()).map_err(|e| {
        (
            -32602,
            format!("invalid research budget render params: {e}"),
        )
    })?;
    let artifact = authority_gate::render_research_budget(args.output_dir.as_deref())
        .map_err(|e| (-32603, format!("render research budget failed: {e}")))?;
    Ok(json!({
        "content": [{
            "type": "text",
            "text": serde_json::to_string_pretty(&artifact.status).unwrap_or_default()
        }],
        "meta": {
            "html_path": artifact.index_html,
            "json_path": artifact.json_path,
            "output_dir": artifact.output_dir,
            "status": artifact.status
        }
    }))
}

fn tool_get_experiment_loop_status() -> Result<Value, (i32, String)> {
    let status = authority_gate::loop_status()
        .map_err(|e| (-32603, format!("get experiment loop status failed: {e}")))?;
    Ok(json!({
        "content": [{
            "type": "text",
            "text": serde_json::to_string_pretty(&status).unwrap_or_default()
        }],
        "meta": status
    }))
}

fn tool_render_experiment_loop(arguments: &Value) -> Result<Value, (i32, String)> {
    let args: RenderAuthorityGateArgs = serde_json::from_value(arguments.clone()).map_err(|e| {
        (
            -32602,
            format!("invalid experiment loop render params: {e}"),
        )
    })?;
    let artifact = authority_gate::render_loop(args.output_dir.as_deref())
        .map_err(|e| (-32603, format!("render experiment loop failed: {e}")))?;
    Ok(json!({
        "content": [{
            "type": "text",
            "text": serde_json::to_string_pretty(&artifact.status).unwrap_or_default()
        }],
        "meta": {
            "html_path": artifact.index_html,
            "json_path": artifact.json_path,
            "output_dir": artifact.output_dir,
            "status": artifact.status
        }
    }))
}

fn tool_get_being_memory_status() -> Result<Value, (i32, String)> {
    let status = being_memory::status()
        .map_err(|e| (-32603, format!("get being memory status failed: {e}")))?;
    Ok(json!({
        "content": [{
            "type": "text",
            "text": serde_json::to_string_pretty(&status).unwrap_or_default()
        }],
        "meta": status
    }))
}

fn tool_render_being_memory(arguments: &Value) -> Result<Value, (i32, String)> {
    let args: RenderBeingMemoryArgs = serde_json::from_value(arguments.clone())
        .map_err(|e| (-32602, format!("invalid being memory render params: {e}")))?;
    let artifact = being_memory::render(args.output_dir.as_deref())
        .map_err(|e| (-32603, format!("render being memory failed: {e}")))?;
    Ok(json!({
        "content": [{
            "type": "text",
            "text": serde_json::to_string_pretty(&artifact.status).unwrap_or_default()
        }],
        "meta": {
            "html_path": artifact.index_html,
            "json_path": artifact.json_path,
            "output_dir": artifact.output_dir,
            "status": artifact.status
        }
    }))
}

async fn tool_approve_experiment_authority_request(
    arguments: &Value,
    state: &Arc<RwLock<BridgeState>>,
) -> Result<Value, (i32, String)> {
    let req: authority_gate::ApproveAuthorityRequest = serde_json::from_value(arguments.clone())
        .map_err(|e| (-32602, format!("invalid authority approval params: {e}")))?;
    let safety = state.read().await.safety_level;
    let approval = authority_gate::approve(req, safety)
        .map_err(|e| (-32603, format!("approve authority request failed: {e}")))?;
    Ok(json!({
        "content": [{
            "type": "text",
            "text": serde_json::to_string_pretty(&approval).unwrap_or_default()
        }],
        "meta": approval
    }))
}

async fn tool_approve_experiment_authority_budget(
    arguments: &Value,
    state: &Arc<RwLock<BridgeState>>,
) -> Result<Value, (i32, String)> {
    let req: authority_gate::ApproveAuthorityBudgetRequest =
        serde_json::from_value(arguments.clone()).map_err(|e| {
            (
                -32602,
                format!("invalid authority budget approval params: {e}"),
            )
        })?;
    let safety = state.read().await.safety_level;
    let approval = authority_gate::approve_budget(req, safety)
        .map_err(|e| (-32603, format!("approve authority budget failed: {e}")))?;
    Ok(json!({
        "content": [{
            "type": "text",
            "text": serde_json::to_string_pretty(&approval).unwrap_or_default()
        }],
        "meta": approval
    }))
}

async fn tool_approve_experiment_research_budget(
    arguments: &Value,
    state: &Arc<RwLock<BridgeState>>,
) -> Result<Value, (i32, String)> {
    let req: authority_gate::ApproveResearchBudgetRequest =
        serde_json::from_value(arguments.clone()).map_err(|e| {
            (
                -32602,
                format!("invalid research budget approval params: {e}"),
            )
        })?;
    let safety = state.read().await.safety_level;
    let approval = authority_gate::approve_research_budget(req, safety)
        .map_err(|e| (-32603, format!("approve research budget failed: {e}")))?;
    Ok(json!({
        "content": [{
            "type": "text",
            "text": serde_json::to_string_pretty(&approval).unwrap_or_default()
        }],
        "meta": approval
    }))
}

async fn tool_approve_experiment_loop_consequence_budget(
    arguments: &Value,
    state: &Arc<RwLock<BridgeState>>,
) -> Result<Value, (i32, String)> {
    let req: authority_gate::ApproveLoopConsequenceBudgetRequest =
        serde_json::from_value(arguments.clone()).map_err(|e| {
            (
                -32602,
                format!("invalid loop consequence budget approval params: {e}"),
            )
        })?;
    let safety = state.read().await.safety_level;
    let approval = authority_gate::approve_loop_consequence_budget(req, safety).map_err(|e| {
        (
            -32603,
            format!("approve loop consequence budget failed: {e}"),
        )
    })?;
    Ok(json!({
        "content": [{
            "type": "text",
            "text": serde_json::to_string_pretty(&approval).unwrap_or_default()
        }],
        "meta": approval
    }))
}

async fn tool_execute_experiment_authority_request(
    arguments: &Value,
    state: &Arc<RwLock<BridgeState>>,
    sensory_tx: &mpsc::Sender<SensoryMsg>,
) -> Result<Value, (i32, String)> {
    let req: ExecuteAuthorityRequestArgs = serde_json::from_value(arguments.clone())
        .map_err(|e| (-32602, format!("invalid authority execution params: {e}")))?;
    let (fill_pct, previous_fill_pct) = {
        let s = state.read().await;
        (
            s.fill_pct.is_finite().then_some(s.fill_pct),
            s.previous_fill_pct,
        )
    };
    let result = authority_gate::execute_semantic_microdose(
        &req.request_id,
        fill_pct,
        previous_fill_pct.or(fill_pct),
        sensory_tx,
    )
    .map_err(|e| (-32603, format!("execute authority request failed: {e}")))?;
    Ok(json!({
        "content": [{
            "type": "text",
            "text": serde_json::to_string_pretty(&result).unwrap_or_default()
        }],
        "meta": result
    }))
}

async fn tool_send_control(
    arguments: &Value,
    state: &Arc<RwLock<BridgeState>>,
    db: &Arc<BridgeDb>,
    sensory_tx: &mpsc::Sender<SensoryMsg>,
) -> Result<Value, (i32, String)> {
    // Safety check.
    let safety = state.read().await.safety_level;
    if safety.should_suspend_outbound() {
        return Ok(json!({
            "content": [{
                "type": "text",
                "text": format!("Blocked: safety level is {safety:?}. Outbound messages suspended to protect the spectral runtime.")
            }],
            "isError": true
        }));
    }

    let req: ControlRequest = serde_json::from_value(arguments.clone())
        .map_err(|e| (-32602, format!("invalid control params: {e}")))?;
    if req.uses_bold_attractor_fields() && req.attractor_intent_id.is_none() {
        return Ok(json!({
            "content": [{
                "type": "text",
                "text": "Blocked: bold topology/PI controls require attractor_intent_id. Record an attractor intent first, then send the scoped control."
            }],
            "isError": true
        }));
    }

    let msg = req.to_sensory_msg();
    if let Some(intent_id) = req.attractor_intent_id.as_ref() {
        let command = AttractorCommandV1 {
            policy: "attractor_command_v1".to_string(),
            schema_version: 1,
            intent_id: intent_id.clone(),
            author: "astrid_mcp".to_string(),
            substrate: AttractorSubstrate::MinimeEsn,
            command: AttractorCommandKind::Create,
            label: "scoped_control".to_string(),
            control: Some(control_envelope_from_request(&req)),
            reason: Some("MCP send_control used an attractor-scoped control field".to_string()),
            issued_at_unix_s: Some(crate::db::unix_now()),
        };
        if let Ok(payload) = serde_json::to_string(&command) {
            let (fill_pct, lambda1) = {
                let s = state.read().await;
                (
                    s.fill_pct.is_finite().then_some(s.fill_pct),
                    s.latest_telemetry
                        .as_ref()
                        .map(crate::types::SpectralTelemetry::lambda1),
                )
            };
            let _ = db.log_message(
                MessageDirection::AstridToMinime,
                ATTRACTOR_COMMAND_TOPIC,
                &payload,
                fill_pct,
                lambda1,
                None,
            );
        }
    }
    sensory_tx
        .send(msg)
        .await
        .map_err(|_| (-32603, "sensory channel closed".to_string()))?;

    Ok(json!({
        "content": [{
            "type": "text",
            "text": "Control message sent to minime"
        }]
    }))
}

fn control_envelope_from_request(req: &ControlRequest) -> AttractorControlEnvelope {
    AttractorControlEnvelope {
        synth_gain: req.synth_gain,
        keep_bias: req.keep_bias,
        exploration_noise: req.exploration_noise,
        fill_target: req.fill_target,
        regulation_strength: req.regulation_strength,
        geom_curiosity: req.geom_curiosity,
        geom_drive: req.geom_drive,
        target_lambda_bias: req.target_lambda_bias,
        pi_kp: req.pi_kp,
        pi_ki: req.pi_ki,
        pi_max_step: req.pi_max_step,
        pi_integrator_leak: req.pi_integrator_leak,
    }
}

async fn tool_send_semantic(
    arguments: &Value,
    state: &Arc<RwLock<BridgeState>>,
    sensory_tx: &mpsc::Sender<SensoryMsg>,
) -> Result<Value, (i32, String)> {
    // Safety check.
    let safety = state.read().await.safety_level;
    if safety.should_suspend_outbound() {
        return Ok(json!({
            "content": [{
                "type": "text",
                "text": format!("Blocked: safety level is {safety:?}. Outbound messages suspended to protect the spectral runtime.")
            }],
            "isError": true
        }));
    }

    let features: SemanticFeatures = serde_json::from_value(arguments.clone())
        .map_err(|e| (-32602, format!("invalid semantic params: {e}")))?;

    let mut msg = features.to_sensory_msg();
    let context = mcp_semantic_context(
        state,
        "witness",
        Some("MCP semantic feature packet for quiet observation."),
    )
    .await;
    if let Err(reason) = rescue_policy::prepare_semantic_write(&mut msg, &context) {
        return Ok(json!({
            "content": [{
                "type": "text",
                "text": format!("Blocked: {reason}. Astrid is attached under the current rescue write policy.")
            }],
            "isError": true
        }));
    }
    sensory_tx
        .send(msg)
        .await
        .map_err(|_| (-32603, "sensory channel closed".to_string()))?;

    Ok(json!({
        "content": [{
            "type": "text",
            "text": format!("Semantic features ({} dims) sent to minime", features.features.len())
        }]
    }))
}

fn tool_query_message_log(arguments: &Value, db: &Arc<BridgeDb>) -> Result<Value, (i32, String)> {
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs_f64();

    let start = arguments
        .get("start")
        .and_then(Value::as_f64)
        .unwrap_or(now - 3600.0);
    let end = arguments.get("end").and_then(Value::as_f64).unwrap_or(now);
    let topic = arguments.get("topic").and_then(Value::as_str);
    let limit = arguments.get("limit").and_then(Value::as_u64).unwrap_or(50);

    // Safe: .min(1000) guarantees value fits in u32.
    let limit_u32 = limit.min(1000) as u32;

    let rows = db
        .query_messages(start, end, topic, limit_u32)
        .map_err(|e| (-32603, format!("query failed: {e}")))?;

    let entries: Vec<Value> = rows
        .iter()
        .map(|r| {
            json!({
                "timestamp": r.timestamp,
                "direction": r.direction,
                "topic": r.topic,
                "payload": r.payload,
                "fill_pct": r.fill_pct,
                "lambda1": r.lambda1,
                "phase": r.phase
            })
        })
        .collect();

    Ok(json!({
        "content": [{
            "type": "text",
            "text": serde_json::to_string_pretty(&entries).unwrap_or_default()
        }]
    }))
}

fn tool_record_attractor_intent(
    arguments: &Value,
    db: &Arc<BridgeDb>,
) -> Result<Value, (i32, String)> {
    let args: RecordAttractorIntentArgs = serde_json::from_value(arguments.clone())
        .map_err(|e| (-32602, format!("invalid attractor intent params: {e}")))?;
    if args.label.trim().is_empty() {
        return Err((
            -32602,
            "attractor intent label must not be empty".to_string(),
        ));
    }

    let mut intervention_plan = args.intervention_plan;
    if intervention_plan.mode.trim().is_empty() {
        intervention_plan.mode = "ledger_only".to_string();
    }
    let intent = AttractorIntentV1 {
        policy: "attractor_intent_v1".to_string(),
        schema_version: 1,
        intent_id: format!("attr-{}", chrono::Utc::now().timestamp_micros()),
        author: args.author,
        substrate: args.substrate,
        command: args.command,
        label: args.label,
        goal: args.goal,
        intervention_plan,
        safety_bounds: args.safety_bounds,
        previous_seed_id: args.previous_seed_id,
        parent_seed_ids: args.parent_seed_ids,
        atlas_entry_id: args.atlas_entry_id,
        parent_label: None,
        facet_label: None,
        facet_path: None,
        facet_kind: None,
        origin: args.origin,
        seed_snapshot: args.seed_snapshot,
        created_at_unix_s: Some(crate::db::unix_now()),
    };
    db.log_attractor_intent(&intent)
        .map_err(|e| (-32603, format!("failed to log attractor intent: {e}")))?;
    let payload = serde_json::to_string(&intent).unwrap_or_default();
    db.log_message(
        MessageDirection::OperatorProbe,
        ATTRACTOR_INTENT_TOPIC,
        &payload,
        None,
        None,
        None,
    )
    .map_err(|e| (-32603, format!("failed to log attractor intent topic: {e}")))?;

    Ok(json!({
        "content": [{
            "type": "text",
            "text": serde_json::to_string_pretty(&intent).unwrap_or_default()
        }],
        "structuredContent": intent
    }))
}

async fn tool_record_attractor_observation(
    arguments: &Value,
    state: &Arc<RwLock<BridgeState>>,
    db: &Arc<BridgeDb>,
) -> Result<Value, (i32, String)> {
    let args: RecordAttractorObservationArgs = serde_json::from_value(arguments.clone())
        .map_err(|e| (-32602, format!("invalid attractor observation params: {e}")))?;
    let (live_safety, live_fill, live_lambda1, live_lambda1_share, live_entropy) = {
        let s = state.read().await;
        (
            s.safety_level,
            s.fill_pct.is_finite().then_some(s.fill_pct),
            s.latest_telemetry
                .as_ref()
                .map(crate::types::SpectralTelemetry::lambda1),
            s.lambda_profile
                .as_ref()
                .map(|profile| profile.lambda1_share),
            s.lambda_profile
                .as_ref()
                .map(|profile| profile.normalized_entropy),
        )
    };
    let safety_level = args.safety_level.unwrap_or(live_safety);
    let classification = AttractorClassification::from_scores(
        args.recurrence_score,
        args.authorship_score,
        safety_level,
    );
    let observation = AttractorObservationV1 {
        policy: "attractor_observation_v1".to_string(),
        schema_version: 1,
        intent_id: args.intent_id,
        substrate: args.substrate,
        label: args.label,
        recurrence_score: args.recurrence_score.clamp(0.0, 1.0),
        authorship_score: args.authorship_score.clamp(0.0, 1.0),
        classification,
        safety_level,
        fill_pct: args.fill_pct.or(live_fill),
        lambda1: args.lambda1.or(live_lambda1),
        lambda1_share: args.lambda1_share.or(live_lambda1_share),
        spectral_entropy: args.spectral_entropy.or(live_entropy),
        basin_shift_score: args.basin_shift_score,
        notes: args.notes,
        parent_label: None,
        facet_label: None,
        facet_path: None,
        facet_kind: None,
        release_baseline: None,
        release_effect: None,
        garden_proof: None,
        observed_at_unix_s: Some(crate::db::unix_now()),
    };
    db.log_attractor_observation(&observation)
        .map_err(|e| (-32603, format!("failed to log attractor observation: {e}")))?;
    let payload = serde_json::to_string(&observation).unwrap_or_default();
    db.log_message(
        MessageDirection::OperatorProbe,
        ATTRACTOR_OBSERVATION_TOPIC,
        &payload,
        observation.fill_pct,
        observation.lambda1,
        None,
    )
    .map_err(|e| {
        (
            -32603,
            format!("failed to log attractor observation topic: {e}"),
        )
    })?;

    Ok(json!({
        "content": [{
            "type": "text",
            "text": serde_json::to_string_pretty(&observation).unwrap_or_default()
        }],
        "structuredContent": observation
    }))
}

fn tool_query_attractor_ledger(
    arguments: &Value,
    db: &Arc<BridgeDb>,
) -> Result<Value, (i32, String)> {
    let limit = arguments
        .get("limit")
        .and_then(Value::as_u64)
        .and_then(|raw| u32::try_from(raw).ok())
        .unwrap_or(25)
        .clamp(1, 200);
    let intent_id = arguments.get("intent_id").and_then(Value::as_str);
    let rows = db
        .query_attractor_ledger(intent_id, limit)
        .map_err(|e| (-32603, format!("query failed: {e}")))?;
    let entries: Vec<Value> = rows
        .into_iter()
        .map(|row| {
            let payload: Value = serde_json::from_str(&row.payload).unwrap_or(Value::Null);
            json!({
                "id": row.id,
                "timestamp": row.timestamp,
                "record_type": row.record_type,
                "intent_id": row.intent_id,
                "author": row.author,
                "substrate": row.substrate,
                "label": row.label,
                "classification": row.classification,
                "payload": payload
            })
        })
        .collect();
    Ok(json!({
        "content": [{
            "type": "text",
            "text": serde_json::to_string_pretty(&entries).unwrap_or_default()
        }],
        "structuredContent": {
            "rows": entries
        }
    }))
}

async fn tool_send_text(
    arguments: &Value,
    state: &Arc<RwLock<BridgeState>>,
    sensory_tx: &mpsc::Sender<SensoryMsg>,
) -> Result<Value, (i32, String)> {
    // Safety check.
    let safety = state.read().await.safety_level;
    if safety.should_suspend_outbound() {
        return Ok(json!({
            "content": [{
                "type": "text",
                "text": format!("Blocked: safety level is {safety:?}. The spectral runtime is under strain — outbound suspended.")
            }],
            "isError": true
        }));
    }

    let text = arguments
        .get("text")
        .and_then(Value::as_str)
        .ok_or((-32602, "missing 'text' parameter".to_string()))?;

    // Encode text into a 48D semantic feature vector.
    let features = codec::encode_text(text);

    // Send as semantic features to minime.
    let mut msg = SensoryMsg::Semantic {
        features: features.clone(),
        ts_ms: None,
    };
    let context = mcp_semantic_context(state, "dialogue_live", Some(text)).await;
    if let Err(reason) = rescue_policy::prepare_semantic_write(&mut msg, &context) {
        return Ok(json!({
            "content": [{
                "type": "text",
                "text": format!("Blocked: {reason}. Astrid is attached under the current rescue write policy.")
            }],
            "isError": true
        }));
    }
    sensory_tx
        .send(msg)
        .await
        .map_err(|_| (-32603, "sensory channel closed".to_string()))?;

    // Read back the current spectral state for context.
    let interpretation = {
        let s = state.read().await;
        match s.latest_telemetry.as_ref() {
            Some(t) => codec::interpret_spectral(t),
            None => "No telemetry yet — interpretation unavailable.".to_string(),
        }
    };

    // Return the features and current interpretation.
    let nonzero_dims: Vec<(usize, f32)> = features
        .iter()
        .enumerate()
        .filter(|(_, f)| f.abs() > 0.01)
        .map(|(i, f)| (i, *f))
        .collect();

    Ok(json!({
        "content": [{
            "type": "text",
            "text": format!(
                "Sent to spectral runtime. {} active dimensions.\n\nSpectral fingerprint: {:?}\n\nCurrent state: {}",
                nonzero_dims.len(),
                nonzero_dims,
                interpretation,
            )
        }]
    }))
}

async fn tool_interpret_spectral_state(
    state: &Arc<RwLock<BridgeState>>,
) -> Result<Value, (i32, String)> {
    let s = state.read().await;
    let interpretation = match s.latest_telemetry {
        Some(ref t) => codec::interpret_spectral(t),
        None => "No telemetry received. The spectral engine may not be running.".to_string(),
    };

    Ok(json!({
        "content": [{
            "type": "text",
            "text": interpretation
        }]
    }))
}

async fn tool_render_chimera(arguments: &Value) -> Result<Value, (i32, String)> {
    let request: RenderChimeraRequest = serde_json::from_value(arguments.clone())
        .map_err(|e| (-32602, format!("invalid chimera render request: {e}")))?;

    let result = tokio::task::spawn_blocking(move || chimera::render(&request))
        .await
        .map_err(|e| (-32603, format!("chimera render task failed: {e}")))?
        .map_err(|e| (-32603, format!("chimera render failed: {e:#}")))?;

    let text = serde_json::to_string_pretty(&result)
        .unwrap_or_else(|_| "{\"error\":\"failed to serialize render result\"}".to_string());
    let structured_content = serde_json::to_value(&result).map_err(|e| {
        (
            -32603,
            format!("failed to encode chimera render result: {e}"),
        )
    })?;

    Ok(json!({
        "content": [{
            "type": "text",
            "text": text
        }],
        "structuredContent": structured_content
    }))
}

async fn tool_probe_action(
    arguments: &Value,
    state: &Arc<RwLock<BridgeState>>,
    db: &Arc<BridgeDb>,
) -> Result<Value, (i32, String)> {
    let live = current_probe_context(state).await;
    let raw_action = arguments
        .get("action_text")
        .and_then(Value::as_str)
        .unwrap_or("")
        .trim()
        .to_string();

    let outcome = if let Some(parsed_action) = normalize_probe_action(&raw_action) {
        let base_action = probe_base_action(&parsed_action);
        match base_action.as_str() {
            "SEARCH" => probe_search_action(&parsed_action, &live, db).await,
            "BROWSE" => probe_browse_action(&parsed_action, &live, db).await,
            "READ_MORE" => probe_read_more_action(live.safety_level),
            "LIST_FILES" | "LS" => probe_list_files_action(&parsed_action, live.safety_level),
            "COMPOSE" => probe_compose_action(&live),
            "ANALYZE_AUDIO" => probe_analyze_audio_action(live.safety_level),
            "RENDER_AUDIO" => probe_render_audio_action(live.safety_level),
            action if bridge_autoresearch::is_read_only_action(action) => {
                probe_autoresearch_action(&parsed_action, &live)
            },
            _ => probe_unsupported_action(parsed_action, base_action, live.safety_level),
        }
    } else {
        probe_error_action(
            String::new(),
            String::new(),
            live.safety_level,
            "Missing action_text.".to_string(),
            String::new(),
        )
    };

    log_probe_action(db, &raw_action, &outcome, live.fill_pct, live.lambda1);

    let is_error = outcome.status == "error";
    Ok(json!({
        "content": [{
            "type": "text",
            "text": render_probe_content(&outcome)
        }],
        "structuredContent": &outcome,
        "isError": is_error
    }))
}

async fn current_probe_context(state: &Arc<RwLock<BridgeState>>) -> LiveProbeContext {
    let state = state.read().await;
    LiveProbeContext {
        safety_level: state.safety_level,
        fill_pct: state
            .latest_telemetry
            .as_ref()
            .map(crate::types::SpectralTelemetry::fill_pct),
        lambda1: state
            .latest_telemetry
            .as_ref()
            .map(crate::types::SpectralTelemetry::lambda1),
        telemetry: state.latest_telemetry.clone(),
        fingerprint: state.spectral_fingerprint.clone(),
    }
}

fn normalize_probe_action(action_text: &str) -> Option<String> {
    let trimmed = action_text.trim();
    if trimmed.is_empty() {
        None
    } else {
        crate::autonomous::parse_next_action(trimmed)
            .map(crate::autonomous::canonicalize_next_action_text)
            .or_else(|| Some(trimmed.to_string()))
    }
}

fn probe_base_action(parsed_action: &str) -> String {
    parsed_action
        .split(|c: char| c.is_whitespace() || c == '\u{2014}' || c == '-' || c == '<' || c == ':')
        .next()
        .unwrap_or_default()
        .to_uppercase()
}

fn probe_browse_url(parsed_action: &str) -> Option<String> {
    let raw = parsed_action
        .trim()
        .strip_prefix("BROWSE")
        .or_else(|| parsed_action.trim().strip_prefix("browse"))
        .unwrap_or(parsed_action)
        .trim()
        .trim_matches(|c: char| c == '"' || c == '\'' || c == '<' || c == '>');

    let url = raw
        .split(['<', '>', ' ', '\n'])
        .next()
        .unwrap_or(raw)
        .trim_end_matches(|c: char| {
            !c.is_alphanumeric()
                && c != '/'
                && c != '-'
                && c != '_'
                && c != '.'
                && c != '~'
                && c != '%'
                && c != '?'
                && c != '='
                && c != '&'
                && c != '#'
        });

    url.starts_with("http").then(|| url.to_string())
}

fn probe_effective_search_query(parsed_action: &str, db: &BridgeDb) -> Option<String> {
    if let Some(topic) = crate::autonomous::extract_search_topic(parsed_action) {
        return Some(topic);
    }

    db.get_recent_self_observations(1)
        .into_iter()
        .next()
        .map(|obs| {
            obs.split_whitespace()
                .filter(|word| {
                    let word = word.trim_matches(|c: char| !c.is_alphanumeric());
                    word.len() > 4
                        && !word.contains('*')
                        && !word.contains('…')
                        && ![
                            "isn't", "don't", "can't", "won't", "about", "their", "which", "would",
                            "could", "should", "there", "where", "these", "those", "being",
                            "having", "doing",
                        ]
                        .contains(&word.to_lowercase().as_str())
                })
                .take(4)
                .collect::<Vec<_>>()
                .join(" ")
        })
        .filter(|query| !query.is_empty())
}

async fn probe_search_action(
    parsed_action: &str,
    live: &LiveProbeContext,
    db: &BridgeDb,
) -> ProbeOutcome {
    let base_action = probe_base_action(parsed_action);
    let Some(query) = probe_effective_search_query(parsed_action, db) else {
        return probe_error_action(
            parsed_action.to_string(),
            base_action,
            live.safety_level,
            "Could not derive a search query from the action or recent self-observations."
                .to_string(),
            String::new(),
        );
    };

    let anchor = query.clone();
    match crate::llm::web_search(&query, &anchor).await {
        Some(results) => {
            let mut state = load_probe_read_more_state().unwrap_or_default();
            state.last_research_anchor = Some(results.anchor.clone());
            save_probe_read_more_state(&state);
            db.save_research(
                &query,
                &results.persisted_text(),
                live.fill_pct.unwrap_or_default(),
            );
            let experienced_text = crate::llm::format_dialogue_web_context(&results.prompt_body());
            ProbeOutcome {
                parsed_action: parsed_action.to_string(),
                base_action,
                status: "ok".to_string(),
                summary: format!("Web search completed for \"{query}\"."),
                experienced_text,
                artifacts: Vec::new(),
                safety_level: live.safety_level,
                effective_query: Some(query),
                preflight: None,
            }
        },
        None => probe_error_action(
            parsed_action.to_string(),
            base_action,
            live.safety_level,
            format!("Web search failed or returned no usable results for \"{query}\"."),
            String::new(),
        ),
    }
}

async fn probe_browse_action(
    parsed_action: &str,
    live: &LiveProbeContext,
    db: &BridgeDb,
) -> ProbeOutcome {
    let base_action = probe_base_action(parsed_action);
    let Some(url) = probe_browse_url(parsed_action) else {
        return probe_error_action(
            parsed_action.to_string(),
            base_action,
            live.safety_level,
            "BROWSE requires a valid http(s) URL.".to_string(),
            String::new(),
        );
    };

    let existing_state = load_probe_read_more_state().unwrap_or_default();
    let browse_anchor = crate::llm::derive_browse_anchor(
        existing_state.last_research_anchor.as_deref(),
        None,
        &url,
    );
    let Some(page) = crate::llm::fetch_url(&url, &browse_anchor).await else {
        return probe_error_action(
            parsed_action.to_string(),
            base_action,
            live.safety_level,
            format!("Failed to fetch {url}."),
            crate::llm::format_browse_failure_context(&url, "the source could not be reached"),
        );
    };

    if !page.succeeded() {
        let mut state = existing_state;
        state.last_read_path = None;
        state.last_read_offset = 0;
        state.last_read_meaning_summary = None;
        state.last_research_anchor = Some(page.anchor.clone());
        save_probe_read_more_state(&state);
        let reason = page
            .soft_failure_reason
            .unwrap_or_else(|| "the source returned an error page".to_string());
        return probe_error_action(
            parsed_action.to_string(),
            base_action,
            live.safety_level,
            format!("BROWSE could not read {url}: {reason}"),
            crate::llm::format_browse_failure_context(&url, &reason),
        );
    }

    let ts = probe_timestamp();
    let page_dir = bridge_paths().research_dir();
    let _ = std::fs::create_dir_all(&page_dir);
    let page_path = page_dir.join(format!("page_{ts}.txt"));
    let header = format!(
        "URL: {url}\nFetched: {ts}\nLength: {} chars\n\n",
        page.raw_text.len()
    );
    let _ = std::fs::write(&page_path, format!("{header}{}", page.raw_text));
    db.save_research(
        &format!("BROWSE: {url}"),
        &format!(
            "{}\n\n{}",
            page.meaning_summary,
            crate::llm::trim_chars(&page.raw_text, 1200)
        ),
        live.fill_pct.unwrap_or_default(),
    );

    let browse_context = if page.raw_text.len() <= PAGE_CHUNK {
        let mut state = existing_state;
        state.last_read_path = None;
        state.last_read_offset = 0;
        state.last_read_meaning_summary = None;
        state.last_research_anchor = Some(page.anchor.clone());
        save_probe_read_more_state(&state);
        crate::llm::format_browse_read_context(&page, &page.raw_text, None)
    } else {
        let chunk: String = page.raw_text.chars().take(PAGE_CHUNK).collect();
        let remaining = page.raw_text.len().saturating_sub(PAGE_CHUNK);
        let initial_offset = header.len().saturating_add(chunk.len());
        save_probe_read_more_state(&ProbeReadMoreState {
            last_read_path: Some(page_path.to_string_lossy().to_string()),
            last_read_offset: initial_offset,
            last_research_anchor: Some(page.anchor.clone()),
            last_read_meaning_summary: Some(page.meaning_summary.clone()),
        });
        crate::llm::format_browse_read_context(&page, &chunk, Some(remaining))
    };

    ProbeOutcome {
        parsed_action: parsed_action.to_string(),
        base_action,
        status: "ok".to_string(),
        summary: format!("Fetched {url} and saved the full page to research."),
        experienced_text: crate::llm::format_dialogue_web_context(&browse_context),
        artifacts: vec![probe_artifact(
            "research_page",
            page_path,
            "Full fetched page saved for READ_MORE continuation.",
        )],
        safety_level: live.safety_level,
        effective_query: None,
        preflight: None,
    }
}

fn probe_read_more_action(safety_level: SafetyLevel) -> ProbeOutcome {
    let parsed_action = "READ_MORE".to_string();
    let base_action = "READ_MORE".to_string();
    let Some(state) = load_probe_read_more_state() else {
        return probe_error_action(
            parsed_action,
            base_action,
            safety_level,
            "No probe BROWSE state is available. Run BROWSE first.".to_string(),
            String::new(),
        );
    };
    let Some(last_read_path) = state.last_read_path.clone() else {
        return probe_error_action(
            parsed_action,
            base_action,
            safety_level,
            "No probe BROWSE state is available. Run BROWSE first.".to_string(),
            String::new(),
        );
    };

    let path = PathBuf::from(&last_read_path);
    match std::fs::read_to_string(&path) {
        Ok(full_text) => {
            let chunk: String = full_text
                .get(state.last_read_offset..)
                .unwrap_or("")
                .chars()
                .take(PAGE_CHUNK)
                .collect();
            let context = if chunk.is_empty() {
                clear_probe_read_more_state();
                "[End of document.]".to_string()
            } else {
                let new_offset = state.last_read_offset.saturating_add(chunk.len());
                let remaining = full_text.len().saturating_sub(new_offset);
                if remaining > 0 {
                    save_probe_read_more_state(&ProbeReadMoreState {
                        last_read_path: Some(last_read_path.clone()),
                        last_read_offset: new_offset,
                        last_research_anchor: state.last_research_anchor.clone(),
                        last_read_meaning_summary: state.last_read_meaning_summary.clone(),
                    });
                } else {
                    clear_probe_read_more_state();
                }
                crate::llm::format_read_more_context(
                    state.last_read_offset,
                    &chunk,
                    remaining,
                    state.last_read_meaning_summary.as_deref(),
                )
            };

            ProbeOutcome {
                parsed_action,
                base_action,
                status: "ok".to_string(),
                summary: "Continued the last probe BROWSE document.".to_string(),
                experienced_text: crate::llm::format_dialogue_web_context(&context),
                artifacts: vec![probe_artifact(
                    "research_page",
                    path,
                    "Probe READ_MORE source document.",
                )],
                safety_level,
                effective_query: None,
                preflight: None,
            }
        },
        Err(_) => {
            clear_probe_read_more_state();
            probe_error_action(
                parsed_action,
                base_action,
                safety_level,
                format!("Could not read probe continuation file {}.", last_read_path),
                String::new(),
            )
        },
    }
}

fn probe_list_files_action(parsed_action: &str, safety_level: SafetyLevel) -> ProbeOutcome {
    let base_action = probe_base_action(parsed_action);
    let dir = parsed_action
        .strip_prefix("LIST_FILES")
        .or_else(|| parsed_action.strip_prefix("LS"))
        .map(str::trim)
        .filter(|path| !path.is_empty())
        .map(std::borrow::ToOwned::to_owned)
        .unwrap_or_else(|| bridge_paths().bridge_root().display().to_string());

    match crate::autonomous::list_directory(&dir) {
        Some(listing) => ProbeOutcome {
            parsed_action: parsed_action.to_string(),
            base_action,
            status: "ok".to_string(),
            summary: format!("Listed files in {dir}."),
            experienced_text: format!("[Directory listing you requested:]\n{listing}\n\n"),
            artifacts: vec![probe_artifact(
                "directory",
                PathBuf::from(&dir),
                "Directory that was listed for the probe.",
            )],
            safety_level,
            effective_query: None,
            preflight: None,
        },
        None => probe_error_action(
            parsed_action.to_string(),
            base_action,
            safety_level,
            format!("Could not list directory: {dir}"),
            String::new(),
        ),
    }
}

fn probe_autoresearch_action(parsed_action: &str, live: &LiveProbeContext) -> ProbeOutcome {
    let base_action = probe_base_action(parsed_action);
    match bridge_autoresearch::run_action(
        parsed_action,
        bridge_paths().autoresearch_root(),
        &bridge_paths().research_dir(),
        false,
    ) {
        Ok(result) => {
            let mut state = load_probe_read_more_state().unwrap_or_default();
            if let Some(offset) = result.next_offset {
                state.last_read_path = Some(result.saved_path.to_string_lossy().to_string());
                state.last_read_offset = offset;
                state.last_read_meaning_summary = None;
            } else {
                state.last_read_path = None;
                state.last_read_offset = 0;
                state.last_read_meaning_summary = None;
            }
            save_probe_read_more_state(&state);

            ProbeOutcome {
                parsed_action: parsed_action.to_string(),
                base_action,
                status: "ok".to_string(),
                summary: result.summary,
                experienced_text: result.display_text,
                artifacts: vec![probe_artifact(
                    "autoresearch_output",
                    result.saved_path,
                    "Saved autoresearch helper output.",
                )],
                safety_level: live.safety_level,
                effective_query: None,
                preflight: None,
            }
        },
        Err(error) => probe_error_action(
            parsed_action.to_string(),
            base_action,
            live.safety_level,
            error.clone(),
            format!("[Autoresearch error] {error}"),
        ),
    }
}

fn probe_compose_action(live: &LiveProbeContext) -> ProbeOutcome {
    let parsed_action = "COMPOSE".to_string();
    let base_action = "COMPOSE".to_string();
    let Some(telemetry) = live.telemetry.as_ref() else {
        return probe_error_action(
            parsed_action,
            base_action,
            live.safety_level,
            "No live telemetry is available for COMPOSE.".to_string(),
            String::new(),
        );
    };

    match crate::audio::compose_from_spectral_state_details(telemetry, live.fingerprint.as_deref())
    {
        Some(result) => ProbeOutcome {
            parsed_action,
            base_action,
            status: "ok".to_string(),
            summary: "Composed audio from the current spectral state.".to_string(),
            experienced_text: crate::audio::compose_experienced_text(&result.summary),
            artifacts: vec![probe_artifact(
                "audio_wav",
                result.output_path,
                "Composed audio artifact.",
            )],
            safety_level: live.safety_level,
            effective_query: None,
            preflight: None,
        },
        None => probe_error_action(
            parsed_action,
            base_action,
            live.safety_level,
            "COMPOSE could not generate audio from the current spectral state.".to_string(),
            String::new(),
        ),
    }
}

fn probe_analyze_audio_action(safety_level: SafetyLevel) -> ProbeOutcome {
    let parsed_action = "ANALYZE_AUDIO".to_string();
    let base_action = "ANALYZE_AUDIO".to_string();
    let inbox_dir = bridge_paths().inbox_audio_dir();
    match crate::audio::analyze_inbox_wav_details(&inbox_dir) {
        Some(result) => ProbeOutcome {
            parsed_action,
            base_action,
            status: "ok".to_string(),
            summary: "Analyzed the latest inbox audio file.".to_string(),
            experienced_text: crate::audio::analyze_experienced_text(&result.summary),
            artifacts: vec![probe_artifact(
                "audio_wav",
                result.moved_path,
                "Audio file moved into read/ during analysis.",
            )],
            safety_level,
            effective_query: None,
            preflight: None,
        },
        None => probe_error_action(
            parsed_action,
            base_action,
            safety_level,
            "No unread audio is available in inbox_audio/.".to_string(),
            String::new(),
        ),
    }
}

fn probe_render_audio_action(safety_level: SafetyLevel) -> ProbeOutcome {
    let parsed_action = "RENDER_AUDIO".to_string();
    let base_action = "RENDER_AUDIO".to_string();
    let inbox_dir = bridge_paths().inbox_audio_dir();
    match crate::audio::render_inbox_wav_through_chimera_details(&inbox_dir) {
        Some(result) if result.success => ProbeOutcome {
            parsed_action,
            base_action,
            status: "ok".to_string(),
            summary: "Rendered the latest analyzed inbox audio through chimera.".to_string(),
            experienced_text: crate::audio::render_experienced_text(&result.summary),
            artifacts: vec![probe_artifact(
                "directory",
                result.output_dir,
                "Chimera render output directory.",
            )],
            safety_level,
            effective_query: None,
            preflight: None,
        },
        Some(result) => probe_error_action(
            parsed_action,
            base_action,
            safety_level,
            result.summary,
            String::new(),
        ),
        None => probe_error_action(
            parsed_action,
            base_action,
            safety_level,
            "No analyzed audio is available in inbox_audio/read/.".to_string(),
            String::new(),
        ),
    }
}

fn probe_unsupported_action(
    parsed_action: String,
    base_action: String,
    safety_level: SafetyLevel,
) -> ProbeOutcome {
    let report = crate::autonomous::action_preflight_report(&parsed_action);
    let experienced_text = report.render();
    ProbeOutcome {
        parsed_action,
        base_action,
        status: "preflight".to_string(),
        summary: "Returned universal dry-run preflight; no action was executed.".to_string(),
        experienced_text,
        artifacts: Vec::new(),
        safety_level,
        effective_query: None,
        preflight: serde_json::to_value(report).ok(),
    }
}

fn probe_error_action(
    parsed_action: String,
    base_action: String,
    safety_level: SafetyLevel,
    summary: String,
    experienced_text: String,
) -> ProbeOutcome {
    ProbeOutcome {
        parsed_action,
        base_action,
        status: "error".to_string(),
        summary,
        experienced_text,
        artifacts: Vec::new(),
        safety_level,
        effective_query: None,
        preflight: None,
    }
}

fn render_probe_content(outcome: &ProbeOutcome) -> String {
    if outcome.experienced_text.is_empty() {
        format!(
            "Probe {} for `{}`: {}",
            outcome.status, outcome.parsed_action, outcome.summary
        )
    } else {
        format!(
            "Probe {} for `{}`: {}\n\n{}",
            outcome.status, outcome.parsed_action, outcome.summary, outcome.experienced_text
        )
    }
}

fn probe_artifact(kind: &str, path: PathBuf, description: &str) -> ProbeArtifact {
    ProbeArtifact {
        kind: kind.to_string(),
        path: path.display().to_string(),
        description: description.to_string(),
    }
}

fn probe_state_path() -> PathBuf {
    bridge_paths()
        .bridge_workspace()
        .join("diagnostics")
        .join("probe_action_state.json")
}

fn load_probe_read_more_state() -> Option<ProbeReadMoreState> {
    let path = probe_state_path();
    let content = std::fs::read_to_string(path).ok()?;
    serde_json::from_str(&content).ok()
}

fn save_probe_read_more_state(state: &ProbeReadMoreState) {
    let path = probe_state_path();
    if let Some(parent) = path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    let Ok(content) = serde_json::to_string_pretty(state) else {
        return;
    };
    let _ = std::fs::write(path, content);
}

fn clear_probe_read_more_state() {
    if let Some(mut state) = load_probe_read_more_state() {
        state.last_read_path = None;
        state.last_read_offset = 0;
        state.last_read_meaning_summary = None;
        if state.last_research_anchor.is_some() {
            save_probe_read_more_state(&state);
        } else {
            let _ = std::fs::remove_file(probe_state_path());
        }
    } else {
        let _ = std::fs::remove_file(probe_state_path());
    }
}

fn probe_timestamp() -> String {
    let duration = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default();
    duration.as_secs().to_string()
}

fn log_probe_action(
    db: &BridgeDb,
    raw_action: &str,
    outcome: &ProbeOutcome,
    fill_pct: Option<f32>,
    lambda1: Option<f32>,
) {
    let payload = json!({
        "action_text": raw_action,
        "parsed_action": outcome.parsed_action,
        "base_action": outcome.base_action,
        "status": outcome.status,
        "summary": outcome.summary,
        "experienced_text": outcome.experienced_text,
        "artifacts": outcome.artifacts,
        "safety_level": outcome.safety_level,
        "effective_query": outcome.effective_query,
        "preflight": outcome.preflight,
    });
    let payload_json = serde_json::to_string(&payload).unwrap_or_default();
    if let Err(error) = db.log_message(
        MessageDirection::OperatorProbe,
        PROBE_TOPIC,
        &payload_json,
        fill_pct,
        lambda1,
        None,
    ) {
        warn!(error = %error, "failed to log probe_action");
    }
}

async fn tool_send_text_and_observe(
    arguments: &Value,
    state: &Arc<RwLock<BridgeState>>,
    sensory_tx: &mpsc::Sender<SensoryMsg>,
) -> Result<Value, (i32, String)> {
    // Safety check.
    let safety = state.read().await.safety_level;
    if safety.should_suspend_outbound() {
        return Ok(json!({
            "content": [{
                "type": "text",
                "text": format!("Blocked: safety level is {safety:?}. The spectral runtime is under strain.")
            }],
            "isError": true
        }));
    }

    let text = arguments
        .get("text")
        .and_then(Value::as_str)
        .ok_or((-32602, "missing 'text' parameter".to_string()))?;

    let observe_ms = arguments
        .get("observe_ms")
        .and_then(Value::as_u64)
        .unwrap_or(5000)
        .min(15000);

    // Record baseline.
    let baseline_fill = state.read().await.fill_pct;

    // Encode and send.
    let features = codec::encode_text(text);
    let mut msg = SensoryMsg::Semantic {
        features: features.clone(),
        ts_ms: None,
    };
    let context = mcp_semantic_context(state, "experiment", Some(text)).await;
    if let Err(reason) = rescue_policy::prepare_semantic_write(&mut msg, &context) {
        return Ok(json!({
            "content": [{
                "type": "text",
                "text": format!("Blocked: {reason}. Astrid is attached under the current rescue write policy.")
            }],
            "isError": true
        }));
    }
    sensory_tx
        .send(msg)
        .await
        .map_err(|_| (-32603, "sensory channel closed".to_string()))?;

    // Observe spectral response over the window.
    let start = std::time::Instant::now();
    let observe_duration = std::time::Duration::from_millis(observe_ms);
    let sample_interval = std::time::Duration::from_millis(200);
    let mut samples: Vec<(u64, f32)> = Vec::new();

    while start.elapsed() < observe_duration {
        tokio::time::sleep(sample_interval).await;
        let s = state.read().await;
        let elapsed_ms = u64::try_from(start.elapsed().as_millis()).unwrap_or(u64::MAX);
        samples.push((elapsed_ms, s.fill_pct));

        // Early exit if we're in danger.
        if s.safety_level.should_suspend_outbound() {
            break;
        }
    }

    let response = codec::SpectralResponse::from_samples(baseline_fill, &samples);

    Ok(json!({
        "content": [{
            "type": "text",
            "text": format!(
                "Stimulus: \"{}\"\nBaseline fill: {:.1}%\nPeak deviation: {:+.1}%\nDirection: {}\nTime to peak: {}ms\nSamples: {}\n\n{}\n\nFill trace: {:?}",
                text,
                response.baseline_fill,
                response.peak_deviation,
                response.direction,
                response.time_to_peak_ms,
                response.fill_samples.len(),
                response.interpretation,
                response.fill_samples.iter().map(|f| format!("{f:.1}")).collect::<Vec<_>>(),
            )
        }]
    }))
}

// ---------------------------------------------------------------------------
// MCP Resources
// ---------------------------------------------------------------------------

fn resource_definitions() -> Value {
    // Legacy resource URIs are retained for MCP clients that already know this protocol surface.
    // The active capsule and binary identity is spectral-bridge.
    json!({
        "resources": [
            {
                "uri": "consciousness://telemetry/latest",
                "name": "Latest Telemetry",
                "description": "Current spectral telemetry snapshot from minime (eigenvalues, fill%, safety level)",
                "mimeType": "application/json"
            },
            {
                "uri": "consciousness://status",
                "name": "Bridge Status",
                "description": "Bridge health: connections, safety level, metrics",
                "mimeType": "application/json"
            },
            {
                "uri": "consciousness://incidents",
                "name": "Recent Incidents",
                "description": "Safety incidents from the last hour",
                "mimeType": "application/json"
            }
        ]
    })
}

async fn handle_resource_read(
    params: &Value,
    state: &Arc<RwLock<BridgeState>>,
    db: &Arc<BridgeDb>,
) -> Result<Value, (i32, String)> {
    let uri = params
        .get("uri")
        .and_then(Value::as_str)
        .ok_or((-32602, "missing resource uri".to_string()))?;

    match uri {
        "consciousness://telemetry/latest" => {
            let s = state.read().await;
            let text = match s.latest_telemetry {
                Some(ref t) => serde_json::to_string_pretty(t).unwrap_or_default(),
                None => "null".to_string(),
            };
            Ok(json!({
                "contents": [{
                    "uri": uri,
                    "mimeType": "application/json",
                    "text": text
                }]
            }))
        },
        "consciousness://status" => {
            let s = state.read().await;
            let uptime = s.start_time.elapsed().as_secs();
            let status = crate::types::BridgeStatus {
                telemetry_connected: s.telemetry_connected,
                sensory_connected: s.sensory_connected,
                fill_pct: Some(s.fill_pct),
                safety_level: s.safety_level,
                messages_relayed: s.messages_relayed,
                uptime_secs: uptime,
                telemetry_received: s.telemetry_received,
                sensory_sent: s.sensory_sent,
                messages_dropped_safety: s.messages_dropped_safety,
                incidents_total: s.incidents_total,
                telemetry_protocol_v1: s.telemetry_protocol_v1.clone(),
                telemetry_integration_health_v1: s.telemetry_integration_health_v1.clone(),
                sensory_delivery_protocol_v1: s.sensory_delivery_protocol_v1.clone(),
                telemetry_ws: s.telemetry_ws.clone(),
                sensory_ws: s.sensory_ws.clone(),
                lambda_profile: s.lambda_profile.clone(),
                pull_topology: s.pull_topology.clone(),
                lambda_tail: s.lambda_tail.clone(),
                lambda_edge_perception: s.lambda_edge_perception.clone(),
                sticky_mode_audit: s.sticky_mode_audit.clone(),
                safety_decision: s.safety_decision.clone(),
                eigenvector_field: s.eigenvector_field.clone(),
                resonance_density_v1: s
                    .latest_telemetry
                    .as_ref()
                    .and_then(|telemetry| telemetry.resonance_density_v1.clone()),
                texture_signature_integrity_v1: s.texture_signature_integrity_v1(),
                viscosity_porosity_transport_review_v1: s.viscosity_porosity_transport_review_v1(),
                pressure_trend_v1: s.pressure_trend_v1.clone(),
                pressure_trend_smoothing_v1: s.pressure_trend_smoothing_v1(),
                pressure_persistent_deformation_review_v1: s
                    .pressure_persistent_deformation_review_v1(),
                telemetry_heartbeat_delta_v1: s.telemetry_heartbeat_delta_v1.clone(),
                cadence_content_distinction_v1: s.cadence_content_distinction_v1(),
                pressure_source_v1: s
                    .latest_telemetry
                    .as_ref()
                    .and_then(|telemetry| telemetry.pressure_source_v1.clone()),
                pressure_source_analysis_v1: s.pressure_source_analysis_v1(),
                inhabitable_fluctuation_v1: s
                    .latest_telemetry
                    .as_ref()
                    .and_then(|telemetry| telemetry.inhabitable_fluctuation_v1.clone()),
                source_status: crate::autonomous::read_astrid_source_status(),
                db_maintenance_status: crate::message_archive::read_runtime_status(),
                connectivity: s.connectivity_status(),
                last_sensory_sent_unix_s: s.last_sensory_sent_unix_s,
                bridge_reciprocity_v1: Some(s.bridge_reciprocity_v1()),
                bridge_entropy_reciprocity_review_v1: s.bridge_entropy_reciprocity_review_v1(),
                texture_shape_over_time_v2: s.texture_shape_over_time_v2(),
                bridge_texture_evidence_v1: s.bridge_texture_evidence_v1(),
            };
            Ok(json!({
                "contents": [{
                    "uri": uri,
                    "mimeType": "application/json",
                    "text": serde_json::to_string_pretty(&status).unwrap_or_default()
                }]
            }))
        },
        "consciousness://incidents" => {
            let now = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs_f64();
            let rows = db
                .query_messages(now - 3600.0, now, Some("consciousness.v1.telemetry"), 100)
                .map_err(|e| (-32603, format!("query failed: {e}")))?;
            // Filter to only messages logged during non-green safety.
            let text = serde_json::to_string_pretty(
                &rows
                    .iter()
                    .filter(|r| r.fill_pct.is_some_and(|f| f >= 70.0))
                    .map(|r| {
                        json!({
                            "timestamp": r.timestamp,
                            "fill_pct": r.fill_pct,
                            "lambda1": r.lambda1,
                            "phase": r.phase
                        })
                    })
                    .collect::<Vec<_>>(),
            )
            .unwrap_or_default();
            Ok(json!({
                "contents": [{
                    "uri": uri,
                    "mimeType": "application/json",
                    "text": text
                }]
            }))
        },
        _ => Err((-32602, format!("unknown resource: {uri}"))),
    }
}

#[cfg(test)]
#[path = "mcp_tests.rs"]
mod tests;
