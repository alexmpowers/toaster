# Deterministic 48 kHz mono PCM16 tone generator for overlap_01 fixture.
#
# Encodes the regression case that slipped past the pre-shipped eval gates:
# forced-alignment returns word_01.end_us > word_02.start_us for repeated
# identical adjacent tokens ("the the"). After word_02 is deleted, the keep
# segment that ends at word_01 must be clamped to word_02.start_us, not
# word_01.end_us. The leaky variant simulates the pre-fix bug (un-clamped end)
# so audio-boundary-eval can prove it would have caught the regression.
#
# Run from repo root:
#     python src-tauri/tests/fixtures/boundary/_generate_overlap_01.py
#
# Outputs (all under src-tauri/tests/fixtures/boundary/):
#   overlap_01.wav                      2.0 s source with overlapping tones
#   overlap_01_edited_clean.wav         clamped edit (seg1 ends at 680 ms)
#   overlap_01_edited_leaky.wav         pre-fix edit (seg1 ends at 700 ms)
#   overlap_01_preview.wav              identical copy of the clean edit
#   overlap_01_stems/word_00.wav..word_03.wav per-word stems

import math
import struct
import wave
import pathlib

SR = 48000
AMP = 0.3
FADE_MS = 5.0
DIR = pathlib.Path(__file__).resolve().parent
STEMS_DIR = DIR / "overlap_01_stems"

# idx, label, freq_hz, start_us, end_us
WORDS = [
    (0, "word_00", 440.0,       0,  400_000),
    (1, "word_01", 660.0, 380_000,  700_000),  # overlap tail -> word_02.start_us
    (2, "word_02", 660.0, 680_000, 1_000_000),  # deleted in test
    (3, "word_03", 880.0, 1_000_000, 1_600_000),
]

TOTAL_US = 2_000_000
TOTAL_SAMPLES = SR * TOTAL_US // 1_000_000  # 96_000

def us_to_samples(us: int) -> int:
    return SR * us // 1_000_000

def sine(n_samples: int, freq_hz: float, phase0: float = 0.0):
    # Zero-phase-at-t0 sine. phase0 lets callers keep continuity across slices.
    out = [0.0] * n_samples
    w = 2.0 * math.pi * freq_hz / SR
    for i in range(n_samples):
        out[i] = math.sin(phase0 + w * i)
    return out

