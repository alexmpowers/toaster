use std::sync::Arc;

use tauri::State;

use crate::commands::editor::EditorStore;
use crate::managers::cleanup::{
    apply_cleanup_plan, plan_cleanup, CleanupAudioContext, CleanupConfig, CleanupPlan,
    CleanupPreset,
};
use crate::managers::media::MediaStore;

type CleanupAudioData = (Arc<Vec<f32>>, u32);

fn load_cleanup_audio(media_store: &MediaStore) -> Result<Option<CleanupAudioData>, String> {
    let media_path = {
        let media =
            crate::lock_recovery::try_lock(media_store.0.lock()).map_err(|e| e.to_string())?;
        media.current().map(|info| info.path.clone())
    };

    let Some(media_path) = media_path else {
        return Ok(None);
    };

    match crate::commands::disfluency::decode_media_audio_cached(&media_path, media_store) {
        Ok(audio) => Ok(Some(audio)),
        Err(error) => {
            log::warn!(
                "cleanup preview/apply: audio decode failed, falling back to non-audio cleanup: {}",
                error
            );
            Ok(None)
        }
    }
}

#[tauri::command]
#[specta::specta]
pub fn preview_cleanup(
    app: tauri::AppHandle,
    store: State<'_, EditorStore>,
    media_store: State<'_, MediaStore>,
    config: Option<CleanupConfig>,
    preset: Option<CleanupPreset>,
) -> Result<CleanupPlan, String> {
    let settings = crate::settings::get_settings(&app);
    let custom_fillers = settings.custom_filler_words.unwrap_or_default();
    let config = config
        .unwrap_or_else(|| CleanupConfig::from_preset(preset.unwrap_or(CleanupPreset::Balanced)));
    let audio = load_cleanup_audio(&media_store)?;
    let audio_context = audio
        .as_ref()
        .map(|(samples, sample_rate)| CleanupAudioContext {
            samples: samples.as_slice(),
            sample_rate: *sample_rate,
        });

    let state = crate::lock_recovery::recover_lock(store.0.lock());
    let mut plan = plan_cleanup(state.get_words(), &config, &custom_fillers, audio_context);
    plan.source_revision = state.timing_contract_snapshot().timeline_revision;
    Ok(plan)
}

#[tauri::command]
#[specta::specta]
pub fn apply_cleanup(
    store: State<'_, EditorStore>,
    plan: CleanupPlan,
) -> Result<CleanupPlan, String> {
    let mut state = crate::lock_recovery::try_lock(store.0.lock()).map_err(|e| e.to_string())?;
    let current_revision = state.timing_contract_snapshot().timeline_revision;
    if current_revision != plan.source_revision {
        return Err("Cleanup preview is stale. Preview again before applying.".to_string());
    }
    if plan.total_affected == 0 {
        return Ok(plan);
    }

    let mut preview_words = state.get_words().to_vec();
    let modified = apply_cleanup_plan(&mut preview_words, &plan);
    if modified == 0 {
        return Ok(plan);
    }

    state.push_undo_snapshot();
    let _ = apply_cleanup_plan(state.get_words_vec_mut(), &plan);
    state.bump_revision();
    Ok(plan)
}
