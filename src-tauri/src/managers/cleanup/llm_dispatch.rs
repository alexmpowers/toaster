// LLM-dispatch helper for transcript cleanup.
//
// Extracted from `cleanup/mod.rs` so the manager file only owns orchestration.
// Provides a single `try_llm_attempt` entry point covering both the
// structured-output (with-schema) attempt and the legacy (without-schema)
// attempt, including the local-openai-provider reasoning special case and
// the legacy retry loop.

use log::{debug, error, warn};
use std::time::Duration;

use super::prompts::{
    build_cleanup_contract_schema, build_cleanup_contract_system_prompt,
    build_cleanup_legacy_prompt,
};
use super::{
    validate_cleanup_candidate, CleanupContractResponse, CLEANUP_TRANSCRIPTION_FIELD,
    TRANSCRIPTION_FIELD,
};
use crate::llm_client::ReasoningConfig;
use crate::settings::PostProcessProvider;

/// Outcome of one LLM-dispatch attempt.
pub(super) enum AttemptOutcome {
    /// Attempt produced a validated cleanup output. Caller should return this.
    Success(String),
    /// Attempt did not yield a usable result. Caller should fall through to the
    /// next path (e.g. structured -> legacy) or preserve the original transcription.
    Fallback,
}

/// Parameters shared by both attempt shapes. Bundled into one struct so the
/// `try_llm_attempt` signature stays manageable.
pub(super) struct AttemptInputs<'a> {
    pub provider: &'a PostProcessProvider,
    pub api_key: String,
    pub model: &'a str,
    pub transcription: &'a str,
    pub prompt: &'a str,
    pub protected_tokens_for_prompt: &'a [String],
    pub local_openai_provider: bool,
    pub reasoning_effort: Option<String>,
    pub reasoning: Option<ReasoningConfig>,
}

/// Run one cleanup-LLM attempt. `use_schema = true` issues a structured-output
/// request and falls through to the legacy path on any failure. `use_schema =
/// false` issues the legacy prompt and, for the local OpenAI-compatible
/// provider, retries once on transient errors or validation failures.
pub(super) async fn try_llm_attempt(
    inputs: &AttemptInputs<'_>,
    use_schema: bool,
) -> AttemptOutcome {
    if use_schema {
        try_structured_attempt(inputs).await
    } else {
        try_legacy_attempt(inputs).await
    }
}

async fn try_structured_attempt(inputs: &AttemptInputs<'_>) -> AttemptOutcome {
    let provider = inputs.provider;
    debug!("Using structured outputs for provider '{}'", provider.id);

    let system_prompt =
        build_cleanup_contract_system_prompt(inputs.prompt, inputs.protected_tokens_for_prompt);
    let user_content = inputs.transcription.to_string();
    let json_schema = build_cleanup_contract_schema();

    match crate::llm_client::send_chat_completion_with_schema(
        provider,
        inputs.api_key.clone(),
        inputs.model,
        user_content,
        Some(system_prompt),
        Some(json_schema),
        inputs.reasoning_effort.clone(),
        inputs.reasoning.clone(),
    )
    .await
    {
        Ok(Some(content)) => match serde_json::from_str::<CleanupContractResponse>(&content) {
            Ok(contract_response) => match validate_cleanup_candidate(
                inputs.transcription,
                &contract_response.cleaned_transcription,
                Some(&contract_response),
            ) {
                Ok(validated) => {
                    debug!(
                        "Structured cleanup post-processing succeeded for provider '{}'. Output length: {} chars",
                        provider.id,
                        validated.len()
                    );
                    AttemptOutcome::Success(validated)
                }
                Err(validation_error) => {
                    warn!(
                        "Structured cleanup output rejected for provider '{}': {}. Falling back to legacy mode.",
                        provider.id, validation_error
                    );
                    AttemptOutcome::Fallback
                }
            },
            Err(contract_parse_error) => {
                warn!(
                    "Structured cleanup contract parse failed for provider '{}': {}. Attempting compatibility fallback.",
                    provider.id, contract_parse_error
                );

                let fallback_candidate = serde_json::from_str::<serde_json::Value>(&content)
                    .ok()
                    .and_then(|json| {
                        json.get(CLEANUP_TRANSCRIPTION_FIELD)
                            .and_then(|value| value.as_str())
                            .map(ToString::to_string)
                            .or_else(|| {
                                json.get(TRANSCRIPTION_FIELD)
                                    .and_then(|value| value.as_str())
                                    .map(ToString::to_string)
                            })
                    });

                if let Some(candidate) = fallback_candidate {
                    match validate_cleanup_candidate(inputs.transcription, &candidate, None) {
                        Ok(validated) => {
                            debug!(
                                "Structured compatibility fallback succeeded for provider '{}'. Output length: {} chars",
                                provider.id,
                                validated.len()
                            );
                            AttemptOutcome::Success(validated)
                        }
                        Err(validation_error) => {
                            warn!(
                                "Structured compatibility fallback rejected for provider '{}': {}. Falling back to legacy mode.",
                                provider.id, validation_error
                            );
                            AttemptOutcome::Fallback
                        }
                    }
                } else {
                    warn!(
                        "Structured response from provider '{}' did not contain '{}' or '{}'; falling back to legacy mode.",
                        provider.id, CLEANUP_TRANSCRIPTION_FIELD, TRANSCRIPTION_FIELD
                    );
                    AttemptOutcome::Fallback
                }
            }
        },
        Ok(None) => {
            warn!(
                "Structured output API returned no content for provider '{}'; falling back to legacy mode.",
                provider.id
            );
            AttemptOutcome::Fallback
        }
        Err(e) => {
            warn!(
                "Structured output call failed for provider '{}': {}. Falling back to legacy mode.",
                provider.id, e
            );
            AttemptOutcome::Fallback
        }
    }
}

