//! Shared shortcut event handling logic
//!
//! This module contains the common logic for handling shortcut events,
//! used by both the Tauri and handy-keys implementations.

use log::warn;
use std::sync::Arc;
use tauri::{AppHandle, Manager};

use crate::managers::audio::AudioRecordingManager;
use crate::settings::get_settings;
use crate::transcription_coordinator::is_transcribe_binding;
use crate::TranscriptionCoordinator;

/// Handle a shortcut event from either implementation.
///
/// Legacy dictation bindings (ACTION_MAP) were removed with actions.rs. Only
/// the "transcribe" path is still wired (via TranscriptionCoordinator); other
/// binding IDs are logged and dropped. This whole handler will be removed
/// together with the shortcut/ module by p1-remove-shortcut.
pub fn handle_shortcut_event(
    app: &AppHandle,
    binding_id: &str,
    hotkey_string: &str,
    is_pressed: bool,
) {
    let settings = get_settings(app);

    // Transcribe bindings are handled by the coordinator.
    if is_transcribe_binding(binding_id) {
        if let Some(coordinator) = app.try_state::<TranscriptionCoordinator>() {
            coordinator.send_input(binding_id, hotkey_string, is_pressed, settings.push_to_talk);
        } else {
            warn!("TranscriptionCoordinator is not initialized");
        }
        return;
    }

    // All non-transcribe bindings (cancel, test, …) died with actions.rs.
    let _ = (binding_id, hotkey_string, is_pressed);
    let _ = app.try_state::<Arc<AudioRecordingManager>>();
}
