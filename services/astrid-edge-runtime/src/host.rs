use std::{
    f32::consts::PI,
    fs,
    io::{ErrorKind, Read as _},
    path::Path,
    process::{Command, Stdio},
    thread,
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};

use tokio::sync::mpsc;

use crate::reservoir::SensoryIngress;

const AUDIO_FEATURE_DIM: usize = 8;
const AUDIO_SPECTRUM_SAMPLES: usize = 256;
const AUDIO_MEL_BANDS: usize = 16;
const AUDIO_MFCCS: usize = 4;
const AUDIO_CHUNKS_PER_SECOND: usize = 10;

#[derive(Clone, Copy, Default)]
struct CpuSample {
    total: u64,
    idle: u64,
}

#[derive(Clone, Copy, Default)]
struct IoSample {
    disk_read: u64,
    disk_write: u64,
    network_receive: u64,
    network_transmit: u64,
}

const HOST_AUX_SOURCE: &str = "linux_proc_sys_cpu_memory_load_disk_network_thermal_clock";
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
    let mut previous_cpu = read_cpu().unwrap_or_default();
    let mut previous_io = read_io().unwrap_or_default();
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
        let current_io = read_io();
        let now = Instant::now();
        let elapsed = now.saturating_duration_since(previous_io_at);
        let (disk_read, disk_write, network_receive, network_transmit) =
            current_io.map_or((0.0, 0.0, 0.0, 0.0), |sample| {
                let rates = (
                    normalize_counter_rate(
                        sample.disk_read.saturating_sub(previous_io.disk_read),
                        elapsed,
                        DISK_RATE_SCALE_BYTES_PER_SECOND,
                    ),
                    normalize_counter_rate(
                        sample.disk_write.saturating_sub(previous_io.disk_write),
                        elapsed,
                        DISK_RATE_SCALE_BYTES_PER_SECOND,
                    ),
                    normalize_counter_rate(
                        sample
                            .network_receive
                            .saturating_sub(previous_io.network_receive),
                        elapsed,
                        NETWORK_RATE_SCALE_BYTES_PER_SECOND,
                    ),
                    normalize_counter_rate(
                        sample
                            .network_transmit
                            .saturating_sub(previous_io.network_transmit),
                        elapsed,
                        NETWORK_RATE_SCALE_BYTES_PER_SECOND,
                    ),
                );
                previous_io = sample;
                previous_io_at = now;
                rates
            });
        let thermal = read_thermal_normalized();
        let (daily_sine, daily_cosine) = daily_phase();
        let io_available = current_io.is_some();
        let availability = vec![
            current_cpu.is_some(),
            memory_used.is_some(),
            load.is_some(),
            io_available,
            io_available,
            io_available,
            io_available,
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
                source: HOST_AUX_SOURCE.to_string(),
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
    let source = format!("physical_alsa:{device}:{sample_rate}hz:{channels}ch");

    loop {
        match capture_audio(ingress_tx, &device, sample_rate, channels, &source) {
            Ok(()) => return,
            Err(error) => {
                eprintln!("edge physical audio unavailable ({source}): {error}; retrying in 10s");
                thread::sleep(Duration::from_secs(10));
            },
        }
    }
}

fn capture_audio(
    ingress_tx: &mpsc::Sender<SensoryIngress>,
    device: &str,
    sample_rate: u32,
    channels: u16,
    source: &str,
) -> std::io::Result<()> {
    let mut child = Command::new("arecord")
        .args([
            "-D",
            device,
            "-q",
            "-t",
            "raw",
            "-f",
            "S16_LE",
            "-r",
            &sample_rate.to_string(),
            "-c",
            &channels.to_string(),
        ])
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::inherit())
        .spawn()?;
    let Some(mut stdout) = child.stdout.take() else {
        return Err(std::io::Error::other("arecord stdout was not piped"));
    };
    eprintln!("edge physical audio connected: {source}");

    let frames_per_chunk = usize::try_from(sample_rate).unwrap_or(44_100) / AUDIO_CHUNKS_PER_SECOND;
    let bytes_per_frame = usize::from(channels).saturating_mul(size_of::<i16>());
    let mut bytes = vec![0_u8; frames_per_chunk.saturating_mul(bytes_per_frame)];

    loop {
        match stdout.read_exact(&mut bytes) {
            Ok(()) => {},
            Err(error) if error.kind() == ErrorKind::Interrupted => continue,
            Err(error) => {
                let _ = child.kill();
                let _ = child.wait();
                return Err(error);
            },
        }
        let samples = decode_mono_s16le(&bytes, channels);
        let features = extract_audio_features(&samples, sample_rate);
        if ingress_tx
            .blocking_send(SensoryIngress::Audio {
                features: features.to_vec(),
                source: source.to_string(),
            })
            .is_err()
        {
            let _ = child.kill();
            let _ = child.wait();
            return Ok(());
        }
    }
}

