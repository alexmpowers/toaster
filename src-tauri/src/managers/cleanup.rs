//! Unified cleanup planning and application.
//!
//! Builds a dry-run cleanup plan by composing filler removal, duplicate
//! collapse, pause silencing, pause trimming, and audio-truth silence
//! removal into a single backend-owned operation.

use crate::audio_toolkit::{detect_silent_ranges, SilenceDetectConfig};
use crate::managers::{
    disfluency,
    editor::Word,
    filler::{
        self, is_silence_sentinel, make_silence_sentinel, FillerConfig, DEFAULT_MAX_GAP_US,
        DEFAULT_PAUSE_THRESHOLD_US,
    },
};

const MAX_PASSES: usize = 10;

/// Cleanup intensity preset.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize, specta::Type)]
pub enum CleanupPreset {
    Gentle,
    Balanced,
    Aggressive,
}

/// Per-category cleanup configuration.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, specta::Type)]
pub struct CleanupConfig {
    pub remove_fillers: bool,
    pub remove_duplicates: bool,
    pub silence_pauses: bool,
    pub trim_pauses: bool,
    pub remove_silence: bool,
    pub pause_threshold_us: Option<i64>,
    pub max_gap_us: Option<i64>,
}

impl CleanupConfig {
    #[must_use]
    pub fn from_preset(preset: CleanupPreset) -> Self {
        match preset {
            CleanupPreset::Gentle => Self {
                remove_fillers: true,
                remove_duplicates: false,
                silence_pauses: false,
                trim_pauses: false,
                remove_silence: false,
                pause_threshold_us: None,
                max_gap_us: None,
            },
            CleanupPreset::Balanced => Self {
                remove_fillers: true,
                remove_duplicates: true,
                silence_pauses: true,
                trim_pauses: false,
                remove_silence: false,
                pause_threshold_us: None,
                max_gap_us: None,
            },
            CleanupPreset::Aggressive => Self {
                remove_fillers: true,
                remove_duplicates: true,
                silence_pauses: true,
                trim_pauses: true,
                remove_silence: true,
                pause_threshold_us: None,
                max_gap_us: None,
            },
        }
    }
}

/// Optional decoded audio used for audio-aware duplicate and silence planning.
pub struct CleanupAudioContext<'a> {
    pub samples: &'a [f32],
    pub sample_rate: u32,
}

/// Per-item action proposed by the cleanup planner.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, specta::Type)]
pub struct CleanupAction {
    pub word_index: usize,
    pub word_text: String,
    pub action: CleanupActionType,
    pub start_us: i64,
    pub end_us: i64,
}

/// Action type emitted by the planner.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize, specta::Type)]
pub enum CleanupActionType {
    DeleteFiller,
    DeleteDuplicate,
    SilencePause,
    TrimPause,
    RemoveSilence,
}

/// Dry-run summary of a cleanup pass.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, specta::Type)]
pub struct CleanupPlan {
    pub source_revision: u64,
    pub actions: Vec<CleanupAction>,
    pub filler_count: usize,
    pub duplicate_count: usize,
    pub pause_count: usize,
    pub trim_count: usize,
    pub silence_count: usize,
    pub total_affected: usize,
    pub estimated_duration_saved_us: i64,
    pub passes: usize,
}

