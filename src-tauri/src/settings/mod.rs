//! Settings module.
//!
//! Behavior-preserving split of the former monolithic `settings.rs`:
//! - [`types`]    — enums, structs, and trait impls for the on-disk schema
//! - [`defaults`] — `default_*` factories + `get_default_settings` +
//!   `ensure_post_process_defaults` migration
//! - [`sanitize`] — validation helpers for post-process provider inputs
//! - [`io`]       — Tauri store read/write + convenience accessors
//!
//! External callers keep using `crate::settings::<Name>` paths; every
//! previously-public item is re-exported below.

pub const APPLE_INTELLIGENCE_PROVIDER_ID: &str = "apple_intelligence";
pub const APPLE_INTELLIGENCE_DEFAULT_MODEL_ID: &str = "Apple Intelligence";
pub const OLLAMA_PROVIDER_ID: &str = "ollama";
pub const LM_STUDIO_PROVIDER_ID: &str = "lm_studio";
pub const CUSTOM_LOCAL_PROVIDER_ID: &str = "custom";
pub const SETTINGS_STORE_PATH: &str = "settings_store.json";

mod defaults;
mod io;
mod sanitize;
mod types;

pub use defaults::get_default_settings;
pub use io::{get_history_limit, get_recording_retention_period, get_settings, write_settings};
pub use sanitize::{
    is_local_post_process_provider, sanitize_local_post_process_base_url,
    sanitize_post_process_model,
};
pub use types::{
    AppSettings, CaptionFontFamily, LLMPrompt, LogLevel, ModelUnloadTimeout, OrtAcceleratorSetting,
    PostProcessProvider, RecordingRetentionPeriod, WhisperAcceleratorSetting,
};

#[cfg(test)]
mod tests {
    use super::defaults::validate_settings;
    use super::types::SecretMap;
    use super::*;
    use std::collections::HashMap;

    #[test]
    fn default_settings_disable_experimental_simplify_mode() {
        let settings = get_default_settings();
        assert!(!settings.experimental_simplify_mode);
    }

    #[test]
    fn debug_output_redacts_api_keys() {
        let mut settings = get_default_settings();
        settings
            .post_process_api_keys
            .insert("openai".to_string(), "sk-proj-secret-key-12345".to_string());
        settings.post_process_api_keys.insert(
            "anthropic".to_string(),
            "sk-ant-secret-key-67890".to_string(),
        );
        settings
            .post_process_api_keys
            .insert("empty_provider".to_string(), "".to_string());

        let debug_output = format!("{:?}", settings);

        assert!(!debug_output.contains("sk-proj-secret-key-12345"));
        assert!(!debug_output.contains("sk-ant-secret-key-67890"));
        assert!(debug_output.contains("[REDACTED]"));
    }

    #[test]
    fn secret_map_debug_redacts_values() {
        let map = SecretMap(HashMap::from([("key".into(), "secret".into())]));
        let out = format!("{:?}", map);
        assert!(!out.contains("secret"));
        assert!(out.contains("[REDACTED]"));
    }

    #[test]
    fn default_post_process_provider_prefers_local_ollama() {
        let settings = get_default_settings();
        assert_eq!(settings.post_process_provider_id, OLLAMA_PROVIDER_ID);

        let ollama = settings
            .post_process_providers
            .iter()
            .find(|provider| provider.id == OLLAMA_PROVIDER_ID)
            .expect("ollama provider should exist");
        assert!(ollama.local_only);
        assert!(!ollama.requires_api_key);
    }

    #[test]
    fn sanitize_local_base_url_rejects_non_loopback_hosts() {
        let result = sanitize_local_post_process_base_url("https://example.com/v1");
        assert!(result.is_err());
    }

    #[test]
    fn sanitize_local_base_url_normalizes_trailing_slash() {
        let result = sanitize_local_post_process_base_url("http://127.0.0.1:11434/v1/");
        assert_eq!(
            result.expect("expected valid loopback URL"),
            "http://127.0.0.1:11434/v1"
        );
    }

    #[test]
    fn sanitize_post_process_model_rejects_control_characters() {
        let result = sanitize_post_process_model("llama3\nbad");
        assert!(result.is_err());
    }

    #[test]
    fn test_validate_settings_clamps_position() {
        let mut s = get_default_settings();
        s.caption_position = 150;
        validate_settings(&mut s);
        assert_eq!(s.caption_position, 90);
    }

    #[test]
    fn test_validate_settings_fixes_invalid_color() {
        let mut s = get_default_settings();
        s.caption_text_color = "not-a-color".to_string();
        validate_settings(&mut s);
        assert_eq!(s.caption_text_color, "#FFFFFF");
    }

    #[test]
    fn test_validate_settings_allows_valid_colors() {
        let mut s = get_default_settings();
        s.caption_text_color = "#FF0000".to_string();
        s.caption_bg_color = "#00FF00AA".to_string();
        validate_settings(&mut s);
        assert_eq!(s.caption_text_color, "#FF0000");
        assert_eq!(s.caption_bg_color, "#00FF00AA");
    }

    #[test]
    fn test_validate_settings_clamps_volume() {
        let mut s = get_default_settings();
        s.export_volume_db = 100.0;
        validate_settings(&mut s);
        assert_eq!(s.export_volume_db, 24.0);
    }

    #[test]
    fn test_settings_version_present() {
        let s = get_default_settings();
        assert_eq!(s.settings_version, 1);
    }
}
