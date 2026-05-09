use tauri::State;

use crate::commands::editor::EditorStore;
use crate::managers::speaker::{
    assign_speaker_range, assign_speakers_by_gaps, clear_speaker_assignments, get_speaker_stats,
    merge_speakers as merge_speaker_ids, SpeakerGapConfig, SpeakerInfo,
};

fn speaker_exists(state: &crate::managers::editor::EditorState, speaker_id: i32) -> bool {
    state
        .get_words()
        .iter()
        .any(|word| word.speaker_id == speaker_id)
}

fn speaker_stats_with_names(state: &crate::managers::editor::EditorState) -> Vec<SpeakerInfo> {
    let mut stats = get_speaker_stats(state.get_words());
    for speaker in &mut stats {
        if let Some(name) = state.get_speaker_name(speaker.id) {
            speaker.name = name.clone();
        }
    }
    stats
}

/// Return aggregated speaker stats for the active transcript.
#[tauri::command]
#[specta::specta]
pub fn get_speakers(store: State<'_, EditorStore>) -> Result<Vec<SpeakerInfo>, String> {
    let state = crate::lock_recovery::recover_lock(store.0.lock());
    Ok(speaker_stats_with_names(&state))
}

/// Auto-assign speakers by detecting pauses between words.
#[tauri::command]
#[specta::specta]
pub fn auto_assign_speakers(
    store: State<'_, EditorStore>,
    min_gap_us: Option<i64>,
    max_speakers: Option<usize>,
) -> Result<Vec<SpeakerInfo>, String> {
    let mut state = crate::lock_recovery::try_lock(store.0.lock()).map_err(|e| e.to_string())?;
    if state.get_words().is_empty() {
        return Ok(Vec::new());
    }

    let config = SpeakerGapConfig {
        min_gap_us: min_gap_us.unwrap_or(SpeakerGapConfig::default().min_gap_us),
        max_speakers: max_speakers
            .unwrap_or(SpeakerGapConfig::default().max_speakers)
            .max(1),
    };

    state.push_undo_snapshot();
    assign_speakers_by_gaps(state.get_words_mut(), &config);
    state.bump_revision();
    Ok(speaker_stats_with_names(&state))
}

/// Merge one speaker into another and return the updated stats.
#[tauri::command]
#[specta::specta]
pub fn merge_speakers(
    store: State<'_, EditorStore>,
    from_id: i32,
    to_id: i32,
) -> Result<Vec<SpeakerInfo>, String> {
    if from_id < 0 || to_id < 0 {
        return Err("Speaker IDs must be non-negative.".to_string());
    }
    if from_id == to_id {
        return Err("Choose two different speakers to merge.".to_string());
    }

    let mut state = crate::lock_recovery::try_lock(store.0.lock()).map_err(|e| e.to_string())?;
    if !speaker_exists(&state, from_id) {
        return Err(format!(
            "Speaker {from_id} is not assigned in this project."
        ));
    }

    state.push_undo_snapshot();
    merge_speaker_ids(state.get_words_mut(), from_id, to_id);
    if state.get_speaker_name(to_id).is_none() {
        if let Some(name) = state.get_speaker_name(from_id).cloned() {
            state.set_speaker_name(to_id, name);
        }
    }
    let _ = state.remove_speaker_name(from_id);
    state.bump_revision();
    Ok(speaker_stats_with_names(&state))
}

/// Assign a speaker to an inclusive word range.
#[tauri::command]
#[specta::specta]
pub fn assign_speaker_to_range(
    store: State<'_, EditorStore>,
    start_index: usize,
    end_index: usize,
    speaker_id: i32,
) -> Result<(), String> {
    if speaker_id < 0 {
        return Err("Speaker ID must be non-negative.".to_string());
    }

    let mut state = crate::lock_recovery::try_lock(store.0.lock()).map_err(|e| e.to_string())?;
    if start_index > end_index || end_index >= state.get_words().len() {
        return Err("Invalid speaker assignment range.".to_string());
    }

    state.push_undo_snapshot();
    assign_speaker_range(state.get_words_mut(), start_index, end_index, speaker_id);
    state.bump_revision();
    Ok(())
}

/// Clear all speaker IDs and custom names from the active transcript.
#[tauri::command]
#[specta::specta]
pub fn clear_speakers(store: State<'_, EditorStore>) -> Result<(), String> {
    let mut state = crate::lock_recovery::try_lock(store.0.lock()).map_err(|e| e.to_string())?;
    if state.get_words().iter().all(|word| word.speaker_id < 0)
        && state.get_speaker_names().is_empty()
    {
        return Ok(());
    }

    state.push_undo_snapshot();
    clear_speaker_assignments(state.get_words_mut());
    state.clear_speaker_names();
    state.bump_revision();
    Ok(())
}

/// Rename a speaker. Empty names remove the custom override.
#[tauri::command]
#[specta::specta]
pub fn rename_speaker(
    store: State<'_, EditorStore>,
    speaker_id: i32,
    name: String,
) -> Result<(), String> {
    if speaker_id < 0 {
        return Err("Speaker ID must be non-negative.".to_string());
    }

    let mut state = crate::lock_recovery::try_lock(store.0.lock()).map_err(|e| e.to_string())?;
    if !speaker_exists(&state, speaker_id) {
        return Err(format!(
            "Speaker {speaker_id} is not assigned in this project."
        ));
    }

    let trimmed = name.trim().to_string();
    let existing_name = state.get_speaker_name(speaker_id).cloned();
    if existing_name.as_deref() == Some(trimmed.as_str()) {
        return Ok(());
    }

    if trimmed.is_empty() {
        if existing_name.is_none() {
            return Ok(());
        }
        state.push_undo_snapshot();
        let _ = state.remove_speaker_name(speaker_id);
        state.bump_revision();
        return Ok(());
    }

    state.push_undo_snapshot();
    state.set_speaker_name(speaker_id, trimmed);
    state.bump_revision();
    Ok(())
}
