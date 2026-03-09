use std::collections::VecDeque;
use std::sync::{Arc, Mutex};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};

use bevy::prelude::*;
use cpal::InterfaceType;
use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use pitch_detection::detector::mcleod::McLeodDetector;
use pitch_detection::detector::PitchDetector;

const PITCH_WINDOW: usize = 2048;
const MIN_PITCH_HZ: f32 = 80.0;
const MAX_PITCH_HZ: f32 = 1000.0;
const PITCH_POWER_THRESHOLD: f32 = 0.2;
const PITCH_CLARITY_THRESHOLD: f32 = 0.4;
const MIC_RMS_GATE: f32 = 0.012;
const REF_RMS_GATE: f32 = 0.005;

fn rms(samples: &[f32]) -> f32 {
    if samples.is_empty() {
        return 0.0;
    }
    let sum_sq: f32 = samples.iter().map(|s| s * s).sum();
    (sum_sq / samples.len() as f32).sqrt()
}

pub struct MicPitchData {
    pub latest_pitch: Option<f32>,
    samples: VecDeque<f32>,
    sample_rate: u32,
}

impl MicPitchData {
    fn new(sample_rate: u32) -> Self {
        Self {
            latest_pitch: None,
            samples: VecDeque::with_capacity(PITCH_WINDOW * 2),
            sample_rate,
        }
    }
}

#[derive(Resource)]
pub struct MicrophoneCapture {
    pub active: bool,
    pub device_name: String,
    shared: Arc<Mutex<MicPitchData>>,
    _stream: Option<cpal::Stream>,
    stream_error: Arc<AtomicBool>,
    silence_only: Arc<AtomicBool>,
    sample_counter: Arc<AtomicU64>,
    last_checked_count: u64,
    stale_ticks: u32,
    shutdown: Arc<AtomicBool>,
    pitch_thread: Option<std::thread::JoinHandle<()>>,
}

impl Drop for MicrophoneCapture {
    fn drop(&mut self) {
        self.shutdown.store(true, Ordering::Relaxed);
        if let Some(handle) = self.pitch_thread.take() {
            let _ = handle.join();
        }
    }
}

const STALE_THRESHOLD_TICKS: u32 = 60;

impl MicrophoneCapture {
    pub fn latest_pitch(&self) -> Option<f32> {
        if !self.active {
            return None;
        }
        self.shared.lock().ok()?.latest_pitch
    }

    pub fn has_stream(&self) -> bool {
        self._stream.is_some()
    }

    pub fn is_silence_only(&self) -> bool {
        self.silence_only.load(Ordering::Relaxed)
    }

    pub fn check_health(&mut self) -> bool {
        if !self.active || !self.has_stream() {
            return true;
        }
        if self.stream_error.load(Ordering::Relaxed) {
            return false;
        }
        let count = self.sample_counter.load(Ordering::Relaxed);
        if count == self.last_checked_count {
            self.stale_ticks += 1;
        } else {
            self.stale_ticks = 0;
            self.last_checked_count = count;
        }
        self.stale_ticks < STALE_THRESHOLD_TICKS
    }
}

fn device_display_name(device: &cpal::Device) -> String {
    device
        .description()
        .map(|d| d.name().to_string())
        .unwrap_or_else(|_| "(unknown)".into())
}

fn is_bluetooth(device: &cpal::Device) -> bool {
    device
        .description()
        .map(|d| d.interface_type() == InterfaceType::Bluetooth)
        .unwrap_or(false)
}

fn is_virtual_device(device: &cpal::Device) -> bool {
    let name = device_display_name(device).to_lowercase();
    let virtual_patterns = [
        "jack audio connection kit",
        "monitor of",
        "loopback",
        "virtual",
        "null",
        "zoom"
    ];
    virtual_patterns.iter().any(|p| name.contains(p))
}

