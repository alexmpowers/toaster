//! Extracted from the inline `mod tests` block (monolith-split).

use super::*;

/// Helper to build a Word with sensible defaults.
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

fn default_config() -> FillerConfig {
    FillerConfig::default()
}

// ── detect_fillers ──────────────────────────────────────────────

#[test]
fn fillers_basic_match() {
    let words = vec![
        word("Hello", 0, 500_000),
        word("um", 600_000, 800_000),
        word("world", 900_000, 1_200_000),
        word("uh", 1_300_000, 1_500_000),
        word("like", 1_600_000, 1_800_000),
    ];
    let result = detect_fillers(&words, &default_config());
    assert_eq!(result, vec![1, 3, 4]);
}

#[test]
fn fillers_case_insensitive() {
    let words = vec![
        word("Um", 0, 500_000),
        word("UH", 600_000, 800_000),
        word("Like", 900_000, 1_100_000),
    ];
    let result = detect_fillers(&words, &default_config());
    assert_eq!(result, vec![0, 1, 2]);
}

#[test]
fn fillers_with_punctuation() {
    let words = vec![
        word("um,", 0, 500_000),
        word("uh.", 600_000, 800_000),
        word("like!", 900_000, 1_100_000),
    ];
    let result = detect_fillers(&words, &default_config());
    assert_eq!(result, vec![0, 1, 2]);
}

#[test]
fn fillers_skips_deleted_words() {
    let words = vec![
        word("hello", 0, 500_000),
        deleted_word("um", 600_000, 800_000),
        word("world", 900_000, 1_200_000),
    ];
    let result = detect_fillers(&words, &default_config());
    assert!(result.is_empty());
}

#[test]
fn fillers_multi_word_you_know() {
    let words = vec![
        word("I", 0, 200_000),
        word("you", 300_000, 500_000),
        word("know", 600_000, 800_000),
        word("right", 900_000, 1_000_000),
    ];
    let result = detect_fillers(&words, &default_config());
    // "you know" → indices 1,2;  "right" → index 3
    assert_eq!(result, vec![1, 2, 3]);
}

#[test]
fn fillers_multi_word_i_mean() {
    let words = vec![
        word("I", 0, 200_000),
        word("mean", 300_000, 500_000),
        word("it's", 600_000, 800_000),
    ];
    let result = detect_fillers(&words, &default_config());
    assert_eq!(result, vec![0, 1]);
}

#[test]
fn fillers_multi_word_broken_by_deleted() {
    let words = vec![
        word("you", 0, 200_000),
        deleted_word("really", 300_000, 500_000),
        word("know", 600_000, 800_000),
    ];
    // "really" is deleted, so "you" and "know" are consecutive active words
    // and should match "you know".
    let result = detect_fillers(&words, &default_config());
    assert_eq!(result, vec![0, 2]);
}

#[test]
fn fillers_empty_list() {
    let result = detect_fillers(&[], &default_config());
    assert!(result.is_empty());
}

#[test]
fn fillers_no_match() {
    let words = vec![word("hello", 0, 500_000), word("world", 600_000, 1_000_000)];
    let result = detect_fillers(&words, &default_config());
    assert!(result.is_empty());
}

#[test]
fn fillers_custom_config() {
    let config = FillerConfig {
        filler_words: vec!["hmm".to_string(), "yeah".to_string()],
        ..Default::default()
    };
    let words = vec![
        word("um", 0, 500_000), // not in custom list
        word("hmm", 600_000, 800_000),
        word("yeah", 900_000, 1_100_000),
    ];
    let result = detect_fillers(&words, &config);
    assert_eq!(result, vec![1, 2]);
}

// ── detect_pauses ───────────────────────────────────────────────

#[test]
fn pauses_finds_long_gaps() {
    let words = vec![
        word("hello", 0, 500_000),
        word("world", 2_500_000, 3_000_000), // 2s gap → detected
    ];
    let result = detect_pauses(&words, &default_config());
    assert_eq!(result, vec![(0, 2_000_000)]);
}

#[test]
fn pauses_ignores_short_gaps() {
    let words = vec![
        word("hello", 0, 500_000),
        word("world", 600_000, 1_000_000), // 100ms gap → not a pause
    ];
    let result = detect_pauses(&words, &default_config());
    assert!(result.is_empty());
}

#[test]
fn pauses_skips_deleted_words() {
    let words = vec![
        word("hello", 0, 500_000),
        deleted_word("filler", 600_000, 800_000),
        word("world", 900_000, 1_200_000), // gap from hello(500k) to world(900k) = 400ms
    ];
    let result = detect_pauses(&words, &default_config());
    assert!(result.is_empty());
}

