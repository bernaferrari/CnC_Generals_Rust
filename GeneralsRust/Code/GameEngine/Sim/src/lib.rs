//! Simulation components shared by the native game and its tools.

pub mod armor;
pub mod bridge_behavior;
pub mod build_list;
pub mod chinook;
pub mod combat;
pub mod command;
pub mod damage;
pub mod debris;
pub mod dozer;
pub mod error;
pub mod frame;
pub mod kind_of;
pub mod object;
pub mod pathfinding;
pub mod production;
pub mod recurring;
pub mod science;
pub mod sides;
pub mod scripting;
pub mod special_power;
mod special_power_targeting;
mod special_power_type;
pub mod step;
pub mod terrain;
mod terrain_boundaries;
mod terrain_bridge;
mod terrain_height_map;
mod terrain_los;
mod terrain_ray;
mod terrain_water;
mod terrain_waypoints;
pub mod time;
pub mod upgrade;
mod water_transitions;
pub mod weapon;

pub use error::{GameLogicError, GameLogicResult};
