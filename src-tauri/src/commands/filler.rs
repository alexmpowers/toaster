use tauri::State;

use crate::commands::editor::EditorStore;
use crate::managers::filler::{self, FillerConfig};

/// Detect filler words and long pauses in the current transcript.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, specta::Type)]
pub struct FillerAnalysis {
    pub filler_indices: Vec<usize>,
    /// Each pause: (word_index_before_gap, gap_duration_us)
    pub pauses: Vec<PauseInfo>,
    pub filler_count: usize,
    pub pause_count: usize,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, specta::Type)]
pub struct PauseInfo {
    pub after_word_index: usize,
    pub gap_duration_us: i64,
}

#[tauri::command]
#[specta::specta]
pub fn analyze_fillers(
    store: State<EditorStore>,
    min_pause_us: Option<i64>,
) -> Result<FillerAnalysis, String> {
    let state = store.0.lock().unwrap();
    let words = state.get_words();

    let mut config = FillerConfig::default();
    if let Some(threshold) = min_pause_us {
        config.pause_threshold_us = threshold;
    }

    let fillers = filler::detect_fillers(words, &config);
    let pauses = filler::detect_pauses(words, &config);

    let pause_infos: Vec<PauseInfo> = pauses
        .into_iter()
        .map(|(idx, dur)| PauseInfo {
            after_word_index: idx,
            gap_duration_us: dur,
        })
        .collect();

    Ok(FillerAnalysis {
        filler_count: fillers.len(),
        pause_count: pause_infos.len(),
        filler_indices: fillers,
        pauses: pause_infos,
    })
}

/// Auto-delete all detected filler words in the transcript.
#[tauri::command]
#[specta::specta]
pub fn delete_fillers(store: State<EditorStore>) -> Result<usize, String> {
    let config = FillerConfig::default();

    let mut state = store.0.lock().unwrap();
    let indices = filler::detect_fillers(state.get_words(), &config);
    let count = indices.len();

    if count == 0 {
        return Ok(0);
    }

    for &idx in &indices {
        state.delete_word(idx);
    }

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

    let mut state = store.0.lock().unwrap();
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
