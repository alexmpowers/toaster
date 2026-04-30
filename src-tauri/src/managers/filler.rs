/// Filler word and pause detection for transcript editing.
///
/// Analyzes a word list to identify filler words (e.g., "um", "uh", "like")
/// and long pauses between words. Results can drive bulk-delete suggestions
/// in the editor UI.
use crate::managers::editor::Word;

// Single source of truth for the English filler list. Frontend reads this via the
// `filler` Tauri commands — do not reintroduce a duplicated frontend constant.
pub const DEFAULT_FILLERS: &[&str] = &[
    "um",
    "uh",
    "uh huh",
    "hmm",
    "mm",
    "mhm",
    "er",
    "ah",
    "like",
    "you know",
    "I mean",
    "basically",
    "actually",
    "literally",
    "so",
    "right",
    "kind of",
    "sort of",
];

/// Minimum gap between words (in microseconds) to be considered a pause.
pub const DEFAULT_PAUSE_THRESHOLD_US: i64 = 1_500_000; // 1.5 seconds

// ---------------------------------------------------------------------
// R-004 — acoustic classification of long filler/pause gaps
// ---------------------------------------------------------------------
//
// See `features/reintroduce-silero-vad/PRD.md` §R-004 and BLUEPRINT §AD-6.
// Pure metadata: no existing caller's behaviour changes (AC-004-c grep
// gate). When a VAD probability curve is available, the editor may
// consume `classify_gaps` output to annotate pause candidates in the
// UI and drive smarter auto-silence decisions. Without a curve the
// function returns `Unknown` for every gap — the default path stays
// untouched.

/// Acoustic classification assigned to a detected pause gap.
///
/// The enum is additive metadata only. No default filler/pause behaviour
/// is driven by it — the editor may surface it as a label in the UI or
/// feed it into heuristics for `auto_silence_pauses`, but legacy
/// consumers that ignore the classification see no behaviour change.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[allow(dead_code)] // consumed by Phase 2 editor metadata surface.
pub enum GapClassification {
    /// Mean P(speech) stayed below 0.2 across the gap — genuine silence.
    TrueSilence,
    /// Mean P(speech) between 0.2 and the speech threshold — background
    /// music, breath noise, or clapping that the ASR correctly
    /// classified as non-speech.
    NonSpeechAcoustic,
    /// Mean P(speech) ≥ the speech threshold — the ASR likely dropped
    /// real speech (a stutter, a very quiet "um", or a model miss).
    MissedSpeech,
    /// No VAD curve provided, or the curve does not cover the gap.
    /// Legacy/default path.
    Unknown,
}

/// Threshold used by [`classify_gap`] below which a gap is treated as
/// [`GapClassification::TrueSilence`] (rather than
/// [`GapClassification::NonSpeechAcoustic`]).
#[allow(dead_code)]
pub const GAP_SILENCE_THRESHOLD: f32 = 0.2;

/// Threshold used by [`classify_gap`] above which a gap is treated as
/// [`GapClassification::MissedSpeech`]. Matches the Silero default
/// speech threshold so classifications are consistent with the
/// prefilter pass.
#[allow(dead_code)]
pub const GAP_SPEECH_THRESHOLD: f32 = 0.5;

/// Classify a single gap using a pre-computed VAD probability curve
/// sampled at 30 ms cadence (see
/// [`crate::managers::splice::boundaries::VAD_FRAME_MS`]). Returns
/// [`GapClassification::Unknown`] when `vad_curve` is empty or does not
/// cover the gap interval — callers never error on missing data (AD-8).
#[allow(dead_code)] // consumed by editor metadata path once wired.
pub fn classify_gap(gap_start_us: i64, gap_end_us: i64, vad_curve: &[f32]) -> GapClassification {
    if vad_curve.is_empty() || gap_end_us <= gap_start_us {
        return GapClassification::Unknown;
    }
    let frame_us: i64 = 30_000; // 30 ms, matches VAD cadence.
    let lo = (gap_start_us / frame_us).max(0) as usize;
    let hi_inclusive = ((gap_end_us - 1) / frame_us).max(0) as usize;
    let hi = hi_inclusive.min(vad_curve.len().saturating_sub(1));
    if hi < lo {
        return GapClassification::Unknown;
    }
    let slice = &vad_curve[lo..=hi];
    if slice.is_empty() {
        return GapClassification::Unknown;
    }
    let mean: f32 = slice.iter().copied().sum::<f32>() / slice.len() as f32;
    if mean < GAP_SILENCE_THRESHOLD {
        GapClassification::TrueSilence
    } else if mean < GAP_SPEECH_THRESHOLD {
        GapClassification::NonSpeechAcoustic
    } else {
        GapClassification::MissedSpeech
    }
}

