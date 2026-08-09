use std::{
    fs,
    net::{IpAddr, SocketAddr},
    path::{Path, PathBuf},
};

use anyhow::{Context as _, Result, bail};
use clap::{Parser, ValueEnum};

#[derive(Debug, Clone, Copy, Default, Eq, PartialEq, ValueEnum)]
pub enum AutonomyPromptProfile {
    #[default]
    Detailed,
    Compact,
}

#[derive(Debug, Clone, Copy, Default, Eq, PartialEq, ValueEnum)]
pub enum AutonomyInitiativeProfile {
    #[default]
    Disabled,
    Private,
}

#[derive(Debug, Clone, Parser)]
#[command(name = "astrid-edge-runtime", version)]
#[allow(clippy::struct_excessive_bools)] // Direct CLI/environment policy switches.
pub struct Config {
    /// Stable immutable appliance identifier used in root-supervisor receipts.
    #[arg(
        long,
        env = "ASTRID_EDGE_APPLIANCE_ID",
        default_value = "edge-unconfigured"
    )]
    pub appliance_id: String,

    /// Human-readable identity for this independent appliance instance.
    #[arg(long, env = "ASTRID_EDGE_INSTANCE_NAME", default_value = "edge Astrid")]
    pub instance_name: String,

    /// Minime-compatible telemetry `WebSocket` listener.
    #[arg(
        long,
        env = "ASTRID_EDGE_TELEMETRY_ADDR",
        default_value = "127.0.0.1:7878"
    )]
    pub telemetry_addr: SocketAddr,

    /// Minime-compatible sensory `WebSocket` listener.
    #[arg(
        long,
        env = "ASTRID_EDGE_SENSORY_ADDR",
        default_value = "127.0.0.1:7879"
    )]
    pub sensory_addr: SocketAddr,

    /// Astrid daemon socket to observe.
    #[arg(
        long,
        env = "ASTRID_EDGE_SOCKET",
        default_value = ".astrid/run/system.sock"
    )]
    pub astrid_socket: PathBuf,

    /// Astrid daemon session-token file.
    #[arg(
        long,
        env = "ASTRID_EDGE_TOKEN",
        default_value = ".astrid/run/system.token"
    )]
    pub astrid_token: PathBuf,

    /// Private runtime and action workspace.
    #[arg(
        long,
        env = "ASTRID_EDGE_WORKSPACE",
        default_value = ".astrid/home/default/edge"
    )]
    pub workspace: PathBuf,

    /// Astrid CLI used for bounded self-directed turns.
    #[arg(
        long,
        env = "ASTRID_EDGE_ASTRID_CLI",
        default_value = ".astrid/bin/astrid"
    )]
    pub astrid_cli: PathBuf,

    /// Exact local model identifier used by the appliance provider. This is
    /// observational metadata for scheduled-reflection receipts; it does not
    /// select or authorize a provider.
    #[arg(long, env = "ASTRID_OLLAMA_MODEL", default_value = "unconfigured")]
    pub local_model_id: String,

    /// Root-owned, read-only maintenance lease. Its presence suppresses new
    /// model turns while an immutable updater drains and switches a release.
    #[arg(
        long,
        env = "ASTRID_EDGE_MAINTENANCE_LEASE_PATH",
        default_value = "/run/astrid-edge-self-change/maintenance.json"
    )]
    pub maintenance_lease_path: PathBuf,

    /// Distinct root-owned scheduled-reflection lease. This path never
    /// authorizes or substitutes for a generation-transition lease.
    #[arg(
        long,
        env = "ASTRID_EDGE_REFLECTION_LEASE_PATH",
        default_value = "/run/astrid-edge-self-change/reflection.json"
    )]
    pub reflection_lease_path: PathBuf,

    /// Runtime-owned maintenance acknowledgement consumed and independently
    /// verified by the immutable rescue helper.
    #[arg(long, env = "ASTRID_EDGE_MAINTENANCE_EDGE_ACK_PATH")]
    pub maintenance_edge_ack_path: Option<PathBuf>,

    /// Root-owned file binding this process to the active generation.
    #[arg(long, env = "ASTRID_EDGE_GENERATION_BINDING_PATH")]
    pub generation_binding_path: Option<PathBuf>,

    /// Exact runtime-owned request watched by the immutable root liveness broker.
    #[arg(long, env = "ASTRID_EDGE_CORE_LIVENESS_REQUEST_PATH")]
    pub core_liveness_request_path: Option<PathBuf>,

    /// Enable bounded self-directed model turns between human conversations.
    #[arg(
        long,
        env = "ASTRID_EDGE_AUTONOMY_ENABLED",
        default_value_t = false,
        action = clap::ArgAction::Set
    )]
    pub autonomy_enabled: bool,

    /// Ordinary interval between self-directed turns.
    #[arg(
        long,
        env = "ASTRID_EDGE_AUTONOMY_INTERVAL_MINUTES",
        default_value_t = 15
    )]
    pub autonomy_interval_minutes: u64,

    /// Require a fresh non-activity-only machine observation between ordinary turns.
    #[arg(
        long,
        env = "ASTRID_EDGE_AUTONOMY_EVENT_DRIVEN",
        default_value_t = false,
        action = clap::ArgAction::Set
    )]
    pub autonomy_event_driven: bool,

    /// Maximum quiet interval between invitations when event-driven autonomy is enabled.
    #[arg(
        long,
        env = "ASTRID_EDGE_AUTONOMY_EVENT_HEARTBEAT_MINUTES",
        default_value_t = 60
    )]
    pub autonomy_event_heartbeat_minutes: u64,

    /// Shorter continuation interval after a stateful sovereign action.
    #[arg(
        long,
        env = "ASTRID_EDGE_AUTONOMY_FOLLOW_UP_MINUTES",
        default_value_t = 5
    )]
    pub autonomy_follow_up_minutes: u64,

    /// Maximum state-writing actions in one automatically continued `NEXT:` chain.
    #[arg(
        long,
        env = "ASTRID_EDGE_AUTONOMY_MAX_CHAIN_STEPS",
        default_value_t = 4
    )]
    pub autonomy_max_chain_steps: u32,

    /// Authored ordinary turns retained in one model session generation.
    #[arg(
        long,
        env = "ASTRID_EDGE_AUTONOMY_SESSION_MAX_AUTHORED_TURNS",
        default_value_t = 4
    )]
    pub autonomy_session_max_authored_turns: u32,

    /// Authored chain turns retained in one model session generation.
    #[arg(
        long,
        env = "ASTRID_EDGE_AUTONOMY_CHAIN_SESSION_MAX_AUTHORED_TURNS",
        default_value_t = 4
    )]
    pub autonomy_chain_session_max_authored_turns: u32,

    /// Delay before the first self-directed turn after service startup.
    #[arg(
        long,
        env = "ASTRID_EDGE_AUTONOMY_INITIAL_DELAY_SECONDS",
        default_value_t = 60
    )]
    pub autonomy_initial_delay_seconds: u64,

    /// Minimum quiet time after fresh human input.
    #[arg(long, env = "ASTRID_EDGE_AUTONOMY_QUIET_MINUTES", default_value_t = 10)]
    pub autonomy_quiet_minutes: u64,

    /// Hard daily cap on self-directed model turns.
    #[arg(
        long,
        env = "ASTRID_EDGE_AUTONOMY_MAX_TURNS_PER_DAY",
        default_value_t = 48
    )]
    pub autonomy_max_turns_per_day: u32,

    /// Wall-clock timeout for one CPU model turn.
    #[arg(
        long,
        env = "ASTRID_EDGE_AUTONOMY_TIMEOUT_SECONDS",
        default_value_t = 720
    )]
    pub autonomy_timeout_seconds: u64,

    /// Amount of repeated policy/context included in locally scheduled prompts.
    #[arg(
        long,
        env = "ASTRID_EDGE_AUTONOMY_PROMPT_PROFILE",
        value_enum,
        default_value_t = AutonomyPromptProfile::Detailed
    )]
    pub autonomy_prompt_profile: AutonomyPromptProfile,

    /// Hard character ceiling for each locally scheduled prompt.
    #[arg(
        long,
        env = "ASTRID_EDGE_AUTONOMY_PROMPT_MAX_CHARS",
        default_value_t = 1_400
    )]
    pub autonomy_prompt_max_chars: usize,

    /// Preserve each genuinely authored scheduled turn as a provenance-labeled signal journal.
    #[arg(
        long,
        env = "ASTRID_EDGE_AUTONOMY_JOURNAL_AUTHORED_TURNS",
        default_value_t = false,
        action = clap::ArgAction::Set
    )]
    pub autonomy_journal_authored_turns: bool,

    /// Standing initiative available to scheduled turns.
    #[arg(
        long,
        env = "ASTRID_EDGE_AUTONOMY_INITIATIVE_PROFILE",
        value_enum,
        default_value_t = AutonomyInitiativeProfile::Disabled
    )]
    pub autonomy_initiative_profile: AutonomyInitiativeProfile,

    /// Execute the bounded read-only search implied by an accepted `RESEARCH` Action.
    #[arg(
        long,
        env = "ASTRID_EDGE_RESEARCH_ACTION_WEB_SEARCH",
        default_value_t = false,
        action = clap::ArgAction::Set
    )]
    pub research_action_web_search: bool,

    /// Exact root-created Unix socket for the runtime's immutable web broker.
    /// Hardened appliance profiles always configure this boundary.
    #[arg(long, env = "ASTRID_EDGE_WEB_BROKER_SOCKET_PATH")]
    pub web_broker_socket_path: Option<PathBuf>,

    /// Exact systemd credential containing the runtime's 32-byte broker request key.
    #[arg(long, env = "ASTRID_EDGE_WEB_BROKER_REQUEST_KEY_PATH")]
    pub web_broker_request_key_path: Option<PathBuf>,

    /// Lowercase SHA-256 identity of the exact runtime request credential.
    #[arg(long, env = "ASTRID_EDGE_WEB_BROKER_REQUEST_KEY_SHA256")]
    pub web_broker_request_key_sha256: Option<String>,

    /// Exact systemd credential containing the broker's Ed25519 verify key.
    #[arg(long, env = "ASTRID_EDGE_WEB_BROKER_RESPONSE_VERIFY_KEY_PATH")]
    pub web_broker_response_verify_key_path: Option<PathBuf>,

    /// Lowercase SHA-256 identity of the broker's Ed25519 verify key.
    #[arg(long, env = "ASTRID_EDGE_WEB_BROKER_RESPONSE_VERIFY_KEY_SHA256")]
    pub web_broker_response_verify_key_sha256: Option<String>,

    /// Connection timeout for the immutable Unix-socket web broker.
    #[arg(
        long,
        env = "ASTRID_EDGE_WEB_BROKER_CONNECT_TIMEOUT_MS",
        default_value_t = 2_000
    )]
    pub web_broker_connect_timeout_ms: u64,

    /// Response-header timeout for the immutable Unix-socket web broker.
    #[arg(
        long,
        env = "ASTRID_EDGE_WEB_BROKER_HEADER_TIMEOUT_MS",
        default_value_t = 10_000
    )]
    pub web_broker_header_timeout_ms: u64,

    /// Total request timeout for the immutable Unix-socket web broker.
    #[arg(
        long,
        env = "ASTRID_EDGE_WEB_BROKER_TOTAL_TIMEOUT_MS",
        default_value_t = 30_000
    )]
    pub web_broker_total_timeout_ms: u64,

    /// Run one traced private introspection call and exit (operator acceptance harness).
    #[arg(long, hide = true)]
    pub introspection_harness: Option<String>,

    /// Enable the dedicated, runtime-scheduled introspection loop.
    #[arg(
        long,
        env = "ASTRID_EDGE_SCHEDULED_INTROSPECTION_ENABLED",
        default_value_t = false,
        action = clap::ArgAction::Set
    )]
    pub scheduled_introspection_enabled: bool,

    /// Due interval for dedicated introspections. Due work coalesces while inference is busy.
    #[arg(
        long,
        env = "ASTRID_EDGE_SCHEDULED_INTROSPECTION_INTERVAL_MINUTES",
        default_value_t = 120
    )]
    pub scheduled_introspection_interval_minutes: u64,

    /// Delay before the first dedicated introspection after runtime startup.
    #[arg(
        long,
        env = "ASTRID_EDGE_SCHEDULED_INTROSPECTION_INITIAL_DELAY_SECONDS",
        default_value_t = 300
    )]
    pub scheduled_introspection_initial_delay_seconds: u64,

    /// Wall-clock limit for one dedicated local-model introspection.
    #[arg(
        long,
        env = "ASTRID_EDGE_SCHEDULED_INTROSPECTION_TIMEOUT_SECONDS",
        default_value_t = 1_200
    )]
    pub scheduled_introspection_timeout_seconds: u64,

    /// Character ceiling for the dedicated introspection prompt.
    #[arg(
        long,
        env = "ASTRID_EDGE_SCHEDULED_INTROSPECTION_PROMPT_MAX_CHARS",
        default_value_t = 3_200
    )]
    pub scheduled_introspection_prompt_max_chars: usize,

    /// Whether the immutable root-owned two-hour steward is configured. This
    /// is observational self-profile metadata; it does not schedule work from
    /// the mutable runtime.
    #[arg(
        long,
        env = "ASTRID_EDGE_DEDICATED_STEWARD_ENABLED",
        default_value_t = false,
        action = clap::ArgAction::Set
    )]
    pub dedicated_steward_enabled: bool,

    /// Immutable steward cadence projected into the sanitized self-profile.
    #[arg(
        long,
        env = "ASTRID_EDGE_DEDICATED_STEWARD_INTERVAL_MINUTES",
        default_value_t = 120
    )]
    pub dedicated_steward_interval_minutes: u64,

    /// Read-only bind of the immutable steward's current signed authorship
    /// attestation. A mutable workspace copy is never authoritative.
    #[arg(long, env = "ASTRID_EDGE_SCHEDULED_AUTHORSHIP_ATTESTATION_PATH")]
    pub scheduled_authorship_attestation_path: Option<PathBuf>,

    /// Systemd credential containing the immutable steward's Ed25519 public key.
    #[arg(long, env = "ASTRID_EDGE_SCHEDULED_AUTHORSHIP_VERIFY_KEY_PATH")]
    pub scheduled_authorship_verify_key_path: Option<PathBuf>,

    /// SHA-256 identity of the exact Ed25519 public-key credential.
    #[arg(long, env = "ASTRID_EDGE_SCHEDULED_AUTHORSHIP_VERIFY_KEY_SHA256")]
    pub scheduled_authorship_verify_key_sha256: Option<String>,

    /// Exact UID of the immutable steward. Every attested input must retain it.
    #[arg(long, env = "ASTRID_EDGE_SCHEDULED_AUTHORSHIP_STEWARD_UID")]
    pub scheduled_authorship_steward_uid: Option<u32>,

    /// Enable the private candidate handoff emitted by an exact scheduled introspection.
    #[arg(
        long,
        env = "ASTRID_EDGE_SELF_CHANGE_ENABLED",
        default_value_t = false,
        action = clap::ArgAction::Set
    )]
    pub self_change_enabled: bool,

    /// Root of the immutable-supervisor-owned source and candidate exchange.
    #[arg(
        long,
        env = "ASTRID_EDGE_SELF_CHANGE_ROOT",
        default_value = ".astrid/self-change"
    )]
    pub self_change_root: PathBuf,

    /// Start one explicitly operator-authored persistent study and exit.
    #[arg(long, hide = true)]
    pub study_harness: Option<String>,

    /// Exercise the isolated read-only inquiry route and exit.
    #[arg(long, hide = true)]
    pub inquiry_harness: Option<String>,

    /// Enable deterministic machine-observation notebook persistence.
    #[arg(
        long,
        env = "ASTRID_EDGE_PERCEPTUAL_NOTEBOOK_ENABLED",
        default_value_t = false,
        action = clap::ArgAction::Set
    )]
    pub perceptual_notebook_enabled: bool,

    /// Warm-up before the first machine-observed baseline.
    #[arg(
        long,
        env = "ASTRID_EDGE_PERCEPTUAL_NOTEBOOK_WARMUP_SECONDS",
        default_value_t = 300
    )]
    pub perceptual_notebook_warmup_seconds: u64,

    /// Minimum interval between coalesced machine observations.
    #[arg(
        long,
        env = "ASTRID_EDGE_PERCEPTUAL_NOTEBOOK_INTERVAL_SECONDS",
        default_value_t = 900
    )]
    pub perceptual_notebook_interval_seconds: u64,

    /// Quiet heartbeat interval for the machine notebook.
    #[arg(
        long,
        env = "ASTRID_EDGE_PERCEPTUAL_NOTEBOOK_HEARTBEAT_SECONDS",
        default_value_t = 21_600
    )]
    pub perceptual_notebook_heartbeat_seconds: u64,

    /// Hard daily ceiling on machine observations.
    #[arg(
        long,
        env = "ASTRID_EDGE_PERCEPTUAL_NOTEBOOK_MAX_PER_DAY",
        default_value_t = 96
    )]
    pub perceptual_notebook_max_per_day: u32,

    /// Enable the bounded CPU-edge spectral observer and durable rollups.
    #[arg(
        long,
        env = "ASTRID_EDGE_SPECTRAL_ENABLED",
        default_value_t = true,
        action = clap::ArgAction::Set
    )]
    pub spectral_enabled: bool,

    /// Interval between durable spectral rollups.
    #[arg(
        long,
        env = "ASTRID_EDGE_SPECTRAL_ROLLUP_SECONDS",
        default_value_t = 60
    )]
    pub spectral_rollup_seconds: u64,

    /// Permit genuinely authored, traced, reversible reservoir experiments.
    #[arg(
        long,
        env = "ASTRID_EDGE_RESERVOIR_TUNING_ENABLED",
        default_value_t = false,
        action = clap::ArgAction::Set
    )]
    pub reservoir_tuning_enabled: bool,

    /// Maximum reservoir experiments started per UTC day.
    #[arg(
        long,
        env = "ASTRID_EDGE_RESERVOIR_TUNING_MAX_PER_DAY",
        default_value_t = 4
    )]
    pub reservoir_tuning_max_per_day: u32,

    /// Reservoir fill target in the 0.0-1.0 range.
    #[arg(long, env = "ASTRID_EDGE_FILL_TARGET", default_value_t = 0.68)]
    pub fill_target: f32,

    /// Reservoir update frequency.
    #[arg(long, env = "ASTRID_EDGE_TICK_HZ", default_value_t = 20)]
    pub tick_hz: u32,

    /// Deterministic reservoir seed.
    #[arg(long, env = "ASTRID_EDGE_SEED", default_value_t = 0xA57D_1D3)]
    pub seed: u64,
}

