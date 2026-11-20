use math_utilities::wwmath_ids::*;

#[test]
fn persist_factory_chunk_ids_match_the_original_headers() {
    // Captured by compiling wwmathids.h with the original saveloadids.h.
    assert_eq!(CHUNKID_WWMATH_BEGIN, 0x0008_0000);
    assert_eq!(WWMATH_CHUNKID_LINEARCURVE1D, 0x0008_0000);
    assert_eq!(WWMATH_CHUNKID_HERMITESPLINE1D, 0x0008_0001);
    assert_eq!(WWMATH_CHUNKID_CATMULLROMSPLINE1D, 0x0008_0002);
    assert_eq!(WWMATH_CHUNKID_CARDINALSPLINE1D, 0x0008_0003);
    assert_eq!(WWMATH_CHUNKID_TCBSPLINE1D, 0x0008_0004);
    assert_eq!(WWMATH_CHUNKID_LINEARCURVE3D, 0x0008_0100);
    assert_eq!(WWMATH_CHUNKID_HERMITESPLINE3D, 0x0008_0101);
    assert_eq!(WWMATH_CHUNKID_CATMULLROMSPLINE3D, 0x0008_0102);
    assert_eq!(WWMATH_CHUNKID_CARDINALSPLINE3D, 0x0008_0103);
    assert_eq!(WWMATH_CHUNKID_TCBSPLINE3D, 0x0008_0104);
    assert_eq!(WWMATH_CHUNKID_VEHICLECURVE, 0x0008_0105);
}