#[must_use]
pub fn plan_cleanup(
    words: &[Word],
    config: &CleanupConfig,
    custom_filler_words: &[String],
    audio: Option<CleanupAudioContext<'_>>,
) -> CleanupPlan {
    let pause_threshold = config
        .pause_threshold_us
        .unwrap_or(DEFAULT_PAUSE_THRESHOLD_US);
    let max_gap_us = config.max_gap_us.unwrap_or(DEFAULT_MAX_GAP_US);
    let filler_config = FillerConfig {
        filler_words: custom_filler_words.to_vec(),
        pause_threshold_us: pause_threshold,
        ..Default::default()
    };

    if words.is_empty() {
        return CleanupPlan {
            source_revision: 0,
            actions: Vec::new(),
            filler_count: 0,
            duplicate_count: 0,
            pause_count: 0,
            trim_count: 0,
            silence_count: 0,
            total_affected: 0,
            estimated_duration_saved_us: 0,
            passes: 0,
        };
    }

    let mut simulated = words.to_vec();
    let mut actions: Vec<CleanupAction> = Vec::new();
    let mut passes = 0usize;

    if config.remove_fillers || config.remove_duplicates {
        for pass in 0..MAX_PASSES {
            let mut changed = false;

            if config.remove_fillers {
                for idx in filler::detect_fillers(&simulated, &filler_config) {
                    if idx >= simulated.len() || simulated[idx].deleted {
                        continue;
                    }
                    actions.push(word_action(
                        &simulated[idx],
                        idx,
                        CleanupActionType::DeleteFiller,
                    ));
                    simulated[idx].deleted = true;
                    changed = true;
                }
            }

            if config.remove_duplicates {
                for idx in detect_duplicate_indices(&simulated, audio.as_ref()) {
                    if idx >= simulated.len() || simulated[idx].deleted {
                        continue;
                    }
                    actions.push(word_action(
                        &simulated[idx],
                        idx,
                        CleanupActionType::DeleteDuplicate,
                    ));
                    simulated[idx].deleted = true;
                    changed = true;
                }
            }

            passes = pass + 1;
            if !changed {
                break;
            }
        }
    }

    if config.silence_pauses && !config.trim_pauses {
        for (after_idx, _) in filler::detect_pauses(&simulated, &filler_config) {
            let Some(next_idx) = next_active_index(&simulated, after_idx) else {
                continue;
            };
            if simulated[next_idx].silenced {
                continue;
            }
            let start_us = simulated[after_idx].end_us;
            let end_us = simulated[next_idx].start_us;
            actions.push(CleanupAction {
                word_index: next_idx,
                word_text: simulated[next_idx].text.clone(),
                action: CleanupActionType::SilencePause,
                start_us,
                end_us,
            });
            simulated[next_idx].silenced = true;
        }
    }

    if config.trim_pauses {
        let trim_ranges = planned_trim_ranges(&simulated, pause_threshold, max_gap_us);
        for (word_index, start_us, end_us) in trim_ranges {
            actions.push(CleanupAction {
                word_index,
                word_text: simulated[word_index].text.clone(),
                action: CleanupActionType::TrimPause,
                start_us,
                end_us,
            });
        }
        insert_silence_ranges(
            &mut simulated,
            &actions
                .iter()
                .filter(|action| action.action == CleanupActionType::TrimPause)
                .map(|action| (action.start_us, action.end_us))
                .collect::<Vec<_>>(),
        );
    }

    if config.remove_silence {
        if let Some(audio) = audio {
            if audio.sample_rate > 0 && !audio.samples.is_empty() {
                let detected = detect_silent_ranges(
                    audio.samples,
                    audio.sample_rate,
                    &SilenceDetectConfig::default(),
                );
                let existing = existing_silence_ranges(&simulated);
                let mut residual_ranges = Vec::new();
                for (start_us, end_us) in detected {
                    residual_ranges.extend(subtract_existing_coverage(start_us, end_us, &existing));
                }
                for (start_us, end_us) in &residual_ranges {
                    let Some(word_index) = anchor_index_for_range(&simulated, *start_us, *end_us)
                    else {
                        continue;
                    };
                    actions.push(CleanupAction {
                        word_index,
                        word_text: simulated[word_index].text.clone(),
                        action: CleanupActionType::RemoveSilence,
                        start_us: *start_us,
                        end_us: *end_us,
                    });
                }
                insert_silence_ranges(&mut simulated, &residual_ranges);
            }
        }
    }

    let filler_count = actions
        .iter()
        .filter(|action| action.action == CleanupActionType::DeleteFiller)
        .count();
    let duplicate_count = actions
        .iter()
        .filter(|action| action.action == CleanupActionType::DeleteDuplicate)
        .count();
    let pause_count = actions
        .iter()
        .filter(|action| action.action == CleanupActionType::SilencePause)
        .count();
    let trim_count = actions
        .iter()
        .filter(|action| action.action == CleanupActionType::TrimPause)
        .count();
    let silence_count = actions
        .iter()
        .filter(|action| action.action == CleanupActionType::RemoveSilence)
        .count();
    let estimated_duration_saved_us = actions.iter().map(action_saved_duration_us).sum();

    CleanupPlan {
        source_revision: 0,
        total_affected: actions.len(),
        actions,
        filler_count,
        duplicate_count,
        pause_count,
        trim_count,
        silence_count,
        estimated_duration_saved_us,
        passes,
    }
}