async fn try_legacy_attempt(inputs: &AttemptInputs<'_>) -> AttemptOutcome {
    let provider = inputs.provider;
    let processed_prompt = build_cleanup_legacy_prompt(
        inputs.prompt,
        inputs.transcription,
        inputs.protected_tokens_for_prompt,
    );
    debug!("Processed prompt length: {} chars", processed_prompt.len());

    let max_attempts = if inputs.local_openai_provider { 2 } else { 1 };
    for attempt in 1..=max_attempts {
        match crate::llm_client::send_chat_completion(
            provider,
            inputs.api_key.clone(),
            inputs.model,
            processed_prompt.clone(),
            inputs.reasoning_effort.clone(),
            inputs.reasoning.clone(),
        )
        .await
        {
            Ok(Some(content)) => {
                match validate_cleanup_candidate(inputs.transcription, &content, None) {
                    Ok(validated) => {
                        debug!(
                            "LLM post-processing succeeded for provider '{}'. Output length: {} chars",
                            provider.id,
                            validated.len()
                        );
                        return AttemptOutcome::Success(validated);
                    }
                    Err(validation_error) => {
                        if inputs.local_openai_provider && attempt < max_attempts {
                            warn!(
                                "Legacy cleanup output rejected for local provider '{}' (attempt {}): {}. Retrying once.",
                                provider.id, attempt, validation_error
                            );
                            tokio::time::sleep(Duration::from_millis(250)).await;
                            continue;
                        }

                        warn!(
                            "Legacy cleanup output rejected for provider '{}': {}. Preserving original transcription.",
                            provider.id, validation_error
                        );
                        return AttemptOutcome::Fallback;
                    }
                }
            }
            Ok(None) => {
                error!(
                    "LLM post-processing returned no content for provider '{}'; preserving original transcription",
                    provider.id
                );
                return AttemptOutcome::Fallback;
            }
            Err(e) => {
                if inputs.local_openai_provider && attempt < max_attempts {
                    warn!(
                        "Transient local LLM error for provider '{}' (attempt {}): {}. Retrying once.",
                        provider.id, attempt, e
                    );
                    tokio::time::sleep(Duration::from_millis(250)).await;
                    continue;
                }

                error!(
                    "LLM post-processing failed for provider '{}': {}. Falling back to original transcription.",
                    provider.id, e
                );
                return AttemptOutcome::Fallback;
            }
        }
    }

    AttemptOutcome::Fallback
}
