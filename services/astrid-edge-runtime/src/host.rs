use std::{
    f32::consts::PI,
    fs,
    io::{BufRead as _, BufReader, ErrorKind, Read as _},
    os::unix::net::UnixStream,
    path::Path,
    thread,
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};

use serde::Deserialize;
use tokio::sync::mpsc;

use crate::reservoir::SensoryIngress;

const AUDIO_FEATURE_DIM: usize = 8;
const AUDIO_FEEDER_SOCKET: &str = "/run/astrid-edge-self-change/audio-features.sock";
const AUDIO_FEEDER_SCHEMA: &str = "astrid.edge.audio_features.v1";
const AUDIO_FEEDER_SOURCE: &str = "physical_alsa_numeric_feeder";
const AUDIO_FEEDER_MAXIMUM_LINE_BYTES: u64 = 1_024;

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct AudioFeatureFrame {
    schema: String,
    sequence: u64,
    sample_rate: u32,
    channels: u16,
    device: String,
    source: String,
    features: Vec<f32>,
}

#[derive(Clone, Copy, Default)]
struct CpuSample {
    total: u64,
    idle: u64,
}

#[derive(Clone, Copy, Default)]
struct IoSample {
    disk: Option<CounterPair>,
    network: Option<CounterPair>,
}

#[derive(Clone, Copy, Default)]
struct CounterPair {
    first: u64,
    second: u64,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum NetworkPolicy {
    PhysicalDeviceOnly,
    UnavailablePrivateNetwork,
    UnavailableInvalidPolicy,
}

impl NetworkPolicy {
    fn from_environment() -> Self {
        match std::env::var("ASTRID_EDGE_HOST_NETWORK_POLICY") {
            Err(std::env::VarError::NotPresent) => Self::PhysicalDeviceOnly,
            Ok(value) if value == "physical_device_only" => Self::PhysicalDeviceOnly,
            Ok(value) if value == "unavailable_private_network" => Self::UnavailablePrivateNetwork,
            Ok(_) | Err(std::env::VarError::NotUnicode(_)) => Self::UnavailableInvalidPolicy,
        }
    }

    const fn source(self, network_available: bool) -> &'static str {
        match (self, network_available) {
            (Self::PhysicalDeviceOnly, true) => {
                "linux_proc_sys_host_cpu_memory_load_disk_thermal_clock_network_physical_device"
            },
            (Self::PhysicalDeviceOnly, false) => {
                "linux_proc_sys_host_cpu_memory_load_disk_thermal_clock_network_unavailable_no_physical_device"
            },
            (Self::UnavailablePrivateNetwork, _) => {
                "linux_proc_sys_host_cpu_memory_load_disk_thermal_clock_network_unavailable_private_namespace"
            },
            (Self::UnavailableInvalidPolicy, _) => {
                "linux_proc_sys_host_cpu_memory_load_disk_thermal_clock_network_unavailable_invalid_policy"
            },
        }
    }

    const fn permits_proc_network(self) -> bool {
        matches!(self, Self::PhysicalDeviceOnly)
    }
}

const DISK_RATE_SCALE_BYTES_PER_SECOND: f32 = 4.0 * 1024.0 * 1024.0;
const NETWORK_RATE_SCALE_BYTES_PER_SECOND: f32 = 1024.0 * 1024.0;

pub async fn run(ingress_tx: mpsc::Sender<SensoryIngress>) {
    let audio_tx = ingress_tx.clone();
    if let Err(error) = thread::Builder::new()
        .name("astrid-edge-audio".to_string())
        .spawn(move || audio_capture_loop(&audio_tx))
    {
        eprintln!("edge physical audio thread unavailable: {error}");
    }
    run_interoception(ingress_tx).await;
}

