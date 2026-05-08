//! Word-from-segment construction + timestamp sanitization (extracted from mod.rs).

use log::{info, warn};
use transcribe_rs::TranscriptionSegment;

use super::alignment::{
    align_onset_boundaries, correct_short_word_boundaries, refine_word_boundaries,
};
use super::{WordAlignmentMeta, SAMPLE_RATE_HZ};
use crate::audio_toolkit::timing::{round_f64_to_i64, seconds_to_us as timing_seconds_to_us};
use crate::managers::editor::Word;

/// Build word-level timestamps from transcription segments.
///
/// **Only called for non-authoritative engines** (Whisper, Moonshine, etc.)
/// whose adapters return `word_timestamps_authoritative = false`. Engines
/// with native per-word timestamps (Parakeet word-level) bypass this
/// entirely and flow through the authoritative path in `transcribe_media_file`.
///
/// Primary alignment is the DP-based forced aligner in
/// [`crate::audio_toolkit::forced_alignment`]: for each ASR segment, the
/// interior boundaries are placed at the frames that minimize the sum of
/// local acoustic energy and deviation from their char-proportional expected
/// position. The segment endpoints themselves come from the ASR engine and
/// are treated as authoritative.
///
/// When `vad` is `Some`, per-frame speech probabilities are computed for
/// each segment and fed into the DP cost function to penalize placing
/// boundaries on speech frames.
///
/// **Fallback to char-proportional split.** When the aligner declines a
/// segment (returns `None`), we fall back to the legacy character-proportional
/// distribution.
///
/// **Refinement gating.** The safety-net passes (`correct_short_word_boundaries`,
/// `refine_word_boundaries`) only fire on boundaries where at least one
/// adjacent word came from the fallback path. DP-aligned boundaries are
/// trusted and skipped. `realign_suspicious_spans` at the call site is
/// also gated on `!authoritative`.
pub(super) fn build_words_from_segments(
    full_text: &str,
    segments: &[TranscriptionSegment],
    samples: &[f32],
    mut vad: Option<&mut crate::audio_toolkit::vad::SileroVad>,
    has_pre_speech_padding: bool,
) -> (Vec<Word>, Vec<WordAlignmentMeta>) {
    let mut words = Vec::new();
    let mut meta = Vec::new();

    // The filtered text may differ from segment text (due to filler filtering,
    // custom word correction). We'll use the final text's words and match them
    // against segment boundaries for the best timestamp assignment.
    let final_words: Vec<&str> = full_text.split_whitespace().collect();

    if final_words.is_empty() || segments.is_empty() {
        return (words, meta);
    }

    // Clamp overlapping segments. Whisper hallucination loops can produce
    // segments whose time ranges overlap, corrupting all downstream word
    // timestamps. We clamp each segment's start to be >= the previous
    // segment's end so the DP aligner sees a strictly monotonic timeline.
    let clamped_segments = clamp_overlapping_segments(segments);

    // Build a flat list of (word, start_us, end_us) from segments first.
    // For each segment we prefer the DP forced aligner; if it declines we
    // fall through to the legacy char-proportional split.
    let mut segment_words: Vec<(String, i64, i64)> = Vec::new();
    // Track whether each segment_word came from the DP aligner (true) or
    // the char-proportional fallback (false). Downstream refinement passes
    // skip DP-aligned boundaries — the global energy optimizer already
    // placed them at the best available position.
    let mut segment_dp_aligned: Vec<bool> = Vec::new();
    for seg in &clamped_segments {
        let seg_text = seg.text.trim();
        if seg_text.is_empty() {
            continue;
        }
        // Half-open convention: a segment covers [seg_start_us, seg_end_us).
        // Both ends are rounded with the same nearest-integer policy so the
        // duration `end - start` is not biased by mixing floor+ceil.
        let seg_start_us = timing_seconds_to_us(seg.start as f64);
        let seg_end_us = timing_seconds_to_us(seg.end as f64);
        let seg_duration_us = seg_end_us - seg_start_us;

        let seg_words: Vec<&str> = seg_text.split_whitespace().collect();
        if seg_words.is_empty() {
            continue;
        }

        // Primary path: DP forced alignment against frame-level RMS.
        // When a VAD is available, compute per-frame speech probabilities for
        // this segment's audio and pass them to the DP cost function. This
        // penalizes placing word boundaries on speech frames, improving
        // alignment on content where energy alone is ambiguous.
        let vad_probs = if let Some(ref mut vad_inst) = vad {
            use crate::audio_toolkit::timing::us_to_sample_clamped;
            let start_s = us_to_sample_clamped(seg_start_us, SAMPLE_RATE_HZ, samples.len());
            let end_s_raw = us_to_sample_clamped(seg_end_us, SAMPLE_RATE_HZ, samples.len());
            let end_s = if end_s_raw + 1 >= samples.len() { samples.len() } else { end_s_raw };
            if end_s > start_s {
                let slice = &samples[start_s..end_s];
                let energy_frames = crate::audio_toolkit::forced_alignment::EnergyFrames::compute(
                    slice,
                    SAMPLE_RATE_HZ,
                );
                crate::audio_toolkit::forced_alignment::compute_vad_probs(
                    vad_inst,
                    slice,
                    energy_frames.frames.len(),
                )
            } else {
                None
            }
        } else {
            None
        };
        if let Some(mut aligned) = crate::audio_toolkit::forced_alignment::align_words_in_segment(
            &seg_words,
            seg_start_us,
            seg_end_us,
            samples,
            SAMPLE_RATE_HZ,
            vad_probs.as_deref(),
        ) {
            // Trim leading silence from the first word. The DP aligner pins
            // boundary[0] to `seg_start_us`, which often includes pre-speech
            // padding from the ASR engine (200-300 ms on Parakeet, variable
            // on Whisper). Use aggressive trim for engines with known padding.
            if let Some(first) = aligned.first_mut() {
                let trimmed_start = if has_pre_speech_padding {
                    crate::audio_toolkit::silence_trim::trim_leading_silence_padded(
                        samples,
                        first.0,
                        first.1,
                        SAMPLE_RATE_HZ,
                    )
                } else {
                    crate::audio_toolkit::silence_trim::trim_leading_silence(
                        samples,
                        first.0,
                        first.1,
                        SAMPLE_RATE_HZ,
                    )
                };
                if trimmed_start < first.1 {
                    first.0 = trimmed_start;
                }
            }
            // Trim trailing silence from the last word. The DP aligner pins
            // the last word's end to `seg_end_us`, which often includes a
            // long trailing pause reported by the ASR engine. Detecting and
            // trimming this silence prevents "for." from spanning 6+ seconds.
            if let Some(last) = aligned.last_mut() {
                let trimmed_end = crate::audio_toolkit::silence_trim::trim_trailing_silence(
                    samples,
                    last.0,   // last word's start
                    last.1,   // last word's end (== seg_end_us)
                    SAMPLE_RATE_HZ,
                );
                if trimmed_end > last.0 {
                    last.1 = trimmed_end;
                }
            }
            for (sw, (ws, we)) in seg_words.iter().zip(aligned) {
                segment_words.push(((*sw).to_string(), ws, we));
                segment_dp_aligned.push(true);
            }
            continue;
        }

        // Fallback path: syllable-proportional split. Fires when the aligner
        // cannot run (segment too short, too few frames, or slice outside
        // the sample buffer). Uses syllable count as a better proxy for
        // spoken duration than raw character count.
        let weights: Vec<usize> = seg_words
            .iter()
            .map(|w| {
                crate::managers::transcription::adapter_normalize::estimate_syllables(w)
            })
            .collect();
        let total_weight: usize = weights.iter().sum();

        let mut cursor_us = seg_start_us;
        for (j, sw) in seg_words.iter().enumerate() {
            let fraction = weights[j] as f64 / total_weight as f64;
            let word_duration_us = round_f64_to_i64(seg_duration_us as f64 * fraction);

            let mut word_start = cursor_us;
            // First word: trim leading silence (symmetric with last-word
            // trailing trim below). Use aggressive trim for padded engines.
            if j == 0 {
                let trimmed = if has_pre_speech_padding {
                    crate::audio_toolkit::silence_trim::trim_leading_silence_padded(
                        samples,
                        cursor_us,
                        seg_end_us,
                        SAMPLE_RATE_HZ,
                    )
                } else {
                    crate::audio_toolkit::silence_trim::trim_leading_silence(
                        samples,
                        cursor_us,
                        seg_end_us,
                        SAMPLE_RATE_HZ,
                    )
                };
                if trimmed < seg_end_us {
                    word_start = trimmed;
                }
            }
            let mut word_end = if j == seg_words.len() - 1 {
                // Last word: trim trailing silence instead of absorbing the
                // entire segment remainder.
                let trimmed = crate::audio_toolkit::silence_trim::trim_trailing_silence(
                    samples,
                    cursor_us,
                    seg_end_us,
                    SAMPLE_RATE_HZ,
                );
                if trimmed > cursor_us { trimmed } else { seg_end_us }
            } else {
                cursor_us + word_duration_us
            };

            // Safety: ensure positive duration
            if word_end <= word_start {
                word_end = word_start + 1;
            }

            segment_words.push((sw.to_string(), word_start, word_end));
            segment_dp_aligned.push(false);
            cursor_us = word_end;
        }
    }

    // Now match filtered final_words against segment_words.
    // The final text may have had filler words removed or words corrected,
    // so we do a greedy forward match. If a final word matches a segment word,
    // use that segment word's timestamps. If not, interpolate.
    let mut seg_idx = 0;
    let mut interpolated_count = 0usize;
    let mut interpolation_examples: Vec<String> = Vec::new();
    for fw in &final_words {
        let fw_lower = fw.to_lowercase();

        // Try to find a matching segment word from current position forward.
        // Use a large lookahead (20 words) to tolerate filler removal, stutters,
        // and word corrections that can shift alignment significantly.
        let mut found = false;
        let search_limit = (seg_idx + 20).min(segment_words.len());
        for (k, seg_word) in segment_words
            .iter()
            .enumerate()
            .skip(seg_idx)
            .take(search_limit.saturating_sub(seg_idx))
        {
            let seg_word_lower = seg_word.0.to_lowercase();
            // Fuzzy match: segment text might have punctuation attached
            if seg_word_lower == fw_lower
                || seg_word_lower.starts_with(&fw_lower)
                || fw_lower.starts_with(&seg_word_lower)
                || seg_word_lower.trim_matches(|c: char| !c.is_alphanumeric()) == fw_lower
            {
                words.push(Word {
                    text: fw.to_string(),
                    start_us: seg_word.1,
                    end_us: seg_word.2,
                    deleted: false,
                    silenced: false,
                    confidence: -1.0,
                    speaker_id: -1,
                });
                meta.push(WordAlignmentMeta {
                    interpolated: false,
                    dp_aligned: segment_dp_aligned[k],
                });
                seg_idx = k + 1;
                found = true;
                break;
            }
        }

        if !found {
            // No match found — interpolate from nearest segment word and advance
            // the pointer so subsequent words don't all pile up at the same position
            let (start, end) = if seg_idx < segment_words.len() {
                let ts = (segment_words[seg_idx].1, segment_words[seg_idx].2);
                seg_idx += 1; // advance past this word to prevent repeated timestamps
                ts
            } else if let Some(last) = segment_words.last() {
                (last.1, last.2)
            } else {
                (0, 0)
            };
            interpolated_count += 1;
            if interpolation_examples.len() < 5 {
                interpolation_examples.push((*fw).to_string());
            }
            words.push(Word {
                text: fw.to_string(),
                start_us: start,
                end_us: end,
                deleted: false,
                silenced: false,
                confidence: -1.0,
                speaker_id: -1,
            });
            meta.push(WordAlignmentMeta { interpolated: true, dp_aligned: false });
        }
    }

    if interpolated_count > 0 {
        let ratio = interpolated_count as f64 / final_words.len() as f64;
        let sample_words = interpolation_examples.join(", ");
        if ratio >= 0.20 {
            warn!(
                "build_words_from_segments: high interpolation rate {}/{} ({:.1}%). examples: [{}]",
                interpolated_count,
                final_words.len(),
                ratio * 100.0,
                sample_words
            );
        } else {
            info!(
                "build_words_from_segments: interpolated {}/{} words ({:.1}%). examples: [{}]",
                interpolated_count,
                final_words.len(),
                ratio * 100.0,
                sample_words
            );
        }
    }

    // Pre-correction for short-word proportional boundaries.
    // Pass metadata so DP-aligned boundaries are trusted and skipped.
    correct_short_word_boundaries(&mut words, samples, Some(&meta));

    // Refine word boundaries by snapping to silence points in the audio.
    // DP-aligned boundaries are skipped — the global optimizer already
    // placed them at energy-optimal positions.
    refine_word_boundaries(&mut words, samples, Some(&meta));

    // Align segment-leading word starts to true speech onset
    align_onset_boundaries(&mut words, samples);

    (words, meta)
}

