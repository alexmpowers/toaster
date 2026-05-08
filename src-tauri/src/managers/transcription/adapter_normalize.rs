//! Shared normalization helpers extracted from `adapter.rs` to keep the
//! per-engine adapter file under the 800-line cap.
//!
//! Per-engine [`super::TranscriptionModelAdapter`] impls compose these
//! helpers to produce a [`super::NormalizedTranscriptionResult`]. The
//! invariants enforced here back the
//! [transcription-adapter-contract](../../../../.github/skills/transcription-adapter-contract/SKILL.md)
//! gate: monotonic non-overlapping word spans, no zero-duration words, and
//! stripped non-speech tokens. No equal-duration synthesis happens here
//! either — the contract invariant documented in `adapter.rs` still holds.

use anyhow::Result;
use log::debug;
use transcribe_rs::{TranscriptionResult, TranscriptionSegment};

use super::adapter::{AudioInfo, CanonicalWord, NormalizedTranscriptionResult};

/// Patterns treated as non-speech / hallucination by every adapter that
/// uses [`is_non_speech_token`]. Intentionally conservative; precise filler
/// filtering lives downstream in `filter_transcription_output`.
const NON_SPEECH_MARKERS: &[&str] = &[
    "[MUSIC]",
    "[Music]",
    "[music]",
    "[APPLAUSE]",
    "[Applause]",
    "[applause]",
    "[LAUGHTER]",
    "[Laughter]",
    "[laughter]",
    "[SILENCE]",
    "[silence]",
    "[INAUDIBLE]",
    "[inaudible]",
    "(music)",
    "(applause)",
    "<|nospeech|>",
    "<|silence|>",
    "<unk>",
];

/// Returns `true` for tokens the adapter should strip. Matches bracketed
/// markers, Whisper special tokens, and common music-note hallucinations
/// (`♪`, `♫`). Whole-token match only — text like "the music" is not
/// filtered here.
pub(super) fn is_non_speech_token(text: &str) -> bool {
    let trimmed = text.trim();
    if trimmed.is_empty() {
        return true;
    }
    if NON_SPEECH_MARKERS
        .iter()
        .any(|m| m.eq_ignore_ascii_case(trimmed))
    {
        return true;
    }
    // Music-note hallucinations: a run of ♪/♫ (optionally with whitespace).
    if trimmed.chars().all(|c| matches!(c, '♪' | '♫' | ' ' | '\t')) {
        return true;
    }
    // Runs of 4+ identical punctuation (.... / ---- / ==== etc.) — common
    // Whisper hallucination on silence.
    if trimmed.chars().count() >= 4 {
        let first = trimmed.chars().next().unwrap();
        if !first.is_alphanumeric() && trimmed.chars().all(|c| c == first) {
            return true;
        }
    }
    false
}

/// Seconds (from `TranscriptionSegment`) -> microseconds. Uses
/// nearest-integer rounding to match `audio_toolkit::timing::seconds_to_us`.
fn seconds_to_us(s: f32) -> i64 {
    (s as f64 * 1_000_000.0).round() as i64
}

/// Char-proportional split of a segment's text across `[start_us, end_us)`.
/// Used by engines whose segment times are authoritative but whose word
/// boundaries aren't (Whisper, Moonshine, SenseVoice, GigaAM, Canary,
/// Cohere). See `build_words_from_segments` in `transcribe_file/mod.rs` for
/// the richer downstream refinement pass — the adapter only produces the
/// proportional baseline so the invariants hold.
fn split_segment_by_chars(seg_text: &str, start_us: i64, end_us: i64) -> Vec<(String, i64, i64)> {
    const MIN_WORD_CHAR_WEIGHT: usize = 1;
    let words: Vec<&str> = seg_text.split_whitespace().collect();
    if words.is_empty() || end_us <= start_us {
        return Vec::new();
    }
    let total: usize = words
        .iter()
        .map(|w| w.len().max(MIN_WORD_CHAR_WEIGHT))
        .sum();
    let duration_us = end_us - start_us;
    let mut out = Vec::with_capacity(words.len());
    let mut cursor = start_us;
    for (i, w) in words.iter().enumerate() {
        let share = (w.len().max(MIN_WORD_CHAR_WEIGHT) as f64 / total as f64 * duration_us as f64)
            .round() as i64;
        let word_end = if i == words.len() - 1 {
            end_us
        } else {
            cursor + share
        };
        out.push((w.to_string(), cursor, word_end));
        cursor = word_end;
    }
    out
}