pub fn apply_cleanup_plan(words: &mut Vec<Word>, plan: &CleanupPlan) -> usize {
    if plan.actions.is_empty() {
        return 0;
    }

    let mut modified = 0usize;
    let mut insertions: Vec<(i64, i64)> = Vec::new();

    for action in &plan.actions {
        match action.action {
            CleanupActionType::DeleteFiller | CleanupActionType::DeleteDuplicate => {
                if let Some(word) = words.get_mut(action.word_index) {
                    if !word.deleted {
                        word.deleted = true;
                        modified += 1;
                    }
                }
            }
            CleanupActionType::SilencePause => {
                if let Some(word) = words.get_mut(action.word_index) {
                    if !word.silenced {
                        word.silenced = true;
                        modified += 1;
                    }
                }
            }
            CleanupActionType::TrimPause | CleanupActionType::RemoveSilence => {
                if action.end_us > action.start_us {
                    insertions.push((action.start_us, action.end_us));
                }
            }
        }
    }

    insertions.sort_unstable();
    insertions.dedup();
    modified += insert_silence_ranges(words, &insertions);
    modified
}

fn detect_duplicate_indices(words: &[Word], audio: Option<&CleanupAudioContext<'_>>) -> Vec<usize> {
    if let Some(audio) = audio {
        if audio.sample_rate > 0 && !audio.samples.is_empty() {
            let mut indices: Vec<usize> = disfluency::plan(words, audio.samples, audio.sample_rate)
                .into_iter()
                .flat_map(|decision| decision.losers)
                .collect();
            indices.sort_unstable();
            indices.dedup();
            return indices;
        }
    }

    filler::detect_duplicates(words)
}

fn word_action(word: &Word, word_index: usize, action: CleanupActionType) -> CleanupAction {
    CleanupAction {
        word_index,
        word_text: word.text.clone(),
        action,
        start_us: word.start_us,
        end_us: word.end_us,
    }
}

fn next_active_index(words: &[Word], after_idx: usize) -> Option<usize> {
    words
        .iter()
        .enumerate()
        .skip(after_idx.saturating_add(1))
        .find_map(|(idx, word)| (!word.deleted).then_some(idx))
}

fn planned_trim_ranges(
    words: &[Word],
    pause_threshold_us: i64,
    max_gap_us: i64,
) -> Vec<(usize, i64, i64)> {
    let mut ranges = Vec::new();
    let mut prev_non_deleted_end: Option<i64> = None;
    let mut sentinel_between = false;

    for (idx, word) in words.iter().enumerate() {
        if word.deleted {
            if is_silence_sentinel(word) {
                sentinel_between = true;
            }
            continue;
        }
        if let Some(previous_end) = prev_non_deleted_end {
            if !sentinel_between {
                let gap = word.start_us.saturating_sub(previous_end);
                let excess = gap.saturating_sub(max_gap_us);
                if gap >= pause_threshold_us && excess > 0 {
                    let start_us = previous_end.saturating_add(max_gap_us).min(word.start_us);
                    ranges.push((idx, start_us, word.start_us));
                }
            }
        }
        prev_non_deleted_end = Some(word.end_us);
        sentinel_between = false;
    }

    ranges
}

fn existing_silence_ranges(words: &[Word]) -> Vec<(i64, i64)> {
    let mut ranges: Vec<(i64, i64)> = words
        .iter()
        .filter(|word| is_silence_sentinel(word) && word.end_us > word.start_us)
        .map(|word| (word.start_us, word.end_us))
        .collect();
    ranges.sort_unstable();
    ranges
}

