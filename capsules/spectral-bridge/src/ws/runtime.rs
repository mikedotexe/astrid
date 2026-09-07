//! Runtime orchestration for Minime telemetry and sensory WebSocket lanes.
//!
//! Two persistent connections:
//! - **Telemetry** (port 7878): subscribes to spectral eigenvalue broadcasts.
//! - **Sensory** (port 7879): sends control/semantic features to Minime.
//!
//! Both connections auto-reconnect with exponential backoff on failure. The
//! implementation is assembled from behavior-preserving ownership units.

#![allow(dead_code)]

#[path = "sensory_delivery.rs"]
mod sensory_delivery;
pub use sensory_delivery::{AddressedSensoryMessage, AddressedSensorySender};
use sensory_delivery::{
    PendingSelfControlReceiptV2, PendingSensoryDeliveryV1, apply_delivery_receipt,
    apply_self_control_receipt, apply_server_hello, encode_sensory_packet_v1,
    record_pending_delivery, record_unknown_deliveries, record_unknown_self_control_receipts,
    unix_now_ms, validate_negotiated_extension_v2,
};
include!("bridge_state.rs");
include!("health.rs");
include!("compatibility_projection.rs");
include!("evidence.rs");
include!("health_trace.rs");
include!("telemetry_port.rs");
include!("sensory_port.rs");
include!("tests.rs");
#[cfg(test)]
#[path = "learning_clock_tests.rs"]
mod learning_clock_tests;