/// Classify every pause returned by [`detect_pauses`] using the supplied
/// VAD curve. Returns `(gap_after_word_index, gap_duration_us,
/// classification)` triples in the same order. Empty curve ⇒ every
/// classification is [`GapClassification::Unknown`], so the function is
/// safe to call unconditionally from Phase 2 editor code.
#[allow(dead_code)] // consumed by editor metadata path once wired.
pub fn classify_pauses(
    pauses: &[(usize, i64)],
    words: &[Word],
    vad_curve: &[f32],
) -> Vec<(usize, i64, GapClassification)> {
    pauses
        .iter()
        .map(|&(i, dur)| {
            let start = words.get(i).map(|w| w.end_us).unwrap_or(0);
            let end = words.get(i + 1).map(|w| w.start_us).unwrap_or(start + dur);
            (i, dur, classify_gap(start, end, vad_curve))
        })
        .collect()
}

/// Configuration for filler/pause detection.
pub struct FillerConfig {
    /// Words to treat as fillers (matched case-insensitively, punctuation stripped).
    pub filler_words: Vec<String>,
    /// Gap in microseconds that qualifies as a "long pause".
    pub pause_threshold_us: i64,
    /// If true, detected fillers are automatically marked deleted.
    #[allow(dead_code)]
    pub auto_delete_fillers: bool,
    /// If true, detected pauses are automatically marked silenced.
    #[allow(dead_code)]
    pub auto_silence_pauses: bool,
}

impl Default for FillerConfig {
    fn default() -> Self {
        Self {
            filler_words: DEFAULT_FILLERS.iter().map(|s| s.to_string()).collect(),
            pause_threshold_us: DEFAULT_PAUSE_THRESHOLD_US,
            auto_delete_fillers: false,
            auto_silence_pauses: false,
        }
    }
}

/// Results from analyzing a word list for fillers and pauses.
#[cfg(test)]
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AnalysisResult {
    /// Indices of words identified as fillers.
    pub filler_indices: Vec<usize>,
    /// `(gap_after_word_index, gap_duration_us)` for each detected pause.
    pub pauses: Vec<(usize, i64)>,
    /// Indices of the second word in each adjacent duplicate pair.
    pub duplicate_indices: Vec<usize>,
}

/// Strip leading/trailing punctuation from a word, returning a lowercase copy.
fn normalize(word: &str) -> String {
    word.trim_matches(|c: char| c.is_ascii_punctuation())
        .to_lowercase()
}

/// Normalize filler word for fuzzy matching.
/// "umm" → "um", "uhhh" → "uh", "hmmm" → "hm", "ummmmm" → "um"
fn normalize_filler(word: &str) -> String {
    let lower = word.to_lowercase();
    let chars: Vec<char> = lower.chars().collect();
    if chars.len() < 2 {
        return lower;
    }
    // Collapse trailing runs of the same character to a single instance
    let last_char = *chars.last().unwrap();
    let mut end = chars.len();
    while end > 1 && chars[end - 2] == last_char {
        end -= 1;
    }
    chars[..end].iter().collect()
}

/// Detect adjacent duplicate words (case-insensitive).
/// Returns indices of the SECOND word in each duplicate pair.
/// "the the best best part" → returns indices of second "the" and second "best"
pub fn detect_duplicates(words: &[Word]) -> Vec<usize> {
    let mut duplicates = Vec::new();
    let mut i = 0;
    while i < words.len() {
        if words[i].deleted {
            i += 1;
            continue;
        }
        // Look for the next non-deleted word
        let mut j = i + 1;
        while j < words.len() && words[j].deleted {
            j += 1;
        }
        if j < words.len() && words[i].text.to_lowercase() == words[j].text.to_lowercase() {
            duplicates.push(j);
            // Continue from after the duplicate to catch triples: "the the the" → [1, 2]
            i = j;
        } else {
            i = j;
        }
    }
    duplicates
}

