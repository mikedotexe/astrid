//! Cancellation for synchronous guests running in a Tokio blocking section.

use crate::error::{CapsuleError, CapsuleResult};
use tokio_util::sync::CancellationToken;
use wasmtime::{Store, UpdateDeadline};

const UNLOAD_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(5);

pub(super) fn configure_cancellation<T: 'static>(store: &mut Store<T>, token: CancellationToken) {
    store.set_epoch_deadline(1);
    store.epoch_deadline_callback(move |_| {
        Ok(if token.is_cancelled() {
            UpdateDeadline::Interrupt
        } else {
            UpdateDeadline::Continue(1)
        })
    });
}

pub(super) async fn join(handle: &mut Option<tokio::task::JoinHandle<()>>) -> CapsuleResult<()> {
    let Some(running) = handle.as_mut() else {
        return Ok(());
    };
    let result = tokio::time::timeout(UNLOAD_TIMEOUT, running)
        .await
        .map_err(|_| {
            CapsuleError::WasmError("WASM run loop did not stop after cancellation".into())
        })?;
    // A timed-out task remains owned and may be joined on a later unload.
    *handle = None;
    result.map_err(|error| CapsuleError::WasmError(format!("WASM run loop join failed: {error}")))
}

#[cfg(test)]
mod tests;
