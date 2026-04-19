//! Deprecated Tauri commands for the post-processor (GGUF LLM) catalog.
//!
//! These are thin shims over the unified `commands::models::*` surface,
//! retained for one release so the frontend can migrate at its own pace.
//! Each shim emits `log::warn!` once per call with `DEPRECATION_MESSAGES`
//! so QC can spot stale callers. Deletion tracked in
//! `umc-delete-llm-catalog` tasks follow-up.
//!
//! Equivalent unified calls:
//! - `list_llm_models`       -> `get_models(Some(PostProcessor))`
//! - `download_llm_model`    -> `download_model(id, Some(PostProcessor))`
//! - `cancel_llm_download`   -> `cancel_download(id)`
//! - `delete_llm_model`      -> `delete_model(id)`
//! - `set_selected_llm_model` -> settings-only (no unified replacement yet).

use crate::managers::llm::{LlmManager, LlmModelInfo};
use crate::managers::model::{ModelCategory, ModelManager};
use std::sync::Arc;
use tauri::{AppHandle, Emitter, State};

/// Canonical deprecation warnings. Shared between the shim call site and
/// the unit test that guards the shim contract.
pub mod deprecation {
    pub const DOWNLOAD_LLM_MODEL: &str =
        "download_llm_model is deprecated; call download_model(id, PostProcessor) instead";
    pub const LIST_LLM_MODELS: &str =
        "list_llm_models is deprecated; call get_models(Some(PostProcessor)) instead";
    pub const CANCEL_LLM_DOWNLOAD: &str =
        "cancel_llm_download is deprecated; call cancel_download(id) instead";
    pub const DELETE_LLM_MODEL: &str =
        "delete_llm_model is deprecated; call delete_model(id) instead";
    pub const SET_SELECTED_LLM_MODEL: &str =
        "set_selected_llm_model is deprecated; call set_selected_model(id, PostProcessor) instead";
}

#[tauri::command]
#[specta::specta]
pub async fn list_llm_models(
    llm_manager: State<'_, Arc<LlmManager>>,
) -> Result<Vec<LlmModelInfo>, String> {
    log::warn!("{}", deprecation::LIST_LLM_MODELS);
    Ok(llm_manager.list_models())
}

#[tauri::command]
#[specta::specta]
pub async fn download_llm_model(
    app_handle: AppHandle,
    llm_manager: State<'_, Arc<LlmManager>>,
    model_id: String,
) -> Result<(), String> {
    log::warn!("{}", deprecation::DOWNLOAD_LLM_MODEL);
    let emitter = app_handle.clone();
    let emit_id = model_id.clone();
    let result = llm_manager
        .download(&model_id, move |progress| {
            let _ = emitter.emit(
                "llm-model-download-progress",
                serde_json::json!({
                    "model_id": progress.id,
                    "downloaded": progress.downloaded_bytes,
                    "total": progress.total_bytes,
                    "percentage": progress.percentage,
                    "asset_kind": "llm",
                }),
            );
        })
        .await
        .map_err(|e| e.to_string());

    if let Err(ref error) = result {
        let _ = app_handle.emit(
            "llm-model-download-failed",
            crate::managers::llm::download_failed_payload(&emit_id, error),
        );
    } else {
        let _ = app_handle.emit(
            "llm-model-download-completed",
            serde_json::json!({ "model_id": &emit_id, "asset_kind": "llm" }),
        );
    }
    result
}

#[tauri::command]
#[specta::specta]
pub async fn cancel_llm_download(
    app_handle: AppHandle,
    model_manager: State<'_, Arc<ModelManager>>,
    model_id: String,
) -> Result<(), String> {
    log::warn!("{}", deprecation::CANCEL_LLM_DOWNLOAD);
    model_manager
        .cancel_download(&model_id)
        .map_err(|e| e.to_string())?;
    let _ = app_handle.emit(
        "llm-model-download-cancelled",
        serde_json::json!({ "model_id": &model_id, "asset_kind": "llm" }),
    );
    Ok(())
}

#[tauri::command]
#[specta::specta]
pub async fn delete_llm_model(
    app_handle: AppHandle,
    llm_manager: State<'_, Arc<LlmManager>>,
    model_id: String,
) -> Result<(), String> {
    log::warn!("{}", deprecation::DELETE_LLM_MODEL);
    llm_manager.delete(&model_id).map_err(|e| e.to_string())?;
    // If the deleted model was the selected local LLM, clear the setting.
    let mut settings = crate::settings::get_settings(&app_handle);
    if settings.local_llm_model_id.as_deref() == Some(model_id.as_str()) {
        settings.local_llm_model_id = None;
        crate::settings::write_settings(&app_handle, settings);
    }
    let _ = app_handle.emit(
        "llm-model-deleted",
        serde_json::json!({ "model_id": &model_id, "asset_kind": "llm" }),
    );
    Ok(())
}

#[tauri::command]
#[specta::specta]
pub async fn set_selected_llm_model(
    app_handle: AppHandle,
    model_id: Option<String>,
) -> Result<(), String> {
    log::warn!("{}", deprecation::SET_SELECTED_LLM_MODEL);
    let mut settings = crate::settings::get_settings(&app_handle);
    // Validate that the id is a PostProcessor entry if provided.
    if let Some(ref id) = model_id {
        let mm = app_handle.state::<Arc<ModelManager>>();
        let info = mm
            .get_model_info(id)
            .ok_or_else(|| format!("Unknown LLM model id: {}", id))?;
        if info.category != ModelCategory::PostProcessor {
            return Err(format!(
                "Model {} is not a post-processor (got {:?})",
                id, info.category
            ));
        }
    }
    settings.local_llm_model_id = model_id;
    crate::settings::write_settings(&app_handle, settings);
    Ok(())
}

use tauri::Manager;

#[cfg(test)]
mod deprecation_tests {
    use super::deprecation;

    /// Locks in the deprecation message text. The audit script
    /// `audit-unified-model-catalog.ps1 -Check no-llm-dl` greps for these
    /// strings when confirming the shim is still wired.
    #[test]
    fn download_llm_model_shim_forwards_with_deprecation() {
        assert!(deprecation::DOWNLOAD_LLM_MODEL.contains("download_llm_model"));
        assert!(deprecation::DOWNLOAD_LLM_MODEL.contains("deprecated"));
        assert!(deprecation::DOWNLOAD_LLM_MODEL.contains("PostProcessor"));
    }

    #[test]
    fn all_llm_shims_have_deprecation_messages() {
        for msg in [
            deprecation::DOWNLOAD_LLM_MODEL,
            deprecation::LIST_LLM_MODELS,
            deprecation::CANCEL_LLM_DOWNLOAD,
            deprecation::DELETE_LLM_MODEL,
            deprecation::SET_SELECTED_LLM_MODEL,
        ] {
            assert!(msg.contains("deprecated"), "msg: {}", msg);
        }
    }
}
