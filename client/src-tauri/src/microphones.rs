mod reactive;

use cpal::device_description::DeviceType;
use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use pitch_detection::detector::mcleod::McLeodDetector;
use pitch_detection::detector::PitchDetector;
use serde::{Deserialize, Serialize};
use tracing::{info, warn};
use std::collections::{HashSet, VecDeque};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use tauri::{AppHandle, Emitter};
use ts_rs::TS;

use reactive::ReactiveAnalyzer;

const PITCH_WINDOW: usize = 2048;
const AUDIO_QUEUE_CAP: usize = 24_000;
const MONITOR_GAIN: f32 = 0.65;
const MIN_PITCH_HZ: f32 = 80.0;
const MAX_PITCH_HZ: f32 = 1000.0;
const PITCH_POWER_THRESHOLD: f32 = 0.2;
const PITCH_CLARITY_THRESHOLD: f32 = 0.4;
const MIC_RMS_GATE: f32 = 0.012;

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct MicrophoneInfo {
    pub name: String,
}

fn device_display_name(device: &cpal::Device) -> String {
    let Ok(desc) = device.description() else {
        return "(unknown)".into();
    };
    if let Some(friendly) = desc.extended().first() {
        return friendly.clone();
    }
    desc.to_string()
}

fn is_virtual(device: &cpal::Device) -> bool {
    let Ok(desc) = device.description() else {
        return false;
    };
    matches!(desc.device_type(), DeviceType::Virtual)
}

#[tauri::command]
pub fn list_microphones() -> Result<Vec<MicrophoneInfo>, String> {
    let host = cpal::default_host();
    let devices = host
        .input_devices()
        .map_err(|e| format!("input devices: {e}"))?;

    let mut seen: HashSet<String> = HashSet::new();
    let mut out = Vec::new();

    for device in devices {
        if device.default_input_config().is_err() || is_virtual(&device) {
            continue;
        }
        let name = device_display_name(&device);
        let key = name.to_lowercase();
        if seen.insert(key) {
            out.push(MicrophoneInfo { name });
        }
    }

    Ok(out)
}

fn rms(samples: &[f32]) -> f32 {
    if samples.is_empty() {
        return 0.0;
    }
    let sum_sq: f32 = samples.iter().map(|s| s * s).sum();
    (sum_sq / samples.len() as f32).sqrt()
}

fn i16_to_f32(sample: i16) -> f32 {
    sample as f32 / i16::MAX as f32
}

fn i32_to_f32(sample: i32) -> f32 {
    sample as f32 / i32::MAX as f32
}

fn f32_to_i16(sample: f32) -> i16 {
    (sample.clamp(-1.0, 1.0) * i16::MAX as f32) as i16
}

fn f32_to_u16(sample: f32) -> u16 {
    ((sample.clamp(-1.0, 1.0) * 0.5 + 0.5) * u16::MAX as f32) as u16
}

fn f32_to_i32(sample: f32) -> i32 {
    (sample.clamp(-1.0, 1.0) * i32::MAX as f32) as i32
}

fn f32_to_u32(sample: f32) -> u32 {
    ((sample.clamp(-1.0, 1.0) * 0.5 + 0.5) * u32::MAX as f32) as u32
}

fn push_mapped_input<T, F>(
    data: &[T],
    push: &Arc<dyn Fn(&[f32]) + Send + Sync>,
    mut map: F,
)
where
    T: Copy,
    F: FnMut(T) -> f32,
{
    let floats: Vec<f32> = data.iter().copied().map(&mut map).collect();
    push(&floats);
}

fn write_output_frames<T, F>(
    data: &mut [T],
    channels: usize,
    next_sample: &Arc<dyn Fn() -> f32 + Send + Sync>,
    mut map: F,
)
where
    T: Copy,
    F: FnMut(f32) -> T,
{
    for frame in data.chunks_mut(channels) {
        let out_sample = map(next_sample());
        for out in frame {
            *out = out_sample;
        }
    }
}

#[derive(Debug, Clone, Serialize)]
struct MicPitchEvent {
    pitch: Option<f32>,
}

#[derive(Debug, Clone, Serialize, TS)]
#[ts(export)]
#[allow(dead_code)]
pub struct MicAudioEvent {
    sample_rate: u32,
    samples: Vec<f32>,
}

#[derive(Debug, Clone, Deserialize, Serialize, TS)]
#[ts(export)]
#[serde(default)]
pub struct MicCaptureOptions {
    pub emit_pitch: bool,
    pub emit_audio: bool,
    pub emit_reactive: bool,
}

