//! Persisted chunk identifiers used by the original WWMath serialization layer.
//!
//! These constants mirror the values defined in `wwmathids.h` and `saveloadids.h`
//! from the C++ codebase so that any tooling or data formats that rely on the
//! legacy chunk layout remain compatible.

/// Base chunk identifier reserved for WWMath content (`0x0008_0000`).
pub const CHUNKID_WWMATH_BEGIN: u32 = 0x0008_0000;

/// Chunk identifiers for 1D curve types.
pub const WWMATH_CHUNKID_LINEARCURVE1D: u32 = CHUNKID_WWMATH_BEGIN;
pub const WWMATH_CHUNKID_HERMITESPLINE1D: u32 = CHUNKID_WWMATH_BEGIN + 0x001;
pub const WWMATH_CHUNKID_CATMULLROMSPLINE1D: u32 = CHUNKID_WWMATH_BEGIN + 0x002;
pub const WWMATH_CHUNKID_CARDINALSPLINE1D: u32 = CHUNKID_WWMATH_BEGIN + 0x003;
pub const WWMATH_CHUNKID_TCBSPLINE1D: u32 = CHUNKID_WWMATH_BEGIN + 0x004;

/// Chunk identifiers for 3D curve types.
pub const WWMATH_CHUNKID_LINEARCURVE3D: u32 = CHUNKID_WWMATH_BEGIN + 0x100;
pub const WWMATH_CHUNKID_HERMITESPLINE3D: u32 = CHUNKID_WWMATH_BEGIN + 0x101;
pub const WWMATH_CHUNKID_CATMULLROMSPLINE3D: u32 = CHUNKID_WWMATH_BEGIN + 0x102;
pub const WWMATH_CHUNKID_CARDINALSPLINE3D: u32 = CHUNKID_WWMATH_BEGIN + 0x103;
pub const WWMATH_CHUNKID_TCBSPLINE3D: u32 = CHUNKID_WWMATH_BEGIN + 0x104;
pub const WWMATH_CHUNKID_VEHICLECURVE: u32 = CHUNKID_WWMATH_BEGIN + 0x105;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn chunk_id_layout_matches_cpp() {
        assert_eq!(WWMATH_CHUNKID_LINEARCURVE1D, CHUNKID_WWMATH_BEGIN + 0x000);
        assert_eq!(WWMATH_CHUNKID_LINEARCURVE3D, CHUNKID_WWMATH_BEGIN + 0x100);
        assert_eq!(WWMATH_CHUNKID_VEHICLECURVE, CHUNKID_WWMATH_BEGIN + 0x105);
    }
}
