use crate::audio_toolkit::{apply_custom_words, filter_transcription_output};
use crate::managers::model::{EngineType, ModelManager};
use crate::settings::{get_settings, ModelUnloadTimeout};
use anyhow::Result;
use log::{debug, error, info, warn};
use serde::Serialize;
use std::panic::{catch_unwind, AssertUnwindSafe};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Condvar, Mutex, MutexGuard};
use std::thread;
use std::time::{Duration, SystemTime};
use tauri::{AppHandle, Emitter};
use transcribe_rs::{
    onnx::{
        canary::CanaryModel,
        cohere::CohereModel,
        gigaam::GigaAMModel,
        moonshine::{MoonshineModel, MoonshineVariant, StreamingModel},
        parakeet::{ParakeetModel, ParakeetParams, TimestampGranularity},
        sense_voice::{SenseVoiceModel, SenseVoiceParams},
        Quantization,
    },
    whisper_cpp::{WhisperEngine, WhisperInferenceParams},
    SpeechModel, TranscribeOptions,
};

mod accelerators;
pub mod adapter;
#[allow(unused_imports)]
pub use accelerators::{
    apply_accelerator_settings, get_available_accelerators, AvailableAccelerators, GpuDeviceOption,
};

#[derive(Clone, Debug, Serialize)]
pub struct ModelStateEvent {
    pub event_type: String,
    pub model_id: Option<String>,
    pub model_name: Option<String>,
    pub error: Option<String>,
}

enum LoadedEngine {
    Whisper(WhisperEngine),
    Parakeet(ParakeetModel),
    Moonshine(MoonshineModel),
    MoonshineStreaming(StreamingModel),
    SenseVoice(SenseVoiceModel),
    GigaAM(GigaAMModel),
    Canary(CanaryModel),
    Cohere(CohereModel),
}

/// RAII guard that clears the `is_loading` flag and notifies waiters on drop.
/// Ensures the loading flag is always reset, even on early returns or panics.
pub struct LoadingGuard {
    is_loading: Arc<Mutex<bool>>,
    loading_condvar: Arc<Condvar>,
}

impl Drop for LoadingGuard {
    fn drop(&mut self) {
        let mut is_loading = self.is_loading.lock().unwrap();
        *is_loading = false;
        self.loading_condvar.notify_all();
    }
}

#[derive(Clone)]
pub struct TranscriptionManager {
    engine: Arc<Mutex<Option<LoadedEngine>>>,
    model_manager: Arc<ModelManager>,
    app_handle: AppHandle,
    current_model_id: Arc<Mutex<Option<String>>>,
    last_activity: Arc<AtomicU64>,
    shutdown_signal: Arc<AtomicBool>,
    watcher_handle: Arc<Mutex<Option<thread::JoinHandle<()>>>>,
    is_loading: Arc<Mutex<bool>>,
    loading_condvar: Arc<Condvar>,
}

