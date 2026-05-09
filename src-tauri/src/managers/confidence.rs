//! Confidence-based word review support.

use crate::managers::editor::Word;

/// A low-confidence word returned to the review UI.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, specta::Type)]
pub struct LowConfidenceWord {
    /// Index into the editor word list.
    pub word_index: usize,
    /// Word text as shown in the transcript.
    pub text: String,
    /// ASR confidence score in the range 0.0-1.0.
    pub confidence: f32,
    /// Word start timestamp in microseconds.
    pub start_us: i64,
    /// Word end timestamp in microseconds.
    pub end_us: i64,
}

/// Returns whether the word at `index` can be marked as verified.
pub(crate) fn can_mark_word_verified(words: &[Word], index: usize) -> bool {
    words
        .get(index)
        .is_some_and(|word| !word.deleted && word.confidence < 1.0)
}

/// Find every active word below `threshold`, sorted from least to most certain.
pub fn find_low_confidence_words(words: &[Word], threshold: f32) -> Vec<LowConfidenceWord> {
    let mut results: Vec<LowConfidenceWord> = words
        .iter()
        .enumerate()
        .filter(|(_, word)| {
            !word.deleted && !word.silenced && word.confidence >= 0.0 && word.confidence < threshold
        })
        .map(|(word_index, word)| LowConfidenceWord {
            word_index,
            text: word.text.clone(),
            confidence: word.confidence,
            start_us: word.start_us,
            end_us: word.end_us,
        })
        .collect();

    results.sort_by(|left, right| left.confidence.total_cmp(&right.confidence));
    results
}

/// Mark a word as fully verified by setting confidence to `1.0`.
pub fn mark_word_verified(words: &mut [Word], index: usize) -> bool {
    let Some(word) = words.get_mut(index) else {
        return false;
    };
    if word.deleted || word.confidence >= 1.0 {
        return false;
    }

    word.confidence = 1.0;
    true
}

/// Mark every eligible index as verified and return the number of updates.
pub fn mark_words_verified(words: &mut [Word], indices: &[usize]) -> usize {
    indices
        .iter()
        .filter(|&&index| mark_word_verified(words, index))
        .count()
}

#[cfg(test)]
mod tests {
    use super::{find_low_confidence_words, mark_word_verified, mark_words_verified};
    use crate::managers::editor::Word;

    fn word(text: &str, confidence: f32) -> Word {
        Word {
            text: text.to_string(),
            start_us: 0,
            end_us: 1_000_000,
            deleted: false,
            silenced: false,
            confidence,
            speaker_id: -1,
        }
    }

    #[test]
    fn find_low_confidence_words_sorts_lowest_first_and_skips_unknown_deleted_and_silenced() {
        let mut deleted = word("deleted", 0.2);
        deleted.deleted = true;
        let mut silenced = word("silenced", 0.1);
        silenced.silenced = true;

        let words = vec![
            word("strong", 0.9),
            word("weak", 0.3),
            word("medium", 0.6),
            word("unknown", -1.0),
            deleted,
            silenced,
        ];

        let results = find_low_confidence_words(&words, 0.7);

        assert_eq!(results.len(), 2);
        assert_eq!(results[0].text, "weak");
        assert_eq!(results[0].word_index, 1);
        assert_eq!(results[1].text, "medium");
        assert_eq!(results[1].word_index, 2);
    }

    #[test]
    fn mark_word_verified_updates_only_eligible_words() {
        let mut words = vec![word("review", 0.4), word("done", 1.0)];

        assert!(mark_word_verified(&mut words, 0));
        assert_eq!(words[0].confidence, 1.0);
        assert!(!mark_word_verified(&mut words, 0));
        assert!(!mark_word_verified(&mut words, 1));
        assert!(!mark_word_verified(&mut words, 99));
    }

    #[test]
    fn mark_words_verified_counts_each_successful_update_once() {
        let mut words = vec![word("first", 0.2), word("second", 0.4), word("done", 1.0)];

        let updated = mark_words_verified(&mut words, &[0, 1, 1, 2, 99]);

        assert_eq!(updated, 2);
        assert_eq!(words[0].confidence, 1.0);
        assert_eq!(words[1].confidence, 1.0);
        assert_eq!(words[2].confidence, 1.0);
    }
}
