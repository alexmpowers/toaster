# Bundled caption fonts

These TTFs are the authoritative font files for both the preview
(`CaptionOverlay`) and the exported caption render (libass via the
FFmpeg `subtitles=` filter with `fontsdir=`). Keep the filenames
stable — `managers/captions/fonts.rs` looks them up by name.

| File         | Family name (CSS / ASS) | License | Source |
|--------------|-------------------------|---------|--------|
| `Inter.ttf`  | Inter                   | OFL-1.1 | https://github.com/rsms/inter |
| `Roboto.ttf` | Roboto                  | Apache-2.0 | https://github.com/openmaptiles/fonts |

`SystemUi` is not a bundled file — the preview falls back to the OS
`system-ui` CSS stack and the export omits `Fontname=` so libass picks
the platform default via fontconfig.

If you add a new face here, also:

1. Add the variant to `CaptionFontFamily` in `settings/types.rs`.
2. Update `managers/captions/fonts.rs` to map the enum to a filename
   and a CSS family name.
3. Add a locale string for the new label in every
   `src/i18n/locales/*/translation.json`.
