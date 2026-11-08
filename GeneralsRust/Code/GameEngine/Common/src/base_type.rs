//! C++ `Lib/BaseType.h` float-to-integer conversions.
//!
//! The original `fast_float_trunc` masks mantissa bits directly
//! (BaseType.h:178-192), `fast_float_floor`/`fast_float_ceil` first add or
//! subtract the "almost one" bit pattern `(126<<23)|0x7fffff` (BaseType.h:195-210),
//! and `REAL_TO_INT_*` then `FISTP` the already-integral float
//! (BaseType.h:164-175, 213-226). `FISTP` of an integral value is exact in every
//! rounding mode, so the only mode-sensitive step is the almost-one add/subtract.
//! `GameLogic.cpp:174-193 setFPMode` selects `_RC_NEAR` with `_PC_24`, so that
//! step is single-precision round-to-nearest-even, which is exactly Rust `f32`
//! arithmetic. Consequences that differ from `f32::floor`/`f32::ceil`:
//! `REAL_TO_INT_CEIL(k) == k + 1` for integral `k >= 1`, `REAL_TO_INT_FLOOR(-1.0)
//! == -2`, and tiny negatives such as `-1e-8` floor to `0`.

/// `0x3F7FFFFF`, the largest float below 1.0 (BaseType.h:197/206).
const ALMOST_ONE: f32 = f32::from_bits((126 << 23) | 0x007f_ffff);

/// C++ `fast_float_trunc`: clear the fractional mantissa bits.
///
/// `shr ecx,23; sub cl,127` uses the biased exponent's low byte; a borrow
/// (exponent < 127) zeroes the value, otherwise `sar` by `cl & 31` (x86
/// shift-count masking) builds the keep-mask.
#[inline]
pub fn fast_float_trunc(f: f32) -> f32 {
    let bits = f.to_bits();
    let exponent = (bits >> 23) as u8;
    let mask = if exponent < 127 {
        0
    } else {
        let shift = u32::from(exponent.wrapping_sub(127)) & 31;
        ((0xff80_0000_u32 as i32) >> shift) as u32
    };
    f32::from_bits(bits & mask)
}

/// C++ `fast_float_floor`.
#[inline]
pub fn fast_float_floor(f: f32) -> f32 {
    let f = if f.to_bits() & 0x8000_0000 != 0 {
        f - ALMOST_ONE
    } else {
        f
    };
    fast_float_trunc(f)
}

/// C++ `fast_float_ceil`.
#[inline]
pub fn fast_float_ceil(f: f32) -> f32 {
    let f = if f.to_bits() & 0x8000_0000 == 0 {
        f + ALMOST_ONE
    } else {
        f
    };
    fast_float_trunc(f)
}

/// C++ `fast_float2long_round` (`FLD`/`FISTP` into a 32-bit `long`) for the
/// integral inputs every `REAL_TO_INT_*` macro supplies. Out-of-range and NaN
/// values store the x87 integer-indefinite value `0x80000000`.
#[inline]
pub fn fast_float2long_round_integral(f: f32) -> i32 {
    if f >= -2_147_483_648.0 && f < 2_147_483_648.0 {
        f as i32
    } else {
        i32::MIN
    }
}

/// C++ `REAL_TO_INT(x)`: truncation toward zero.
#[inline]
pub fn real_to_int(f: f32) -> i32 {
    fast_float2long_round_integral(fast_float_trunc(f))
}

/// C++ `REAL_TO_INT_FLOOR(x)`.
#[inline]
pub fn real_to_int_floor(f: f32) -> i32 {
    fast_float2long_round_integral(fast_float_floor(f))
}

/// C++ `REAL_TO_INT_CEIL(x)`.
#[inline]
pub fn real_to_int_ceil(f: f32) -> i32 {
    fast_float2long_round_integral(fast_float_ceil(f))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn trunc_masks_mantissa_like_basetype_asm() {
        assert_eq!(fast_float_trunc(2.75), 2.0);
        assert_eq!(fast_float_trunc(-2.75), -2.0);
        assert_eq!(fast_float_trunc(0.999).to_bits(), 0);
        assert_eq!(fast_float_trunc(-0.5).to_bits(), 0);
        assert_eq!(fast_float_trunc(8_388_609.0), 8_388_609.0);
        // Exponent 159 (2^32): shift count wraps to 0, keeping only sign+exponent.
        assert_eq!(fast_float_trunc(6_000_000_000.0), 4_294_967_296.0);
    }

    #[test]
    fn floor_matches_almost_one_subtract_under_round_nearest() {
        assert_eq!(real_to_int_floor(0.0), 0);
        assert_eq!(real_to_int_floor(9.99), 9);
        assert_eq!(real_to_int_floor(10.0), 10);
        assert_eq!(real_to_int_floor(-0.5), -1);
        assert_eq!(real_to_int_floor(-2.0), -3);
        // -1 - 0.99999994 ties to even at -2.0, so the source floors -1.0 to -2.
        assert_eq!(real_to_int_floor(-1.0), -2);
        // -1e-8 - 0.99999994 rounds back to -0.99999994, which truncates to 0.
        assert_eq!(real_to_int_floor(-1e-8), 0);
        assert_eq!(real_to_int_floor(-0.0), 0);
    }

    #[test]
    fn ceil_adds_one_at_positive_integral_boundaries() {
        assert_eq!(real_to_int_ceil(0.0), 0);
        assert_eq!(real_to_int_ceil(0.4), 1);
        assert_eq!(real_to_int_ceil(1.0), 2);
        assert_eq!(real_to_int_ceil(2.0), 3);
        assert_eq!(real_to_int_ceil(100.0), 101);
        assert_eq!(real_to_int_ceil(1e-8), 0);
        assert_eq!(real_to_int_ceil(-1.5), -1);
        assert_eq!(real_to_int_ceil(-1.0), -1);
    }

    #[test]
    fn fistp_out_of_range_is_integer_indefinite() {
        assert_eq!(real_to_int(3.0e9), i32::MIN);
        assert_eq!(real_to_int(f32::NAN), i32::MIN);
        assert_eq!(real_to_int(-7.9), -7);
    }
}