pub fn available_devices() -> Vec<String> {
    let host = cpal::default_host();
    let Some(devices) = host.input_devices().ok() else {
        return vec![];
    };
    devices
        .filter(|d| !is_virtual_device(d))
        .filter_map(|d| d.description().ok().map(|desc| desc.name().to_string()))
        .collect()
}

fn select_device(host: &cpal::Host, preferred: Option<&str>) -> Option<cpal::Device> {
    if let Some(pref_name) = preferred {
        let devices = host.input_devices().ok()?;
        for dev in devices {
            if device_display_name(&dev) == pref_name {
                if is_virtual_device(&dev) {
                    warn!("Preferred mic '{pref_name}' is a virtual device, skipping");
                    break;
                }
                if let Ok(cfg) = dev.default_input_config() {
                    let bt = if is_bluetooth(&dev) { " [Bluetooth]" } else { "" };
                    info!(
                        "Using preferred mic '{pref_name}' ({}Hz){bt}",
                        cfg.sample_rate()
                    );
                    return Some(dev);
                }
            }
        }
        warn!("Preferred mic '{pref_name}' not found, auto-selecting");
    }

    let devices: Vec<cpal::Device> = host.input_devices().ok()?.collect();
    if devices.is_empty() {
        return None;
    }

    let mut best: Option<(cpal::Device, u32, bool)> = None;
    for dev in devices {
        let Ok(cfg) = dev.default_input_config() else {
            continue;
        };
        let sr = cfg.sample_rate();
        let bt = is_bluetooth(&dev);
        let virt = is_virtual_device(&dev);
        let name = device_display_name(&dev);
        let labels = match (bt, virt) {
            (true, _) => " [Bluetooth]",
            (_, true) => " [Virtual — skipped]",
            _ => "",
        };
        info!("  Available mic: '{name}' ({sr}Hz){labels}");

        if virt {
            continue;
        }

        let dominated = match &best {
            None => false,
            Some((_, best_sr, best_bt)) => {
                if bt && !best_bt {
                    true
                } else if !bt && *best_bt {
                    false
                } else {
                    sr <= *best_sr
                }
            }
        };
        if !dominated {
            best = Some((dev, sr, bt));
        }
    }

    best.map(|(dev, _, _)| dev)
}

pub fn start_microphone(preferred: Option<&str>) -> MicrophoneCapture {
    let shutdown = Arc::new(AtomicBool::new(false));
    let (shared, stream, name, error_flag, silence_flag, counter, pitch_thread) =
        try_build_stream(preferred, Arc::clone(&shutdown));

    let shared = shared.unwrap_or_else(|| {
        warn!("No microphone found or permission denied; scoring disabled");
        Arc::new(Mutex::new(MicPitchData::new(44100)))
    });

    MicrophoneCapture {
        active: stream.is_some(),
        device_name: name,
        shared,
        _stream: stream,
        stream_error: error_flag,
        silence_only: silence_flag,
        sample_counter: counter,
        last_checked_count: 0,
        stale_ticks: 0,
        shutdown,
        pitch_thread,
    }
}