impl Default for MicCaptureOptions {
    fn default() -> Self {
        Self {
            emit_pitch: true,
            emit_audio: false,
            emit_reactive: false,
        }
    }
}

static MIC_RUNNING: AtomicBool = AtomicBool::new(false);
static MIC_SHUTDOWN: once_cell::sync::Lazy<Arc<AtomicBool>> =
    once_cell::sync::Lazy::new(|| Arc::new(AtomicBool::new(false)));
static MIC_OPTIONS: once_cell::sync::Lazy<Arc<Mutex<MicCaptureOptions>>> =
    once_cell::sync::Lazy::new(|| Arc::new(Mutex::new(MicCaptureOptions::default())));

fn find_device(preferred: Option<&str>) -> Result<(cpal::Device, String), String> {
    let host = cpal::default_host();

    if let Some(name) = preferred {
        let devices = host
            .input_devices()
            .map_err(|e| format!("input devices: {e}"))?;
        for dev in devices {
            if device_display_name(&dev) == name {
                return Ok((dev, name.to_string()));
            }
        }
        return Err(format!("Microphone '{name}' not found"));
    }

    let device = host
        .default_input_device()
        .ok_or_else(|| "No default microphone found".to_string())?;
    let name = device_display_name(&device);
    Ok((device, name))
}

#[tauri::command]
pub fn start_mic_capture(
    app: AppHandle,
    preferred: Option<String>,
    options: Option<MicCaptureOptions>,
) -> Result<String, String> {
    let next_options = options.unwrap_or_default();
    if let Ok(mut opts) = MIC_OPTIONS.lock() {
        *opts = next_options;
    }

    if MIC_RUNNING.swap(true, Ordering::SeqCst) {
        MIC_SHUTDOWN.store(false, Ordering::SeqCst);
        return Ok("already running".into());
    }

    MIC_SHUTDOWN.store(false, Ordering::SeqCst);

    let (device, name) = find_device(preferred.as_deref()).map_err(|e| {
        MIC_RUNNING.store(false, Ordering::SeqCst);
        e
    })?;

    let device_name = name.clone();
    let shutdown = Arc::clone(&MIC_SHUTDOWN);
    let options = Arc::clone(&MIC_OPTIONS);

    std::thread::spawn(move || {
        run_mic_loop(device, &name, app, shutdown, options);
        MIC_RUNNING.store(false, Ordering::SeqCst);
    });

    Ok(device_name)
}

fn try_build_stream(
    device: &cpal::Device,
    config: &cpal::StreamConfig,
    sample_format: cpal::SampleFormat,
    pitch_shared: Arc<Mutex<VecDeque<f32>>>,
    audio_shared: Arc<Mutex<VecDeque<f32>>>,
    reactive_shared: Arc<Mutex<VecDeque<f32>>>,
    options: Arc<Mutex<MicCaptureOptions>>,
) -> Option<cpal::Stream> {
    let ch = config.channels as usize;
    let push_samples: Arc<dyn Fn(&[f32]) + Send + Sync> = {
        let pitch_cb = Arc::clone(&pitch_shared);
        let audio_cb = Arc::clone(&audio_shared);
        let reactive_cb = Arc::clone(&reactive_shared);
        let options_cb = Arc::clone(&options);
        Arc::new(move |data: &[f32]| {
            let options = options_cb
                .lock()
                .map(|o| o.clone())
                .unwrap_or_else(|_| MicCaptureOptions::default());
            if !options.emit_pitch && !options.emit_audio && !options.emit_reactive {
                return;
            }

            let mut mono_samples = Vec::with_capacity(data.len() / ch.max(1));
            for chunk in data.chunks(ch) {
                mono_samples.push(chunk.iter().sum::<f32>() / ch as f32);
            }

            if options.emit_pitch {
                if let Ok(mut q) = pitch_cb.try_lock() {
                    for sample in &mono_samples {
                        q.push_back(*sample);
                    }
                    while q.len() > PITCH_WINDOW * 2 {
                        q.pop_front();
                    }
                }
            }

            if options.emit_audio {
                if let Ok(mut q) = audio_cb.try_lock() {
                    for sample in &mono_samples {
                        q.push_back(*sample);
                    }
                    while q.len() > AUDIO_QUEUE_CAP {
                        q.pop_front();
                    }
                }
            }

            if options.emit_reactive {
                if let Ok(mut q) = reactive_cb.try_lock() {
                    for sample in &mono_samples {
                        q.push_back(*sample);
                    }
                    while q.len() > reactive::QUEUE_CAP {
                        q.pop_front();
                    }
                }
            }
        })
    };

    use cpal::SampleFormat;
    let stream = match sample_format {
        SampleFormat::F32 => {
            let push = push_samples.clone();
            device.build_input_stream(
                config,
                move |data: &[f32], _: &cpal::InputCallbackInfo| push(data),
                |err| warn!("[mic] stream error: {err}"),
                None,
            )
        }
        SampleFormat::I16 => {
            let push = push_samples.clone();
            device.build_input_stream(
                config,
                move |data: &[i16], _: &cpal::InputCallbackInfo| {
                    push_mapped_input(data, &push, i16_to_f32);
                },
                |err| warn!("[mic] stream error: {err}"),
                None,
            )
        }
        SampleFormat::I32 => {
            let push = push_samples.clone();
            device.build_input_stream(
                config,
                move |data: &[i32], _: &cpal::InputCallbackInfo| {
                    push_mapped_input(data, &push, i32_to_f32);
                },
                |err| warn!("[mic] stream error: {err}"),
                None,
            )
        }
        _ => return None,
    };

    let stream = match stream {
        Ok(s) => s,
        Err(e) => {
            warn!("[mic] build stream failed: {e}");
            return None;
        }
    };

    if let Err(e) = stream.play() {
        warn!("[mic] play failed: {e}");
        return None;
    }

    Some(stream)
}

