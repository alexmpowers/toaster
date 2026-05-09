//! PCM-level silence detection with optional VAD confirmation.
//!
//! Complements the inter-word gap analysis in `managers::filler` by
//! examining raw audio amplitude. When a VAD probability curve is
//! available, gaps are cross-referenced to distinguish true silence
//! from non-speech acoustic content.

use crate::managers::editor::Word;
use crate::managers::filler::{classify_gap, GapClassification};

/// Configuration for silence detection.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, specta::Type)]
pub struct SilenceConfig {
    /// Peak amplitude threshold (linear scale, 0.0-1.0).
    pub amplitude_threshold: f32,
    /// Minimum duration in microseconds for a silence region or gap.
    pub min_duration_us: i64,
    /// Whether to use VAD curve confirmation when available.
    pub use_vad: bool,
}

impl Default for SilenceConfig {
    fn default() -> Self {
        Self {
            amplitude_threshold: 0.01,
            min_duration_us: 500_000,
            use_vad: true,
        }
    }
}

/// A detected silence region in the audio.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, specta::Type)]
pub struct SilenceRegion {
    pub start_us: i64,
    pub end_us: i64,
    pub duration_us: i64,
    pub peak_amplitude: f32,
    pub classification: SilenceClassification,
}

/// High-level classification for a silence candidate.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize, specta::Type)]
pub enum SilenceClassification {
    /// Confirmed silence by amplitude and VAD.
    Confirmed,
    /// Amplitude says silent, but VAD indicates acoustic or speech activity.
    PossibleSpeech,
    /// Amplitude-only classification because VAD is unavailable or disabled.
    AmplitudeOnly,
}

/// Result of silence analysis.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, specta::Type)]
pub struct SilenceAnalysis {
    pub regions: Vec<SilenceRegion>,
    pub total_silence_us: i64,
    pub confirmed_silence_us: i64,
    pub region_count: usize,
}

/// A word-gap candidate affected by silence.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, specta::Type, PartialEq, Eq)]
pub struct SilenceWordCandidate {
    /// Index of the word BEFORE the silence gap.
    pub word_index: usize,
    /// The actual gap start in microseconds.
    pub gap_start_us: i64,
    pub gap_end_us: i64,
    pub gap_duration_us: i64,
    pub classification: SilenceClassification,
}

/// Detect silence regions in PCM audio data.
pub fn detect_silence_regions(
    samples: &[f32],
    sample_rate: u32,
    config: &SilenceConfig,
    vad_curve: &[f32],
) -> SilenceAnalysis {
    if samples.is_empty() || sample_rate == 0 {
        return SilenceAnalysis {
            regions: Vec::new(),
            total_silence_us: 0,
            confirmed_silence_us: 0,
            region_count: 0,
        };
    }

    let window_samples = ((sample_rate / 100).max(1)) as usize;
    let min_duration_us = config.min_duration_us.max(0);
    let mut regions = Vec::new();
    let mut run_start_sample: Option<usize> = None;
    let mut run_peak_amplitude = 0.0_f32;

    let sample_idx_to_us = |sample_idx: usize| sample_idx as i64 * 1_000_000 / sample_rate as i64;
    let mut finish_run = |start_sample: usize, end_sample: usize, peak_amplitude: f32| {
        let start_us = sample_idx_to_us(start_sample);
        let end_us = sample_idx_to_us(end_sample);
        let duration_us = end_us - start_us;
        if duration_us < min_duration_us || end_us <= start_us {
            return;
        }

        regions.push(SilenceRegion {
            start_us,
            end_us,
            duration_us,
            peak_amplitude,
            classification: map_gap_classification(config, start_us, end_us, vad_curve),
        });
    };

    for window_start in (0..samples.len()).step_by(window_samples) {
        let window_end = (window_start + window_samples).min(samples.len());
        let peak_amplitude = samples[window_start..window_end]
            .iter()
            .fold(0.0_f32, |acc, sample| acc.max(sample.abs()));
        let is_silent = peak_amplitude <= config.amplitude_threshold;

        match run_start_sample {
            Some(start_sample) if !is_silent => {
                finish_run(start_sample, window_start, run_peak_amplitude);
                run_start_sample = None;
                run_peak_amplitude = 0.0;
            }
            Some(_) if is_silent => {
                run_peak_amplitude = run_peak_amplitude.max(peak_amplitude);
            }
            None if is_silent => {
                run_start_sample = Some(window_start);
                run_peak_amplitude = peak_amplitude;
            }
            None => {}
            Some(_) => {}
        }
    }

    if let Some(start_sample) = run_start_sample {
        finish_run(start_sample, samples.len(), run_peak_amplitude);
    }

    let total_silence_us = regions.iter().map(|region| region.duration_us).sum();
    let confirmed_silence_us = regions
        .iter()
        .filter(|region| region.classification == SilenceClassification::Confirmed)
        .map(|region| region.duration_us)
        .sum();

    SilenceAnalysis {
        region_count: regions.len(),
        regions,
        total_silence_us,
        confirmed_silence_us,
    }
}

