//! Tests for `EditorState::get_keep_segments` overlap handling.
//!
//! These tests exercise the audio-truth Remove Silence path: silence
//! sentinels (deleted "words" with `text=""`) may overlap non-deleted word
//! ranges in source-time because Parakeet pads word boundaries past the
//! actual speech. The interval-subtraction algorithm must split, clip, or
//! drop overlapping word ranges accordingly without putting deleted audio
//! back on the timeline.
//!
//! Sister tests in `basic.rs` and `seams.rs` cover the no-overlap fast
//! path. Anything here that diverges from those is by design.

use super::super::*;

fn make_word(text: &str, start_us: i64, end_us: i64, deleted: bool) -> Word {
    Word {
        text: text.into(),
        start_us,
        end_us,
        deleted,
        silenced: false,
        confidence: 1.0,
        speaker_id: 0,
    }
}

fn silence_sentinel(start_us: i64, end_us: i64) -> Word {
    Word {
        text: "".into(),
        start_us,
        end_us,
        deleted: true,
        silenced: false,
        confidence: -1.0,
        speaker_id: -1,
    }
}

#[test]
fn sentinel_inside_a_word_splits_it_into_head_and_tail() {
    // Word [3_500_000, 5_000_000); sentinel [3_700_000, 4_500_000) inside.
    // After interval subtraction we expect TWO sub-intervals contributing
    // to keep-segments: [3_500_000, 3_700_000) and [4_500_000, 5_000_000).
    // Each is well above MIN_KEEP_SEGMENT_US (150_000) so neither gets
    // micro-merged away.
    let mut editor = EditorState::new();
    editor.set_words(vec![
        make_word("use.", 3_500_000, 5_000_000, false),
        silence_sentinel(3_700_000, 4_500_000),
    ]);

    let segments = editor.get_keep_segments();
    assert_eq!(
        segments,
        vec![(3_500_000, 3_700_000), (4_500_000, 5_000_000)],
        "sentinel inside word should split it into head + tail"
    );
}

#[test]
fn sentinel_spanning_word_end_clips_only_the_tail() {
    // Word [3_500_000, 5_000_000); sentinel [4_500_000, 6_000_000) extends
    // past the word's end. Expect a single kept range [3_500_000, 4_500_000).
    let mut editor = EditorState::new();
    editor.set_words(vec![
        make_word("use.", 3_500_000, 5_000_000, false),
        silence_sentinel(4_500_000, 6_000_000),
    ]);

    let segments = editor.get_keep_segments();
    assert_eq!(segments, vec![(3_500_000, 4_500_000)]);
}

#[test]
fn sentinel_spanning_word_start_clips_only_the_head() {
    // Word [3_500_000, 5_000_000); sentinel [3_000_000, 4_000_000) starts
    // before the word and clips its head. Expect [4_000_000, 5_000_000).
    let mut editor = EditorState::new();
    editor.set_words(vec![
        silence_sentinel(3_000_000, 4_000_000),
        make_word("use.", 3_500_000, 5_000_000, false),
    ]);

    let segments = editor.get_keep_segments();
    assert_eq!(segments, vec![(4_000_000, 5_000_000)]);
}

#[test]
fn sentinel_covering_an_entire_word_drops_it() {
    // Word [3_500_000, 5_000_000); sentinel [3_000_000, 6_000_000) covers
    // it completely. With surrounding kept words, the kept segments span
    // only the neighbours.
    let mut editor = EditorState::new();
    editor.set_words(vec![
        make_word("hello", 1_000_000, 2_000_000, false),
        silence_sentinel(3_000_000, 6_000_000),
        make_word("there", 7_000_000, 8_000_000, false),
        // Word entirely covered by the sentinel; not in source order on
        // purpose to keep the input stable.
        make_word("um", 4_000_000, 4_500_000, false),
    ]);

    let segments = editor.get_keep_segments();
    assert_eq!(
        segments,
        vec![(1_000_000, 2_000_000), (7_000_000, 8_000_000)]
    );
}