impl TranscriptionManager {
    pub fn new(app_handle: &AppHandle, model_manager: Arc<ModelManager>) -> Result<Self> {
        let manager = Self {
            engine: Arc::new(Mutex::new(None)),
            model_manager,
            app_handle: app_handle.clone(),
            current_model_id: Arc::new(Mutex::new(None)),
            last_activity: Arc::new(AtomicU64::new(Self::now_ms())),
            shutdown_signal: Arc::new(AtomicBool::new(false)),
            watcher_handle: Arc::new(Mutex::new(None)),
            is_loading: Arc::new(Mutex::new(false)),
            loading_condvar: Arc::new(Condvar::new()),
        };

        // Start the idle watcher
        {
            let app_handle_cloned = app_handle.clone();
            let manager_cloned = manager.clone();
            let shutdown_signal = manager.shutdown_signal.clone();
            let handle = thread::spawn(move || {
                debug!("Idle watcher thread started");
                while !shutdown_signal.load(Ordering::Relaxed) {
                    thread::sleep(Duration::from_secs(10)); // Check every 10 seconds

                    // Check shutdown signal again after sleep
                    if shutdown_signal.load(Ordering::Relaxed) {
                        break;
                    }

                    let settings = get_settings(&app_handle_cloned);
                    let timeout = settings.model_unload_timeout;

                    // Skip Immediately — that variant is handled by
                    // maybe_unload_immediately() after each transcription.
                    if timeout == ModelUnloadTimeout::Immediately {
                        continue;
                    }

                    if let Some(limit_seconds) = timeout.to_seconds() {
                        let last = manager_cloned.last_activity.load(Ordering::Relaxed);
                        let now_ms = TranscriptionManager::now_ms();
                        let idle_ms = now_ms.saturating_sub(last);
                        let limit_ms = limit_seconds * 1000;

                        if idle_ms > limit_ms {
                            // idle -> unload
                            if manager_cloned.is_model_loaded() {
                                let unload_start = std::time::Instant::now();
                                info!(
                                    "Model idle for {}s (limit: {}s), unloading",
                                    idle_ms / 1000,
                                    limit_seconds
                                );
                                match manager_cloned.unload_model() {
                                    Ok(()) => {
                                        let unload_duration = unload_start.elapsed();
                                        info!(
                                            "Model unloaded due to inactivity (took {}ms)",
                                            unload_duration.as_millis()
                                        );
                                    }
                                    Err(e) => {
                                        error!("Failed to unload idle model: {}", e);
                                    }
                                }
                            }
                        }
                    }
                }
                debug!("Idle watcher thread shutting down gracefully");
            });
            *manager.watcher_handle.lock().unwrap() = Some(handle);
        }

        Ok(manager)
    }

