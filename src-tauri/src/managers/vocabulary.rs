//! Vocabulary-based post-transcription correction.
//!
//! After ASR produces a word list, this module scans for low-confidence words
//! that fuzzy-match entries in the user's custom vocabulary and suggests or
//! applies corrections without modifying timestamps.

use std::collections::HashSet;

use crate::audio_toolkit::text::find_best_match;
use crate::managers::editor::Word;

/// A suggested correction for a single word.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, specta::Type)]
pub struct VocabularyCorrection {
    /// Index into the word list.
    pub word_index: usize,
    /// Original word text from ASR.
    pub original_text: String,
    /// Suggested replacement from custom vocabulary.
    pub suggested_text: String,
    /// Match quality score (lower = better match).
    pub match_score: f64,
    /// Word confidence from ASR.
    pub word_confidence: f32,
}

fn normalize_candidate(text: &str) -> String {
    text.trim_matches(|c: char| !c.is_alphanumeric())
        .chars()
        .filter(|c| !c.is_whitespace())
        .collect::<String>()
        .to_lowercase()
}

fn build_match_index(custom_vocabulary: &[String]) -> Vec<String> {
    custom_vocabulary
        .iter()
        .map(|word| word.to_lowercase().replace(' ', ""))
        .collect()
}

/// Scan words for potential vocabulary corrections.
///
/// Only considers words with confidence below `confidence_threshold`
/// (or all words if threshold is 1.0). Returns corrections sorted by
/// match quality (best first).
pub fn find_vocabulary_corrections(
    words: &[Word],
    custom_vocabulary: &[String],
    match_threshold: f64,
    confidence_threshold: f32,
) -> Vec<VocabularyCorrection> {
    if words.is_empty() || custom_vocabulary.is_empty() {
        return Vec::new();
    }

    let custom_vocabulary_nospace = build_match_index(custom_vocabulary);
    let include_all_words = confidence_threshold >= 1.0;

    let mut corrections: Vec<VocabularyCorrection> = words
        .iter()
        .enumerate()
        .filter_map(|(word_index, word)| {
            if word.deleted || word.silenced {
                return None;
            }

            if !include_all_words && word.confidence >= confidence_threshold {
                return None;
            }

            let candidate = normalize_candidate(&word.text);
            let (suggested_text, match_score) = find_best_match(
                &candidate,
                custom_vocabulary,
                &custom_vocabulary_nospace,
                match_threshold,
            )?;

            if suggested_text == &word.text {
                return None;
            }

            Some(VocabularyCorrection {
                word_index,
                original_text: word.text.clone(),
                suggested_text: suggested_text.clone(),
                match_score,
                word_confidence: word.confidence,
            })
        })
        .collect();

    corrections.sort_by(|left, right| {
        left.match_score
            .total_cmp(&right.match_score)
            .then_with(|| left.word_index.cmp(&right.word_index))
    });
    corrections
}

fn correction_applies(word: &Word, correction: &VocabularyCorrection) -> bool {
    !word.deleted
        && !word.silenced
        && word.text == correction.original_text
        && word.text != correction.suggested_text
}

/// Count how many corrections would apply to the current word list.
pub fn count_applicable_corrections(words: &[Word], corrections: &[VocabularyCorrection]) -> usize {
    let mut applicable = 0;
    let mut applied_indices = HashSet::new();

    for correction in corrections {
        if applied_indices.contains(&correction.word_index) {
            continue;
        }

        if words
            .get(correction.word_index)
            .is_some_and(|word| correction_applies(word, correction))
        {
            applicable += 1;
            applied_indices.insert(correction.word_index);
        }
    }

    applicable
}

/// Apply corrections to word texts without modifying timestamps.
/// Returns the number of words corrected.
pub fn apply_corrections(words: &mut [Word], corrections: &[VocabularyCorrection]) -> usize {
    let mut applied = 0;
    let mut applied_indices = HashSet::new();

    for correction in corrections {
        if applied_indices.contains(&correction.word_index) {
            continue;
        }

        let Some(word) = words.get_mut(correction.word_index) else {
            continue;
        };

        if !correction_applies(word, correction) {
            continue;
        }

        word.text = correction.suggested_text.clone();
        applied += 1;
        applied_indices.insert(correction.word_index);
    }

    applied
}