/// Find word indices whose inter-word gaps overlap detected silence regions.
pub fn find_silence_affected_words(
    words: &[Word],
    regions: &[SilenceRegion],
) -> Vec<SilenceWordCandidate> {
    if words.len() < 2 || regions.is_empty() {
        return Vec::new();
    }

    let active_indices: Vec<usize> = words
        .iter()
        .enumerate()
        .filter_map(|(idx, word)| (!word.deleted).then_some(idx))
        .collect();
    let mut candidates = Vec::new();

    for pair in active_indices.windows(2) {
        let word_index = pair[0];
        let next_index = pair[1];
        let gap_start_us = words[word_index].end_us;
        let gap_end_us = words[next_index].start_us;
        let gap_duration_us = gap_end_us - gap_start_us;
        if gap_duration_us <= 0 {
            continue;
        }

        let best_region = regions
            .iter()
            .filter_map(|region| {
                let overlap_start = gap_start_us.max(region.start_us);
                let overlap_end = gap_end_us.min(region.end_us);
                let overlap_us = overlap_end - overlap_start;
                (overlap_us > 0).then_some((overlap_us, region))
            })
            .filter(|(overlap_us, _)| overlap_us.saturating_mul(2) >= gap_duration_us)
            .max_by_key(|(overlap_us, _)| *overlap_us)
            .map(|(_, region)| region);

        if let Some(region) = best_region {
            candidates.push(SilenceWordCandidate {
                word_index,
                gap_start_us,
                gap_end_us,
                gap_duration_us,
                classification: region.classification,
            });
        }
    }

    candidates
}

/// Find silence gaps directly from word timestamps, optionally enhanced by VAD.
pub fn find_silence_gaps_from_words(
    words: &[Word],
    config: &SilenceConfig,
    vad_curve: &[f32],
) -> Vec<SilenceWordCandidate> {
    if words.len() < 2 {
        return Vec::new();
    }

    let min_duration_us = config.min_duration_us.max(0);
    let active_indices: Vec<usize> = words
        .iter()
        .enumerate()
        .filter_map(|(idx, word)| (!word.deleted).then_some(idx))
        .collect();
    let mut candidates = Vec::new();

    for pair in active_indices.windows(2) {
        let word_index = pair[0];
        let next_index = pair[1];
        let gap_start_us = words[word_index].end_us;
        let gap_end_us = words[next_index].start_us;
        let gap_duration_us = gap_end_us - gap_start_us;
        if gap_duration_us < min_duration_us || gap_duration_us <= 0 {
            continue;
        }

        candidates.push(SilenceWordCandidate {
            word_index,
            gap_start_us,
            gap_end_us,
            gap_duration_us,
            classification: map_gap_classification(config, gap_start_us, gap_end_us, vad_curve),
        });
    }

    candidates
}

/// Mark words bordering silence gaps.
pub fn mark_silence_gaps(
    words: &mut [Word],
    candidates: &[SilenceWordCandidate],
    only_confirmed: bool,
) -> usize {
    let mut count = 0;

    for candidate in candidates {
        if only_confirmed && candidate.classification != SilenceClassification::Confirmed {
            continue;
        }

        let Some(next_index) = candidate.word_index.checked_add(1) else {
            continue;
        };
        if next_index >= words.len() {
            continue;
        }
        if words[candidate.word_index].deleted || words[next_index].deleted {
            continue;
        }

        let gap_start_us = words[candidate.word_index].end_us;
        let gap_end_us = words[next_index].start_us;
        let gap_duration_us = gap_end_us - gap_start_us;
        if gap_start_us != candidate.gap_start_us
            || gap_end_us != candidate.gap_end_us
            || gap_duration_us != candidate.gap_duration_us
        {
            continue;
        }

        if !words[next_index].silenced {
            words[next_index].silenced = true;
            count += 1;
        }
    }

    count
}

