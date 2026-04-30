//! Precision eval tests (extracted from editor/mod.rs).

use super::super::*;

/// Heterogeneous, non-uniform durations — critical for anti-synthesis
/// guard. If any code path ever resets these to equal spans, this
/// fixture will detect it.
fn heterogeneous_words() -> Vec<Word> {
    vec![
        Word {
            text: "The".into(),
            start_us: 100_000,
            end_us: 250_000,
            deleted: false,
            silenced: false,
            confidence: 0.98,
            speaker_id: 0,
        },
        Word {
            text: "quick".into(),
            start_us: 280_000,
            end_us: 690_000,
            deleted: false,
            silenced: false,
            confidence: 0.97,
            speaker_id: 0,
        },
        Word {
            text: "brown".into(),
            start_us: 720_000,
            end_us: 1_180_000,
            deleted: false,
            silenced: false,
            confidence: 0.96,
            speaker_id: 0,
        },
        Word {
            text: "fox".into(),
            start_us: 1_220_000,
            end_us: 1_500_000,
            deleted: false,
            silenced: false,
            confidence: 0.99,
            speaker_id: 0,
        },
        Word {
            text: "jumps".into(),
            start_us: 1_600_000,
            end_us: 2_050_000,
            deleted: false,
            silenced: false,
            confidence: 0.95,
            speaker_id: 0,
        },
    ]
}

/// Anti-synthesis guard: per-word durations must never collapse to a
/// single equal value after any round-trip through editor state.
#[test]
fn precision_eval_no_equal_duration_synthesis() {
    let words = heterogeneous_words();
    let mut editor = EditorState::new();
    editor.set_words(words.clone());

    let got = editor.get_words();
    let original_durations: Vec<i64> = words.iter().map(|w| w.end_us - w.start_us).collect();
    let got_durations: Vec<i64> = got.iter().map(|w| w.end_us - w.start_us).collect();

    assert_eq!(
        original_durations, got_durations,
        "set_words must not mutate per-word durations",
    );

    let first = got_durations[0];
    assert!(
        got_durations.iter().any(|d| *d != first),
        "precision violation: all word durations are equal ({first}) \
         — a synthesis path has been introduced",
    );
}

/// Keep-segment round-trip: delete then undo must restore exact
/// per-word timing and produce an identical keep-segment set.
#[test]
fn precision_eval_delete_undo_roundtrip_preserves_timing() {
    let words = heterogeneous_words();
    let mut editor = EditorState::new();
    editor.set_words(words.clone());

    let original_words = editor.get_words().to_vec();
    let original_keep = editor.get_keep_segments();

    assert!(editor.delete_word(2), "delete_word(2) should succeed");
    assert_ne!(editor.get_keep_segments(), original_keep);

    assert!(editor.undo(), "undo should succeed");

    let restored = editor.get_words();
    assert_eq!(
        restored.len(),
        original_words.len(),
        "undo should restore word count",
    );
    for (o, r) in original_words.iter().zip(restored.iter()) {
        assert_eq!(o.start_us, r.start_us, "start_us drift after undo");
        assert_eq!(o.end_us, r.end_us, "end_us drift after undo");
        assert_eq!(o.text, r.text, "text drift after undo");
        assert_eq!(o.deleted, r.deleted, "deleted flag drift after undo");
    }
    assert_eq!(
        editor.get_keep_segments(),
        original_keep,
        "keep_segments drift after undo",
    );
}

/// Midstream deletion splice: deleting a word in the middle must
/// produce keep-segments whose boundaries match the kept words'
/// original timestamps exactly — no smoothing, no remnants.
#[test]
fn precision_eval_midstream_delete_clean_splice() {
    let words = heterogeneous_words();
    let mut editor = EditorState::new();
    editor.set_words(words.clone());

    assert!(editor.delete_word(2), "delete 'brown'");

    let segs = editor.get_keep_segments();
    assert_eq!(segs.len(), 2, "expected exactly two keep segments");

    let (a_start, a_end) = segs[0];
    let (b_start, b_end) = segs[1];
    assert_eq!(a_start, words[0].start_us, "first segment start drift");
    assert_eq!(a_end, words[1].end_us, "first segment end drift");
    assert_eq!(b_start, words[3].start_us, "second segment start drift");
    assert_eq!(b_end, words[4].end_us, "second segment end drift");

    let edit_point = a_end - a_start;
    let mapped = editor.map_edit_time_to_source_time(edit_point);
    assert_eq!(
        mapped, b_start,
        "splice point must map to the start of the next kept word — \
         any other value means remnant content leaked through",
    );
}

