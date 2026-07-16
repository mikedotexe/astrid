//! Compile-time authority states for the two bounded live microdose lanes.

use anyhow::{Result, anyhow};
use serde_json::Value;
use tokio::sync::mpsc;

use crate::types::SensoryMsg;

/// Untrusted evidence loaded from disk. This state cannot be dispatched.
#[derive(Debug)]
pub struct EvidenceOnly<T> {
    value: T,
}

/// A request whose persisted approval material is still being verified.
#[derive(Debug)]
pub struct ApprovalPending<T> {
    value: T,
}

/// Scope, token, expiry, consumption, budget, and lifecycle are verified.
#[derive(Debug)]
pub struct AuthorityGranted<T> {
    value: T,
}

/// Current safety and applicable rescue checks have also passed.
#[derive(Debug)]
pub struct LiveExecutable<T> {
    value: T,
}

impl<T> EvidenceOnly<T> {
    pub(crate) const fn from_untrusted(value: T) -> Self {
        Self { value }
    }

    pub(crate) fn into_pending(self) -> ApprovalPending<T> {
        ApprovalPending { value: self.value }
    }
}

impl<T> ApprovalPending<T> {
    pub(crate) const fn as_ref(&self) -> &T {
        &self.value
    }

    pub(crate) fn into_granted(self) -> AuthorityGranted<T> {
        AuthorityGranted { value: self.value }
    }
}

impl<T> AuthorityGranted<T> {
    pub(crate) const fn as_ref(&self) -> &T {
        &self.value
    }

    pub(crate) fn map<U>(self, map: impl FnOnce(T) -> U) -> AuthorityGranted<U> {
        AuthorityGranted {
            value: map(self.value),
        }
    }

    pub(crate) fn into_live(self) -> LiveExecutable<T> {
        LiveExecutable { value: self.value }
    }
}

/// A rescue-shaped semantic packet whose authority provenance is already checked.
#[derive(Debug)]
pub struct SemanticMicrodose {
    message: SensoryMsg,
    feature_len: usize,
}

impl SemanticMicrodose {
    pub(crate) const fn new(message: SensoryMsg, feature_len: usize) -> Self {
        Self {
            message,
            feature_len,
        }
    }
}

/// A bounded, one-shot mode-release control packet.
#[derive(Debug)]
pub struct ModeReleaseMicrodose {
    message: SensoryMsg,
    result_metadata: Value,
}

impl ModeReleaseMicrodose {
    pub(crate) const fn new(message: SensoryMsg, result_metadata: Value) -> Self {
        Self {
            message,
            result_metadata,
        }
    }
}

/// Dispatch the semantic lane. Earlier authority states are type-incompatible.
pub fn dispatch_semantic_microdose(
    executable: LiveExecutable<SemanticMicrodose>,
    sensory_tx: &mpsc::Sender<SensoryMsg>,
) -> Result<usize> {
    let microdose = executable.value;
    sensory_tx
        .try_send(microdose.message)
        .map_err(|err| anyhow!("semantic microdose send failed: {err}"))?;
    Ok(microdose.feature_len)
}

/// Dispatch the mode-release lane. Earlier authority states are type-incompatible.
pub fn dispatch_mode_release_microdose(
    executable: LiveExecutable<ModeReleaseMicrodose>,
    sensory_tx: &mpsc::Sender<SensoryMsg>,
) -> Result<Value> {
    let microdose = executable.value;
    sensory_tx
        .try_send(microdose.message)
        .map_err(|err| anyhow!("mode release microdose send failed: {err}"))?;
    Ok(microdose.result_metadata)
}
