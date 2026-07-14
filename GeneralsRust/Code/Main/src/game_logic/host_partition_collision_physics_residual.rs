//! Wave 96 residual peels: partition / collision / physics / projectile residual deepen.
//!
//! Host-testable residual for fuller ZH PartitionManager, GeometryInfo collision bounds,
//! PhysicsBehavior defaults, and DumbProjectileBehavior residual without full exclusive
//! module graph / live flight:
//! 1. Partition residual peels (HUGE_DIST, DistanceCalculationType, FindPositionFlags,
//!    DirtyStatus, ValueOrThreat, RelationshipAllowTypes, PartitionFilter name table,
//!    contact-list socket count, cell-size cross-link)
//! 2. Collision residual peels (GeometryInfo bounding formulas, max/min height, footprint
//!    area, collide-test matrix ordering keyed off GeometryType, CollideModule ground NULL)
//! 3. Physics residual peels (PhysicsBehaviorModuleData defaults, friction clamps,
//!    PhysicsTurningType, PhysicsFlagsType bits, motive frames, height→speed residual)
//! 4. Projectile residual deepen (DumbProjectileBehaviorModuleData defaults,
//!    DEFAULT_MAX_LIFESPAN, bezier/angle residual constants)
//!
//! Fail-closed:
//! - Not full PartitionManager filter stack / live COI registration residual
//! - Not full CollideModule partition pair dispatch / live onCollide graph
//! - Not full PhysicsBehavior motive force / bounce exclusive residual
//! - Not full DumbProjectileBehavior live Bezier flight / ThingFactory Object residual
//! - Network residual deferred; shell playable_claim stays false

use std::f32::consts::PI;

// ---------------------------------------------------------------------------
// Shared logic-frame residual (GameCommon.h LOGICFRAMES_PER_SECOND)
// ---------------------------------------------------------------------------

/// C++ `LOGICFRAMES_PER_SECOND` residual.
pub const LOGIC_FRAMES_PER_SECOND_RESIDUAL: u32 = 30;

/// C++ `SECONDS_PER_LOGICFRAME_REAL` residual (= 1/30).
pub const SECONDS_PER_LOGIC_FRAME_RESIDUAL: f32 = 1.0 / 30.0;

// ---------------------------------------------------------------------------
// 1. Partition residual peels (PartitionManager.h / .cpp)
// ---------------------------------------------------------------------------

/// C++ `HUGE_DIST` residual.
pub const HUGE_DIST_RESIDUAL: f32 = 1_000_000.0;

/// C++ `HUGE_DIST_SQR` residual (`HUGE_DIST * HUGE_DIST`).
pub const HUGE_DIST_SQR_RESIDUAL: f32 = HUGE_DIST_RESIDUAL * HUGE_DIST_RESIDUAL;

/// C++ `RANDOM_START_ANGLE` residual (sentinel "no start angle").
pub const RANDOM_START_ANGLE_RESIDUAL: f32 = -99999.9;

/// Retail GameData.ini `PartitionCellSize` residual (cross-link Wave 86).
pub const PARTITION_CELL_SIZE_RESIDUAL: f32 = 40.0;

/// C++ `PartitionContactList_SOCKET_COUNT` residual (prime bucket count).
pub const PARTITION_CONTACT_LIST_SOCKET_COUNT_RESIDUAL: u32 = 5381;

/// C++ `DistanceCalculationType` residual count.
pub const DISTANCE_CALCULATION_TYPE_COUNT: u32 = 4;

/// Ordered C++ `DistanceCalculationType` residual names.
pub const DISTANCE_CALCULATION_TYPE_NAME_LIST: &[&str] = &[
    "FROM_CENTER_2D",
    "FROM_CENTER_3D",
    "FROM_BOUNDINGSPHERE_2D",
    "FROM_BOUNDINGSPHERE_3D",
];

/// C++ `FROM_CENTER_2D` residual ordinal.
pub const FROM_CENTER_2D: u32 = 0;
/// C++ `FROM_CENTER_3D` residual ordinal.
pub const FROM_CENTER_3D: u32 = 1;
/// C++ `FROM_BOUNDINGSPHERE_2D` residual ordinal.
pub const FROM_BOUNDINGSPHERE_2D: u32 = 2;
/// C++ `FROM_BOUNDINGSPHERE_3D` residual ordinal.
pub const FROM_BOUNDINGSPHERE_3D: u32 = 3;

/// Lookup DistanceCalculationType name index residual.
pub fn distance_calculation_type_name_index(name: &str) -> Option<usize> {
    DISTANCE_CALCULATION_TYPE_NAME_LIST
        .iter()
        .position(|&n| n.eq_ignore_ascii_case(name))
}

/// C++ `ValueOrThreat` residual (VOT_CashValue = 1 … VOT_NumItems sentinel).
pub const VOT_CASH_VALUE: u32 = 1;
/// C++ `VOT_ThreatValue` residual.
pub const VOT_THREAT_VALUE: u32 = 2;
/// C++ `VOT_NumItems` residual (sentinel after last value).
pub const VOT_NUM_ITEMS: u32 = 3;

/// C++ `FindPositionFlags` residual bits.
pub const FPF_NONE: u32 = 0x0000_0000;
/// C++ `FPF_IGNORE_WATER` residual bit.
pub const FPF_IGNORE_WATER: u32 = 0x0000_0001;
/// C++ `FPF_WATER_ONLY` residual bit.
pub const FPF_WATER_ONLY: u32 = 0x0000_0002;
/// C++ `FPF_IGNORE_ALL_OBJECTS` residual bit.
pub const FPF_IGNORE_ALL_OBJECTS: u32 = 0x0000_0004;
/// C++ `FPF_IGNORE_ALLY_OR_NEUTRAL_UNITS` residual bit.
pub const FPF_IGNORE_ALLY_OR_NEUTRAL_UNITS: u32 = 0x0000_0008;
/// C++ `FPF_IGNORE_ALLY_OR_NEUTRAL_STRUCTURES` residual bit.
pub const FPF_IGNORE_ALLY_OR_NEUTRAL_STRUCTURES: u32 = 0x0000_0010;
/// C++ `FPF_IGNORE_ENEMY_UNITS` residual bit.
pub const FPF_IGNORE_ENEMY_UNITS: u32 = 0x0000_0020;
/// C++ `FPF_IGNORE_ENEMY_STRUCTURES` residual bit.
pub const FPF_IGNORE_ENEMY_STRUCTURES: u32 = 0x0000_0040;
/// C++ `FPF_USE_HIGHEST_LAYER` residual bit.
pub const FPF_USE_HIGHEST_LAYER: u32 = 0x0000_0080;
/// C++ `FPF_CLEAR_CELLS_ONLY` residual bit.
pub const FPF_CLEAR_CELLS_ONLY: u32 = 0x0000_0100;