/// Remove consecutive repeated phrases from the word list. ASR models
/// (especially on long audio with pauses) sometimes hallucinate by repeating
/// a phrase verbatim. We detect runs of N consecutive words (N ≥ 3) that
/// match the immediately preceding N words and remove the earlier copy,
/// keeping the later one (which often continues the sentence naturally).
///
/// Only exact case-insensitive word matches count. Single-word and two-word
/// repeats are intentionally kept — they're common in natural speech
/// ("very very", "no no no").
///
/// NOT called during transcription (transcript must be faithful to audio).
/// Available for the user-initiated "Clean Up" / filler-removal flow.
#[allow(dead_code)]
pub(crate) fn dedup_repeated_phrases(words: Vec<CanonicalWord>) -> Vec<CanonicalWord> {
    if words.len() < 6 {
        return words;
    }

    // Find spans to remove. Each entry is (start_idx, length) of the FIRST
    // (earlier) occurrence to remove.
    let mut remove_spans: Vec<(usize, usize)> = Vec::new();
    let texts_lower: Vec<String> = words.iter().map(|w| w.text.to_lowercase()).collect();
    let n = texts_lower.len();

    let mut i = 0;
    while i < n {
        // Try phrase lengths from large to small to find the longest repeat.
        let max_phrase = (n - i) / 2; // can't have a phrase longer than half the remaining
        let mut found_len = 0;
        for phrase_len in (3..=max_phrase.min(30)).rev() {
            if i + 2 * phrase_len > n {
                continue;
            }
            let first = &texts_lower[i..i + phrase_len];
            let second = &texts_lower[i + phrase_len..i + 2 * phrase_len];
            if first == second {
                found_len = phrase_len;
                break;
            }
        }
        if found_len >= 3 {
            remove_spans.push((i, found_len));
            i += found_len; // skip past the removed span to the kept copy
        } else {
            i += 1;
        }
    }

    if remove_spans.is_empty() {
        return words;
    }

    // Build a removal set
    let mut remove_set = vec![false; n];
    for (start, len) in &remove_spans {
        for item in remove_set.iter_mut().skip(*start).take(*len) {
            *item = true;
        }
    }

    let removed_count: usize = remove_set.iter().filter(|&&r| r).count();
    if removed_count > 0 {
        debug!(
            "dedup_repeated_phrases: removed {} words across {} repeated phrase(s)",
            removed_count,
            remove_spans.len()
        );
    }

    words
        .into_iter()
        .enumerate()
        .filter(|(idx, _)| !remove_set[*idx])
        .map(|(_, w)| w)
        .collect()
}

/// Strip non-speech segments and enforce the canonical invariants (monotonic,
/// non-overlapping, non-zero-duration). Words flagged `is_non_speech` are
/// removed here so they never appear in the returned result.
///
/// No content-altering transforms (dedup, stutter collapse) happen here —
/// the transcription pipeline's job is to faithfully reproduce what was
/// spoken. Content cleanup belongs in the user-initiated "Clean Up" flow.
fn finalize_words(mut words: Vec<CanonicalWord>, audio_info: AudioInfo) -> Vec<CanonicalWord> {
    // Strip non-speech tokens. We never emit them.
    words.retain(|w| !w.is_non_speech);

    if words.is_empty() {
        return words;
    }

    // Clamp to audio duration and enforce monotonic / non-zero-duration.
    let max_us = audio_info.duration_us.max(0);
    let mut cursor: i64 = 0;
    let mut out: Vec<CanonicalWord> = Vec::with_capacity(words.len());

    for mut w in words {
        // Clamp into [0, max_us] if we have a known duration; otherwise
        // trust the engine's times (max_us == 0 means "unknown").
        if max_us > 0 {
            w.start_us = w.start_us.clamp(0, max_us);
            w.end_us = w.end_us.clamp(0, max_us);
        }
        if w.start_us < cursor {
            w.start_us = cursor;
        }
        if w.end_us <= w.start_us {
            // Grant a 1 ms floor when audio budget allows; otherwise drop.
            let floor = w.start_us + 1_000;
            if max_us == 0 || floor <= max_us {
                w.end_us = floor;
            } else {
                continue;
            }
        }
        cursor = w.end_us;
        out.push(w);
    }
    out
}