/// Defensive overlap clamping for segments arriving at the word builder.
///
/// The primary sanitization now happens upstream in the adapter layer
/// (`adapter_normalize::sanitize_segments`). This function is a lightweight
/// safety net that enforces monotonicity in case segments bypass the adapter
/// (e.g. test fixtures) or a future adapter forgets to sanitize.
///
/// Unlike the upstream version, this does NOT strip non-speech segments
/// (that's the adapter's job) — it only enforces time ordering.
pub(super) fn clamp_overlapping_segments(segments: &[TranscriptionSegment]) -> Vec<TranscriptionSegment> {
    if segments.is_empty() {
        return Vec::new();
    }

    let mut result: Vec<TranscriptionSegment> = Vec::with_capacity(segments.len());
    let mut overlap_count = 0usize;
    let mut dropped_count = 0usize;

    for seg in segments {
        let mut clamped = seg.clone();
        if let Some(prev) = result.last() {
            if clamped.start < prev.end {
                overlap_count += 1;
                clamped.start = prev.end;
            }
        }
        if clamped.end <= clamped.start {
            dropped_count += 1;
            continue;
        }
        result.push(clamped);
    }

    if overlap_count > 0 {
        warn!(
            "clamp_overlapping_segments (defensive): clamped {} overlaps, dropped {} \
             (segments should have been sanitized upstream)",
            overlap_count, dropped_count,
        );
    }

    // Log segment statistics for alignment debugging
    if !result.is_empty() {
        let durations: Vec<f64> = result.iter().map(|s| (s.end - s.start) as f64).collect();
        let min_dur = durations.iter().cloned().fold(f64::INFINITY, f64::min);
        let max_dur = durations.iter().cloned().fold(f64::NEG_INFINITY, f64::max);
        let avg_dur: f64 = durations.iter().sum::<f64>() / durations.len() as f64;
        let max_words = result
            .iter()
            .map(|s| s.text.split_whitespace().count())
            .max()
            .unwrap_or(0);

        info!(
            "build_words_from_segments: {} segments (min {:.2}s, max {:.2}s, avg {:.2}s), \
             max words/segment={}",
            result.len(), min_dur, max_dur, avg_dur, max_words,
        );
    }

    result
}

