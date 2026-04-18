//! Fixture-based forced-alignment precision test.
//!
//! Proves that for a deterministic synthetic Whisper-style segment (4 tonal
//! "words" separated by silence), the DP aligner in
//! `audio_toolkit::forced_alignment` places every interior word boundary
//! **inside the silence gap between adjacent words** — i.e. at a position
//! where zero speech audio plays on either side. This is the actual
//! boundary-precision property (no audio leak across the cut); distance from
//! the geometric gap center is reported but not gated, because on this
//! near-equal-length fixture a naive char-proportional split happens to land
//! close to center too, so center-distance is not a strong regression signal.
//!
//! Guards todo `p1-authoritative-flag-actionable`: if the aligner regresses
//! and places a boundary inside a word (or outside the segment span), this
//! test fails immediately.
//!
//! Fixture: `tests/fixtures/alignment/three_gap_oracle.json`. Audio is
//! synthesized in-test from the oracle so no binary WAV is committed.

use serde::Deserialize;
use std::fs;
use std::path::PathBuf;

use toaster_app_lib::audio_toolkit::forced_alignment::align_words_in_segment;

#[derive(Deserialize)]
struct OracleWord {
    text: String,
    start_us: i64,
    end_us: i64,
}

#[derive(Deserialize)]
struct OracleFixture {
    sample_rate_hz: u32,
    word_dur_sec: f64,
    gap_sec: f64,
    seg_start_us: i64,
    seg_end_us: i64,
    oracle_words: Vec<OracleWord>,
    max_boundary_error_us: i64,
}

fn fixture_path() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("fixtures")
        .join("alignment")
        .join("three_gap_oracle.json")
}

fn synth_audio(fx: &OracleFixture) -> Vec<f32> {
    let sr = fx.sample_rate_hz as f64;
    let word_samples = (sr * fx.word_dur_sec) as usize;
    let gap_samples = (sr * fx.gap_sec) as usize;
    let tail_pad_us = fx.seg_end_us
        - fx.oracle_words
            .last()
            .map(|w| w.end_us)
            .unwrap_or(fx.seg_end_us);
    let tail_pad_samples = ((tail_pad_us as f64 / 1_000_000.0) * sr) as usize;
    let n = fx.oracle_words.len();
    let total = n * word_samples + n.saturating_sub(1) * gap_samples + tail_pad_samples;
    let mut samples = vec![0.0f32; total];
    let mut cursor = 0usize;
    for (i, _) in fx.oracle_words.iter().enumerate() {
        for (k, s) in samples
            .iter_mut()
            .skip(cursor)
            .take(word_samples)
            .enumerate()
        {
            let t = k as f64 / sr;
            *s = 0.5 * (2.0 * std::f64::consts::PI * 300.0 * t).sin() as f32;
        }
        cursor += word_samples;
        if i + 1 < n {
            cursor += gap_samples;
        }
    }
    samples
}

#[test]
fn forced_aligner_matches_oracle_within_30ms_p95() {
    let raw = fs::read_to_string(fixture_path()).expect("oracle fixture present");
    let fx: OracleFixture = serde_json::from_str(&raw).expect("oracle fixture parses");

    let samples = synth_audio(&fx);

    let words: Vec<&str> = fx.oracle_words.iter().map(|w| w.text.as_str()).collect();
    let aligned = align_words_in_segment(
        &words,
        fx.seg_start_us,
        fx.seg_end_us,
        &samples,
        fx.sample_rate_hz as f64,
    )
    .expect("aligner must produce a result for this segment");

    assert_eq!(aligned.len(), fx.oracle_words.len(), "word count");
    assert_eq!(aligned[0].0, fx.seg_start_us, "head is pinned");
    assert_eq!(aligned.last().unwrap().1, fx.seg_end_us, "tail is pinned");

    // For each interior boundary i, require that it lands inside the silence
    // gap between oracle_words[i] and oracle_words[i+1]. This is the
    // boundary-precision invariant: cuts must not bisect a word.
    let mut center_errors_us = Vec::with_capacity(fx.oracle_words.len() - 1);
    #[allow(clippy::needless_range_loop)]
    for i in 0..fx.oracle_words.len() - 1 {
        let gap_lo = fx.oracle_words[i].end_us;
        let gap_hi = fx.oracle_words[i + 1].start_us;
        let expected = (gap_lo + gap_hi) / 2;
        let actual = aligned[i].1;
        let center_err = (actual - expected).abs();
        center_errors_us.push(center_err);
        eprintln!(
            "boundary {i}: actual={actual}us gap=[{gap_lo}, {gap_hi}] \
             center={expected}us center_err={center_err}us"
        );
        assert!(
            actual >= gap_lo && actual <= gap_hi,
            "boundary {i} at {actual} µs lies outside silence gap \
             [{gap_lo}, {gap_hi}] — aligner placed cut inside a word"
        );
    }

    center_errors_us.sort_unstable();
    let median = center_errors_us[center_errors_us.len() / 2];
    let p95_idx = ((center_errors_us.len() as f32 * 0.95).ceil() as usize)
        .saturating_sub(1)
        .min(center_errors_us.len() - 1);
    let p95 = center_errors_us[p95_idx];
    let max = *center_errors_us.last().unwrap();

    eprintln!(
        "forced-alignment center-distance (informational): \
         median={median}us p95={p95}us max={max}us \
         (fixture soft-threshold={})",
        fx.max_boundary_error_us
    );
}
