//! World snapshot / Xfer residual for host save/load.
//!
//! # Wave 79 Drawable residual fields
//!
//! `ObjectStatusSnapshot.camo_stealth_look` freezes C++ `Drawable::m_stealthLook`
//! ordinal residual (`Object::camo_stealth_look`) so mid-flight CamoNetting /
//! Camouflage looks survive save/load.
//!
//! # Secondary weapon residual (2026-07-12)
//!
//! Host `Object` gained `secondary_weapon` + `active_weapon_slot` for combat /
//! FlashBang / TOW residual paths. Snapshot capture previously only stored
//! primary in `weapons[0]` and restore never rebound secondary — load desynced
//! dual-slot combat and lost upgrade-equipped secondaries.
//!
//! Closed residual layout (not full C++ WeaponSet Xfer table):
//! - `weapons[0]` = primary, `weapons[1]` = secondary when present
//! - secondary-only uses a zero-damage primary pad so secondary stays at index 1
//! - `ObjectStatusSnapshot.active_weapon_slot` survives player weapon-toggle
//!
//! # Special-power strike residual (2026-07-12)
//!
//! Host `HostSpecialPowerStrikeRegistry` queues DaisyCutter / A10 / ScudStorm /
//! ParticleCannon / NuclearMissile / AnthraxBomb / SpectreGunship / CarpetBomb /
//! ArtilleryBarrage / CruiseMissile impacts with a multi-frame delay (nuke also
//! spawns residual radiation; anthrax also spawns residual toxin; carpet bomb
//! multi-point line damage; artillery multi-shell scatter damage; cruise missile
//! loft then MOAB area damage).
//! Without snapshot persistence, save mid-flight dropped the pending strike
//! and impact never fired after load.
//!
//! Closed residual layout:
//! - `WorldSnapshot.special_power_strikes` stores `next_id` + all strike records
//!   (queued / completed / cancelled), including absolute `impact_frame`
//! - restore rebinds registry so remaining delay continues and area damage still applies
//! - `WorldSnapshot.combat_particles` optionally stores active host particle systems
//!   (template name + pose + spawn frame; not full W3D GPU particle state)
//!
//! # Host upgrade research residual (2026-07-12)
//!
//! Host `HostUpgradeRegistry` records QueueUpgrade → research complete honesty for
//! CaptureBuilding / FlashBang / TOW / SupplyLines. Player `queued_upgrades` already
//! survived via `PlayerSnapshot.research_queue`, but the host registry (pending ids,
//! source object, honesty flags) was live-only — mid-flight save dropped residual
//! queue honesty and could desync complete bookkeeping after load.
//!
//! Closed residual layout:
//! - `WorldSnapshot.host_upgrades` stores `next_id` + all research records
//!   (queued / completed / cancelled) including `queue_frame` / `complete_frame`
//! - restore rebinds registry + `pending_index` so mid-research entries complete
//!   on the next `update_player_upgrades` with unlocks still applied
//!
//! Still residual (fail-closed, not claimed):
//! - Full retail OCL / aircraft / beam / multiplayer superweapon Xfer tables
//! - Full retail Upgrade.ini BuildTime / ProductionUpdate research timers
//! - Full C++ per-module WeaponSet / SpecialPowerModule / Upgrade Xfer tables

// Wave 956: host_object/host_objects authority dual-read seal.
//! Wave 957: host_object/host_objects authority dual-read seal.

mod ai;
mod battle_plan_persist;
mod builder;

mod ability_hijack_persist;
mod booby_trap_persist;
mod carpet_bomb_persist;
mod client_drawable;
mod client_drawable_xfer;
mod dozer_repair_persist;
mod game_client_save;
mod game_state;
mod hotkey_squad_persist;
mod legacy_bincode;
mod lifecycle_tail;
mod load_post_process;
mod object;
pub(crate) mod persist_v18;
mod player;
mod player_upgrade_persist;
mod production_door_persist;
mod rebuild_hole_persist;
mod restore;
mod shroud;
mod special_power_cooldown_persist;
mod special_powers;
mod subdual_persist;
mod weapon_set_persist;

pub(crate) mod ai_player_queue_persist;
mod ai_team_persist;
mod angry_mob_persist;
mod auto_deposit_persist;
mod bridge_behavior_persist;
mod chinook_ai_persist;
mod cleanup_hazard_persist;
mod deliver_payload_persist;
mod dock_queue_persist;
mod dynamic_shroud_persist;
mod firewall_persist;
mod garrison_firepoint_persist;
mod gps_scrambler_persist;
mod hacker_income_persist;
mod helix_napalm_persist;
mod inferno_fire_persist;
mod jet_ai_persist;
mod module_runtime_persist;
mod money_crate_persist;
mod neutron_slow_death_persist;
mod object_module_xfer_persist;
mod object_xfer_persist;
mod point_defense_persist;
mod power_plant_rods_persist;
mod projectile_stream_persist;
mod score_keeper_persist;
mod stealth_detector_persist;
mod stealth_grant_persist;
mod supply_drop_persist;
mod transport_exit_persist;
mod turret_aim_persist;
mod warehouse_crippling_persist;
mod weapon_leech_persist;

mod particle_system_save;
mod player_team_persist;
mod terrain_visual_save;

mod terrain;
mod types;
mod w3d_ghost_save;
mod xfer_helpers;