/// Ordered C++ `FindPositionFlags` residual bit-name list (excluding FPF_NONE).
pub const FIND_POSITION_FLAG_NAME_LIST: &[&str] = &[
    "FPF_IGNORE_WATER",
    "FPF_WATER_ONLY",
    "FPF_IGNORE_ALL_OBJECTS",
    "FPF_IGNORE_ALLY_OR_NEUTRAL_UNITS",
    "FPF_IGNORE_ALLY_OR_NEUTRAL_STRUCTURES",
    "FPF_IGNORE_ENEMY_UNITS",
    "FPF_IGNORE_ENEMY_STRUCTURES",
    "FPF_USE_HIGHEST_LAYER",
    "FPF_CLEAR_CELLS_ONLY",
];

/// Residual: convert FindPositionFlags name index to bit value (`1 << index`).
pub fn find_position_flag_bit_value(name: &str) -> Option<u32> {
    FIND_POSITION_FLAG_NAME_LIST
        .iter()
        .position(|&n| n.eq_ignore_ascii_case(name))
        .map(|i| 1u32 << i)
}

/// C++ PartitionData `DirtyStatus` residual ordinals.
pub const DIRTY_STATUS_NOT_DIRTY: u32 = 0;
/// C++ `NEED_COLLISION_CHECK` residual.
pub const DIRTY_STATUS_NEED_COLLISION_CHECK: u32 = 1;
/// C++ `NEED_CELL_UPDATE_AND_COLLISION_CHECK` residual.
pub const DIRTY_STATUS_NEED_CELL_UPDATE_AND_COLLISION_CHECK: u32 = 2;

/// Ordered C++ DirtyStatus residual names.
pub const DIRTY_STATUS_NAME_LIST: &[&str] = &[
    "NOT_DIRTY",
    "NEED_COLLISION_CHECK",
    "NEED_CELL_UPDATE_AND_COLLISION_CHECK",
];

/// C++ Relationship residual ordinals (cross-link Wave 84).
pub const RELATIONSHIP_ENEMIES: u32 = 0;
/// C++ NEUTRAL residual ordinal.
pub const RELATIONSHIP_NEUTRAL: u32 = 1;
/// C++ ALLIES residual ordinal.
pub const RELATIONSHIP_ALLIES: u32 = 2;

/// C++ `PartitionFilterRelationship::ALLOW_ENEMIES` residual (`1 << ENEMIES`).
pub const ALLOW_ENEMIES: u32 = 1 << RELATIONSHIP_ENEMIES;
/// C++ `ALLOW_NEUTRAL` residual (`1 << NEUTRAL`).
pub const ALLOW_NEUTRAL: u32 = 1 << RELATIONSHIP_NEUTRAL;
/// C++ `ALLOW_ALLIES` residual (`1 << ALLIES`).
pub const ALLOW_ALLIES: u32 = 1 << RELATIONSHIP_ALLIES;

/// C++ concrete PartitionFilter residual class-name table (declaration order).
///
/// Base `PartitionFilter` is abstract and not listed. Count = **33**.
pub const PARTITION_FILTER_RESIDUAL_NAME_COUNT: usize = 33;

/// Ordered C++ PartitionFilter residual names (PartitionManager.h).
pub const PARTITION_FILTER_RESIDUAL_NAME_LIST: &[&str] = &[
    "PartitionFilterIsFlying",
    "PartitionFilterWouldCollide",
    "PartitionFilterSamePlayer",
    "PartitionFilterRelationship",
    "PartitionFilterAcceptOnTeam",
    "PartitionFilterAcceptOnSquad",
    "PartitionFilterLineOfSight",
    "PartitionFilterPossibleToAttack",
    "PartitionFilterPossibleToEnter",
    "PartitionFilterPossibleToHijack",
    "PartitionFilterLastAttackedBy",
    "PartitionFilterAcceptByObjectStatus",
    "PartitionFilterRejectByObjectStatus",
    "PartitionFilterStealthedAndUndetected",
    "PartitionFilterAcceptByKindOf",
    "PartitionFilterRejectByKindOf",
    "PartitionFilterRejectBehind",
    "PartitionFilterAlive",
    "PartitionFilterSameMapStatus",
    "PartitionFilterOnMap",
    "PartitionFilterRejectBuildings",
    "PartitionFilterInsignificantBuildings",
    "PartitionFilterFreeOfFog",
    "PartitionFilterRepulsor",
    "PartitionFilterIrregularArea",
    "PartitionFilterPolygonTrigger",
    "PartitionFilterPlayer",
    "PartitionFilterPlayerAffiliation",
    "PartitionFilterThing",
    "PartitionFilterGarrisonable",
    "PartitionFilterGarrisonableByPlayer",
    "PartitionFilterUnmannedObject",
    "PartitionFilterValidCommandButtonTarget",
];

/// Whether a PartitionFilter residual class name is known.
pub fn partition_filter_residual_name_known(name: &str) -> bool {
    PARTITION_FILTER_RESIDUAL_NAME_LIST
        .iter()
        .any(|n| n.eq_ignore_ascii_case(name))
}

