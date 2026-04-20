use tauri::State;

use crate::commands::editor::EditorStore;
use crate::managers::filler::{self, FillerConfig};

/// Detect filler words, duplicates, and long pauses in the current transcript.
/// Runs iterative simulation: after marking fillers as deleted, re-scans for
/// cascading duplicates (e.g., "the um the" → "the the" after filler removal).
/// This ensures the reported counts match what `cleanup_all` would actually remove.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, specta::Type)]
pub struct FillerAnalysis {
    pub filler_indices: Vec<usize>,
    /// Each pause: (word_index_before_gap, gap_duration_us)
    pub pauses: Vec<PauseInfo>,
    pub filler_count: usize,
    pub pause_count: usize,
    /// Indices of the second word in each adjacent duplicate pair.
    pub duplicate_indices: Vec<usize>,
    pub duplicate_count: usize,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, specta::Type)]
pub struct PauseInfo {
    pub after_word_index: usize,
    pub gap_duration_us: i64,
}

#[tauri::command]
#[specta::specta]
pub fn analyze_fillers(
    app: tauri::AppHandle,
    store: State<EditorStore>,
    min_pause_us: Option<i64>,
) -> Result<FillerAnalysis, String> {
    let state = crate::lock_recovery::try_lock(store.0.lock()).map_err(|e| e.to_string())?;
    let mut words = state.get_words().to_vec();

    let settings = crate::settings::get_settings(&app);
    let filler_list = settings.custom_filler_words.clone().unwrap_or_default();

    let mut config = FillerConfig {
        filler_words: filler_list,
        ..Default::default()
    };
    if let Some(threshold) = min_pause_us {
        config.pause_threshold_us = threshold;
    }

    let mut all_filler_indices: Vec<usize> = Vec::new();
    let mut all_duplicate_indices: Vec<usize> = Vec::new();
    const MAX_PASSES: usize = 10;

    for _ in 0..MAX_PASSES {
        let mut changed = false;

        let filler_indices = filler::detect_fillers(&words, &config);
        if !filler_indices.is_empty() {
            for &idx in &filler_indices {
                if idx < words.len() {
                    all_filler_indices.push(idx);
                    words[idx].deleted = true;
                }
            }
            changed = true;
        }

        let dup_indices = filler::detect_duplicates(&words);
        if !dup_indices.is_empty() {
            for &idx in &dup_indices {
                if idx < words.len() {
                    all_duplicate_indices.push(idx);
                    words[idx].deleted = true;
                }
            }
            changed = true;
        }

        if !changed {
            break;
        }
    }

    // Detect pauses on the simulated cleaned-up word list
    let pauses = filler::detect_pauses(&words, &config);
    let pause_infos: Vec<PauseInfo> = pauses
        .into_iter()
        .map(|(idx, dur)| PauseInfo {
            after_word_index: idx,
            gap_duration_us: dur,
        })
        .collect();

    Ok(FillerAnalysis {
        filler_count: all_filler_indices.len(),
        pause_count: pause_infos.len(),
        duplicate_count: all_duplicate_indices.len(),
        filler_indices: all_filler_indices,
        pauses: pause_infos,
        duplicate_indices: all_duplicate_indices,
    })
}

/// Auto-delete all detected filler words in the transcript.
#[tauri::command]
#[specta::specta]
pub fn delete_fillers(app: tauri::AppHandle, store: State<EditorStore>) -> Result<usize, String> {
    let settings = crate::settings::get_settings(&app);
    let filler_list = settings.custom_filler_words.clone().unwrap_or_default();

    let config = FillerConfig {
        filler_words: filler_list,
        ..Default::default()
    };

    let mut state = crate::lock_recovery::try_lock(store.0.lock()).map_err(|e| e.to_string())?;
    let indices = filler::detect_fillers(state.get_words(), &config);
    let count = indices.len();

    if count == 0 {
        return Ok(0);
    }

    state.push_undo_snapshot();
    let words = state.get_words_mut();
    for &idx in &indices {
        if idx < words.len() {
            words[idx].deleted = true;
        }
    }
    state.bump_revision();

    Ok(count)
}

