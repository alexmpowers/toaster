//! Single-shot ASR engine dispatch — extracted from `job.rs` so both the
//! full-buffer path and the R-002 prefilter chunk path share one
//! implementation of the per-engine `match` (BLUEPRINT §AD-3 dual-path
//! invariant: one backend implementation, two consumers).
//!
//! No mutex acquisition, no panic catching, no settings reload — those
//! concerns stay in `job.rs`. This module is the pure
//! "engine + audio + options → TranscriptionResult" computation step.

use anyhow::Result;
use transcribe_rs::{
    onnx::{
        parakeet::{ParakeetParams, TimestampGranularity},
        sense_voice::SenseVoiceParams,
    },
    whisper_cpp::WhisperInferenceParams,
    SpeechModel, TranscribeOptions, TranscriptionResult,
};

use crate::settings::AppSettings;

use super::LoadedEngine;

/// Run a single ASR pass on `audio`. Used both directly (no-prefilter
/// path) and per-window (prefilter chunk loop). Returned timestamps are
/// always relative to the start of `audio` — chunk-to-file-time
/// remapping is the caller's responsibility (see `prefilter::run`).
pub(super) fn call_engine_chunk(
    engine: &mut LoadedEngine,
    audio: &[f32],
    settings: &AppSettings,
    normalized_language: &Option<String>,
) -> Result<TranscriptionResult> {
    match engine {
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
                .transcribe_with(audio, &params)
                .map_err(|e| anyhow::anyhow!("Whisper transcription failed: {}", e))
        }
        LoadedEngine::Parakeet(parakeet_engine) => {
            let params = ParakeetParams {
                timestamp_granularity: Some(TimestampGranularity::Word),
                ..Default::default()
            };
            parakeet_engine
                .transcribe_with(audio, &params)
                .map_err(|e| anyhow::anyhow!("Parakeet transcription failed: {}", e))
        }
        LoadedEngine::Moonshine(moonshine_engine) => moonshine_engine
            .transcribe(audio, &TranscribeOptions::default())
            .map_err(|e| anyhow::anyhow!("Moonshine transcription failed: {}", e)),
        LoadedEngine::MoonshineStreaming(streaming_engine) => streaming_engine
            .transcribe(audio, &TranscribeOptions::default())
            .map_err(|e| anyhow::anyhow!("Moonshine streaming transcription failed: {}", e)),
        LoadedEngine::SenseVoice(sense_voice_engine) => {
            let params = SenseVoiceParams {
                language: normalized_language.clone(),
                use_itn: Some(true),
            };
            sense_voice_engine
                .transcribe_with(audio, &params)
                .map_err(|e| anyhow::anyhow!("SenseVoice transcription failed: {}", e))
        }
        LoadedEngine::GigaAM(gigaam_engine) => gigaam_engine
            .transcribe(audio, &TranscribeOptions::default())
            .map_err(|e| anyhow::anyhow!("GigaAM transcription failed: {}", e)),
        LoadedEngine::Canary(canary_engine) => {
            let options = TranscribeOptions {
                language: normalized_language.clone(),
                translate: settings.translate_to_english,
                ..Default::default()
            };
            canary_engine
                .transcribe(audio, &options)
                .map_err(|e| anyhow::anyhow!("Canary transcription failed: {}", e))
        }
        LoadedEngine::Cohere(cohere_engine) => {
            let options = TranscribeOptions {
                language: normalized_language.clone(),
                ..Default::default()
            };
            cohere_engine
                .transcribe(audio, &options)
                .map_err(|e| anyhow::anyhow!("Cohere transcription failed: {}", e))
        }
    }
}
