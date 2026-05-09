use std::collections::BTreeMap;

use crate::managers::editor::Word;

/// Aggregated speaker metadata for the current transcript.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, specta::Type)]
pub struct SpeakerInfo {
    pub id: i32,
    pub name: String,
    pub word_count: usize,
    pub total_duration_us: i64,
}

/// Turn-taking heuristic configuration for gap-based speaker assignment.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize, specta::Type)]
pub struct SpeakerGapConfig {
    pub min_gap_us: i64,
    pub max_speakers: usize,
}

impl Default for SpeakerGapConfig {
    fn default() -> Self {
        Self {
            min_gap_us: 1_500_000,
            max_speakers: 2,
        }
    }
}

/// Summarize every assigned speaker in ascending speaker-id order.
pub fn get_speaker_stats(words: &[Word]) -> Vec<SpeakerInfo> {
    let mut stats = BTreeMap::<i32, (usize, i64)>::new();

    for word in words.iter().filter(|word| word.speaker_id >= 0) {
        let entry = stats.entry(word.speaker_id).or_insert((0, 0));
        entry.0 += 1;
        entry.1 += (word.end_us - word.start_us).max(0);
    }

    stats
        .into_iter()
        .map(|(id, (word_count, total_duration_us))| SpeakerInfo {
            id,
            name: String::new(),
            word_count,
            total_duration_us,
        })
        .collect()
}

/// Assign alternating speaker IDs whenever a pause exceeds the configured gap.
pub fn assign_speakers_by_gaps(words: &mut [Word], config: &SpeakerGapConfig) {
    if words.is_empty() {
        return;
    }

    let speaker_count = config.max_speakers.max(1) as i32;
    let mut current_speaker = 0;
    let mut previous_end_us: Option<i64> = None;

    for word in words.iter_mut() {
        if let Some(previous_end_us) = previous_end_us {
            let gap_us = word.start_us - previous_end_us;
            if gap_us > config.min_gap_us {
                current_speaker = (current_speaker + 1) % speaker_count;
            }
        }

        word.speaker_id = current_speaker;
        previous_end_us = Some(word.end_us);
    }
}

/// Merge one speaker into another by rewriting matching word speaker IDs.
pub fn merge_speakers(words: &mut [Word], from_id: i32, to_id: i32) {
    if from_id == to_id {
        return;
    }

    for word in words.iter_mut().filter(|word| word.speaker_id == from_id) {
        word.speaker_id = to_id;
    }
}

/// Assign a speaker to an inclusive range of words.
pub fn assign_speaker_range(
    words: &mut [Word],
    start_index: usize,
    end_index: usize,
    speaker_id: i32,
) {
    if start_index > end_index || end_index >= words.len() {
        return;
    }

    for word in &mut words[start_index..=end_index] {
        word.speaker_id = speaker_id;
    }
}

/// Reset every word back to the unknown-speaker sentinel.
pub fn clear_speaker_assignments(words: &mut [Word]) {
    for word in words.iter_mut() {
        word.speaker_id = -1;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_word(text: &str, start_us: i64, end_us: i64, speaker_id: i32) -> Word {
        Word {
            text: text.to_string(),
            start_us,
            end_us,
            deleted: false,
            silenced: false,
            confidence: 1.0,
            speaker_id,
        }
    }

    fn sample_words() -> Vec<Word> {
        vec![
            make_word("hello", 0, 400_000, -1),
            make_word("there", 450_000, 900_000, -1),
            make_word("general", 2_700_000, 3_100_000, -1),
            make_word("kenobi", 3_150_000, 3_600_000, -1),
            make_word("again", 5_500_000, 5_900_000, -1),
        ]
    }

    #[test]
    fn get_speaker_stats_counts_words_and_duration() {
        let words = vec![
            make_word("one", 0, 500_000, 0),
            make_word("two", 500_000, 1_100_000, 0),
            make_word("three", 1_100_000, 1_600_000, 1),
            make_word("unknown", 1_700_000, 1_900_000, -1),
        ];

        let stats = get_speaker_stats(&words);

        assert_eq!(stats.len(), 2);
        assert_eq!(stats[0].id, 0);
        assert_eq!(stats[0].word_count, 2);
        assert_eq!(stats[0].total_duration_us, 1_100_000);
        assert_eq!(stats[1].id, 1);
        assert_eq!(stats[1].word_count, 1);
        assert_eq!(stats[1].total_duration_us, 500_000);
    }

    #[test]
    fn assign_speakers_by_gaps_alternates_between_turns() {
        let mut words = sample_words();

        assign_speakers_by_gaps(
            &mut words,
            &SpeakerGapConfig {
                min_gap_us: 1_500_000,
                max_speakers: 2,
            },
        );

        let speaker_ids: Vec<i32> = words.iter().map(|word| word.speaker_id).collect();
        assert_eq!(speaker_ids, vec![0, 0, 1, 1, 0]);
    }

    #[test]
    fn assign_speakers_by_gaps_cycles_when_max_speakers_is_three() {
        let mut words = sample_words();

        assign_speakers_by_gaps(
            &mut words,
            &SpeakerGapConfig {
                min_gap_us: 1_500_000,
                max_speakers: 3,
            },
        );

        let speaker_ids: Vec<i32> = words.iter().map(|word| word.speaker_id).collect();
        assert_eq!(speaker_ids, vec![0, 0, 1, 1, 2]);
    }

    #[test]
    fn merge_speakers_reassigns_matching_words() {
        let mut words = vec![
            make_word("one", 0, 100_000, 0),
            make_word("two", 100_000, 200_000, 1),
            make_word("three", 200_000, 300_000, 1),
        ];

        merge_speakers(&mut words, 1, 0);

        assert!(words.iter().all(|word| word.speaker_id == 0));
    }

    #[test]
    fn assign_speaker_range_updates_inclusive_slice() {
        let mut words = sample_words();

        assign_speaker_range(&mut words, 1, 3, 4);

        assert_eq!(words[0].speaker_id, -1);
        assert_eq!(words[1].speaker_id, 4);
        assert_eq!(words[2].speaker_id, 4);
        assert_eq!(words[3].speaker_id, 4);
        assert_eq!(words[4].speaker_id, -1);
    }

    #[test]
    fn clear_speaker_assignments_resets_all_words_to_unknown() {
        let mut words = vec![
            make_word("one", 0, 100_000, 0),
            make_word("two", 100_000, 200_000, 1),
        ];

        clear_speaker_assignments(&mut words);

        assert!(words.iter().all(|word| word.speaker_id == -1));
    }
}
