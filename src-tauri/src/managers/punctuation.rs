//! Rule-based sentence boundary detection and punctuation insertion.
//!
//! Detects sentence, paragraph, and clause boundaries in a transcript word
//! list using inter-word pauses and capitalization cues. Punctuation is
//! appended to word text in place without modifying timestamps.

use crate::managers::editor::Word;

/// Default pause threshold for sentence boundaries (0.8 s).
pub const DEFAULT_SENTENCE_GAP_US: i64 = 800_000;
/// Default pause threshold for paragraph boundaries (2.0 s).
pub const DEFAULT_PARAGRAPH_GAP_US: i64 = 2_000_000;
/// Default pause threshold for clause/comma boundaries (0.4 s).
pub const DEFAULT_COMMA_GAP_US: i64 = 400_000;

/// Configuration for punctuation analysis.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize, specta::Type)]
pub struct PunctuationConfig {
    /// Minimum gap between words to consider a sentence boundary.
    pub sentence_gap_us: i64,
    /// Minimum gap between words to insert a paragraph break.
    pub paragraph_gap_us: i64,
    /// Whether to insert periods at sentence and paragraph boundaries.
    pub insert_periods: bool,
    /// Whether to insert commas at clause boundaries.
    pub insert_commas: bool,
    /// Minimum gap between words to consider a clause boundary.
    pub comma_gap_us: i64,
}

impl Default for PunctuationConfig {
    fn default() -> Self {
        Self {
            sentence_gap_us: DEFAULT_SENTENCE_GAP_US,
            paragraph_gap_us: DEFAULT_PARAGRAPH_GAP_US,
            insert_periods: true,
            insert_commas: true,
            comma_gap_us: DEFAULT_COMMA_GAP_US,
        }
    }
}

/// A detected sentence, paragraph, or clause boundary.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize, specta::Type)]
pub struct SentenceBoundary {
    /// Index of the last word before the detected gap.
    pub word_index: usize,
    /// Duration of the gap to the next active word, in microseconds.
    pub gap_us: i64,
    /// Classification for the detected boundary.
    pub boundary_type: BoundaryType,
    /// Whether applying punctuation would mutate the word text.
    pub punctuation_inserted: bool,
}

/// Boundary type inferred from a gap between active words.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize, specta::Type)]
pub enum BoundaryType {
    /// A sentence-ending boundary.
    Sentence,
    /// A paragraph boundary caused by a longer pause.
    Paragraph,
    /// A clause-level boundary represented by a comma.
    Clause,
}

/// Aggregate punctuation analysis for a transcript word list.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize, specta::Type)]
pub struct PunctuationAnalysis {
    pub boundaries: Vec<SentenceBoundary>,
    pub sentence_count: usize,
    pub paragraph_count: usize,
    pub punctuation_insertions: usize,
}

fn has_terminal_punctuation(text: &str) -> bool {
    text.trim_end()
        .chars()
        .last()
        .is_some_and(|c| matches!(c, '.' | '!' | '?' | '…'))
}

fn has_clause_punctuation(text: &str) -> bool {
    text.trim_end()
        .chars()
        .last()
        .is_some_and(|c| matches!(c, ',' | ';' | ':' | '—' | '–'))
}

fn starts_with_uppercase(text: &str) -> bool {
    text.trim_start()
        .chars()
        .next()
        .is_some_and(|c| c.is_uppercase())
}

fn gap_between_words(current_word: &Word, next_word: &Word) -> Option<i64> {
    let gap_us = next_word.start_us - current_word.end_us;
    (gap_us >= 0).then_some(gap_us)
}

fn sentence_insertion_needed(text: &str, insert_periods: bool) -> bool {
    insert_periods && !has_terminal_punctuation(text) && !has_clause_punctuation(text)
}

fn clause_insertion_needed(text: &str, insert_commas: bool) -> bool {
    insert_commas && !has_terminal_punctuation(text) && !has_clause_punctuation(text)
}