fn try_build_output_stream(
    device: &cpal::Device,
    audio_shared: Arc<Mutex<VecDeque<f32>>>,
    options: Arc<Mutex<MicCaptureOptions>>,
) -> Option<cpal::Stream> {
    let default_cfg = match device.default_output_config() {
        Ok(c) => c,
        Err(e) => {
            warn!("[mic] output config error: {e}");
            return None;
        }
    };
    let sample_format = default_cfg.sample_format();
    let config = cpal::StreamConfig {
        channels: default_cfg.channels(),
        sample_rate: default_cfg.sample_rate(),
        buffer_size: cpal::BufferSize::Default,
    };
    let ch = config.channels as usize;

    let next_sample: Arc<dyn Fn() -> f32 + Send + Sync> = {
        let options = Arc::clone(&options);
        let audio_shared = Arc::clone(&audio_shared);
        Arc::new(move || -> f32 {
            let emit_audio = options.lock().map(|o| o.emit_audio).unwrap_or(false);
            if !emit_audio {
                return 0.0;
            }
            if let Ok(mut q) = audio_shared.try_lock() {
                q.pop_front().unwrap_or(0.0) * MONITOR_GAIN
            } else {
                0.0
            }
        })
    };

    use cpal::SampleFormat;
    let stream = match sample_format {
        SampleFormat::F32 => {
            let next = Arc::clone(&next_sample);
            device.build_output_stream(
                &config,
                move |data: &mut [f32], _: &cpal::OutputCallbackInfo| {
                    write_output_frames(data, ch, &next, |sample| sample);
                },
                |err| warn!("[mic] output stream error: {err}"),
                None,
            )
        }
        SampleFormat::I16 => {
            let next = Arc::clone(&next_sample);
            device.build_output_stream(
                &config,
                move |data: &mut [i16], _: &cpal::OutputCallbackInfo| {
                    write_output_frames(data, ch, &next, f32_to_i16);
                },
                |err| warn!("[mic] output stream error: {err}"),
                None,
            )
        }
        SampleFormat::U16 => {
            let next = Arc::clone(&next_sample);
            device.build_output_stream(
                &config,
                move |data: &mut [u16], _: &cpal::OutputCallbackInfo| {
                    write_output_frames(data, ch, &next, f32_to_u16);
                },
                |err| warn!("[mic] output stream error: {err}"),
                None,
            )
        }
        SampleFormat::I32 => {
            let next = Arc::clone(&next_sample);
            device.build_output_stream(
                &config,
                move |data: &mut [i32], _: &cpal::OutputCallbackInfo| {
                    write_output_frames(data, ch, &next, f32_to_i32);
                },
                |err| warn!("[mic] output stream error: {err}"),
                None,
            )
        }
        SampleFormat::U32 => {
            let next = Arc::clone(&next_sample);
            device.build_output_stream(
                &config,
                move |data: &mut [u32], _: &cpal::OutputCallbackInfo| {
                    write_output_frames(data, ch, &next, f32_to_u32);
                },
                |err| warn!("[mic] output stream error: {err}"),
                None,
            )
        }
        _ => {
            warn!("[mic] unsupported output sample format: {sample_format:?}");
            return None;
        }
    };

    let stream = match stream {
        Ok(s) => s,
        Err(e) => {
            warn!("[mic] build output stream failed: {e}");
            return None;
        }
    };
    if let Err(e) = stream.play() {
        warn!("[mic] output play failed: {e}");
        return None;
    }
    Some(stream)
}

