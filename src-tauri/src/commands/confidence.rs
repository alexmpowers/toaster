use tauri::State;

use crate::commands::editor::EditorStore;
use crate::managers::confidence::{
    can_mark_word_verified, find_low_confidence_words, mark_word_verified, mark_words_verified,
    LowConfidenceWord,
};

#[tauri::command]
#[specta::specta]
pub fn get_low_confidence_words(
    store: State<EditorStore>,
    threshold: Option<f32>,
) -> Result<Vec<LowConfidenceWord>, String> {
    let state = crate::lock_recovery::recover_lock(store.0.lock());
    Ok(find_low_confidence_words(
        state.get_words(),
        threshold.unwrap_or(0.7),
    ))
}

#[tauri::command]
#[specta::specta]
pub fn verify_word(store: State<EditorStore>, index: usize) -> Result<bool, String> {
    let mut state = crate::lock_recovery::recover_lock(store.0.lock());
    if !can_mark_word_verified(state.get_words(), index) {
        return Ok(false);
    }

    state.push_undo_snapshot();
    let updated = mark_word_verified(state.get_words_mut(), index);
    if updated {
        state.bump_revision();
    }

    Ok(updated)
}

#[tauri::command]
#[specta::specta]
pub fn verify_all_words(store: State<EditorStore>, indices: Vec<usize>) -> Result<usize, String> {
    let mut state = crate::lock_recovery::recover_lock(store.0.lock());
    if !indices
        .iter()
        .any(|&index| can_mark_word_verified(state.get_words(), index))
    {
        return Ok(0);
    }

    state.push_undo_snapshot();
    let updated = mark_words_verified(state.get_words_mut(), &indices);
    if updated > 0 {
        state.bump_revision();
    }

    Ok(updated)
}
