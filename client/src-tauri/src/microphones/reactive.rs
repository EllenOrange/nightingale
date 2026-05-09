use std::sync::Arc;

use rustfft::num_complex::Complex32;
use rustfft::FftPlanner;
use serde::Serialize;
use ts_rs::TS;

pub const FFT_SIZE: usize = 1024;
pub const QUEUE_CAP: usize = 4096;
pub const EMIT_PERIOD_MS: u64 = 16;
pub const EMIT_DT_SEC: f32 = EMIT_PERIOD_MS as f32 / 1000.0;

const WAVE_BINS: usize = 256;
const LOW_HZ: f32 = 250.0;
const MID_HZ: f32 = 2000.0;

const FAST_SMOOTH_ALPHA: f32 = 0.35;

const RMS_NORM_GAIN: f32 = 6.0;
const BAND_NORM_GAIN: f32 = 8.0;

const SLOW_TAU_ENERGY_SEC: f32 = 0.3;
const SLOW_TAU_TONE_SEC: f32 = 0.7;
const SLOW_TAU_BLEND_SEC: f32 = 0.4;

const ENERGY_VOLUME_WEIGHT: f32 = 0.7;
const ENERGY_BAND_WEIGHT: f32 = 0.3;

const HUE_PERIOD_SEC: f32 = 14.0;
const HUE_PITCH_GAIN: f32 = 1.2;
const HUE_VOLUME_GAIN: f32 = 0.6;

const FLOW_PERIOD_SEC: f32 = 30.0;
const FLOW_CENTROID_GAIN: f32 = 1.2;
const FLOW_ENERGY_GAIN: f32 = 0.7;

const DEFAULT_NEUTRAL: f32 = 0.5;

/// Shader-ready reactive frame. All values are slow-smoothed and ready to
/// pipe straight into shader uniforms — no second-stage processing on the
/// frontend. `hue` is a 0..1 phase, `flow` is a radians 0..2π angle, and
/// `wave` is the downsampled time-domain waveform for the oscilloscope shader.
#[derive(Debug, Clone, Serialize, TS)]
#[ts(export)]
pub struct MicReactiveEvent {
    pub volume: f32,
    pub low: f32,
    pub mid: f32,
    pub high: f32,
    pub centroid: f32,
    pub pitch: f32,
    pub energy: f32,
    pub hue: f32,
    pub flow: f32,
    pub wave: Vec<f32>,
}

pub struct ReactiveAnalyzer {
    fft: Arc<dyn rustfft::Fft<f32>>,
    window: Vec<f32>,
    scratch: Vec<Complex32>,
    rms_fast: f32,
    centroid_fast: f32,
    low_fast: f32,
    mid_fast: f32,
    high_fast: f32,
    pitch_fast: f32,
    volume_slow: f32,
    low_slow: f32,
    mid_slow: f32,
    high_slow: f32,
    centroid_slow: f32,
    pitch_slow: f32,
    energy_slow: f32,
    hue_phase: f32,
    flow_angle: f32,
    min_pitch_hz: f32,
    max_pitch_hz: f32,
    rms_gate: f32,
}

impl ReactiveAnalyzer {
    pub fn new(min_pitch_hz: f32, max_pitch_hz: f32, rms_gate: f32) -> Self {
        let mut planner = FftPlanner::<f32>::new();
        let fft = planner.plan_fft_forward(FFT_SIZE);
        let window = (0..FFT_SIZE)
            .map(|i| {
                let t = i as f32 / (FFT_SIZE as f32 - 1.0);
                0.5 - 0.5 * (std::f32::consts::TAU * t).cos()
            })
            .collect();
        Self {
            fft,
            window,
            scratch: vec![Complex32::default(); FFT_SIZE],
            rms_fast: 0.0,
            centroid_fast: 0.0,
            low_fast: 0.0,
            mid_fast: 0.0,
            high_fast: 0.0,
            pitch_fast: DEFAULT_NEUTRAL,
            volume_slow: 0.0,
            low_slow: 0.0,
            mid_slow: 0.0,
            high_slow: 0.0,
            centroid_slow: DEFAULT_NEUTRAL,
            pitch_slow: DEFAULT_NEUTRAL,
            energy_slow: 0.0,
            hue_phase: 0.0,
            flow_angle: 0.0,
            min_pitch_hz,
            max_pitch_hz,
            rms_gate,
        }
    }