fn run_mic_loop(
    device: cpal::Device,
    name: &str,
    app: AppHandle,
    shutdown: Arc<AtomicBool>,
    options: Arc<Mutex<MicCaptureOptions>>,
) {
    let default_cfg = match device.default_input_config() {
        Ok(c) => c,
        Err(e) => {
            warn!("[mic] '{name}' config error: {e}");
            return;
        }
    };

    let sample_format = default_cfg.sample_format();
    let config = cpal::StreamConfig {
        channels: default_cfg.channels(),
        sample_rate: default_cfg.sample_rate(),
        buffer_size: cpal::BufferSize::Default,
    };
    let sr = config.sample_rate as usize;

    info!(
        "[mic] opening '{name}': {sr} Hz, {}ch, {sample_format:?}",
        config.channels
    );

    let pitch_shared = Arc::new(Mutex::new(VecDeque::<f32>::with_capacity(PITCH_WINDOW * 2)));
    let audio_shared = Arc::new(Mutex::new(VecDeque::<f32>::with_capacity(AUDIO_QUEUE_CAP)));
    let reactive_shared =
        Arc::new(Mutex::new(VecDeque::<f32>::with_capacity(reactive::QUEUE_CAP)));
    let Some(_stream) = try_build_stream(
        &device,
        &config,
        sample_format,
        Arc::clone(&pitch_shared),
        Arc::clone(&audio_shared),
        Arc::clone(&reactive_shared),
        Arc::clone(&options),
    ) else {
        warn!("[mic] failed to open '{name}'");
        return;
    };
    let monitor_stream = cpal::default_host()
        .default_output_device()
        .and_then(|output_device| {
            try_build_output_stream(&output_device, Arc::clone(&audio_shared), Arc::clone(&options))
        });
    if monitor_stream.is_none() {
        warn!("[mic] no output monitoring stream available");
    }

    info!("[mic] active: {name}");

    let mut detector = McLeodDetector::new(PITCH_WINDOW, PITCH_WINDOW / 2);
    let mut analyzer = ReactiveAnalyzer::new(MIN_PITCH_HZ, MAX_PITCH_HZ, MIC_RMS_GATE);
    let mut last_reactive_emit =
        std::time::Instant::now() - std::time::Duration::from_millis(reactive::EMIT_PERIOD_MS);
    let sleep_dur = std::time::Duration::from_millis(4);

    loop {
        std::thread::sleep(sleep_dur);

        if shutdown.load(Ordering::Relaxed) {
            break;
        }

        let opts = options
            .lock()
            .map(|o| o.clone())
            .unwrap_or_else(|_| MicCaptureOptions::default());

        if opts.emit_pitch {
            let window = {
                let Ok(q) = pitch_shared.lock() else { break };
                if q.len() < PITCH_WINDOW {
                    Vec::new()
                } else {
                    let start = q.len() - PITCH_WINDOW;
                    q.range(start..).copied().collect::<Vec<_>>()
                }
            };

            if !window.is_empty() {
                let pitch = if rms(&window) < MIC_RMS_GATE {
                    None
                } else {
                    detector
                        .get_pitch(&window, sr, PITCH_POWER_THRESHOLD, PITCH_CLARITY_THRESHOLD)
                        .filter(|p| p.frequency >= MIN_PITCH_HZ && p.frequency <= MAX_PITCH_HZ)
                        .map(|p| p.frequency)
                };
                let _ = app.emit("mic-pitch", MicPitchEvent { pitch });
            }
        }

        if opts.emit_reactive
            && last_reactive_emit.elapsed()
                >= std::time::Duration::from_millis(reactive::EMIT_PERIOD_MS)
        {
            let window = {
                let Ok(q) = reactive_shared.lock() else { break };
                if q.len() < reactive::FFT_SIZE {
                    Vec::new()
                } else {
                    let start = q.len() - reactive::FFT_SIZE;
                    q.range(start..).copied().collect::<Vec<_>>()
                }
            };

            if !window.is_empty() {
                let event = analyzer.analyze(&window, sr as u32);
                let _ = app.emit("mic-reactive", event);
                last_reactive_emit = std::time::Instant::now();
            }
        }
    }
}

#[tauri::command]
pub fn stop_mic_capture() {
    MIC_SHUTDOWN.store(true, Ordering::SeqCst);
}
