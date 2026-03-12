////////////////////////////////////////////////////////////////////////////////
//                                                                            //
//  (c) 2001-2003 Electronic Arts Inc.                                       //
//                                                                            //
////////////////////////////////////////////////////////////////////////////////

//! Formation System - Coordinated Unit Movement
//!
//! This module provides a complete formation system for managing coordinated
//! unit movement, formation shapes, leader-follower relationships, and
//! tactical group behavior. Matches C++ TensileFormationUpdate behavior.
//!
//! Features:
//! - Multiple formation types (Line, Column, Wedge, Box, Scatter, Custom)
//! - Leader-follower tracking
//! - Speed matching
//! - Formation keeping and reformation
//! - Combat integration
//! - Pathfinding coordination

pub mod combat_integration;
pub mod formation_calculator;
pub mod formation_manager;
pub mod formation_types;
pub mod leader_follower;
pub mod movement_coordinator;

pub use formation_types::{
    FormationSettings, FormationShape, FormationSlot, FormationState, FormationType, ScatterPattern,
};

pub use formation_manager::{FormationCommand, FormationGroup, FormationManager, FormationMember};

pub use formation_calculator::{FormationCalculator, FormationLayout, PositionCalculator};

pub use leader_follower::{
    FollowerRole, LeaderFollowerSystem, LeaderSelection, LeadershipTransfer,
};

pub use movement_coordinator::{
    FormationPathfinder, MovementCoordinator, MovementOrder, SpeedMatcher,
};

pub use combat_integration::{CombatBehavior, CombatState, FormationCombat, FormationTactics};

use crate::common::{Coord3D, ObjectID, Real};

/// Formation system error types
#[derive(Debug, Clone)]
pub enum FormationError {
    /// Formation not found
    NotFound,
    /// No units in formation
    NoUnits,
    /// No valid leader available
    NoLeader,
    /// Formation is full
    FormationFull,
    /// Unit is not in formation
    UnitNotInFormation,
    /// Invalid formation type
    InvalidFormationType,
    /// Formation is locked
    Locked,
    /// Pathfinding failed
    PathfindingFailed,
}

/// Formation result type
pub type FormationResult<T> = Result<T, FormationError>;

/// Maximum units per formation
pub const MAX_FORMATION_SIZE: usize = 100;

/// Minimum units to maintain formation
pub const MIN_FORMATION_SIZE: usize = 2;

/// Default formation spacing
pub const DEFAULT_SPACING: Real = 50.0;

/// Maximum distance before formation breaks
pub const MAX_FORMATION_DISTANCE: Real = 500.0;

/// Minimum distance for reformation
pub const MIN_REFORMATION_DISTANCE: Real = 100.0;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_constants() {
        assert!(MAX_FORMATION_SIZE > MIN_FORMATION_SIZE);
        assert!(DEFAULT_SPACING > 0.0);
        assert!(MAX_FORMATION_DISTANCE > MIN_REFORMATION_DISTANCE);
    }
}