#[cfg(test)]
mod lifecycle_save_file;
#[cfg(test)]
mod tests;
pub use ai::*;
pub use builder::*;
pub use client_drawable::*;
pub use game_client_save::{
    CHUNK_GAME_CLIENT, capture_game_client_xfer_bytes, restore_game_client_from_xfer_bytes,
    restore_objectless_from_client_drawables, stash_loaded_game_client_xfer,
    take_loaded_game_client_xfer,
};
pub use game_state::*;
pub use gamelogic::system::shroud_manager::{
    ShroudCellSnapshot, ShroudGridSnapshot, ShroudPendingUndoRevealSnapshot, ShroudSnapshot,
};
pub use hotkey_squad_persist::{
    peek_pending_control_groups, set_pending_control_groups, take_pending_control_groups,
};
pub(crate) use legacy_bincode::*;
pub use lifecycle_tail::{
    ContainLink, LifecycleTail, ProducerLink, apply_lifecycle_tail_to_host, capture_lifecycle_tail,
    contain_fixups_from_tail, decode_lifecycle_tail, encode_lifecycle_tail,
    producer_fixups_from_tail,
};
pub use object::*;
pub use particle_system_save::{
    CHUNK_PARTICLE_SYSTEM, capture_particle_system_xfer_bytes,
    restore_particle_system_from_xfer_bytes, stash_loaded_particle_system_xfer,
    take_loaded_particle_system_xfer,
};
pub use persist_v18::{
    CameraPersist, WorldPersistV18, peek_pending_camera, set_pending_camera, take_pending_camera,
};
pub use player::*;
pub use player_team_persist::{
    CHUNK_PLAYERS, CHUNK_TEAM_FACTORY, apply_pending as apply_pending_player_team_chunks,
    stamp_from_live as stamp_player_team_chunks,
    stash_loaded_chunks as stash_loaded_player_team_chunks, write_players_block,
    write_team_factory_block,
};
pub use special_powers::*;
pub use terrain::*;
pub use terrain_visual_save::{
    CHUNK_TERRAIN_VISUAL, capture_terrain_visual_xfer_bytes,
    restore_terrain_visual_from_xfer_bytes, stash_loaded_terrain_visual_xfer,
    take_loaded_terrain_visual_xfer,
};
pub use types::*;
pub use w3d_ghost_save::{
    CHUNK_GHOST_OBJECT, capture_w3d_ghost_xfer_bytes, restore_w3d_ghost_manager_from_xfer_bytes,
    save_lock_live_w3d_ghosts, stash_loaded_w3d_ghost_xfer, take_loaded_w3d_ghost_xfer,
};

/// Concatenated live snapshot sources for residual `include_str` scans.
pub const SNAPSHOT_SRC: &str = concat!(
    include_str!("mod.rs"),
    include_str!("ai.rs"),
    include_str!("battle_plan_persist.rs"),
    include_str!("builder.rs"),
    include_str!("client_drawable.rs"),
    include_str!("client_drawable_xfer.rs"),
    include_str!("game_state.rs"),
    include_str!("game_client_save.rs"),
    include_str!("lifecycle_tail.rs"),
    include_str!("load_post_process.rs"),
    include_str!("object.rs"),
    include_str!("persist_v18.rs"),
    include_str!("player.rs"),
    include_str!("restore.rs"),
    include_str!("shroud.rs"),
    include_str!("special_powers.rs"),
    include_str!("special_power_cooldown_persist.rs"),
    include_str!("subdual_persist.rs"),
    include_str!("hotkey_squad_persist.rs"),
    include_str!("booby_trap_persist.rs"),
    include_str!("carpet_bomb_persist.rs"),
    include_str!("production_door_persist.rs"),
    include_str!("dozer_repair_persist.rs"),
    include_str!("rebuild_hole_persist.rs"),
    include_str!("weapon_set_persist.rs"),
    include_str!("ability_hijack_persist.rs"),
    include_str!("ai_team_persist.rs"),
    include_str!("dock_queue_persist.rs"),
    include_str!("module_runtime_persist.rs"),
    include_str!("deliver_payload_persist.rs"),
    include_str!("object_module_xfer_persist.rs"),
    include_str!("auto_deposit_persist.rs"),
    include_str!("supply_drop_persist.rs"),
    include_str!("jet_ai_persist.rs"),
    include_str!("chinook_ai_persist.rs"),
    include_str!("hacker_income_persist.rs"),
    include_str!("warehouse_crippling_persist.rs"),
    include_str!("helix_napalm_persist.rs"),
    include_str!("money_crate_persist.rs"),
    include_str!("gps_scrambler_persist.rs"),
    include_str!("dynamic_shroud_persist.rs"),
    include_str!("angry_mob_persist.rs"),
    include_str!("power_plant_rods_persist.rs"),
    include_str!("cleanup_hazard_persist.rs"),
    include_str!("point_defense_persist.rs"),
    include_str!("projectile_stream_persist.rs"),
    include_str!("transport_exit_persist.rs"),
    include_str!("bridge_behavior_persist.rs"),
    include_str!("object_xfer_persist.rs"),
    include_str!("ai_player_queue_persist.rs"),
    include_str!("inferno_fire_persist.rs"),
    include_str!("firewall_persist.rs"),
    include_str!("neutron_slow_death_persist.rs"),
    include_str!("turret_aim_persist.rs"),
    include_str!("stealth_grant_persist.rs"),
    include_str!("weapon_leech_persist.rs"),
    include_str!("score_keeper_persist.rs"),
    include_str!("garrison_firepoint_persist.rs"),
    include_str!("stealth_detector_persist.rs"),
    include_str!("particle_system_save.rs"),
    include_str!("terrain_visual_save.rs"),
    include_str!("player_team_persist.rs"),
    include_str!("terrain.rs"),
    include_str!("types.rs"),
    include_str!("xfer_helpers.rs"),
);
