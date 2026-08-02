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

    /// Run one traced private introspection call and exit (operator acceptance harness).
    #[arg(long, hide = true)]
    pub introspection_harness: Option<String>,

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
        if !(15..=3_600).contains(&self.spectral_rollup_seconds) {
            bail!("spectral rollup interval must be between 15 and 3600 seconds");
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
            &self.workspace.join("perception"),
            &self.workspace.join("perception/observations"),
            &self.workspace.join("journal"),
            &self.workspace.join("memories"),
            &self.workspace.join("introspections"),
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
}

const fn is_loopback(ip: IpAddr) -> bool {
    ip.is_loopback()
}
