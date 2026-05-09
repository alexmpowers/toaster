//! Validated word sequence — the single trust boundary for word timing.
//!
//! `ValidatedWordSequence` wraps a `Vec<Word>` that has been verified to
//! satisfy all timing invariants. Once constructed, the invariants are
//! guaranteed by construction — callers do not need to re-validate.
//!
//! # Invariants (checked at construction)
//!
//! 1. **Non-empty text**: every word has non-empty trimmed text.
//! 2. **Positive duration**: `start_us < end_us` for every word.
//! 3. **Bounded**: `0 <= start_us` and `end_us <= audio_duration_us`.
//! 4. **Monotonic non-overlapping**: `words[i].end_us <= words[i+1].start_us`.
//! 5. **Minimum duration**: `end_us - start_us >= MIN_WORD_DURATION_US` (1 ms).
//!
//! # Interval convention
//!
//! Word timestamps use half-open intervals: `[start_us, end_us)`.
//! A word is "active" at time T if `start_us <= T < end_us`.
//! This matches the frontend's `EditorView.tsx` `handleTimeUpdate` logic.

use super::Word;
use log::info;
use std::fmt;

/// Minimum duration for a single word (1 ms = 1000 µs).
const MIN_WORD_DURATION_US: i64 = 1_000;

/// Errors that prevent construction of a valid word sequence.
#[derive(Debug)]
pub enum TimingError {
    /// A word has empty or whitespace-only text.
    EmptyText { index: usize },
    /// A word has non-positive duration (start >= end).
    NonPositiveDuration {
        index: usize,
        start_us: i64,
        end_us: i64,
    },
    /// A word's timestamps fall outside [0, audio_duration_us].
    OutOfBounds {
        index: usize,
        start_us: i64,
        end_us: i64,
        audio_duration_us: i64,
    },
    /// Two adjacent words overlap: words[index-1].end_us > words[index].start_us.
    Overlap {
        index: usize,
        prev_end_us: i64,
        this_start_us: i64,
    },
    /// A word's duration is below the minimum threshold.
    TooShort {
        index: usize,
        duration_us: i64,
        min_us: i64,
    },
}

impl fmt::Display for TimingError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::EmptyText { index } => {
                write!(f, "word[{index}] has empty text")
            }
            Self::NonPositiveDuration {
                index,
                start_us,
                end_us,
            } => {
                write!(
                    f,
                    "word[{index}] has non-positive duration: start={start_us}, end={end_us}"
                )
            }
            Self::OutOfBounds {
                index,
                start_us,
                end_us,
                audio_duration_us,
            } => {
                write!(f, "word[{index}] out of bounds: [{start_us}, {end_us}) vs audio [0, {audio_duration_us})")
            }
            Self::Overlap {
                index,
                prev_end_us,
                this_start_us,
            } => {
                write!(f, "word[{index}] overlaps predecessor: prev.end={prev_end_us} > this.start={this_start_us}")
            }
            Self::TooShort {
                index,
                duration_us,
                min_us,
            } => {
                write!(
                    f,
                    "word[{index}] too short: {duration_us}µs < minimum {min_us}µs"
                )
            }
        }
    }
}

/// A word sequence with guaranteed timing invariants.
///
/// Use `ValidatedWordSequence::new()` for strict validation (rejects bad data)
/// or `ValidatedWordSequence::sanitize()` for best-effort repair (fixes what
/// it can, drops what it can't). The strict path is for freshly-aligned words;
/// the repair path is for legacy `.toaster` project files.
#[derive(Debug, Clone)]
pub struct ValidatedWordSequence {
    words: Vec<Word>,
}

impl ValidatedWordSequence {
    /// Strict construction: validate all invariants and return an error on
    /// the first violation. Use this for words coming from the transcription
    /// pipeline (which should produce valid output).
    pub fn new(words: Vec<Word>, audio_duration_us: i64) -> Result<Self, TimingError> {
        let max_us = audio_duration_us.max(0);
        let mut prev_end: i64 = 0;

        for (i, w) in words.iter().enumerate() {
            if w.text.trim().is_empty() {
                return Err(TimingError::EmptyText { index: i });
            }
            if w.start_us < 0 || w.end_us > max_us {
                return Err(TimingError::OutOfBounds {
                    index: i,
                    start_us: w.start_us,
                    end_us: w.end_us,
                    audio_duration_us: max_us,
                });
            }
            if w.start_us >= w.end_us {
                return Err(TimingError::NonPositiveDuration {
                    index: i,
                    start_us: w.start_us,
                    end_us: w.end_us,
                });
            }
            let duration = w.end_us - w.start_us;
            if duration < MIN_WORD_DURATION_US {
                return Err(TimingError::TooShort {
                    index: i,
                    duration_us: duration,
                    min_us: MIN_WORD_DURATION_US,
                });
            }
            if w.start_us < prev_end {
                return Err(TimingError::Overlap {
                    index: i,
                    prev_end_us: prev_end,
                    this_start_us: w.start_us,
                });
            }
            prev_end = w.end_us;
        }

        Ok(Self { words })
    }

