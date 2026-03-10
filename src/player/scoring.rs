use std::collections::VecDeque;
use std::path::{Path, PathBuf};
use std::sync::{Mutex, mpsc};

use bevy::prelude::*;

use super::audio;
use super::microphone::{MicrophoneCapture, detect_pitch_from_samples};
use audio::KaraokeAudio;
use bevy_kira_audio::AudioInstance;

const PITCH_BUFFER_SIZE: usize = 200;
const BASE_DISPLAY_WIDTH: f32 = 480.0;
const BASE_DISPLAY_HEIGHT: f32 = 56.0;
const BASE_DISPLAY_TOP_OFFSET: f32 = 55.0;
const REFERENCE_HEIGHT: f32 = 720.0;

pub fn display_scale(window_height: f32) -> f32 {
    (window_height / REFERENCE_HEIGHT).max(1.0)
}

pub fn display_width(window_height: f32) -> f32 {
    BASE_DISPLAY_WIDTH * display_scale(window_height)
}

pub fn display_height(window_height: f32) -> f32 {
    BASE_DISPLAY_HEIGHT * display_scale(window_height)
}

pub fn display_top_offset(window_height: f32) -> f32 {
    BASE_DISPLAY_TOP_OFFSET * display_scale(window_height)
}
const PUSH_INTERVAL: f64 = 0.02;
const SMOOTHING: f32 = 0.55;

const SEMITONE_TOLERANCE: f32 = 6.0;
const MIC_LATENCY_COMPENSATION: f64 = 0.08;

const GOOD_COLOR: Srgba = Srgba::new(0.2, 0.9, 0.3, 1.0);
const OK_COLOR: Srgba = Srgba::new(0.95, 0.8, 0.1, 1.0);
const BAD_COLOR: Srgba = Srgba::new(0.9, 0.2, 0.2, 1.0);

#[derive(Resource)]
pub struct VocalsBuffer {
    pub samples: Vec<f32>,
    pub sample_rate: u32,
    scratch: Vec<f32>,
}

impl VocalsBuffer {
    pub fn extract_window(&mut self, time_secs: f64, window_size: usize) -> Option<&[f32]> {
        let sample_idx = (time_secs * self.sample_rate as f64) as usize;
        if sample_idx + window_size > self.samples.len() {
            return None;
        }
        self.scratch.clear();
        self.scratch.extend_from_slice(&self.samples[sample_idx..sample_idx + window_size]);
        Some(&self.scratch)
    }
}

const VOCALS_SAMPLE_RATE: u32 = 44100;

pub fn load_vocals_buffer(path: &Path) -> Option<VocalsBuffer> {
    use std::process::{Command, Stdio};

    let result = Command::new(crate::vendor::ffmpeg_path())
        .args([
            "-i",
            path.to_str().unwrap_or(""),
            "-f", "f32le",
            "-ac", "1",
            "-ar", &VOCALS_SAMPLE_RATE.to_string(),
            "-v", "error",
            "-",
        ])
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output();

    let output = match result {
        Ok(o) => o,
        Err(e) => {
            warn!("Failed to run ffmpeg for vocals decoding: {e}");
            return None;
        }
    };

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        warn!("ffmpeg vocals decode failed: {stderr}");
        return None;
    }

    let byte_data = output.stdout;
    if byte_data.len() < 4 {
        warn!("ffmpeg returned no audio data for vocals");
        return None;
    }

    let samples: Vec<f32> = byte_data
        .chunks_exact(4)
        .map(|c| f32::from_le_bytes([c[0], c[1], c[2], c[3]]))
        .collect();

    info!(
        "Loaded vocals buffer: {} samples, {}Hz",
        samples.len(),
        VOCALS_SAMPLE_RATE
    );

    Some(VocalsBuffer {
        samples,
        sample_rate: VOCALS_SAMPLE_RATE,
        scratch: Vec::with_capacity(2048),
    })
}

#[derive(Resource)]
pub struct VocalsLoadTask {
    rx: Mutex<mpsc::Receiver<(VocalsBuffer, f64)>>,
}

pub fn spawn_vocals_load(commands: &mut Commands, vocals_path: PathBuf) {
    let (tx, rx) = mpsc::channel();
    std::thread::spawn(move || {
        if let Some(buf) = load_vocals_buffer(&vocals_path) {
            let singable = compute_singable_time(&buf);
            let _ = tx.send((buf, singable));
        }
    });
    commands.insert_resource(VocalsLoadTask { rx: Mutex::new(rx) });
}

pub fn poll_vocals_load(
    mut commands: Commands,
    task: Option<Res<VocalsLoadTask>>,
    mut scoring: Option<ResMut<ScoringState>>,
) {
    let Some(task) = task else { return };
    let Ok(rx) = task.rx.lock() else { return };
    if let Ok((buf, singable)) = rx.try_recv() {
        drop(rx);
        commands.insert_resource(buf);
        if let Some(ref mut s) = scoring {
            s.total_singable = singable;
        }
        commands.remove_resource::<VocalsLoadTask>();
    }
}