#[allow(clippy::cast_precision_loss)] // /proc counters become bounded ratios.
async fn run_interoception(ingress_tx: mpsc::Sender<SensoryIngress>) {
    let network_policy = NetworkPolicy::from_environment();
    let mut previous_cpu = read_cpu().unwrap_or_default();
    let mut previous_io = read_io(network_policy);
    let mut previous_io_at = Instant::now();
    let mut interval = tokio::time::interval(Duration::from_millis(500));

    loop {
        interval.tick().await;
        let current_cpu = read_cpu();
        let cpu_busy = current_cpu.map_or(0.0, |sample| {
            let total_delta = sample.total.saturating_sub(previous_cpu.total);
            let idle_delta = sample.idle.saturating_sub(previous_cpu.idle);
            previous_cpu = sample;
            if total_delta == 0 {
                0.0
            } else {
                1.0 - (idle_delta as f32 / total_delta as f32)
            }
        });
        let memory_used = read_memory_used();
        let load = read_normalized_load();
        let current_io = read_io(network_policy);
        let now = Instant::now();
        let elapsed = now.saturating_duration_since(previous_io_at);
        let (disk_read, disk_write) = normalized_pair_rate(
            current_io.disk,
            previous_io.disk,
            elapsed,
            DISK_RATE_SCALE_BYTES_PER_SECOND,
        );
        let (network_receive, network_transmit) = normalized_pair_rate(
            current_io.network,
            previous_io.network,
            elapsed,
            NETWORK_RATE_SCALE_BYTES_PER_SECOND,
        );
        previous_io = current_io;
        previous_io_at = now;
        let thermal = read_thermal_normalized();
        let (daily_sine, daily_cosine) = daily_phase();
        let disk_available = current_io.disk.is_some();
        let network_available = current_io.network.is_some();
        let availability = vec![
            current_cpu.is_some(),
            memory_used.is_some(),
            load.is_some(),
            disk_available,
            disk_available,
            network_available,
            network_available,
            thermal.is_some(),
            true,
            true,
        ];
        let features = vec![
            cpu_busy.clamp(0.0, 1.0),
            memory_used.unwrap_or_default().clamp(0.0, 1.0),
            load.unwrap_or_default().clamp(0.0, 1.0),
            disk_read,
            disk_write,
            network_receive,
            network_transmit,
            thermal.unwrap_or_default(),
            daily_sine,
            daily_cosine,
        ];
        if ingress_tx
            .send(SensoryIngress::Aux {
                features,
                source: network_policy.source(network_available).to_string(),
                availability: Some(availability),
            })
            .await
            .is_err()
        {
            return;
        }
    }
}

fn audio_capture_loop(ingress_tx: &mpsc::Sender<SensoryIngress>) {
    let device =
        std::env::var("ASTRID_EDGE_AUDIO_DEVICE").unwrap_or_else(|_| "default".to_string());
    if matches!(device.trim().to_ascii_lowercase().as_str(), "off" | "none") {
        eprintln!("edge physical audio disabled by ASTRID_EDGE_AUDIO_DEVICE={device}");
        return;
    }
    let sample_rate =
        environment_u32("ASTRID_EDGE_AUDIO_SAMPLE_RATE", 16_000).clamp(8_000, 192_000);
    let channels = environment_u16("ASTRID_EDGE_AUDIO_CHANNELS", 1).clamp(1, 8);
    let Some(expected_peer_uid) = std::env::var("ASTRID_EDGE_AUDIO_FEEDER_UID")
        .ok()
        .and_then(|value| value.parse::<u32>().ok())
        .filter(|uid| *uid > 0)
    else {
        eprintln!("edge physical audio disabled: immutable feeder UID is absent");
        return;
    };
    let source = format!("{AUDIO_FEEDER_SOURCE}:{device}:{sample_rate}hz:{channels}ch");

    loop {
        match receive_audio_features(
            ingress_tx,
            expected_peer_uid,
            sample_rate,
            channels,
            &source,
        ) {
            Ok(()) => return,
            Err(error) => {
                eprintln!(
                    "edge physical audio feeder unavailable ({source}): {error}; retrying in 10s"
                );
                thread::sleep(Duration::from_secs(10));
            },
        }
    }
}

fn receive_audio_features(
    ingress_tx: &mpsc::Sender<SensoryIngress>,
    expected_peer_uid: u32,
    sample_rate: u32,
    channels: u16,
    source: &str,
) -> std::io::Result<()> {
    let stream = UnixStream::connect(AUDIO_FEEDER_SOCKET)?;
    stream.set_read_timeout(Some(Duration::from_secs(3)))?;
    validate_audio_peer(&stream, expected_peer_uid)?;
    let mut reader = BufReader::with_capacity(2_048, stream);
    let mut last_sequence = None;
    eprintln!("edge physical audio numeric feeder connected: {source}");

    loop {
        let mut line = Vec::new();
        let read = match std::io::Read::by_ref(&mut reader)
            .take(AUDIO_FEEDER_MAXIMUM_LINE_BYTES.saturating_add(1))
            .read_until(b'\n', &mut line)
        {
            Ok(read) => read,
            Err(error) if error.kind() == ErrorKind::Interrupted => continue,
            Err(error) => return Err(error),
        };
        if read == 0 {
            return Err(std::io::Error::new(
                ErrorKind::UnexpectedEof,
                "audio feeder closed the feature stream",
            ));
        }
        if u64::try_from(line.len()).unwrap_or(u64::MAX) > AUDIO_FEEDER_MAXIMUM_LINE_BYTES
            || !line.ends_with(b"\n")
        {
            return Err(std::io::Error::new(
                ErrorKind::InvalidData,
                "audio feeder frame exceeded the immutable line bound",
            ));
        }
        line.pop();
        let frame: AudioFeatureFrame = serde_json::from_slice(&line)
            .map_err(|error| std::io::Error::new(ErrorKind::InvalidData, error))?;
        validate_audio_frame(
            &frame,
            last_sequence,
            &device_from_source(source)?,
            sample_rate,
            channels,
        )?;
        last_sequence = Some(frame.sequence);
        if ingress_tx
            .blocking_send(SensoryIngress::Audio {
                features: frame.features,
                source: source.to_string(),
            })
            .is_err()
        {
            return Ok(());
        }
    }
}