/// Detect sentence, paragraph, and clause boundaries.
pub fn detect_boundaries(words: &[Word], config: &PunctuationConfig) -> PunctuationAnalysis {
    let active_words: Vec<(usize, &Word)> = words
        .iter()
        .enumerate()
        .filter(|(_, word)| !word.deleted && !word.silenced)
        .collect();

    let mut boundaries = Vec::new();

    for window in active_words.windows(2) {
        let (word_index, current_word) = window[0];
        let (_, next_word) = window[1];
        let Some(gap_us) = gap_between_words(current_word, next_word) else {
            continue;
        };

        if gap_us >= config.paragraph_gap_us {
            boundaries.push(SentenceBoundary {
                word_index,
                gap_us,
                boundary_type: BoundaryType::Paragraph,
                punctuation_inserted: sentence_insertion_needed(
                    &current_word.text,
                    config.insert_periods,
                ),
            });
            continue;
        }

        if gap_us >= config.sentence_gap_us {
            let next_is_uppercase = starts_with_uppercase(&next_word.text);
            if next_is_uppercase || gap_us >= config.sentence_gap_us * 3 / 2 {
                boundaries.push(SentenceBoundary {
                    word_index,
                    gap_us,
                    boundary_type: BoundaryType::Sentence,
                    punctuation_inserted: sentence_insertion_needed(
                        &current_word.text,
                        config.insert_periods,
                    ),
                });
                continue;
            }
        }

        if gap_us >= config.comma_gap_us
            && clause_insertion_needed(&current_word.text, config.insert_commas)
        {
            boundaries.push(SentenceBoundary {
                word_index,
                gap_us,
                boundary_type: BoundaryType::Clause,
                punctuation_inserted: true,
            });
        }
    }

    let sentence_count = boundaries
        .iter()
        .filter(|boundary| {
            matches!(
                boundary.boundary_type,
                BoundaryType::Sentence | BoundaryType::Paragraph
            )
        })
        .count();
    let paragraph_count = boundaries
        .iter()
        .filter(|boundary| matches!(boundary.boundary_type, BoundaryType::Paragraph))
        .count();
    let punctuation_insertions = boundaries
        .iter()
        .filter(|boundary| boundary.punctuation_inserted)
        .count();

    PunctuationAnalysis {
        boundaries,
        sentence_count,
        paragraph_count,
        punctuation_insertions,
    }
}

/// Count how many proposed boundaries would modify the current word list.
pub fn count_applicable_insertions(words: &[Word], boundaries: &[SentenceBoundary]) -> usize {
    boundaries
        .iter()
        .filter(|boundary| boundary.punctuation_inserted)
        .filter_map(|boundary| words.get(boundary.word_index))
        .filter(|word| {
            !word.deleted
                && !word.silenced
                && !word.text.trim_end().is_empty()
                && !has_terminal_punctuation(&word.text)
                && !has_clause_punctuation(&word.text)
        })
        .count()
}

