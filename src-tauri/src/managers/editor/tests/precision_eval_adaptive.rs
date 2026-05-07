//! Adaptive gap threshold precision eval tests.
//!
//! Extracted from `precision_eval.rs` to keep that file under the 800-line cap.
//! These tests validate the `adaptive_gap_threshold()` algorithm introduced to
//! fix timeline fragmentation on presentation-style content (e.g.,
//! `pagineted-reports.mp4`).

use super::super::*;

/// Presentation-style content: multiple phrases separated by 400–800 ms
/// natural pauses (slide transitions, topic shifts). Validates that the
/// adaptive threshold absorbs these gaps rather than fragmenting the
/// timeline — the root cause of the timeline-vs-transcript misalignment
/// reported for `pagineted-reports.mp4`: the old hardcoded 200 ms threshold
/// split every inter-slide pause into a separate segment, causing timeline
/// drift and misalignment.
#[test]
fn precision_eval_presentation_style_adaptive_threshold() {
    // Simulates a presentation with 6 phrases separated by 400-800ms
    // natural pauses (typical slide/topic transitions). With the old
    // 200ms threshold, this would produce 6 segments. With adaptive
    // threshold, all should merge into 1 (gaps are within the natural
    // distribution).
    let words: Vec<Word> = vec![
        // Phrase 1
        Word { text: "Welcome".into(), start_us: 0, end_us: 400_000,
               deleted: false, silenced: false, confidence: 0.95, speaker_id: 0 },
        Word { text: "everyone".into(), start_us: 400_000, end_us: 900_000,
               deleted: false, silenced: false, confidence: 0.93, speaker_id: 0 },
        // 500ms natural pause (slide transition)
        // Phrase 2
        Word { text: "Today".into(), start_us: 1_400_000, end_us: 1_800_000,
               deleted: false, silenced: false, confidence: 0.94, speaker_id: 0 },
        Word { text: "we".into(), start_us: 1_800_000, end_us: 1_950_000,
               deleted: false, silenced: false, confidence: 0.92, speaker_id: 0 },
        Word { text: "discuss".into(), start_us: 1_950_000, end_us: 2_500_000,
               deleted: false, silenced: false, confidence: 0.91, speaker_id: 0 },
        // 600ms natural pause
        // Phrase 3
        Word { text: "the".into(), start_us: 3_100_000, end_us: 3_250_000,
               deleted: false, silenced: false, confidence: 0.90, speaker_id: 0 },
        Word { text: "quarterly".into(), start_us: 3_250_000, end_us: 3_800_000,
               deleted: false, silenced: false, confidence: 0.88, speaker_id: 0 },
        Word { text: "results".into(), start_us: 3_800_000, end_us: 4_400_000,
               deleted: false, silenced: false, confidence: 0.89, speaker_id: 0 },
        // 400ms natural pause
        // Phrase 4
        Word { text: "Revenue".into(), start_us: 4_800_000, end_us: 5_300_000,
               deleted: false, silenced: false, confidence: 0.95, speaker_id: 0 },
        Word { text: "grew".into(), start_us: 5_300_000, end_us: 5_600_000,
               deleted: false, silenced: false, confidence: 0.93, speaker_id: 0 },
        // 700ms natural pause
        // Phrase 5
        Word { text: "by".into(), start_us: 6_300_000, end_us: 6_450_000,
               deleted: false, silenced: false, confidence: 0.91, speaker_id: 0 },
        Word { text: "fifteen".into(), start_us: 6_450_000, end_us: 6_900_000,
               deleted: false, silenced: false, confidence: 0.90, speaker_id: 0 },
        Word { text: "percent".into(), start_us: 6_900_000, end_us: 7_400_000,
               deleted: false, silenced: false, confidence: 0.92, speaker_id: 0 },
        // 500ms natural pause
        // Phrase 6
        Word { text: "Any".into(), start_us: 7_900_000, end_us: 8_100_000,
               deleted: false, silenced: false, confidence: 0.94, speaker_id: 0 },
        Word { text: "questions".into(), start_us: 8_100_000, end_us: 8_700_000,
               deleted: false, silenced: false, confidence: 0.93, speaker_id: 0 },
    ];

    let mut editor = EditorState::new();
    editor.set_words(words.clone());

    let segments = editor.get_keep_segments();

    // With adaptive threshold, natural 400-700ms presentation gaps
    // should NOT fragment the timeline. The old hardcoded 200ms
    // threshold would produce 6 segments here.
    assert!(
        segments.len() <= 2,
        "presentation-style content with 400-700ms natural gaps should not \
         fragment into {} segments; adaptive threshold should absorb these \
         gaps (segments: {:?})",
        segments.len(),
        segments
    );

    // Total kept duration should span the full content (no gaps excluded)
    let total_kept: i64 = segments.iter().map(|(s, e)| e - s).sum();
    let total_content = words.last().unwrap().end_us - words.first().unwrap().start_us;
    assert_eq!(
        total_kept, total_content,
        "all natural pauses should be kept (no dead-air exclusion for \
         presentation-style gaps)"
    );

    // Now delete a word from the middle — should create exactly 2
    // segments split at the delete boundary (not more).
    assert!(editor.delete_word(7), "delete 'results'");
    let post_delete = editor.get_keep_segments();
    assert_eq!(
        post_delete.len(),
        2,
        "deleting one word should create exactly 2 segments, not more; \
         got {:?}",
        post_delete
    );

    // Undo should restore original state.
    assert!(editor.undo(), "undo should succeed");
    let restored = editor.get_keep_segments();
    assert_eq!(
        restored, segments,
        "undo should restore original segmentation"
    );
}
