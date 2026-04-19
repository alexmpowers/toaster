//! R-005 / AC-005-c — prefilter must degrade gracefully when the
//! Silero ONNX is absent.
//!
//! Pure integration test: exercises `try_open_silero` with a
//! non-existent path and `prefilter_speech_windows` with a scripted
//! always-voice VAD, validating that
//!   (a) missing model is `Ok(None)`, never `Err`, and
//!   (b) the windowing state machine over sustained speech produces
//!       exactly one contiguous window with monotonic padded bounds
//! — which is what the transcription manager's fall-back path
//! depends on to decide "use the whole buffer" vs. "use windows".
//!
//! This test does NOT require a Silero ONNX on disk (no network, no
//! model) — it's the graceful-absence contract the feature ships with
//! Phase 2.

use anyhow::Result;
use std::path::Path;

use toaster_app_lib::audio_toolkit::vad::prefilter::{
    prefilter_speech_windows, try_open_silero, SpeechWindow, VAD_SAMPLE_RATE_HZ,
};
use toaster_app_lib::audio_toolkit::vad::{
    VadFrame, VoiceActivityDetector, SILERO_FRAME_SAMPLES_16K,
};

struct AlwaysVoice;
impl VoiceActivityDetector for AlwaysVoice {
    fn push_frame<'a>(&'a mut self, frame: &'a [f32]) -> Result<VadFrame<'a>> {
        Ok(VadFrame::Speech(frame))
    }
}

struct AlwaysSilence;
impl VoiceActivityDetector for AlwaysSilence {
    fn push_frame<'a>(&'a mut self, _frame: &'a [f32]) -> Result<VadFrame<'a>> {
        Ok(VadFrame::Noise)
    }
}

#[test]
fn missing_silero_model_yields_ok_none() {
    let result = try_open_silero(Path::new("tests/__does_not_exist__.onnx"));
    match result {
        Ok(None) => {}
        Ok(Some(_)) => panic!("non-existent path must yield Ok(None)"),
        Err(e) => panic!("non-existent path must yield Ok(None), got Err({e})"),
    }
}

#[test]
fn sustained_voice_produces_single_monotonic_window() {
    let n_frames = 40;
    let samples = vec![0.0f32; n_frames * SILERO_FRAME_SAMPLES_16K];
    let mut vad = AlwaysVoice;
    let windows: Vec<SpeechWindow> = prefilter_speech_windows(&samples, &mut vad);
    assert_eq!(windows.len(), 1, "expected 1 window, got {windows:?}");
    let w = windows[0];
    assert!(w.start_us >= 0);
    assert!(w.end_us > w.start_us, "window must be monotonic");

    // Buffer is 40 frames × 30 ms = 1_200_000 µs. With padding the
    // window is clamped inside the buffer.
    let buffer_us: i64 =
        1_000_000 * SILERO_FRAME_SAMPLES_16K as i64 * n_frames as i64
            / VAD_SAMPLE_RATE_HZ as i64;
    assert!(w.end_us <= buffer_us, "padded end {} > buffer_us {}", w.end_us, buffer_us);
}

#[test]
fn all_silence_produces_no_windows() {
    let samples = vec![0.0f32; 20 * SILERO_FRAME_SAMPLES_16K];
    let mut vad = AlwaysSilence;
    let windows = prefilter_speech_windows(&samples, &mut vad);
    assert!(windows.is_empty(), "expected no windows, got {windows:?}");
}
