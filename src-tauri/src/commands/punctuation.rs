use tauri::State;

use crate::commands::editor::EditorStore;
use crate::managers::punctuation::{
    apply_punctuation, count_applicable_insertions, detect_boundaries, PunctuationAnalysis,
    PunctuationConfig, SentenceBoundary,
};

#[tauri::command]
#[specta::specta]
pub fn analyze_punctuation(
    store: State<EditorStore>,
    config: Option<PunctuationConfig>,
) -> Result<PunctuationAnalysis, String> {
    let state = crate::lock_recovery::recover_lock(store.0.lock());
    let config = config.unwrap_or_default();
    Ok(detect_boundaries(state.get_words(), &config))
}

#[tauri::command]
#[specta::specta]
pub fn apply_punctuation_corrections(
    store: State<EditorStore>,
    boundaries: Vec<SentenceBoundary>,
) -> Result<usize, String> {
    let mut state =
        crate::lock_recovery::try_lock(store.0.lock()).map_err(|err| err.to_string())?;
    if count_applicable_insertions(state.get_words(), &boundaries) == 0 {
        return Ok(0);
    }

    state.push_undo_snapshot();
    let modified = apply_punctuation(state.get_words_mut(), &boundaries);
    if modified > 0 {
        state.bump_revision();
    }

    Ok(modified)
}