#[cfg(test)]
mod tests {
    use super::*;

    fn word(text: &str, confidence: f32, start_us: i64, end_us: i64) -> Word {
        Word {
            text: text.to_string(),
            start_us,
            end_us,
            deleted: false,
            silenced: false,
            confidence,
            speaker_id: -1,
        }
    }

    #[test]
    fn finds_low_confidence_vocabulary_corrections() {
        let words = vec![
            word("charg bee", 0.42, 0, 1_000_000),
            word("stable", 0.95, 1_000_000, 2_000_000),
        ];
        let custom_vocabulary = vec!["ChargeBee".to_string(), "OpenAI".to_string()];

        let corrections = find_vocabulary_corrections(&words, &custom_vocabulary, 0.5, 0.8);

        assert_eq!(corrections.len(), 1);
        assert_eq!(corrections[0].word_index, 0);
        assert_eq!(corrections[0].original_text, "charg bee");
        assert_eq!(corrections[0].suggested_text, "ChargeBee");
        assert!(corrections[0].match_score < 0.5);
    }

    #[test]
    fn apply_corrections_preserves_timestamps() {
        let mut words = vec![word("chat gpt", 0.31, 125, 875)];
        let corrections = vec![VocabularyCorrection {
            word_index: 0,
            original_text: "chat gpt".to_string(),
            suggested_text: "ChatGPT".to_string(),
            match_score: 0.1,
            word_confidence: 0.31,
        }];

        let applied = apply_corrections(&mut words, &corrections);

        assert_eq!(applied, 1);
        assert_eq!(words[0].text, "ChatGPT");
        assert_eq!(words[0].start_us, 125);
        assert_eq!(words[0].end_us, 875);
    }

    #[test]
    fn respects_confidence_threshold_filtering() {
        let words = vec![word("Open A I", 0.94, 0, 1_000_000)];
        let custom_vocabulary = vec!["OpenAI".to_string()];

        let corrections = find_vocabulary_corrections(&words, &custom_vocabulary, 0.5, 0.8);

        assert!(corrections.is_empty());
    }

    #[test]
    fn returns_no_corrections_for_empty_vocabulary() {
        let words = vec![word("Open A I", 0.2, 0, 1_000_000)];

        let corrections = find_vocabulary_corrections(&words, &[], 0.5, 0.8);

        assert!(corrections.is_empty());
    }

    #[test]
    fn matches_case_insensitively() {
        let words = vec![word("chatgpt", 0.2, 0, 1_000_000)];
        let custom_vocabulary = vec!["ChatGPT".to_string()];

        let corrections = find_vocabulary_corrections(&words, &custom_vocabulary, 0.1, 1.0);

        assert_eq!(corrections.len(), 1);
        assert_eq!(corrections[0].suggested_text, "ChatGPT");
        assert_eq!(corrections[0].match_score, 0.0);
    }

    #[test]
    fn skips_deleted_and_silenced_words() {
        let mut deleted = word("charg bee", 0.2, 0, 1_000_000);
        deleted.deleted = true;
        let mut silenced = word("chat gpt", 0.2, 1_000_000, 2_000_000);
        silenced.silenced = true;

        let corrections = find_vocabulary_corrections(
            &[deleted, silenced],
            &["ChargeBee".to_string(), "ChatGPT".to_string()],
            0.5,
            1.0,
        );

        assert!(corrections.is_empty());
    }

    #[test]
    fn counts_only_applicable_corrections() {
        let words = vec![word("charg bee", 0.2, 0, 1_000_000)];
        let corrections = vec![
            VocabularyCorrection {
                word_index: 0,
                original_text: "wrong".to_string(),
                suggested_text: "ChargeBee".to_string(),
                match_score: 0.1,
                word_confidence: 0.2,
            },
            VocabularyCorrection {
                word_index: 0,
                original_text: "charg bee".to_string(),
                suggested_text: "ChargeBee".to_string(),
                match_score: 0.1,
                word_confidence: 0.2,
            },
        ];

        assert_eq!(count_applicable_corrections(&words, &corrections), 1);
    }
}
