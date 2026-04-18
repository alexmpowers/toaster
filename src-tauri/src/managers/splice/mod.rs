//! Splice-quality primitives for edit-time and render-time use.
//!
//! This module is the single home for audio-aware splice logic that is
//! shared between preview and export. Today it hosts:
//!
//! * [`boundaries`] — zero-crossing snap for kept-segment endpoints.
//! * [`loudness`]  — deterministic EBU R128 / LUFS measurement + gain.
//! * [`clarity`]   — spectral clarity features for survivor scoring.
//!
//! None of these pieces wire themselves into the live pipeline; callers
//! opt in. This keeps the AGENTS.md "single source of truth for
//! dual-path logic" invariant — preview and export consume the same
//! helpers or neither does.

pub mod boundaries;
pub mod clarity;
pub mod loudness;
