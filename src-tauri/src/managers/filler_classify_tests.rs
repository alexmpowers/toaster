//! Tests for `classify_gap` / `classify_pauses` (R-004 gap
//! classification by VAD curve). Extracted from `filler_tests.rs` to
//! keep that file under the 800-line cap.

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

#[test]
fn classify_gap_unknown_without_curve() {
    assert_eq!(classify_gap(0, 1_000_000, &[]), GapClassification::Unknown);
}

#[test]
fn classify_gap_true_silence_below_threshold() {
    // 10 frames × 30ms = 300ms curve, all well below GAP_SILENCE_THRESHOLD.
    let curve = vec![0.05f32; 10];
    assert_eq!(
        classify_gap(0, 300_000, &curve),
        GapClassification::TrueSilence,
    );
}

#[test]
fn classify_gap_missed_speech_above_threshold() {
    let curve = vec![0.9f32; 10];
    assert_eq!(
        classify_gap(0, 300_000, &curve),
        GapClassification::MissedSpeech,
    );
}

#[test]
fn classify_gap_non_speech_acoustic_in_middle_band() {
    let curve = vec![0.3f32; 10];
    assert_eq!(
        classify_gap(0, 300_000, &curve),
        GapClassification::NonSpeechAcoustic,
    );
}

#[test]
fn classify_pauses_maps_one_to_one_with_empty_curve() {
    let words = vec![word("a", 0, 200_000), word("b", 2_000_000, 2_200_000)];
    let config = FillerConfig::default();
    let pauses = detect_pauses(&words, &config);
    let classified = classify_pauses(&pauses, &words, &[]);
    assert_eq!(classified.len(), pauses.len());
    for (_, _, class) in &classified {
        assert_eq!(*class, GapClassification::Unknown);
    }
}
