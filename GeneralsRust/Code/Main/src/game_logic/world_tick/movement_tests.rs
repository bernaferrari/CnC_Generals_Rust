//! Host movement regression fixtures.
use super::*;
use crate::game_logic::ObjectAudio;
use crate::game_logic::pathfinding::PathfindingGround;
use crate::game_logic::{
    GameLogic, GridPos, KindOf, LocomotorAppearance, Object, ObjectId, PathfindingGrid, Team,
    ThingTemplate,
};
use glam::Vec3;

use super::fixtures::*;

#[path = "movement_tests/goal_cell_arrival_and_repath.rs"]
mod goal_cell_arrival_and_repath;
#[path = "movement_tests/ground_march_and_physics.rs"]
mod ground_march_and_physics;
#[path = "movement_tests/invalid_position_airborne_and_stuck.rs"]
mod invalid_position_airborne_and_stuck;
#[path = "movement_tests/repath_braking_and_locomotors.rs"]
mod repath_braking_and_locomotors;

#[cfg(feature = "game_client")]
#[path = "movement_tests/tree_owner.rs"]
mod tree_owner;

#[path = "movement_tests/angle_position_owner.rs"]
mod angle_position_owner;