#[test]
fn pauses_gap_across_deleted_words() {
    let words = vec![
        word("hello", 0, 500_000),
        deleted_word("x", 600_000, 700_000),
        word("world", 3_000_000, 3_500_000), // gap 500k→3M = 2.5s
    ];
    let result = detect_pauses(&words, &default_config());
    assert_eq!(result, vec![(0, 2_500_000)]);
}

#[test]
fn pauses_empty_list() {
    let result = detect_pauses(&[], &default_config());
    assert!(result.is_empty());
}

#[test]
fn pauses_single_word() {
    let words = vec![word("hello", 0, 500_000)];
    let result = detect_pauses(&words, &default_config());
    assert!(result.is_empty());
}

#[test]
fn pauses_custom_threshold() {
    let config = FillerConfig {
        pause_threshold_us: 500_000, // 0.5 seconds
        ..Default::default()
    };
    let words = vec![
        word("hello", 0, 500_000),
        word("world", 1_200_000, 1_500_000), // 700ms gap → detected at 500ms threshold
    ];
    let result = detect_pauses(&words, &config);
    assert_eq!(result, vec![(0, 700_000)]);
}

// ── analyze ─────────────────────────────────────────────────────

#[test]
fn analyze_returns_fillers_and_pauses() {
    let words = vec![
        word("so", 0, 200_000),       // "so" IS a default filler
        word("um", 300_000, 500_000), // filler
        word("hello", 600_000, 1_000_000),
        word("world", 3_000_000, 3_500_000), // 2s gap after "hello"
    ];
    let result = analyze(&words, &default_config());
    assert_eq!(result.filler_indices, vec![0, 1]); // both "so" and "um"
    assert_eq!(result.pauses, vec![(2, 2_000_000)]);
}

#[test]
fn analyze_empty_words() {
    let result = analyze(&[], &default_config());
    assert_eq!(
        result,
        AnalysisResult {
            filler_indices: vec![],
            pauses: vec![],
            duplicate_indices: vec![],
        }
    );
}

// ── normalize_filler ────────────────────────────────────────────

#[test]
fn normalize_filler_collapses_trailing_repeat() {
    assert_eq!(normalize_filler("umm"), "um");
    assert_eq!(normalize_filler("uhhh"), "uh");
    assert_eq!(normalize_filler("hmmm"), "hm");
    assert_eq!(normalize_filler("ummmmm"), "um");
}

#[test]
fn normalize_filler_already_normalized() {
    assert_eq!(normalize_filler("um"), "um");
}

#[test]
fn normalize_filler_no_trailing_repeat() {
    assert_eq!(normalize_filler("like"), "like");
}

#[test]
fn fuzzy_filler_matches_umm() {
    let words = vec![
        word("hello", 0, 500_000),
        word("umm", 600_000, 800_000),
        word("world", 900_000, 1_200_000),
    ];
    let result = detect_fillers(&words, &default_config());
    assert_eq!(result, vec![1]);
}

// ── detect_duplicates ───────────────────────────────────────────

#[test]
fn duplicates_finds_adjacent_pair() {
    let words = vec![
        word("the", 0, 200_000),
        word("the", 300_000, 500_000),
        word("best", 600_000, 800_000),
    ];
    assert_eq!(detect_duplicates(&words), vec![1]);
}

#[test]
fn duplicates_no_match_non_adjacent() {
    let words = vec![
        word("the", 0, 200_000),
        word("a", 300_000, 500_000),
        word("the", 600_000, 800_000),
    ];
    assert_eq!(detect_duplicates(&words), Vec::<usize>::new());
}

#[test]
fn duplicates_triple() {
    let words = vec![
        word("the", 0, 200_000),
        word("the", 300_000, 500_000),
        word("the", 600_000, 800_000),
    ];
    assert_eq!(detect_duplicates(&words), vec![1, 2]);
}

#[test]
fn duplicates_skips_deleted() {
    let words = vec![
        word("the", 0, 200_000),
        deleted_word("the", 300_000, 500_000),
        word("best", 600_000, 800_000),
    ];
    assert_eq!(detect_duplicates(&words), Vec::<usize>::new());
}

#[test]
fn duplicates_across_deleted_gap() {
    let words = vec![
        word("the", 0, 200_000),
        deleted_word("um", 300_000, 400_000),
        word("the", 500_000, 700_000),
    ];
    // "the" and "the" are adjacent non-deleted words
    assert_eq!(detect_duplicates(&words), vec![2]);
}

