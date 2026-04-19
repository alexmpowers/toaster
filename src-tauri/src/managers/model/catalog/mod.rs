//! Unified model catalog — aggregates transcription + post-processor entries.
//!
//! The legacy `catalog.rs` monolith split into:
//! - [`transcription`] — Whisper / Parakeet / Moonshine / SenseVoice / GigaAM /
//!   Canary / Cohere entries (all `category = Transcription`).
//! - [`post_processor`] — curated GGUF LLM entries (all
//!   `category = PostProcessor`). Migrated from the now-deprecated
//!   `managers::llm::catalog` per feature `unified-model-catalog` R-002.
//!
//! `build_static_catalog()` is called by `ModelManager::new` and contains
//! every entry keyed by id. `all()` returns the same set as a flat `Vec`
//! for code paths that prefer ordering over lookup. Custom-model discovery
//! (`.bin` files in the models dir) and `verify_sha256` live here because
//! they operate over the in-memory catalog rather than a specific category.

use std::collections::{HashMap, HashSet};
use std::fs;
use std::path::Path;

use anyhow::Result;
use log::{info, warn};

use super::{hash, EngineType, ModelInfo};

pub mod post_processor;
pub mod transcription;

/// Build the full catalog — both categories combined — keyed by model id.
/// Called once at `ModelManager::new`.
pub(super) fn build_static_catalog() -> HashMap<String, ModelInfo> {
    let mut out: HashMap<String, ModelInfo> = HashMap::new();
    for entry in transcription::entries() {
        out.insert(entry.id.clone(), entry);
    }
    for entry in post_processor::entries() {
        if out.insert(entry.id.clone(), entry).is_some() {
            warn!("Duplicate model id between transcription and post-processor catalogs");
        }
    }
    out
}

/// Flat view of every curated catalog entry across all categories.
/// Test-visible aggregator for `post_processor_entry_count_matches_legacy`
/// and the future unified command layer.
#[allow(dead_code)] // consumed by the unified command layer in umc-command-unify.
pub fn all() -> Vec<ModelInfo> {
    let mut v = transcription::entries();
    v.extend(post_processor::entries());
    v
}

/// Look up a post-processor entry by id from the static catalog.
/// Used by `managers::llm::LlmManager` (and its tests) for metadata
/// that is not on the runtime `ModelInfo` snapshot. This helper is the
/// direct replacement for the deleted `managers::llm::catalog::find_entry`.
pub fn find_post_processor(id: &str) -> Option<ModelInfo> {
    post_processor::entries().into_iter().find(|m| m.id == id)
}

/// Flat list of post-processor catalog entries. Used by tests.
pub fn post_processor_entries() -> Vec<ModelInfo> {
    post_processor::entries()
}

pub(super) fn discover_custom_whisper_models(
    models_dir: &Path,
    available_models: &mut HashMap<String, ModelInfo>,
) -> Result<()> {
    if !models_dir.exists() {
        return Ok(());
    }

    // Collect filenames of predefined Whisper file-based models to skip
    let predefined_filenames: HashSet<String> = available_models
        .values()
        .filter(|m| matches!(m.engine_type, EngineType::Whisper) && !m.is_directory)
        .map(|m| m.filename.clone())
        .collect();

    // Scan models directory for .bin files
    for entry in fs::read_dir(models_dir)? {
        let entry = match entry {
            Ok(e) => e,
            Err(e) => {
                warn!("Failed to read directory entry: {}", e);
                continue;
            }
        };

        let path = entry.path();

        // Only process .bin files (not directories)
        if !path.is_file() {
            continue;
        }

        let filename = match path.file_name().and_then(|s| s.to_str()) {
            Some(name) => name.to_string(),
            None => continue,
        };

        // Skip hidden files
        if filename.starts_with('.') {
            continue;
        }

        // Only process .bin files (Whisper GGML format).
        // This also excludes .partial downloads (e.g., "model.bin.partial").
        // If we add discovery for other formats, add a .partial check before this filter.
        if !filename.ends_with(".bin") {
            continue;
        }

        // Skip predefined model files
        if predefined_filenames.contains(&filename) {
            continue;
        }

        // Generate model ID from filename (remove .bin extension)
        let model_id = filename.trim_end_matches(".bin").to_string();

        // Skip if model ID already exists (shouldn't happen, but be safe)
        if available_models.contains_key(&model_id) {
            continue;
        }

        // Generate display name: replace - and _ with space, capitalize words
        let display_name = model_id
            .replace(['-', '_'], " ")
            .split_whitespace()
            .map(|word| {
                let mut chars = word.chars();
                match chars.next() {
                    None => String::new(),
                    Some(first) => first.to_uppercase().collect::<String>() + chars.as_str(),
                }
            })
            .collect::<Vec<_>>()
            .join(" ");

        // Get file size in MB
        let size_mb = match path.metadata() {
            Ok(meta) => meta.len() / (1024 * 1024),
            Err(e) => {
                warn!("Failed to get metadata for {}: {}", filename, e);
                0
            }
        };

        info!(
            "Discovered custom Whisper model: {} ({}, {} MB)",
            model_id, filename, size_mb
        );

        available_models.insert(
            model_id.clone(),
            ModelInfo {
                id: model_id,
                name: display_name,
                description: "Not officially supported".to_string(),
                filename,
                url: None,    // Custom models have no download URL
                sha256: None, // Custom models skip verification
                size_mb,
                is_downloaded: true, // Already present on disk
                is_downloading: false,
                partial_size: 0,
                is_directory: false,
                engine_type: EngineType::Whisper,
                accuracy_score: 0.0, // Sentinel: UI hides score bars when both are 0
                speed_score: 0.0,
                supports_translation: false,
                is_recommended: false,
                supported_languages: vec![],
                supports_language_selection: true,
                is_custom: true,
                category: super::ModelCategory::Transcription,
                transcription_metadata: None,
                llm_metadata: None,
            },
        );
    }

    Ok(())
}