/// Edit-time → source-time mapping stays monotonic across multiple
/// deletions. A non-monotonic map would play audio out of order.
#[test]
fn precision_eval_time_mapping_monotonic_after_multiple_deletes() {
    let words = heterogeneous_words();
    let mut editor = EditorState::new();
    editor.set_words(words);
    assert!(editor.delete_word(1));
    assert!(editor.delete_word(3));

    let samples = [0_i64, 100_000, 250_000, 500_000, 800_000];
    let mut prev = 0_i64;
    for (i, &edit_t) in samples.iter().enumerate() {
        let src = editor.map_edit_time_to_source_time(edit_t);
        if i > 0 {
            assert!(
                src >= prev,
                "time map non-monotonic at sample {i}: prev={prev} got={src}",
            );
        }
        prev = src;
    }
}

/// Fixture-based precision eval.
///
/// Loads the checked-in golden word fixture
/// (`src-tauri/tests/fixtures/toaster_example.words.golden.json`) and
/// validates the full precision contract in one pass:
///
/// 1. Loaded durations are heterogeneous (anti-synthesis baseline).
/// 2. `set_words` preserves every field byte-for-byte.
/// 3. A midstream deletion produces keep-segments whose boundaries
///    match the fixture's original per-word timestamps exactly.
/// 4. Delete + undo round-trips the fixture to its original state.
///
/// Any future regression in word timing, keep-segment arithmetic, or
/// undo fidelity will fail this test. DO NOT regenerate the fixture
/// without human verification — see
/// `.github/skills/transcript-precision-eval/SKILL.md`.
#[test]
fn precision_eval_golden_fixture_roundtrip() {
    #[derive(serde::Deserialize)]
    struct Fixture {
        words: Vec<Word>,
    }

    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("fixtures")
        .join("toaster_example.words.golden.json");
    let raw = std::fs::read_to_string(&path)
        .unwrap_or_else(|e| panic!("read golden fixture {}: {}", path.display(), e));
    let fixture: Fixture = serde_json::from_str(&raw)
        .unwrap_or_else(|e| panic!("parse golden fixture {}: {}", path.display(), e));
    let words = fixture.words;
    assert!(
        words.len() >= 6,
        "golden fixture must have >= 6 words to exercise midstream splice"
    );

    let durations: Vec<i64> = words.iter().map(|w| w.end_us - w.start_us).collect();
    let unique: std::collections::HashSet<i64> = durations.iter().copied().collect();
    assert!(
        unique.len() >= durations.len() / 2,
        "golden fixture durations collapsed to too few unique values ({unique:?}) — \
         fixture may have been regenerated by a synthesis path"
    );

    let mut editor = EditorState::new();
    editor.set_words(words.clone());

    let loaded = editor.get_words().to_vec();
    for (orig, got) in words.iter().zip(loaded.iter()) {
        assert_eq!(orig.text, got.text);
        assert_eq!(orig.start_us, got.start_us, "start_us drift on load");
        assert_eq!(orig.end_us, got.end_us, "end_us drift on load");
        assert_eq!(orig.confidence, got.confidence);
        assert_eq!(orig.speaker_id, got.speaker_id);
    }

    let mid = words.len() / 2;
    let baseline_keep = editor.get_keep_segments();
    assert!(editor.delete_word(mid), "delete_word({mid})");

    let segs = editor.get_keep_segments();
    assert_eq!(
        segs.len(),
        2,
        "midstream delete should produce exactly two keep-segments"
    );
    let (a_start, a_end) = segs[0];
    let (b_start, b_end) = segs[1];
    assert_eq!(a_start, words[0].start_us, "seg-A start drift");
    assert_eq!(a_end, words[mid - 1].end_us, "seg-A end drift");
    assert_eq!(b_start, words[mid + 1].start_us, "seg-B start drift");
    assert_eq!(b_end, words[words.len() - 1].end_us, "seg-B end drift");

    let edit_point = a_end - a_start;
    let mapped = editor.map_edit_time_to_source_time(edit_point);
    assert_eq!(
        mapped, b_start,
        "midstream splice remnant leaked: mapped={mapped} expected={b_start}"
    );

    assert!(editor.undo(), "undo after midstream delete");
    assert_eq!(
        editor.get_keep_segments(),
        baseline_keep,
        "undo did not restore keep-segment baseline"
    );
    let restored = editor.get_words();
    for (orig, got) in words.iter().zip(restored.iter()) {
        assert_eq!(orig.start_us, got.start_us, "start_us drift after undo");
        assert_eq!(orig.end_us, got.end_us, "end_us drift after undo");
        assert_eq!(orig.deleted, got.deleted, "deleted flag drift after undo");
    }
}