fn validate_audio_frame(
    frame: &AudioFeatureFrame,
    last_sequence: Option<u64>,
    device: &str,
    sample_rate: u32,
    channels: u16,
) -> std::io::Result<()> {
    if frame.schema != AUDIO_FEEDER_SCHEMA
        || frame.source != AUDIO_FEEDER_SOURCE
        || frame.device != device
        || frame.sample_rate != sample_rate
        || frame.channels != channels
        || frame.features.len() != AUDIO_FEATURE_DIM
        || frame
            .features
            .iter()
            .any(|value| !value.is_finite() || !(-1.0..=1.0).contains(value))
        || last_sequence.is_some_and(|previous| frame.sequence <= previous)
    {
        return Err(std::io::Error::new(
            ErrorKind::InvalidData,
            "audio feeder frame failed the exact numeric contract",
        ));
    }
    Ok(())
}

fn device_from_source(source: &str) -> std::io::Result<String> {
    let prefix = format!("{AUDIO_FEEDER_SOURCE}:");
    source
        .strip_prefix(&prefix)
        .and_then(|value| value.rsplit_once(':'))
        .and_then(|(value, _channels)| value.rsplit_once(':'))
        .map(|(device, _rate)| device.to_owned())
        .filter(|device| !device.is_empty())
        .ok_or_else(|| {
            std::io::Error::new(
                ErrorKind::InvalidInput,
                "audio source did not preserve the configured device",
            )
        })
}

#[cfg(target_os = "linux")]
fn validate_audio_peer(stream: &UnixStream, expected_uid: u32) -> std::io::Result<()> {
    let credentials =
        nix::sys::socket::getsockopt(stream, nix::sys::socket::sockopt::PeerCredentials)
            .map_err(std::io::Error::other)?;
    if credentials.uid() != expected_uid || credentials.pid() <= 0 {
        return Err(std::io::Error::new(
            ErrorKind::PermissionDenied,
            "audio feeder Unix peer identity is unauthorized",
        ));
    }
    Ok(())
}

#[cfg(not(target_os = "linux"))]
fn validate_audio_peer(_stream: &UnixStream, _expected_uid: u32) -> std::io::Result<()> {
    Err(std::io::Error::new(
        ErrorKind::Unsupported,
        "audio feeder peer credentials require Linux",
    ))
}

fn environment_u32(name: &str, fallback: u32) -> u32 {
    std::env::var(name)
        .ok()
        .and_then(|value| value.parse().ok())
        .unwrap_or(fallback)
}

fn environment_u16(name: &str, fallback: u16) -> u16 {
    std::env::var(name)
        .ok()
        .and_then(|value| value.parse().ok())
        .unwrap_or(fallback)
}

fn read_cpu() -> Option<CpuSample> {
    let content = fs::read_to_string("/proc/stat").ok()?;
    let line = content.lines().find(|line| line.starts_with("cpu "))?;
    let values = line
        .split_whitespace()
        .skip(1)
        .filter_map(|value| value.parse::<u64>().ok())
        .collect::<Vec<_>>();
    if values.len() < 4 {
        return None;
    }
    let total = values.iter().copied().fold(0_u64, u64::saturating_add);
    let idle = values[3].saturating_add(values.get(4).copied().unwrap_or(0));
    Some(CpuSample { total, idle })
}

#[allow(clippy::cast_precision_loss)]
fn read_memory_used() -> Option<f32> {
    let content = fs::read_to_string("/proc/meminfo").ok()?;
    let mut total_kib = None;
    let mut available_kib = None;
    for line in content.lines() {
        let mut fields = line.split_whitespace();
        match fields.next()? {
            "MemTotal:" => total_kib = fields.next().and_then(|value| value.parse::<u64>().ok()),
            "MemAvailable:" => {
                available_kib = fields.next().and_then(|value| value.parse::<u64>().ok());
            },
            _ => {},
        }
    }
    let total = total_kib?;
    let available = available_kib?;
    if total == 0 {
        return None;
    }
    Some(1.0 - available as f32 / total as f32)
}