/// Wave 96 honesty: Partition residual peels pack.
pub fn honesty_partition_residual_pack_wave96() -> bool {
    HUGE_DIST_RESIDUAL == 1_000_000.0
        && HUGE_DIST_SQR_RESIDUAL == 1_000_000.0 * 1_000_000.0
        && (RANDOM_START_ANGLE_RESIDUAL - (-99999.9)).abs() < 0.01
        && PARTITION_CELL_SIZE_RESIDUAL == 40.0
        && PARTITION_CONTACT_LIST_SOCKET_COUNT_RESIDUAL == 5381
        // DistanceCalculationType residual table.
        && DISTANCE_CALCULATION_TYPE_COUNT == 4
        && DISTANCE_CALCULATION_TYPE_NAME_LIST.len() == 4
        && DISTANCE_CALCULATION_TYPE_NAME_LIST[0] == "FROM_CENTER_2D"
        && DISTANCE_CALCULATION_TYPE_NAME_LIST[3] == "FROM_BOUNDINGSPHERE_3D"
        && FROM_CENTER_2D == 0
        && FROM_CENTER_3D == 1
        && FROM_BOUNDINGSPHERE_2D == 2
        && FROM_BOUNDINGSPHERE_3D == 3
        && distance_calculation_type_name_index("FROM_CENTER_2D") == Some(0)
        && distance_calculation_type_name_index("FROM_BOUNDINGSPHERE_3D") == Some(3)
        && distance_calculation_type_name_index("FROM_EDGE").is_none()
        // ValueOrThreat residual (1-based values + NumItems sentinel).
        && VOT_CASH_VALUE == 1
        && VOT_THREAT_VALUE == 2
        && VOT_NUM_ITEMS == 3
        // FindPositionFlags residual bits.
        && FPF_NONE == 0
        && FPF_IGNORE_WATER == 0x01
        && FPF_WATER_ONLY == 0x02
        && FPF_IGNORE_ALL_OBJECTS == 0x04
        && FPF_IGNORE_ALLY_OR_NEUTRAL_UNITS == 0x08
        && FPF_IGNORE_ALLY_OR_NEUTRAL_STRUCTURES == 0x10
        && FPF_IGNORE_ENEMY_UNITS == 0x20
        && FPF_IGNORE_ENEMY_STRUCTURES == 0x40
        && FPF_USE_HIGHEST_LAYER == 0x80
        && FPF_CLEAR_CELLS_ONLY == 0x100
        && FIND_POSITION_FLAG_NAME_LIST.len() == 9
        && find_position_flag_bit_value("FPF_IGNORE_WATER") == Some(0x01)
        && find_position_flag_bit_value("FPF_CLEAR_CELLS_ONLY") == Some(0x100)
        && find_position_flag_bit_value("FPF_NONE").is_none()
        // DirtyStatus residual ordinals.
        && DIRTY_STATUS_NOT_DIRTY == 0
        && DIRTY_STATUS_NEED_COLLISION_CHECK == 1
        && DIRTY_STATUS_NEED_CELL_UPDATE_AND_COLLISION_CHECK == 2
        && DIRTY_STATUS_NAME_LIST.len() == 3
        && DIRTY_STATUS_NAME_LIST[0] == "NOT_DIRTY"
        && DIRTY_STATUS_NAME_LIST[2] == "NEED_CELL_UPDATE_AND_COLLISION_CHECK"
        // RelationshipAllowTypes residual (bit = 1 << Relationship ordinal).
        && ALLOW_ENEMIES == 0x01
        && ALLOW_NEUTRAL == 0x02
        && ALLOW_ALLIES == 0x04
        // PartitionFilter residual name table.
        && PARTITION_FILTER_RESIDUAL_NAME_LIST.len() == PARTITION_FILTER_RESIDUAL_NAME_COUNT
        && PARTITION_FILTER_RESIDUAL_NAME_COUNT == 33
        && PARTITION_FILTER_RESIDUAL_NAME_LIST[0] == "PartitionFilterIsFlying"
        && PARTITION_FILTER_RESIDUAL_NAME_LIST[3] == "PartitionFilterRelationship"
        && PARTITION_FILTER_RESIDUAL_NAME_LIST[18] == "PartitionFilterSameMapStatus"
        && PARTITION_FILTER_RESIDUAL_NAME_LIST[26] == "PartitionFilterPlayer"
        && PARTITION_FILTER_RESIDUAL_NAME_LIST[32]
            == "PartitionFilterValidCommandButtonTarget"
        && partition_filter_residual_name_known("PartitionFilterPlayer")
        && partition_filter_residual_name_known("PartitionFilterUnmannedObject")
        // Fail-closed: abstract base not in residual table.
        && !partition_filter_residual_name_known("PartitionFilter")
        && !partition_filter_residual_name_known("PartitionFilterBogus")
        // Unique names residual.
        && {
            let mut seen = std::collections::HashSet::new();
            PARTITION_FILTER_RESIDUAL_NAME_LIST
                .iter()
                .all(|n| seen.insert(*n))
        }
}

// ---------------------------------------------------------------------------
// 2. Collision residual peels (Geometry.cpp + PartitionManager collide matrix)
// ---------------------------------------------------------------------------

/// C++ `GEOMETRY_NUM_TYPES` residual (cross-link Wave 84).
pub const GEOMETRY_NUM_TYPES_RESIDUAL: u32 = 3;
/// C++ `GEOMETRY_SPHERE` residual ordinal.
pub const GEOMETRY_SPHERE_RESIDUAL: u32 = 0;
/// C++ `GEOMETRY_CYLINDER` residual ordinal.
pub const GEOMETRY_CYLINDER_RESIDUAL: u32 = 1;
/// C++ `GEOMETRY_BOX` residual ordinal.
pub const GEOMETRY_BOX_RESIDUAL: u32 = 2;
/// C++ `GEOMETRY_FIRST` residual (= SPHERE).
pub const GEOMETRY_FIRST_RESIDUAL: u32 = GEOMETRY_SPHERE_RESIDUAL;

/// Ordered C++ `theCollideTestProcs` residual names (row-major GeometryType × GeometryType).
///
/// Index = `(thisGeom - GEOMETRY_FIRST) * GEOMETRY_NUM_TYPES + (thatGeom - GEOMETRY_FIRST)`.
/// Order **depends** on GeometryType enum starting at 0 and count = 3.
pub const COLLIDE_TEST_PROC_NAME_LIST: &[&str] = &[
    "collideTest_Sphere_Sphere",
    "collideTest_Sphere_Cylinder",
    "collideTest_Sphere_Box",
    "collideTest_Cylinder_Sphere",
    "collideTest_Cylinder_Cylinder",
    "collideTest_Cylinder_Box",
    "collideTest_Box_Sphere",
    "collideTest_Box_Cylinder",
    "collideTest_Box_Box",
];

/// Residual collide-test matrix index for two geometry types.
pub fn collide_test_proc_index(this_geom: u32, that_geom: u32) -> Option<usize> {
    if this_geom >= GEOMETRY_NUM_TYPES_RESIDUAL || that_geom >= GEOMETRY_NUM_TYPES_RESIDUAL {
        return None;
    }
    Some((this_geom * GEOMETRY_NUM_TYPES_RESIDUAL + that_geom) as usize)
}