/// End-to-end precision contract for `remove_silence`.
///
/// Regression guard for the bug fixed by switching `filler::trim_pauses`
/// from in-place timestamp shifting to silence-sentinel insertion. The
/// pre-fix implementation mutated `word.start_us` / `word.end_us` of
/// real words; downstream code in [`EditorState::get_keep_segments`]
/// continued to read those fields as **source-timeline** microseconds,
/// silently corrupting preview/waveform/export. This test asserts the
/// post-fix contract:
///
/// 1. Real-word source timestamps are byte-identical pre/post trim.
/// 2. Each long gap collapses to a clean keep-segment seam (the
///    sentinel splits the segment exactly the way a user-driven
///    delete would).
/// 3. The full pipeline through `timing_contract_snapshot` reports
///    `keep_segments_valid = true` (no overlap, no out-of-bounds).
/// 4. Edit-time → source-time mapping at the seam returns the next
///    real word's `start_us`, proving no remnant audio leaks across
///    the splice.
/// 5. `remove_silence` is idempotent — a second invocation is a no-op.
#[test]
fn precision_eval_remove_silence_preserves_source_time_and_seams() {
    use crate::managers::filler::{
        is_silence_sentinel, trim_pauses, REMOVE_SILENCE_MAX_GAP_US, REMOVE_SILENCE_THRESHOLD_US,
    };

    // Heterogeneous fixture with two long pauses ≥ threshold (800 ms)
    // and one short gap that must remain untouched.
    let original_words = vec![
        Word {
            text: "alpha".into(),
            start_us: 0,
            end_us: 500_000,
            deleted: false,
            silenced: false,
            confidence: 0.95,
            speaker_id: 0,
        },
        Word {
            text: "beta".into(),
            // 1.0 s gap (above 750 ms threshold) → sentinel
            start_us: 1_500_000,
            end_us: 2_000_000,
            deleted: false,
            silenced: false,
            confidence: 0.92,
            speaker_id: 0,
        },
        Word {
            text: "gamma".into(),
            // 0.2 s gap → below threshold, no sentinel
            start_us: 2_200_000,
            end_us: 2_700_000,
            deleted: false,
            silenced: false,
            confidence: 0.90,
            speaker_id: 0,
        },
        Word {
            text: "delta".into(),
            // 1.5 s gap (above threshold) → sentinel
            start_us: 4_200_000,
            end_us: 4_900_000,
            deleted: false,
            silenced: false,
            confidence: 0.88,
            speaker_id: 0,
        },
    ];

    let mut editor = EditorState::new();
    editor.set_words(original_words.clone());
    let baseline_keep = editor.get_keep_segments();

    // Drive the same primitive `commands::remove_silence` calls.
    let words_mut = editor.get_words_vec_mut();
    let trimmed = trim_pauses(
        words_mut,
        REMOVE_SILENCE_THRESHOLD_US,
        REMOVE_SILENCE_MAX_GAP_US,
    );
    assert_eq!(trimmed, 2, "expected exactly two qualifying gaps");

    // (1) Real-word source timestamps are byte-identical.
    let after = editor.get_words().to_vec();
    for orig in &original_words {
        let surviving = after
            .iter()
            .find(|w| !is_silence_sentinel(w) && w.text == orig.text)
            .unwrap_or_else(|| panic!("real word {} missing after trim", orig.text));
        assert_eq!(
            surviving.start_us, orig.start_us,
            "{} start_us drifted: source-time invariant violated",
            orig.text
        );
        assert_eq!(
            surviving.end_us, orig.end_us,
            "{} end_us drifted: source-time invariant violated",
            orig.text
        );
    }

    // (2) Sentinel coverage matches the [prev.end, next.start] gap exactly
    //     for each long pause; the short gap has no sentinel.
    let sentinels: Vec<_> = after.iter().filter(|w| is_silence_sentinel(w)).collect();
    assert_eq!(sentinels.len(), 2, "expected one sentinel per long gap");
    assert_eq!(sentinels[0].start_us, 500_000, "sentinel-0 start drift");
    assert_eq!(sentinels[0].end_us, 1_500_000, "sentinel-0 end drift");
    assert_eq!(sentinels[1].start_us, 2_700_000, "sentinel-1 start drift");
    assert_eq!(sentinels[1].end_us, 4_200_000, "sentinel-1 end drift");

    // (3) Keep-segments must split at every long-gap sentinel. The
    //     200 ms short gap stays inside one segment because the
    //     intra-segment-gap budget already absorbs it.
    let segs = editor.get_keep_segments();
    assert_eq!(segs.len(), 3, "expected 3 segments split at the 2 sentinels");
    assert_eq!(segs[0], (0, 500_000), "segment-0 boundary drift");
    assert_eq!(
        segs[1],
        (1_500_000, 2_700_000),
        "segment-1 must merge beta+gamma across short gap"
    );
    assert_eq!(segs[2], (4_200_000, 4_900_000), "segment-2 boundary drift");

    // (4) Public timing-contract validator stays green.
    let snapshot = editor.timing_contract_snapshot();
    assert!(
        snapshot.keep_segments_valid,
        "validator failed after remove_silence: {:?}",
        snapshot.warning
    );

    // (5) Edit-time → source-time mapping at the splice seam returns
    //     the next real word's start_us — proves no remnant leaks.
    let seg0_dur = segs[0].1 - segs[0].0;
    let mapped_at_seam = editor.map_edit_time_to_source_time(seg0_dur);
    assert_eq!(
        mapped_at_seam, segs[1].0,
        "splice point must map cleanly to start of next kept segment; \
         leak detected"
    );

    // (6) Idempotence: a second `trim_pauses` is a no-op.
    let words_mut = editor.get_words_vec_mut();
    let second = trim_pauses(
        words_mut,
        REMOVE_SILENCE_THRESHOLD_US,
        REMOVE_SILENCE_MAX_GAP_US,
    );
    assert_eq!(second, 0, "remove_silence not idempotent");

    // (7) Keep-segments before and after trim are *identical* — long
    //     gaps already exceeded MAX_INTRA_SEGMENT_GAP_US (200 ms) and
    //     were therefore excluded from the audio timeline before the
    //     trim ran. The pre-fix bug (in-place start_us/end_us shift)
    //     made the segment **boundaries** drift even though their
    //     count was unchanged; this stronger equality assertion
    //     pins both the count *and* the source-time values.
    assert_eq!(
        editor.get_keep_segments(),
        baseline_keep,
        "remove_silence corrupted segment boundaries — \
         source-time fields were mutated by `trim_pauses`"
    );
}