    /// Best-effort sanitization: repair timing violations instead of
    /// rejecting. Use for loading legacy `.toaster` project files that may
    /// contain invalid data from older code paths.
    ///
    /// Repairs applied (in order):
    /// 1. Strip words with empty/whitespace-only text
    /// 2. Clamp timestamps to `[0, audio_duration_us]`
    /// 3. Enforce `start < end` (swap if inverted)
    /// 4. Enforce monotonic non-overlapping progression
    /// 5. Ensure minimum 1 ms duration
    pub fn sanitize(mut words: Vec<Word>, audio_duration_us: i64) -> Self {
        let max_us = audio_duration_us.max(0);
        let original_count = words.len();

        // 1. Strip empty-text words
        words.retain(|w| !w.text.trim().is_empty());

        let stripped = original_count - words.len();
        if stripped > 0 {
            info!(
                "ValidatedWordSequence::sanitize: stripped {stripped} empty-text words \
                 ({original_count} → {})",
                words.len()
            );
        }

        // 2-5. Enforce timing invariants in a single forward pass
        let mut cursor_us: i64 = 0;
        for word in words.iter_mut() {
            // Clamp to audio bounds
            word.start_us = word.start_us.clamp(0, max_us);
            word.end_us = word.end_us.clamp(0, max_us);

            // Enforce start < end
            if word.start_us > word.end_us {
                std::mem::swap(&mut word.start_us, &mut word.end_us);
            }

            // Enforce monotonic progression
            if word.start_us < cursor_us {
                word.start_us = cursor_us;
                word.start_us = word.start_us.min(max_us);
                if word.end_us < word.start_us {
                    word.end_us = word.start_us;
                }
            }

            // Ensure minimum duration
            if word.end_us - word.start_us < MIN_WORD_DURATION_US
                && word.start_us + MIN_WORD_DURATION_US <= max_us
            {
                word.end_us = word.start_us + MIN_WORD_DURATION_US;
            }

            cursor_us = word.end_us;
        }

        Self { words }
    }

    /// Consume the wrapper and return the underlying `Vec<Word>`.
    /// Use at Tauri command boundaries where `Vec<Word>` is required.
    pub fn into_inner(self) -> Vec<Word> {
        self.words
    }

    /// Borrow the validated words as a slice.
    pub fn as_slice(&self) -> &[Word] {
        &self.words
    }

    /// Number of words in the sequence.
    pub fn len(&self) -> usize {
        self.words.len()
    }

    /// Whether the sequence is empty.
    pub fn is_empty(&self) -> bool {
        self.words.is_empty()
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
            confidence: -1.0,
            speaker_id: -1,
        }
    }

    #[test]
    fn valid_sequence_accepted() {
        let words = vec![word("hello", 0, 500_000), word("world", 500_000, 1_000_000)];
        let seq = ValidatedWordSequence::new(words, 1_000_000);
        assert!(seq.is_ok());
        assert_eq!(seq.unwrap().len(), 2);
    }

    #[test]
    fn rejects_empty_text() {
        let words = vec![word("", 0, 100_000)];
        let err = ValidatedWordSequence::new(words, 1_000_000).unwrap_err();
        assert!(matches!(err, TimingError::EmptyText { index: 0 }));
    }

    #[test]
    fn rejects_overlap() {
        let words = vec![
            word("hello", 0, 600_000),
            word("world", 500_000, 1_000_000), // overlaps
        ];
        let err = ValidatedWordSequence::new(words, 1_000_000).unwrap_err();
        assert!(matches!(err, TimingError::Overlap { index: 1, .. }));
    }

    #[test]
    fn rejects_non_positive_duration() {
        let words = vec![word("hello", 500_000, 500_000)]; // zero duration
        let err = ValidatedWordSequence::new(words, 1_000_000).unwrap_err();
        assert!(matches!(err, TimingError::NonPositiveDuration { .. }));
    }

    #[test]
    fn rejects_out_of_bounds() {
        let words = vec![word("hello", 0, 2_000_000)]; // exceeds audio duration
        let err = ValidatedWordSequence::new(words, 1_000_000).unwrap_err();
        assert!(matches!(err, TimingError::OutOfBounds { .. }));
    }

    #[test]
    fn sanitize_strips_empty_text() {
        let words = vec![
            word("hello", 0, 500_000),
            word("", 500_000, 600_000),   // empty — stripped
            word("  ", 600_000, 700_000), // whitespace — stripped
            word("world", 700_000, 1_000_000),
        ];
        let seq = ValidatedWordSequence::sanitize(words, 1_000_000);
        assert_eq!(seq.len(), 2);
        assert_eq!(seq.as_slice()[0].text, "hello");
        assert_eq!(seq.as_slice()[1].text, "world");
    }

    #[test]
    fn sanitize_fixes_overlaps() {
        let words = vec![
            word("hello", 0, 600_000),
            word("world", 400_000, 1_000_000), // overlaps — clamped
        ];
        let seq = ValidatedWordSequence::sanitize(words, 1_000_000);
        let w = seq.as_slice();
        assert!(w[0].end_us <= w[1].start_us, "overlap not resolved");
    }

    #[test]
    fn sanitize_ensures_minimum_duration() {
        let words = vec![word("hi", 0, 100)]; // 100 µs < 1000 µs minimum
        let seq = ValidatedWordSequence::sanitize(words, 1_000_000);
        let dur = seq.as_slice()[0].end_us - seq.as_slice()[0].start_us;
        assert!(dur >= 1_000, "duration {dur} below minimum");
    }

    #[test]
    fn into_inner_returns_vec() {
        let words = vec![word("test", 0, 500_000)];
        let seq = ValidatedWordSequence::new(words, 500_000).unwrap();
        let inner = seq.into_inner();
        assert_eq!(inner.len(), 1);
        assert_eq!(inner[0].text, "test");
    }
}
