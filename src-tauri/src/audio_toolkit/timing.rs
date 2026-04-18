//! Time / sample / seconds rounding policy.
//!
//! Background (todo `p0-rounding-policy`): the transcription pipeline
//! converts back and forth between sample indices, microseconds, and
//! seconds many times per word. The previous implementation used naive
//! `as i64` / `as usize` casts on `f64`, which truncate toward zero. That
//! introduces a one-sided floor bias of up to ~1 sample (≈62.5 µs at
//! 16 kHz) at every conversion, and the bias accumulates over a
//! µs → sample → µs round-trip. On the cut path this drift makes
//! boundaries land at *earlier* samples than intended, which leaves extra
//! audio in kept segments (audible remnants on midstream deletions).
//!
//! Policy enforced by this module:
//!   * Every float→integer conversion uses **round-half-to-even**
//!     (banker's rounding, `f64::round_ties_even`) so error is symmetric
//!     and bounded by ±0.5 of the destination unit, with ties distributed
//!     evenly rather than systematically biased away from zero. Over many
//!     round-trips (e.g. 10 000 µs→sample→µs conversions) net drift stays
//!     within a handful of µs instead of accumulating ~2.5 ms of bias as
//!     `f64::round` (ties-away-from-zero) does on a positive-only domain.
//!   * Out-of-range and non-finite inputs **saturate** (`i64::MAX` /
//!     `i64::MIN` / `usize::MAX` / `0`) instead of producing UB via
//!     `as` casts on out-of-range floats.
//!   * Segment ranges are half-open `[start_us, end_us)`. Both ends are
//!     rounded with the **same** policy (nearest); never mix floor on one
//!     end with ceil on the other, which would systematically bias the
//!     duration `end - start`.
//!   * Clamping a sample index to a valid frame range
//!     (`min(total_samples - 1)`) is intentional truncation of a different
//!     kind — that lives at the call site, not in this module.

/// Largest `f64` that is `<= i64::MAX` and round-trips losslessly.
const I64_MAX_AS_F64: f64 = 9_223_372_036_854_774_784_f64; // i64::MAX rounded down to the nearest f64
/// Smallest `f64` that is `>= i64::MIN` and round-trips losslessly.
const I64_MIN_AS_F64: f64 = -9_223_372_036_854_775_808_f64; // i64::MIN is exactly representable

/// Round a finite `f64` to the nearest `i64`, saturating on overflow and
/// returning `0` for `NaN`. Ties round to even (banker's rounding) so net
/// bias over many conversions is zero.
#[inline]
pub fn round_f64_to_i64(x: f64) -> i64 {
    if x.is_nan() {
        return 0;
    }
    let r = x.round_ties_even();
    if r >= I64_MAX_AS_F64 {
        i64::MAX
    } else if r <= I64_MIN_AS_F64 {
        i64::MIN
    } else {
        r as i64
    }
}

/// Round a finite non-negative `f64` to the nearest `usize`, saturating on
/// overflow. Negative or non-finite values clamp to `0`.
#[inline]
pub fn round_f64_to_usize(x: f64) -> usize {
    if !x.is_finite() || x <= 0.0 {
        return 0;
    }
    let r = x.round_ties_even();
    if r >= usize::MAX as f64 {
        usize::MAX
    } else {
        r as usize
    }
}

/// Convert a sample index to microseconds with nearest-integer rounding.
#[inline]
pub fn sample_to_us(sample_idx: usize, sample_rate_hz: f64) -> i64 {
    debug_assert!(sample_rate_hz > 0.0, "sample_rate_hz must be positive");
    round_f64_to_i64(sample_idx as f64 / sample_rate_hz * 1_000_000.0)
}

/// Convert microseconds (or a duration in µs) to a sample count with
/// nearest-integer rounding. Negative timestamps clamp to `0`.
#[inline]
pub fn us_to_sample(timestamp_us: i64, sample_rate_hz: f64) -> usize {
    debug_assert!(sample_rate_hz > 0.0, "sample_rate_hz must be positive");
    round_f64_to_usize(timestamp_us.max(0) as f64 / 1_000_000.0 * sample_rate_hz)
}

/// Like [`us_to_sample`] but clamps to `total_samples - 1` so the result
/// is always a valid frame index. The clamp is *intentional* truncation
/// at the boundary of valid array indices and is separate from the
/// rounding policy above.
#[inline]
pub fn us_to_sample_clamped(timestamp_us: i64, sample_rate_hz: f64, total_samples: usize) -> usize {
    if total_samples == 0 {
        return 0;
    }
    us_to_sample(timestamp_us, sample_rate_hz).min(total_samples - 1)
}

/// Convert seconds (e.g. a segment timestamp from an ASR engine) to
/// microseconds with nearest-integer rounding. Both ends of a half-open
/// `[start, end)` range MUST be converted with this function so duration
/// is not biased.
#[inline]
pub fn seconds_to_us(seconds: f64) -> i64 {
    round_f64_to_i64(seconds * 1_000_000.0)
}

#[cfg(test)]
mod tests {
    use super::*;

    const SR: f64 = 16_000.0;

