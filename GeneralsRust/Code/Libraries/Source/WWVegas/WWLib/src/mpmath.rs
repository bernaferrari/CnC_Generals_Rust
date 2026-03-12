//! Multi-precision math utilities (high-level replacement for original WWLib mpmath).
//!
//! This module provides constants and re-exports used by the BigInt implementation.
//! The original WWLib exposed many low-level digit-array routines; the Rust port
//! uses `num-bigint` internally and surfaces equivalent high-level behavior.

pub use crate::int::{generate_prime, BigInt, RemainderTable};

pub const UNITSIZE: u32 = 32;
pub const MAX_BIT_PRECISION: u32 = 2048;

pub type Digit = u32;

pub fn der_encode(value: &BigInt) -> Vec<u8> {
    value.der_encode()
}

pub fn der_decode(input: &[u8]) -> Option<BigInt> {
    BigInt::der_decode(input)
}