fn decode_mono_s16le(bytes: &[u8], channels: u16) -> Vec<f32> {
    let channels = usize::from(channels.max(1));
    let sample_count = bytes.len() / size_of::<i16>();
    let frame_count = sample_count / channels;
    let mut mono = Vec::with_capacity(frame_count);
    for frame in 0..frame_count {
        let mut sum = 0.0_f32;
        for channel in 0..channels {
            let sample_index = frame.saturating_mul(channels).saturating_add(channel);
            let byte_index = sample_index.saturating_mul(size_of::<i16>());
            let sample = i16::from_le_bytes([bytes[byte_index], bytes[byte_index + 1]]);
            sum += f32::from(sample) / 32_768.0;
        }
        mono.push(sum / f32::from(u16::try_from(channels).unwrap_or(u16::MAX)));
    }
    mono
}

#[allow(clippy::cast_precision_loss)]
fn extract_audio_features(samples: &[f32], sample_rate: u32) -> [f32; AUDIO_FEATURE_DIM] {
    if samples.is_empty() {
        return [0.0; AUDIO_FEATURE_DIM];
    }
    let sample_count = samples.len() as f32;
    let rms = (samples.iter().map(|sample| sample * sample).sum::<f32>() / sample_count).sqrt();
    let normalized_rms = (((rms + 1.0e-10).ln().max(-6.0) + 6.0) / 6.0).clamp(0.0, 1.0);
    let zero_crossings = samples
        .windows(2)
        .filter(|pair| (pair[0] >= 0.0) != (pair[1] >= 0.0))
        .count();
    let zcr = zero_crossings as f32 / samples.len().saturating_sub(1).max(1) as f32;

    let spectrum_input = spectrum_window(samples);
    let magnitudes = magnitude_spectrum(&spectrum_input);
    let magnitude_total = magnitudes.iter().sum::<f32>();
    let nyquist = sample_rate as f32 / 2.0;
    let frequency_step = sample_rate as f32 / AUDIO_SPECTRUM_SAMPLES as f32;
    let centroid_hz = if magnitude_total <= f32::EPSILON {
        0.0
    } else {
        magnitudes
            .iter()
            .enumerate()
            .map(|(bin, magnitude)| bin as f32 * frequency_step * magnitude)
            .sum::<f32>()
            / magnitude_total
    };
    let bandwidth_hz = if magnitude_total <= f32::EPSILON {
        0.0
    } else {
        (magnitudes
            .iter()
            .enumerate()
            .map(|(bin, magnitude)| {
                let delta = bin as f32 * frequency_step - centroid_hz;
                magnitude * delta * delta
            })
            .sum::<f32>()
            / magnitude_total)
            .sqrt()
    };
    let cepstral = cepstral_coefficients(&magnitudes, sample_rate);

    [
        normalized_rms,
        (centroid_hz / nyquist).clamp(0.0, 1.0),
        (bandwidth_hz / nyquist).clamp(0.0, 1.0),
        zcr.clamp(0.0, 1.0),
        cepstral[0],
        cepstral[1],
        cepstral[2],
        cepstral[3],
    ]
}

#[allow(clippy::cast_precision_loss)]
fn spectrum_window(samples: &[f32]) -> [f32; AUDIO_SPECTRUM_SAMPLES] {
    let mut window = [0.0_f32; AUDIO_SPECTRUM_SAMPLES];
    let final_source_index = samples.len().saturating_sub(1);
    let final_target_index = AUDIO_SPECTRUM_SAMPLES.saturating_sub(1);
    for (index, value) in window.iter_mut().enumerate() {
        let source_index = index.saturating_mul(final_source_index) / final_target_index.max(1);
        let hann = 0.5 - 0.5 * (2.0 * PI * index as f32 / final_target_index.max(1) as f32).cos();
        *value = samples[source_index] * hann;
    }
    window
}

#[allow(clippy::cast_precision_loss)]
fn magnitude_spectrum(samples: &[f32; AUDIO_SPECTRUM_SAMPLES]) -> Vec<f32> {
    let mut magnitudes = Vec::with_capacity(AUDIO_SPECTRUM_SAMPLES / 2 + 1);
    for bin in 0..=AUDIO_SPECTRUM_SAMPLES / 2 {
        let mut real = 0.0_f32;
        let mut imaginary = 0.0_f32;
        for (index, sample) in samples.iter().enumerate() {
            let angle = 2.0 * PI * bin as f32 * index as f32 / AUDIO_SPECTRUM_SAMPLES as f32;
            real += sample * angle.cos();
            imaginary -= sample * angle.sin();
        }
        magnitudes.push((real * real + imaginary * imaginary).sqrt());
    }
    magnitudes
}