#[allow(clippy::cast_precision_loss)]
fn read_normalized_load() -> Option<f32> {
    let content = fs::read_to_string("/proc/loadavg").ok()?;
    let load = content.split_whitespace().next()?.parse::<f32>().ok()?;
    let processors = std::thread::available_parallelism().ok()?.get() as f32;
    Some((load / processors.max(1.0)).clamp(0.0, 1.0))
}

fn read_io(network_policy: NetworkPolicy) -> IoSample {
    let disk = fs::read_to_string("/proc/diskstats")
        .ok()
        .and_then(|content| {
            parse_disk_counters(&content, |name| Path::new("/sys/block").join(name).is_dir())
        })
        .map(|(first, second)| CounterPair { first, second });
    let network = network_policy.permits_proc_network().then(|| {
        fs::read_to_string("/proc/net/dev")
            .ok()
            .and_then(|content| {
                parse_network_counters(&content, |name| {
                    Path::new("/sys/class/net")
                        .join(name)
                        .join("device")
                        .exists()
                })
            })
            .map(|(first, second)| CounterPair { first, second })
    });
    IoSample {
        disk,
        network: network.flatten(),
    }
}

fn parse_disk_counters(
    content: &str,
    is_whole_device: impl Fn(&str) -> bool,
) -> Option<(u64, u64)> {
    let mut found = false;
    let mut read_sectors = 0_u64;
    let mut write_sectors = 0_u64;
    for line in content.lines() {
        let fields = line.split_whitespace().collect::<Vec<_>>();
        if fields.len() < 10 || !is_whole_device(fields[2]) {
            continue;
        }
        let (Ok(read), Ok(write)) = (fields[5].parse::<u64>(), fields[9].parse::<u64>()) else {
            continue;
        };
        found = true;
        read_sectors = read_sectors.saturating_add(read);
        write_sectors = write_sectors.saturating_add(write);
    }
    found.then(|| {
        (
            read_sectors.saturating_mul(512),
            write_sectors.saturating_mul(512),
        )
    })
}

fn parse_network_counters(
    content: &str,
    is_physical_device: impl Fn(&str) -> bool,
) -> Option<(u64, u64)> {
    let mut found = false;
    let mut received = 0_u64;
    let mut transmitted = 0_u64;
    for line in content.lines() {
        let Some((interface, counters)) = line.split_once(':') else {
            continue;
        };
        let interface = interface.trim();
        if interface == "lo" || !is_physical_device(interface) {
            continue;
        }
        let fields = counters.split_whitespace().collect::<Vec<_>>();
        if fields.len() < 16 {
            continue;
        }
        let (Ok(rx), Ok(tx)) = (fields[0].parse::<u64>(), fields[8].parse::<u64>()) else {
            continue;
        };
        found = true;
        received = received.saturating_add(rx);
        transmitted = transmitted.saturating_add(tx);
    }
    found.then_some((received, transmitted))
}

fn normalized_pair_rate(
    current: Option<CounterPair>,
    previous: Option<CounterPair>,
    elapsed: Duration,
    scale: f32,
) -> (f32, f32) {
    let (Some(current), Some(previous)) = (current, previous) else {
        return (0.0, 0.0);
    };
    (
        normalize_counter_rate(current.first.saturating_sub(previous.first), elapsed, scale),
        normalize_counter_rate(
            current.second.saturating_sub(previous.second),
            elapsed,
            scale,
        ),
    )
}

#[allow(clippy::cast_precision_loss)]
fn normalize_counter_rate(delta: u64, elapsed: Duration, scale: f32) -> f32 {
    let seconds = elapsed.as_secs_f32().max(0.001);
    let rate = delta as f32 / seconds;
    (rate / (rate + scale)).clamp(0.0, 1.0)
}

fn read_thermal_normalized() -> Option<f32> {
    let entries = fs::read_dir("/sys/class/thermal").ok()?;
    let maximum_millidegrees = entries
        .flatten()
        .filter_map(|entry| fs::read_to_string(entry.path().join("temp")).ok())
        .filter_map(|value| value.trim().parse::<f32>().ok())
        .filter(|value| value.is_finite())
        .max_by(f32::total_cmp)?;
    let celsius = if maximum_millidegrees > 1_000.0 {
        maximum_millidegrees / 1_000.0
    } else {
        maximum_millidegrees
    };
    Some(((celsius - 20.0) / 80.0).clamp(0.0, 1.0))
}