fn map_gap_classification(
    config: &SilenceConfig,
    gap_start_us: i64,
    gap_end_us: i64,
    vad_curve: &[f32],
) -> SilenceClassification {
    if !config.use_vad || vad_curve.is_empty() || gap_end_us <= gap_start_us {
        return SilenceClassification::AmplitudeOnly;
    }

    match classify_gap(gap_start_us, gap_end_us, vad_curve) {
        GapClassification::TrueSilence => SilenceClassification::Confirmed,
        GapClassification::NonSpeechAcoustic | GapClassification::MissedSpeech => {
            SilenceClassification::PossibleSpeech
        }
        GapClassification::Unknown => SilenceClassification::AmplitudeOnly,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_word(text: &str, start_us: i64, end_us: i64) -> Word {
        Word {
            text: text.to_string(),
            start_us,
            end_us,
            deleted: false,
            silenced: false,
            confidence: 1.0,
            speaker_id: 0,
        }
    }

    #[test]
    fn detect_silence_regions_finds_long_silence() {
        let samples = [vec![0.2; 100], vec![0.0; 600], vec![0.2; 100]].concat();
        let analysis = detect_silence_regions(&samples, 1_000, &SilenceConfig::default(), &[]);

        assert_eq!(analysis.region_count, 1);
        assert_eq!(analysis.total_silence_us, 600_000);
        assert_eq!(analysis.confirmed_silence_us, 0);
        assert_eq!(analysis.regions[0].start_us, 100_000);
        assert_eq!(analysis.regions[0].end_us, 700_000);
        assert_eq!(analysis.regions[0].duration_us, 600_000);
        assert_eq!(analysis.regions[0].peak_amplitude, 0.0);
        assert_eq!(
            analysis.regions[0].classification,
            SilenceClassification::AmplitudeOnly
        );
    }

    #[test]
    fn detect_silence_regions_respects_amplitude_threshold() {
        let samples = vec![0.015; 600];
        let low_threshold = SilenceConfig {
            amplitude_threshold: 0.01,
            ..SilenceConfig::default()
        };
        let high_threshold = SilenceConfig {
            amplitude_threshold: 0.02,
            ..SilenceConfig::default()
        };

        assert!(detect_silence_regions(&samples, 1_000, &low_threshold, &[])
            .regions
            .is_empty());
        assert_eq!(
            detect_silence_regions(&samples, 1_000, &high_threshold, &[]).region_count,
            1
        );
    }

    #[test]
    fn detect_silence_regions_uses_vad_for_confirmation() {
        let samples = [vec![0.2; 100], vec![0.0; 600], vec![0.2; 100]].concat();
        let analysis =
            detect_silence_regions(&samples, 1_000, &SilenceConfig::default(), &vec![0.0; 32]);

        assert_eq!(analysis.region_count, 1);
        assert_eq!(analysis.confirmed_silence_us, 600_000);
        assert_eq!(
            analysis.regions[0].classification,
            SilenceClassification::Confirmed
        );
    }

    #[test]
    fn detect_silence_regions_flags_possible_speech_when_vad_stays_hot() {
        let samples = [vec![0.2; 100], vec![0.0; 600], vec![0.2; 100]].concat();
        let analysis =
            detect_silence_regions(&samples, 1_000, &SilenceConfig::default(), &vec![0.8; 32]);

        assert_eq!(analysis.region_count, 1);
        assert_eq!(analysis.confirmed_silence_us, 0);
        assert_eq!(
            analysis.regions[0].classification,
            SilenceClassification::PossibleSpeech
        );
    }

    #[test]
    fn detect_silence_regions_filters_short_runs() {
        let samples = [vec![0.2; 100], vec![0.0; 200], vec![0.2; 100]].concat();
        let analysis = detect_silence_regions(&samples, 1_000, &SilenceConfig::default(), &[]);

        assert!(analysis.regions.is_empty());
        assert_eq!(analysis.total_silence_us, 0);
        assert_eq!(analysis.region_count, 0);
    }

    #[test]
    fn find_silence_affected_words_requires_half_gap_overlap() {
        let words = vec![
            make_word("hello", 0, 1_000_000),
            make_word("world", 2_000_000, 3_000_000),
        ];
        let regions = vec![SilenceRegion {
            start_us: 1_400_000,
            end_us: 1_900_000,
            duration_us: 500_000,
            peak_amplitude: 0.0,
            classification: SilenceClassification::Confirmed,
        }];

        let candidates = find_silence_affected_words(&words, &regions);
        assert_eq!(candidates.len(), 1);
        assert_eq!(candidates[0].word_index, 0);
        assert_eq!(candidates[0].gap_start_us, 1_000_000);
        assert_eq!(candidates[0].gap_end_us, 2_000_000);
        assert_eq!(candidates[0].gap_duration_us, 1_000_000);
        assert_eq!(
            candidates[0].classification,
            SilenceClassification::Confirmed
        );
    }

    #[test]
    fn find_silence_affected_words_skips_small_overlaps() {
        let words = vec![
            make_word("hello", 0, 1_000_000),
            make_word("world", 2_000_000, 3_000_000),
        ];
        let regions = vec![SilenceRegion {
            start_us: 1_700_000,
            end_us: 1_900_000,
            duration_us: 200_000,
            peak_amplitude: 0.0,
            classification: SilenceClassification::Confirmed,
        }];

        assert!(find_silence_affected_words(&words, &regions).is_empty());
    }

    #[test]
    fn find_silence_gaps_from_words_uses_timestamp_gaps() {
        let words = vec![
            make_word("one", 0, 400_000),
            make_word("two", 1_100_000, 1_500_000),
            make_word("three", 1_800_000, 2_000_000),
        ];
        let candidates = find_silence_gaps_from_words(&words, &SilenceConfig::default(), &[]);

        assert_eq!(candidates.len(), 1);
        assert_eq!(candidates[0].word_index, 0);
        assert_eq!(candidates[0].gap_start_us, 400_000);
        assert_eq!(candidates[0].gap_end_us, 1_100_000);
        assert_eq!(candidates[0].gap_duration_us, 700_000);
        assert_eq!(
            candidates[0].classification,
            SilenceClassification::AmplitudeOnly
        );
    }

    #[test]
    fn find_silence_gaps_from_words_applies_vad_classification() {
        let words = vec![
            make_word("one", 0, 400_000),
            make_word("two", 1_100_000, 1_500_000),
        ];
        let candidates =
            find_silence_gaps_from_words(&words, &SilenceConfig::default(), &vec![0.0; 40]);

        assert_eq!(candidates.len(), 1);
        assert_eq!(
            candidates[0].classification,
            SilenceClassification::Confirmed
        );
    }

    #[test]
    fn mark_silence_gaps_marks_following_words_and_honors_confirmed_filter() {
        let mut words = vec![
            make_word("one", 0, 400_000),
            make_word("two", 1_100_000, 1_500_000),
        ];
        let candidate = SilenceWordCandidate {
            word_index: 0,
            gap_start_us: 400_000,
            gap_end_us: 1_100_000,
            gap_duration_us: 700_000,
            classification: SilenceClassification::AmplitudeOnly,
        };

        assert_eq!(
            mark_silence_gaps(&mut words, std::slice::from_ref(&candidate), true),
            0
        );
        assert!(!words[1].silenced);

        assert_eq!(mark_silence_gaps(&mut words, &[candidate], false), 1);
        assert!(words[1].silenced);
    }

    #[test]
    fn detect_silence_regions_handles_edge_cases() {
        let empty = detect_silence_regions(&[], 1_000, &SilenceConfig::default(), &[]);
        assert!(empty.regions.is_empty());

        let loud = detect_silence_regions(&vec![0.5; 800], 1_000, &SilenceConfig::default(), &[]);
        assert!(loud.regions.is_empty());

        let all_silence =
            detect_silence_regions(&vec![0.0; 800], 1_000, &SilenceConfig::default(), &[]);
        assert_eq!(all_silence.region_count, 1);
        assert_eq!(all_silence.regions[0].start_us, 0);
        assert_eq!(all_silence.regions[0].end_us, 800_000);
    }
}
