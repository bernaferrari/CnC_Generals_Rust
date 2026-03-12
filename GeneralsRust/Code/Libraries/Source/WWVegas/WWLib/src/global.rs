//! RSAREF types and constants mirroring WWLib `global.h`.

/// Generic pointer type.
pub type POINTER = *mut u8;
/// Two-byte word.
pub type UINT2 = u16;
/// Four-byte word.
pub type UINT4 = u32;

/// PROTOTYPES flag (always true in Rust).
pub const PROTOTYPES: bool = true;
