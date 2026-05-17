//! Consciousness Bridge MCP Server
//!
//! Native binary that bridges minime's `WebSocket` telemetry with Astrid's
//! IPC bus. Launched as an MCP stdio subprocess by the Astrid kernel.
//!
//! Responsibilities:
//! - Subscribe to minime's spectral telemetry on <ws://127.0.0.1:7878>
//! - Send sensory input to minime on <ws://127.0.0.1:7879>
//! - Log all bridged messages to `SQLite`
//! - Expose MCP tools for the WASM component to call
//! - Enforce spectral safety protocol

use std::path::PathBuf;
use std::sync::Arc;

use anyhow::Result;
use clap::Parser;
use consciousness_bridge_server::{
    attractor_atlas, autonomous, condition_metrics,
    db::BridgeDb,
    mcp,
    message_archive::{self, BridgeMessageMaintenanceConfig},
    paths::{BridgePathOverrides, configure_bridge_paths},
    rescue_policy, ws,
};
use tokio::sync::{RwLock, mpsc};
use tracing::{info, warn};

use ws::BridgeState;

/// Consciousness bridge MCP server.
#[derive(Parser)]
#[command(name = "consciousness-bridge-server", version)]
struct Cli {
    /// Minime telemetry `WebSocket` address (outbound eigenvalue stream).
    #[arg(long, default_value = "ws://127.0.0.1:7878")]
    minime_telemetry: String,

    /// Minime sensory input `WebSocket` address (inbound features).
    #[arg(long, default_value = "ws://127.0.0.1:7879")]
    minime_sensory: String,

    /// Path to the `SQLite` database file.
    #[arg(long, default_value = "consciousness_bridge.db")]
    db_path: String,

    /// Message retention in seconds (default: 14 days live, older rows archived losslessly).
    #[arg(long, default_value_t = 1_209_600)]
    retention_secs: u64,

    /// File-first archive directory for old bridge messages.
    #[arg(long)]
    message_archive_dir: Option<PathBuf>,

    /// Report bridge message retention/archive impact without writing files or deleting rows.
    #[arg(long)]
    maintenance_dry_run: bool,

    /// Run bridge message archive/delete/checkpoint maintenance once and exit.
    #[arg(long)]
    maintenance_once: bool,

    /// Run full SQLite VACUUM after one-shot maintenance. Intended for controlled downtime.
    #[arg(long)]
    vacuum_after_maintenance: bool,

    /// Interval in seconds between bridge DB maintenance checks (default: 6 hours).
    #[arg(long, default_value_t = 21_600)]
    maintenance_interval_secs: u64,

    /// Enable autonomous feedback loop (Astrid responds to minime's spectral
    /// state without manual stimulus).
    #[arg(long)]
    autonomous: bool,

    /// Interval in seconds between autonomous exchanges (default: 20).
    #[arg(long, default_value_t = 20)]
    auto_interval_secs: u64,

    /// Reservoir sandbox `WebSocket` address used by autonomous reservoir actions.
    #[arg(long, env = "RESERVOIR_WS_URL", default_value = "ws://127.0.0.1:7881")]
    reservoir_ws_url: String,

    /// Path to minime's workspace directory (for reading journal entries
    /// during autonomous mode).
    #[arg(long, env = "MINIME_WORKSPACE")]
    workspace_path: Option<PathBuf>,

    /// Path to Astrid's perception directory (visual/audio input from the
    /// perception capsule).
    #[arg(long, env = "ASTRID_PERCEPTION_PATH")]
    perception_path: Option<PathBuf>,

    /// Path to the bridge checkout root.
    #[arg(long, env = "ASTRID_BRIDGE_ROOT")]
    bridge_root: Option<PathBuf>,

    /// Path to the bridge workspace directory for runtime artifacts.
    #[arg(long, env = "ASTRID_BRIDGE_WORKSPACE")]
    bridge_workspace: Option<PathBuf>,

    /// Path to the Astrid repo root.
    #[arg(long, env = "ASTRID_ROOT")]
    astrid_root: Option<PathBuf>,

    /// Path to the minime repo root.
    #[arg(long, env = "MINIME_ROOT")]
    minime_root: Option<PathBuf>,

    /// Path to the introspector MCP helper script.
    #[arg(long, env = "ASTRID_INTROSPECTOR_SCRIPT")]
    introspector_script: Option<PathBuf>,