/// Delete all detected adjacent duplicate words in the transcript.
#[tauri::command]
#[specta::specta]
pub fn delete_duplicates(store: State<EditorStore>) -> Result<usize, String> {
    let mut state = crate::lock_recovery::try_lock(store.0.lock()).map_err(|e| e.to_string())?;
    let duplicates = filler::detect_duplicates(state.get_words());
    let count = duplicates.len();

    if count == 0 {
        return Ok(0);
    }

    state.push_undo_snapshot();
    let words = state.get_words_mut();
    for &idx in &duplicates {
        if idx < words.len() {
            words[idx].deleted = true;
        }
    }
    state.bump_revision();

    Ok(count)
}

/// Silence all detected long pauses by marking adjacent words as silenced.
#[tauri::command]
#[specta::specta]
pub fn silence_pauses(
    store: State<EditorStore>,
    min_pause_us: Option<i64>,
) -> Result<usize, String> {
    let mut config = FillerConfig::default();
    if let Some(threshold) = min_pause_us {
        config.pause_threshold_us = threshold;
    }

    let mut state = crate::lock_recovery::try_lock(store.0.lock()).map_err(|e| e.to_string())?;
    let pauses = filler::detect_pauses(state.get_words(), &config);
    let count = pauses.len();

    if count == 0 {
        return Ok(0);
    }

    // Silence the word after each pause gap to mark the dead-air region
    for (after_word_idx, _) in &pauses {
        let next_idx = after_word_idx + 1;
        if next_idx < state.get_words().len() && !state.get_words()[next_idx].silenced {
            state.silence_word(next_idx);
        }
    }

    Ok(count)
}

/// Trim long pauses by reducing dead-air gaps to a maximum duration.
///
/// Unlike `silence_pauses` (which marks words as silenced), this command
/// shifts timestamps so that gaps exceeding the threshold are capped at
/// 300 ms, effectively removing dead air from the timeline.
#[tauri::command]
#[specta::specta]
pub fn trim_pauses(
    store: State<EditorStore>,
    min_pause_us: Option<i64>,
    max_gap_us: Option<i64>,
) -> Result<usize, String> {
    let threshold = min_pause_us.unwrap_or(filler::DEFAULT_PAUSE_THRESHOLD_US);
    let max_gap = max_gap_us.unwrap_or(filler::DEFAULT_MAX_GAP_US);

    let mut state = crate::lock_recovery::try_lock(store.0.lock()).map_err(|e| e.to_string())?;
    state.push_undo_snapshot();

    let words = state.get_words_mut();
    let count = filler::trim_pauses(words, threshold, max_gap);

    if count > 0 {
        state.bump_revision();
    }

    Ok(count)
}

/// Tighten all inter-word gaps to a maximum target duration.
/// Shortens ALL gaps exceeding the target — creating a tighter pace.
#[tauri::command]
#[specta::specta]
pub fn tighten_gaps(
    store: State<EditorStore>,
    target_gap_us: Option<i64>,
) -> Result<usize, String> {
    let target = target_gap_us.unwrap_or(filler::DEFAULT_TIGHTEN_TARGET_US);
    let mut state = crate::lock_recovery::try_lock(store.0.lock()).map_err(|e| e.to_string())?;
    state.push_undo_snapshot();
    let words = state.get_words_mut();
    let count = filler::tighten_gaps(words, target);
    if count > 0 {
        state.bump_revision();
    }
    Ok(count)
}

/// Remove silence: collapse inter-word gaps ≥ 750 ms to 0 ms.
///
/// Word-gap based (no VAD / RMS) — reuses whisper's existing per-word
/// timings. Delegates the actual shift to `filler::trim_pauses` so
/// preview and export stay on the single source of truth (the same
/// time-map shift the trim-pauses path already uses).
///
/// Returns the number of gaps collapsed. When `0`, the call is a no-op
/// (no undo snapshot, no revision bump) so the UI can surface a subtle
/// "no dead-air found" notice without polluting the undo stack.
#[tauri::command]
#[specta::specta]
pub fn remove_silence(store: State<EditorStore>) -> Result<usize, String> {
    let mut state = crate::lock_recovery::try_lock(store.0.lock()).map_err(|e| e.to_string())?;

    // Dry-run detection first so we can skip the undo snapshot on no-op
    // runs (AC-001-f: no destructive edit recorded when nothing to do).
    // Same lock is held through the mutating call below — no TOCTOU.
    let prelim_count = filler::count_trimmable_pauses(
        state.get_words(),
        filler::REMOVE_SILENCE_THRESHOLD_US,
        filler::REMOVE_SILENCE_MAX_GAP_US,
    );

    if prelim_count == 0 {
        return Ok(0);
    }

    state.push_undo_snapshot();
    let words = state.get_words_mut();
    let count = filler::trim_pauses(
        words,
        filler::REMOVE_SILENCE_THRESHOLD_US,
        filler::REMOVE_SILENCE_MAX_GAP_US,
    );
    if count > 0 {
        state.bump_revision();
    }
    Ok(count)
}