/// Sanitize word timestamps to guarantee monotonic, non-overlapping,
/// duration-positive ordering within [0, total_audio_duration_us].
///
/// Whisper segments (and proportional distribution within them) can
/// occasionally produce:
///   - start > end (inverted range)
///   - next.start < prev.end (overlap / rewind)
///   - values outside the actual audio duration
///
/// All of these break keep-segment calculation and cause playback jumps.
/// This function fixes them in a single forward pass without altering the
/// ordering of words.
pub(super) fn sanitize_word_timestamps(words: &mut [Word], total_duration_us: i64) {
    const MIN_WORD_DURATION_US: i64 = 1_000; // 1 ms minimum word duration

    let max_us = total_duration_us.max(0);
    let mut cursor_us: i64 = 0; // tracks the earliest start allowed for the next word

    for word in words.iter_mut() {
        // 1. Clamp both endpoints into [0, max_us]
        word.start_us = word.start_us.clamp(0, max_us);
        word.end_us = word.end_us.clamp(0, max_us);

        // 2. Enforce start <= end
        if word.start_us > word.end_us {
            word.end_us = word.start_us;
        }

        // 3. Enforce monotonic progression: start must be >= cursor
        if word.start_us < cursor_us {
            word.start_us = cursor_us;
            // Re-clamp start after shift
            word.start_us = word.start_us.min(max_us);
            // Ensure end is still >= start after shift
            if word.end_us < word.start_us {
                word.end_us = word.start_us;
            }
        }

        // 4. Ensure minimal non-zero duration where audio budget allows
        if word.end_us == word.start_us && word.start_us + MIN_WORD_DURATION_US <= max_us {
            word.end_us = word.start_us + MIN_WORD_DURATION_US;
        }

        cursor_us = word.end_us;
    }
}