    /// (a) Round-trip `us → sample → us` is idempotent within ≤ 1 µs at 16 kHz
    /// for every µs that lands exactly on a sample boundary. Off-grid µs
    /// values may round to the nearest sample but must not exceed half a
    /// sample period (≈31 µs) of error.
    #[test]
    fn us_to_sample_to_us_roundtrip_at_sample_boundaries() {
        // Sample boundaries: us = sample * 1_000_000 / 16_000 = sample * 62.5
        // We pick samples whose µs is an integer (every 2 samples = 125 µs).
        for sample in [0_usize, 2, 4, 100, 800, 1_600, 16_000, 160_000] {
            let us = sample_to_us(sample, SR);
            let recovered_sample = us_to_sample(us, SR);
            let recovered_us = sample_to_us(recovered_sample, SR);
            assert_eq!(
                recovered_sample, sample,
                "sample {} did not round-trip ({} µs → {})",
                sample, us, recovered_sample
            );
            assert!(
                (recovered_us - us).abs() <= 1,
                "us {} did not round-trip within 1 µs (got {})",
                us,
                recovered_us
            );
        }
    }

    /// Off-grid µs values are bounded by half a sample period (~31 µs at 16 kHz).
    #[test]
    fn us_to_sample_error_bounded_by_half_sample() {
        let half_sample_us = (1_000_000.0 / SR / 2.0).ceil() as i64; // 32
        for us in [1_i64, 31, 62, 999, 1_000_000, 1_234_567, 60_000_000] {
            let s = us_to_sample(us, SR);
            let back = sample_to_us(s, SR);
            assert!(
                (back - us).abs() <= half_sample_us,
                "us {} → sample {} → us {} drifted more than {} µs",
                us,
                s,
                back,
                half_sample_us
            );
        }
    }

    /// (b) Boundary cases: zero, exact sample boundaries, half-sample between
    /// frames, and large values near `i64::MAX / 2`.
    #[test]
    fn boundary_cases() {
        // value = 0
        assert_eq!(sample_to_us(0, SR), 0);
        assert_eq!(us_to_sample(0, SR), 0);
        assert_eq!(seconds_to_us(0.0), 0);

        // value at sample boundary: sample 16_000 ↔ 1_000_000 µs
        assert_eq!(sample_to_us(16_000, SR), 1_000_000);
        assert_eq!(us_to_sample(1_000_000, SR), 16_000);

        // value at half-sample between frames (sample period = 62.5 µs).
        // Halfway between sample 0 and sample 1 is 31.25 µs. With
        // round-half-to-even, exact half-sample ties go to the nearest
        // even frame, but 31 µs (0.496 samples) and 32 µs (0.512 samples)
        // are not ties — they fall cleanly to 0 and 1 respectively.
        assert_eq!(us_to_sample(31, SR), 0); // 31 µs → 0.496 → 0
        assert_eq!(us_to_sample(32, SR), 1); // 32 µs → 0.512 → 1

        // Large values near i64::MAX / 2 must not panic and must remain finite.
        let big_us = i64::MAX / 4;
        let _ = us_to_sample(big_us, SR); // saturates to usize::MAX rather than UB
        let big_sample = (1_usize << 40).min(usize::MAX);
        let _ = sample_to_us(big_sample, SR);

        // Saturation on overflow: clearly out-of-range f64 → i64::MAX, not UB.
        assert_eq!(round_f64_to_i64(1e30), i64::MAX);
        assert_eq!(round_f64_to_i64(-1e30), i64::MIN);
        assert_eq!(round_f64_to_i64(f64::NAN), 0);
        assert_eq!(round_f64_to_usize(-1.0), 0);
        assert_eq!(round_f64_to_usize(f64::NAN), 0);
    }

    /// (c) `seconds_to_us` of `[0.0, 1.0]` yields exactly `[0, 1_000_000]`.
    #[test]
    fn seconds_to_us_unit_interval_is_exact() {
        assert_eq!(seconds_to_us(0.0), 0);
        assert_eq!(seconds_to_us(1.0), 1_000_000);
        // And a few more representative segment boundaries from real ASR output.
        assert_eq!(seconds_to_us(0.5), 500_000);
        assert_eq!(seconds_to_us(2.345), 2_345_000);
    }

    /// Round-trip is unbiased: errors are symmetric, not always in one direction.
    /// This is the *whole point* of the rounding policy — the previous
    /// truncation-based code biased every conversion downward.
    #[test]
    fn rounding_is_unbiased_over_many_values() {
        let mut signed_error_sum: i64 = 0;
        for us in 0_i64..10_000 {
            let s = us_to_sample(us, SR);
            let back = sample_to_us(s, SR);
            signed_error_sum += back - us;
        }
        // Truncation would accumulate a large negative bias here (~ -310_000 µs
        // over 10_000 samples). With nearest rounding the sum should be tiny.
        assert!(
            signed_error_sum.abs() < 100,
            "rounding policy is biased: net error over 10k µs = {} µs",
            signed_error_sum
        );
    }

    /// Half-open `[start, end)` rounding policy: both ends rounded with the
    /// same rule, so converting `[0.0, 1.0]` seconds to µs yields a duration
    /// of exactly 1_000_000 µs (no floor/ceil mix).
    #[test]
    fn half_open_range_duration_is_unbiased() {
        let start = seconds_to_us(0.123_456);
        let end = seconds_to_us(1.123_456);
        assert_eq!(end - start, 1_000_000);
    }
}