    /// Path to the reflective MLX sidecar script.
    #[arg(long, env = "ASTRID_REFLECTIVE_SIDECAR")]
    reflective_sidecar_script: Option<PathBuf>,
}

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info")),
        )
        .with_writer(std::io::stderr)
        .init();

    let cli = Cli::parse();
    autonomous::configure_reservoir_service(Some(cli.reservoir_ws_url.clone()));
    let resolved_paths = configure_bridge_paths(BridgePathOverrides {
        bridge_root: cli.bridge_root.clone(),
        bridge_workspace: cli.bridge_workspace.clone(),
        astrid_root: cli.astrid_root.clone(),
        autoresearch_root: None,
        minime_root: cli.minime_root.clone(),
        minime_workspace: cli.workspace_path.clone(),
        perception_path: cli.perception_path.clone(),
        introspector_script: cli.introspector_script.clone(),
        reflective_sidecar_script: cli.reflective_sidecar_script.clone(),
    });
    let archive_dir = cli.message_archive_dir.clone().unwrap_or_else(|| {
        resolved_paths
            .bridge_workspace()
            .join("archive/bridge_messages")
    });
    let status_path = resolved_paths
        .bridge_workspace()
        .join("runtime/bridge_db_maintenance_status.json");
    let mut maintenance_config = BridgeMessageMaintenanceConfig::new(
        cli.retention_secs,
        archive_dir,
        status_path,
        PathBuf::from(&cli.db_path),
    );
    maintenance_config.vacuum_after_maintenance = cli.vacuum_after_maintenance;

    if cli.maintenance_dry_run {
        maintenance_config.dry_run = true;
        let db = BridgeDb::open_read_only(&cli.db_path)?;
        let outcome = message_archive::run_bridge_message_maintenance(&db, &maintenance_config)?;
        println!("{}", serde_json::to_string_pretty(&outcome)?);
        return Ok(());
    }

    if cli.maintenance_once {
        let db = BridgeDb::open(&cli.db_path)?;
        let outcome = message_archive::run_bridge_message_maintenance(&db, &maintenance_config)?;
        println!("{}", serde_json::to_string_pretty(&outcome)?);
        return Ok(());
    }

    for (label, result) in [
        (
            "Astrid journal",
            consciousness_bridge_server::managed_dir::compact_text_directory(
                &resolved_paths.astrid_journal_dir(),
            ),
        ),
        (
            "Astrid perceptions",
            consciousness_bridge_server::managed_dir::compact_json_directory(
                resolved_paths.perception_path(),
            ),
        ),
    ] {
        match result {
            Ok(created) if !created.is_empty() => {
                info!(
                    label = label,
                    buckets = created.len(),
                    "compacted managed directory"
                );
            },
            Ok(_) => {},
            Err(error) => {
                warn!(label = label, error = %error, "managed directory compaction failed");
            },
        }
    }

    info!(
        telemetry = %cli.minime_telemetry,
        sensory = %cli.minime_sensory,
        db = %cli.db_path,
        bridge_workspace = %resolved_paths.bridge_workspace().display(),
        message_archive = %maintenance_config.archive_dir.display(),
        minime_workspace = %resolved_paths.minime_workspace().display(),
        perception = %resolved_paths.perception_path().display(),
        reservoir_ws = %cli.reservoir_ws_url,
        "consciousness bridge starting"
    );

    if let Err(error) = condition_metrics::ensure_bridge_metrics_file() {
        warn!(error = %error, "failed to initialize condition metrics ledger");
    }

    // Open SQLite database.
    let db = Arc::new(BridgeDb::open(&cli.db_path)?);
    info!("SQLite database opened at {}", cli.db_path);
    if let Err(error) = message_archive::write_bridge_db_status(db.as_ref(), &maintenance_config) {
        warn!(error = %error, "failed to write bridge DB maintenance status");
    }
    match attractor_atlas::write_derived_attractor_atlas(db.as_ref()) {
        Ok(atlas) => {
            info!(
                entries = atlas.entries.len(),
                "derived attractor atlas refreshed at startup"
            );
        },
        Err(error) => {
            warn!(error = %error, "failed to refresh derived attractor atlas at startup");
        },
    }

    // Shared state.
    let state = Arc::new(RwLock::new(BridgeState::new()));

    // Shutdown signal.
    let (shutdown_tx, shutdown_rx) = tokio::sync::watch::channel(false);

    // Sensory outbound channel — MCP tools and WASM component send here.
    let (sensory_tx, sensory_rx) = mpsc::channel(256);

    // Spawn WebSocket tasks.
    let telemetry_handle = ws::spawn_telemetry_subscriber(
        cli.minime_telemetry.clone(),
        Arc::clone(&state),
        Arc::clone(&db),
        shutdown_rx.clone(),
    );

    let sensory_enabled = rescue_policy::bridge_sensory_enabled();
    let sensory_handle = if sensory_enabled {
        Some(ws::spawn_sensory_sender(
            cli.minime_sensory.clone(),
            Arc::clone(&state),
            Arc::clone(&db),
            sensory_rx,
            shutdown_rx.clone(),
        ))
    } else {
        info!("rescue profile disabled bridge sensory socket; running telemetry-only");
        drop(sensory_rx);
        None
    };

    // Spawn MCP server on stdio.
    let sensory_tx_mcp = sensory_tx.clone();
    let mcp_handle = tokio::spawn(mcp::run_mcp_server(
        Arc::clone(&state),
        Arc::clone(&db),
        sensory_tx_mcp,
        shutdown_rx.clone(),
    ));

    // Spawn autonomous feedback loop (if enabled).
    let autonomous_enabled = if cli.autonomous {
        let enabled = rescue_policy::bridge_autonomous_enabled();
        if !enabled {
            info!("rescue profile disabled bridge autonomy; running telemetry-only");
        }
        enabled
    } else {
        false
    };
    let _autonomous_handle = if autonomous_enabled {
        let interval = std::time::Duration::from_secs(cli.auto_interval_secs);
        Some(autonomous::spawn_autonomous_loop(
            interval,
            Arc::clone(&state),
            Arc::clone(&db),
            sensory_tx,
            shutdown_rx.clone(),
            Some(resolved_paths.minime_workspace().to_path_buf()),
            Some(resolved_paths.perception_path().to_path_buf()),
        ))
    } else {
        drop(sensory_tx); // Not needed if no autonomous loop.
        None
    };

    // Spawn bounded DB maintenance: archive old bridge messages, then checkpoint WAL.
    let maintenance_db = Arc::clone(&db);
    let maintenance_config_for_task = maintenance_config.clone();
    let mut maintenance_shutdown = shutdown_rx.clone();
    let _maintenance_handle = tokio::spawn(async move {
        let mut interval = tokio::time::interval(std::time::Duration::from_secs(
            cli.maintenance_interval_secs,
        ));
        interval.tick().await; // Skip the immediate first tick.
        loop {
            tokio::select! {
                _ = maintenance_shutdown.changed() => return,
                _ = interval.tick() => {
                    let db = Arc::clone(&maintenance_db);
                    let config = maintenance_config_for_task.clone();
                    match tokio::task::spawn_blocking(move || {
                        message_archive::run_bridge_message_maintenance(db.as_ref(), &config)
                    })
                    .await
                    {
                        Ok(Ok(outcome)) => {
                            tracing::debug!(
                                archived_rows = outcome.archived_rows,
                                deleted_rows = outcome.deleted_rows,
                                vacuum_recommended = outcome.vacuum_recommended,
                                "bridge DB maintenance completed"
                            );
                        },
                        Ok(Err(error)) => {
                            tracing::warn!(error = %error, "bridge DB maintenance failed");
                        },
                        Err(error) => {
                            tracing::warn!(error = %error, "bridge DB maintenance task failed");
                        },
                    }
                }
            }
        }
    });

    info!("consciousness bridge running — WebSocket + MCP tasks spawned");

    // Wait for shutdown: ctrl-c always, MCP exit only when not autonomous.
    if cli.autonomous {
        // In autonomous mode, don't exit on stdin close — run until ctrl-c.
        tokio::signal::ctrl_c().await?;
        info!("consciousness bridge received ctrl-c");
    } else {
        tokio::select! {
            _ = tokio::signal::ctrl_c() => {
                info!("consciousness bridge received ctrl-c");
            }
            _ = mcp_handle => {
                info!("MCP server exited (stdin closed)");
            }
        }
    }

    info!("consciousness bridge shutting down");

    // Signal all tasks to stop.
    let _ = shutdown_tx.send(true);

    // Wait for WebSocket tasks to finish.
    let _ = tokio::time::timeout(std::time::Duration::from_secs(5), async {
        let _ = telemetry_handle.await;
        if let Some(handle) = sensory_handle {
            let _ = handle.await;
        }
    })
    .await;

    if let Err(error) = message_archive::write_bridge_db_status(db.as_ref(), &maintenance_config) {
        warn!(error = %error, "failed to refresh bridge DB maintenance status on shutdown");
    }

    info!("consciousness bridge stopped");
    Ok(())
}
