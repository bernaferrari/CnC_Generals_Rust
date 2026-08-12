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
//! - Client `ParticleSystemManager` GPU rebind after load (host registry only)
//! - Full retail Upgrade.ini BuildTime / ProductionUpdate research timers
//! - Full C++ per-module WeaponSet / SpecialPowerModule / Upgrade Xfer tables

// Wave 956: host_object/host_objects authority dual-read seal.
//! Wave 957: host_object/host_objects authority dual-read seal.

mod ai;
mod builder;
mod game_state;
mod load_post_process;
mod object;
mod player;
mod restore;
mod special_powers;
mod terrain;
mod types;
mod xfer_helpers;

#[cfg(test)]
mod tests;

pub use ai::*;
pub use builder::*;
pub use game_state::*;
pub use object::*;
pub use player::*;
pub use special_powers::*;
pub use terrain::*;
pub use types::*;

/// Concatenated live snapshot sources for residual `include_str` scans.
pub const SNAPSHOT_SRC: &str = concat!(
    include_str!("mod.rs"),
    include_str!("ai.rs"),
    include_str!("builder.rs"),
    include_str!("game_state.rs"),
    include_str!("load_post_process.rs"),
    include_str!("object.rs"),
    include_str!("player.rs"),
    include_str!("restore.rs"),
    include_str!("special_powers.rs"),
    include_str!("terrain.rs"),
    include_str!("types.rs"),
    include_str!("xfer_helpers.rs"),
);