/// Detect filler words in the word list. Returns indices of filler words.
///
/// Multi-word fillers (e.g., "you know") are detected by joining consecutive
/// non-deleted words and checking for a match. When a multi-word filler is
/// found, all constituent word indices are included in the result.
pub fn detect_fillers(words: &[Word], config: &FillerConfig) -> Vec<usize> {
    let mut indices: Vec<usize> = Vec::new();

    // Pre-compute the maximum token count among filler phrases.
    let max_filler_tokens = config
        .filler_words
        .iter()
        .map(|f| f.split_whitespace().count())
        .max()
        .unwrap_or(0);

    // Build fast-lookup sets keyed by token count for O(1) membership
    // checks. `exact_by_len[k]` holds exact lowercase k-token phrases.
    // `fuzzy_single` holds fuzzy-normalized single-word fillers.
    // Previously this was a single Vec scanned linearly for every window
    // at every word — the inner loop is hot on long transcripts.
    use std::collections::HashSet;
    let mut exact_by_len: Vec<HashSet<String>> = vec![HashSet::new(); max_filler_tokens + 1];
    let mut fuzzy_single: HashSet<String> = HashSet::new();
    for f in &config.filler_words {
        let lower = f.to_lowercase();
        let len = f.split_whitespace().count();
        if len == 0 || len > max_filler_tokens {
            continue;
        }
        if len == 1 {
            fuzzy_single.insert(normalize_filler(&lower));
        }
        exact_by_len[len].insert(lower);
    }

    // Collect indices of non-deleted words so we can walk them in order.
    let active: Vec<usize> = words
        .iter()
        .enumerate()
        .filter(|(_, w)| !w.deleted)
        .map(|(i, _)| i)
        .collect();

    let mut skip_until = 0usize; // active-array index to skip to (for multi-word matches)

    for (ai, &wi) in active.iter().enumerate() {
        if ai < skip_until {
            continue;
        }

        // Try longest filler phrases first so "you know" beats "you".
        let mut matched = false;
        for window in (1..=max_filler_tokens).rev() {
            if ai + window > active.len() {
                continue;
            }
            let phrase: String = (0..window)
                .map(|offset| normalize(&words[active[ai + offset]].text))
                .collect::<Vec<_>>()
                .join(" ");

            let hit = exact_by_len[window].contains(&phrase)
                || (window == 1 && fuzzy_single.contains(&normalize_filler(&phrase)));
            if hit {
                for offset in 0..window {
                    indices.push(active[ai + offset]);
                }
                skip_until = ai + window;
                matched = true;
                break;
            }
        }

        if !matched {
            // Single-word check (already covered by window==1 above, but kept
            // explicit for clarity).
            let norm = normalize(&words[wi].text);
            let fuzzy_norm = normalize_filler(&norm);
            if exact_by_len
                .get(1)
                .map(|s| s.contains(&norm))
                .unwrap_or(false)
                || fuzzy_single.contains(&fuzzy_norm)
            {
                indices.push(wi);
            }
        }
    }

    indices.sort_unstable();
    indices.dedup();
    indices
}

/// Detect long pauses between words. Returns `(gap_after_word_index, gap_duration_us)`.
///
/// Only considers non-deleted words when measuring gaps.
pub fn detect_pauses(words: &[Word], config: &FillerConfig) -> Vec<(usize, i64)> {
    let active: Vec<usize> = words
        .iter()
        .enumerate()
        .filter(|(_, w)| !w.deleted)
        .map(|(i, _)| i)
        .collect();

    let mut pauses = Vec::new();
    for pair in active.windows(2) {
        let (i, j) = (pair[0], pair[1]);
        let gap = words[j].start_us - words[i].end_us;
        if gap >= config.pause_threshold_us {
            pauses.push((i, gap));
        }
    }
    pauses
}

/// Default maximum gap after trimming (300 ms).
pub const DEFAULT_MAX_GAP_US: i64 = 300_000;