/// Build `CanonicalWord`s from raw segments using char-proportional split.
/// Shared by every non-word-level adapter.
pub(super) fn words_from_segments_proportional(
    segments: &[TranscriptionSegment],
    audio_info: AudioInfo,
) -> Vec<CanonicalWord> {
    let mut words: Vec<CanonicalWord> = Vec::new();
    for seg in segments {
        let text = seg.text.trim();
        if text.is_empty() {
            continue;
        }
        if is_non_speech_token(text) {
            debug!("adapter: stripping non-speech segment: {:?}", text);
            continue;
        }
        let start_us = seconds_to_us(seg.start);
        let end_us = seconds_to_us(seg.end);
        for (word_text, ws, we) in split_segment_by_chars(text, start_us, end_us) {
            if is_non_speech_token(&word_text) {
                continue;
            }
            words.push(CanonicalWord {
                text: word_text,
                start_us: ws,
                end_us: we,
                confidence: -1.0,
                speaker_id: -1,
                is_non_speech: false,
            });
        }
    }
    finalize_words(words, audio_info)
}

/// Build `CanonicalWord`s from per-word segments, preserving native times.
/// Used when the adapter detects one-word-per-segment output (Parakeet with
/// `TimestampGranularity::Word`).
pub(super) fn words_from_segments_native(
    segments: &[TranscriptionSegment],
    audio_info: AudioInfo,
) -> Vec<CanonicalWord> {
    let mut words: Vec<CanonicalWord> = Vec::with_capacity(segments.len());
    for seg in segments {
        let text = seg.text.trim();
        if text.is_empty() || is_non_speech_token(text) {
            continue;
        }
        words.push(CanonicalWord {
            text: text.to_string(),
            start_us: seconds_to_us(seg.start),
            end_us: seconds_to_us(seg.end),
            confidence: -1.0,
            speaker_id: -1,
            is_non_speech: false,
        });
    }
    finalize_words(words, audio_info)
}

/// Heuristic used to decide between native per-word times and the char-split
/// fallback: if ≥80 % of segments contain exactly one whitespace-separated
/// token, treat segments as word-level.
///
/// **Rationale for 80 %:** ASR engines emitting `TimestampGranularity::Word`
/// occasionally produce a multi-word segment for compound terms or misaligned
/// output. Requiring 100 % would discard good native times due to a handful
/// of outliers; 80 % tolerates up to 20 % multi-word segments while still
/// being a strong signal that the engine intended word-level granularity.
/// At the boundary (e.g. 79 %), the proportional-split fallback runs and DP
/// forced alignment downstream still produces usable timestamps — just not
/// as authoritative.
pub(super) fn segments_are_word_level(segments: &[TranscriptionSegment]) -> bool {
    if segments.is_empty() {
        return false;
    }
    let single = segments
        .iter()
        .filter(|s| s.text.split_whitespace().count() == 1)
        .count();
    (single as f64) / (segments.len() as f64) >= 0.8
}

