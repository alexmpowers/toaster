//! R-003 / AC-003-d — VAD-biased boundary snap must be byte-identical
//! to the energy-only snap when the `vad_refine_boundaries` setting is
//! off (simulated by passing an empty `vad_curve`).
//!
//! Dual-path SSoT guard: preview and export both consume
//! `snap_segments_vad_biased`, so if this invariant ever breaks we'd
//! get subtle audio drift between the two surfaces. This lives as an
//! integration test so the contract is checked against the public
//! library surface.

use toaster_app_lib::managers::splice::boundaries::{
    snap_segments_energy_biased, snap_segments_vad_biased,
    DEFAULT_ENERGY_RADIUS_US, DEFAULT_SNAP_RADIUS_US,
};

const SR: u32 = 16_000;

fn sine(freq_hz: f32, sr: u32, samples: usize, amp: f32) -> Vec<f32> {
    use std::f32::consts::TAU;
    (0..samples)
        .map(|i| amp * (TAU * freq_hz * (i as f32) / sr as f32).sin())
        .collect()
}

#[test]
fn empty_vad_curve_matches_energy_path() {
    let buf = sine(120.0, SR, 16_000, 0.6);
    let segments = vec![
        (100_000, 400_000),
        (450_000, 700_000),
        (780_000, 950_000),
    ];
    let baseline = snap_segments_energy_biased(
        &segments,
        &buf,
        SR,
        DEFAULT_ENERGY_RADIUS_US,
        DEFAULT_SNAP_RADIUS_US,
    );
    let vad_off = snap_segments_vad_biased(
        &segments,
        &buf,
        SR,
        &[], // simulates vad_refine_boundaries = false
        DEFAULT_ENERGY_RADIUS_US,
        DEFAULT_SNAP_RADIUS_US,
    );
    assert_eq!(
        baseline, vad_off,
        "byte-identical invariant broken: energy={baseline:?} vad_off={vad_off:?}"
    );
}