def apply_fade(buf, fade_samples: int):
    n = len(buf)
    fs = min(fade_samples, n // 2)
    for i in range(fs):
        # raised-cosine (Hann half-window): 0.5 * (1 - cos(pi * i / fs))
        g = 0.5 * (1.0 - math.cos(math.pi * i / fs))
        buf[i] *= g
        buf[n - 1 - i] *= g
    return buf

def write_wav(path: pathlib.Path, samples):
    path.parent.mkdir(parents=True, exist_ok=True)
    with wave.open(str(path), "wb") as w:
        w.setnchannels(1)
        w.setsampwidth(2)
        w.setframerate(SR)
        data = bytearray()
        for s in samples:
            v = max(-1.0, min(1.0, s))
            i = int(round(v * 32767.0))
            if i > 32767: i = 32767
            if i < -32768: i = -32768
            data += struct.pack("<h", i)
        w.writeframes(bytes(data))

def generate_stem(freq_hz: float, start_us: int, end_us: int):
    # Stem is synthesized with phase=0 at its own t=0. Fades prevent intra-stem
    # clicks; the seam-click gate cares about the concatenation seam, not this.
    n = us_to_samples(end_us) - us_to_samples(start_us)
    buf = [AMP * s for s in sine(n, freq_hz, 0.0)]
    apply_fade(buf, int(SR * FADE_MS / 1000.0))
    return buf

def generate_source():
    # Sum overlapping tones into one 2.0 s buffer. Each tone keeps a continuous
    # global phase so slicing the buffer produces the same samples as slicing
    # an idealized continuous signal.
    buf = [0.0] * TOTAL_SAMPLES
    for _, _, freq, s_us, e_us in WORDS:
        s0 = us_to_samples(s_us)
        s1 = us_to_samples(e_us)
        w = 2.0 * math.pi * freq / SR
        for i in range(s0, s1):
            buf[i] += AMP * math.sin(w * i)
    return buf

def slice_source(source, start_us: int, end_us: int):
    s0 = us_to_samples(start_us)
    s1 = us_to_samples(end_us)
    return list(source[s0:s1])

def apply_seam_fade(buf, fade_samples: int):
    # Equal-power micro-fade on seg1 end and seg2 start — mirrors the
    # sample-boundary smoothing the real export pipeline applies at splice
    # points. The seam-click gate (E2) asserts this is in place on the clean
    # edit; it is not the leak gate (E1), which is what this fixture primarily
    # guards.
    n = len(buf)
    fs = min(fade_samples, n // 2)
    for i in range(fs):
        g = 0.5 * (1.0 - math.cos(math.pi * i / fs))
        buf[n - 1 - i] *= g  # fade out at end
    return buf

def apply_seam_fade_in(buf, fade_samples: int):
    n = len(buf)
    fs = min(fade_samples, n // 2)
    for i in range(fs):
        g = 0.5 * (1.0 - math.cos(math.pi * i / fs))
        buf[i] *= g  # fade in at start
    return buf

def main():
    STEMS_DIR.mkdir(parents=True, exist_ok=True)

    # Per-word stems (reproducibility + E1 deleted_word_stem = word_02).
    for _, label, freq, s_us, e_us in WORDS:
        stem = generate_stem(freq, s_us, e_us)
        write_wav(STEMS_DIR / f"{label}.wav", stem)

    # Source: 2.0 s overlapping-tone timeline.
    source = generate_source()
    write_wav(DIR / "overlap_01.wav", source)

    # Clean edit: delete word_02 with clamped seg1 end -> min(word_01.end_us,
    # word_02.start_us) = 680_000 us. Keep segments [(0, 680_000),
    # (1_000_000, 1_600_000)]. Raw concat; no crossfade.
    seg1_clean = slice_source(source, 0, 680_000)
    seg2       = slice_source(source, 1_000_000, 1_600_000)
    seam_fade = int(SR * 2.5 / 1000.0)  # 2.5 ms
    apply_seam_fade(seg1_clean, seam_fade)
    apply_seam_fade_in(seg2, seam_fade)
    clean_edit = seg1_clean + seg2
    assert len(clean_edit) == 32_640 + 28_800 == 61_440
    write_wav(DIR / "overlap_01_edited_clean.wav", clean_edit)

    # Preview: identical to clean edit (E3 parity).
    write_wav(DIR / "overlap_01_preview.wav", clean_edit)

    # Leaky edit: pre-fix behavior — seg1 extends to word_01.end_us = 700_000,
    # leaking the onset of word_02 (same 660 Hz tone) into the edit. Detectable
    # by E1 cross-correlation against word_02 stem.
    seg1_leaky = slice_source(source, 0, 700_000)
    apply_seam_fade(seg1_leaky, seam_fade)
    leaky_edit = seg1_leaky + seg2
    assert len(leaky_edit) == 33_600 + 28_800 == 62_400
    write_wav(DIR / "overlap_01_edited_leaky.wav", leaky_edit)

    print("overlap_01 fixture generated:")
    print(f"  source:      overlap_01.wav ({len(source)} samples)")
    print(f"  clean_edit:  overlap_01_edited_clean.wav ({len(clean_edit)} samples)")
    print(f"  leaky_edit:  overlap_01_edited_leaky.wav ({len(leaky_edit)} samples)")
    print(f"  preview:     overlap_01_preview.wav ({len(clean_edit)} samples)")
    print(f"  stems:       overlap_01_stems/word_00..word_03.wav")

if __name__ == "__main__":
    main()