#[test]
fn two_overlapping_sentinels_act_as_one_forbidden_range() {
    // Sentinels [800_000, 3_000_000) and [2_000_000, 5_000_000) overlap.
    // After merging, forbidden = [800_000, 5_000_000). Word [600_000,
    // 6_000_000) gets clipped at both ends.
    let mut editor = EditorState::new();
    editor.set_words(vec![
        make_word("phrase", 600_000, 6_000_000, false),
        silence_sentinel(800_000, 3_000_000),
        silence_sentinel(2_000_000, 5_000_000),
    ]);

    let segments = editor.get_keep_segments();
    assert_eq!(
        segments,
        vec![(600_000, 800_000), (5_000_000, 6_000_000)]
    );
}

#[test]
fn delete_seam_inside_word_refuses_micro_merge() {
    // Word [0, 5_000_000); sentinel [200_000, 4_900_000) leaves a 200ms
    // head and a 100ms tail. The 100ms tail is below MIN_KEEP_SEGMENT_US
    // so the micro-merge pass would normally try to merge it backwards,
    // but the seam is delete-driven and must not be bridged.
    let mut editor = EditorState::new();
    editor.set_words(vec![
        make_word("phrase", 0, 5_000_000, false),
        silence_sentinel(200_000, 4_900_000),
    ]);

    let segments = editor.get_keep_segments();
    // Expect the head as a kept segment; the tail is too short and there
    // is no neighbour it can legally merge with (forward seam would also
    // be delete-driven, no neighbour exists).
    assert_eq!(segments.first().copied(), Some((0, 200_000)));
    // The tail is allowed to either drop (acceptable — too short to keep)
    // or be present as its own segment. Both are correctness-preserving;
    // we only forbid bridging the deleted region.
    for seg in &segments {
        assert!(
            seg.0 >= 4_900_000 || seg.1 <= 200_000,
            "no segment should span the deleted region: {:?}",
            seg
        );
    }
}

#[test]
fn audio_truth_pattern_five_long_silences_produces_five_seam_breaks() {
    // Mimic the user's screenshot: five long silence sentinels distributed
    // across a 60-second timeline, each well above the 400ms detection
    // threshold. The kept segments should split at every sentinel.
    let mut editor = EditorState::new();
    let mut words = Vec::new();
    // Six speech regions (between the five silences), each 5 seconds long.
    let speech_starts = [0_i64, 7_000_000, 14_000_000, 21_000_000, 28_000_000, 35_000_000];
    for (i, &start) in speech_starts.iter().enumerate() {
        words.push(make_word(
            &format!("speech_{}", i),
            start,
            start + 5_000_000,
            false,
        ));
    }
    // Five silence sentinels, one between each speech region (each 2 s).
    for &start in &[5_000_000_i64, 12_000_000, 19_000_000, 26_000_000, 33_000_000] {
        words.push(silence_sentinel(start, start + 2_000_000));
    }

    editor.set_words(words);

    let segments = editor.get_keep_segments();
    assert_eq!(
        segments.len(),
        6,
        "expected 6 kept segments (one per speech region), got {:?}",
        segments
    );
    for (i, &start) in speech_starts.iter().enumerate() {
        assert_eq!(
            segments[i],
            (start, start + 5_000_000),
            "segment {} should match speech region {:?}",
            i,
            (start, start + 5_000_000)
        );
    }
}

#[test]
fn no_overlap_path_remains_numerically_identical() {
    // A typical no-overlap transcript: deleted filler between two kept
    // words, no sentinels overlapping anything. The new algorithm must
    // produce the same segments today's output did, so the existing
    // 451 lib tests stay green.
    let mut editor = EditorState::new();
    editor.set_words(vec![
        make_word("Hello", 0, 500_000, false),
        // Filler: deleted, gap-only, no overlap with neighbours.
        make_word("um", 600_000, 800_000, true),
        make_word("world", 900_000, 1_500_000, false),
    ]);

    let segments = editor.get_keep_segments();
    // Two segments split at the deleted filler. The micro-merge pass
    // refuses to bridge the delete-driven seam.
    assert_eq!(segments.len(), 2);
    assert_eq!(segments[0], (0, 500_000));
    assert_eq!(segments[1], (900_000, 1_500_000));
}