fn try_build_stream(
    preferred: Option<&str>,
    shutdown: Arc<AtomicBool>,
) -> (
    Option<Arc<Mutex<MicPitchData>>>,
    Option<cpal::Stream>,
    String,
    Arc<AtomicBool>,
    Arc<AtomicBool>,
    Arc<AtomicU64>,
    Option<std::thread::JoinHandle<()>>,
) {
    let error_flag = Arc::new(AtomicBool::new(false));
    let silence_flag = Arc::new(AtomicBool::new(false));
    let sample_counter = Arc::new(AtomicU64::new(0));

    let ret_err = Arc::clone(&error_flag);
    let ret_sil = Arc::clone(&silence_flag);
    let ret_ctr = Arc::clone(&sample_counter);

    let host = cpal::default_host();
    let device = match select_device(&host, preferred) {
        Some(d) => d,
        None => return (None, None, "(no mic)".into(), ret_err, ret_sil, ret_ctr, None),
    };
    let name = device_display_name(&device);
    let bt = is_bluetooth(&device);
    info!("Microphone: {name}{}", if bt { " [Bluetooth]" } else { "" });

    if bt {
        warn!(
            "Bluetooth mic selected — this may force HFP profile and degrade audio output quality. \
             Press [N] to switch to a different mic."
        );
    }

    let default_cfg = match device.default_input_config() {
        Ok(c) => c,
        Err(e) => {
            warn!("Cannot get default mic config: {e}");
            return (None, None, name, ret_err, ret_sil, ret_ctr, None);
        }
    };

    let sample_rate: u32 = default_cfg.sample_rate();
    let channels: u16 = default_cfg.channels();
    let sample_format = default_cfg.sample_format();
    info!("Mic config: {sample_rate}Hz, {channels}ch, format={sample_format:?}");

    let config = cpal::StreamConfig {
        channels,
        sample_rate,
        buffer_size: cpal::BufferSize::Default,
    };

    let shared = Arc::new(Mutex::new(MicPitchData::new(sample_rate)));
    let shared_cb = Arc::clone(&shared);
    let shared_detect = Arc::clone(&shared);

    let counter_cb = Arc::clone(&sample_counter);

    let push_samples: Arc<dyn Fn(&[f32]) + Send + Sync> = Arc::new(move |data: &[f32]| {
        if let Ok(mut lock) = shared_cb.try_lock() {
            let ch = channels as usize;
            for chunk in data.chunks(ch) {
                let mono = chunk.iter().sum::<f32>() / ch as f32;
                lock.samples.push_back(mono);
            }
            while lock.samples.len() > PITCH_WINDOW * 2 {
                lock.samples.pop_front();
            }
        }
        counter_cb.fetch_add(data.len() as u64, Ordering::Relaxed);
    });

    let err_flag_cb = Arc::clone(&error_flag);
    let make_err_cb = move || {
        let flag = Arc::clone(&err_flag_cb);
        move |err: cpal::StreamError| {
            error!("Microphone stream error: {err}");
            flag.store(true, Ordering::Relaxed);
        }
    };

    use cpal::SampleFormat;
    let stream = match sample_format {
        SampleFormat::F32 => {
            let push = push_samples.clone();
            device.build_input_stream(
                &config,
                move |data: &[f32], _: &cpal::InputCallbackInfo| push(data),
                make_err_cb(),
                None,
            )
        }
        SampleFormat::I16 => {
            let push = push_samples.clone();
            let mut float_buf: Vec<f32> = Vec::new();
            device.build_input_stream(
                &config,
                move |data: &[i16], _: &cpal::InputCallbackInfo| {
                    float_buf.clear();
                    float_buf.extend(data.iter().map(|&s| s as f32 / i16::MAX as f32));
                    push(&float_buf);
                },
                make_err_cb(),
                None,
            )
        }
        SampleFormat::I32 => {
            let push = push_samples.clone();
            let mut float_buf: Vec<f32> = Vec::new();
            device.build_input_stream(
                &config,
                move |data: &[i32], _: &cpal::InputCallbackInfo| {
                    float_buf.clear();
                    float_buf.extend(data.iter().map(|&s| s as f32 / i32::MAX as f32));
                    push(&float_buf);
                },
                make_err_cb(),
                None,
            )
        }
        other => {
            warn!("Unsupported mic sample format: {other:?}");
            return (Some(shared), None, name, error_flag, silence_flag, sample_counter, None);
        }
    };

    let stream = match stream {
        Ok(s) => s,
        Err(e) => {
            warn!("Failed to build mic stream: {e}");
            return (Some(shared), None, name, error_flag, silence_flag, sample_counter, None);
        }
    };

    if let Err(e) = stream.play() {
        warn!("Failed to start mic stream: {e}");
        return (Some(shared), None, name, error_flag, silence_flag, sample_counter, None);
    }

    let detect_counter = Arc::clone(&sample_counter);
    let detect_shutdown = Arc::clone(&shutdown);
    let detect_silence = Arc::clone(&silence_flag);
    let pitch_thread = std::thread::spawn(move || {
        pitch_detection_loop(shared_detect, detect_counter, detect_shutdown, detect_silence);
    });

    (Some(shared), Some(stream), name, error_flag, silence_flag, sample_counter, Some(pitch_thread))
}