#[test]
fn duplicates_multiple_pairs() {
    let words = vec![
        word("the", 0, 200_000),
        word("the", 300_000, 500_000),
        word("best", 600_000, 800_000),
        word("best", 900_000, 1_100_000),
        word("part", 1_200_000, 1_400_000),
    ];
    assert_eq!(detect_duplicates(&words), vec![1, 3]);
}

// ── trim_pauses ─────────────────────────────────────────────────
//
// Contract (post-correctness-fix): `trim_pauses` inserts deleted
// "silence sentinel" Words covering the excess of every gap above
// `pause_threshold_us`. Real word source-time is **never** mutated —
// `EditorState::get_keep_segments` already excludes deleted ranges,
// so the seam closes correctly without breaking the source-time
// invariant the export and waveform pipelines depend on.

/// Predicate for a synthetic silence sentinel: deleted with empty text.
/// Local to the test module — production code should call
/// `filler::is_silence_sentinel`.
fn is_sentinel(w: &Word) -> bool {
    w.deleted && w.text.is_empty()
}

#[test]
fn trim_inserts_sentinel_for_2s_gap_with_300ms_target() {
    let mut words = vec![
        word("hello", 0, 500_000),
        word("world", 2_500_000, 3_000_000), // 2s gap
    ];
    let count = trim_pauses(&mut words, DEFAULT_PAUSE_THRESHOLD_US, DEFAULT_MAX_GAP_US);
    assert_eq!(count, 1);
    // Real word timestamps must be untouched.
    assert_eq!(words[0].start_us, 0);
    assert_eq!(words[0].end_us, 500_000);
    // The newly inserted index-1 entry is the sentinel.
    assert!(is_sentinel(&words[1]));
    // Sentinel covers `[prev.end + max_gap, next.start]`.
    assert_eq!(words[1].start_us, 500_000 + DEFAULT_MAX_GAP_US);
    assert_eq!(words[1].end_us, 2_500_000);
    // World is still at its original source-time.
    assert_eq!(words[2].start_us, 2_500_000);
    assert_eq!(words[2].end_us, 3_000_000);
}

#[test]
fn trim_preserves_source_time_for_subsequent_words() {
    let mut words = vec![
        word("hello", 0, 500_000),
        word("world", 2_500_000, 3_000_000), // 2s gap
        word("foo", 3_100_000, 3_500_000),
    ];
    let original_world_start = words[1].start_us;
    let original_foo_start = words[2].start_us;

    let count = trim_pauses(&mut words, DEFAULT_PAUSE_THRESHOLD_US, DEFAULT_MAX_GAP_US);
    assert_eq!(count, 1);

    // Find the surviving real words by text — sentinel insertion may
    // have shifted their array positions but never their timestamps.
    let world = words.iter().find(|w| w.text == "world").unwrap();
    let foo = words.iter().find(|w| w.text == "foo").unwrap();
    assert_eq!(world.start_us, original_world_start);
    assert_eq!(foo.start_us, original_foo_start);
}

#[test]
fn trim_finds_silence_after_a_filler_was_deleted_in_the_gap() {
    // Regression for the false negative reported by the user: workflow
    // is "Remove fillers" first, "Remove silence" second. Every long
    // silence in real-world audio has a filler ("um", "uh") sitting in
    // the middle of it, and after fillers are deleted the surrounding
    // dead air is still trimmable silence. The bug: an earlier
    // implementation skipped any gap that contained any deleted word
    // (including user-deleted fillers), so Remove silence reported
    // "0 pauses" after Remove fillers had run. Fixed by treating only
    // **silence sentinels** (deleted-empty Words) as bridging.
    let mut words = vec![
        word("hello", 0, 500_000),
        deleted_word("um", 600_000, 700_000),
        word("world", 2_500_000, 3_000_000),
    ];
    let count = trim_pauses(&mut words, DEFAULT_PAUSE_THRESHOLD_US, DEFAULT_MAX_GAP_US);
    assert_eq!(
        count, 1,
        "deleted filler in the gap must NOT block silence trimming"
    );
    // Real word source-time still inviolate, including the user-deleted
    // filler — its boundaries describe what audio to excise.
    assert_eq!(words.iter().find(|w| w.text == "hello").unwrap().end_us, 500_000);
    assert_eq!(words.iter().find(|w| w.text == "um").unwrap().start_us, 600_000);
    assert_eq!(words.iter().find(|w| w.text == "um").unwrap().end_us, 700_000);
    assert_eq!(words.iter().find(|w| w.text == "world").unwrap().start_us, 2_500_000);
    // Exactly one sentinel, covering [hello.end + max_gap, world.start].
    let sentinels: Vec<_> = words.iter().filter(|w| is_sentinel(w)).collect();
    assert_eq!(sentinels.len(), 1);
    assert_eq!(sentinels[0].start_us, 500_000 + DEFAULT_MAX_GAP_US);
    assert_eq!(sentinels[0].end_us, 2_500_000);
}