fn subtract_existing_coverage(
    start_us: i64,
    end_us: i64,
    existing: &[(i64, i64)],
) -> Vec<(i64, i64)> {
    if end_us <= start_us {
        return Vec::new();
    }

    let mut residual = Vec::new();
    let mut cursor = start_us;
    for &(existing_start, existing_end) in existing {
        if existing_end <= cursor {
            continue;
        }
        if existing_start >= end_us {
            break;
        }
        if existing_start > cursor {
            residual.push((cursor, existing_start.min(end_us)));
        }
        cursor = cursor.max(existing_end);
        if cursor >= end_us {
            break;
        }
    }
    if cursor < end_us {
        residual.push((cursor, end_us));
    }
    residual
}

fn insert_silence_ranges(words: &mut Vec<Word>, ranges: &[(i64, i64)]) -> usize {
    if ranges.is_empty() {
        return 0;
    }

    let mut inserted = 0usize;
    for &(start_us, end_us) in ranges.iter().rev() {
        if end_us <= start_us {
            continue;
        }
        let insert_idx = match words.binary_search_by_key(&start_us, |word| word.start_us) {
            Ok(idx) | Err(idx) => idx,
        };
        words.insert(insert_idx, make_silence_sentinel(start_us, end_us));
        inserted += 1;
    }
    inserted
}

fn anchor_index_for_range(words: &[Word], start_us: i64, end_us: i64) -> Option<usize> {
    if let Some((idx, _)) = words
        .iter()
        .enumerate()
        .find(|(_, word)| !word.deleted && word.start_us >= end_us)
    {
        return Some(idx);
    }
    if let Some((idx, _)) = words
        .iter()
        .enumerate()
        .rev()
        .find(|(_, word)| !word.deleted && word.end_us <= start_us)
    {
        return Some(idx);
    }
    words.iter().position(|word| !word.deleted)
}

