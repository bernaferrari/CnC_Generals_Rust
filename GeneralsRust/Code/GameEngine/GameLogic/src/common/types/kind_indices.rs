// Kind-of engine indices and geometry conversion
//
// Split from `types.rs` for module-size parity.
// Observable behavior is unchanged.

// Utility functions

pub use generals_sim::kind_of::classic::kind_of_indices;

fn engine_geometry_to_logic(info: &EngineGeometryInfo) -> GeometryInfo {
    GeometryInfo {
        shape: *info,
        height_above_terrain: info.get_max_height_above_position(),
        ..GeometryInfo::default()
    }
}

/// Test disabled mask function (matching C++ TEST_DISABLEDMASK)
pub fn test_disabled_mask(mask: DisabledMaskType, disabled_type: DisabledType) -> bool {
    mask.test(disabled_type)
}