/// Combined iterative cleanup: delete fillers, then delete cascading
/// duplicates, then trim pauses — all in a single undo snapshot.
///
/// After deleting fillers, new duplicates may emerge (e.g., "the um the"
/// becomes "the the"). This command loops until no more fillers or
/// duplicates are found, then trims pauses.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, specta::Type)]
pub struct CleanupResult {
    pub fillers_removed: usize,
    pub duplicates_removed: usize,
    pub pauses_trimmed: usize,
    pub gaps_tightened: usize,
    pub passes: usize,
}

#[tauri::command]
#[specta::specta]
pub fn cleanup_all(
    app: tauri::AppHandle,
    store: State<EditorStore>,
    media_store: State<'_, crate::managers::media::MediaStore>,
    min_pause_us: Option<i64>,
    max_gap_us: Option<i64>,
) -> Result<CleanupResult, String> {
    let settings = crate::settings::get_settings(&app);
    let filler_list = settings.custom_filler_words.clone().unwrap_or_default();

    let config = FillerConfig {
        filler_words: filler_list,
        ..Default::default()
    };

    let _threshold = min_pause_us.unwrap_or(filler::DEFAULT_PAUSE_THRESHOLD_US);
    let _max_gap = max_gap_us.unwrap_or(filler::DEFAULT_MAX_GAP_US);

    // Audio-aware survivor selection requires the source audio. When a
    // media file is loaded we decode once up front and reuse it for every
    // cleanup pass; when it isn't we fall back to the positional rule so
    // offline unit tests keep working. The live app always has media
    // loaded, so this is the expected path in practice.
    //
    // `decode_media_audio_cached` keeps the decoded buffer on `MediaStore`
    // keyed by path + mtime, so repeated cleanup invocations on the same
    // file skip the multi-second ffmpeg spawn.
    let smart_audio: Option<(std::sync::Arc<Vec<f32>>, u32)> = {
        let media_path = {
            let media = crate::lock_recovery::try_lock(media_store.0.lock()).map_err(|e| e.to_string())?;
            media.current().map(|m| m.path.clone())
        };
        match media_path {
            Some(path) => match crate::commands::disfluency::decode_media_audio_cached(
                &path,
                &media_store,
            ) {
                Ok(pair) => Some(pair),
                Err(e) => {
                    log::warn!(
                        "cleanup_all: audio decode failed, falling back to positional collapse: {}",
                        e
                    );
                    None
                }
            },
            None => None,
        }
    };

    let mut state = crate::lock_recovery::try_lock(store.0.lock()).map_err(|e| e.to_string())?;
    state.push_undo_snapshot();

    let mut total_fillers: usize = 0;
    let mut total_duplicates: usize = 0;
    let mut passes: usize = 0;
    const MAX_PASSES: usize = 10;

    // Iterative loop: delete fillers → collapse repeat groups → repeat
    // Use direct word mutation to avoid undo snapshot per word
    loop {
        passes += 1;
        let mut changed = false;

        // Detect and delete fillers
        let filler_indices = filler::detect_fillers(state.get_words(), &config);
        if !filler_indices.is_empty() {
            let words = state.get_words_mut();
            for &idx in &filler_indices {
                if idx < words.len() && !words[idx].deleted {
                    words[idx].deleted = true;
                }
            }
            total_fillers += filler_indices.len();
            changed = true;
        }

        // Collapse repeat groups. Audio-aware when we have samples; when
        // we don't, `detect_duplicates` returns the positional second-
        // word rule as a documented fallback.
        let dup_indices: Vec<usize> = match smart_audio.as_ref() {
            Some((samples, sr)) => {
                let (_decisions, indices) = crate::commands::disfluency::plan_smart_collapse(
                    state.get_words(),
                    samples,
                    *sr,
                );
                indices
            }
            None => filler::detect_duplicates(state.get_words()),
        };
        if !dup_indices.is_empty() {
            let words = state.get_words_mut();
            for &idx in &dup_indices {
                if idx < words.len() && !words[idx].deleted {
                    words[idx].deleted = true;
                }
            }
            total_duplicates += dup_indices.len();
            changed = true;
        }

        if !changed || passes >= MAX_PASSES {
            break;
        }
    }

    if total_fillers > 0 || total_duplicates > 0 {
        state.bump_revision();
    }

    Ok(CleanupResult {
        fillers_removed: total_fillers,
        duplicates_removed: total_duplicates,
        pauses_trimmed: 0,
        gaps_tightened: 0,
        passes,
    })
}