const SILENCE_CHECK_MS: u64 = 1500;
const SILENCE_RMS_THRESHOLD: f32 = 0.001;

fn pitch_detection_loop(
    shared: Arc<Mutex<MicPitchData>>,
    sample_counter: Arc<AtomicU64>,
    shutdown: Arc<AtomicBool>,
    silence_flag: Arc<AtomicBool>,
) {
    let sleep_dur = std::time::Duration::from_millis(25);

    std::thread::sleep(std::time::Duration::from_millis(SILENCE_CHECK_MS));
    if shutdown.load(Ordering::Relaxed) {
        return;
    }
    let count = sample_counter.load(Ordering::Relaxed);
    if count == 0 {
        error!("Mic: no samples received after {SILENCE_CHECK_MS}ms — mic may be blocked or muted");
        silence_flag.store(true, Ordering::Relaxed);
        return;
    }

    let is_silent = {
        let Ok(lock) = shared.lock() else { return };
        if lock.samples.is_empty() {
            true
        } else {
            let samples: Vec<f32> = lock.samples.iter().copied().collect();
            rms(&samples) < SILENCE_RMS_THRESHOLD
        }
    };

    if is_silent {
        warn!("Mic: only silence detected after {SILENCE_CHECK_MS}ms — no real microphone connected");
        silence_flag.store(true, Ordering::Relaxed);
        return;
    }

    info!("Mic: received {count} samples in first {SILENCE_CHECK_MS}ms, pitch detection starting");

    let mut detector = McLeodDetector::new(PITCH_WINDOW, PITCH_WINDOW / 2);
    let mut detect_count: u64 = 0;
    let mut hit_count: u64 = 0;

    loop {
        std::thread::sleep(sleep_dur);

        if shutdown.load(Ordering::Relaxed) {
            info!("Mic pitch detection thread shutting down");
            return;
        }

        let (window, sr) = {
            let Ok(lock) = shared.lock() else { return };
            if lock.samples.len() < PITCH_WINDOW {
                continue;
            }
            let start = lock.samples.len() - PITCH_WINDOW;
            let w: Vec<f32> = lock.samples.range(start..).copied().collect();
            (w, lock.sample_rate)
        };

        let pitch = if rms(&window) < MIC_RMS_GATE {
            None
        } else {
            detector
                .get_pitch(&window, sr as usize, PITCH_POWER_THRESHOLD, PITCH_CLARITY_THRESHOLD)
                .filter(|p| p.frequency >= MIN_PITCH_HZ && p.frequency <= MAX_PITCH_HZ)
                .map(|p| p.frequency)
        };

        if let Ok(mut lock) = shared.lock() {
            lock.latest_pitch = pitch;
        }

        detect_count += 1;
        if pitch.is_some() {
            hit_count += 1;
        }
        if detect_count % 200 == 0 {
            info!("Mic pitch: {hit_count}/{detect_count} detections, latest={pitch:?}");
        }
    }
}

pub fn detect_pitch_from_samples(samples: &[f32], sample_rate: u32) -> Option<f32> {
    if samples.len() < PITCH_WINDOW {
        return None;
    }
    let window = &samples[samples.len() - PITCH_WINDOW..];
    if rms(window) < REF_RMS_GATE {
        return None;
    }
    let mut detector = McLeodDetector::new(PITCH_WINDOW, PITCH_WINDOW / 2);
    detector
        .get_pitch(window, sample_rate as usize, PITCH_POWER_THRESHOLD, PITCH_CLARITY_THRESHOLD)
        .filter(|p| p.frequency >= MIN_PITCH_HZ && p.frequency <= MAX_PITCH_HZ)
        .map(|p| p.frequency)
}
