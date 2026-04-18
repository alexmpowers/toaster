//! Font bundle registry.
//!
//! Maps `CaptionFontFamily` → bundled TTF bytes, a CSS `font-family` stack
//! the preview applies verbatim, and an ASS `Fontname` the export passes
//! to libass. The files live in `src-tauri/assets/fonts/` and are embedded
//! at compile time so layout calculations are deterministic and work in
//! unit tests without the Tauri resource path.

use crate::settings::CaptionFontFamily;
use fontdue::Font;
use std::sync::Arc;

const INTER_TTF: &[u8] = include_bytes!("../../../assets/fonts/Inter.ttf");
const ROBOTO_TTF: &[u8] = include_bytes!("../../../assets/fonts/Roboto.ttf");

/// A parsed font ready for metrics-based line wrapping.
pub struct FontMetricsHandle {
    pub font: Arc<Font>,
    /// Human-readable family name matching both preview CSS and ASS `Fontname=`.
    pub ass_name: &'static str,
    /// CSS `font-family` value (including fallback stack).
    pub css_stack: &'static str,
    /// Filename (relative to the fonts dir) shipped in the bundle. `None`
    /// for `SystemUi` which has no bundled file.
    pub file_name: Option<&'static str>,
}

/// Resolve all caption fonts once and share them across calls.
pub struct FontRegistry {
    pub inter: FontMetricsHandle,
    pub roboto: FontMetricsHandle,
    /// System UI has no bundled file; we reuse Inter's metrics for wrap
    /// calculations (visually close stack; any minor drift is absorbed by
    /// the live font at render time).
    pub system_ui: FontMetricsHandle,
}

impl FontRegistry {
    pub fn new() -> Result<Self, String> {
        let inter = Arc::new(
            Font::from_bytes(INTER_TTF, fontdue::FontSettings::default())
                .map_err(|e| format!("Inter.ttf parse failed: {e}"))?,
        );
        let roboto = Arc::new(
            Font::from_bytes(ROBOTO_TTF, fontdue::FontSettings::default())
                .map_err(|e| format!("Roboto.ttf parse failed: {e}"))?,
        );

        Ok(Self {
            inter: FontMetricsHandle {
                font: inter.clone(),
                ass_name: "Inter",
                css_stack: "Inter, system-ui, sans-serif",
                file_name: Some("Inter.ttf"),
            },
            roboto: FontMetricsHandle {
                font: roboto,
                ass_name: "Roboto",
                css_stack: "Roboto, system-ui, sans-serif",
                file_name: Some("Roboto.ttf"),
            },
            system_ui: FontMetricsHandle {
                font: inter,
                ass_name: "Arial",
                css_stack: "system-ui, -apple-system, Segoe UI, Roboto, sans-serif",
                file_name: None,
            },
        })
    }

    pub fn resolve(&self, family: CaptionFontFamily) -> &FontMetricsHandle {
        match family {
            CaptionFontFamily::Inter => &self.inter,
            CaptionFontFamily::Roboto => &self.roboto,
            CaptionFontFamily::SystemUi => &self.system_ui,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn registry_loads_bundled_fonts() {
        let reg = FontRegistry::new().expect("fonts parse");
        assert_eq!(reg.inter.ass_name, "Inter");
        assert_eq!(reg.roboto.ass_name, "Roboto");
        let (metrics, _) = reg.inter.font.rasterize('A', 48.0);
        assert!(metrics.width > 0);
    }

    #[test]
    fn resolve_picks_correct_handle() {
        let reg = FontRegistry::new().unwrap();
        assert_eq!(reg.resolve(CaptionFontFamily::Inter).ass_name, "Inter");
        assert_eq!(reg.resolve(CaptionFontFamily::Roboto).ass_name, "Roboto");
        assert!(reg.resolve(CaptionFontFamily::SystemUi).file_name.is_none());
    }
}