/// Sanitize raw ASR segments: clamp overlaps, drop fully-contained
/// hallucinated segments, and strip non-speech segments. This runs in the
/// adapter layer so downstream consumers (DP forced alignment, word builder)
/// receive clean, monotonic segments.
///
/// Returns the sanitized segment list (may be shorter than input).
pub(super) fn sanitize_segments(segments: &[TranscriptionSegment]) -> Vec<TranscriptionSegment> {
    if segments.is_empty() {
        return Vec::new();
    }

    let mut result: Vec<TranscriptionSegment> = Vec::with_capacity(segments.len());
    let mut overlap_count = 0usize;
    let mut dropped_count = 0usize;
    let mut nonspeech_count = 0usize;

    for seg in segments {
        // Strip non-speech segments early
        let text = seg.text.trim();
        if text.is_empty() || is_non_speech_token(text) {
            nonspeech_count += 1;
            continue;
        }

        let mut clamped = seg.clone();

        // Clamp start to previous segment's end to enforce monotonicity
        if let Some(prev) = result.last() {
            if clamped.start < prev.end {
                overlap_count += 1;
                clamped.start = prev.end;
            }
        }

        // Drop segments that became zero/negative duration after clamping
        if clamped.end <= clamped.start {
            dropped_count += 1;
            continue;
        }

        result.push(clamped);
    }

    if overlap_count > 0 || dropped_count > 0 || nonspeech_count > 0 {
        use log::info;
        info!(
            "sanitize_segments: {} segments → {} (overlaps_clamped={}, dropped={}, nonspeech={})",
            segments.len(),
            result.len(),
            overlap_count,
            dropped_count,
            nonspeech_count,
        );
    }

    result
}

/// Build + validate a `NormalizedTranscriptionResult` from the parts each
/// adapter produces. Centralizing this removes 9 copies of the same struct
/// literal + `validate()?` pattern and is the single place that carries
/// `raw.text` / `raw.segments` onto the normalized result.
pub(super) fn make_normalized(
    raw: TranscriptionResult,
    words: Vec<CanonicalWord>,
    word_timestamps_authoritative: bool,
) -> Result<NormalizedTranscriptionResult> {
    let result = NormalizedTranscriptionResult {
        words,
        text: raw.text,
        segments: raw.segments,
        language: "und".to_string(),
        word_timestamps_authoritative,
    };
    result.validate()?;
    Ok(result)
}

/// Shared adaptation path for engines that may or may not produce word-level
/// segments. Auto-detects the segment granularity via
/// [`segments_are_word_level`] and picks the appropriate conversion:
///
/// - **Word-level:** preserves native per-word times (`authoritative: true`).
/// - **Phrase-level:** char-proportional split into the segment span
///   (`authoritative: false`); DP forced alignment downstream refines the
///   result before it reaches the editor.
///
/// Every production adapter except Whisper delegates to this function.
/// Whisper is excluded because its segments carry authoritative segment-level
/// times, but its per-word breakdown is always char-proportional — routing
/// through DP forced alignment produces strictly better timestamps than
/// declaring the proportional seeds authoritative.
pub(super) fn adapt_with_auto_detection(
    engine_name: &str,
    raw: TranscriptionResult,
    audio_info: AudioInfo,
) -> Result<NormalizedTranscriptionResult> {
    use super::adapter::segments_of;
    use log::info;

    // Sanitize segments before word extraction: clamp overlaps, drop
    // hallucinated segments, strip non-speech. This gives downstream DP
    // forced alignment a clean, monotonic timeline.
    let clean_segs = sanitize_segments(segments_of(&raw));
    let word_level = segments_are_word_level(&clean_segs);

    if word_level {
        info!(
            "{}: detected word-level segments ({} segs), using native timestamps (authoritative)",
            engine_name,
            clean_segs.len()
        );
    } else {
        info!(
            "{}: segments are phrase-level ({} segs), using char-proportional split",
            engine_name,
            clean_segs.len()
        );
    }

    let words = if word_level {
        words_from_segments_native(&clean_segs, audio_info)
    } else {
        words_from_segments_proportional(&clean_segs, audio_info)
    };

    // Store the sanitized segments in the result so build_words_from_segments
    // receives pre-cleaned data.
    let mut normalized_raw = raw;
    normalized_raw.segments = Some(clean_segs);
    make_normalized(normalized_raw, words, word_level)
}