/// Pause threshold for the "Remove silence" command (750 ms).
/// Shorter than `DEFAULT_PAUSE_THRESHOLD_US` because the user-facing
/// action targets natural breathing/thinking pauses that still feel like
/// dead air, not only the very long ones the cleanup pipeline catches.
pub const REMOVE_SILENCE_THRESHOLD_US: i64 = 750_000;

/// Collapse target for "Remove silence" — hard cut (0 ms).
pub const REMOVE_SILENCE_MAX_GAP_US: i64 = 0;

/// Sentinel marker text for synthetic "silence" Words inserted by
/// `trim_pauses`/`tighten_gaps`. Empty because these are not real
/// words — they exist purely to mark a source-time region as deleted
/// so [`crate::managers::editor::EditorState::get_keep_segments`]
/// excludes it from the timeline. The frontend transcript view filters
/// them out so they never render with strikethrough.
pub const SILENCE_SENTINEL_TEXT: &str = "";

/// True iff `word` is a synthetic silence sentinel (deleted with empty
/// text). Real user-typed empty deletions are not produced anywhere in
/// the codebase, so this predicate is safe to treat as authoritative.
pub fn is_silence_sentinel(word: &Word) -> bool {
    word.deleted && word.text.is_empty()
}

pub(crate) fn make_silence_sentinel(start_us: i64, end_us: i64) -> Word {
    Word {
        text: SILENCE_SENTINEL_TEXT.to_string(),
        start_us,
        end_us,
        deleted: true,
        silenced: false,
        confidence: -1.0,
        speaker_id: -1,
    }
}

/// Count how many gaps `trim_pauses` would trim for the given thresholds,
/// without mutating anything. Shares the gap-walk predicate with
/// `trim_pauses` so callers that need a pre-flight (e.g. to skip
/// push_undo_snapshot on no-op) cannot drift from the real behavior.
///
/// Gaps that already contain a **silence sentinel** (a previously
/// inserted deleted-empty Word) are *skipped* — they have already been
/// trimmed and re-trimming them would only stack redundant sentinels
/// and break idempotence. Gaps that contain only **user-deleted words**
/// (e.g. excised fillers) are NOT skipped: the dead air around the
/// excised content is still trimmable silence from the user's
/// perspective, and the user explicitly clicking "Remove silence"
/// after "Remove fillers" expects to see it counted.
pub fn count_trimmable_pauses(words: &[Word], pause_threshold_us: i64, max_gap_us: i64) -> usize {
    if words.len() < 2 {
        return 0;
    }
    let mut count = 0usize;
    let mut prev_non_deleted_end: Option<i64> = None;
    let mut sentinel_between = false;
    for word in words.iter() {
        if word.deleted {
            if is_silence_sentinel(word) {
                sentinel_between = true;
            }
            continue;
        }
        if let Some(pe) = prev_non_deleted_end {
            if !sentinel_between {
                let gap = word.start_us.saturating_sub(pe);
                if gap >= pause_threshold_us && gap.saturating_sub(max_gap_us) > 0 {
                    count += 1;
                }
            }
        }
        prev_non_deleted_end = Some(word.end_us);
        sentinel_between = false;
    }
    count
}