/// Residual: geometry bounding circle radius (2d).
///
/// Sphere/Cylinder → majorRadius; Box → sqrt(major² + minor²).
pub fn geometry_bounding_circle_radius(
    geom_type: u32,
    major_radius: f32,
    minor_radius: f32,
) -> Option<f32> {
    match geom_type {
        GEOMETRY_SPHERE_RESIDUAL | GEOMETRY_CYLINDER_RESIDUAL => Some(major_radius),
        GEOMETRY_BOX_RESIDUAL => Some((major_radius * major_radius + minor_radius * minor_radius).sqrt()),
        _ => None,
    }
}

/// Residual: geometry bounding sphere radius (3d).
///
/// Sphere → majorRadius; Cylinder → max(height/2, majorRadius);
/// Box → sqrt(major² + minor² + (height/2)²).
pub fn geometry_bounding_sphere_radius(
    geom_type: u32,
    height: f32,
    major_radius: f32,
    minor_radius: f32,
) -> Option<f32> {
    match geom_type {
        GEOMETRY_SPHERE_RESIDUAL => Some(major_radius),
        GEOMETRY_CYLINDER_RESIDUAL => Some((height * 0.5).max(major_radius)),
        GEOMETRY_BOX_RESIDUAL => {
            let half_h = height * 0.5;
            Some(
                (major_radius * major_radius
                    + minor_radius * minor_radius
                    + half_h * half_h)
                    .sqrt(),
            )
        }
        _ => None,
    }
}

/// Residual: `getMaxHeightAbovePosition` (Sphere→majorRadius; Box/Cylinder→height).
pub fn geometry_max_height_above_position(geom_type: u32, height: f32, major_radius: f32) -> Option<f32> {
    match geom_type {
        GEOMETRY_SPHERE_RESIDUAL => Some(major_radius),
        GEOMETRY_BOX_RESIDUAL | GEOMETRY_CYLINDER_RESIDUAL => Some(height),
        _ => None,
    }
}

/// Residual: `getMaxHeightBelowPosition` (Sphere→majorRadius; Box/Cylinder→0).
pub fn geometry_max_height_below_position(geom_type: u32, major_radius: f32) -> Option<f32> {
    match geom_type {
        GEOMETRY_SPHERE_RESIDUAL => Some(major_radius),
        GEOMETRY_BOX_RESIDUAL | GEOMETRY_CYLINDER_RESIDUAL => Some(0.0),
        _ => None,
    }
}

/// Residual: `getZDeltaToCenterPosition` (Sphere→0; else height/2).
pub fn geometry_z_delta_to_center(geom_type: u32, height: f32) -> Option<f32> {
    match geom_type {
        GEOMETRY_SPHERE_RESIDUAL => Some(0.0),
        GEOMETRY_BOX_RESIDUAL | GEOMETRY_CYLINDER_RESIDUAL => Some(height * 0.5),
        _ => None,
    }
}

/// Residual: `getFootprintArea` (Sphere/Cylinder→π r²; Box→4*major*minor).
pub fn geometry_footprint_area(
    geom_type: u32,
    major_radius: f32,
    minor_radius: f32,
    bounding_circle_radius: f32,
) -> Option<f32> {
    match geom_type {
        GEOMETRY_SPHERE_RESIDUAL | GEOMETRY_CYLINDER_RESIDUAL => {
            Some(PI * bounding_circle_radius * bounding_circle_radius)
        }
        GEOMETRY_BOX_RESIDUAL => Some(4.0 * major_radius * minor_radius),
        _ => None,
    }
}

/// Wave 96 honesty: Collision / GeometryInfo residual peels pack.
pub fn honesty_collision_residual_pack_wave96() -> bool {
    // Geometry type residual anchors (cross-link Wave 84).
    GEOMETRY_NUM_TYPES_RESIDUAL == 3
        && GEOMETRY_SPHERE_RESIDUAL == 0
        && GEOMETRY_CYLINDER_RESIDUAL == 1
        && GEOMETRY_BOX_RESIDUAL == 2
        && GEOMETRY_FIRST_RESIDUAL == 0
        // Collide-test matrix residual 3×3.
        && COLLIDE_TEST_PROC_NAME_LIST.len() == 9
        && COLLIDE_TEST_PROC_NAME_LIST[0] == "collideTest_Sphere_Sphere"
        && COLLIDE_TEST_PROC_NAME_LIST[4] == "collideTest_Cylinder_Cylinder"
        && COLLIDE_TEST_PROC_NAME_LIST[8] == "collideTest_Box_Box"
        && collide_test_proc_index(GEOMETRY_SPHERE_RESIDUAL, GEOMETRY_SPHERE_RESIDUAL)
            == Some(0)
        && collide_test_proc_index(GEOMETRY_SPHERE_RESIDUAL, GEOMETRY_BOX_RESIDUAL) == Some(2)
        && collide_test_proc_index(GEOMETRY_CYLINDER_RESIDUAL, GEOMETRY_SPHERE_RESIDUAL)
            == Some(3)
        && collide_test_proc_index(GEOMETRY_BOX_RESIDUAL, GEOMETRY_BOX_RESIDUAL) == Some(8)
        && collide_test_proc_index(3, 0).is_none()
        && COLLIDE_TEST_PROC_NAME_LIST
            [collide_test_proc_index(GEOMETRY_BOX_RESIDUAL, GEOMETRY_CYLINDER_RESIDUAL).unwrap()]
            == "collideTest_Box_Cylinder"
        // Bounding residual formulas (sample anchors).
        && {
            let sphere_bc = geometry_bounding_circle_radius(GEOMETRY_SPHERE_RESIDUAL, 5.0, 0.0);
            let sphere_bs = geometry_bounding_sphere_radius(GEOMETRY_SPHERE_RESIDUAL, 0.0, 5.0, 0.0);
            sphere_bc == Some(5.0) && sphere_bs == Some(5.0)
        }
        && {
            // Cylinder major=10 height=30 → circle 10, sphere max(15,10)=15.
            let bc = geometry_bounding_circle_radius(GEOMETRY_CYLINDER_RESIDUAL, 10.0, 0.0);
            let bs = geometry_bounding_sphere_radius(GEOMETRY_CYLINDER_RESIDUAL, 30.0, 10.0, 0.0);
            bc == Some(10.0) && bs == Some(15.0)
        }
        && {
            // Cylinder short: major=10 height=10 → sphere max(5,10)=10.
            let bs = geometry_bounding_sphere_radius(GEOMETRY_CYLINDER_RESIDUAL, 10.0, 10.0, 0.0);
            bs == Some(10.0)
        }
        && {
            // Box major=3 minor=4 height=0 → circle 5, sphere 5.
            let bc = geometry_bounding_circle_radius(GEOMETRY_BOX_RESIDUAL, 3.0, 4.0);
            let bs = geometry_bounding_sphere_radius(GEOMETRY_BOX_RESIDUAL, 0.0, 3.0, 4.0);
            match (bc, bs) {
                (Some(c), Some(s)) => (c - 5.0).abs() < 1e-5 && (s - 5.0).abs() < 1e-5,
                _ => false,
            }
        }
        // Height residual formulas.
        && geometry_max_height_above_position(GEOMETRY_SPHERE_RESIDUAL, 0.0, 7.0) == Some(7.0)
        && geometry_max_height_above_position(GEOMETRY_CYLINDER_RESIDUAL, 12.0, 7.0) == Some(12.0)
        && geometry_max_height_above_position(GEOMETRY_BOX_RESIDUAL, 9.0, 1.0) == Some(9.0)
        && geometry_max_height_below_position(GEOMETRY_SPHERE_RESIDUAL, 7.0) == Some(7.0)
        && geometry_max_height_below_position(GEOMETRY_BOX_RESIDUAL, 7.0) == Some(0.0)
        && geometry_z_delta_to_center(GEOMETRY_SPHERE_RESIDUAL, 10.0) == Some(0.0)
        && geometry_z_delta_to_center(GEOMETRY_BOX_RESIDUAL, 10.0) == Some(5.0)
        // Footprint area residual.
        && {
            let a = geometry_footprint_area(GEOMETRY_SPHERE_RESIDUAL, 2.0, 0.0, 2.0);
            match a {
                Some(v) => (v - (PI * 4.0)).abs() < 1e-5,
                None => false,
            }
        }
        && geometry_footprint_area(GEOMETRY_BOX_RESIDUAL, 3.0, 4.0, 5.0) == Some(48.0)
        // CollideModule residual: other=NULL indicates ground collision (documented residual).
        // Fail-closed: unknown geometry types reject.
        && geometry_bounding_circle_radius(99, 1.0, 1.0).is_none()
        && geometry_max_height_above_position(99, 1.0, 1.0).is_none()
}

