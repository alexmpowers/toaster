//! Spectral clarity features for disfluency survivor scoring.
//!
//! Our v1 `articulation_score` (peak / RMS / silence) is really a
//! loudness proxy — two equally loud takes with different intelligibility
//! tie. These features fix that:
//!
//! * **Spectral flatness (Wiener entropy)**: measures tonal-vs-noisy.
//!   Clear voiced speech has low flatness (concentrated formants);
//!   mumbled / breathy speech trends higher.
//! * **HF-to-LF energy ratio**: crisp consonants live above ~2 kHz.
//!   A clear "the" has measurably more HF energy than a mumbled "the".
//! * **Spectral centroid stddev**: articulation creates centroid motion
//!   (formant transitions). A monotone mumble has a near-static centroid.
//!
//! All features are in `[0.0, 1.0]` after normalization so they compose
//! cleanly with the existing `articulation_score` weighting. No
//! positional bias.
//!
//! Implementation uses `rustfft` (MIT/Apache-2.0). FFT size = 512 with
//! 50% hop (256). A Hann window is applied before each FFT. Inputs
//! shorter than one frame produce a neutral (0.5) score so we never
//! panic or divide-by-zero.

use rustfft::num_complex::Complex32;
use rustfft::FftPlanner;

const FFT_SIZE: usize = 512;
const HOP_SIZE: usize = 256;

/// Pre-computed features over a single word-window buffer.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SpectralClarity {
    /// Inverse spectral flatness normalized to [0,1]. Higher = more tonal / clearer.
    pub tonal: f32,
    /// HF (>= 2 kHz) to total energy ratio in [0,1]. Higher = crisper.
    pub hf_ratio: f32,
    /// Centroid motion (stddev normalized by Nyquist) in [0,1]. Higher = more articulation.
    pub centroid_motion: f32,
    /// Combined clarity in [0,1] — equal-weighted mean of the three.
    pub score: f32,
    /// Number of valid frames analyzed.
    pub frames: u32,
}

impl SpectralClarity {
    pub fn neutral() -> Self {
        Self {
            tonal: 0.5,
            hf_ratio: 0.5,
            centroid_motion: 0.5,
            score: 0.5,
            frames: 0,
        }
    }
}

/// Analyze a mono f32 window and return a clarity report.
///
/// `sample_rate_hz` is used to locate the 2 kHz cutoff for the HF band.
pub fn analyze(samples: &[f32], sample_rate_hz: u32) -> SpectralClarity {
    if samples.len() < FFT_SIZE || sample_rate_hz == 0 {
        return SpectralClarity::neutral();
    }

    let window = hann_window(FFT_SIZE);
    let mut planner = FftPlanner::<f32>::new();
    let fft = planner.plan_fft_forward(FFT_SIZE);

    let hf_bin = hz_to_bin(2_000.0, sample_rate_hz, FFT_SIZE);

    let mut flatness_accum = 0.0f64;
    let mut hf_ratio_accum = 0.0f64;
    let mut centroids: Vec<f32> = Vec::new();
    let mut frames: u32 = 0;

    let mut buffer = vec![Complex32::new(0.0, 0.0); FFT_SIZE];
    let mut frame_start = 0usize;
    while frame_start + FFT_SIZE <= samples.len() {
        for i in 0..FFT_SIZE {
            let sample = samples[frame_start + i] * window[i];
            buffer[i] = Complex32::new(sample, 0.0);
        }
        fft.process(&mut buffer);

        // Single-sided magnitude spectrum, skipping bin 0 (DC).
        let half = FFT_SIZE / 2;
        let mut mags = Vec::with_capacity(half);
        for bin in buffer.iter().take(half).skip(1) {
            mags.push(bin.norm());
        }

        let total_energy: f32 = mags.iter().map(|m| m * m).sum();
        if total_energy <= f32::EPSILON {
            frame_start += HOP_SIZE;
            continue;
        }

        // Spectral flatness: geometric_mean / arithmetic_mean.
        let (geo_log_sum, arith_sum) = mags
            .iter()
            .map(|m| m.max(1e-12))
            .fold((0.0f64, 0.0f64), |(g, a), m| {
                (g + (m as f64).ln(), a + m as f64)
            });
        let n = mags.len() as f64;
        let geo = (geo_log_sum / n).exp();
        let arith = arith_sum / n;
        let flatness = if arith > 0.0 { (geo / arith) as f32 } else { 1.0 };

        // HF / total energy ratio.
        let hf_energy: f32 = mags
            .iter()
            .enumerate()
            .filter(|(bin, _)| *bin + 1 >= hf_bin)
            .map(|(_, m)| m * m)
            .sum();
        let hf_ratio = (hf_energy / total_energy).clamp(0.0, 1.0);

        // Centroid in bins.
        let weighted: f32 = mags.iter().enumerate().map(|(i, m)| (i as f32) * m).sum();
        let centroid = weighted / mags.iter().sum::<f32>().max(f32::EPSILON);

        flatness_accum += flatness as f64;
        hf_ratio_accum += hf_ratio as f64;
        centroids.push(centroid);
        frames += 1;
        frame_start += HOP_SIZE;
    }

    if frames == 0 {
        return SpectralClarity::neutral();
    }

    let mean_flatness = (flatness_accum / frames as f64) as f32;
    let tonal = (1.0 - mean_flatness).clamp(0.0, 1.0);
    let hf_ratio = (hf_ratio_accum / frames as f64) as f32;

    // Normalize centroid stddev by Nyquist (= FFT_SIZE/2 bins).
    let mean_centroid = centroids.iter().sum::<f32>() / centroids.len() as f32;
    let var = centroids
        .iter()
        .map(|c| (c - mean_centroid).powi(2))
        .sum::<f32>()
        / centroids.len() as f32;
    let stddev = var.sqrt();
    let nyquist_bins = (FFT_SIZE / 2) as f32;
    let centroid_motion = (stddev / (nyquist_bins * 0.25)).clamp(0.0, 1.0);

    let score = ((tonal + hf_ratio + centroid_motion) / 3.0).clamp(0.0, 1.0);

    SpectralClarity {
        tonal,
        hf_ratio,
        centroid_motion,
        score,
        frames,
    }
}