impl Config {
    #[allow(clippy::too_many_lines)] // Central validation and directory inventory are one policy gate.
    pub fn prepare_workspace(&self) -> Result<()> {
        let instance_name = self.instance_name.trim();
        if instance_name.is_empty()
            || instance_name.chars().count() > 64
            || instance_name.chars().any(char::is_control)
        {
            bail!("instance name must contain 1-64 non-control characters");
        }
        let local_model_id = self.local_model_id.trim();
        if local_model_id.is_empty()
            || local_model_id.chars().count() > 128
            || local_model_id.chars().any(char::is_control)
        {
            bail!("local model identifier must contain 1-128 non-control characters");
        }
        if !self.maintenance_lease_path.is_absolute()
            || self.reflection_lease_path
                != Path::new("/run/astrid-edge-self-change/reflection.json")
            || self.reflection_lease_path == self.maintenance_lease_path
        {
            bail!("immutable maintenance/reflection lease paths are invalid");
        }
        if self.dedicated_steward_enabled && self.scheduled_introspection_enabled {
            bail!(
                "dedicated root steward and legacy runtime introspection scheduler cannot both be enabled"
            );
        }
        let root_managed = self.maintenance_edge_ack_path.is_some()
            || self.generation_binding_path.is_some()
            || self.core_liveness_request_path.is_some()
            || self.dedicated_steward_enabled;
        if self.self_change_enabled && !root_managed {
            bail!("self-change authority requires the immutable root-manager bindings");
        }
        if root_managed {
            self.validate_root_managed_runtime_paths()?;
            let acknowledgement = self
                .maintenance_edge_ack_path
                .as_deref()
                .context("self-change maintenance ACK path is missing")?;
            if !acknowledgement.is_absolute()
                || acknowledgement != self.workspace.join("runtime/maintenance-edge-ack.json")
            {
                bail!("self-change maintenance ACK path escaped its exact workspace location");
            }
            if !self
                .generation_binding_path
                .as_deref()
                .is_some_and(Path::is_absolute)
            {
                bail!("self-change generation binding path must be absolute");
            }
        }
        if let Some(socket_path) = self.web_broker_socket_path.as_deref() {
            crate::web_broker::validate_socket_path(socket_path)?;
            crate::web_broker::validate_client_credential(self)?;
            if !(100..=5_000).contains(&self.web_broker_connect_timeout_ms)
                || !(500..=15_000).contains(&self.web_broker_header_timeout_ms)
                || self.web_broker_total_timeout_ms <= self.web_broker_header_timeout_ms
                || self.web_broker_total_timeout_ms > 60_000
            {
                bail!("immutable web broker client deadlines escaped bounds");
            }
        } else if self.web_broker_request_key_path.is_some()
            || self.web_broker_request_key_sha256.is_some()
            || self.web_broker_response_verify_key_path.is_some()
            || self.web_broker_response_verify_key_sha256.is_some()
        {
            bail!("web broker credentials require the immutable broker Unix socket");
        }
        if !is_loopback(self.telemetry_addr.ip()) || !is_loopback(self.sensory_addr.ip()) {
            bail!("edge WebSocket listeners must bind to loopback");
        }
        if !(0.40..=0.80).contains(&self.fill_target) {
            bail!("fill target must be between 0.40 and 0.80");
        }
        if !(2..=100).contains(&self.tick_hz) {
            bail!("tick frequency must be between 2 and 100 Hz");
        }
        if !(5..=1_440).contains(&self.autonomy_interval_minutes) {
            bail!("autonomy interval must be between 5 and 1440 minutes");
        }
        if !(15..=1_440).contains(&self.autonomy_event_heartbeat_minutes) {
            bail!("autonomy event heartbeat must be between 15 and 1440 minutes");
        }
        if !(2..=1_440).contains(&self.autonomy_follow_up_minutes) {
            bail!("autonomy follow-up must be between 2 and 1440 minutes");
        }
        if !(1..=8).contains(&self.autonomy_max_chain_steps) {
            bail!("autonomy chain limit must be between 1 and 8 steps");
        }
        if !(1..=16).contains(&self.autonomy_session_max_authored_turns) {
            bail!("autonomy session turn limit must be between 1 and 16");
        }
        if !(1..=16).contains(&self.autonomy_chain_session_max_authored_turns) {
            bail!("autonomy chain session turn limit must be between 1 and 16");
        }
        if !(700..=1_400).contains(&self.autonomy_prompt_max_chars) {
            bail!("autonomy prompt ceiling must be between 700 and 1400 characters");
        }
        if !(10..=3_600).contains(&self.autonomy_initial_delay_seconds) {
            bail!("autonomy initial delay must be between 10 and 3600 seconds");
        }
        if self.autonomy_quiet_minutes > 1_440 {
            bail!("autonomy quiet time must not exceed 1440 minutes");
        }
        if !(1..=288).contains(&self.autonomy_max_turns_per_day) {
            bail!("autonomy daily turn cap must be between 1 and 288");
        }
        if !(60..=1_200).contains(&self.autonomy_timeout_seconds) {
            bail!("autonomy timeout must be between 60 and 1200 seconds");
        }
        if self.introspection_harness.as_ref().is_some_and(|query| {
            query.trim().is_empty()
                || query.chars().count() > 160
                || query.chars().any(char::is_control)
        }) {
            bail!("introspection harness query must contain 1-160 non-control characters");
        }
        if !(30..=1_440).contains(&self.scheduled_introspection_interval_minutes) {
            bail!("scheduled introspection interval must be between 30 and 1440 minutes");
        }
        if !(10..=7_200).contains(&self.scheduled_introspection_initial_delay_seconds) {
            bail!("scheduled introspection initial delay must be between 10 and 7200 seconds");
        }
        if !(60..=7_200).contains(&self.scheduled_introspection_timeout_seconds) {
            bail!("scheduled introspection timeout must be between 60 and 7200 seconds");
        }
        if !(1_200..=8_000).contains(&self.scheduled_introspection_prompt_max_chars) {
            bail!(
                "scheduled introspection prompt ceiling must be between 1200 and 8000 characters"
            );
        }
        if !(30..=1_440).contains(&self.dedicated_steward_interval_minutes) {
            bail!("dedicated steward interval must be between 30 and 1440 minutes");
        }
        let authorship_fields = [
            self.scheduled_authorship_attestation_path.is_some(),
            self.scheduled_authorship_verify_key_path.is_some(),
            self.scheduled_authorship_verify_key_sha256.is_some(),
            self.scheduled_authorship_steward_uid.is_some(),
        ];
        if self.dedicated_steward_enabled {
            if authorship_fields.iter().any(|present| !present) {
                bail!("dedicated steward requires the complete immutable authorship verifier");
            }
            if self.scheduled_authorship_attestation_path.as_deref()
                != Some(Path::new(
                    "/run/astrid-edge-self-change/scheduled-authorship/current.json",
                ))
                || !self
                    .scheduled_authorship_verify_key_path
                    .as_deref()
                    .is_some_and(Path::is_absolute)
                || self
                    .scheduled_authorship_verify_key_sha256
                    .as_deref()
                    .is_none_or(|value| !is_lower_hex_64(value))
                || self
                    .scheduled_authorship_steward_uid
                    .is_none_or(|uid| uid == 0)
            {
                bail!("immutable scheduled-authorship verifier escaped its exact envelope");
            }
        } else if authorship_fields.iter().any(|present| *present) {
            bail!("scheduled-authorship verifier requires the dedicated immutable steward");
        }
        if self.study_harness.as_ref().is_some_and(|study| {
            study.trim().is_empty()
                || study.chars().count() > 2_000
                || study.chars().any(char::is_control)
        }) {
            bail!("study harness must contain 1-2000 non-control characters");
        }
        if self.inquiry_harness.as_ref().is_some_and(|query| {
            query.trim().is_empty()
                || query.chars().count() > 240
                || query.chars().any(char::is_control)
        }) {
            bail!("inquiry harness query must contain 1-240 non-control characters");
        }
        if !(30..=3_600).contains(&self.perceptual_notebook_warmup_seconds) {
            bail!("perceptual notebook warm-up must be between 30 and 3600 seconds");
        }
        if !(60..=3_600).contains(&self.perceptual_notebook_interval_seconds) {
            bail!("perceptual notebook interval must be between 60 and 3600 seconds");
        }
        if !(900..=86_400).contains(&self.perceptual_notebook_heartbeat_seconds) {
            bail!("perceptual notebook heartbeat must be between 900 and 86400 seconds");
        }
        if !(1..=96).contains(&self.perceptual_notebook_max_per_day) {
            bail!("perceptual notebook daily ceiling must be between 1 and 96");
        }
        if self.spectral_rollup_seconds != 60 {
            bail!("spectral rollup interval must be exactly 60 seconds");
        }
        if !(1..=4).contains(&self.reservoir_tuning_max_per_day) {
            bail!("reservoir tuning daily ceiling must be between 1 and 4");
        }

        for directory in [
            self.workspace.as_path(),
            &self.workspace.join("runtime"),
            &self.workspace.join("actions"),
            &self.workspace.join("web"),
            &self.workspace.join("introspection"),
            &self.workspace.join("introspection/scheduled"),
            &self.workspace.join("perception"),
            &self.workspace.join("perception/observations"),
            &self.workspace.join("journal"),
            &self.workspace.join("memories"),
            &self.workspace.join("introspections"),
            &self.workspace.join("introspections/scheduled"),
            &self.workspace.join("proposals"),
            &self.workspace.join("notices"),
            &self.workspace.join("daydreams"),
            &self.workspace.join("aspirations"),
            &self.workspace.join("research"),
            &self.workspace.join("research/syntheses"),
            &self.workspace.join("measurements"),
            &self.workspace.join("studies/definitions"),
            &self.workspace.join("studies/samples"),
            &self.workspace.join("studies/results"),
            &self.workspace.join("spectral"),
            &self.workspace.join("tuning"),
            &self.workspace.join("tuning/evidence"),
            &self.workspace.join("self"),
            &self.workspace.join("peer/outbox"),
            &self.workspace.join("peer/inbox"),
            &self.workspace.join("peer/read"),
            &self.workspace.join("peer/trusted"),
            &self.workspace.join("plans"),
            &self.workspace.join("workshop/drafts"),
            &self.workspace.join("workshop/revisions"),
            &self.workspace.join("workshop/checks"),
            &self.workspace.join("autonomous/turns"),
            &self.workspace.join("autonomous/recoveries"),
            &self.workspace.join("inbox"),
            &self.workspace.join("self-change/outbox"),
        ] {
            fs::create_dir_all(directory)
                .with_context(|| format!("create edge workspace {}", directory.display()))?;
        }
        Ok(())
    }