    pub fn analyze(&mut self, samples: &[f32], sample_rate: u32) -> MicReactiveEvent {
        debug_assert_eq!(samples.len(), FFT_SIZE);

        let mut sum_sq = 0.0f32;
        for (i, sample) in samples.iter().enumerate() {
            let s = *sample;
            sum_sq += s * s;
            self.scratch[i] = Complex32::new(s * self.window[i], 0.0);
        }
        let raw_rms = (sum_sq / samples.len() as f32).sqrt();

        self.fft.process(&mut self.scratch);

        let bin_count = FFT_SIZE / 2;
        let bin_hz = sample_rate as f32 / FFT_SIZE as f32;
        let mag_norm = 1.0 / (FFT_SIZE as f32 * 0.5);

        let mut low_sum = 0.0f32;
        let mut mid_sum = 0.0f32;
        let mut high_sum = 0.0f32;
        let mut low_count = 0u32;
        let mut mid_count = 0u32;
        let mut high_count = 0u32;
        let mut centroid_num = 0.0f32;
        let mut centroid_den = 0.0f32;
        let mut peak_bin = 0usize;
        let mut peak_mag = 0.0f32;

        for i in 0..bin_count {
            let mag = self.scratch[i].norm() * mag_norm;
            let freq = i as f32 * bin_hz;

            if freq < LOW_HZ {
                low_sum += mag;
                low_count += 1;
            } else if freq < MID_HZ {
                mid_sum += mag;
                mid_count += 1;
            } else {
                high_sum += mag;
                high_count += 1;
            }

            centroid_num += freq * mag;
            centroid_den += mag;

            if mag > peak_mag {
                peak_mag = mag;
                peak_bin = i;
            }
        }

        let nyquist = sample_rate as f32 * 0.5;
        let centroid_raw = if centroid_den > 1e-6 {
            (centroid_num / centroid_den / nyquist).clamp(0.0, 1.0)
        } else {
            0.0
        };

        let band_norm = |sum: f32, count: u32| -> f32 {
            if count == 0 {
                0.0
            } else {
                (sum / count as f32 * BAND_NORM_GAIN).clamp(0.0, 1.0)
            }
        };
        let low_raw = band_norm(low_sum, low_count);
        let mid_raw = band_norm(mid_sum, mid_count);
        let high_raw = band_norm(high_sum, high_count);
        let rms_raw = (raw_rms * RMS_NORM_GAIN).clamp(0.0, 1.0);

        let lerp = |prev: f32, next: f32, alpha: f32| prev + alpha * (next - prev);
        self.rms_fast = lerp(self.rms_fast, rms_raw, FAST_SMOOTH_ALPHA);
        self.centroid_fast = lerp(self.centroid_fast, centroid_raw, FAST_SMOOTH_ALPHA);
        self.low_fast = lerp(self.low_fast, low_raw, FAST_SMOOTH_ALPHA);
        self.mid_fast = lerp(self.mid_fast, mid_raw, FAST_SMOOTH_ALPHA);
        self.high_fast = lerp(self.high_fast, high_raw, FAST_SMOOTH_ALPHA);

        let pitch_norm_raw = if peak_mag > 0.0 && raw_rms > self.rms_gate {
            let f = peak_bin as f32 * bin_hz;
            if (self.min_pitch_hz..=self.max_pitch_hz).contains(&f) {
                Some(((f - self.min_pitch_hz) / (self.max_pitch_hz - self.min_pitch_hz))
                    .clamp(0.0, 1.0))
            } else {
                None
            }
        } else {
            None
        };
        if let Some(p) = pitch_norm_raw {
            self.pitch_fast = lerp(self.pitch_fast, p, FAST_SMOOTH_ALPHA);
        }

        let alpha_from_tau = |tau_sec: f32| -> f32 {
            1.0 - (-EMIT_DT_SEC / tau_sec.max(1e-6)).exp()
        };
        let alpha_energy = alpha_from_tau(SLOW_TAU_ENERGY_SEC);
        let alpha_tone = alpha_from_tau(SLOW_TAU_TONE_SEC);
        let alpha_blend = alpha_from_tau(SLOW_TAU_BLEND_SEC);

        self.volume_slow = lerp(self.volume_slow, self.rms_fast, alpha_energy);
        self.low_slow = lerp(self.low_slow, self.low_fast, alpha_energy);
        self.mid_slow = lerp(self.mid_slow, self.mid_fast, alpha_energy);
        self.high_slow = lerp(self.high_slow, self.high_fast, alpha_energy);
        self.centroid_slow = lerp(self.centroid_slow, self.centroid_fast, alpha_tone);
        if pitch_norm_raw.is_some() {
            self.pitch_slow = lerp(self.pitch_slow, self.pitch_fast, alpha_tone);
        }

        let energy_target = (self.volume_slow * ENERGY_VOLUME_WEIGHT
            + (self.mid_slow + self.high_slow) * ENERGY_BAND_WEIGHT)
            .clamp(0.0, 1.0);
        self.energy_slow = lerp(self.energy_slow, energy_target, alpha_blend);

        // Phase accumulators are deliberately NOT wrapped: shaders consume
        // them as `uHue * k` / `uFlow * k` for various k, so a wrap at 1.0
        // (or TAU) shows up as a visible jump every period since `k * wrap`
        // isn't itself a full cycle. f32 precision stays comfortable at
        // these slow rates over realistic session lengths.
        let hue_rate = (1.0 / HUE_PERIOD_SEC)
            * (1.0
                + (self.pitch_slow - DEFAULT_NEUTRAL) * HUE_PITCH_GAIN
                + self.volume_slow * HUE_VOLUME_GAIN);
        self.hue_phase += hue_rate * EMIT_DT_SEC;

        let flow_rate = (std::f32::consts::TAU / FLOW_PERIOD_SEC)
            * (1.0
                + (self.centroid_slow - DEFAULT_NEUTRAL) * FLOW_CENTROID_GAIN
                + self.energy_slow * FLOW_ENERGY_GAIN);
        self.flow_angle += flow_rate * EMIT_DT_SEC;

        let wave = downsample_wave(samples, WAVE_BINS);

        MicReactiveEvent {
            volume: self.volume_slow,
            low: self.low_slow,
            mid: self.mid_slow,
            high: self.high_slow,
            centroid: self.centroid_slow,
            pitch: self.pitch_slow,
            energy: self.energy_slow,
            hue: self.hue_phase,
            flow: self.flow_angle,
            wave,
        }
    }
}

fn downsample_wave(samples: &[f32], bins: usize) -> Vec<f32> {
    if bins == 0 {
        return Vec::new();
    }
    let mut out = Vec::with_capacity(bins);
    let chunk = samples.len() / bins;
    if chunk == 0 {
        out.extend_from_slice(samples);
        out.resize(bins, 0.0);
        return out;
    }
    for b in 0..bins {
        let start = b * chunk;
        let end = if b + 1 == bins {
            samples.len()
        } else {
            start + chunk
        };
        let mut sum = 0.0f32;
        for s in &samples[start..end] {
            sum += *s;
        }
        out.push(sum / (end - start) as f32);
    }
    out
}
