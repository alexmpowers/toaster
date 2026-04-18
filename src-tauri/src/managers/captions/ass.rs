//! ASS (Advanced SubStation Alpha) file emitter.
//!
//! Takes the authoritative `CaptionBlock` stream from `layout` and
//! produces an ASS document that libass (via FFmpeg's `subtitles=` filter)
//! renders identically to the preview. Two events are emitted per block:
//!
//! 1. A background event on `Layer=0` containing a `\p1` vector shape —
//!    a rounded rectangle sized to wrap the text with `padding_x/y_px`
//!    gutters — filled with the block's background color + alpha.
//! 2. A text event on `Layer=1` positioned inside the same rectangle,
//!    with `\N` between visual lines.
//!
//! This replaces the old SRT + `force_style` flow whose hard-edged
//! `BorderStyle=3` rectangle had no way to carry the preview's rounded
//! corners, Inter font, pixel-width wrap, or proportional padding.

use super::layout::{CaptionBlock, Rgba};
use std::fmt::Write;

/// Serialize `CaptionBlock`s into a complete ASS document string.
pub fn blocks_to_ass(blocks: &[CaptionBlock]) -> String {
    let (play_w, play_h) = blocks
        .first()
        .map(|b| (b.frame_width, b.frame_height))
        .unwrap_or((1920, 1080));

    // The ASS font name for the *style* — every block in one run uses the
    // same family (the user's selected `caption_font_family`).
    let font_name = blocks
        .first()
        .map(|b| b.font_ass_name.as_str())
        .unwrap_or("Arial");
    let font_size = blocks.first().map(|b| b.font_size_px).unwrap_or(24);

    let mut out = String::new();
    writeln!(out, "[Script Info]").unwrap();
    writeln!(out, "ScriptType: v4.00+").unwrap();
    writeln!(out, "WrapStyle: 2").unwrap();
    writeln!(out, "ScaledBorderAndShadow: yes").unwrap();
    writeln!(out, "PlayResX: {play_w}").unwrap();
    writeln!(out, "PlayResY: {play_h}").unwrap();
    writeln!(out).unwrap();

    writeln!(out, "[V4+ Styles]").unwrap();
    writeln!(
        out,
        "Format: Name, Fontname, Fontsize, PrimaryColour, SecondaryColour, OutlineColour, BackColour, Bold, Italic, Underline, StrikeOut, ScaleX, ScaleY, Spacing, Angle, BorderStyle, Outline, Shadow, Alignment, MarginL, MarginR, MarginV, Encoding"
    )
    .unwrap();
    // BorderStyle=1 (outline + shadow, but Outline=0/Shadow=0 means no
    // outline or shadow). We do backgrounds via the drawing event, not
    // ASS's built-in box — so the style is otherwise neutral.
    writeln!(
        out,
        "Style: Default,{font_name},{font_size},&H00FFFFFF,&H000000FF,&H00000000,&H00000000,0,0,0,0,100,100,0,0,1,0,0,7,0,0,0,1"
    )
    .unwrap();
    writeln!(out).unwrap();

    writeln!(out, "[Events]").unwrap();
    writeln!(
        out,
        "Format: Layer, Start, End, Style, Name, MarginL, MarginR, MarginV, Effect, Text"
    )
    .unwrap();

    for block in blocks {
        let num_lines = block.lines.len().max(1) as u32;
        let box_w = block.text_width_px + 2 * block.padding_x_px;
        let box_h = num_lines * block.line_height_px + 2 * block.padding_y_px;
        // Top-left of the rect (frame-pixel coords).
        let tx = block.frame_width.saturating_sub(box_w) / 2;
        let ty = block
            .frame_height
            .saturating_sub(block.margin_v_px)
            .saturating_sub(box_h);

        let start = format_ass_time(block.start_us);
        let end = format_ass_time(block.end_us);

        // ── Layer 0: rounded-rect background ───────────────────────
        let bg_color = ass_color_bgr(block.background);
        let bg_alpha = ass_alpha(block.background.a);
        let path = rounded_rect_path(box_w, box_h, block.radius_px.min(box_w / 2).min(box_h / 2));
        // `\an7` = top-left anchor so `\pos` refers to the rect's top-left.
        // `\bord0\shad0` kills any residual outline/shadow.
        // `\1c` + `\1a` paint the drawing fill.
        writeln!(
            out,
            "Dialogue: 0,{start},{end},Default,,0,0,0,,{{\\an7\\pos({tx},{ty})\\bord0\\shad0\\1c{bg_color}\\1a{bg_alpha}\\p1}}{path}{{\\p0}}"
        )
        .unwrap();

        // ── Layer 1: text ──────────────────────────────────────────
        let text_color = ass_color_bgr(block.text_color);
        let text_alpha = ass_alpha(block.text_color.a);
        let text_x = tx + block.padding_x_px;
        let text_y = ty + block.padding_y_px;
        let joined = block
            .lines
            .iter()
            .map(|l| escape_ass_text(l))
            .collect::<Vec<_>>()
            .join("\\N");
        writeln!(
            out,
            "Dialogue: 1,{start},{end},Default,,0,0,0,,{{\\an7\\pos({text_x},{text_y})\\bord0\\shad0\\1c{text_color}\\1a{text_alpha}}}{joined}"
        )
        .unwrap();
    }

    out
}

