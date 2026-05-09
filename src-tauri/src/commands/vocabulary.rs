use tauri::{AppHandle, State};

use crate::commands::editor::EditorStore;
use crate::managers::vocabulary::{
    apply_corrections, count_applicable_corrections, find_vocabulary_corrections,
    VocabularyCorrection,
};
use crate::settings::{get_settings, write_settings};

fn normalize_vocabulary_word(word: &str) -> anyhow::Result<String> {
    let normalized = word.trim().to_string();
    if normalized.is_empty() {
        anyhow::bail!("Vocabulary entries cannot be empty")
    }
    Ok(normalized)
}

fn vocabulary_key(word: &str) -> String {
    word.trim().to_lowercase()
}

fn append_vocabulary_word(vocabulary: &mut Vec<String>, word: String) -> bool {
    let key = vocabulary_key(&word);
    if vocabulary
        .iter()
        .any(|existing| vocabulary_key(existing) == key)
    {
        return false;
    }

    vocabulary.push(word);
    true
}

pub(crate) fn normalize_vocabulary_entries(words: Vec<String>) -> anyhow::Result<Vec<String>> {
    let mut normalized = Vec::new();

    for word in words {
        let word = normalize_vocabulary_word(&word)?;
        append_vocabulary_word(&mut normalized, word);
    }

    Ok(normalized)
}

fn remove_vocabulary_word(vocabulary: &mut Vec<String>, word: &str) -> bool {
    let key = vocabulary_key(word);
    let original_len = vocabulary.len();
    vocabulary.retain(|existing| vocabulary_key(existing) != key);
    vocabulary.len() != original_len
}

fn update_vocabulary<F>(app: &AppHandle, update: F) -> Result<Vec<String>, String>
where
    F: FnOnce(&mut Vec<String>) -> anyhow::Result<()>,
{
    let mut settings = get_settings(app);
    update(&mut settings.custom_words).map_err(|err| err.to_string())?;
    let vocabulary = settings.custom_words.clone();
    write_settings(app, settings);
    Ok(vocabulary)
}

#[tauri::command]
#[specta::specta]
pub fn get_vocabulary_suggestions(
    app: AppHandle,
    store: State<EditorStore>,
    confidence_threshold: Option<f32>,
) -> Result<Vec<VocabularyCorrection>, String> {
    let settings = get_settings(&app);
    let confidence_threshold = confidence_threshold.unwrap_or(1.0);
    let state = crate::lock_recovery::recover_lock(store.0.lock());

    Ok(find_vocabulary_corrections(
        state.get_words(),
        &settings.custom_words,
        settings.word_correction_threshold,
        confidence_threshold,
    ))
}

#[tauri::command]
#[specta::specta]
pub fn apply_vocabulary_corrections(
    store: State<EditorStore>,
    corrections: Vec<VocabularyCorrection>,
) -> Result<usize, String> {
    let mut state =
        crate::lock_recovery::try_lock(store.0.lock()).map_err(|err| err.to_string())?;
    if count_applicable_corrections(state.get_words(), &corrections) == 0 {
        return Ok(0);
    }

    state.push_undo_snapshot();
    let applied = apply_corrections(state.get_words_mut(), &corrections);
    if applied > 0 {
        state.bump_revision();
    }

    Ok(applied)
}

#[tauri::command]
#[specta::specta]
pub fn add_to_vocabulary(app: AppHandle, word: String) -> Result<Vec<String>, String> {
    let normalized = normalize_vocabulary_word(&word).map_err(|err| err.to_string())?;
    update_vocabulary(&app, move |vocabulary| {
        let mut updated = vocabulary.clone();
        updated.push(normalized);
        *vocabulary = normalize_vocabulary_entries(updated)?;
        Ok(())
    })
}

#[tauri::command]
#[specta::specta]
pub fn remove_from_vocabulary(app: AppHandle, word: String) -> Result<Vec<String>, String> {
    let normalized = normalize_vocabulary_word(&word).map_err(|err| err.to_string())?;
    update_vocabulary(&app, move |vocabulary| {
        remove_vocabulary_word(vocabulary, &normalized);
        Ok(())
    })
}

#[tauri::command]
#[specta::specta]
pub fn get_vocabulary(app: AppHandle) -> Result<Vec<String>, String> {
    Ok(get_settings(&app).custom_words)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalize_vocabulary_word_rejects_blank_entries() {
        let result = normalize_vocabulary_word("   ");

        assert!(result.is_err());
    }

    #[test]
    fn append_vocabulary_word_deduplicates_case_insensitively() {
        let mut vocabulary = vec!["ChatGPT".to_string()];

        let inserted = append_vocabulary_word(&mut vocabulary, "chatgpt".to_string());

        assert!(!inserted);
        assert_eq!(vocabulary, vec!["ChatGPT".to_string()]);
    }

    #[test]
    fn remove_vocabulary_word_matches_case_insensitively() {
        let mut vocabulary = vec!["ChargeBee".to_string(), "OpenAI".to_string()];

        let removed = remove_vocabulary_word(&mut vocabulary, "chargebee");

        assert!(removed);
        assert_eq!(vocabulary, vec!["OpenAI".to_string()]);
    }

    #[test]
    fn normalize_vocabulary_entries_trims_and_deduplicates() {
        let normalized = normalize_vocabulary_entries(vec![
            "  ChatGPT  ".to_string(),
            "chatgpt".to_string(),
            "OpenAI".to_string(),
        ])
        .expect("normalization should succeed");

        assert_eq!(
            normalized,
            vec!["ChatGPT".to_string(), "OpenAI".to_string()]
        );
    }
}
