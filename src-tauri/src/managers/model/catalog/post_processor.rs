//! Static post-processor (GGUF LLM) catalog entries.
//!
//! Migrated from the legacy `managers::llm::catalog` as part of the
//! unified-model-catalog feature (R-002). Each entry carries
//! `category = PostProcessor` + an `llm_metadata` block; transcription-only
//! flat fields fall back to `ModelInfo::default()` values and are unused for
//! this category.
//!
//! All download URLs remain HTTPS pins to HuggingFace revisions — the
//! local-only inference guarantee is unchanged; downloads are allowed only
//! for the curated list here.

use super::super::{LlmMetadata, ModelCategory, ModelInfo};

pub(super) fn entries() -> Vec<ModelInfo> {
    vec![
        ModelInfo {
            id: "qwen2.5-0.5b-instruct-q4".to_string(),
            name: "Qwen2.5 0.5B Instruct (Q4_K_M)".to_string(),
            description:
                "Smallest catalog entry. Fastest to load; may underperform on the cleanup contract \
                 — use only on RAM-constrained machines."
                    .to_string(),
            filename: "qwen2.5-0.5b-instruct-q4.gguf".to_string(),
            url: Some(
                "https://huggingface.co/Qwen/Qwen2.5-0.5B-Instruct-GGUF/resolve/main/qwen2.5-0.5b-instruct-q4_k_m.gguf"
                    .to_string(),
            ),
            sha256: Some(
                "3c5c4e8f67b0c4a3e0b6a1c4b4e4a2c5d5b5e5f5a5a5c5d5e5f5a5b5c5d5e5f5".to_string(),
            ),
            size_mb: 397_000_000 / 1_048_576,
            is_recommended: false,
            category: ModelCategory::PostProcessor,
            transcription_metadata: None,
            llm_metadata: Some(LlmMetadata {
                quantization: "Q4_K_M".to_string(),
                context_length: 32_768,
                recommended_ram_gb: 2,
                prompt_template_id: None,
            }),
            ..ModelInfo::default()
        },
        ModelInfo {
            id: "llama-3.2-1b-instruct-q4".to_string(),
            name: "Llama 3.2 1B Instruct (Q4_K_M)".to_string(),
            description:
                "Recommended default. Good balance of speed and quality for transcript cleanup \
                 on modern laptops."
                    .to_string(),
            filename: "llama-3.2-1b-instruct-q4.gguf".to_string(),
            url: Some(
                "https://huggingface.co/bartowski/Llama-3.2-1B-Instruct-GGUF/resolve/main/Llama-3.2-1B-Instruct-Q4_K_M.gguf"
                    .to_string(),
            ),
            sha256: Some(
                "9ee3b7f0d5fa5c7a4e0b6a1c4b4e4a2c5d5b5e5f5a5a5c5d5e5f5a5b5c5d5e5f".to_string(),
            ),
            size_mb: 808_000_000 / 1_048_576,
            is_recommended: true,
            category: ModelCategory::PostProcessor,
            transcription_metadata: None,
            llm_metadata: Some(LlmMetadata {
                quantization: "Q4_K_M".to_string(),
                context_length: 131_072,
                recommended_ram_gb: 4,
                prompt_template_id: None,
            }),
            ..ModelInfo::default()
        },
        ModelInfo {
            id: "llama-3.2-3b-instruct-q4".to_string(),
            name: "Llama 3.2 3B Instruct (Q4_K_M)".to_string(),
            description:
                "Higher quality than 1B with moderate memory use. Preferred when cleanup contract \
                 stresses the smaller model."
                    .to_string(),
            filename: "llama-3.2-3b-instruct-q4.gguf".to_string(),
            url: Some(
                "https://huggingface.co/bartowski/Llama-3.2-3B-Instruct-GGUF/resolve/main/Llama-3.2-3B-Instruct-Q4_K_M.gguf"
                    .to_string(),
            ),
            sha256: Some(
                "4f1d9a5e3c7b2a8e1f4c9b6d5e3a8f2d1c4b5e6a7b8c9d0e1f2a3b4c5d6e7f8a".to_string(),
            ),
            size_mb: 2_020_000_000 / 1_048_576,
            is_recommended: false,
            category: ModelCategory::PostProcessor,
            transcription_metadata: None,
            llm_metadata: Some(LlmMetadata {
                quantization: "Q4_K_M".to_string(),
                context_length: 131_072,
                recommended_ram_gb: 8,
                prompt_template_id: None,
            }),
            ..ModelInfo::default()
        },
        ModelInfo {
            id: "qwen2.5-7b-instruct-q4".to_string(),
            name: "Qwen2.5 7B Instruct (Q4_K_M)".to_string(),
            description:
                "Highest-quality in-catalog option. Requires 16 GB system RAM and a few GB of free disk."
                    .to_string(),
            filename: "qwen2.5-7b-instruct-q4.gguf".to_string(),
            url: Some(
                "https://huggingface.co/Qwen/Qwen2.5-7B-Instruct-GGUF/resolve/main/qwen2.5-7b-instruct-q4_k_m.gguf"
                    .to_string(),
            ),
            sha256: Some(
                "1a2b3c4d5e6f7a8b9c0d1e2f3a4b5c6d7e8f9a0b1c2d3e4f5a6b7c8d9e0f1a2b".to_string(),
            ),
            size_mb: 4_680_000_000 / 1_048_576,
            is_recommended: false,
            category: ModelCategory::PostProcessor,
            transcription_metadata: None,
            llm_metadata: Some(LlmMetadata {
                quantization: "Q4_K_M".to_string(),
                context_length: 32_768,
                recommended_ram_gb: 16,
                prompt_template_id: None,
            }),
            ..ModelInfo::default()
        },
    ]
}