/// User-workflow regression: "Remove fillers" → "Remove silence".
///
/// Reported in the field: after deleting filler words, Remove silence
/// reported "0 pauses found" because the count walk treated any deleted
/// word in a gap as a bridge. The fix narrows that check to
/// `is_silence_sentinel`, so deleted fillers no longer suppress the
/// detection of surrounding dead air. This test pins both the count
/// and the resulting keep-segment structural validity end-to-end.
#[test]
fn precision_eval_remove_silence_finds_pauses_after_filler_deletion() {
    use crate::managers::filler::{
        count_trimmable_pauses, trim_pauses, REMOVE_SILENCE_MAX_GAP_US,
        REMOVE_SILENCE_THRESHOLD_US,
    };

    // alpha — short gap — "um" filler — long silent gap — beta.
    // The filler's own audio is excised by deletion, but ~1.7 s of
    // dead air still surrounds it.
    let original_words = vec![
        Word {
            text: "alpha".into(),
            start_us: 0,
            end_us: 500_000,
            deleted: false,
            silenced: false,
            confidence: 0.95,
            speaker_id: 0,
        },
        Word {
            text: "um".into(),
            start_us: 600_000,
            end_us: 800_000,
            deleted: false,
            silenced: false,
            confidence: 0.50,
            speaker_id: 0,
        },
        Word {
            text: "beta".into(),
            start_us: 2_500_000,
            end_us: 3_000_000,
            deleted: false,
            silenced: false,
            confidence: 0.93,
            speaker_id: 0,
        },
    ];

    let mut editor = EditorState::new();
    editor.set_words(original_words.clone());

    // Step 1: simulate "Remove fillers" by marking the filler deleted.
    assert!(editor.delete_word(1), "delete filler 'um'");

    // Step 2: simulate "Remove silence". The pre-fix dry-run / mutating
    // count would both return 0 here; the post-fix code must return 1.
    let dry_run = count_trimmable_pauses(
        editor.get_words(),
        REMOVE_SILENCE_THRESHOLD_US,
        REMOVE_SILENCE_MAX_GAP_US,
    );
    assert_eq!(
        dry_run, 1,
        "dry-run count: deleted filler in gap must NOT block detection"
    );

    let words_mut = editor.get_words_vec_mut();
    let trimmed = trim_pauses(
        words_mut,
        REMOVE_SILENCE_THRESHOLD_US,
        REMOVE_SILENCE_MAX_GAP_US,
    );
    assert_eq!(
        trimmed, dry_run,
        "applied count drift from dry-run — parity broken"
    );

    // Filler is still deleted with its original boundaries.
    let after = editor.get_words().to_vec();
    let filler = after
        .iter()
        .find(|w| w.text == "um")
        .expect("filler still in word list");
    assert!(filler.deleted, "filler must remain user-deleted");
    assert_eq!(filler.start_us, 600_000);
    assert_eq!(filler.end_us, 800_000);

    // Snapshot stays valid (no overlap, no out-of-bounds).
    let snapshot = editor.timing_contract_snapshot();
    assert!(
        snapshot.keep_segments_valid,
        "keep-segments invalid after filler-then-silence: {:?}",
        snapshot.warning
    );

    // Idempotence: running Remove silence a second time is a no-op.
    let second_dry = count_trimmable_pauses(
        editor.get_words(),
        REMOVE_SILENCE_THRESHOLD_US,
        REMOVE_SILENCE_MAX_GAP_US,
    );
    assert_eq!(second_dry, 0, "second-call must report 0 — sentinel not bridging");
    let words_mut = editor.get_words_vec_mut();
    let second_trim = trim_pauses(
        words_mut,
        REMOVE_SILENCE_THRESHOLD_US,
        REMOVE_SILENCE_MAX_GAP_US,
    );
    assert_eq!(second_trim, 0);
}

