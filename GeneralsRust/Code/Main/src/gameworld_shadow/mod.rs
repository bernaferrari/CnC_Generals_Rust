//! Shadow parity bridge: Main `GameLogic` (temp host authority) → `gamelogic::world::GameWorld`.
//!
//! This is **not** production authority yet. It maintains a borrow-first `GameWorld`
//! plus a **stable** host `ObjectId` → `EntityId` map so damage/spawn/destroy can be
//! applied as `WorldMutation`s without pointer ownership.
//!
//! Production default ON; opt out with `GENERALS_GAMEWORLD_SHADOW=0`.
//!
//! Policy: borrow host for sync phases only; never store long-lived host references.

//!
//! Wave 956: host_object/host_objects authority dual-read seal.
//! Wave 958: host_object dual-read seal (tests + residual).

use crate::game_logic::{GameLogic, ObjectId, Team};

use gamelogic::world::entities::{EntityId, EntityProductionItem, TemplateRef, Transform};
use gamelogic::world::{GameWorld, PlayerId, WorldMutation, WorldSnapshot};
use std::collections::{HashMap, HashSet};

mod types;
#[path = "tick/mod.rs"]
mod tick;
pub use types::*;
pub use tick::*;

mod construct;
mod writeback_core;
mod writeback_production;
mod counts;
mod apply_host_events;
mod apply_host_combat;
mod apply_host_weapon_set;
mod apply_host_stealth;
mod apply_host_misc;
mod writeback_misc;
mod writeback_combat_status;
mod apply_host_damage;
mod session;
mod presentation;
mod couple_guard;
pub use session::*;
pub use presentation::*;
pub use couple_guard::*;

/// Session holding GameWorld + stable host↔entity ID maps.
#[derive(Debug)]
pub struct GameWorldShadow {
    world: GameWorld,
    host_to_entity: HashMap<u32, EntityId>,
    entity_to_host: HashMap<u32, u32>,
    max_entities: usize,
    /// Host player id → dense GameWorld PlayerId
    host_player_to_gw: HashMap<u32, PlayerId>,
    /// Last host energy shortfall residual per producer host id (sole-tick).
    production_power_factor_by_host: HashMap<u32, f32>,
    /// Last host construction effective_rate residual per host object id.
    construction_rate_by_host: HashMap<u32, f32>,
    /// Host isDisabled/pauseCountdown freeze residual for SP sole-tick.
    special_power_frozen_by_host: HashMap<u32, bool>,
    /// Pending A10 missile drops (mirrored from host registry under dual-tick).
    a10_pending_drops: Vec<crate::game_logic::host_a10_strike_flight::PendingA10MissileDrop>,
    artillery_pending_drops:
        Vec<crate::game_logic::host_artillery_barrage_flight::PendingArtilleryShellDrop>,
    carpet_pending_drops: Vec<crate::game_logic::host_carpet_bomb_flight::PendingCarpetBombDrop>,
}


#[cfg(test)]
mod tests;
