use std::collections::VecDeque;
use std::sync::{Arc, Mutex};
use std::sync::atomic::{AtomicU64, Ordering};

use bevy::prelude::*;
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
    shared: Arc<Mutex<MicPitchData>>,
    _stream: Option<cpal::Stream>,
}

impl MicrophoneCapture {
    pub fn latest_pitch(&self) -> Option<f32> {
        if !self.active {
            return None;
        }
        self.shared.lock().ok()?.latest_pitch
    }
}

pub fn start_microphone() -> MicrophoneCapture {
    let (shared, stream) = try_build_stream();

    let shared = shared.unwrap_or_else(|| {
        warn!("No microphone found or permission denied; scoring disabled");
        Arc::new(Mutex::new(MicPitchData::new(44100)))
    });

    MicrophoneCapture {
        active: stream.is_some(),
        shared,
        _stream: stream,
    }
}

fn try_build_stream() -> (Option<Arc<Mutex<MicPitchData>>>, Option<cpal::Stream>) {
    let host = cpal::default_host();
    let device = match host.default_input_device() {
        Some(d) => d,
        None => return (None, None),
    };
    info!(
        "Microphone: {}",
        device.description().map(|d| d.name().to_string()).unwrap_or_default()
    );

    let default_cfg = match device.default_input_config() {
        Ok(c) => c,
        Err(e) => {
            warn!("Cannot get default mic config: {e}");
            return (None, None);
        }
    };

    let sample_rate: u32 = default_cfg.sample_rate();
    let channels: u16 = default_cfg.channels();
    info!("Mic config: {sample_rate}Hz, {channels}ch");

    let config = cpal::StreamConfig {
        channels,
        sample_rate,
        buffer_size: cpal::BufferSize::Default,
    };

    let shared = Arc::new(Mutex::new(MicPitchData::new(sample_rate)));
    let shared_cb = Arc::clone(&shared);
    let shared_detect = Arc::clone(&shared);

    let sample_counter = Arc::new(AtomicU64::new(0));
    let counter_cb = Arc::clone(&sample_counter);

    let stream = match device.build_input_stream(
        &config,
        move |data: &[f32], _: &cpal::InputCallbackInfo| {
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
        },
        |err| {
            error!("Microphone stream error: {err}");
        },
        None,
    ) {
        Ok(s) => s,
        Err(e) => {
            warn!("Failed to build mic stream: {e}");
            return (Some(shared), None);
        }
    };

    if let Err(e) = stream.play() {
        warn!("Failed to start mic stream: {e}");
        return (Some(shared), None);
    }

    std::thread::spawn(move || {
        pitch_detection_loop(shared_detect, sample_counter);
    });

    (Some(shared), Some(stream))
}

fn pitch_detection_loop(shared: Arc<Mutex<MicPitchData>>, sample_counter: Arc<AtomicU64>) {
    let sleep_dur = std::time::Duration::from_millis(25);

    std::thread::sleep(std::time::Duration::from_millis(500));
    let count = sample_counter.load(Ordering::Relaxed);
    if count == 0 {
        error!("Mic: no samples received after 500ms — mic may be blocked or muted");
    } else {
        info!("Mic: received {count} samples in first 500ms, pitch detection starting");
    }

    let mut detector = McLeodDetector::new(PITCH_WINDOW, PITCH_WINDOW / 2);
    let mut detect_count: u64 = 0;
    let mut hit_count: u64 = 0;

    loop {
        std::thread::sleep(sleep_dur);

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
