//! Immutable, read-only sampling of the candidate runtime's loopback telemetry.

use std::io::{Read, Write};
use std::net::TcpStream;
use std::time::Duration;

use serde_json::Value;

use crate::config::{AudioPolicy, Config};
use crate::fs_guard::{canonical_json, sha256};
use crate::health::{
    LiveAudioHealth, LiveAuxiliaryHealth, LiveStateCorrelation, LiveTelemetryHealth,
};
use crate::{Error, Result};

#[derive(Debug)]
struct ParsedTelemetry {
    protocol_major: u64,
    protocol_minor: u64,
    t_ms: u64,
    fill_ratio: f64,
    snapshot_generation_id: String,
    audio_fresh: bool,
    audio_source: String,
    audio_rms: f64,
    modality_audio_source: Option<String>,
    audio_class: Option<String>,
    aux_fresh: bool,
    aux_source: String,
    memory_used: f64,
    thermal_normalized: f64,
    network_unavailable: bool,
}

pub(crate) struct RuntimeStateReference<'a> {
    pub generation_id: &'a str,
    pub fill_ratio: f64,
}

pub(crate) struct DirectHostSample {
    pub total_ram_bytes: u64,
    pub available_ram_bytes: u64,
    pub thermal_celsius: f64,
}

pub(crate) fn observe_live_telemetry(
    config: &Config,
    state: &RuntimeStateReference<'_>,
    host: &DirectHostSample,
) -> Result<LiveTelemetryHealth> {
    let value = read_live_value(config)?;
    let parsed = parse_live_value(&value)?;
    evaluate_live_value(config, &value, parsed, state, host)
}

fn read_live_value(config: &Config) -> Result<Value> {
    let mut stream =
        TcpStream::connect_timeout(&config.health.telemetry_addr, Duration::from_secs(3))?;
    stream.set_read_timeout(Some(Duration::from_secs(5)))?;
    stream.set_write_timeout(Some(Duration::from_secs(5)))?;
    let request = format!(
        "GET / HTTP/1.1\r\nHost: {}\r\nUpgrade: websocket\r\nConnection: Upgrade\r\nSec-WebSocket-Key: dGhlIHNhbXBsZSBub25jZQ==\r\nSec-WebSocket-Version: 13\r\n\r\n",
        config.health.telemetry_addr
    );
    stream.write_all(request.as_bytes())?;
    stream.flush()?;
    read_websocket_handshake(&mut stream)?;
    let payload = read_websocket_text(&mut stream, 128 * 1024)?;
    serde_json::from_slice(&payload).map_err(Into::into)
}

fn parse_live_value(value: &Value) -> Result<ParsedTelemetry> {
    let protocol_major = live_u64(value, "/protocol/major")?;
    let protocol_minor = live_u64(value, "/protocol/minor")?;
    if value.pointer("/protocol/name").and_then(Value::as_str) != Some("astrid_minime")
        || protocol_major != 1
        || protocol_minor != 0
        || value
            .pointer("/spectral_substrate_v1/substrate_kind")
            .and_then(Value::as_str)
            != Some("cpu_edge_covariance_effective_rank")
    {
        return Err(Error::new(
            "live telemetry protocol or spectral substrate is not exact",
        ));
    }
    let extension = value
        .get("edge_runtime_v1")
        .ok_or_else(|| Error::new("live edge telemetry extension is absent"))?;
    if extension.get("kind").and_then(Value::as_str) != Some("cpu_effective_rank_esn")
        || extension.get("reservoir_dim").and_then(Value::as_u64) != Some(128)
    {
        return Err(Error::new("live edge telemetry identity is not exact"));
    }
    Ok(ParsedTelemetry {
        protocol_major,
        protocol_minor,
        t_ms: live_u64(value, "/t_ms")?,
        fill_ratio: live_f64(value, "/fill_ratio", 0.0, 1.0)?,
        snapshot_generation_id: live_string(extension, "/snapshot_generation_id")?,
        audio_fresh: live_bool(extension, "/audio_fresh")?,
        audio_source: live_string(extension, "/audio_source")?,
        audio_rms: live_f64(value, "/modalities/audio_rms", 0.0, 1.0)?,
        modality_audio_source: optional_live_string(value, "/modalities/audio_source")?,
        audio_class: optional_live_string(value, "/modalities/audio_freshness_class")?,
        aux_fresh: live_bool(extension, "/aux_fresh")?,
        aux_source: live_string(extension, "/aux_source")?,
        memory_used: live_f64(extension, "/aux_features/memory_used", 0.0, 1.0)?,
        thermal_normalized: live_f64(extension, "/aux_features/thermal_normalized", 0.0, 1.0)?,
        network_unavailable: ["network_receive_rate", "network_transmit_rate"]
            .iter()
            .all(|name| {
                extension
                    .pointer(&format!("/aux_features/{name}"))
                    .is_some_and(Value::is_null)
            }),
    })
}