fn hann_window(n: usize) -> Vec<f32> {
    (0..n)
        .map(|i| {
            let x = std::f32::consts::PI * 2.0 * (i as f32) / ((n - 1) as f32);
            0.5 * (1.0 - x.cos())
        })
        .collect()
}

fn hz_to_bin(freq_hz: f32, sample_rate_hz: u32, fft_size: usize) -> usize {
    ((freq_hz * fft_size as f32) / sample_rate_hz as f32) as usize
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::f32::consts::TAU;

    fn sine(freq_hz: f32, sr: u32, samples: usize, amp: f32) -> Vec<f32> {
        (0..samples)
            .map(|i| amp * (TAU * freq_hz * (i as f32) / sr as f32).sin())
            .collect()
    }

    fn white_noise(samples: usize, amp: f32) -> Vec<f32> {
        // Deterministic LCG so tests are reproducible.
        let mut state: u64 = 0x5EED_BEEFu64;
        (0..samples)
            .map(|_| {
                state = state.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
                let x = ((state >> 33) as i32 as f32) / (i32::MAX as f32);
                x * amp
            })
            .collect()
    }

    #[test]
    fn too_short_returns_neutral() {
        let s = SpectralClarity::neutral();
        let got = analyze(&vec![0.1f32; 100], 16_000);
        assert_eq!(got.score, s.score);
        assert_eq!(got.frames, 0);
    }

    #[test]
    fn tonal_signal_scores_higher_than_noise() {
        // 1 kHz pure tone vs white noise at the same RMS — tonal must
        // score strictly higher. This is the defining test for the
        // "clarity != loudness" refactor.
        let sr = 16_000u32;
        let tone = sine(1_000.0, sr, 16_000, 0.3);
        let noise = white_noise(16_000, 0.3);
        let t = analyze(&tone, sr);
        let n = analyze(&noise, sr);
        assert!(
            t.tonal > n.tonal,
            "tonal tone={} noise={}",
            t.tonal,
            n.tonal
        );
        // Combined score must also favour the tone.
        assert!(t.score > n.score, "score tone={} noise={}", t.score, n.score);
    }

    #[test]
    fn hf_ratio_higher_for_hf_content() {
        let sr = 16_000u32;
        let low = sine(200.0, sr, 16_000, 0.3);
        let high = sine(4_000.0, sr, 16_000, 0.3);
        let lo = analyze(&low, sr);
        let hi = analyze(&high, sr);
        assert!(
            hi.hf_ratio > lo.hf_ratio,
            "hf ratio low={} high={}",
            lo.hf_ratio,
            hi.hf_ratio
        );
    }

    #[test]
    fn silent_input_stays_neutral_and_never_nans() {
        let got = analyze(&vec![0.0f32; 16_000], 16_000);
        assert!(got.score.is_finite());
        assert!(got.tonal.is_finite());
        assert!(got.hf_ratio.is_finite());
        assert!(got.centroid_motion.is_finite());
    }
}