/// Apply punctuation to the word list without modifying timestamps.
pub fn apply_punctuation(words: &mut [Word], boundaries: &[SentenceBoundary]) -> usize {
    let mut modified = 0;

    for boundary in boundaries {
        if !boundary.punctuation_inserted {
            continue;
        }

        let Some(word) = words.get_mut(boundary.word_index) else {
            continue;
        };

        if word.deleted || word.silenced {
            continue;
        }

        let trimmed = word.text.trim_end();
        if trimmed.is_empty()
            || has_terminal_punctuation(trimmed)
            || has_clause_punctuation(trimmed)
        {
            continue;
        }

        let suffix = match boundary.boundary_type {
            BoundaryType::Paragraph | BoundaryType::Sentence => ".",
            BoundaryType::Clause => ",",
        };
        word.text = format!("{trimmed}{suffix}");
        modified += 1;
    }

    modified
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

    fn deleted_word(text: &str, start_us: i64, end_us: i64) -> Word {
        Word {
            deleted: true,
            ..word(text, start_us, end_us)
        }
    }

    fn silenced_word(text: &str, start_us: i64, end_us: i64) -> Word {
        Word {
            silenced: true,
            ..word(text, start_us, end_us)
        }
    }

    #[test]
    fn detects_sentence_boundaries_from_gaps() {
        let words = vec![
            word("hello", 0, 300_000),
            word("World", 1_350_000, 1_700_000),
            word("again", 1_800_000, 2_100_000),
        ];

        let analysis = detect_boundaries(&words, &PunctuationConfig::default());

        assert_eq!(analysis.boundaries.len(), 1);
        assert_eq!(analysis.boundaries[0].word_index, 0);
        assert_eq!(analysis.boundaries[0].gap_us, 1_050_000);
        assert_eq!(analysis.boundaries[0].boundary_type, BoundaryType::Sentence);
        assert!(analysis.boundaries[0].punctuation_inserted);
        assert_eq!(analysis.sentence_count, 1);
        assert_eq!(analysis.paragraph_count, 0);
        assert_eq!(analysis.punctuation_insertions, 1);
    }

    #[test]
    fn detects_paragraph_boundaries_from_long_gaps() {
        let words = vec![
            word("hello", 0, 300_000),
            word("Next", 2_500_000, 2_900_000),
        ];

        let analysis = detect_boundaries(&words, &PunctuationConfig::default());

        assert_eq!(analysis.boundaries.len(), 1);
        assert_eq!(
            analysis.boundaries[0].boundary_type,
            BoundaryType::Paragraph
        );
        assert_eq!(analysis.boundaries[0].gap_us, 2_200_000);
        assert_eq!(analysis.sentence_count, 1);
        assert_eq!(analysis.paragraph_count, 1);
    }

    #[test]
    fn skips_already_punctuated_terminal_words() {
        let words = vec![
            word("hello!", 0, 300_000),
            word("World", 1_200_000, 1_500_000),
        ];

        let analysis = detect_boundaries(&words, &PunctuationConfig::default());

        assert_eq!(analysis.boundaries.len(), 1);
        assert!(!analysis.boundaries[0].punctuation_inserted);
        assert_eq!(analysis.punctuation_insertions, 0);
    }

    #[test]
    fn detects_clause_boundaries_for_medium_gaps() {
        let words = vec![word("however", 0, 200_000), word("still", 700_000, 900_000)];

        let analysis = detect_boundaries(&words, &PunctuationConfig::default());

        assert_eq!(analysis.boundaries.len(), 1);
        assert_eq!(analysis.boundaries[0].boundary_type, BoundaryType::Clause);
        assert_eq!(analysis.sentence_count, 0);
        assert_eq!(analysis.paragraph_count, 0);
        assert_eq!(analysis.punctuation_insertions, 1);
    }

    #[test]
    fn skips_deleted_and_silenced_words_when_scanning_gaps() {
        let words = vec![
            word("alpha", 0, 100_000),
            deleted_word("beta", 200_000, 300_000),
            silenced_word("gamma", 400_000, 500_000),
            word("Delta", 1_450_000, 1_700_000),
        ];

        let analysis = detect_boundaries(&words, &PunctuationConfig::default());

        assert_eq!(analysis.boundaries.len(), 1);
        assert_eq!(analysis.boundaries[0].word_index, 0);
        assert_eq!(analysis.boundaries[0].boundary_type, BoundaryType::Sentence);
    }

    #[test]
    fn returns_no_boundaries_for_empty_single_or_tight_word_lists() {
        assert!(detect_boundaries(&[], &PunctuationConfig::default())
            .boundaries
            .is_empty());
        assert!(
            detect_boundaries(&[word("solo", 0, 100_000)], &PunctuationConfig::default())
                .boundaries
                .is_empty()
        );
        assert!(detect_boundaries(
            &[
                word("one", 0, 100_000),
                word("two", 150_000, 250_000),
                word("three", 300_000, 400_000),
            ],
            &PunctuationConfig::default(),
        )
        .boundaries
        .is_empty());
    }

    #[test]
    fn apply_punctuation_does_not_double_punctuate() {
        let mut words = vec![
            word("hello!", 0, 100_000),
            word("world", 1_000_000, 1_100_000),
        ];
        let boundaries = vec![SentenceBoundary {
            word_index: 0,
            gap_us: 900_000,
            boundary_type: BoundaryType::Sentence,
            punctuation_inserted: true,
        }];

        let modified = apply_punctuation(&mut words, &boundaries);

        assert_eq!(modified, 0);
        assert_eq!(words[0].text, "hello!");
    }

    #[test]
    fn apply_punctuation_preserves_timestamps() {
        let mut words = vec![word("hello", 123, 456), word("World", 1_500_000, 1_800_000)];
        let boundaries = vec![SentenceBoundary {
            word_index: 0,
            gap_us: 1_499_544,
            boundary_type: BoundaryType::Sentence,
            punctuation_inserted: true,
        }];

        let modified = apply_punctuation(&mut words, &boundaries);

        assert_eq!(modified, 1);
        assert_eq!(words[0].text, "hello.");
        assert_eq!(words[0].start_us, 123);
        assert_eq!(words[0].end_us, 456);
    }

    #[test]
    fn count_applicable_insertions_skips_ineligible_words() {
        let words = vec![
            word("ready", 0, 100_000),
            deleted_word("gone", 200_000, 300_000),
            silenced_word("quiet", 400_000, 500_000),
            word("done,", 600_000, 700_000),
        ];
        let boundaries = vec![
            SentenceBoundary {
                word_index: 0,
                gap_us: 900_000,
                boundary_type: BoundaryType::Sentence,
                punctuation_inserted: true,
            },
            SentenceBoundary {
                word_index: 1,
                gap_us: 900_000,
                boundary_type: BoundaryType::Sentence,
                punctuation_inserted: true,
            },
            SentenceBoundary {
                word_index: 2,
                gap_us: 900_000,
                boundary_type: BoundaryType::Sentence,
                punctuation_inserted: true,
            },
            SentenceBoundary {
                word_index: 3,
                gap_us: 900_000,
                boundary_type: BoundaryType::Sentence,
                punctuation_inserted: true,
            },
        ];

        assert_eq!(count_applicable_insertions(&words, &boundaries), 1);
    }
}