fn evaluate_live_value(
    config: &Config,
    value: &Value,
    parsed: ParsedTelemetry,
    state: &RuntimeStateReference<'_>,
    host: &DirectHostSample,
) -> Result<LiveTelemetryHealth> {
    if host.total_ram_bytes == 0 || host.available_ram_bytes > host.total_ram_bytes {
        return Err(Error::new("direct host memory sample is malformed"));
    }
    #[allow(clippy::cast_precision_loss)]
    let direct_memory_used = 1.0 - host.available_ram_bytes as f64 / host.total_ram_bytes as f64;
    let direct_thermal_normalized = ((host.thermal_celsius - 20.0) / 80.0).clamp(0.0, 1.0);
    let host_memory_delta = (parsed.memory_used - direct_memory_used).abs();
    let host_thermal_delta = (parsed.thermal_normalized - direct_thermal_normalized).abs();
    let host_aux_crosscheck_verified = parsed.aux_fresh
        && parsed.aux_source
            == "linux_proc_sys_host_cpu_memory_load_disk_thermal_clock_network_unavailable_private_namespace"
        && parsed.network_unavailable
        && host_memory_delta <= 0.10
        && host_thermal_delta <= 0.10;
    let audio_policy_verified = verify_audio_policy(config, &parsed);
    Ok(LiveTelemetryHealth {
        endpoint: config.health.telemetry_addr.to_string(),
        protocol_major: parsed.protocol_major,
        protocol_minor: parsed.protocol_minor,
        t_ms: parsed.t_ms,
        fill_ratio: parsed.fill_ratio,
        snapshot_generation_id: parsed.snapshot_generation_id.clone(),
        state_correlation: LiveStateCorrelation {
            generation_matches: parsed.snapshot_generation_id == state.generation_id,
            fill_delta: (parsed.fill_ratio - state.fill_ratio).abs(),
        },
        audio: LiveAudioHealth {
            audio_fresh: parsed.audio_fresh,
            audio_source: parsed.audio_source,
            audio_rms: parsed.audio_rms,
            audio_policy_verified,
        },
        auxiliary: LiveAuxiliaryHealth {
            aux_fresh: parsed.aux_fresh,
            aux_source: parsed.aux_source,
            host_memory_delta,
            host_thermal_delta,
            host_aux_crosscheck_verified,
        },
        observed_sha256: sha256(&canonical_json(value)?),
        authority: "immutable_root_live_loopback_observation_candidate_output_corroborative_only",
    })
}

fn verify_audio_policy(config: &Config, parsed: &ParsedTelemetry) -> bool {
    match config.health.audio_policy {
        AudioPolicy::RequiredFreshNumeric => {
            parsed.audio_fresh
                && parsed.audio_source == config.health.expected_audio_source
                && parsed.modality_audio_source.as_deref() == Some(parsed.audio_source.as_str())
                && parsed.audio_class.as_deref() == Some("fresh")
                && parsed.audio_rms.is_finite()
        },
        AudioPolicy::RequiredUnavailable => {
            !parsed.audio_fresh
                && parsed.audio_source == config.health.expected_audio_source
                && parsed.modality_audio_source.as_deref() == Some(parsed.audio_source.as_str())
                && parsed.audio_class.as_deref() == Some("unavailable")
                && parsed.audio_rms == 0.0
        },
    }
}

fn live_u64(value: &Value, pointer: &str) -> Result<u64> {
    value
        .pointer(pointer)
        .and_then(Value::as_u64)
        .ok_or_else(|| Error::new(format!("live telemetry integer is absent: {pointer}")))
}

fn live_f64(value: &Value, pointer: &str, minimum: f64, maximum: f64) -> Result<f64> {
    value
        .pointer(pointer)
        .and_then(Value::as_f64)
        .filter(|number| number.is_finite() && (minimum..=maximum).contains(number))
        .ok_or_else(|| Error::new(format!("live telemetry number is invalid: {pointer}")))
}

fn live_bool(value: &Value, pointer: &str) -> Result<bool> {
    value
        .pointer(pointer)
        .and_then(Value::as_bool)
        .ok_or_else(|| Error::new(format!("live telemetry boolean is absent: {pointer}")))
}

fn live_string(value: &Value, pointer: &str) -> Result<String> {
    optional_live_string(value, pointer)?
        .ok_or_else(|| Error::new(format!("live telemetry string is absent: {pointer}")))
}

