//! Persisted chunk identifiers used by the original WWMath serialization layer.
//!
//! These constants mirror the values defined in `wwmathids.h` and `saveloadids.h`
//! from the C++ codebase so that any tooling or data formats that rely on the
//! legacy chunk layout remain compatible.

/// Base chunk identifier reserved for WWMath content (`0x0008_0000`).
pub const CHUNKID_WWMATH_BEGIN: u32 = 0x0008_0000;

/// Persist factory ID for a one-dimensional linear curve.
pub const WWMATH_CHUNKID_LINEARCURVE1D: u32 = CHUNKID_WWMATH_BEGIN;
/// Persist factory ID for a one-dimensional Hermite spline.
pub const WWMATH_CHUNKID_HERMITESPLINE1D: u32 = CHUNKID_WWMATH_BEGIN + 0x001;
/// Persist factory ID for a one-dimensional Catmull–Rom spline.
pub const WWMATH_CHUNKID_CATMULLROMSPLINE1D: u32 = CHUNKID_WWMATH_BEGIN + 0x002;
/// Persist factory ID for a one-dimensional cardinal spline.
pub const WWMATH_CHUNKID_CARDINALSPLINE1D: u32 = CHUNKID_WWMATH_BEGIN + 0x003;
/// Persist factory ID for a one-dimensional tension–continuity–bias spline.
pub const WWMATH_CHUNKID_TCBSPLINE1D: u32 = CHUNKID_WWMATH_BEGIN + 0x004;

/// Persist factory ID for a three-dimensional linear curve.
pub const WWMATH_CHUNKID_LINEARCURVE3D: u32 = CHUNKID_WWMATH_BEGIN + 0x100;
/// Persist factory ID for a three-dimensional Hermite spline.
pub const WWMATH_CHUNKID_HERMITESPLINE3D: u32 = CHUNKID_WWMATH_BEGIN + 0x101;
/// Persist factory ID for a three-dimensional Catmull–Rom spline.
pub const WWMATH_CHUNKID_CATMULLROMSPLINE3D: u32 = CHUNKID_WWMATH_BEGIN + 0x102;
/// Persist factory ID for a three-dimensional cardinal spline.
pub const WWMATH_CHUNKID_CARDINALSPLINE3D: u32 = CHUNKID_WWMATH_BEGIN + 0x103;
/// Persist factory ID for a three-dimensional tension–continuity–bias spline.
pub const WWMATH_CHUNKID_TCBSPLINE3D: u32 = CHUNKID_WWMATH_BEGIN + 0x104;
/// Persist factory ID for a vehicle curve.
pub const WWMATH_CHUNKID_VEHICLECURVE: u32 = CHUNKID_WWMATH_BEGIN + 0x105;