/// Audio-truth Remove Silence: drives a synthetic PCM buffer through
/// `audio_toolkit::silence::detect_silent_ranges`, inserts the resulting
/// sentinels at sorted positions in the word list (replicating the
/// `commands::filler::remove_silence` flow without the Tauri State
/// machinery), then asserts:
///
/// - At least 5 silence sentinels are inserted (matching the 5 red
///   flatline regions in the user's screenshot).
/// - `timing_contract_snapshot().keep_segments_valid` stays true.
/// - `total_keep_duration_us` is strictly less than the source duration.
/// - No silent region from the audio survives in any keep-segment.
///
/// This is the end-to-end gate for the audio-truth pipeline: if the
/// silence detector, sentinel insertion, or `get_keep_segments`
/// interval-subtraction regresses, this test fails.
#[test]
fn precision_eval_audio_truth_remove_silence() {
    use crate::audio_toolkit::{detect_silent_ranges, SilenceDetectConfig};
    use crate::managers::filler::make_silence_sentinel;

    const SAMPLE_RATE: u32 = 16_000;

    // Build a synthetic 30-second audio buffer: 6 speech regions
    // separated by 5 long silences. Each speech region is 4 s loud,
    // each silence is 2 s flat. Total = 6*4 + 5*2 = 34 s. Mirrors the
    // user's screenshot: 5 visible flatlines.
    let speech_us = 4_000_000_i64;
    let silence_us = 2_000_000_i64;
    let mut samples: Vec<f32> = Vec::new();
    let mut speech_starts: Vec<i64> = Vec::new();
    let mut silence_starts: Vec<i64> = Vec::new();
    let mut cursor_us: i64 = 0;
    for i in 0..6 {
        speech_starts.push(cursor_us);
        let n = ((SAMPLE_RATE as i64 * speech_us) / 1_000_000) as usize;
        // Square wave at 0.9 amplitude so peak == 0.9 in every window.
        for j in 0..n {
            samples.push(if j % 2 == 0 { 0.9 } else { -0.9 });
        }
        cursor_us += speech_us;
        if i < 5 {
            silence_starts.push(cursor_us);
            let n = ((SAMPLE_RATE as i64 * silence_us) / 1_000_000) as usize;
            samples.extend(std::iter::repeat(0.0).take(n));
            cursor_us += silence_us;
        }
    }
    let total_source_us = cursor_us;

    // Build a Parakeet-style word list. Each speech region holds one
    // word that is INTENTIONALLY padded past its actual speech into the
    // surrounding silence (mimicking `PARAKEET_OUTER_TRIM_US`). This is
    // the failure mode that the previous word-gap algorithm could not
    // see: the silences sit *inside* word ranges.
    let mut words: Vec<Word> = Vec::new();
    for (i, &start) in speech_starts.iter().enumerate() {
        // Pad each word's start back by 200 ms (clamped to 0) and its
        // end forward by 300 ms (clamped to total_source_us) to mimic
        // Parakeet padding. The first/last words receive extra padding.
        let pad_left = if i == 0 { 0 } else { 200_000 };
        let pad_right = if i == 5 { 0 } else { 300_000 };
        let padded_start = (start - pad_left).max(0);
        let padded_end = (start + speech_us + pad_right).min(total_source_us);
        words.push(Word {
            text: format!("speech_{}", i),
            start_us: padded_start,
            end_us: padded_end,
            deleted: false,
            silenced: false,
            confidence: 0.95,
            speaker_id: 0,
        });
    }

    let mut editor = EditorState::new();
    editor.set_words(words);

    // Run the audio-truth detector.
    let cfg = SilenceDetectConfig::default();
    let detected = detect_silent_ranges(&samples, SAMPLE_RATE, &cfg);
    assert!(
        detected.len() >= 5,
        "expected at least 5 detected silent ranges (matching the user's \
         screenshot), got {} ranges: {:?}",
        detected.len(),
        detected
    );

    // Insert each detected range as a silence sentinel (sorted by start).
    let words_mut = editor.get_words_vec_mut();
    for (s, e) in &detected {
        let insert_idx = match words_mut.binary_search_by_key(s, |w| w.start_us) {
            Ok(idx) | Err(idx) => idx,
        };
        words_mut.insert(insert_idx, make_silence_sentinel(*s, *e));
    }
    editor.bump_revision();

    // Validate the timing contract.
    let snapshot = editor.timing_contract_snapshot();
    assert!(
        snapshot.keep_segments_valid,
        "keep-segments invalid after audio-truth sentinel insertion: {:?}",
        snapshot.warning
    );

    // Total kept duration must be strictly less than the source duration —
    // we excised at least 5*~2s of silence (with up to 30 ms of grid slop
    // per silence end). Lower bound: 5 * (silence_us - 60_000) = 9.7 s.
    let trimmed_us = total_source_us - snapshot.total_keep_duration_us;
    assert!(
        trimmed_us >= 5 * (silence_us - 60_000),
        "expected at least {} us trimmed, got {} (kept {} of {})",
        5 * (silence_us - 60_000),
        trimmed_us,
        snapshot.total_keep_duration_us,
        total_source_us
    );

    // Each keep-segment must avoid the cores of the silent regions
    // (give the detector one window of grid slop on each side).
    let one_window = cfg.window_us;
    for seg in &snapshot.keep_segments {
        for &silence_start in &silence_starts {
            let silence_end = silence_start + silence_us;
            // Inner core (not touching the boundary) must not overlap
            // any keep-segment.
            let core_start = silence_start + one_window;
            let core_end = silence_end - one_window;
            assert!(
                seg.end_us <= core_start || seg.start_us >= core_end,
                "keep-segment {:?} overlaps the core of silence \
                 [{}, {}]",
                seg,
                silence_start,
                silence_end
            );
        }
    }
}