#[allow(
    clippy::cast_precision_loss,
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss
)]
fn cepstral_coefficients(magnitudes: &[f32], sample_rate: u32) -> [f32; AUDIO_MFCCS] {
    let mut energies = [0.0_f32; AUDIO_MEL_BANDS];
    let mut counts = [0_u16; AUDIO_MEL_BANDS];
    let nyquist = sample_rate as f32 / 2.0;
    let maximum_mel = hz_to_mel(nyquist);
    for (bin, magnitude) in magnitudes.iter().enumerate().skip(1) {
        let frequency = bin as f32 * sample_rate as f32 / AUDIO_SPECTRUM_SAMPLES as f32;
        let mel_position = hz_to_mel(frequency) / maximum_mel * AUDIO_MEL_BANDS as f32;
        let band = (mel_position.floor() as usize).min(AUDIO_MEL_BANDS - 1);
        energies[band] += magnitude * magnitude;
        counts[band] = counts[band].saturating_add(1);
    }
    for (energy, count) in energies.iter_mut().zip(counts) {
        *energy = (*energy / f32::from(count.max(1)) + 1.0e-8).ln();
    }

    let mut coefficients = [0.0_f32; AUDIO_MFCCS];
    for (coefficient, value) in coefficients.iter_mut().enumerate() {
        let raw = energies
            .iter()
            .enumerate()
            .map(|(band, energy)| {
                let phase = PI * coefficient as f32 * (band as f32 + 0.5) / AUDIO_MEL_BANDS as f32;
                energy * phase.cos()
            })
            .sum::<f32>()
            / AUDIO_MEL_BANDS as f32;
        *value = (raw / 4.0).tanh();
    }
    coefficients
}

fn hz_to_mel(frequency: f32) -> f32 {
    2_595.0 * (1.0 + frequency / 700.0).log10()
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

fn read_io() -> Option<IoSample> {
    let disk = fs::read_to_string("/proc/diskstats")
        .ok()
        .and_then(|content| {
            parse_disk_counters(&content, |name| Path::new("/sys/block").join(name).is_dir())
        });
    let network = fs::read_to_string("/proc/net/dev")
        .ok()
        .and_then(|content| parse_network_counters(&content));
    match (disk, network) {
        (None, None) => None,
        (disk, network) => {
            let disk = disk.unwrap_or_default();
            let network = network.unwrap_or_default();
            Some(IoSample {
                disk_read: disk.0,
                disk_write: disk.1,
                network_receive: network.0,
                network_transmit: network.1,
            })
        },
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

fn parse_network_counters(content: &str) -> Option<(u64, u64)> {
    let mut found = false;
    let mut received = 0_u64;
    let mut transmitted = 0_u64;
    for line in content.lines() {
        let Some((interface, counters)) = line.split_once(':') else {
            continue;
        };
        if interface.trim() == "lo" {
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
        decode_mono_s16le, extract_audio_features, normalize_counter_rate, parse_disk_counters,
        parse_network_counters,
    };
    use std::time::Duration;

    #[test]
    fn stereo_pcm_is_mixed_to_mono() {
        let bytes = [
            0x00, 0x40, 0x00, 0x00, // 0.5 left, silence right
            0x00, 0x00, 0x00, 0xc0, // silence left, -0.5 right
        ];
        let mono = decode_mono_s16le(&bytes, 2);
        assert_eq!(mono.len(), 2);
        assert!((mono[0] - 0.25).abs() < 0.001);
        assert!((mono[1] + 0.25).abs() < 0.001);
    }

    #[test]
    fn audio_features_are_bounded_and_distinguish_tone_from_silence() {
        let silence = vec![0.0_f32; 4_410];
        let tone = (0_u16..4_410)
            .map(|index| {
                let phase = 2.0 * std::f32::consts::PI * 440.0 * f32::from(index) / 44_100.0;
                0.4 * phase.sin()
            })
            .collect::<Vec<_>>();
        let silence_features = extract_audio_features(&silence, 44_100);
        let tone_features = extract_audio_features(&tone, 44_100);

        assert!(
            tone_features
                .iter()
                .all(|value| value.is_finite() && (-1.0..=1.0).contains(value))
        );
        assert!(tone_features[0] > silence_features[0]);
        assert!(tone_features[1] > silence_features[1]);
        assert!(
            tone_features
                .iter()
                .zip(silence_features)
                .any(|(tone, silence)| (tone - silence).abs() > 0.001)
        );
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
        assert_eq!(parse_network_counters(fixture), Some((800, 1_000)));
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