#[allow(clippy::cast_precision_loss)]
fn daily_phase() -> (f32, f32) {
    let seconds = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
        % 86_400;
    let phase = 2.0 * PI * seconds as f32 / 86_400.0;
    (phase.sin(), phase.cos())
}

#[cfg(test)]
mod tests {
    use super::{
        AUDIO_FEATURE_DIM, AUDIO_FEEDER_SCHEMA, AUDIO_FEEDER_SOURCE, AudioFeatureFrame,
        CounterPair, NetworkPolicy, device_from_source, normalize_counter_rate,
        normalized_pair_rate, parse_disk_counters, parse_network_counters, validate_audio_frame,
    };
    use std::time::Duration;

    #[test]
    fn audio_feature_frames_are_exact_bounded_and_monotonic() {
        let frame = AudioFeatureFrame {
            schema: AUDIO_FEEDER_SCHEMA.to_owned(),
            sequence: 2,
            sample_rate: 16_000,
            channels: 1,
            device: "hw:1,0".to_owned(),
            source: AUDIO_FEEDER_SOURCE.to_owned(),
            features: vec![0.25; AUDIO_FEATURE_DIM],
        };
        assert!(validate_audio_frame(&frame, Some(1), "hw:1,0", 16_000, 1).is_ok());
        assert!(validate_audio_frame(&frame, Some(2), "hw:1,0", 16_000, 1).is_err());
        assert_eq!(
            device_from_source("physical_alsa_numeric_feeder:hw:1,0:16000hz:1ch").unwrap(),
            "hw:1,0"
        );

        let mut malformed = frame;
        malformed.features[0] = f32::NAN;
        assert!(validate_audio_frame(&malformed, None, "hw:1,0", 16_000, 1).is_err());
    }

    #[test]
    fn disk_counters_include_only_resolved_whole_devices() {
        let fixture = "\
           8       0 sda 10 0 20 0 30 0 40 0 0 0 0 0 0 0 0 0\n\
           8       1 sda1 50 0 60 0 70 0 80 0 0 0 0 0 0 0 0 0\n\
         179       0 mmcblk0 1 0 2 0 3 0 4 0 0 0 0 0 0 0 0 0";
        let counters =
            parse_disk_counters(fixture, |name| matches!(name, "sda" | "mmcblk0")).unwrap();
        assert_eq!(counters, (22 * 512, 44 * 512));
    }

    #[test]
    fn network_counters_exclude_loopback_and_sum_physical_interfaces() {
        let fixture = "\
Inter-| Receive | Transmit\n\
 lo: 100 0 0 0 0 0 0 0 200 0 0 0 0 0 0 0\n\
eth0: 300 0 0 0 0 0 0 0 400 0 0 0 0 0 0 0\n\
wlan0: 500 0 0 0 0 0 0 0 600 0 0 0 0 0 0 0";
        assert_eq!(
            parse_network_counters(fixture, |name| matches!(name, "eth0" | "wlan0")),
            Some((800, 1_000))
        );
        assert_eq!(parse_network_counters(fixture, |_| false), None);
    }

    #[test]
    fn private_network_is_explicitly_unavailable_and_independent_of_disk() {
        let policy = NetworkPolicy::UnavailablePrivateNetwork;
        assert!(!policy.permits_proc_network());
        assert_eq!(
            policy.source(false),
            "linux_proc_sys_host_cpu_memory_load_disk_thermal_clock_network_unavailable_private_namespace"
        );
        let disk = Some(CounterPair {
            first: 2_048,
            second: 4_096,
        });
        let prior = Some(CounterPair {
            first: 1_024,
            second: 2_048,
        });
        let disk_rates = normalized_pair_rate(disk, prior, Duration::from_secs(1), 1_024.0);
        let network_rates = normalized_pair_rate(None, None, Duration::from_secs(1), 1_024.0);
        assert!(disk_rates.0 > 0.0 && disk_rates.1 > 0.0);
        assert_eq!(network_rates, (0.0, 0.0));
    }

    #[test]
    fn counter_rates_are_bounded_and_monotonic() {
        let elapsed = Duration::from_secs(1);
        let low = normalize_counter_rate(1_024, elapsed, 1_024.0);
        let high = normalize_counter_rate(8_192, elapsed, 1_024.0);
        assert!((0.0..=1.0).contains(&low));
        assert!((0.0..=1.0).contains(&high));
        assert!(high > low);
    }
}
