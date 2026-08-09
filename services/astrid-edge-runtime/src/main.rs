#![deny(unsafe_code)]

mod actions;
mod autonomy;
mod codec;
mod config;
mod host;
mod inquiry;
mod ipc;
mod maintenance;
mod notebook;
mod operator_inquiry;
mod peer;
mod reservoir;
mod scheduled_admission;
mod scheduled_introspection;
mod self_profile;
mod spectral;
mod trace;
mod tuning;
mod web_broker;
mod ws;

use std::sync::Arc;

use anyhow::Result;
use clap::Parser as _;
use tokio::sync::{Mutex, broadcast, mpsc, watch};
use uuid::Uuid;

use crate::{
    actions::{ActionCandidate, ActionOutcomeDelivery},
    config::Config,
    reservoir::{ReservoirSnapshot, SensoryIngress},
};

#[tokio::main]
#[allow(clippy::too_many_lines)] // Explicit task wiring keeps shutdown ownership visible.
async fn main() -> Result<()> {
    let config = Arc::new(Config::parse());
    config.prepare_workspace()?;
    if let Some(question) = config.inquiry_harness.as_deref() {
        let result = operator_inquiry::run(&config, question).await?;
        println!("{}", serde_json::to_string_pretty(&result)?);
        return Ok(());
    }
    if let Some(declaration) = config.study_harness.as_deref() {
        let spec = inquiry::parse_study(declaration)
            .ok_or_else(|| anyhow::anyhow!("invalid operator study harness grammar"))?;
        let mut manager = inquiry::StudyManager::load(&config);
        let path = manager.start(
            &config,
            &ReservoirSnapshot::default(),
            u64::try_from(
                std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap_or_default()
                    .as_millis(),
            )
            .unwrap_or(u64::MAX),
            &spec,
            None,
            None,
            "operator_harness",
        )?;
        println!("{path}");
        return Ok(());
    }
    if let Some(query) = config.introspection_harness.as_deref() {
        let session_id = ipc::operator_introspection_session_id();
        let trace = trace::IpcTraceContextV1::root(Uuid::new_v4(), session_id, None);
        let result = ipc::execute_introspection_search(
            &config,
            query,
            Some(&trace),
            None,
            "operator_harness",
        )
        .await?;
        println!("{}", serde_json::to_string_pretty(&result)?);
        return Ok(());
    }

    let (ingress_tx, ingress_rx) = mpsc::channel::<SensoryIngress>(2_048);
    let (reservoir_command_tx, reservoir_command_rx) =
        mpsc::channel::<reservoir::ReservoirCommand>(32);
    let (tuning_tx, tuning_rx) = mpsc::channel::<tuning::TuningRequest>(32);
    let (action_tx, action_rx) = mpsc::channel::<ActionCandidate>(64);
    let (action_outcome_tx, action_outcome_rx) = mpsc::channel::<ActionOutcomeDelivery>(64);
    let (telemetry_tx, _) = broadcast::channel::<String>(64);
    let (snapshot_tx, snapshot_rx) = watch::channel(ReservoirSnapshot::default());
    let (human_activity_tx, human_activity_rx) = watch::channel(0_u64);
    let (notebook_activity_tx, _) = broadcast::channel::<notebook::ActivityEvent>(128);
    let study_manager = Arc::new(std::sync::Mutex::new(inquiry::StudyManager::load(&config)));
    let autonomy_trace_registry = Arc::new(trace::AutonomyTraceRegistry::default());
    let model_turn_lock = Arc::new(Mutex::new(()));
    let maintenance_probe = config
        .self_change_enabled
        .then(|| maintenance::LeaseProbe::from_config(&config))
        .transpose()?;
    let maintenance_work = Arc::new(maintenance::WorkTracker::new(
        config.maintenance_edge_ack_path.clone(),
        maintenance_probe,
    ));

    let mut maintenance_task = config.self_change_enabled.then(|| {
        let config = Arc::clone(&config);
        let maintenance_work = Arc::clone(&maintenance_work);
        tokio::spawn(async move {
            if let Err(error) = maintenance::run(config, maintenance_work).await {
                eprintln!("maintenance acknowledgement failed: {error:#}");
            }
        })
    });

    let mut reservoir_task = tokio::spawn(reservoir::run(
        Arc::clone(&config),
        ingress_rx,
        reservoir_command_rx,
        telemetry_tx.clone(),
        snapshot_tx,
    ));
    let mut scheduled_admission_task = tokio::spawn(scheduled_admission::run(
        Arc::clone(&config),
        ingress_tx.clone(),
    ));
    let telemetry_task = tokio::spawn(ws::serve_telemetry(
        config.telemetry_addr,
        telemetry_tx,
        snapshot_rx.clone(),
    ));
    let sensory_task = tokio::spawn(ws::serve_sensory(config.sensory_addr, ingress_tx.clone()));
    let host_task = tokio::spawn(host::run(ingress_tx.clone()));
    let ipc_task = tokio::spawn(ipc::run(
        Arc::clone(&config),
        ingress_tx.clone(),
        action_tx.clone(),
        human_activity_tx,
        notebook_activity_tx.clone(),
        Arc::clone(&autonomy_trace_registry),
        Arc::clone(&maintenance_work),
    ));
    let mut action_task = tokio::spawn(actions::run(
        Arc::clone(&config),
        action_rx,
        snapshot_rx.clone(),
        ingress_tx.clone(),
        action_outcome_tx,
        notebook_activity_tx.clone(),
        Arc::clone(&study_manager),
        tuning_tx,
        Arc::clone(&maintenance_work),
    ));
    let mut tuning_task = tokio::spawn(tuning::run(
        Arc::clone(&config),
        tuning_rx,
        reservoir_command_tx,
        snapshot_rx.clone(),
    ));
    let inquiry_task = tokio::spawn(inquiry::run(
        Arc::clone(&config),
        study_manager,
        snapshot_rx.clone(),
        notebook_activity_tx.subscribe(),
        ingress_tx.clone(),
        notebook_activity_tx.clone(),
    ));
    let self_profile_task =
        tokio::spawn(self_profile::run(Arc::clone(&config), snapshot_rx.clone()));
    let peer_task = tokio::spawn(peer::run(Arc::clone(&config)));
    let notebook_task = config.perceptual_notebook_enabled.then(|| {
        tokio::spawn(notebook::run(
            Arc::clone(&config),
            snapshot_rx.clone(),
            notebook_activity_tx.subscribe(),
            ingress_tx.clone(),
        ))
    });
    let mut spectral_task = config.spectral_enabled.then(|| {
        tokio::spawn(spectral::run(
            Arc::clone(&config),
            snapshot_rx.clone(),
            notebook_activity_tx.subscribe(),
        ))
    });
    let mut autonomy_task = config.autonomy_enabled.then(|| {
        tokio::spawn(autonomy::run(
            Arc::clone(&config),
            snapshot_rx.clone(),
            human_activity_rx.clone(),
            ingress_tx.clone(),
            action_outcome_rx,
            action_tx,
            Arc::clone(&autonomy_trace_registry),
            Arc::clone(&model_turn_lock),
            Arc::clone(&maintenance_work),
        ))
    });
    let mut scheduled_introspection_task = config.scheduled_introspection_enabled.then(|| {
        tokio::spawn(scheduled_introspection::run(
            Arc::clone(&config),
            snapshot_rx.clone(),
            human_activity_rx,
            ingress_tx,
            autonomy_trace_registry,
            model_turn_lock,
            maintenance_work,
        ))
    });

    eprintln!(
        "astrid-edge-runtime ready: telemetry=ws://{} sensory=ws://{} target_fill={:.0}% autonomy={}",
        config.telemetry_addr,
        config.sensory_addr,
        config.fill_target * 100.0,
        config.autonomy_enabled,
    );

    let critical_exit = tokio::select! {
        signal = tokio::signal::ctrl_c() => {
            signal?;
            None
        },
        result = &mut reservoir_task => {
            Some(critical_task_exit("reservoir", result))
        },
        result = &mut tuning_task => {
            Some(critical_task_exit("tuning manager", result))
        },
        result = &mut action_task => {
            Some(critical_task_exit("Action executor", result))
        },
        result = &mut scheduled_admission_task => {
            Some(critical_task_exit("scheduled introspection admission", result))
        },
        result = optional_task_exit(&mut maintenance_task) => {
            Some(critical_task_exit("maintenance acknowledgement", result))
        },
        result = optional_task_exit(&mut spectral_task) => {
            Some(critical_task_exit("spectral observer", result))
        },
        result = optional_task_exit(&mut autonomy_task) => {
            Some(critical_task_exit("autonomy scheduler", result))
        },
        result = optional_task_exit(&mut scheduled_introspection_task) => {
            Some(critical_task_exit("scheduled introspection scheduler", result))
        },
    };
    if let Some(error) = critical_exit.as_ref() {
        eprintln!("astrid-edge-runtime critical task failed: {error}");
    } else {
        eprintln!("astrid-edge-runtime shutting down");
    }

    let mut tasks = vec![
        reservoir_task,
        telemetry_task,
        sensory_task,
        host_task,
        ipc_task,
        action_task,
        inquiry_task,
        self_profile_task,
        peer_task,
        tuning_task,
        scheduled_admission_task,
    ];
    if let Some(task) = autonomy_task {
        tasks.push(task);
    }
    if let Some(task) = notebook_task {
        tasks.push(task);
    }
    if let Some(task) = spectral_task {
        tasks.push(task);
    }
    if let Some(task) = scheduled_introspection_task {
        tasks.push(task);
    }
    if let Some(task) = maintenance_task {
        tasks.push(task);
    }
    for task in tasks {
        task.abort();
    }
    critical_exit.map_or(Ok(()), Err)
}

fn critical_task_exit(
    task: &'static str,
    result: std::result::Result<(), tokio::task::JoinError>,
) -> anyhow::Error {
    match result {
        Ok(()) => anyhow::anyhow!("critical {task} task exited unexpectedly"),
        Err(error) => anyhow::anyhow!("critical {task} task failed: {error}"),
    }
}

async fn optional_task_exit(
    task: &mut Option<tokio::task::JoinHandle<()>>,
) -> std::result::Result<(), tokio::task::JoinError> {
    match task {
        Some(task) => task.await,
        None => std::future::pending().await,
    }
}
