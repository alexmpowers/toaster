//! Transcription job lifecycle: the `transcribe` entry point that drives a
//! single audio buffer through the loaded ASR engine, normalizes the result
//! through the adapter layer, and applies post-processing (custom words,
//! filler/whitespace filtering).
//!
//! Extracted from `mod.rs` to keep that file under the 800-line cap.

use crate::audio_toolkit::{apply_custom_words, filter_transcription_output};
use crate::managers::model::EngineType;
use crate::settings::get_settings;
use anyhow::Result;
use log::{debug, error, info, warn};
use std::panic::{catch_unwind, AssertUnwindSafe};
use tauri::Emitter;

use super::engine_call::call_engine_chunk;
use super::{adapter, ModelStateEvent, TranscriptionManager};

impl TranscriptionManager {
    pub fn transcribe(&self, audio: Vec<f32>) -> Result<adapter::NormalizedTranscriptionResult> {
        #[cfg(debug_assertions)]
        if std::env::var("HANDY_FORCE_TRANSCRIPTION_FAILURE").is_ok() {
            return Err(anyhow::anyhow!(
                "Simulated transcription failure (HANDY_FORCE_TRANSCRIPTION_FAILURE)"
            ));
        }

        // Update last activity timestamp
        self.touch_activity();

        let st = std::time::Instant::now();

        debug!("Audio vector length: {}", audio.len());

        if audio.is_empty() {
            debug!("Empty audio vector");
            self.maybe_unload_immediately("empty audio");
            return Ok(adapter::NormalizedTranscriptionResult {
                words: Vec::new(),
                text: String::new(),
                segments: None,
                language: "und".to_string(),
                word_timestamps_authoritative: false,
            });
        }

        // Check if model is loaded, if not try to load it
        {
            // If the model is loading, wait for it to complete.
            let mut is_loading = crate::lock_recovery::recover_lock(self.is_loading.lock());
            while *is_loading {
                let (guard, timeout_result) = self
                    .loading_condvar
                    .wait_timeout(is_loading, std::time::Duration::from_secs(300))
                    .unwrap();
                is_loading = guard;
                if timeout_result.timed_out() && *is_loading {
                    return Err(anyhow::anyhow!("Model loading timed out after 5 minutes"));
                }
            }

            let engine_guard = self.lock_engine();
            if engine_guard.is_none() {
                return Err(anyhow::anyhow!("Model is not loaded for transcription."));
            }
        }

        // Get current settings for configuration
        let settings = get_settings(&self.app_handle);

        // Validate selected language against the model's supported languages.
        // If the language isn't supported, fall back to "auto" to prevent errors.
        let validated_language = if settings.selected_language == "auto" {
            "auto".to_string()
        } else {
            let is_supported = self
                .model_manager
                .get_model_info(&settings.selected_model)
                .map(|info| {
                    info.supported_languages.is_empty()
                        || info
                            .supported_languages
                            .contains(&settings.selected_language)
                })
                .unwrap_or(true);

            if is_supported {
                settings.selected_language.clone()
            } else {
                warn!(
                    "Language '{}' not supported by current model, falling back to auto-detect",
                    settings.selected_language
                );
                "auto".to_string()
            }
        };

        // Resolve the adapter for the current model. Capabilities drive the
        // prompt-injection vs fuzzy-correction branch below (replacing the
        // old `is_whisper` bool) and `normalize_language` replaces the
        // per-engine `zh-Hans` / `auto` match arms.
        let engine_type_for_adapter = self
            .model_manager
            .get_model_info(&settings.selected_model)
            .map(|info| info.engine_type.clone());
        let adapter: &'static dyn adapter::TranscriptionModelAdapter =
            match &engine_type_for_adapter {
                Some(et) => adapter::adapter_for_engine(et),
                // Fall back to Whisper's adapter — historical default. This
                // branch only fires if ModelManager can't find the model at
                // all, which usually means the settings file points at a
                // deleted model; `transcribe_with` below will still fail
                // with a clearer error.
                None => adapter::adapter_for_engine(&EngineType::Whisper),
            };
        let normalized_language = adapter.normalize_language(&validated_language);

        // Perform transcription with the appropriate engine.
        // We use catch_unwind to prevent engine panics from poisoning the mutex,
        // which would make the app hang indefinitely on subsequent operations.
        let result = {
            let mut engine_guard = self.lock_engine();

            // Take the engine out so we own it during transcription.
            // If the engine panics, we simply don't put it back (effectively unloading it)
            // instead of poisoning the mutex.
            let mut engine = match engine_guard.take() {
                Some(e) => e,
                None => {
                    return Err(anyhow::anyhow!(
                        "Model failed to load after auto-load attempt. Please check your model settings."
                    ));
                }
            };

            // Release the lock before transcribing — no mutex held during the engine call
            drop(engine_guard);

            let transcribe_result = catch_unwind(AssertUnwindSafe(
                || -> Result<transcribe_rs::TranscriptionResult> {
                    // Always take the full-buffer ASR path. The R-002 VAD
                    // pre-filter has been removed — user feedback showed it
                    // degraded transcript timing edits (short words / fillers
                    // at splice boundaries were clipped). Boundary refinement
                    // (R-003) remains available via `vad_refine_boundaries`.
                    call_engine_chunk(&mut engine, &audio, &settings, &normalized_language)
                },
            ));

            match transcribe_result {
                Ok(inner_result) => {
                    // Success or normal error — put the engine back
                    let mut engine_guard = self.lock_engine();
                    *engine_guard = Some(engine);
                    inner_result?
                }
                Err(panic_payload) => {
                    // Engine panicked — do NOT put it back (it's in an unknown state).
                    // The engine is dropped here, effectively unloading it.
                    let panic_msg = if let Some(s) = panic_payload.downcast_ref::<&str>() {
                        s.to_string()
                    } else if let Some(s) = panic_payload.downcast_ref::<String>() {
                        s.clone()
                    } else {
                        "unknown panic".to_string()
                    };
                    error!(
                        "Transcription engine panicked: {}. Model has been unloaded.",
                        panic_msg
                    );

                    // Clear the model ID so it will be reloaded on next attempt
                    {
                        let mut current_model = self
                            .current_model_id
                            .lock()
                            .unwrap_or_else(|e| e.into_inner());
                        *current_model = None;
                    }

                    let _ = self.app_handle.emit(
                        "model-state-changed",
                        ModelStateEvent {
                            event_type: "unloaded".to_string(),
                            model_id: None,
                            model_name: None,
                            error: Some(format!("Engine panicked: {}", panic_msg)),
                        },
                    );

                    return Err(anyhow::anyhow!(
                        "Transcription engine panicked: {}. The model has been unloaded and will reload on next attempt.",
                        panic_msg
                    ));
                }
            }
        };

        // Apply word correction if custom words are configured. Adapters with
        // `supports_prompt_injection = true` (Whisper) biased via
        // `initial_prompt` already, so fuzzy correction is skipped for them.
        // This replaces the old `is_whisper` bool check.
        let corrected_text = if !settings.custom_words.is_empty()
            && adapter.capabilities().supports_fuzzy_word_correction
        {
            apply_custom_words(
                &result.text,
                &settings.custom_words,
                settings.word_correction_threshold,
            )
        } else {
            result.text.clone()
        };

        // Filter out stutter artifacts / excess whitespace. Filler words are
        // kept — the editor's Clean Up feature is responsible for removing
        // them on user confirmation.
        let filtered_text = filter_transcription_output(&corrected_text);

        let et = std::time::Instant::now();
        let translation_note = if settings.translate_to_english {
            " (translated)"
        } else {
            ""
        };
        info!(
            "Transcription completed in {}ms{}",
            (et - st).as_millis(),
            translation_note
        );

        if filtered_text.is_empty() {
            info!("Transcription result is empty");
        } else {
            info!("Transcription result: {}", filtered_text);
        }

        self.maybe_unload_immediately("transcription");

        // Normalize through the adapter, then overwrite the text blob with
        // the post-filtered version. `raw_for_adapt` keeps the engine-reported
        // segment timings intact for downstream `build_words_from_segments`.
        let raw_for_adapt = transcribe_rs::TranscriptionResult {
            text: filtered_text,
            segments: result.segments,
        };
        let audio_info = adapter::AudioInfo::from_samples(
            audio.len(),
            adapter.capabilities().native_input_sample_rate_hz,
            1,
        );
        let normalized = adapter.adapt(raw_for_adapt, audio_info)?;
        info!(
            "Transcription normalized: language={} word_timestamps_authoritative={}",
            normalized.language, normalized.word_timestamps_authoritative
        );
        // TEMP-BLEED-DEBUG: dump word timings for splice-bleed investigation.
        // Remove once bleed-phase1-timings is complete.
        if let Ok(path) = std::env::var("TOASTER_DUMP_WORDS_PATH") {
            let mut buf = String::from("[\n");
            for (i, w) in normalized.words.iter().enumerate() {
                let comma = if i + 1 < normalized.words.len() {
                    ","
                } else {
                    ""
                };
                let text_escaped = w.text.replace('\\', "\\\\").replace('"', "\\\"");
                buf.push_str(&format!(
                    "  {{\"i\":{},\"text\":\"{}\",\"start_us\":{},\"end_us\":{},\"confidence\":{}}}{}\n",
                    i, text_escaped, w.start_us, w.end_us, w.confidence, comma
                ));
            }
            buf.push_str("]\n");
            match std::fs::write(&path, &buf) {
                Ok(_) => info!(
                    "TEMP-BLEED-DEBUG: wrote {} words to {}",
                    normalized.words.len(),
                    path
                ),
                Err(e) => info!("TEMP-BLEED-DEBUG: failed to write {}: {}", path, e),
            }
        }
        Ok(normalized)
    }
}