fn action_saved_duration_us(action: &CleanupAction) -> i64 {
    match action.action {
        CleanupActionType::SilencePause => 0,
        _ => action.end_us.saturating_sub(action.start_us),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn word(text: &str, start_us: i64, end_us: i64) -> Word {
        Word {
            text: text.to_string(),
            start_us,
            end_us,
            deleted: false,
            silenced: false,
            confidence: 1.0,
            speaker_id: -1,
        }
    }

    fn silence(samples: &mut Vec<f32>, sample_rate: u32, duration_ms: u32) {
        let sample_count = (sample_rate as u64 * duration_ms as u64 / 1000) as usize;
        samples.extend(std::iter::repeat_n(0.0, sample_count));
    }

    fn tone_word(
        samples: &mut Vec<f32>,
        sample_rate: u32,
        text: &str,
        amplitude: f32,
        duration_ms: u32,
    ) -> Word {
        let start_sample = samples.len();
        let sample_count = (sample_rate as u64 * duration_ms as u64 / 1000) as usize;
        for i in 0..sample_count {
            let t = i as f32 / sample_rate as f32;
            samples.push(amplitude * (2.0 * std::f32::consts::PI * 440.0 * t).sin());
        }
        let start_us = (start_sample as i64 * 1_000_000) / sample_rate as i64;
        let end_us = (samples.len() as i64 * 1_000_000) / sample_rate as i64;
        word(text, start_us, end_us)
    }

    #[test]
    fn gentle_preset_only_removes_fillers_and_keeps_words_unchanged() {
        let words = vec![word("um", 0, 100_000), word("hello", 100_000, 300_000)];
        let before = words.clone();

        let fillers = vec!["um".to_string()];
        let plan = plan_cleanup(
            &words,
            &CleanupConfig::from_preset(CleanupPreset::Gentle),
            &fillers,
            None,
        );

        assert_eq!(plan.filler_count, 1);
        assert_eq!(plan.duplicate_count, 0);
        assert_eq!(plan.pause_count, 0);
        assert_eq!(plan.trim_count, 0);
        assert_eq!(plan.silence_count, 0);
        assert_eq!(plan.total_affected, 1);
        assert_eq!(plan.estimated_duration_saved_us, 100_000);
        assert_eq!(
            words.len(),
            before.len(),
            "planning must not mutate the transcript"
        );
        assert!(words.iter().zip(before.iter()).all(|(after, prior)| {
            after.text == prior.text
                && after.start_us == prior.start_us
                && after.end_us == prior.end_us
                && after.deleted == prior.deleted
                && after.silenced == prior.silenced
        }));
    }

    #[test]
    fn balanced_preset_cascades_fillers_duplicates_and_pause_silencing() {
        let words = vec![
            word("the", 0, 100_000),
            word("um", 100_000, 150_000),
            word("the", 150_000, 250_000),
            word("world", 2_000_000, 2_100_000),
        ];

        let fillers = vec!["um".to_string()];
        let plan = plan_cleanup(
            &words,
            &CleanupConfig::from_preset(CleanupPreset::Balanced),
            &fillers,
            None,
        );

        assert_eq!(plan.filler_count, 1);
        assert_eq!(
            plan.duplicate_count, 1,
            "duplicate should emerge after filler deletion"
        );
        assert_eq!(plan.pause_count, 1);
        assert_eq!(plan.trim_count, 0);
        assert_eq!(plan.silence_count, 0);
        assert_eq!(plan.total_affected, 3);
        assert_eq!(plan.estimated_duration_saved_us, 150_000);
    }

    #[test]
    fn aggressive_preset_counts_pause_trimming_duration() {
        let words = vec![
            word("hello", 0, 500_000),
            word("world", 2_000_000, 2_500_000),
        ];

        let plan = plan_cleanup(
            &words,
            &CleanupConfig::from_preset(CleanupPreset::Aggressive),
            &[],
            None,
        );

        assert_eq!(plan.trim_count, 1);
        assert_eq!(plan.pause_count, 0);
        assert_eq!(plan.silence_count, 0);
        assert_eq!(plan.total_affected, 1);
        assert_eq!(plan.estimated_duration_saved_us, 1_200_000);
    }

    #[test]
    fn audio_silence_preview_uses_detected_ranges() {
        let sample_rate = 16_000;
        let mut samples = Vec::new();
        let first = tone_word(&mut samples, sample_rate, "hello", 0.4, 180);
        silence(&mut samples, sample_rate, 600);
        let second = tone_word(&mut samples, sample_rate, "world", 0.4, 180);
        let words = vec![first, second];

        let plan = plan_cleanup(
            &words,
            &CleanupConfig {
                remove_fillers: false,
                remove_duplicates: false,
                silence_pauses: false,
                trim_pauses: false,
                remove_silence: true,
                pause_threshold_us: None,
                max_gap_us: None,
            },
            &[],
            Some(CleanupAudioContext {
                samples: &samples,
                sample_rate,
            }),
        );

        assert_eq!(plan.silence_count, 1);
        assert_eq!(plan.total_affected, 1);
        assert_eq!(plan.estimated_duration_saved_us, 600_000);
    }

    #[test]
    fn apply_cleanup_plan_marks_words_and_inserts_sentinels() {
        let mut words = vec![
            word("um", 0, 100_000),
            word("hello", 100_000, 300_000),
            word("world", 2_000_000, 2_500_000),
        ];
        let fillers = vec!["um".to_string()];
        let plan = plan_cleanup(
            &words,
            &CleanupConfig {
                remove_fillers: true,
                remove_duplicates: false,
                silence_pauses: false,
                trim_pauses: true,
                remove_silence: false,
                pause_threshold_us: None,
                max_gap_us: None,
            },
            &fillers,
            None,
        );

        let modified = apply_cleanup_plan(&mut words, &plan);

        assert_eq!(modified, 2);
        assert!(words[0].deleted, "filler should be marked deleted");
        assert!(words.iter().any(|w| w.deleted && w.text.is_empty()));
    }

    #[test]
    fn empty_words_return_empty_plan() {
        let plan = plan_cleanup(
            &[],
            &CleanupConfig::from_preset(CleanupPreset::Aggressive),
            &[],
            None,
        );

        assert!(plan.actions.is_empty());
        assert_eq!(plan.total_affected, 0);
        assert_eq!(plan.estimated_duration_saved_us, 0);
    }
}
