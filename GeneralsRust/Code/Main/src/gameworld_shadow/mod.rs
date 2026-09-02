//! Shadow parity bridge: Main `GameLogic` (live C++ `TheGameLogic` counterpart)
//! → observe-only `gamelogic::world::GameWorld` mirror.
//!
//! This is **not** production authority. It maintains a borrow-first `GameWorld`
//! plus a **stable** host `ObjectId` → `EntityId` map so damage/spawn/destroy can
//! be mirrored as `WorldMutation`s without pointer ownership.
//!
//! Shadow session production default ON (`GENERALS_GAMEWORLD_SHADOW=0` to opt out).
//! Last-writer `*_AUTHORITY` channels are per-`GameLogic` context fields
//! (`GameWorldAuthority`, default **off**) so host `GameLogic` is the sole
//! writer (C++ single-store). Opt in per channel via `GameLogic::set_*_authority`
//! setters — the `GENERALS_GAMEWORLD_*_AUTHORITY` env flags are retired (hq-e84zk).
//!
//! Dual-tick policy is **AuthorityOnly** (see `authoritative_world::dual_tick_policy`).
//! Do not populate OBJECT_REGISTRY with host objects.
//!
//! Policy: borrow host for sync phases only; never store long-lived host references.

//!
//! Wave 956: host_object/host_objects authority dual-read seal.
//! Wave 958: host_object dual-read seal (tests + residual).

use crate::game_logic::{GameLogic, ObjectId, Team};

use gamelogic::world::entities::{EntityId, EntityProductionItem, TemplateRef, Transform};
use gamelogic::world::{GameWorld, PlayerId, WorldMutation, WorldSnapshot};
use std::collections::{HashMap, HashSet};

mod tick;
mod types;
pub use tick::*;
use types::HordePlayerRel;
pub use types::*;

mod apply_host_combat;
mod apply_host_damage;
mod apply_host_events;
mod apply_host_misc;
mod apply_host_stealth;
mod apply_host_weapon_set;
mod command_authority;
mod construct;
mod counts;
mod couple_guard;
mod factory_authority;
mod presentation;
mod session;
mod writeback_combat_status;
mod writeback_core;
mod writeback_misc;
mod writeback_production;
pub use couple_guard::*;
pub use presentation::*;
pub use session::*;

/// Session holding GameWorld + stable host↔entity ID maps.
#[derive(Debug)]
pub struct GameWorldShadow {
    world: GameWorld,
    host_to_entity: HashMap<u32, EntityId>,
    entity_to_host: HashMap<u32, u32>,
    max_entities: usize,
    /// Host player id → dense GameWorld PlayerId
    host_player_to_gw: HashMap<u32, PlayerId>,
    /// Host player relationship snapshot for HordeUpdate AlliesOnly
    /// (C++ `PartitionFilterHordeMember` `getRelationship == ALLIES`).
    horde_player_rel: HashMap<u32, HordePlayerRel>,

    /// Last host energy shortfall residual per producer host id (sole-tick).
    production_power_factor_by_host: HashMap<u32, f32>,
    /// Last host construction effective_rate residual per host object id.
    construction_rate_by_host: HashMap<u32, f32>,
    /// Host isDisabled/pauseCountdown freeze residual for SP sole-tick.
    special_power_frozen_by_host: HashMap<u32, bool>,
    /// Host world XZ extent for DeliverPayload HeadOffMap / isOffMap.
    map_min_x: f32,
    map_min_z: f32,
    map_max_x: f32,
    map_max_z: f32,

    /// Pending A10 missile drops (mirrored from host registry under dual-tick).
    a10_pending_drops: Vec<crate::game_logic::host_a10_strike_flight::PendingA10MissileDrop>,
    artillery_pending_drops:
        Vec<crate::game_logic::host_artillery_barrage_flight::PendingArtilleryShellDrop>,
    carpet_pending_drops: Vec<crate::game_logic::host_carpet_bomb_flight::PendingCarpetBombDrop>,
}

#[cfg(test)]
mod tests;

/// Concatenated live gameworld_shadow sources for residual `include_str` scans.
pub const GAMEWORLD_SHADOW_SRC: &str = concat!(
    include_str!("mod.rs"),
    include_str!("apply_host_combat.rs"),
    include_str!("apply_host_damage.rs"),
    include_str!("apply_host_events.rs"),
    include_str!("apply_host_misc.rs"),
    include_str!("apply_host_stealth.rs"),
    include_str!("apply_host_weapon_set.rs"),
    include_str!("command_authority.rs"),
    include_str!("construct.rs"),
    include_str!("counts.rs"),
    include_str!("couple_guard.rs"),
    include_str!("factory_authority.rs"),
    include_str!("presentation.rs"),
    include_str!("session.rs"),
    include_str!("types.rs"),
    include_str!("writeback_combat_status.rs"),
    include_str!("writeback_core.rs"),
    include_str!("writeback_misc.rs"),
    include_str!("writeback_production.rs"),
    include_str!("tick/authority.rs"),
    include_str!("tick/couple.rs"),
    include_str!("tick/dispatch.rs"),
    include_str!("tick/eager_ai.rs"),
    include_str!("tick/eager_combat.rs"),
    include_str!("tick/eager_contain.rs"),
    include_str!("tick/eager_economy.rs"),
    include_str!("tick/eager_identity.rs"),
    include_str!("tick/eager_misc.rs"),
    include_str!("tick/eager_orders.rs"),
    include_str!("tick/eager_stealth.rs"),
    include_str!("tick/eager_weapon.rs"),
    include_str!("tick/env.rs"),
    include_str!("tick/mod.rs"),
    include_str!("tick/status_timers.rs"),
    include_str!("tick/status_timers_death.rs"),
    include_str!("tick/status_timers_economy.rs"),
    include_str!("tick/status_timers_payload.rs"),
    include_str!("tick/status_timers_post.rs"),
    include_str!("tick/status_timers_projectiles.rs"),
    include_str!("tick/status_timers_specials.rs"),
    include_str!("tick/status_timers_stealth.rs"),
    include_str!("tick/status_timers_structure.rs"),
    include_str!("tick/status_timers_updates.rs"),
);