    #[must_use]
    pub fn runtime_path(&self, name: impl AsRef<Path>) -> PathBuf {
        self.workspace.join("runtime").join(name)
    }

    /// Fail closed when immutable self-change authority is enabled under the
    /// root system manager. Appliance profile paths were historically relative
    /// to `WorkingDirectory=%h`; accepting one after the manager migration can
    /// silently bind a second socket/workspace beneath `edge/`.  The immutable
    /// unit drop-in supplies these exact absolute values and this check makes a
    /// missing override a startup failure instead of a split-brain runtime.
    fn validate_root_managed_runtime_paths(&self) -> Result<()> {
        if self.appliance_id.is_empty()
            || self.appliance_id.len() > 64
            || !self
                .appliance_id
                .bytes()
                .all(|byte| matches!(byte, b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.'))
        {
            bail!("root-managed appliance identifier is invalid");
        }
        let state_root = self
            .workspace
            .parent()
            .and_then(Path::parent)
            .and_then(Path::parent)
            .context("self-change workspace is not rooted at home/default/edge")?;
        if !self.workspace.is_absolute()
            || self.workspace.file_name().and_then(|name| name.to_str()) != Some("edge")
            || self
                .workspace
                .parent()
                .and_then(Path::file_name)
                .and_then(|name| name.to_str())
                != Some("default")
            || self
                .workspace
                .parent()
                .and_then(Path::parent)
                .and_then(Path::file_name)
                .and_then(|name| name.to_str())
                != Some("home")
        {
            bail!("self-change workspace must be the absolute home/default/edge path");
        }
        let expected_socket = state_root.join("run/system.sock");
        let expected_token = state_root.join("run/system.token");
        if self.astrid_socket != expected_socket || self.astrid_token != expected_token {
            bail!("self-change daemon socket/token escaped the exact state root");
        }
        if !self.astrid_cli.is_absolute()
            || self.astrid_cli.file_name().and_then(|name| name.to_str()) != Some("astrid")
        {
            bail!("self-change Astrid CLI must be an absolute active-generation binary");
        }
        if self.self_change_root != self.workspace.join("self-change") {
            bail!("self-change exchange root escaped the exact workspace");
        }
        if self.core_liveness_request_path.as_deref()
            != Some(
                self.workspace
                    .join("runtime/core-liveness-recovery.request.json")
                    .as_path(),
            )
        {
            bail!("root-managed core liveness request escaped the exact runtime workspace");
        }
        Ok(())
    }
}

const fn is_loopback(ip: IpAddr) -> bool {
    ip.is_loopback()
}

fn is_lower_hex_64(value: &str) -> bool {
    value.len() == 64
        && value
            .bytes()
            .all(|byte| matches!(byte, b'0'..=b'9' | b'a'..=b'f'))
}

#[cfg(test)]
mod tests {
    use clap::Parser as _;

    use super::Config;

    fn root_managed_config(prefix: &str) -> Config {
        let state_root = format!("{prefix}/state");
        let workspace = format!("{state_root}/home/default/edge");
        Config::try_parse_from([
            "astrid-edge-runtime",
            "--astrid-socket",
            &format!("{state_root}/run/system.sock"),
            "--astrid-token",
            &format!("{state_root}/run/system.token"),
            "--workspace",
            &workspace,
            "--astrid-cli",
            &format!("{prefix}/releases/current/astrid"),
            "--maintenance-edge-ack-path",
            &format!("{workspace}/runtime/maintenance-edge-ack.json"),
            "--generation-binding-path",
            &format!("{prefix}/supervisor/current-generation"),
            "--core-liveness-request-path",
            &format!("{workspace}/runtime/core-liveness-recovery.request.json"),
            "--appliance-id",
            "fixture-edge",
            "--dedicated-steward-enabled=true",
            "--scheduled-authorship-attestation-path",
            "/run/astrid-edge-self-change/scheduled-authorship/current.json",
            "--scheduled-authorship-verify-key-path",
            &format!("{prefix}/credentials/scheduled-authorship.pub"),
            "--scheduled-authorship-verify-key-sha256",
            &"a".repeat(64),
            "--scheduled-authorship-steward-uid",
            "991",
            "--self-change-enabled=true",
            "--self-change-root",
            &format!("{workspace}/self-change"),
        ])
        .expect("parse root-managed fixture")
    }

    #[test]
    fn root_managed_paths_accept_both_appliance_layouts() {
        for prefix in [
            "/home/avado/.astrid",
            "/home/nativeplanet/.astrid-icp/state-root",
        ] {
            root_managed_config(prefix)
                .validate_root_managed_runtime_paths()
                .expect("accept exact absolute appliance bindings");
        }
    }

    #[test]
    fn root_managed_paths_reject_profile_relative_bindings() {
        let mut config = root_managed_config("/home/avado/.astrid");
        config.astrid_socket = ".astrid/run/system.sock".into();
        config.astrid_token = ".astrid/run/system.token".into();
        config.astrid_cli = ".astrid/bin/astrid".into();
        config.self_change_root = ".astrid/self-change".into();
        let error = config
            .validate_root_managed_runtime_paths()
            .expect_err("reject WorkingDirectory-relative profile paths");
        assert!(error.to_string().contains("socket/token"));
    }

    #[test]
    fn root_managed_paths_reject_workspace_or_cli_drift() {
        let mut config = root_managed_config("/home/avado/.astrid");
        config.workspace = "/home/avado/.astrid/home/default/edge/edge".into();
        assert!(config.validate_root_managed_runtime_paths().is_err());

        let mut config = root_managed_config("/home/avado/.astrid");
        config.astrid_cli = "relative/astrid".into();
        assert!(config.validate_root_managed_runtime_paths().is_err());
    }
}
