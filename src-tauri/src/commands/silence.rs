use tauri::State;

use crate::commands::editor::EditorStore;
use crate::managers::silence::{
    find_silence_gaps_from_words, mark_silence_gaps, SilenceConfig, SilenceWordCandidate,
};

/// Analyze silence candidates from transcript word gaps.
///
/// This command stays on the backend side of the source-of-truth boundary:
/// it derives silence candidates from existing word timestamps and can be
/// upgraded later to consume a backend-built VAD curve without changing the
/// frontend IPC shape.
#[tauri::command]
#[specta::specta]
pub fn analyze_silence(
    store: State<EditorStore>,
    config: Option<SilenceConfig>,
) -> Result<Vec<SilenceWordCandidate>, String> {
    let state = crate::lock_recovery::recover_lock(store.0.lock());
    let config = config.unwrap_or_default();
    Ok(find_silence_gaps_from_words(
        state.get_words(),
        &config,
        &[],
    ))
}

/// Apply silence-gap markings to the current transcript.
#[tauri::command]
#[specta::specta]
pub fn apply_silence_removal(
    store: State<EditorStore>,
    candidates: Vec<SilenceWordCandidate>,
    only_confirmed: bool,
) -> Result<usize, String> {
    let mut state = crate::lock_recovery::recover_lock(store.0.lock());

    let mut preview_words = state.get_words().to_vec();
    let preview_count = mark_silence_gaps(&mut preview_words, &candidates, only_confirmed);
    if preview_count == 0 {
        return Ok(0);
    }

    state.push_undo_snapshot();
    let count = mark_silence_gaps(state.get_words_mut(), &candidates, only_confirmed);
    if count > 0 {
        state.bump_revision();
    }

    Ok(count)
}