    /// Lock the engine mutex, recovering from poison if a previous transcription panicked.
    fn lock_engine(&self) -> MutexGuard<'_, Option<LoadedEngine>> {
        self.engine.lock().unwrap_or_else(|poisoned| {
            warn!("Engine mutex was poisoned by a previous panic, recovering");
            poisoned.into_inner()
        })
    }

    pub fn is_model_loaded(&self) -> bool {
        let engine = self.lock_engine();
        engine.is_some()
    }

    /// Atomically check whether a model load is in progress and, if not, mark
    /// one as starting. Returns a [`LoadingGuard`] whose [`Drop`] impl will
    /// clear the flag and wake waiters. Returns `None` if a load is already in
    /// progress.
    pub fn try_start_loading(&self) -> Option<LoadingGuard> {
        let mut is_loading = self.is_loading.lock().unwrap();
        if *is_loading {
            return None;
        }
        *is_loading = true;
        Some(LoadingGuard {
            is_loading: self.is_loading.clone(),
            loading_condvar: self.loading_condvar.clone(),
        })
    }

    pub fn unload_model(&self) -> Result<()> {
        let unload_start = std::time::Instant::now();
        debug!("Starting to unload model");

        {
            let mut engine = self.lock_engine();
            // Dropping the engine frees all resources
            *engine = None;
        }
        {
            let mut current_model = self.current_model_id.lock().unwrap();
            *current_model = None;
        }

        // Emit unloaded event
        let _ = self.app_handle.emit(
            "model-state-changed",
            ModelStateEvent {
                event_type: "unloaded".to_string(),
                model_id: None,
                model_name: None,
                error: None,
            },
        );

        let unload_duration = unload_start.elapsed();
        debug!(
            "Model unloaded manually (took {}ms)",
            unload_duration.as_millis()
        );
        Ok(())
    }

    fn now_ms() -> u64 {
        SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)
            .unwrap()
            .as_millis() as u64
    }

    /// Reset the idle timer to now.
    fn touch_activity(&self) {
        self.last_activity.store(Self::now_ms(), Ordering::Relaxed);
    }

    /// Unloads the model immediately if the setting is enabled and the model is loaded
    pub fn maybe_unload_immediately(&self, context: &str) {
        let settings = get_settings(&self.app_handle);
        if settings.model_unload_timeout == ModelUnloadTimeout::Immediately
            && self.is_model_loaded()
        {
            info!("Immediately unloading model after {}", context);
            if let Err(e) = self.unload_model() {
                warn!("Failed to immediately unload model: {}", e);
            }
        }
    }

    pub fn load_model(&self, model_id: &str) -> Result<()> {
        let load_start = std::time::Instant::now();
        debug!("Starting to load model: {}", model_id);

        // Emit loading started event
        let _ = self.app_handle.emit(
            "model-state-changed",
            ModelStateEvent {
                event_type: "loading_started".to_string(),
                model_id: Some(model_id.to_string()),
                model_name: None,
                error: None,
            },
        );

        let model_info = self
            .model_manager
            .get_model_info(model_id)
            .ok_or_else(|| anyhow::anyhow!("Model not found: {}", model_id))?;

        if !model_info.is_downloaded {
            let error_msg = "Model not downloaded";
            let _ = self.app_handle.emit(
                "model-state-changed",
                ModelStateEvent {
                    event_type: "loading_failed".to_string(),
                    model_id: Some(model_id.to_string()),
                    model_name: Some(model_info.name.clone()),
                    error: Some(error_msg.to_string()),
                },
            );
            return Err(anyhow::anyhow!(error_msg));
        }

        let model_path = self.model_manager.get_model_path(model_id)?;

        // Create appropriate engine based on model type
        let emit_loading_failed = |error_msg: &str| {
            let _ = self.app_handle.emit(
                "model-state-changed",
                ModelStateEvent {
                    event_type: "loading_failed".to_string(),
                    model_id: Some(model_id.to_string()),
                    model_name: Some(model_info.name.clone()),
                    error: Some(error_msg.to_string()),
                },
            );
        };

        let loaded_engine = match model_info.engine_type {
            EngineType::Whisper => {
                let engine = WhisperEngine::load(&model_path).map_err(|e| {
                    let error_msg = format!("Failed to load whisper model {}: {}", model_id, e);
                    emit_loading_failed(&error_msg);
                    anyhow::anyhow!(error_msg)
                })?;
                LoadedEngine::Whisper(engine)
            }
            EngineType::Parakeet => {
                let engine =
                    ParakeetModel::load(&model_path, &Quantization::Int8).map_err(|e| {
                        let error_msg =
                            format!("Failed to load parakeet model {}: {}", model_id, e);
                        emit_loading_failed(&error_msg);
                        anyhow::anyhow!(error_msg)
                    })?;
                LoadedEngine::Parakeet(engine)
            }
            EngineType::Moonshine => {
                let engine = MoonshineModel::load(
                    &model_path,
                    MoonshineVariant::Base,
                    &Quantization::default(),
                )
                .map_err(|e| {
                    let error_msg = format!("Failed to load moonshine model {}: {}", model_id, e);
                    emit_loading_failed(&error_msg);
                    anyhow::anyhow!(error_msg)
                })?;
                LoadedEngine::Moonshine(engine)
            }
            EngineType::MoonshineStreaming => {
                let engine = StreamingModel::load(&model_path, 0, &Quantization::default())
                    .map_err(|e| {
                        let error_msg = format!(
                            "Failed to load moonshine streaming model {}: {}",
                            model_id, e
                        );
                        emit_loading_failed(&error_msg);
                        anyhow::anyhow!(error_msg)
                    })?;
                LoadedEngine::MoonshineStreaming(engine)
            }
            EngineType::SenseVoice => {
                let engine =
                    SenseVoiceModel::load(&model_path, &Quantization::Int8).map_err(|e| {
                        let error_msg =
                            format!("Failed to load SenseVoice model {}: {}", model_id, e);
                        emit_loading_failed(&error_msg);
                        anyhow::anyhow!(error_msg)
                    })?;
                LoadedEngine::SenseVoice(engine)
            }
            EngineType::GigaAM => {
                let engine = GigaAMModel::load(&model_path, &Quantization::Int8).map_err(|e| {
                    let error_msg = format!("Failed to load gigaam model {}: {}", model_id, e);
                    emit_loading_failed(&error_msg);
                    anyhow::anyhow!(error_msg)
                })?;
                LoadedEngine::GigaAM(engine)
            }
            EngineType::Canary => {
                let engine = CanaryModel::load(&model_path, &Quantization::Int8).map_err(|e| {
                    let error_msg = format!("Failed to load canary model {}: {}", model_id, e);
                    emit_loading_failed(&error_msg);
                    anyhow::anyhow!(error_msg)
                })?;
                LoadedEngine::Canary(engine)
            }
            EngineType::Cohere => {
                let engine = CohereModel::load(&model_path, &Quantization::Int8).map_err(|e| {
                    let error_msg = format!("Failed to load cohere model {}: {}", model_id, e);
                    emit_loading_failed(&error_msg);
                    anyhow::anyhow!(error_msg)
                })?;
                LoadedEngine::Cohere(engine)
            }
        };

        // Update the current engine and model ID
        {
            let mut engine = self.lock_engine();
            *engine = Some(loaded_engine);
        }
        {
            let mut current_model = self.current_model_id.lock().unwrap();
            *current_model = Some(model_id.to_string());
        }

        // Reset idle timer so the watcher doesn't immediately unload a just-loaded model
        self.touch_activity();

        // Emit loading completed event
        let _ = self.app_handle.emit(
            "model-state-changed",
            ModelStateEvent {
                event_type: "loading_completed".to_string(),
                model_id: Some(model_id.to_string()),
                model_name: Some(model_info.name.clone()),
                error: None,
            },
        );

        let load_duration = load_start.elapsed();
        debug!(
            "Successfully loaded transcription model: {} (took {}ms)",
            model_id,
            load_duration.as_millis()
        );
        Ok(())
    }

    /// Kicks off the model loading in a background thread if it's not already loaded
    pub fn initiate_model_load(&self) {
        let mut is_loading = self.is_loading.lock().unwrap();
        if *is_loading || self.is_model_loaded() {
            return;
        }

        *is_loading = true;
        let self_clone = self.clone();
        thread::spawn(move || {
            let settings = get_settings(&self_clone.app_handle);
            if let Err(e) = self_clone.load_model(&settings.selected_model) {
                error!("Failed to load model: {}", e);
            }
            let mut is_loading = self_clone.is_loading.lock().unwrap();
            *is_loading = false;
            self_clone.loading_condvar.notify_all();
        });
    }

    pub fn get_current_model(&self) -> Option<String> {
        let current_model = self.current_model_id.lock().unwrap();
        current_model.clone()
    }

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
            let mut is_loading = self.is_loading.lock().unwrap();
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
                    match &mut engine {
                        LoadedEngine::Whisper(whisper_engine) => {
                            let params = WhisperInferenceParams {
                                language: normalized_language.clone(),
                                translate: settings.translate_to_english,
                                initial_prompt: if settings.custom_words.is_empty() {
                                    None
                                } else {
                                    Some(settings.custom_words.join(", "))
                                },
                                ..Default::default()
                            };

                            whisper_engine
                                .transcribe_with(&audio, &params)
                                .map_err(|e| anyhow::anyhow!("Whisper transcription failed: {}", e))
                        }
                        LoadedEngine::Parakeet(parakeet_engine) => {
                            let params = ParakeetParams {
                                timestamp_granularity: Some(TimestampGranularity::Word),
                                ..Default::default()
                            };
                            parakeet_engine
                                .transcribe_with(&audio, &params)
                                .map_err(|e| {
                                    anyhow::anyhow!("Parakeet transcription failed: {}", e)
                                })
                        }
                        LoadedEngine::Moonshine(moonshine_engine) => moonshine_engine
                            .transcribe(&audio, &TranscribeOptions::default())
                            .map_err(|e| anyhow::anyhow!("Moonshine transcription failed: {}", e)),
                        LoadedEngine::MoonshineStreaming(streaming_engine) => streaming_engine
                            .transcribe(&audio, &TranscribeOptions::default())
                            .map_err(|e| {
                                anyhow::anyhow!("Moonshine streaming transcription failed: {}", e)
                            }),
                        LoadedEngine::SenseVoice(sense_voice_engine) => {
                            let params = SenseVoiceParams {
                                language: normalized_language.clone(),
                                use_itn: Some(true),
                            };
                            sense_voice_engine
                                .transcribe_with(&audio, &params)
                                .map_err(|e| {
                                    anyhow::anyhow!("SenseVoice transcription failed: {}", e)
                                })
                        }
                        LoadedEngine::GigaAM(gigaam_engine) => gigaam_engine
                            .transcribe(&audio, &TranscribeOptions::default())
                            .map_err(|e| anyhow::anyhow!("GigaAM transcription failed: {}", e)),
                        LoadedEngine::Canary(canary_engine) => {
                            let options = TranscribeOptions {
                                language: normalized_language.clone(),
                                translate: settings.translate_to_english,
                                ..Default::default()
                            };
                            canary_engine
                                .transcribe(&audio, &options)
                                .map_err(|e| anyhow::anyhow!("Canary transcription failed: {}", e))
                        }
                        LoadedEngine::Cohere(cohere_engine) => {
                            let options = TranscribeOptions {
                                language: normalized_language.clone(),
                                ..Default::default()
                            };
                            cohere_engine
                                .transcribe(&audio, &options)
                                .map_err(|e| anyhow::anyhow!("Cohere transcription failed: {}", e))
                        }
                    }
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
                let comma = if i + 1 < normalized.words.len() { "," } else { "" };
                let text_escaped = w.text.replace('\\', "\\\\").replace('"', "\\\"");
                buf.push_str(&format!(
                    "  {{\"i\":{},\"text\":\"{}\",\"start_us\":{},\"end_us\":{},\"confidence\":{}}}{}\n",
                    i, text_escaped, w.start_us, w.end_us, w.confidence, comma
                ));
            }
            buf.push_str("]\n");
            match std::fs::write(&path, &buf) {
                Ok(_) => info!("TEMP-BLEED-DEBUG: wrote {} words to {}", normalized.words.len(), path),
                Err(e) => info!("TEMP-BLEED-DEBUG: failed to write {}: {}", path, e),
            }
        }
        Ok(normalized)
    }
}