#[cfg(test)]
mod remove_silence_tests {
    //! Unit tests for the Remove silence threshold/collapse pair.
    //! Exercises `filler::count_trimmable_pauses` + `filler::trim_pauses`
    //! at the exact constants `remove_silence` applies, so regressions
    //! in either helper surface here before they reach the editor UI.
    use crate::managers::editor::Word;
    use crate::managers::filler::{
        count_trimmable_pauses, trim_pauses, REMOVE_SILENCE_MAX_GAP_US,
        REMOVE_SILENCE_THRESHOLD_US,
    };

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
        Word { deleted: true, ..word(text, start_us, end_us) }
    }

    #[test]
    fn count_trimmable_agrees_with_trim_pauses_return() {
        // Parity gate: the dry-run count used by remove_silence must
        // match the mutating trim_pauses count for every fixture.
        let fixtures: Vec<Vec<Word>> = vec![
            // no gaps
            vec![word("a", 0, 500_000), word("b", 500_000, 1_000_000)],
            // one long gap at/above threshold (800 ms)
            vec![word("a", 0, 500_000), word("b", 1_300_000, 1_600_000)],
            // threshold boundary — exactly 750 ms, collapse 0 ⇒ count
            vec![word("a", 0, 500_000), word("b", 1_250_000, 1_500_000)],
            // threshold boundary — 749 ms, below ⇒ no count
            vec![word("a", 0, 500_000), word("b", 1_249_000, 1_500_000)],
            // deleted words skipped
            vec![
                word("a", 0, 500_000),
                deleted_word("x", 600_000, 700_000),
                word("b", 2_000_000, 2_500_000),
            ],
        ];
        for mut f in fixtures {
            let predicted = count_trimmable_pauses(
                &f,
                REMOVE_SILENCE_THRESHOLD_US,
                REMOVE_SILENCE_MAX_GAP_US,
            );
            let applied = trim_pauses(
                &mut f,
                REMOVE_SILENCE_THRESHOLD_US,
                REMOVE_SILENCE_MAX_GAP_US,
            );
            assert_eq!(predicted, applied, "dry-run vs mutating count must match");
        }
    }

    #[test]
    fn collapses_one_second_gap_to_zero() {
        let mut words = vec![
            word("hello", 0, 500_000),
            word("world", 1_500_000, 2_000_000), // 1 s gap
        ];
        let count = trim_pauses(
            &mut words,
            REMOVE_SILENCE_THRESHOLD_US,
            REMOVE_SILENCE_MAX_GAP_US,
        );
        assert_eq!(count, 1);
        // gap was 1_000_000, target 0 ⇒ shift = 1_000_000
        assert_eq!(words[1].start_us, 500_000);
        assert_eq!(words[1].end_us, 1_000_000);
    }

    #[test]
    fn idempotent_on_second_call() {
        let mut words = vec![
            word("hello", 0, 500_000),
            word("world", 1_500_000, 2_000_000),
            word("there", 3_500_000, 4_000_000),
        ];
        let first = trim_pauses(
            &mut words,
            REMOVE_SILENCE_THRESHOLD_US,
            REMOVE_SILENCE_MAX_GAP_US,
        );
        assert!(first > 0);
        let second = trim_pauses(
            &mut words,
            REMOVE_SILENCE_THRESHOLD_US,
            REMOVE_SILENCE_MAX_GAP_US,
        );
        assert_eq!(second, 0);
        let dry = count_trimmable_pauses(
            &words,
            REMOVE_SILENCE_THRESHOLD_US,
            REMOVE_SILENCE_MAX_GAP_US,
        );
        assert_eq!(dry, 0);
    }
}
