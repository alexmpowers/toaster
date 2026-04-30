//! Audio-truth silence detection.
//!
//! Walks PCM samples in fixed-size windows, classifies each window as
//! silent / non-silent by peak amplitude, and groups runs of silent windows
//! into ranges measured in microseconds of source-time.
//!
//! Pure function with no `EditorState` or filesystem coupling: callers feed
//! it samples (typically from `audio_toolkit::read_wav_samples`) and a
//! sample rate, and receive `(start_us, end_us)` ranges suitable for
//! constructing silence sentinels.
//!
//! The peak-amplitude metric matches `commands::waveform::generate_waveform_peaks`
//! so visual flatlines in the UI map 1:1 to detected ranges (modulo dBFS
//! threshold versus the UI's renormalized peaks).

/// Configuration for silence detection.
///
/// Defaults are tuned for spoken-word recordings sampled at 16 kHz mono
/// (the format Toaster's transcription pipeline caches under
/// `%TEMP%\toaster_audio\extract_*.wav`):
///
/// - `threshold_dbfs = -45.0` — anything quieter than this is silence.
/// - `min_duration_us = 400_000` — ranges shorter than 400 ms are dropped.
/// - `window_us = 30_000` — 30 ms analysis window (~480 samples at 16 kHz).
#[derive(Debug, Clone, Copy)]
pub struct SilenceDetectConfig {
    pub threshold_dbfs: f32,
    pub min_duration_us: i64,
    pub window_us: i64,
}

impl Default for SilenceDetectConfig {
    fn default() -> Self {
        Self {
            threshold_dbfs: -45.0,
            min_duration_us: 400_000,
            window_us: 30_000,
        }
    }
}

/// Detect silent ranges in `samples`.
///
/// Returns a sorted, non-overlapping `Vec<(start_us, end_us)>` of ranges
/// where the peak absolute sample amplitude stays below the threshold for
/// at least `cfg.min_duration_us`.
///
/// Ranges are clamped to the buffer duration. Empty / near-empty inputs
/// return `vec![]`.
pub fn detect_silent_ranges(
    samples: &[f32],
    sample_rate_hz: u32,
    cfg: &SilenceDetectConfig,
) -> Vec<(i64, i64)> {
    if samples.is_empty() || sample_rate_hz == 0 || cfg.window_us <= 0 {
        return Vec::new();
    }

    let window_samples = window_samples_for(sample_rate_hz, cfg.window_us);
    if window_samples == 0 {
        return Vec::new();
    }

    // 0 dBFS == 1.0 linear; threshold_linear = 10^(dbfs / 20).
    let threshold_linear = 10f32.powf(cfg.threshold_dbfs / 20.0);

    let total_samples = samples.len();
    let total_duration_us = samples_to_us(total_samples, sample_rate_hz);

    let mut ranges: Vec<(i64, i64)> = Vec::new();
    let mut silence_start_us: Option<i64> = None;

    let mut window_idx: usize = 0;
    while window_idx * window_samples < total_samples {
        let start_sample = window_idx * window_samples;
        let end_sample = (start_sample + window_samples).min(total_samples);

        let peak = samples[start_sample..end_sample]
            .iter()
            .map(|s| s.abs())
            .fold(0.0_f32, f32::max);

        let window_start_us = samples_to_us(start_sample, sample_rate_hz);
        let window_end_us = samples_to_us(end_sample, sample_rate_hz);

        if peak < threshold_linear {
            if silence_start_us.is_none() {
                silence_start_us = Some(window_start_us);
            }
        } else if let Some(start_us) = silence_start_us.take() {
            push_if_long_enough(&mut ranges, start_us, window_start_us, cfg.min_duration_us);
        }

        // Last window: if we end inside a silence run, close it at buffer end.
        if end_sample == total_samples {
            if let Some(start_us) = silence_start_us.take() {
                push_if_long_enough(&mut ranges, start_us, window_end_us, cfg.min_duration_us);
            }
            break;
        }

        window_idx += 1;
    }

    // Defensive clamp; samples_to_us uses integer division so end may equal
    // total_duration_us already, but never exceed it.
    for (_, end_us) in &mut ranges {
        if *end_us > total_duration_us {
            *end_us = total_duration_us;
        }
    }

    ranges
}

fn window_samples_for(sample_rate_hz: u32, window_us: i64) -> usize {
    if window_us <= 0 {
        return 0;
    }
    ((sample_rate_hz as i64 * window_us) / 1_000_000) as usize
}

fn samples_to_us(sample_count: usize, sample_rate_hz: u32) -> i64 {
    if sample_rate_hz == 0 {
        return 0;
    }
    ((sample_count as i64) * 1_000_000) / (sample_rate_hz as i64)
}

