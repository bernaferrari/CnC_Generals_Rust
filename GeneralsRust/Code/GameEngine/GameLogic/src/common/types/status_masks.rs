// Object status, special-power, and disabled masks
//
// Split from `types.rs` for module-size parity.
// Observable behavior is unchanged.

pub use generals_sim::object::ObjectStatusMaskType;

bitflags! {
    /// Special power mask (matching C++ SpecialPowerMaskType)
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub struct SpecialPowerMaskType: u32 {
        const SUPERWEAPON_A = 1 << 0;
        const SUPERWEAPON_B = 1 << 1;
        const SUPERWEAPON_C = 1 << 2;
        const CASH_HACK = 1 << 3;
        const RADAR_VAN_SCAN = 1 << 4;
        const SPY_SATELLITE = 1 << 5;
        const DISGUISE = 1 << 6;
        const RADAR_JAMMER = 1 << 7;
        // Add more as needed
    }
}

#[cfg(test)]
mod tests {
    use super::{
        EngineGeometryInfo, EngineGeometryType, GeometryExtentModType, GeometryInfo,
        ObjectStatusMaskType, engine_geometry_to_logic, geometry_type_from_u32,
        geometry_type_to_u32,
    };

    #[test]
    fn object_status_parse_tokens_matches_legacy_helper() {
        let mask = ObjectStatusMaskType::parse_tokens(["STEALTHED", "DETECTED"].iter().copied())
            .expect("parse succeeds");
        assert!(mask.contains(ObjectStatusMaskType::STEALTHED));
        assert!(mask.contains(ObjectStatusMaskType::DETECTED));
        assert!(!mask.contains(ObjectStatusMaskType::AFLAME));
    }

    #[test]
    fn object_status_parse_tokens_accepts_additive_modifiers() {
        let mask = ObjectStatusMaskType::parse_tokens(
            ["+STEALTHED", "+DETECTED", "-STEALTHED"].iter().copied(),
        )
        .expect("parse succeeds");
        assert!(mask.contains(ObjectStatusMaskType::DETECTED));
        assert!(!mask.contains(ObjectStatusMaskType::STEALTHED));
    }

    #[test]
    fn object_status_parse_tokens_errors_on_mixed_none() {
        let err = ObjectStatusMaskType::parse_tokens(["NONE", "STEALTHED"].iter().copied())
            .expect_err("mixing NONE with other tokens is invalid");
        assert!(
            err.contains("NONE"),
            "error message should reference NONE token"
        );
    }

    #[test]
    fn geometry_info_tweak_extents_cycles_type_and_clears_small_flag() {
        let mut geometry = GeometryInfo::default();
        geometry.shape.geometry_type = EngineGeometryType::Box;
        geometry.shape.is_small = true;
        geometry.tweak_extents(GeometryExtentModType::Type, 1.0);

        assert_eq!(geometry.shape.geometry_type, EngineGeometryType::Sphere);
        assert!(!geometry.shape.is_small);
    }

    #[test]
    fn geometry_type_round_trip_helpers_match_cpp_order() {
        assert_eq!(geometry_type_to_u32(EngineGeometryType::Sphere), 0);
        assert_eq!(geometry_type_to_u32(EngineGeometryType::Cylinder), 1);
        assert_eq!(geometry_type_to_u32(EngineGeometryType::Box), 2);

        assert_eq!(geometry_type_from_u32(0), EngineGeometryType::Sphere);
        assert_eq!(geometry_type_from_u32(1), EngineGeometryType::Cylinder);
        assert_eq!(geometry_type_from_u32(2), EngineGeometryType::Box);
    }

    #[test]
    fn engine_geometry_to_logic_preserves_type_and_small_flag() {
        // Geometry.cpp:61-85: sphere/cylinder use equal radii; box keeps both.
        // Inputs are authored radii, not diameters.
        for (kind, expected_minor, expected_height, expected_below) in [
            (EngineGeometryType::Sphere, 12.0, 12.0, 12.0),
            (EngineGeometryType::Cylinder, 12.0, 8.0, 0.0),
            (EngineGeometryType::Box, 4.0, 8.0, 0.0),
        ] {
            for small in [false, true] {
                let engine_geometry = EngineGeometryInfo::new(kind, small, 12.0, 8.0, 4.0);
                let logic_geometry = engine_geometry_to_logic(&engine_geometry);
                assert_eq!(logic_geometry.shape.geometry_type, kind);
                assert_eq!(logic_geometry.shape.is_small, small);
                assert_eq!(logic_geometry.get_major_radius(), 12.0);
                assert_eq!(logic_geometry.get_minor_radius(), expected_minor);
                assert_eq!(
                    logic_geometry.get_max_height_above_position(),
                    expected_height
                );
                assert_eq!(
                    logic_geometry.get_max_height_below_position(),
                    expected_below
                );
                let expected_circle = if kind == EngineGeometryType::Box {
                    160.0_f32.sqrt()
                } else {
                    12.0
                };
                let expected_sphere = if kind == EngineGeometryType::Box {
                    176.0_f32.sqrt()
                } else {
                    12.0
                };
                assert!(
                    (logic_geometry.get_bounding_circle_radius() - expected_circle).abs() < 1e-5
                );
                assert!(
                    (logic_geometry.get_bounding_sphere_radius() - expected_sphere).abs() < 1e-5
                );
            }
        }
    }
    #[test]
    fn geometry_radii_keep_cpp_axis_identity_and_shape_specific_bounds() {
        let box_geometry = engine_geometry_to_logic(&EngineGeometryInfo::new(
            EngineGeometryType::Box,
            false,
            4.0,
            8.0,
            12.0,
        ));
        assert_eq!(box_geometry.get_major_radius(), 4.0);
        assert_eq!(box_geometry.get_minor_radius(), 12.0);
        assert!((box_geometry.get_bounding_circle_radius() - 160.0_f32.sqrt()).abs() < 1e-5);
        assert!((box_geometry.get_bounding_sphere_radius() - 176.0_f32.sqrt()).abs() < 1e-5);

        let tall_cylinder = engine_geometry_to_logic(&EngineGeometryInfo::new(
            EngineGeometryType::Cylinder,
            false,
            4.0,
            20.0,
            12.0,
        ));
        assert_eq!(tall_cylinder.get_minor_radius(), 4.0);
        assert_eq!(tall_cylinder.get_bounding_circle_radius(), 4.0);
        assert_eq!(tall_cylinder.get_bounding_sphere_radius(), 10.0);
    }
}

impl SpecialPowerMaskType {
    pub fn none() -> Self {
        Self::empty()
    }
}

/// Disabled mask (matching C++ DisabledMaskType).
///
/// Definition and inherent helpers hoisted into `Common`
/// (`common::thing::update_module`) and re-exported here so the Common
/// `Module` trait can expose the typed `get_update_module_interface()`
/// accessor. Path kept stable for the existing call sites.
pub use game_engine::common::thing::update_module::DisabledMaskType;

/// Type alias for backward compatibility with C++ naming
pub type DisabledMask = DisabledMaskType;

/// ID type for ThingTemplates
pub type ThingTemplateId = u32;

/// ID type for UpgradeTemplates
pub type UpgradeTemplateId = u32;

/// Production ID for tracking unit construction
pub type ProductionID = u32;

/// Invalid production ID constant
pub const PRODUCTIONID_INVALID: ProductionID = 0;
