//! Caption parity dump — Rust-side entry for `eval-caption-parity.ps1`.
//!
//! The harness drives this test binary once per fixture:
//!
//! ```powershell
//! $env:CAPTION_PARITY_FIXTURE = "<path>/input.json"
//! $env:CAPTION_PARITY_OUTPUT  = "<path>/_actual.json"
//! cargo test --test caption_parity_dump -- --nocapture
//! ```
//!
//! When both env vars are set the test reads the fixture, runs the
//! authoritative `build_blocks` + `compute_caption_layout` from
//! `managers::captions::layout`, serialises `blocks_to_ass`, writes the
//! combined dump to `CAPTION_PARITY_OUTPUT`, and passes.
//!
//! When either env var is unset the test is a no-op pass so `cargo test`
//! stays green in normal developer workflows. This mirrors
//! `precision_eval_multi_backend.rs`'s skip-soft posture.
//!
//! The harness compares this output against `expected.json`; the feature
//! bundle's BLUEPRINT documents the tolerance policy (1 px geometry,
//! 1 sample @ 48 kHz timing).

use std::fs;
use std::path::PathBuf;

use serde::{Deserialize, Serialize};
use serde_json::Value;

use toaster_app_lib::managers::captions::{
    blocks_to_ass, build_blocks, compute_caption_layout, CaptionBlock, CaptionLayout,
    CaptionLayoutConfig, FontRegistry, TimelineDomain,
};
use toaster_app_lib::managers::editor::Word;
use toaster_app_lib::settings::{CaptionProfile, VideoDims};

/// Declarative fixture input consumed by the dump binary and the
/// `eval-caption-parity.ps1` harness. Flat by design so the harness can
/// patch a single field for `-ForceDrift` without needing a second parser.
#[derive(Debug, Deserialize)]
struct FixtureInput {
    /// Human-readable label echoed back into the dump for diagnostics.
    id: String,
    /// Edited-or-source timeline the caller wants the block timestamps on.
    /// Serialised as `"Source"` or `"Edited"`.
    timeline_domain: TimelineDomain,
    /// The target video dimensions the layout is computed against.
    video_dims: VideoDims,
    /// The authoritative `CaptionProfile` as stored in settings.
    caption_profile: CaptionProfile,
    /// Post-edit word list. Silenced/deleted flags mean the same thing as
    /// in the editor.
    words: Vec<Word>,
    /// Keep-segments on the source timeline as `[start_us, end_us]` pairs.
    /// Only consulted when `timeline_domain == Edited`.
    #[serde(default)]
    keep_segments: Vec<[i64; 2]>,
    /// Linear scale factor from `frame_height` to the rendered `<video>`
    /// element height used by the preview. Captured here so the harness
    /// can reason about pixel tolerances in preview space when needed.
    /// Defaults to 1.0 (authoritative pixel space).
    #[serde(default = "default_preview_scale")]
    preview_scale_factor: f64,
    /// Optional overrides applied over the profile-derived config. Used
    /// for tests that need to pin `max_segment_duration_us` or
    /// `include_silenced` independent of the profile schema.
    #[serde(default)]
    config_overrides: ConfigOverrides,
}

fn default_preview_scale() -> f64 {
    1.0
}

#[derive(Debug, Default, Deserialize)]
struct ConfigOverrides {
    #[serde(default)]
    max_segment_duration_us: Option<i64>,
    #[serde(default)]
    include_silenced: Option<bool>,
}

/// Shape serialised to `CAPTION_PARITY_OUTPUT`. Matches the shape the
/// harness diffs against `expected.json`.
#[derive(Debug, Serialize)]
struct DumpOutput {
    id: String,
    preview_scale_factor: f64,
    layout: CaptionLayout,
    blocks: Vec<CaptionBlock>,
    /// Raw ASS document `blocks_to_ass` emits. The harness parses the
    /// `Dialogue:` + `Style:` lines for export-side geometry comparison.
    ass: String,
}

#[test]
fn caption_parity_dump() {
    let fixture_path = match std::env::var("CAPTION_PARITY_FIXTURE") {
        Ok(v) if !v.trim().is_empty() => PathBuf::from(v),
        _ => {
            eprintln!(
                "caption_parity_dump: CAPTION_PARITY_FIXTURE not set — skipping (this is normal under plain `cargo test`)"
            );
            return;
        }
    };
    let output_path = match std::env::var("CAPTION_PARITY_OUTPUT") {
        Ok(v) if !v.trim().is_empty() => PathBuf::from(v),
        _ => panic!(
            "caption_parity_dump: CAPTION_PARITY_FIXTURE was set but CAPTION_PARITY_OUTPUT was not"
        ),
    };

    let fixture_text = fs::read_to_string(&fixture_path)
        .unwrap_or_else(|e| panic!("read fixture {:?}: {}", fixture_path, e));
    // Parse as raw Value first for a clearer error if the schema drifts.
    let _sanity: Value = serde_json::from_str(&fixture_text)
        .unwrap_or_else(|e| panic!("parse fixture {:?} as JSON: {}", fixture_path, e));
    let fixture: FixtureInput = serde_json::from_str(&fixture_text)
        .unwrap_or_else(|e| panic!("decode fixture {:?}: {}", fixture_path, e));

    // Build config the same way the production path does
    // (`CaptionLayoutConfig::from_profile`), then fold in any overrides.
    let layout = compute_caption_layout(&fixture.caption_profile, fixture.video_dims);
    let mut config =
        CaptionLayoutConfig::from_profile(&fixture.caption_profile, fixture.video_dims);
    if let Some(v) = fixture.config_overrides.max_segment_duration_us {
        config.max_segment_duration_us = v;
    }
    if let Some(v) = fixture.config_overrides.include_silenced {
        config.include_silenced = v;
    }

    let fonts = FontRegistry::new().expect("FontRegistry::new");

    let keep: Vec<(i64, i64)> = fixture
        .keep_segments
        .iter()
        .map(|pair| (pair[0], pair[1]))
        .collect();

    let blocks = build_blocks(
        &fixture.words,
        &keep,
        &config,
        &fonts,
        fixture.timeline_domain,
    );
    let ass = blocks_to_ass(&blocks);

    let out = DumpOutput {
        id: fixture.id,
        preview_scale_factor: fixture.preview_scale_factor,
        layout,
        blocks,
        ass,
    };

    if let Some(parent) = output_path.parent() {
        fs::create_dir_all(parent)
            .unwrap_or_else(|e| panic!("create parent dir for {:?}: {}", output_path, e));
    }
    let serialised = serde_json::to_string_pretty(&out).expect("serialise dump");
    fs::write(&output_path, serialised)
        .unwrap_or_else(|e| panic!("write dump {:?}: {}", output_path, e));
}