// ---------------------------------------------------------------------------
// 3. Physics residual peels (PhysicsUpdate.cpp / PhysicsUpdate.h)
// ---------------------------------------------------------------------------

/// C++ `DEFAULT_MASS` residual.
pub const PHYSICS_DEFAULT_MASS_RESIDUAL: f32 = 1.0;
/// C++ `DEFAULT_SHOCK_YAW` residual.
pub const PHYSICS_DEFAULT_SHOCK_YAW_RESIDUAL: f32 = 0.05;
/// C++ `DEFAULT_SHOCK_PITCH` residual.
pub const PHYSICS_DEFAULT_SHOCK_PITCH_RESIDUAL: f32 = 0.025;
/// C++ `DEFAULT_SHOCK_ROLL` residual.
pub const PHYSICS_DEFAULT_SHOCK_ROLL_RESIDUAL: f32 = 0.025;
/// C++ `DEFAULT_FORWARD_FRICTION` residual.
pub const PHYSICS_DEFAULT_FORWARD_FRICTION_RESIDUAL: f32 = 0.15;
/// C++ `DEFAULT_LATERAL_FRICTION` residual.
pub const PHYSICS_DEFAULT_LATERAL_FRICTION_RESIDUAL: f32 = 0.15;
/// C++ `DEFAULT_Z_FRICTION` residual.
pub const PHYSICS_DEFAULT_Z_FRICTION_RESIDUAL: f32 = 0.8;
/// C++ `DEFAULT_AERO_FRICTION` residual.
pub const PHYSICS_DEFAULT_AERO_FRICTION_RESIDUAL: f32 = 0.0;
/// C++ `MIN_AERO_FRICTION` residual.
pub const PHYSICS_MIN_AERO_FRICTION_RESIDUAL: f32 = 0.00;
/// C++ `MIN_NON_AERO_FRICTION` residual.
pub const PHYSICS_MIN_NON_AERO_FRICTION_RESIDUAL: f32 = 0.01;
/// C++ `MAX_FRICTION` residual.
pub const PHYSICS_MAX_FRICTION_RESIDUAL: f32 = 0.99;
/// C++ `STUN_RELIEF_EPSILON` residual.
pub const PHYSICS_STUN_RELIEF_EPSILON_RESIDUAL: f32 = 0.5;
/// C++ `MOTIVE_FRAMES` residual (`LOGICFRAMES_PER_SECOND / 3` → 10).
pub const PHYSICS_MOTIVE_FRAMES_RESIDUAL: u32 = LOGIC_FRAMES_PER_SECOND_RESIDUAL / 3;
/// C++ `INVALID_VEL_MAG` residual.
pub const PHYSICS_INVALID_VEL_MAG_RESIDUAL: f32 = -1.0;
/// C++ ctor residual: `m_pitchRollYawFactor` default (double-apply historical factor).
pub const PHYSICS_PITCH_ROLL_YAW_FACTOR_RESIDUAL: f32 = 2.0;
/// C++ ctor residual: `m_fallHeightDamageFactor` default.
pub const PHYSICS_FALL_HEIGHT_DAMAGE_FACTOR_RESIDUAL: f32 = 1.0;
/// C++ ctor residual: MinFallHeightForDamage height input (→ heightToSpeed).
pub const PHYSICS_MIN_FALL_HEIGHT_FOR_DAMAGE_RESIDUAL: f32 = 40.0;
/// C++ ctor residual: `m_allowBouncing` default.
pub const PHYSICS_ALLOW_BOUNCING_DEFAULT_RESIDUAL: bool = false;
/// C++ ctor residual: `m_allowCollideForce` default.
pub const PHYSICS_ALLOW_COLLIDE_FORCE_DEFAULT_RESIDUAL: bool = true;
/// C++ ctor residual: `m_killWhenRestingOnGround` default.
pub const PHYSICS_KILL_WHEN_RESTING_ON_GROUND_DEFAULT_RESIDUAL: bool = false;
/// C++ ctor residual: `m_shockResistance` default.
pub const PHYSICS_SHOCK_RESISTANCE_DEFAULT_RESIDUAL: f32 = 0.0;
/// C++ ctor residual: `m_centerOfMassOffset` default.
pub const PHYSICS_CENTER_OF_MASS_OFFSET_DEFAULT_RESIDUAL: f32 = 0.0;

