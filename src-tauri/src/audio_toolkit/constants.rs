/// Fallback ASR input sample rate (Hz).
///
/// Historically this was a Whisper-only constant (`WHISPER_SAMPLE_RATE`),
/// but all current ASR engines in `transcribe-rs` accept 16 kHz input so
/// it now serves as a sensible *fallback* when an adapter (or ModelInfo)
/// declares no sample rate. Callers that know their model's native rate
/// should prefer `TranscriptionModelAdapter::capabilities().native_input_sample_rate_hz`
/// (or `ModelInfo::input_sample_rate_hz()`), not this constant.
pub const ASR_INPUT_SAMPLE_RATE_HZ_DEFAULT: u32 = 16000;