#[test]
fn trim_skips_gap_already_bridged_by_a_silence_sentinel() {
    // True idempotence guard: the sentinel from a prior trim must
    // bridge the gap so a second invocation is a no-op. The bridging
    // marker is the empty-text + deleted predicate that
    // `is_silence_sentinel` checks — a user-deleted real word does
    // not count.
    let mut words = vec![
        word("hello", 0, 500_000),
        // Pre-existing sentinel covering the bulk of the gap.
        Word {
            text: String::new(),
            start_us: 500_000 + DEFAULT_MAX_GAP_US,
            end_us: 2_500_000,
            deleted: true,
            silenced: false,
            confidence: -1.0,
            speaker_id: -1,
        },
        word("world", 2_500_000, 3_000_000),
    ];
    let snapshot = words.clone();
    let count = trim_pauses(&mut words, DEFAULT_PAUSE_THRESHOLD_US, DEFAULT_MAX_GAP_US);
    assert_eq!(count, 0);
    assert_eq!(words.len(), snapshot.len());
    for (a, b) in words.iter().zip(snapshot.iter()) {
        assert_eq!(a.text, b.text);
        assert_eq!(a.start_us, b.start_us);
        assert_eq!(a.end_us, b.end_us);
        assert_eq!(a.deleted, b.deleted);
    }
}

#[test]
fn trim_ignores_gap_below_threshold() {
    let mut words = vec![
        word("hello", 0, 500_000),
        word("world", 900_000, 1_200_000), // 400ms gap, below 1.5s threshold
    ];
    let original_len = words.len();
    let count = trim_pauses(&mut words, DEFAULT_PAUSE_THRESHOLD_US, DEFAULT_MAX_GAP_US);
    assert_eq!(count, 0);
    assert_eq!(words.len(), original_len);
}

#[test]
fn trim_handles_empty_and_single() {
    let mut empty: Vec<Word> = vec![];
    assert_eq!(
        trim_pauses(&mut empty, DEFAULT_PAUSE_THRESHOLD_US, DEFAULT_MAX_GAP_US),
        0
    );

    let mut single = vec![word("hello", 0, 500_000)];
    assert_eq!(
        trim_pauses(&mut single, DEFAULT_PAUSE_THRESHOLD_US, DEFAULT_MAX_GAP_US),
        0
    );
}

#[test]
fn trim_inserts_a_sentinel_for_every_long_gap() {
    let mut words = vec![
        word("a", 0, 500_000),
        word("b", 2_500_000, 3_000_000), // 2s gap
        word("c", 5_000_000, 5_500_000), // 2s gap after b
    ];
    let count = trim_pauses(&mut words, DEFAULT_PAUSE_THRESHOLD_US, DEFAULT_MAX_GAP_US);
    assert_eq!(count, 2);
    let sentinels: Vec<&Word> = words.iter().filter(|w| is_sentinel(w)).collect();
    assert_eq!(sentinels.len(), 2);
    // Real words preserved their source-time.
    assert_eq!(words.iter().find(|w| w.text == "b").unwrap().start_us, 2_500_000);
    assert_eq!(words.iter().find(|w| w.text == "c").unwrap().start_us, 5_000_000);
}

#[test]
fn trim_is_idempotent_on_repeated_invocation() {
    let mut words = vec![
        word("hello", 0, 500_000),
        word("world", 2_500_000, 3_000_000),
        word("there", 5_500_000, 6_000_000),
    ];
    let first = trim_pauses(&mut words, DEFAULT_PAUSE_THRESHOLD_US, DEFAULT_MAX_GAP_US);
    assert!(first > 0);
    let second = trim_pauses(&mut words, DEFAULT_PAUSE_THRESHOLD_US, DEFAULT_MAX_GAP_US);
    assert_eq!(second, 0, "second trim must be a no-op on already-trimmed transcript");
    let dry =
        count_trimmable_pauses(&words, DEFAULT_PAUSE_THRESHOLD_US, DEFAULT_MAX_GAP_US);
    assert_eq!(dry, 0);
}