/// Retail GameData.ini Gravity residual (cross-link Wave 86).
pub const PHYSICS_GRAVITY_RESIDUAL: f32 = -64.0;

/// C++ vehicle crash weapon template residual names.
pub const PHYSICS_VEHICLE_CRASHES_INTO_BUILDING_WEAPON: &str = "VehicleCrashesIntoBuildingWeapon";
/// C++ vehicle crash non-building weapon residual name.
pub const PHYSICS_VEHICLE_CRASHES_INTO_NON_BUILDING_WEAPON: &str =
    "VehicleCrashesIntoNonBuildingWeapon";

/// C++ `PhysicsTurningType` residual: TURN_NEGATIVE.
pub const PHYSICS_TURN_NEGATIVE: i32 = -1;
/// C++ `TURN_NONE` residual.
pub const PHYSICS_TURN_NONE: i32 = 0;
/// C++ `TURN_POSITIVE` residual.
pub const PHYSICS_TURN_POSITIVE: i32 = 1;

/// C++ `PhysicsFlagsType` residual bits (xfer-stable numbers).
pub const PHYSICS_FLAG_STICK_TO_GROUND: u32 = 0x0001;
/// C++ `ALLOW_BOUNCE` residual bit.
pub const PHYSICS_FLAG_ALLOW_BOUNCE: u32 = 0x0002;
/// C++ `APPLY_FRICTION2D_WHEN_AIRBORNE` residual bit.
pub const PHYSICS_FLAG_APPLY_FRICTION2D_WHEN_AIRBORNE: u32 = 0x0004;
/// C++ `UPDATE_EVER_RUN` residual bit.
pub const PHYSICS_FLAG_UPDATE_EVER_RUN: u32 = 0x0008;
/// C++ `WAS_AIRBORNE_LAST_FRAME` residual bit.
pub const PHYSICS_FLAG_WAS_AIRBORNE_LAST_FRAME: u32 = 0x0010;
/// C++ `ALLOW_COLLIDE_FORCE` residual bit.
pub const PHYSICS_FLAG_ALLOW_COLLIDE_FORCE: u32 = 0x0020;
/// C++ `ALLOW_TO_FALL` residual bit.
pub const PHYSICS_FLAG_ALLOW_TO_FALL: u32 = 0x0040;
/// C++ `HAS_PITCHROLLYAW` residual bit.
pub const PHYSICS_FLAG_HAS_PITCHROLLYAW: u32 = 0x0080;
/// C++ `IMMUNE_TO_FALLING_DAMAGE` residual bit.
pub const PHYSICS_FLAG_IMMUNE_TO_FALLING_DAMAGE: u32 = 0x0100;
/// C++ `IS_IN_FREEFALL` residual bit.
pub const PHYSICS_FLAG_IS_IN_FREEFALL: u32 = 0x0200;
/// C++ `IS_IN_UPDATE` residual bit.
pub const PHYSICS_FLAG_IS_IN_UPDATE: u32 = 0x0400;
/// C++ `IS_STUNNED` residual bit.
pub const PHYSICS_FLAG_IS_STUNNED: u32 = 0x0800;

/// Ordered C++ PhysicsFlagsType residual bit-name list.
pub const PHYSICS_FLAG_NAME_LIST: &[&str] = &[
    "STICK_TO_GROUND",
    "ALLOW_BOUNCE",
    "APPLY_FRICTION2D_WHEN_AIRBORNE",
    "UPDATE_EVER_RUN",
    "WAS_AIRBORNE_LAST_FRAME",
    "ALLOW_COLLIDE_FORCE",
    "ALLOW_TO_FALL",
    "HAS_PITCHROLLYAW",
    "IMMUNE_TO_FALLING_DAMAGE",
    "IS_IN_FREEFALL",
    "IS_IN_UPDATE",
    "IS_STUNNED",
];

/// Residual: convert PhysicsFlagsType name index to bit value.
pub fn physics_flag_bit_value(name: &str) -> Option<u32> {
    PHYSICS_FLAG_NAME_LIST
        .iter()
        .position(|&n| n.eq_ignore_ascii_case(name))
        .map(|i| 1u32 << i)
}

/// Residual: C++ `heightToSpeed` = `sqrt(|2 * gravity * height|)` with gravity magnitude.
pub fn physics_height_to_speed(height: f32, gravity: f32) -> f32 {
    (2.0 * gravity.abs() * height).abs().sqrt()
}

/// Residual: C++ `parseFrictionPerSec` → friction per frame = fricPerSec * SECONDS_PER_LOGICFRAME.
pub fn physics_friction_per_frame(fric_per_sec: f32) -> f32 {
    fric_per_sec * SECONDS_PER_LOGIC_FRAME_RESIDUAL
}

