use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicU32, AtomicU64, AtomicU8, Ordering};
use std::thread::JoinHandle;
use std::time::Duration;

use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use cpal::{FromSample, Sample, SampleFormat, SizedSample};
use parking_lot::Mutex;
use rtrb::{Consumer, Producer, RingBuffer};

mod error;
pub mod wav;

pub use error::{RecorderError, Result};
pub use wav::{WavInfo, WavWriter};

const STATE_RECORDING: u8 = 0;
const STATE_PAUSED: u8 = 1;
const STATE_STOPPING: u8 = 2;
const STATE_STOPPED: u8 = 3;
const STATE_ERROR: u8 = 4;

const SNAPSHOT_PEAKS: usize = 480;
const PEAKS_PER_SECOND: u64 = 50;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DeviceInfo {
    pub name: String,
    pub is_default: bool,
    pub sample_rate: u32,
    pub channels: u16,
}

#[derive(Clone, Debug)]
pub struct CaptureConfig {
    pub device: Option<String>,
    pub sample_rate: Option<u32>,
}

impl Default for CaptureConfig {
    fn default() -> Self {
        Self {
            device: None,
            sample_rate: None,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RecorderState {
    Recording,
    Paused,
    Stopping,
    Stopped,
    Error,
}

#[derive(Clone, Debug)]
pub struct Mark {
    pub offset_ms: u64,
    pub label: Option<String>,
}

#[derive(Clone, Debug)]
pub struct RecorderSnapshot {
    pub state: RecorderState,
    pub duration_ms: u64,
    pub level: f32,
    pub peaks: Vec<f32>,
    pub marks: Vec<Mark>,
    pub dropped_samples: u64,
    pub error: Option<String>,
}

#[derive(Clone, Debug)]
pub struct StopResult {
    pub path: PathBuf,
    pub sample_rate: u32,
    pub duration_ms: u64,
    pub marks: Vec<Mark>,
    pub peaks: Vec<f32>,
    pub dropped_samples: u64,
    pub error: Option<String>,
}

#[derive(Clone, Debug)]
pub struct Recovered {
    pub path: PathBuf,
    pub sample_rate: u32,
    pub duration_ms: u64,
    pub peaks: Vec<u8>,
}

struct Shared {
    state: AtomicU8,
    written_frames: AtomicU64,
    dropped_samples: AtomicU64,
    level_bits: AtomicU32,
    peaks: Mutex<Vec<f32>>,
    marks: Mutex<Vec<Mark>>,
    error: Mutex<Option<String>>,
    sample_rate: u32,
    finished: Mutex<Option<WavInfo>>,
}

impl Shared {
    fn state(&self) -> u8 {
        self.state.load(Ordering::Acquire)
    }

    fn set_state(&self, state: u8) {
        self.state.store(state, Ordering::Release);
    }

    fn set_error(&self, error: impl Into<String>) {
        let error = error.into();
        log::error!("recorder: {error}");
        *self.error.lock() = Some(error);
        self.set_state(STATE_ERROR);
    }

    fn duration_ms(&self) -> u64 {
        self.written_frames.load(Ordering::Acquire) * 1000 / self.sample_rate.max(1) as u64
    }

    fn push_peak(&self, peak: f32) {
        let peak = peak.clamp(0.0, 1.0);
        self.level_bits.store(peak.to_bits(), Ordering::Release);
        self.peaks.lock().push(peak);
    }

    fn snapshot(&self) -> RecorderSnapshot {
        let peaks = self.peaks.lock();
        let start = peaks.len().saturating_sub(SNAPSHOT_PEAKS);
        let recent = peaks[start..].to_vec();
        drop(peaks);

        let state = match self.state() {
            STATE_RECORDING => RecorderState::Recording,
            STATE_PAUSED => RecorderState::Paused,
            STATE_STOPPING => RecorderState::Stopping,
            STATE_STOPPED => RecorderState::Stopped,
            _ => RecorderState::Error,
        };
        RecorderSnapshot {
            state,
            duration_ms: self.duration_ms(),
            level: f32::from_bits(self.level_bits.load(Ordering::Acquire)),
            peaks: recent,
            marks: self.marks.lock().clone(),
            dropped_samples: self.dropped_samples.load(Ordering::Relaxed),
            error: self.error.lock().clone(),
        }
    }
}

pub struct Recorder {
    shared: Arc<Shared>,
    stream: Option<cpal::Stream>,
    writer: Option<JoinHandle<()>>,
    part_path: PathBuf,
    sample_rate: u32,
    channels: u16,
    device_name: String,
}

impl Recorder {
    pub fn start(config: CaptureConfig, part_path: impl Into<PathBuf>) -> Result<Self> {
        let part_path = part_path.into();
        let (device, stream_config, device_name) = resolve_input(&config)?;
        let sample_rate = stream_config.sample_rate();
        let channels = stream_config.channels();
        let shared = Arc::new(Shared {
            state: AtomicU8::new(STATE_RECORDING),
            written_frames: AtomicU64::new(0),
            dropped_samples: AtomicU64::new(0),
            level_bits: AtomicU32::new(0),
            peaks: Mutex::new(Vec::new()),
            marks: Mutex::new(Vec::new()),
            error: Mutex::new(None),
            sample_rate,
            finished: Mutex::new(None),
        });

        let (producer, consumer) = RingBuffer::<f32>::new((sample_rate as usize * 2).max(4096));
        let stream = build_stream(
            &device,
            &stream_config,
            producer,
            shared.clone(),
            channels,
        )?;

        let writer = WavWriter::create(&part_path, sample_rate, 1)?;
        let writer_shared = shared.clone();
        let writer_handle = std::thread::Builder::new()
            .name("aareco-wav".into())
            .spawn(move || {
                writer_loop(writer, consumer, writer_shared, sample_rate);
            })
            .map_err(|e| RecorderError::Thread(e.to_string()))?;

        stream
            .play()
            .map_err(|e| RecorderError::Start(format!("could not start microphone: {e}")))?;

        log::info!(
            "recording from {device_name:?} at {sample_rate} Hz, {channels} channel(s)"
        );
        Ok(Self {
            shared,
            stream: Some(stream),
            writer: Some(writer_handle),
            part_path,
            sample_rate,
            channels,
            device_name,
        })
    }

    pub fn sample_rate(&self) -> u32 {
        self.sample_rate
    }

    pub fn channels(&self) -> u16 {
        self.channels
    }

    pub fn device_name(&self) -> &str {
        &self.device_name
    }

    pub fn snapshot(&self) -> RecorderSnapshot {
        self.shared.snapshot()
    }

    pub fn pause(&self) {
        if self.shared.state() == STATE_RECORDING {
            self.shared.set_state(STATE_PAUSED);
            self.shared.level_bits.store(0f32.to_bits(), Ordering::Release);
        }
    }

    pub fn resume(&self) {
        if self.shared.state() == STATE_PAUSED {
            self.shared.set_state(STATE_RECORDING);
        }
    }

    pub fn toggle_pause(&self) {
        match self.shared.state() {
            STATE_RECORDING => self.pause(),
            STATE_PAUSED => self.resume(),
            _ => {}
        }
    }

    pub fn mark(&self, label: Option<String>) -> Mark {
        let mark = Mark {
            offset_ms: self.shared.duration_ms(),
            label,
        };
        self.shared.marks.lock().push(mark.clone());
        mark
    }

    pub fn stop(mut self) -> Result<StopResult> {
        self.shutdown()
    }

    pub fn cancel(mut self) -> Result<()> {
        let result = self.shutdown();
        let _ = std::fs::remove_file(self.part_path.with_extension("wav"));
        let _ = std::fs::remove_file(&self.part_path);
        result.map(|_| ())
    }

    fn shutdown(&mut self) -> Result<StopResult> {
        if self.stream.is_some() || self.writer.is_some() {
            self.shared.set_state(STATE_STOPPING);
            self.stream.take();
            if let Some(writer) = self.writer.take() {
                writer
                    .join()
                    .map_err(|e| RecorderError::Thread(format!("{e:?}")))?;
            }
        }

        let info = self.shared.finished.lock().clone();
        let error = self.shared.error.lock().clone();
        let path = info
            .as_ref()
            .map(|i| i.path.clone())
            .unwrap_or_else(|| self.part_path.clone());
        Ok(StopResult {
            path,
            sample_rate: self.sample_rate,
            duration_ms: self.shared.duration_ms(),
            marks: self.shared.marks.lock().clone(),
            peaks: self.shared.peaks.lock().clone(),
            dropped_samples: self.shared.dropped_samples.load(Ordering::Relaxed),
            error,
        })
    }
}

impl Drop for Recorder {
    fn drop(&mut self) {
        if self.writer.is_some() {
            let _ = self.shutdown();
        }
    }
}

fn writer_loop(mut writer: WavWriter, mut consumer: Consumer<f32>, shared: Arc<Shared>, rate: u32) {
    let bucket_frames = (rate as u64 / PEAKS_PER_SECOND).max(1);
    let mut bucket_count = 0u64;
    let mut bucket_peak = 0f32;
    let mut frames_written = 0u64;
    let mut scratch: Vec<i16> = Vec::with_capacity(8192);

    loop {
        let state = shared.state();
        while let Ok(sample) = consumer.pop() {
            if state == STATE_PAUSED {
                continue;
            }
            let clamped = sample.clamp(-1.0, 1.0);
            scratch.push((clamped * i16::MAX as f32) as i16);
            bucket_peak = bucket_peak.max(clamped.abs());
            bucket_count += 1;
            if bucket_count >= bucket_frames {
                shared.push_peak(bucket_peak);
                bucket_count = 0;
                bucket_peak = 0.0;
            }
        }

        if !scratch.is_empty() {
            if let Err(e) = writer.write_samples(&scratch) {
                shared.set_error(e.to_string());
                break;
            }
            frames_written += scratch.len() as u64;
            shared
                .written_frames
                .store(frames_written, Ordering::Release);
            scratch.clear();
        }

        if state == STATE_PAUSED || state == STATE_STOPPING || state == STATE_ERROR {
            shared.level_bits.store(0f32.to_bits(), Ordering::Release);
        }
        if (state == STATE_STOPPING || state == STATE_ERROR) && consumer.is_empty() {
            break;
        }
        std::thread::sleep(Duration::from_millis(3));
    }

    if bucket_count > 0 {
        shared.push_peak(bucket_peak);
    }
    match writer.finalize() {
        Ok(info) => {
            log::info!("wrote {} frames to {}", info.frames, info.path.display());
            *shared.finished.lock() = Some(info);
            if shared.state() != STATE_ERROR {
                shared.set_state(STATE_STOPPED);
            }
        }
        Err(e) => shared.set_error(e.to_string()),
    }
}

fn resolve_input(config: &CaptureConfig) -> Result<(cpal::Device, cpal::SupportedStreamConfig, String)> {
    let host = cpal::default_host();
    let device = match &config.device {
        Some(name) => {
            let mut found = None;
            for candidate in host
                .input_devices()
                .map_err(|e| RecorderError::Device(e.to_string()))?
            {
                if device_label(&candidate).as_deref() == Some(name.as_str()) {
                    found = Some(candidate);
                    break;
                }
            }
            found.ok_or_else(|| RecorderError::DeviceNotFound(name.clone()))?
        }
        None => host
            .default_input_device()
            .ok_or(RecorderError::NoInputDevice)?,
    };
    let device_name = device_label(&device).unwrap_or_else(|| "microphone".to_string());
    let default = device
        .default_input_config()
        .map_err(|e| RecorderError::Device(e.to_string()))?;

    let mut chosen = default.clone();
    if let Some(rate) = config.sample_rate
        && rate != default.sample_rate()
        && let Ok(ranges) = device.supported_input_configs()
    {
        for range in ranges {
            if range.sample_format() == default.sample_format()
                && rate >= range.min_sample_rate()
                && rate <= range.max_sample_rate()
            {
                chosen = range.with_sample_rate(rate);
                break;
            }
        }
    }
    Ok((device, chosen, device_name))
}

fn build_stream(
    device: &cpal::Device,
    config: &cpal::SupportedStreamConfig,
    producer: Producer<f32>,
    shared: Arc<Shared>,
    channels: u16,
) -> Result<cpal::Stream> {
    let stream_config = config.config();
    match config.sample_format() {
        SampleFormat::I8 => build_typed::<i8>(device, stream_config, producer, shared, channels),
        SampleFormat::I16 => build_typed::<i16>(device, stream_config, producer, shared, channels),
        SampleFormat::I24 => build_typed::<cpal::I24>(device, stream_config, producer, shared, channels),
        SampleFormat::I32 => build_typed::<i32>(device, stream_config, producer, shared, channels),
        SampleFormat::I64 => build_typed::<i64>(device, stream_config, producer, shared, channels),
        SampleFormat::U8 => build_typed::<u8>(device, stream_config, producer, shared, channels),
        SampleFormat::U16 => build_typed::<u16>(device, stream_config, producer, shared, channels),
        SampleFormat::U24 => build_typed::<cpal::U24>(device, stream_config, producer, shared, channels),
        SampleFormat::U32 => build_typed::<u32>(device, stream_config, producer, shared, channels),
        SampleFormat::U64 => build_typed::<u64>(device, stream_config, producer, shared, channels),
        SampleFormat::F32 => build_typed::<f32>(device, stream_config, producer, shared, channels),
        SampleFormat::F64 => build_typed::<f64>(device, stream_config, producer, shared, channels),
        other => Err(RecorderError::UnsupportedFormat(format!("{other:?}"))),
    }
}

fn build_typed<T>(
    device: &cpal::Device,
    config: cpal::StreamConfig,
    mut producer: Producer<f32>,
    shared: Arc<Shared>,
    channels: u16,
) -> Result<cpal::Stream>
where
    T: SizedSample + Send + 'static,
    f32: FromSample<T>,
{
    let channels = channels.max(1);
    let error_shared = shared.clone();
    device
        .build_input_stream(
            config,
            move |data: &[T], _| {
                for frame in data.chunks(channels as usize) {
                    let mut sum = 0f32;
                    for sample in frame {
                        sum += f32::from_sample(*sample);
                    }
                    let mono = (sum / frame.len() as f32).clamp(-1.0, 1.0);
                    if producer.push(mono).is_err() {
                        shared.dropped_samples.fetch_add(1, Ordering::Relaxed);
                    }
                }
            },
            move |err| error_shared.set_error(format!("microphone stream stopped: {err}")),
            None,
        )
        .map_err(|e| RecorderError::Device(e.to_string()))
}

fn device_label(device: &cpal::Device) -> Option<String> {
    device
        .description()
        .map(|description| description.name().to_string())
        .ok()
        .or_else(|| {
            let name = device.to_string();
            (!name.trim().is_empty()).then_some(name)
        })
}

pub fn input_devices() -> Result<Vec<DeviceInfo>> {
    let host = cpal::default_host();
    let default_name = host
        .default_input_device()
        .and_then(|d| device_label(&d));
    let mut devices = Vec::new();
    let mut seen = std::collections::HashSet::new();
    for device in host
        .input_devices()
        .map_err(|e| RecorderError::Device(e.to_string()))?
    {
        let Some(name) = device_label(&device) else {
            continue;
        };
        if name.trim().is_empty() || !seen.insert(name.clone()) {
            continue;
        }
        #[cfg(target_os = "linux")]
        if is_alsa_plugin(&name) {
            continue;
        }
        let Ok(config) = device.default_input_config() else {
            continue;
        };
        if config.channels() == 0 {
            continue;
        }
        devices.push(DeviceInfo {
            is_default: default_name.as_deref() == Some(name.as_str()),
            name,
            sample_rate: config.sample_rate(),
            channels: config.channels(),
        });
    }
    Ok(devices)
}

/// Finalize partial WAV files left by a crash. Safe to call at startup.
pub fn recover_partial_files(dir: &Path) -> Vec<Recovered> {
    let mut recovered = Vec::new();
    let Ok(entries) = std::fs::read_dir(dir) else {
        return recovered;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.extension().and_then(|e| e.to_str()) != Some("part") {
            continue;
        }
        match wav::recover_partial(&path) {
            Ok(Some(info)) => {
                let peaks = wav::read_peaks(&info.path, 600).unwrap_or_default();
                let duration_ms = info.duration_ms();
                recovered.push(Recovered {
                    path: info.path,
                    sample_rate: info.sample_rate,
                    duration_ms,
                    peaks,
                });
            }
            Ok(None) => {}
            Err(e) => log::warn!("could not recover {}: {e}", path.display()),
        }
    }
    recovered
}

pub fn downsample_peaks(peaks: &[f32], buckets: usize) -> Vec<u8> {
    wav::downsample_peaks(peaks, buckets)
}

#[cfg(target_os = "linux")]
fn is_alsa_plugin(name: &str) -> bool {
    const MARKERS: [&str; 8] = [
        "Discard all samples",
        "Rate Converter Plugin",
        "JACK Audio Connection Kit",
        "Open Sound System",
        "Plugin using Speex",
        "Plugin for channel upmix",
        "Plugin for downmix",
        "Plugin for surround",
    ];
    MARKERS.iter().any(|marker| name.contains(marker))
}