pub fn compute_singable_time(vocals: &VocalsBuffer) -> f64 {
    let window_size = 2048_usize;
    let hop = window_size / 2;
    let hop_secs = hop as f64 / vocals.sample_rate as f64;
    let mut total = 0.0;
    let mut offset = 0;
    while offset + window_size <= vocals.samples.len() {
        let window = &vocals.samples[offset..offset + window_size];
        if detect_pitch_from_samples(window, vocals.sample_rate).is_some() {
            total += hop_secs;
        }
        offset += hop;
    }
    total
}

#[derive(Resource)]
pub struct PitchState {
    pub ref_pitches: VecDeque<Option<f32>>,
    pub user_pitches: VecDeque<Option<f32>>,
    pub similarities: VecDeque<f32>,
    smoothed_ref: Option<f32>,
    smoothed_user: Option<f32>,
    last_push_time: f64,
}

impl Default for PitchState {
    fn default() -> Self {
        Self {
            ref_pitches: VecDeque::with_capacity(PITCH_BUFFER_SIZE),
            user_pitches: VecDeque::with_capacity(PITCH_BUFFER_SIZE),
            similarities: VecDeque::with_capacity(PITCH_BUFFER_SIZE),
            smoothed_ref: None,
            smoothed_user: None,
            last_push_time: 0.0,
        }
    }
}

impl PitchState {
    fn try_push(
        &mut self,
        ref_pitch: Option<f32>,
        user_pitch: Option<f32>,
        similarity: f32,
        time: f64,
    ) {
        self.smoothed_ref = ema(self.smoothed_ref, ref_pitch);
        self.smoothed_user = ema(self.smoothed_user, user_pitch);

        if time - self.last_push_time < PUSH_INTERVAL {
            return;
        }
        self.last_push_time = time;

        if self.ref_pitches.len() >= PITCH_BUFFER_SIZE {
            self.ref_pitches.pop_front();
            self.user_pitches.pop_front();
            self.similarities.pop_front();
        }
        self.ref_pitches.push_back(self.smoothed_ref);
        self.user_pitches.push_back(self.smoothed_user);
        self.similarities.push_back(similarity);
    }
}

fn ema(prev: Option<f32>, current: Option<f32>) -> Option<f32> {
    match (prev, current) {
        (Some(p), Some(c)) => Some(p * SMOOTHING + c * (1.0 - SMOOTHING)),
        (None, c) => c,
        (_, None) => None,
    }
}

#[derive(Resource)]
pub struct ScoringState {
    pub total_singable: f64,
    earned: f64,
    last_time: f64,
}

impl ScoringState {
    pub fn new(total_singable: f64) -> Self {
        Self {
            total_singable,
            earned: 0.0,
            last_time: 0.0,
        }
    }

    pub fn score(&self) -> u32 {
        if self.total_singable < 0.5 {
            return 0;
        }
        ((self.earned / self.total_singable) * 1000.0)
            .round()
            .clamp(0.0, 1000.0) as u32
    }
}

fn freq_to_semitone(hz: f32) -> f32 {
    12.0 * (hz / 440.0).log2() + 69.0
}

fn pitch_similarity(ref_hz: f32, user_hz: f32) -> f32 {
    let ref_semi = freq_to_semitone(ref_hz);
    let user_semi = freq_to_semitone(user_hz);
    let diff = (ref_semi - user_semi).abs() % 12.0;
    let distance = diff.min(12.0 - diff);
    (1.0 - distance / SEMITONE_TOLERANCE).max(0.0)
}

fn similarity_to_color(sim: f32) -> Srgba {
    if sim >= 0.7 {
        lerp_srgba(OK_COLOR, GOOD_COLOR, (sim - 0.7) / 0.3)
    } else {
        lerp_srgba(BAD_COLOR, OK_COLOR, (sim / 0.7).max(0.0))
    }
}

fn lerp_srgba(a: Srgba, b: Srgba, t: f32) -> Srgba {
    Srgba::new(
        a.red + (b.red - a.red) * t,
        a.green + (b.green - a.green) * t,
        a.blue + (b.blue - a.blue) * t,
        1.0,
    )
}

fn snap_to_ref_octave(ref_semi: f32, user_semi: f32) -> f32 {
    let diff = user_semi - ref_semi;
    let octave_offset = (diff / 12.0).round() * 12.0;
    user_semi - octave_offset
}

