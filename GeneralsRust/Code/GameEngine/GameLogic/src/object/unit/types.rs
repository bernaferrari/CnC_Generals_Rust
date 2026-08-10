//! Unit movement, formation, combat, and order enums.

#![allow(unused_imports)]

use super::imports::*;

pub(super) const WAYPOINT_PATH_LIMIT: usize = 1024;
pub(super) const AI_UPDATE_MAX_WAYPOINTS: usize = 16;

/// Movement states for units
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MovementState {
    Idle,
    Moving,
    TurningToFace,
    Attacking,
    Retreating,
    Following,
    Patrolling,
    Guarding,
    Pursuing,
    Fleeing,
    Backing,
}

/// Formation positions for group movement
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FormationType {
    None,
    Line,
    Column,
    Wedge,
    Box,
    Scattered,
}

/// Combat modes for units
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CombatMode {
    Aggressive,   // Attack anything in range
    Defensive,    // Only attack when attacked
    HoldPosition, // Don't move to attack
    HoldFire,     // Don't attack at all
    GuardArea,    // Stay in designated area
}

/// Orders that can be given to units
#[derive(Debug, Clone)]
pub enum UnitOrder {
    Stop,
    Move {
        destination: Coord3D,
        use_formation: bool,
        waypoints: Vec<Waypoint>,
    },
    Attack {
        target: ObjectID,
        pursue: bool,
    },
    AttackMove {
        destination: Coord3D,
        engage_enemies: bool,
    },
    Guard {
        position: Coord3D,
        area_radius: Real,
    },
    Follow {
        target: ObjectID,
        distance: Real,
    },
    Patrol {
        waypoints: Vec<Coord3D>,
        loop_patrol: bool,
    },
    Garrison {
        building: ObjectID,
    },
    Ungarrison {
        exit_position: Option<Coord3D>,
    },
    Capture {
        building: ObjectID,
    },
    Sabotage {
        target: ObjectID,
    },
    Hack {
        target: ObjectID,
    },
    PickupSupplies {
        supply_source: ObjectID,
    },
    Retreat {
        safe_position: Coord3D,
        organized: bool,
    },
}