fn push_if_long_enough(out: &mut Vec<(i64, i64)>, start_us: i64, end_us: i64, min_us: i64) {
    if end_us - start_us >= min_us {
        out.push((start_us, end_us));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const SR: u32 = 16_000;

    fn cfg() -> SilenceDetectConfig {
        SilenceDetectConfig::default()
    }

    fn silent_buffer(duration_us: i64) -> Vec<f32> {
        let samples = ((SR as i64 * duration_us) / 1_000_000) as usize;
        vec![0.0; samples]
    }

    fn loud_buffer(duration_us: i64, amplitude: f32) -> Vec<f32> {
        let samples = ((SR as i64 * duration_us) / 1_000_000) as usize;
        // Square-ish wave at full amplitude alternating sign so peak == amplitude
        // in every window, regardless of window alignment.
        (0..samples)
            .map(|i| if i % 2 == 0 { amplitude } else { -amplitude })
            .collect()
    }

    #[test]
    fn empty_buffer_emits_no_ranges() {
        let ranges = detect_silent_ranges(&[], SR, &cfg());
        assert!(ranges.is_empty());
    }

    #[test]
    fn silent_buffer_emits_one_range_covering_the_whole_buffer() {
        let dur_us = 1_000_000; // 1 s
        let samples = silent_buffer(dur_us);
        let ranges = detect_silent_ranges(&samples, SR, &cfg());
        assert_eq!(ranges.len(), 1, "expected one silence range, got {:?}", ranges);
        let (start, end) = ranges[0];
        assert_eq!(start, 0);
        assert!(
            (end - dur_us).abs() < cfg().window_us,
            "range end {} should be within one window of buffer duration {}",
            end,
            dur_us
        );
    }

    #[test]
    fn full_energy_buffer_emits_no_ranges() {
        let samples = loud_buffer(1_000_000, 0.9);
        let ranges = detect_silent_ranges(&samples, SR, &cfg());
        assert!(ranges.is_empty(), "expected no silence ranges, got {:?}", ranges);
    }

    #[test]
    fn mixed_buffer_emits_only_long_silences() {
        // Pattern: [loud 200ms][silence 200ms][loud 200ms][silence 800ms][loud 200ms]
        // Only the 800 ms silence should be reported with the default 400 ms minimum.
        let mut samples: Vec<f32> = Vec::new();
        samples.extend(loud_buffer(200_000, 0.9));
        samples.extend(silent_buffer(200_000));
        samples.extend(loud_buffer(200_000, 0.9));
        samples.extend(silent_buffer(800_000));
        samples.extend(loud_buffer(200_000, 0.9));

        let ranges = detect_silent_ranges(&samples, SR, &cfg());
        assert_eq!(ranges.len(), 1, "expected exactly one silence range, got {:?}", ranges);

        let (start, end) = ranges[0];
        let one_window = cfg().window_us;
        // 200ms loud + 200ms silence + 200ms loud = 600_000 us before the long silence.
        let expected_start = 600_000;
        let expected_end = 1_400_000;
        assert!(
            (start - expected_start).abs() <= one_window,
            "silence start {} not within one window of {}",
            start,
            expected_start
        );
        assert!(
            (end - expected_end).abs() <= one_window,
            "silence end {} not within one window of {}",
            end,
            expected_end
        );
    }

    #[test]
    fn boundary_threshold_inclusive_at_min_duration() {
        // Custom config: 200 ms minimum.
        //
        // Fixed-window detection has up to one window of slop at each end of
        // a silence run (a window straddling the speech-silence boundary
        // classifies as speech because peak amplitude wins). We give the
        // silence enough headroom to clear the threshold even with a full
        // window of slop on the trailing edge.
        let mut cfg = cfg();
        cfg.min_duration_us = 200_000;

        let mut samples: Vec<f32> = Vec::new();
        samples.extend(loud_buffer(60_000, 0.9));
        samples.extend(silent_buffer(240_000));
        samples.extend(loud_buffer(60_000, 0.9));

        let ranges = detect_silent_ranges(&samples, SR, &cfg);
        assert_eq!(
            ranges.len(),
            1,
            "expected silence longer than the minimum duration to be emitted: {:?}",
            ranges
        );
        let (start, end) = ranges[0];
        assert!(
            end - start >= cfg.min_duration_us,
            "detected silence {} us shorter than min_duration {}",
            end - start,
            cfg.min_duration_us
        );
    }

    #[test]
    fn silence_shorter_than_min_duration_is_dropped() {
        // 100 ms silence with 200 ms minimum should never be emitted.
        let mut cfg = cfg();
        cfg.min_duration_us = 200_000;

        let mut samples: Vec<f32> = Vec::new();
        samples.extend(loud_buffer(60_000, 0.9));
        samples.extend(silent_buffer(100_000));
        samples.extend(loud_buffer(60_000, 0.9));

        let ranges = detect_silent_ranges(&samples, SR, &cfg);
        assert!(ranges.is_empty(), "100 ms silence should be dropped: {:?}", ranges);
    }

    #[test]
    fn quiet_but_above_threshold_is_not_silence() {
        // Default threshold is -45 dBFS ~= 0.0056 linear. Use 0.05 (well above).
        let samples = loud_buffer(1_000_000, 0.05);
        let ranges = detect_silent_ranges(&samples, SR, &cfg());
        assert!(ranges.is_empty(), "0.05 amplitude should not be silence: {:?}", ranges);
    }

    #[test]
    fn very_quiet_below_threshold_is_silence() {
        // 0.001 linear == -60 dBFS, well below -45.
        let samples = loud_buffer(1_000_000, 0.001);
        let ranges = detect_silent_ranges(&samples, SR, &cfg());
        assert_eq!(ranges.len(), 1, "expected one silence range, got {:?}", ranges);
    }

    #[test]
    fn ranges_are_sorted_and_non_overlapping() {
        // Two separated long silences should come back in order.
        let mut samples: Vec<f32> = Vec::new();
        samples.extend(silent_buffer(800_000));
        samples.extend(loud_buffer(300_000, 0.9));
        samples.extend(silent_buffer(800_000));
        samples.extend(loud_buffer(300_000, 0.9));

        let ranges = detect_silent_ranges(&samples, SR, &cfg());
        assert_eq!(ranges.len(), 2, "expected two ranges, got {:?}", ranges);
        assert!(ranges[0].1 <= ranges[1].0, "ranges overlap or out of order: {:?}", ranges);
    }

    #[test]
    fn zero_sample_rate_returns_empty() {
        let samples = silent_buffer(1_000_000);
        let ranges = detect_silent_ranges(&samples, 0, &cfg());
        assert!(ranges.is_empty());
    }
}
