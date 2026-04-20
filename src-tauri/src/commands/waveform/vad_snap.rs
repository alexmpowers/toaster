//! R-003 splice-boundary refinement live wiring.
//!
//! Single source of truth for preview / export / loudness-preflight
//! boundary snap. Extracted from `commands/waveform/mod.rs` to keep
//! that file under the 800-line cap. Callers go through
//! [`snap_segments_against_media`]; nothing else in the waveform
//! module reaches into the VAD layer directly.
//!
//! Behavior is gated on `settings.vad_refine_boundaries` and
//! degrades silently when the Silero ONNX is missing, ORT init
//! fails, or per-frame compute errors out (BLUEPRINT.md §AD-8).

use std::path::Path;
use std::sync::Arc;

use log::{info, warn};
use tauri::{AppHandle, Manager};

use crate::audio_toolkit::vad::prefilter::try_open_silero;
use crate::audio_toolkit::vad::{VadFrame, VoiceActivityDetector, SILERO_FRAME_SAMPLES_16K};
use crate::managers::model::ModelManager;
use crate::managers::splice::boundaries::{
    snap_segments_vad_biased, DEFAULT_ENERGY_RADIUS_US, DEFAULT_SNAP_RADIUS_US,
};

/// Snap every `(start_us, end_us)` pair to the nearest **energy valley**
/// (plus zero-crossing) in the decoded source audio.
///
/// Zero-crossing snap alone eliminates the *click* at a seam but still lands
/// the boundary at whichever ZC is arithmetically closest — which, right at
/// the trailing edge of a deleted phoneme, is often a few ms *inside* that
/// phoneme. The result is faint bleed-through of the deleted sound ("uh"
/// after "And uh" → "And").
///
/// This energy-biased variant widens the search to ±`DEFAULT_ENERGY_RADIUS_US`
/// (20 ms), picks the quietest short frame, then snaps that to the nearest
/// zero-crossing within ±`DEFAULT_SNAP_RADIUS_US`. In voiced-only audio with
/// no energy gradient the behaviour degenerates back to plain ZC snap.
///
/// Decodes the media exactly once (via `ffmpeg -f f32le`), so preview and
/// export pay the same decode cost they already pay during the current
/// render. Returns the input segments unchanged if decode fails — **never**
/// regresses the current behavior.
///
/// When `settings.vad_refine_boundaries` is true and the Silero ONNX is
/// downloaded, computes a P(speech) curve over the decoded buffer and
/// hands it to [`snap_segments_vad_biased`]. With an empty curve the
/// vad-biased path is byte-identical to the energy path
/// (AC-003-d guard, exercised by tests::vad_biased_snap_disabled_matches_baseline).
/// Per BLUEPRINT.md §AD-8 every failure mode in the VAD path —
/// missing model, ORT init, per-frame compute error — degrades silently
/// to the energy-only behavior. R-003 single-source-of-truth lives here:
/// preview, export, and loudness preflight all consume this function.
pub(super) fn snap_segments_against_media(
    app: &AppHandle,
    segments: &[(i64, i64)],
    media_path: &Path,
) -> Vec<(i64, i64)> {
    if segments.len() < 2 {
        return segments.to_vec();
    }
    let samples = match crate::commands::disfluency::decode_media_audio(media_path) {
        Ok(samples) => samples,
        Err(e) => {
            warn!(
                "Zero-crossing snap skipped for {}: decode failed ({}). Falling back to original segments.",
                media_path.display(),
                e
            );
            return segments.to_vec();
        }
    };

    let vad_refine = crate::settings::get_settings(app).vad_refine_boundaries;
    let vad_curve = if vad_refine {
        compute_vad_curve_for_app(app, &samples)
    } else {
        Vec::new()
    };

    let snapped = snap_segments_vad_biased(
        segments,
        &samples,
        16_000,
        &vad_curve,
        DEFAULT_ENERGY_RADIUS_US,
        DEFAULT_SNAP_RADIUS_US,
    );
    if snapped.is_empty() {
        segments.to_vec()
    } else {
        snapped
    }
}

/// Build a per-30 ms-frame Silero P(speech) curve over `samples_16k` for
/// boundary refinement. Returns an empty vector on any failure (model
/// not downloaded, ORT init failure, model-manager state missing) — the
/// vad-biased snap then behaves identically to the energy-only path.
fn compute_vad_curve_for_app(app: &AppHandle, samples_16k: &[f32]) -> Vec<f32> {
    let model_manager = match app.try_state::<Arc<ModelManager>>() {
        Some(state) => state,
        None => {
            warn!("VAD boundary refinement: ModelManager state unavailable; falling back to energy-only snap.");
            return Vec::new();
        }
    };
    let model_path = match model_manager.get_model_path("silero-vad") {
        Ok(path) => path,
        Err(_) => return Vec::new(),
    };
    let mut vad = match try_open_silero(&model_path) {
        Ok(Some(vad)) => vad,
        Ok(None) => return Vec::new(),
        Err(e) => {
            warn!(
                "VAD boundary refinement: Silero open failed ({e}); falling back to energy-only snap."
            );
            return Vec::new();
        }
    };
    let total_frames = samples_16k.len() / SILERO_FRAME_SAMPLES_16K;
    let mut curve = Vec::with_capacity(total_frames);
    for fi in 0..total_frames {
        let lo = fi * SILERO_FRAME_SAMPLES_16K;
        let hi = lo + SILERO_FRAME_SAMPLES_16K;
        // Per-frame failure -> assume voice (1.0) so the boundary stays
        // where the energy-only path would have put it.
        let prob = match vad.push_frame(&samples_16k[lo..hi]) {
            Ok(VadFrame::Speech(_)) => 1.0,
            Ok(VadFrame::Noise) => 0.0,
            Err(_) => 1.0,
        };
        curve.push(prob);
    }
    info!(
        "VAD boundary refinement: computed {} frame probabilities ({} samples).",
        curve.len(),
        samples_16k.len()
    );
    curve
}
