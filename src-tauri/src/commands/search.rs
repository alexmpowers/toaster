use tauri::State;

use crate::commands::editor::EditorStore;
use crate::managers::search::{search_words, SearchMode, SearchResult};

/// Search the active transcript using exact, fuzzy, or phonetic matching.
#[tauri::command]
#[specta::specta]
pub fn search_transcript(
    store: State<EditorStore>,
    query: String,
    mode: SearchMode,
    max_distance: Option<usize>,
) -> Result<SearchResult, String> {
    let state = crate::lock_recovery::recover_lock(store.0.lock());
    Ok(search_words(state.get_words(), &query, mode, max_distance))
}
