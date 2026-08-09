//! Reusable, authority-free CPU-edge domain and source-broker APIs.
//!
//! Deployment authority remains outside this crate. The native runtime and
//! immutable steward consume these typed contracts without gaining ambient
//! shell, network, service, or host access from the library itself.

#![deny(unsafe_code)]

pub mod self_change;
pub mod source_tools;