/// Wave 96 honesty: Physics residual peels pack.
pub fn honesty_physics_residual_pack_wave96() -> bool {
    PHYSICS_DEFAULT_MASS_RESIDUAL == 1.0
        && (PHYSICS_DEFAULT_SHOCK_YAW_RESIDUAL - 0.05).abs() < 1e-6
        && (PHYSICS_DEFAULT_SHOCK_PITCH_RESIDUAL - 0.025).abs() < 1e-6
        && (PHYSICS_DEFAULT_SHOCK_ROLL_RESIDUAL - 0.025).abs() < 1e-6
        && (PHYSICS_DEFAULT_FORWARD_FRICTION_RESIDUAL - 0.15).abs() < 1e-6
        && (PHYSICS_DEFAULT_LATERAL_FRICTION_RESIDUAL - 0.15).abs() < 1e-6
        && (PHYSICS_DEFAULT_Z_FRICTION_RESIDUAL - 0.8).abs() < 1e-6
        && PHYSICS_DEFAULT_AERO_FRICTION_RESIDUAL == 0.0
        && PHYSICS_MIN_AERO_FRICTION_RESIDUAL == 0.0
        && (PHYSICS_MIN_NON_AERO_FRICTION_RESIDUAL - 0.01).abs() < 1e-6
        && (PHYSICS_MAX_FRICTION_RESIDUAL - 0.99).abs() < 1e-6
        && (PHYSICS_STUN_RELIEF_EPSILON_RESIDUAL - 0.5).abs() < 1e-6
        && PHYSICS_MOTIVE_FRAMES_RESIDUAL == 10
        && LOGIC_FRAMES_PER_SECOND_RESIDUAL / 3 == 10
        && PHYSICS_INVALID_VEL_MAG_RESIDUAL == -1.0
        && PHYSICS_PITCH_ROLL_YAW_FACTOR_RESIDUAL == 2.0
        && PHYSICS_FALL_HEIGHT_DAMAGE_FACTOR_RESIDUAL == 1.0
        && PHYSICS_MIN_FALL_HEIGHT_FOR_DAMAGE_RESIDUAL == 40.0
        && !PHYSICS_ALLOW_BOUNCING_DEFAULT_RESIDUAL
        && PHYSICS_ALLOW_COLLIDE_FORCE_DEFAULT_RESIDUAL
        && !PHYSICS_KILL_WHEN_RESTING_ON_GROUND_DEFAULT_RESIDUAL
        && PHYSICS_SHOCK_RESISTANCE_DEFAULT_RESIDUAL == 0.0
        && PHYSICS_CENTER_OF_MASS_OFFSET_DEFAULT_RESIDUAL == 0.0
        && PHYSICS_GRAVITY_RESIDUAL == -64.0
        // Turning residual.
        && PHYSICS_TURN_NEGATIVE == -1
        && PHYSICS_TURN_NONE == 0
        && PHYSICS_TURN_POSITIVE == 1
        // Flag bits residual.
        && PHYSICS_FLAG_NAME_LIST.len() == 12
        && PHYSICS_FLAG_STICK_TO_GROUND == 0x0001
        && PHYSICS_FLAG_ALLOW_BOUNCE == 0x0002
        && PHYSICS_FLAG_ALLOW_COLLIDE_FORCE == 0x0020
        && PHYSICS_FLAG_IS_STUNNED == 0x0800
        && physics_flag_bit_value("STICK_TO_GROUND") == Some(0x0001)
        && physics_flag_bit_value("IS_STUNNED") == Some(0x0800)
        && physics_flag_bit_value("NOT_A_FLAG").is_none()
        // heightToSpeed residual with retail gravity.
        && {
            let v = physics_height_to_speed(
                PHYSICS_MIN_FALL_HEIGHT_FOR_DAMAGE_RESIDUAL,
                PHYSICS_GRAVITY_RESIDUAL,
            );
            // sqrt(2 * 64 * 40) = sqrt(5120) ≈ 71.554175
            (v - 5120f32.sqrt()).abs() < 1e-3
        }
        // Friction-per-sec residual: 0.15/sec → 0.005/frame at 30 FPS.
        && {
            let f = physics_friction_per_frame(0.15);
            (f - 0.005).abs() < 1e-6
        }
        // Crash weapon residual names.
        && PHYSICS_VEHICLE_CRASHES_INTO_BUILDING_WEAPON
            == "VehicleCrashesIntoBuildingWeapon"
        && PHYSICS_VEHICLE_CRASHES_INTO_NON_BUILDING_WEAPON
            == "VehicleCrashesIntoNonBuildingWeapon"
}

// ---------------------------------------------------------------------------
// 4. Projectile residual deepen (DumbProjectileBehavior.h / .cpp)
// ---------------------------------------------------------------------------

/// C++ `DEFAULT_MAX_LIFESPAN` residual (`10 * LOGICFRAMES_PER_SECOND` → 300 frames).
pub const PROJECTILE_DEFAULT_MAX_LIFESPAN_FRAMES_RESIDUAL: u32 =
    10 * LOGIC_FRAMES_PER_SECOND_RESIDUAL;

/// C++ ctor residual: `m_orientToFlightPath` default TRUE.
pub const PROJECTILE_ORIENT_TO_FLIGHT_PATH_DEFAULT_RESIDUAL: bool = true;
/// C++ ctor residual: `m_tumbleRandomly` default FALSE.
pub const PROJECTILE_TUMBLE_RANDOMLY_DEFAULT_RESIDUAL: bool = false;
/// C++ ctor residual: `m_detonateCallsKill` default FALSE.
pub const PROJECTILE_DETONATE_CALLS_KILL_DEFAULT_RESIDUAL: bool = false;
/// C++ ctor residual: bezier FirstHeight default.
pub const PROJECTILE_FIRST_HEIGHT_DEFAULT_RESIDUAL: f32 = 0.0;
/// C++ ctor residual: bezier SecondHeight default.
pub const PROJECTILE_SECOND_HEIGHT_DEFAULT_RESIDUAL: f32 = 0.0;
/// C++ ctor residual: FirstPercentIndent default.
pub const PROJECTILE_FIRST_PERCENT_INDENT_DEFAULT_RESIDUAL: f32 = 0.0;
/// C++ ctor residual: SecondPercentIndent default.
pub const PROJECTILE_SECOND_PERCENT_INDENT_DEFAULT_RESIDUAL: f32 = 0.0;
/// C++ ctor residual: GarrisonHitKillCount default.
pub const PROJECTILE_GARRISON_HIT_KILL_COUNT_DEFAULT_RESIDUAL: u32 = 0;
/// C++ ctor residual: FlightPathAdjustDistPerFrame default.
pub const PROJECTILE_FLIGHT_PATH_ADJUST_DIST_PER_FRAME_DEFAULT_RESIDUAL: f32 = 0.0;

/// C++ ballistic residual: `SHALLOW_ANGLE` = 0.5° in radians.
pub const PROJECTILE_SHALLOW_ANGLE_RAD_RESIDUAL: f32 = 0.5 * PI / 180.0;
/// C++ ballistic residual: `MIN_ANGLE_DIFF` = 1/16 degree in radians.
pub const PROJECTILE_MIN_ANGLE_DIFF_RAD_RESIDUAL: f32 = PI / (180.0 * 16.0);
/// C++ ballistic residual: `CLOSE_ENOUGH_RANGE` world units.
pub const PROJECTILE_CLOSE_ENOUGH_RANGE_RESIDUAL: f32 = 5.0;

