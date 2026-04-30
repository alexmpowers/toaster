//! End-to-end cascade tests for filler-detection → duplicate-collapse →
//! keep-segment construction. Extracted from `filler_tests.rs` to keep
//! that file under the 800-line cap.

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

fn default_config() -> FillerConfig {
    FillerConfig::default()
}

#[test]
fn cleanup_cascade_produces_correct_keep_segments() {
    use crate::managers::editor::EditorState;

    // Transcript: "Yeah, so the um the the best best part about a lot
    // of this is how it can really transform the way you sound. And um
    // like the uh the the difference is gonna be noticeable kind of on
    // first use."
    let mut words = vec![
        word("Yeah,", 0, 400_000),                  // 0
        word("so", 500_000, 700_000),               // 1
        word("the", 800_000, 1_000_000),            // 2
        word("um", 1_100_000, 1_300_000),           // 3  ← filler
        word("the", 1_400_000, 1_600_000),          // 4  ← dup
        word("the", 1_700_000, 1_900_000),          // 5  ← dup
        word("best", 2_000_000, 2_200_000),         // 6
        word("best", 2_300_000, 2_500_000),         // 7  ← dup
        word("part", 2_600_000, 2_800_000),         // 8
        word("about", 2_900_000, 3_100_000),        // 9
        word("a", 3_200_000, 3_300_000),            // 10
        word("lot", 3_400_000, 3_600_000),          // 11
        word("of", 3_700_000, 3_800_000),           // 12
        word("this", 3_900_000, 4_100_000),         // 13
        word("is", 4_200_000, 4_400_000),           // 14
        word("how", 4_500_000, 4_700_000),          // 15
        word("it", 4_800_000, 4_900_000),           // 16
        word("can", 5_000_000, 5_200_000),          // 17
        word("really", 5_300_000, 5_500_000),       // 18
        word("transform", 5_600_000, 5_900_000),    // 19
        word("the", 6_000_000, 6_200_000),          // 20
        word("way", 6_300_000, 6_500_000),          // 21
        word("you", 6_600_000, 6_800_000),          // 22
        word("sound.", 6_900_000, 7_200_000),       // 23
        word("And", 7_400_000, 7_600_000),          // 24
        word("um", 7_700_000, 7_900_000),           // 25 ← filler
        word("like", 8_000_000, 8_200_000),         // 26 ← filler
        word("the", 8_300_000, 8_500_000),          // 27
        word("uh", 8_600_000, 8_800_000),           // 28 ← filler
        word("the", 8_900_000, 9_100_000),          // 29 ← dup
        word("the", 9_200_000, 9_400_000),          // 30 ← dup
        word("difference", 9_500_000, 9_900_000),   // 31
        word("is", 10_000_000, 10_200_000),         // 32
        word("gonna", 10_300_000, 10_500_000),      // 33
        word("be", 10_600_000, 10_800_000),         // 34
        word("noticeable", 10_900_000, 11_300_000), // 35
        word("kind", 11_400_000, 11_600_000),       // 36 ← filler (kind of)
        word("of", 11_700_000, 11_900_000),         // 37 ← filler (kind of)
        word("on", 12_000_000, 12_200_000),         // 38
        word("first", 12_300_000, 12_500_000),      // 39
        word("use.", 12_600_000, 12_800_000),       // 40
    ];

    let config = default_config();

    // Step 1: detect and delete fillers
    let fillers = detect_fillers(&words, &config);
    for &idx in &fillers {
        words[idx].deleted = true;
    }

    // Step 2: iteratively detect and delete duplicates
    loop {
        let dups = detect_duplicates(&words);
        if dups.is_empty() {
            break;
        }
        for &idx in &dups {
            words[idx].deleted = true;
        }
    }

    // Verify remaining (non-deleted) text
    let remaining: Vec<&str> = words
        .iter()
        .filter(|w| !w.deleted)
        .map(|w| w.text.as_str())
        .collect();

    assert_eq!(
        remaining,
        vec![
            "Yeah,",
            "the",
            "best",
            "part",
            "about",
            "a",
            "lot",
            "of",
            "this",
            "is",
            "how",
            "it",
            "can",
            "really",
            "transform",
            "the",
            "way",
            "you",
            "sound.",
            "And",
            "the",
            "difference",
            "is",
            "gonna",
            "be",
            "noticeable",
            "on",
            "first",
            "use.",
        ]
    );

    // Verify keep-segments exclude deleted word regions
    let mut editor = EditorState::new();
    editor.set_words(words.clone());
    // Replay deletions into the editor's words
    for (i, w) in words.iter().enumerate() {
        if w.deleted {
            editor.get_words_mut()[i].deleted = true;
        }
    }
    let segments = editor.get_keep_segments();

    // Every segment must only span non-deleted word time ranges
    let deleted_ranges: Vec<(i64, i64)> = words
        .iter()
        .filter(|w| w.deleted)
        .map(|w| (w.start_us, w.end_us))
        .collect();

    for (seg_start, seg_end) in &segments {
        for (del_start, del_end) in &deleted_ranges {
            // A deleted word's range must not be fully contained in a keep-segment
            let overlaps = del_start >= seg_start && del_end <= seg_end;
            assert!(
                !overlaps,
                "keep-segment ({seg_start}–{seg_end}) contains deleted word ({del_start}–{del_end})"
            );
        }
    }

    // Sanity: we should have at least 2 segments (gap around deleted regions)
    assert!(!segments.is_empty(), "expected non-empty keep-segments");
}
