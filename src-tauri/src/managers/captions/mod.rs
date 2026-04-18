//! Caption layout + font resolution.
//!
//! One authoritative layout engine that both the preview and the export
//! render from. See AGENTS.md "Single source of truth for dual-path logic."

pub mod ass;
pub mod fonts;
pub mod layout;

pub use ass::blocks_to_ass;
pub use fonts::{FontMetricsHandle, FontRegistry};
pub use layout::{
    build_blocks, CaptionBlock, CaptionLayoutConfig, Rgba, TimelineDomain,
};