/// Draw a rounded rectangle using ASS vector drawing commands.
/// Origin is `(0, 0)`, size is `w × h`, corner radius `r`.
fn rounded_rect_path(w: u32, h: u32, r: u32) -> String {
    let r = r.min(w / 2).min(h / 2);
    if r == 0 {
        // Square rect — skip the bezier arcs.
        return format!("m 0 0 l {w} 0 l {w} {h} l 0 {h} l 0 0");
    }
    // `b x1 y1 x2 y2 x3 y3` draws a cubic bezier; repeating the end
    // points flattens the curve for a smooth quarter-arc approximation.
    let mut s = String::new();
    write!(s, "m {r} 0 ").unwrap();
    write!(s, "l {} 0 ", w - r).unwrap();
    write!(s, "b {w} 0 {w} 0 {w} {r} ").unwrap();
    write!(s, "l {w} {} ", h - r).unwrap();
    write!(s, "b {w} {h} {w} {h} {} {h} ", w - r).unwrap();
    write!(s, "l {r} {h} ").unwrap();
    write!(s, "b 0 {h} 0 {h} 0 {} ", h - r).unwrap();
    write!(s, "l 0 {r} ").unwrap();
    write!(s, "b 0 0 0 0 {r} 0").unwrap();
    s
}

/// Format `Rgba` as `&HBBGGRR&` (ASS color format — RGB channels only).
fn ass_color_bgr(c: Rgba) -> String {
    format!("&H{:02X}{:02X}{:02X}&", c.b, c.g, c.r)
}

/// Format an 8-bit CSS alpha as an ASS `\1a` override value.
/// ASS alpha is inverted relative to CSS: `00` = fully opaque,
/// `FF` = fully transparent.
fn ass_alpha(css_alpha: u8) -> String {
    format!("&H{:02X}&", 255 - css_alpha)
}

/// Escape an ASS text literal. ASS uses `\` for override tags and
/// `{` / `}` for tag groups; newlines must be expressed as `\N`.
fn escape_ass_text(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for ch in s.chars() {
        match ch {
            '\\' => out.push_str("\\\\"),
            '{' => out.push_str("\\{"),
            '}' => out.push_str("\\}"),
            '\n' => out.push_str("\\N"),
            c => out.push(c),
        }
    }
    out
}

