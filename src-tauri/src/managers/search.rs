//! Transcript search helpers for exact, fuzzy, and phonetic find flows.

use natural::phonetics::soundex;
use strsim::levenshtein;

use crate::audio_toolkit::text::find_best_match;
use crate::managers::editor::Word;

const DEFAULT_MAX_DISTANCE: usize = 2;
const PHONETIC_MATCH_SCORE: f64 = 0.5;

/// Available transcript search strategies.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize, specta::Type)]
pub enum SearchMode {
    Exact,
    Fuzzy,
    Phonetic,
}

/// A single transcript word match.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, specta::Type)]
pub struct SearchMatch {
    /// Index of the matched word in the editor word list.
    pub word_index: usize,
    /// Original word text.
    pub text: String,
    /// Lower is better. Exact matches return 0.0.
    pub match_score: f64,
    /// Search strategy that produced the match.
    pub match_type: SearchMode,
}

/// Search response payload returned to the frontend.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, specta::Type)]
pub struct SearchResult {
    pub matches: Vec<SearchMatch>,
    pub total_count: usize,
}

fn normalize_candidate(text: &str) -> String {
    text.trim_matches(|c: char| !c.is_alphanumeric())
        .chars()
        .filter(|c| !c.is_whitespace())
        .collect::<String>()
        .to_lowercase()
}

fn exact_match(word: &Word, query_lower: &str, word_index: usize) -> Option<SearchMatch> {
    let word_lower = word.text.to_lowercase();
    if !word_lower.contains(query_lower) {
        return None;
    }

    Some(SearchMatch {
        word_index,
        text: word.text.clone(),
        match_score: 0.0,
        match_type: SearchMode::Exact,
    })
}

fn fuzzy_match(
    word: &Word,
    query_normalized: &str,
    max_distance: usize,
    word_index: usize,
) -> Option<SearchMatch> {
    let normalized_word = normalize_candidate(&word.text);
    if normalized_word.is_empty() {
        return None;
    }

    let max_len = query_normalized.len().max(normalized_word.len()).max(1);
    let threshold = (max_distance as f64 + f64::EPSILON) / max_len as f64;
    let original_words = [word.text.clone()];
    let normalized_words = [normalized_word.clone()];
    find_best_match(
        query_normalized,
        &original_words,
        &normalized_words,
        threshold,
    )?;

    let distance = levenshtein(query_normalized, &normalized_word);
    if distance > max_distance {
        return None;
    }

    Some(SearchMatch {
        word_index,
        text: word.text.clone(),
        match_score: distance as f64 / max_len as f64,
        match_type: SearchMode::Fuzzy,
    })
}

fn phonetic_match(word: &Word, query_normalized: &str, word_index: usize) -> Option<SearchMatch> {
    let normalized_word = normalize_candidate(&word.text);
    if normalized_word.is_empty() || !soundex(query_normalized, &normalized_word) {
        return None;
    }

    Some(SearchMatch {
        word_index,
        text: word.text.clone(),
        match_score: PHONETIC_MATCH_SCORE,
        match_type: SearchMode::Phonetic,
    })
}

/// Search transcript words for the provided query.
pub fn search_words(
    words: &[Word],
    query: &str,
    mode: SearchMode,
    max_distance: Option<usize>,
) -> SearchResult {
    let query_lower = query.trim().to_lowercase();
    let query_normalized = normalize_candidate(query);
    if query_lower.is_empty() || query_normalized.is_empty() {
        return SearchResult {
            matches: Vec::new(),
            total_count: 0,
        };
    }

    let matches: Vec<SearchMatch> = words
        .iter()
        .enumerate()
        .filter(|(_, word)| !word.deleted && !word.silenced)
        .filter_map(|(word_index, word)| {
            if let Some(exact) = exact_match(word, &query_lower, word_index) {
                return Some(exact);
            }

            match mode {
                SearchMode::Exact => None,
                SearchMode::Fuzzy => fuzzy_match(
                    word,
                    &query_normalized,
                    max_distance.unwrap_or(DEFAULT_MAX_DISTANCE),
                    word_index,
                ),
                SearchMode::Phonetic => phonetic_match(word, &query_normalized, word_index),
            }
        })
        .collect();

    SearchResult {
        total_count: matches.len(),
        matches,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn word(text: &str) -> Word {
        Word {
            text: text.to_string(),
            start_us: 0,
            end_us: 1_000_000,
            deleted: false,
            silenced: false,
            confidence: 1.0,
            speaker_id: -1,
        }
    }

    #[test]
    fn exact_search_matches_case_insensitively() {
        let words = vec![word("Hello"), word("world")];

        let result = search_words(&words, "hello", SearchMode::Exact, None);

        assert_eq!(result.total_count, 1);
        assert_eq!(result.matches[0].word_index, 0);
        assert_eq!(result.matches[0].match_type, SearchMode::Exact);
        assert_eq!(result.matches[0].match_score, 0.0);
    }

    #[test]
    fn fuzzy_search_matches_within_distance_threshold() {
        let words = vec![word("helo"), word("world")];

        let result = search_words(&words, "hello", SearchMode::Fuzzy, Some(1));

        assert_eq!(result.total_count, 1);
        assert_eq!(result.matches[0].word_index, 0);
        assert_eq!(result.matches[0].match_type, SearchMode::Fuzzy);
        assert!(result.matches[0].match_score > 0.0);
    }

    #[test]
    fn fuzzy_search_rejects_distant_matches() {
        let words = vec![word("world")];

        let result = search_words(&words, "hello", SearchMode::Fuzzy, Some(1));

        assert!(result.matches.is_empty());
        assert_eq!(result.total_count, 0);
    }

    #[test]
    fn phonetic_search_matches_soundalikes() {
        let words = vec![word("rupert")];

        let result = search_words(&words, "robert", SearchMode::Phonetic, None);

        assert_eq!(result.total_count, 1);
        assert_eq!(result.matches[0].word_index, 0);
        assert_eq!(result.matches[0].match_type, SearchMode::Phonetic);
    }

    #[test]
    fn search_skips_deleted_and_silenced_words() {
        let mut deleted = word("hello");
        deleted.deleted = true;
        let mut silenced = word("hello");
        silenced.silenced = true;
        let active = word("hello");

        let result = search_words(
            &[deleted, silenced, active],
            "hello",
            SearchMode::Fuzzy,
            None,
        );

        assert_eq!(result.total_count, 1);
        assert_eq!(result.matches[0].word_index, 2);
    }

    #[test]
    fn empty_query_returns_no_matches() {
        let words = vec![word("hello")];

        let result = search_words(&words, "   ", SearchMode::Exact, None);

        assert!(result.matches.is_empty());
        assert_eq!(result.total_count, 0);
    }
}
