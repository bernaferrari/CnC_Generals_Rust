/// Wave 423: host-only path has no dual-world factory objects.
#[inline]
fn dual_world_registry_unavailable() -> bool {
    crate::object::registry::OBJECT_REGISTRY.is_empty()
}

// ============================================================================
// ENUMS AND CONSTANTS
// ============================================================================

/// Logic frames per second — canonical constant from Common (C++ TheGlobalData::m_framesPerSecond)
use game_engine::common::game_common::LOGICFRAMES_PER_SECOND;

/// Donut timer delay in seconds. Matches C++ Locomotor.cpp:31 DONUT_TIME_DELAY_SECONDS
const DONUT_TIME_DELAY_SECONDS: Real = 2.5;

/// Donut distance threshold. Matches C++ Locomotor.cpp:32 DONUT_DISTANCE
const DONUT_DISTANCE: Real = 4.0 * PATHFIND_CELL_SIZE_F;

/// Maximum braking factor clamp. Matches C++ Locomotor.cpp:35 MAX_BRAKING_FACTOR
const MAX_BRAKING_FACTOR: Real = 5.0;

/// C++ `BIGNUM` used for omitted Braking / MinTurnSpeed.
const LOCO_BIGNUM: Real = 99999.0;

/// C++ LocomotorTemplate::m_speedLimitZ default.
const LOCO_DEFAULT_SPEED_LIMIT_Z: Real = 999999.0;

/// C++ `IS_CONDITION_BETTER` on locomotor BodyDamageType (Pristine < Damaged < ReallyDamaged < Rubble).
fn loco_is_condition_better(a: BodyDamageType, b: BodyDamageType) -> bool {
    (a as u8) < (b as u8)
}

/// Damaged maxima of -1 mean "same as undamaged" (C++ LocomotorTemplate::validate).
fn heal_damaged_stat(damaged: Real, undamaged: Real) -> Real {
    if damaged < 0.0 {
        undamaged
    } else {
        damaged
    }
}

/// C++ `TheGlobalData->m_movementPenaltyDamageState` (default BODY_REALLYDAMAGED).
fn movement_penalty_damage_state() -> BodyDamageType {
    use game_engine::common::ini::get_global_data;
    use game_engine::common::ini::ini_game_data::BodyDamageType as IniBody;
    get_global_data()
        .map(|data| match data.read().movement_penalty_damage_state {
            IniBody::Pristine => BodyDamageType::Pristine,
            IniBody::Damaged => BodyDamageType::Damaged,
            IniBody::ReallyDamaged => BodyDamageType::ReallyDamaged,
            IniBody::Rubble => BodyDamageType::Rubble,
        })
        .unwrap_or(BodyDamageType::ReallyDamaged)
}

/// C++ `TheGlobalData->m_gravity`.
fn loco_gravity() -> Real {
    game_engine::common::ini::get_global_data()
        .map(|data| data.read().gravity)
        .unwrap_or(-1.0)
}

/// Locomotor surface type mask - bitmask for allowed terrain types
pub type LocomotorSurfaceTypeMask = u32;

// Surface type constants matching C++ implementation
pub const SURFACE_GROUND: u32 = 0x01;
pub const SURFACE_WATER: u32 = 0x02;
pub const SURFACE_CLIFF: u32 = 0x04;
pub const SURFACE_AIR: u32 = 0x08;
pub const SURFACE_RUBBLE: u32 = 0x10;

/// Locomotor appearance/type - matches C++ LocomotorAppearance (Locomotor.h)
///
/// C++ enum has exactly 9 values: LOCO_LEGS_TWO, LOCO_WHEELS_FOUR, LOCO_TREADS,
/// LOCO_HOVER, LOCO_THRUST, LOCO_WINGS, LOCO_CLIMBER, LOCO_OTHER, LOCO_MOTORCYCLE.
/// Naval/tunnel behavior is determined by surface masks and physics type, not appearance.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum LocomotorAppearance {
    /// Two-legged infantry (C++ LOCO_LEGS_TWO / "TWO_LEGS")
    TwoLegs,
    /// Four-wheeled vehicles (C++ LOCO_WHEELS_FOUR / "FOUR_WHEELS")
    FourWheels,
    /// Tracked vehicles (C++ LOCO_TREADS / "TREADS")
    Treads,
    /// Hovering units (C++ LOCO_HOVER / "HOVER")
    Hover,
    /// Thrust-based / helicopters (C++ LOCO_THRUST / "THRUST")
    Thrust,
    /// Fixed-wing aircraft (C++ LOCO_WINGS / "WINGS")
    Wings,
    /// Cliff climbers (C++ LOCO_CLIMBER / "CLIMBER")
    Climber,
    /// Motorcycle (C++ LOCO_MOTORCYCLE / "MOTORCYCLE")
    Motorcycle,
    /// Other / default (C++ LOCO_OTHER / "OTHER")
    Other,
}

/// Locomotor priority for group movement
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LocomotorPriority {
    /// Moves to back of group
    Back = 0,
    /// Stays in middle of group
    Middle = 1,
    /// Moves to front of group
    Front = 2,
}

/// Z-axis behavior - matches C++ LocomotorBehaviorZ
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LocomotorBehaviorZ {
    /// No Z-axis motive force
    NoZMotiveForce,
    /// Maintain sea level
    SeaLevel,
    /// Follow surface-relative height
    SurfaceRelativeHeight,
    /// Follow absolute height
    AbsoluteHeight,
    /// Fixed surface-relative height
    FixedSurfaceRelativeHeight,
    /// Fixed absolute height
    FixedAbsoluteHeight,
    /// Relative to ground and buildings
    RelativeToGroundAndBuildings,
    /// Smooth relative to highest layer
    SmoothRelativeToHighestLayer,
}

/// Body damage type affecting locomotor performance
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum BodyDamageType {
    Pristine = 0,
    Damaged = 1,
    ReallyDamaged = 2,
    Rubble = 3,
}