fn optional_live_string(value: &Value, pointer: &str) -> Result<Option<String>> {
    let Some(text) = value.pointer(pointer).and_then(Value::as_str) else {
        return Ok(None);
    };
    if text.is_empty() || text.len() > 256 || text.chars().any(char::is_control) {
        return Err(Error::new(format!(
            "live telemetry string is invalid: {pointer}"
        )));
    }
    Ok(Some(text.to_owned()))
}

fn read_websocket_handshake(stream: &mut TcpStream) -> Result<()> {
    let mut response = Vec::new();
    let mut byte = [0_u8; 1];
    while !response.ends_with(b"\r\n\r\n") && response.len() < 8 * 1024 {
        stream.read_exact(&mut byte)?;
        response.push(byte[0]);
    }
    let text = String::from_utf8(response)
        .map_err(|_| Error::new("live telemetry handshake is not UTF-8"))?;
    let lower = text.to_ascii_lowercase();
    if !text.starts_with("HTTP/1.1 101 ")
        || !lower.contains("upgrade: websocket")
        || !lower.contains("sec-websocket-accept: s3pplmbitxaq9kygzzhzrbk+xoo=")
    {
        return Err(Error::new("live telemetry WebSocket handshake failed"));
    }
    Ok(())
}

fn read_websocket_text(stream: &mut TcpStream, maximum: usize) -> Result<Vec<u8>> {
    let mut header = [0_u8; 2];
    stream.read_exact(&mut header)?;
    if header[0] != 0x81 || header[1] & 0x80 != 0 {
        return Err(Error::new(
            "live telemetry frame is not final unmasked text",
        ));
    }
    let mut length = usize::from(header[1] & 0x7f);
    if length == 126 {
        let mut extended = [0_u8; 2];
        stream.read_exact(&mut extended)?;
        length = usize::from(u16::from_be_bytes(extended));
    } else if length == 127 {
        let mut extended = [0_u8; 8];
        stream.read_exact(&mut extended)?;
        length = usize::try_from(u64::from_be_bytes(extended))
            .map_err(|_| Error::new("live telemetry frame length overflow"))?;
    }
    if length == 0 || length > maximum {
        return Err(Error::new("live telemetry frame exceeds immutable bound"));
    }
    let mut payload = vec![0_u8; length];
    stream.read_exact(&mut payload)?;
    Ok(payload)
}

#[cfg(test)]
mod tests {
    use super::parse_live_value;
    use serde_json::json;

    fn packet() -> serde_json::Value {
        json!({
            "protocol": {"name":"astrid_minime","major":1,"minor":0},
            "t_ms": 1234,
            "fill_ratio": 0.68,
            "spectral_substrate_v1": {
                "substrate_kind":"cpu_edge_covariance_effective_rank"
            },
            "modalities": {
                "audio_rms": 0.2,
                "audio_source":"physical_alsa:default:16000hz:1ch",
                "audio_freshness_class":"fresh"
            },
            "edge_runtime_v1": {
                "kind":"cpu_effective_rank_esn",
                "reservoir_dim":128,
                "snapshot_generation_id":"reservoir-1",
                "audio_fresh":true,
                "audio_source":"physical_alsa:default:16000hz:1ch",
                "aux_fresh":true,
                "aux_source":"linux_proc_sys_host_cpu_memory_load_disk_thermal_clock_network_unavailable_private_namespace",
                "aux_features":{
                    "memory_used":0.5,
                    "thermal_normalized":0.25,
                    "network_receive_rate":null,
                    "network_transmit_rate":null
                }
            }
        })
    }

    #[test]
    fn live_packet_parser_binds_actual_edge_wire_fields() {
        let parsed = parse_live_value(&packet()).unwrap();
        assert_eq!(parsed.protocol_minor, 0);
        assert_eq!(parsed.snapshot_generation_id, "reservoir-1");
        assert!((parsed.thermal_normalized - 0.25).abs() < f64::EPSILON);
    }

    #[test]
    fn live_packet_parser_rejects_control_protocol_minor_and_wrong_substrate() {
        let mut value = packet();
        value["protocol"]["minor"] = json!(3);
        assert!(parse_live_value(&value).is_err());
        let mut value = packet();
        value["spectral_substrate_v1"]["substrate_kind"] = json!("minime_thresholded_eigenfill");
        assert!(parse_live_value(&value).is_err());
    }

    #[test]
    fn private_network_provenance_requires_both_network_lanes_unavailable() {
        let parsed = parse_live_value(&packet()).unwrap();
        assert!(parsed.network_unavailable);

        let mut value = packet();
        value["edge_runtime_v1"]["aux_features"]["network_receive_rate"] = json!(0.25);
        let parsed = parse_live_value(&value).unwrap();
        assert!(!parsed.network_unavailable);
    }
}
