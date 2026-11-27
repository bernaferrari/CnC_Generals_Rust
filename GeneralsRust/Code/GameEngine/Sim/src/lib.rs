//! Simulation components shared by the native game and its tools.

pub mod armor;
pub mod chinook;
pub mod damage;
pub mod debris;
pub mod dozer;
pub mod error;
pub mod kind_of;
pub mod object;
pub mod pathfinding;
pub mod production;
pub mod recurring;
pub mod science;
pub mod scripting;
pub mod special_power;
pub mod step;
pub mod terrain;
pub mod time;
pub mod upgrade;
pub mod weapon;

pub use error::{GameLogicError, GameLogicResult};