pub fn update_pitch_scoring(
    karaoke: Option<Res<KaraokeAudio>>,
    audio_instances: Res<Assets<AudioInstance>>,
    mic: Option<Res<MicrophoneCapture>>,
    mut vocals: Option<ResMut<VocalsBuffer>>,
    mut pitch_state: Option<ResMut<PitchState>>,
    mut scoring: Option<ResMut<ScoringState>>,
) {
    let (Some(karaoke), Some(mic), Some(pitch_state), Some(scoring)) =
        (karaoke, mic, pitch_state.as_mut(), scoring.as_mut())
    else {
        return;
    };

    if !mic.active {
        return;
    }

    let current_time = audio::playback_time(&karaoke, &audio_instances);
    if current_time <= 0.0 {
        return;
    }

    let ref_time = (current_time - MIC_LATENCY_COMPENSATION).max(0.0);
    let ref_pitch = vocals.as_mut().and_then(|v| {
        let sr = v.sample_rate;
        let window = v.extract_window(ref_time, 2048)?;
        detect_pitch_from_samples(window, sr)
    });

    let user_pitch = mic.latest_pitch();

    let similarity = match (ref_pitch, user_pitch) {
        (Some(r), Some(u)) => pitch_similarity(r, u),
        _ => 0.0,
    };

    pitch_state.try_push(ref_pitch, user_pitch, similarity, current_time);

    let dt = (current_time - scoring.last_time).clamp(0.0, 0.1);
    scoring.last_time = current_time;

    if ref_pitch.is_some() && user_pitch.is_some() {
        scoring.earned += similarity as f64 * dt;
    }
}

#[derive(Component)]
pub struct ScoreText;

#[derive(Component)]
pub struct MicStatusText;

pub fn draw_pitch_waves(
    mut gizmos: Gizmos,
    pitch_state: Option<Res<PitchState>>,
    mic: Option<Res<MicrophoneCapture>>,
    windows: Query<&Window>,
    theme: Res<crate::ui::UiTheme>,
    mut buf: Local<Vec<(Vec2, Color)>>,
) {
    let Some(state) = pitch_state else { return };
    let Some(mic) = mic else { return };
    if !mic.active {
        return;
    }

    let Ok(window) = windows.single() else {
        return;
    };
    let wh = window.height();
    let half_h = wh / 2.0;
    let dw = display_width(wh);
    let dh = display_height(wh);
    let dto = display_top_offset(wh);
    let center_y = half_h - dto;
    let len = state.ref_pitches.len();
    if len < 2 {
        return;
    }

    let semi_to_y = |semi: f32| -> f32 {
        let normalized = ((semi - 45.0) / 36.0).clamp(0.0, 1.0);
        center_y - dh / 2.0 + normalized * dh
    };

    let x_step = dw / (PITCH_BUFFER_SIZE as f32 - 1.0);
    let x_start = -dw / 2.0;
    let buf_offset = PITCH_BUFFER_SIZE.saturating_sub(len);
    let x_for = |i: usize| x_start + (buf_offset + i) as f32 * x_step;
    let age_alpha = |i: usize| {
        0.25 + 0.75 * (i as f32 / len.saturating_sub(1).max(1) as f32)
    };

    let ref_color = theme.pitch_ref_line;
    let user_base = theme.pitch_user_base;

    buf.clear();
    for i in 0..len {
        match state.ref_pitches[i] {
            Some(hz) => {
                let a = ref_color.alpha * age_alpha(i);
                buf.push((
                    Vec2::new(x_for(i), semi_to_y(freq_to_semitone(hz))),
                    Color::srgba(ref_color.red, ref_color.green, ref_color.blue, a),
                ));
            }
            None => {
                flush_run(&mut gizmos, &mut buf);
            }
        }
    }
    flush_run(&mut gizmos, &mut buf);

    buf.clear();
    for i in 0..len {
        match state.user_pitches[i] {
            Some(user_hz) => {
                let user_semi = freq_to_semitone(user_hz);
                let display_semi = match state.ref_pitches[i] {
                    Some(ref_hz) => {
                        snap_to_ref_octave(freq_to_semitone(ref_hz), user_semi)
                    }
                    None => user_semi,
                };

                let sim = state.similarities[i];
                let age = age_alpha(i);
                let prox = similarity_to_color(sim);
                let base = lerp_srgba(user_base, prox, sim);
                let alpha = (0.3 + sim * 0.7) * age;

                buf.push((
                    Vec2::new(x_for(i), semi_to_y(display_semi)),
                    Color::srgba(base.red, base.green, base.blue, alpha),
                ));
            }
            None => {
                flush_run(&mut gizmos, &mut buf);
            }
        }
    }
    flush_run(&mut gizmos, &mut buf);
}

fn flush_run(gizmos: &mut Gizmos, run: &mut Vec<(Vec2, Color)>) {
    if run.len() >= 2 {
        gizmos.linestrip_gradient_2d(std::mem::take(run));
    } else {
        run.clear();
    }
}

pub fn update_score_text(
    scoring: Option<Res<ScoringState>>,
    mic: Option<Res<MicrophoneCapture>>,
    mut query: Query<&mut Text, With<ScoreText>>,
) {
    let Some(scoring) = scoring else { return };
    let active = mic.as_ref().is_some_and(|m| m.active);
    if let Ok(mut text) = query.single_mut() {
        if active {
            **text = format!("Score: {}", scoring.score());
        } else {
            **text = "Score: --".into();
        }
    }
}
