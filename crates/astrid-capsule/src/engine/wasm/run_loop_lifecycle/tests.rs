use super::*;
use crate::engine::ExecutionEngine;
use crate::engine::wasm::{WasmEngine, build_wasmtime_engine, spawn_epoch_ticker};
use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
};
use std::time::{Duration, Instant};

#[test]
fn epoch_checks_continue_until_cancelled() {
    let engine = build_wasmtime_engine().unwrap();
    let module = wasmtime::Module::new(&engine, "(module (func (export \"run\")))").unwrap();
    let token = CancellationToken::new();
    let mut store = Store::new(&engine, ());
    configure_cancellation(&mut store, token.clone());
    let instance = wasmtime::Instance::new(&mut store, &module, &[]).unwrap();
    let run = instance
        .get_typed_func::<(), ()>(&mut store, "run")
        .unwrap();
    for _ in 0..100 {
        engine.increment_epoch();
        run.call(&mut store, ()).unwrap();
    }
    token.cancel();
    engine.increment_epoch();
    let error = run.call(&mut store, ()).unwrap_err();
    assert_eq!(
        error.downcast_ref::<wasmtime::Trap>(),
        Some(&wasmtime::Trap::Interrupt)
    );
}

#[test]
fn unload_stops_running_guests_and_runtime_exits() {
    const CHILD: &str = "ASTRID_WASM_UNLOAD_TEST_CHILD";
    if std::env::var_os(CHILD).is_some() {
        tokio::runtime::Builder::new_multi_thread()
            .worker_threads(2)
            .enable_all()
            .build()
            .unwrap()
            .block_on(async {
                exercise_unload(false).await;
                exercise_unload(true).await;
            });
        return;
    }
    let mut child = std::process::Command::new(std::env::current_exe().unwrap())
        .args(["--exact", "engine::wasm::run_loop_lifecycle::tests::unload_stops_running_guests_and_runtime_exits", "--nocapture"])
        .env(CHILD, "1")
        .spawn()
        .unwrap();
    let started = Instant::now();
    loop {
        if let Some(status) = child.try_wait().unwrap() {
            assert!(status.success());
            break;
        }
        if started.elapsed() > Duration::from_secs(20) {
            child.kill().unwrap();
            child.wait().unwrap();
            panic!("isolated guest unload/runtime exit deadline exceeded");
        }
        std::thread::sleep(Duration::from_millis(20));
    }
}

async fn exercise_unload(repeat_host_call: bool) {
    let engine = build_wasmtime_engine().unwrap();
    let token = CancellationToken::new();
    let entered = Arc::new(AtomicBool::new(false));
    let mut linker = wasmtime::Linker::new(&engine);
    let seen = entered.clone();
    let cancellation = token.clone();
    let runtime = tokio::runtime::Handle::current();
    let semaphore = Arc::new(tokio::sync::Semaphore::new(1));
    linker
        .func_wrap("host", "wait", move || {
            seen.store(true, Ordering::SeqCst);
            if repeat_host_call {
                // Model a guest ignoring the cancelled IPC result and trying again.
                let _ = crate::engine::wasm::host::util::bounded_block_on_cancellable(
                    &runtime,
                    &semaphore,
                    &cancellation,
                    std::future::pending::<()>(),
                );
            }
        })
        .unwrap();
    let loop_body = if repeat_host_call { "call $wait" } else { "" };
    let module = wasmtime::Module::new(
        &engine,
        format!(
            "(module (import \"host\" \"wait\" (func $wait))
            (func (export \"run\") call $wait (loop $again {loop_body} br $again)))"
        ),
    )
    .unwrap();
    let mut store = Store::new(&engine, ());
    configure_cancellation(&mut store, token.clone());
    let instance = linker.instantiate(&mut store, &module).unwrap();
    let run = instance
        .get_typed_func::<(), ()>(&mut store, "run")
        .unwrap();
    let manifest =
        toml::from_str("[package]\nname = 'shutdown-test'\nversion = '0.1.0'\n").unwrap();
    let mut capsule = WasmEngine::new(manifest, std::path::PathBuf::new());
    capsule.epoch_ticker = Some(spawn_epoch_ticker(&engine));
    capsule.wasmtime_engine = Some(engine);
    capsule.cancel_token = Some(token);
    let finished = Arc::new(AtomicBool::new(false));
    let done = finished.clone();
    capsule.run_handle = Some(tokio::spawn(async move {
        tokio::task::block_in_place(|| {
            let error = run.call(&mut store, ()).unwrap_err();
            assert_eq!(
                error.downcast_ref::<wasmtime::Trap>(),
                Some(&wasmtime::Trap::Interrupt)
            );
            done.store(true, Ordering::SeqCst);
        });
    }));
    let started = Instant::now();
    while !entered.load(Ordering::SeqCst) {
        assert!(started.elapsed() < Duration::from_secs(5));
        tokio::task::yield_now().await;
    }
    capsule.unload().await.unwrap();
    assert!(
        finished.load(Ordering::SeqCst),
        "unload must join, not detach"
    );
    assert!(capsule.run_handle.is_none());
    assert!(capsule.epoch_ticker.is_none());
    capsule.unload().await.unwrap();
}

#[tokio::test]
async fn timed_out_join_keeps_task_owned_for_retry() {
    let (release, wait) = tokio::sync::oneshot::channel();
    let mut handle = Some(tokio::spawn(async {
        let _ = wait.await;
    }));
    assert!(join(&mut handle).await.is_err());
    assert!(handle.is_some());
    release.send(()).unwrap();
    join(&mut handle).await.unwrap();
    assert!(handle.is_none());
}

#[tokio::test]
async fn failed_task_is_not_reported_as_successful_unload() {
    let mut handle = Some(tokio::spawn(async {
        panic!("synthetic guest failure");
    }));
    assert!(join(&mut handle).await.is_err());
    assert!(handle.is_none());
}