// ── tighten_gaps ────────────────────────────────────────────────

#[test]
fn tighten_inserts_sentinel_for_500ms_gap_with_250ms_target() {
    let mut words = vec![
        word("hello", 0, 500_000),
        word("world", 1_000_000, 1_500_000), // 500ms gap
    ];
    let count = tighten_gaps(&mut words, DEFAULT_TIGHTEN_TARGET_US);
    assert_eq!(count, 1);
    // Real word source-time unchanged.
    assert_eq!(words[0].end_us, 500_000);
    let world = words.iter().find(|w| w.text == "world").unwrap();
    assert_eq!(world.start_us, 1_000_000);
    assert_eq!(world.end_us, 1_500_000);
    // Sentinel covers `[prev.end + target, next.start]` =
    //                  [500_000 + 250_000, 1_000_000].
    let sentinel = words.iter().find(|w| is_sentinel(w)).unwrap();
    assert_eq!(sentinel.start_us, 750_000);
    assert_eq!(sentinel.end_us, 1_000_000);
}

#[test]
fn tighten_ignores_gap_below_target() {
    let mut words = vec![
        word("hello", 0, 500_000),
        word("world", 700_000, 1_000_000), // 200ms gap, below 250ms target
    ];
    let original_len = words.len();
    let count = tighten_gaps(&mut words, DEFAULT_TIGHTEN_TARGET_US);
    assert_eq!(count, 0);
    assert_eq!(words.len(), original_len);
}

#[test]
fn tighten_inserts_independent_sentinels_for_each_gap() {
    let mut words = vec![
        word("a", 0, 500_000),
        word("b", 1_000_000, 1_500_000), // 500ms gap → excess 250ms
        word("c", 2_500_000, 3_000_000), // 1000ms gap → excess 750ms
    ];
    let count = tighten_gaps(&mut words, DEFAULT_TIGHTEN_TARGET_US);
    assert_eq!(count, 2);
    // Real words at their original source-time.
    assert_eq!(words.iter().find(|w| w.text == "b").unwrap().start_us, 1_000_000);
    assert_eq!(words.iter().find(|w| w.text == "c").unwrap().start_us, 2_500_000);
}

#[test]
fn tighten_finds_gap_after_a_filler_was_deleted_in_the_gap() {
    // Same regression as `trim_finds_silence_after_a_filler_was_deleted`
    // but for the explicit-target tightener path. Deleted user words
    // inside the gap (e.g. excised fillers) must NOT block tightening.
    let mut words = vec![
        word("hello", 0, 500_000),
        deleted_word("um", 550_000, 650_000),
        word("world", 1_000_000, 1_500_000),
    ];
    let count = tighten_gaps(&mut words, DEFAULT_TIGHTEN_TARGET_US);
    assert_eq!(count, 1);
    assert_eq!(words.iter().filter(|w| is_sentinel(w)).count(), 1);
}

#[test]
fn tighten_skips_gap_already_bridged_by_a_silence_sentinel() {
    // Idempotence guard: a pre-existing silence sentinel blocks the
    // tightener from re-inserting another one over the same gap.
    let mut words = vec![
        word("hello", 0, 500_000),
        Word {
            text: String::new(),
            start_us: 500_000 + DEFAULT_TIGHTEN_TARGET_US,
            end_us: 1_000_000,
            deleted: true,
            silenced: false,
            confidence: -1.0,
            speaker_id: -1,
        },
        word("world", 1_000_000, 1_500_000),
    ];
    let snapshot_len = words.len();
    let count = tighten_gaps(&mut words, DEFAULT_TIGHTEN_TARGET_US);
    assert_eq!(count, 0);
    assert_eq!(words.len(), snapshot_len);
}

#[test]
fn tighten_handles_empty_and_single() {
    let mut empty: Vec<Word> = vec![];
    assert_eq!(tighten_gaps(&mut empty, DEFAULT_TIGHTEN_TARGET_US), 0);

    let mut single = vec![word("hello", 0, 500_000)];
    assert_eq!(tighten_gaps(&mut single, DEFAULT_TIGHTEN_TARGET_US), 0);
}

#[test]
fn tighten_rejects_non_positive_target() {
    let mut words = vec![
        word("hello", 0, 500_000),
        word("world", 1_000_000, 1_500_000),
    ];
    assert_eq!(tighten_gaps(&mut words, 0), 0);
    assert_eq!(tighten_gaps(&mut words, -100), 0);
}