impl Drop for TranscriptionManager {
    fn drop(&mut self) {
        // Skip shutdown unless this is the very last clone. TranscriptionManager
        // is cloned by initiate_model_load() and the watcher thread — those
        // clones dropping must not kill the watcher. The watcher thread holds
        // its own clone, so engine's strong_count is always >= 2 while the
        // watcher is alive. When it reaches 1, only this instance remains
        // and we can safely shut down.
        if Arc::strong_count(&self.engine) > 1 {
            return;
        }

        // Signal the watcher thread to shutdown
        self.shutdown_signal.store(true, Ordering::Relaxed);

        // Wait for the thread to finish gracefully
        if let Some(handle) = self.watcher_handle.lock().unwrap().take() {
            if let Err(e) = handle.join() {
                warn!("Failed to join idle watcher thread: {:?}", e);
            } else {
                debug!("Idle watcher thread joined successfully");
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // ── LoadingGuard ────────────────────────────────────────────────

    #[test]
    fn loading_guard_clears_flag_on_drop() {
        let is_loading = Arc::new(Mutex::new(true));
        let condvar = Arc::new(Condvar::new());

        let guard = LoadingGuard {
            is_loading: is_loading.clone(),
            loading_condvar: condvar.clone(),
        };

        assert!(*is_loading.lock().unwrap());
        drop(guard);
        assert!(!*is_loading.lock().unwrap());
    }

    #[test]
    fn loading_guard_notifies_waiters_on_drop() {
        let is_loading = Arc::new(Mutex::new(true));
        let condvar = Arc::new(Condvar::new());

        let guard = LoadingGuard {
            is_loading: is_loading.clone(),
            loading_condvar: condvar.clone(),
        };

        let is_loading_clone = is_loading.clone();
        let condvar_clone = condvar.clone();
        let waiter = thread::spawn(move || {
            let mut lock = is_loading_clone.lock().unwrap();
            while *lock {
                let (guard, timeout) = condvar_clone
                    .wait_timeout(lock, Duration::from_secs(5))
                    .unwrap();
                lock = guard;
                if timeout.timed_out() {
                    panic!("Timed out waiting for loading guard drop");
                }
            }
        });

        // Small delay so the waiter thread parks on the condvar
        thread::sleep(Duration::from_millis(50));
        drop(guard);

        waiter.join().expect("Waiter thread panicked");
        assert!(!*is_loading.lock().unwrap());
    }

    #[test]
    fn loading_guard_clears_flag_even_when_already_false() {
        let is_loading = Arc::new(Mutex::new(false));
        let condvar = Arc::new(Condvar::new());

        let guard = LoadingGuard {
            is_loading: is_loading.clone(),
            loading_condvar: condvar.clone(),
        };
        drop(guard);
        assert!(!*is_loading.lock().unwrap());
    }

    // ── now_ms ──────────────────────────────────────────────────────

    #[test]
    fn now_ms_returns_plausible_epoch_millis() {
        let ts = TranscriptionManager::now_ms();
        // Should be well past year-2020 in millis (1_577_836_800_000)
        assert!(ts > 1_577_836_800_000, "timestamp too small: {}", ts);
    }

    #[test]
    fn now_ms_is_monotonic_over_short_interval() {
        let t1 = TranscriptionManager::now_ms();
        thread::sleep(Duration::from_millis(10));
        let t2 = TranscriptionManager::now_ms();
        assert!(t2 >= t1, "expected t2 ({}) >= t1 ({})", t2, t1);
    }

    // ── get_available_accelerators ──────────────────────────────────

    #[test]
    fn available_accelerators_whisper_has_all_options() {
        let accel = get_available_accelerators();
        assert!(accel.whisper.contains(&"auto".to_string()));
        assert!(accel.whisper.contains(&"cpu".to_string()));
        assert!(accel.whisper.contains(&"gpu".to_string()));
        assert_eq!(accel.whisper.len(), 3);
    }

    #[test]
    fn available_accelerators_ort_is_non_empty() {
        let accel = get_available_accelerators();
        assert!(!accel.ort.is_empty(), "ORT should have at least one option");
    }

    // ── ModelStateEvent serialization ───────────────────────────────

    #[test]
    fn model_state_event_serializes_all_fields() {
        let event = ModelStateEvent {
            event_type: "loading_started".to_string(),
            model_id: Some("whisper-base".to_string()),
            model_name: Some("Whisper Base".to_string()),
            error: None,
        };
        let json = serde_json::to_value(&event).unwrap();
        assert_eq!(json["event_type"], "loading_started");
        assert_eq!(json["model_id"], "whisper-base");
        assert_eq!(json["model_name"], "Whisper Base");
        assert!(json["error"].is_null());
    }

    #[test]
    fn model_state_event_serializes_error() {
        let event = ModelStateEvent {
            event_type: "loading_failed".to_string(),
            model_id: Some("bad-model".to_string()),
            model_name: None,
            error: Some("File not found".to_string()),
        };
        let json = serde_json::to_value(&event).unwrap();
        assert_eq!(json["event_type"], "loading_failed");
        assert_eq!(json["error"], "File not found");
        assert!(json["model_name"].is_null());
    }

    #[test]
    fn model_state_event_unloaded_has_no_ids() {
        let event = ModelStateEvent {
            event_type: "unloaded".to_string(),
            model_id: None,
            model_name: None,
            error: None,
        };
        let json = serde_json::to_value(&event).unwrap();
        assert_eq!(json["event_type"], "unloaded");
        assert!(json["model_id"].is_null());
    }

    // ── GpuDeviceOption serialization ───────────────────────────────

    #[test]
    fn gpu_device_option_serializes() {
        let opt = GpuDeviceOption {
            id: 0,
            name: "Test GPU".to_string(),
            total_vram_mb: 8192,
        };
        let json = serde_json::to_value(&opt).unwrap();
        assert_eq!(json["id"], 0);
        assert_eq!(json["name"], "Test GPU");
        assert_eq!(json["total_vram_mb"], 8192);
    }

    // ── AvailableAccelerators serialization ─────────────────────────

    #[test]
    fn available_accelerators_serializes() {
        let accel = AvailableAccelerators {
            whisper: vec!["auto".to_string(), "cpu".to_string()],
            ort: vec!["cpu".to_string()],
            gpu_devices: vec![],
        };
        let json = serde_json::to_value(&accel).unwrap();
        assert_eq!(json["whisper"].as_array().unwrap().len(), 2);
        assert_eq!(json["ort"].as_array().unwrap().len(), 1);
        assert!(json["gpu_devices"].as_array().unwrap().is_empty());
    }

    // ── ModelUnloadTimeout (tested here for proximity to idle logic) ─

    #[test]
    fn timeout_never_returns_none() {
        assert_eq!(ModelUnloadTimeout::Never.to_seconds(), None);
    }

    #[test]
    fn timeout_immediately_returns_zero() {
        assert_eq!(ModelUnloadTimeout::Immediately.to_seconds(), Some(0));
    }

    #[test]
    fn timeout_sec15_returns_15() {
        assert_eq!(ModelUnloadTimeout::Sec15.to_seconds(), Some(15));
    }

    #[test]
    fn timeout_min5_returns_300() {
        assert_eq!(ModelUnloadTimeout::Min5.to_seconds(), Some(300));
    }

    #[test]
    fn timeout_hour1_returns_3600() {
        assert_eq!(ModelUnloadTimeout::Hour1.to_seconds(), Some(3600));
    }

    #[test]
    fn timeout_default_is_min5() {
        assert_eq!(ModelUnloadTimeout::default(), ModelUnloadTimeout::Min5);
    }

    // ── Idle-timeout arithmetic (mirrors watcher logic) ─────────────

    #[test]
    fn idle_detection_triggers_when_elapsed_exceeds_limit() {
        // Simulate the idle watcher's comparison logic
        let last_activity_ms = 1_000_000u64;
        let now_ms = 1_400_000u64; // 400 seconds later
        let limit_seconds = 300u64; // 5 minutes

        let idle_ms = now_ms.saturating_sub(last_activity_ms);
        let limit_ms = limit_seconds * 1000;

        assert!(
            idle_ms > limit_ms,
            "should detect idle after 400s > 300s limit"
        );
    }

    #[test]
    fn idle_detection_does_not_trigger_when_within_limit() {
        let last_activity_ms = 1_000_000u64;
        let now_ms = 1_100_000u64; // 100 seconds later
        let limit_seconds = 300u64;

        let idle_ms = now_ms.saturating_sub(last_activity_ms);
        let limit_ms = limit_seconds * 1000;

        assert!(
            idle_ms <= limit_ms,
            "should not detect idle after 100s < 300s limit"
        );
    }

    #[test]
    fn saturating_sub_handles_clock_wrap() {
        // If now < last (e.g. clock adjustment), saturating_sub returns 0
        let last_activity_ms = 2_000_000u64;
        let now_ms = 1_000_000u64;
        let idle_ms = now_ms.saturating_sub(last_activity_ms);
        assert_eq!(idle_ms, 0, "saturating_sub should prevent underflow");
    }

    // ── Loading-flag state machine (simulates try_start_loading) ────

    #[test]
    fn try_start_loading_pattern_grants_first_caller() {
        let is_loading = Arc::new(Mutex::new(false));
        let condvar = Arc::new(Condvar::new());

        // Simulate try_start_loading: CAS on the flag
        {
            let mut flag = is_loading.lock().unwrap();
            assert!(!*flag, "should start unlocked");
            *flag = true;
        }

        // Now construct guard (simulates successful try_start_loading)
        let guard = LoadingGuard {
            is_loading: is_loading.clone(),
            loading_condvar: condvar.clone(),
        };
        assert!(*is_loading.lock().unwrap());

        drop(guard);
        assert!(!*is_loading.lock().unwrap());
    }

    #[test]
    fn try_start_loading_pattern_rejects_second_caller() {
        let is_loading = Arc::new(Mutex::new(false));
        let condvar = Arc::new(Condvar::new());

        // First caller succeeds
        {
            let mut flag = is_loading.lock().unwrap();
            *flag = true;
        }
        let _guard = LoadingGuard {
            is_loading: is_loading.clone(),
            loading_condvar: condvar.clone(),
        };

        // Second caller sees is_loading == true and would return None
        {
            let flag = is_loading.lock().unwrap();
            assert!(*flag, "second caller should see loading in progress");
        }
    }

    // ── Touch-activity pattern ──────────────────────────────────────

    #[test]
    fn atomic_activity_timestamp_updates() {
        let last_activity = AtomicU64::new(0);
        assert_eq!(last_activity.load(Ordering::Relaxed), 0);

        let now = TranscriptionManager::now_ms();
        last_activity.store(now, Ordering::Relaxed);
        assert_eq!(last_activity.load(Ordering::Relaxed), now);
    }
}