/// Ordered C++ DumbProjectileBehavior INI field residual names.
pub const PROJECTILE_INI_FIELD_NAME_LIST: &[&str] = &[
    "MaxLifespan",
    "TumbleRandomly",
    "DetonateCallsKill",
    "OrientToFlightPath",
    "FirstHeight",
    "SecondHeight",
    "FirstPercentIndent",
    "SecondPercentIndent",
    "GarrisonHitKillRequiredKindOf",
    "GarrisonHitKillForbiddenKindOf",
    "GarrisonHitKillCount",
    "GarrisonHitKillFX",
    "FlightPathAdjustDistPerSecond",
];

/// Whether a DumbProjectileBehavior INI field residual name is known.
pub fn projectile_ini_field_name_known(name: &str) -> bool {
    PROJECTILE_INI_FIELD_NAME_LIST
        .iter()
        .any(|n| n.eq_ignore_ascii_case(name))
}

/// Residual: lifespan frame = launchFrame + maxLifespan (ctor path).
pub fn projectile_lifespan_frame(launch_frame: u32, max_lifespan: u32) -> u32 {
    launch_frame.saturating_add(max_lifespan)
}

/// Wave 96 honesty: Projectile residual deepen pack.
pub fn honesty_projectile_residual_deepen_pack_wave96() -> bool {
    PROJECTILE_DEFAULT_MAX_LIFESPAN_FRAMES_RESIDUAL == 300
        && 10 * LOGIC_FRAMES_PER_SECOND_RESIDUAL == 300
        && PROJECTILE_ORIENT_TO_FLIGHT_PATH_DEFAULT_RESIDUAL
        && !PROJECTILE_TUMBLE_RANDOMLY_DEFAULT_RESIDUAL
        && !PROJECTILE_DETONATE_CALLS_KILL_DEFAULT_RESIDUAL
        && PROJECTILE_FIRST_HEIGHT_DEFAULT_RESIDUAL == 0.0
        && PROJECTILE_SECOND_HEIGHT_DEFAULT_RESIDUAL == 0.0
        && PROJECTILE_FIRST_PERCENT_INDENT_DEFAULT_RESIDUAL == 0.0
        && PROJECTILE_SECOND_PERCENT_INDENT_DEFAULT_RESIDUAL == 0.0
        && PROJECTILE_GARRISON_HIT_KILL_COUNT_DEFAULT_RESIDUAL == 0
        && PROJECTILE_FLIGHT_PATH_ADJUST_DIST_PER_FRAME_DEFAULT_RESIDUAL == 0.0
        // Angle residual constants.
        && (PROJECTILE_SHALLOW_ANGLE_RAD_RESIDUAL - (0.5 * PI / 180.0)).abs() < 1e-7
        && (PROJECTILE_MIN_ANGLE_DIFF_RAD_RESIDUAL - (PI / (180.0 * 16.0))).abs() < 1e-7
        && PROJECTILE_CLOSE_ENOUGH_RANGE_RESIDUAL == 5.0
        // INI field residual table.
        && PROJECTILE_INI_FIELD_NAME_LIST.len() == 13
        && PROJECTILE_INI_FIELD_NAME_LIST[0] == "MaxLifespan"
        && PROJECTILE_INI_FIELD_NAME_LIST[4] == "FirstHeight"
        && PROJECTILE_INI_FIELD_NAME_LIST[12] == "FlightPathAdjustDistPerSecond"
        && projectile_ini_field_name_known("OrientToFlightPath")
        && projectile_ini_field_name_known("GarrisonHitKillCount")
        && !projectile_ini_field_name_known("HomingStrength")
        // Lifespan residual: launch at frame 100 + default 300 → 400.
        && projectile_lifespan_frame(100, PROJECTILE_DEFAULT_MAX_LIFESPAN_FRAMES_RESIDUAL) == 400
        && projectile_lifespan_frame(0, 0) == 0
        // Unique field names.
        && {
            let mut seen = std::collections::HashSet::new();
            PROJECTILE_INI_FIELD_NAME_LIST.iter().all(|n| seen.insert(*n))
        }
}

// ---------------------------------------------------------------------------
// Combined Wave 96 residual pack
// ---------------------------------------------------------------------------

/// Wave 96 honesty: partition + collision + physics + projectile residual peels.
pub fn honesty_partition_collision_physics_residual_pack_wave96() -> bool {
    honesty_partition_residual_pack_wave96()
        && honesty_collision_residual_pack_wave96()
        && honesty_physics_residual_pack_wave96()
        && honesty_projectile_residual_deepen_pack_wave96()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn honesty_partition_residual_pack_wave96_ok() {
        assert!(honesty_partition_residual_pack_wave96());
    }

    #[test]
    fn honesty_collision_residual_pack_wave96_ok() {
        assert!(honesty_collision_residual_pack_wave96());
    }

    #[test]
    fn honesty_physics_residual_pack_wave96_ok() {
        assert!(honesty_physics_residual_pack_wave96());
    }

    #[test]
    fn honesty_projectile_residual_deepen_pack_wave96_ok() {
        assert!(honesty_projectile_residual_deepen_pack_wave96());
    }

    #[test]
    fn honesty_partition_collision_physics_residual_pack_wave96_ok() {
        assert!(honesty_partition_collision_physics_residual_pack_wave96());
    }

    #[test]
    fn partition_filter_table_anchors() {
        assert_eq!(PARTITION_FILTER_RESIDUAL_NAME_COUNT, 33);
        assert!(partition_filter_residual_name_known(
            "PartitionFilterSameMapStatus"
        ));
        assert!(!partition_filter_residual_name_known("PartitionFilter"));
    }

    #[test]
    fn collide_matrix_depends_on_geometry_order() {
        // C++ comment: collidesWith depends on GeometryType order starting at 0.
        assert_eq!(
            collide_test_proc_index(GEOMETRY_SPHERE_RESIDUAL, GEOMETRY_CYLINDER_RESIDUAL),
            Some(1)
        );
        assert_eq!(
            COLLIDE_TEST_PROC_NAME_LIST[1],
            "collideTest_Sphere_Cylinder"
        );
    }

    #[test]
    fn physics_motive_and_fall_speed() {
        assert_eq!(PHYSICS_MOTIVE_FRAMES_RESIDUAL, 10);
        let v = physics_height_to_speed(40.0, -64.0);
        assert!((v * v - 5120.0).abs() < 1e-2);
    }
}