/// Verifies the SHA256 of `path` against `expected_sha256` (if provided).
/// On mismatch or read error the partial file is deleted and an error is returned,
/// so the next download attempt always starts from a clean state.
/// When `expected_sha256` is `None` (custom user models) verification is skipped.
pub(super) fn verify_sha256(
    path: &Path,
    expected_sha256: Option<&str>,
    model_id: &str,
) -> Result<()> {
    hash::verify_sha256(path, expected_sha256, model_id)
}

#[cfg(test)]
mod catalog_aggregator_tests {
    use super::*;
    use crate::managers::model::ModelCategory;

    /// Snapshot of the LLM catalog length **before** the migration
    /// (`managers::llm::catalog::catalog()` returned 4 entries on
    /// 2026-04-18). Locked in here so that a drift in either direction
    /// — accidental deletion, silent addition — trips the gate.
    const POST_PROCESSOR_CATALOG_COUNT: usize = 4;

    #[test]
    fn post_processor_entry_count_matches_legacy() {
        let pp_count = all()
            .iter()
            .filter(|m| m.category == ModelCategory::PostProcessor)
            .count();
        assert_eq!(
            pp_count, POST_PROCESSOR_CATALOG_COUNT,
            "post-processor catalog count drifted from the pre-migration \
             legacy count ({}); update POST_PROCESSOR_CATALOG_COUNT only when \
             the PRD records a deliberate catalog change",
            POST_PROCESSOR_CATALOG_COUNT
        );
    }

    #[test]
    fn post_processor_entries_have_llm_metadata() {
        for entry in all()
            .into_iter()
            .filter(|m| m.category == ModelCategory::PostProcessor)
        {
            assert!(
                entry.llm_metadata.is_some(),
                "post-processor entry {} must carry llm_metadata",
                entry.id
            );
            assert!(
                entry.transcription_metadata.is_none(),
                "post-processor entry {} must not carry transcription_metadata",
                entry.id
            );
            let meta = entry.llm_metadata.as_ref().unwrap();
            assert!(!meta.quantization.is_empty());
            assert!(meta.context_length > 0);
            assert!(meta.recommended_ram_gb > 0);
            assert!(
                entry
                    .url
                    .as_ref()
                    .map(|u| u.starts_with("https://"))
                    .unwrap_or(false),
                "post-processor entry {} must have an https url",
                entry.id
            );
            assert_eq!(
                entry.sha256.as_ref().map(|s| s.len()).unwrap_or(0),
                64,
                "post-processor entry {} sha256 must be 64 hex chars",
                entry.id
            );
        }
    }

    #[test]
    fn exactly_one_post_processor_is_recommended_default() {
        let recs: Vec<_> = all()
            .into_iter()
            .filter(|m| m.category == ModelCategory::PostProcessor && m.is_recommended)
            .collect();
        assert_eq!(
            recs.len(),
            1,
            "post-processor catalog must have exactly one is_recommended=true entry; got {}",
            recs.len()
        );
    }
}