/// ASS time format: `H:MM:SS.cc` (centiseconds).
fn format_ass_time(us: i64) -> String {
    let total_cs = us.max(0) / 10_000;
    let cs = total_cs % 100;
    let total_s = total_cs / 100;
    let s = total_s % 60;
    let total_m = total_s / 60;
    let m = total_m % 60;
    let h = total_m / 60;
    format!("{h}:{m:02}:{s:02}.{cs:02}")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::managers::captions::{CaptionBlock, Rgba};

    fn mk_block(idx: usize, start_us: i64, end_us: i64) -> CaptionBlock {
        CaptionBlock {
            index: idx,
            start_us,
            end_us,
            lines: vec!["Hello world".to_string()],
            font_css: "Inter, sans-serif".into(),
            font_ass_name: "Inter".into(),
            font_size_px: 32,
            text_color: Rgba { r: 255, g: 255, b: 255, a: 255 },
            background: Rgba { r: 0, g: 0, b: 0, a: 0xB3 },
            padding_x_px: 12,
            padding_y_px: 4,
            radius_px: 4,
            margin_v_px: 108,
            text_width_px: 200,
            line_height_px: 40,
            frame_width: 1280,
            frame_height: 720,
        }
    }

    #[test]
    fn ass_time_formats_like_libass_expects() {
        assert_eq!(format_ass_time(0), "0:00:00.00");
        assert_eq!(format_ass_time(1_500_000), "0:00:01.50");
        assert_eq!(format_ass_time(3_661_234_000), "1:01:01.23");
    }

    #[test]
    fn rgba_to_ass_color_swaps_to_bgr() {
        let c = Rgba { r: 0xAA, g: 0xBB, b: 0xCC, a: 0xFF };
        assert_eq!(ass_color_bgr(c), "&HCCBBAA&");
    }

    #[test]
    fn alpha_inverts_from_css_convention() {
        assert_eq!(ass_alpha(0xFF), "&H00&"); // fully opaque CSS → 00 ASS
        assert_eq!(ass_alpha(0x00), "&HFF&"); // fully transparent CSS → FF
        assert_eq!(ass_alpha(0xB3), "&H4C&");
    }

    #[test]
    fn text_escapes_braces_and_backslashes() {
        assert_eq!(escape_ass_text(r"a{b}c"), r"a\{b\}c");
        assert_eq!(escape_ass_text(r"\N"), r"\\N");
    }

    #[test]
    fn rounded_rect_path_has_four_arcs() {
        let p = rounded_rect_path(200, 80, 4);
        // One `m`, four `l`, four `b` commands.
        assert_eq!(p.matches(" b ").count() + p.starts_with("b ") as usize, 4);
        assert_eq!(p.matches(" l ").count(), 4);
    }

    #[test]
    fn square_rect_when_radius_is_zero() {
        let p = rounded_rect_path(200, 80, 0);
        assert!(!p.contains("b "));
        assert_eq!(p.matches(" l ").count(), 4); // m 0 0 l w 0 l w h l 0 h l 0 0
    }

    #[test]
    fn document_contains_script_info_and_events() {
        let blocks = vec![mk_block(1, 0, 2_000_000), mk_block(2, 2_000_000, 4_000_000)];
        let doc = blocks_to_ass(&blocks);
        assert!(doc.contains("[Script Info]"));
        assert!(doc.contains("PlayResX: 1280"));
        assert!(doc.contains("PlayResY: 720"));
        assert!(doc.contains("[V4+ Styles]"));
        assert!(doc.contains("Style: Default,Inter,32,"));
        assert!(doc.contains("[Events]"));
        // Two blocks × two events (bg + text) = four dialogue lines.
        assert_eq!(doc.matches("Dialogue: ").count(), 4);
        assert!(doc.contains("\\p1"));
        assert!(doc.contains("\\p0"));
        assert!(doc.contains("Hello world"));
    }

    #[test]
    fn empty_blocks_still_produce_valid_header() {
        let doc = blocks_to_ass(&[]);
        assert!(doc.contains("[Script Info]"));
        assert!(doc.contains("[V4+ Styles]"));
        assert!(doc.contains("[Events]"));
        assert_eq!(doc.matches("Dialogue: ").count(), 0);
    }
}