/// Trim long pauses by inserting deleted "silence sentinel" words that
/// cover the excess of every gap exceeding `pause_threshold_us`.
///
/// Real word timestamps are **never mutated** — `EditorState` words are
/// always source-timeline microseconds, and the export/preview
/// pipelines depend on that invariant. Each qualifying gap of length
/// `g` between two consecutive non-deleted words gets a deleted
/// sentinel inserted that covers `[prev.end_us + max_gap_us,
/// next.start_us]`. `EditorState::get_keep_segments` already excludes
/// deleted ranges, so the seam closes naturally with no source-time
/// drift, no overlap, and no further work in downstream consumers.
///
/// Gaps that already contain a **silence sentinel** (a previously
/// inserted deleted-empty Word) are skipped — re-trimming them would
/// only add redundant sentinels and break idempotence. Gaps that
/// contain only **user-deleted words** (e.g. excised fillers) are
/// still trimmed: the surrounding source-time silence is what the
/// user wants gone, and the new sentinel may overlap an existing
/// user-deleted word in source-time without ill effect (overlapping
/// deleted ranges collapse correctly inside `get_keep_segments`).
///
/// Returns the number of sentinels inserted.
pub fn trim_pauses(words: &mut Vec<Word>, pause_threshold_us: i64, max_gap_us: i64) -> usize {
    if words.len() < 2 {
        return 0;
    }

    // First pass: collect (insert-at-index, sentinel) tuples.
    // We never mutate during iteration so the indices recorded here are
    // valid against the original `words` snapshot.
    let mut insertions: Vec<(usize, Word)> = Vec::new();

    let mut prev_non_deleted_end: Option<i64> = None;
    let mut sentinel_between = false;

    for (i, word) in words.iter().enumerate() {
        if word.deleted {
            if is_silence_sentinel(word) {
                sentinel_between = true;
            }
            continue;
        }
        if let Some(pe) = prev_non_deleted_end {
            if !sentinel_between {
                let gap = word.start_us.saturating_sub(pe);
                if gap >= pause_threshold_us {
                    let excess = gap.saturating_sub(max_gap_us);
                    if excess > 0 {
                        let sentinel_start = pe.saturating_add(max_gap_us);
                        // Defensive: clamp `sentinel_start` so the
                        // sentinel never has end <= start. The earlier
                        // checks guarantee this in practice but a stray
                        // call with `max_gap_us > gap` would otherwise
                        // produce a degenerate sentinel.
                        let sentinel_start = sentinel_start.min(word.start_us);
                        insertions.push((i, make_silence_sentinel(sentinel_start, word.start_us)));
                    }
                }
            }
        }
        prev_non_deleted_end = Some(word.end_us);
        sentinel_between = false;
    }

    let count = insertions.len();
    if count == 0 {
        return 0;
    }

    // Apply in reverse-index order so each insertion does not shift the
    // indices of insertions still to come.
    for (idx, sentinel) in insertions.into_iter().rev() {
        words.insert(idx, sentinel);
    }
    count
}

/// Default target gap duration after tightening (250ms).
pub const DEFAULT_TIGHTEN_TARGET_US: i64 = 250_000;

/// Tighten all inter-word gaps exceeding `target_gap_us` by inserting
/// deleted silence sentinels covering the excess.
///
/// Same source-time-preserving mechanism as [`trim_pauses`]: real word
/// timestamps are never mutated. Differs from `trim_pauses` only in
/// applying to *every* gap above `target_gap_us`, not just the very
/// long ones (no separate threshold).
///
/// Returns the number of sentinels inserted.
pub fn tighten_gaps(words: &mut Vec<Word>, target_gap_us: i64) -> usize {
    if words.len() < 2 || target_gap_us <= 0 {
        return 0;
    }

    let mut insertions: Vec<(usize, Word)> = Vec::new();
    let mut prev_non_deleted_end: Option<i64> = None;
    let mut sentinel_between = false;

    for (i, word) in words.iter().enumerate() {
        if word.deleted {
            if is_silence_sentinel(word) {
                sentinel_between = true;
            }
            continue;
        }
        if let Some(pe) = prev_non_deleted_end {
            if !sentinel_between {
                let gap = word.start_us.saturating_sub(pe);
                if gap > target_gap_us {
                    let sentinel_start = pe.saturating_add(target_gap_us);
                    let sentinel_start = sentinel_start.min(word.start_us);
                    insertions.push((i, make_silence_sentinel(sentinel_start, word.start_us)));
                }
            }
        }
        prev_non_deleted_end = Some(word.end_us);
        sentinel_between = false;
    }

    let count = insertions.len();
    if count == 0 {
        return 0;
    }
    for (idx, sentinel) in insertions.into_iter().rev() {
        words.insert(idx, sentinel);
    }
    count
}

/// Analyze words and return fillers, pauses, and duplicates.
#[cfg(test)]
pub fn analyze(words: &[Word], config: &FillerConfig) -> AnalysisResult {
    AnalysisResult {
        filler_indices: detect_fillers(words, config),
        pauses: detect_pauses(words, config),
        duplicate_indices: detect_duplicates(words),
    }
}

#[cfg(test)]
#[path = "filler_tests.rs"]
mod tests;

#[cfg(test)]
#[path = "filler_cleanup_cascade_tests.rs"]
mod cleanup_cascade_tests;

#[cfg(test)]
#[path = "filler_classify_tests.rs"]
mod classify_tests;
