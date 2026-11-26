//! Host tick `impl GameLogic` split.
//! Submodules are grandchildren of `game_logic.rs`; `pub(in super::super)`
//! matches the previous `pub(super)` visibility on the flat file.
#![allow(unused_imports, non_snake_case)]

mod ai;
mod ai_phase;
mod airfield;
mod jet_lockon;
#[cfg(test)]
pub(in crate::game_logic) use airfield::heli_motion_tests;
mod attack;
mod collide_dispatch;
pub(in crate::game_logic) use collide_dispatch::{host_geom_collides, host_object_footprint};
mod collide_modules;
mod crate_contact;

mod combat;
mod combat_fire_fx;
mod crates;
mod disabled_expiry;
mod flight_terrain;
mod logic_crc;
#[cfg(all(test, not(target_arch = "wasm32")))]
mod projectile_terrain_owner_tests;
pub(crate) use flight_terrain::FlightTerrainView;
mod mood;
mod movement;
mod movement_support;
mod physics;
mod presence;
mod production;
mod shock;
#[cfg(test)]
mod splash_partition_order_tests;
#[cfg(test)]
mod self_position_damage_tests;
mod step;
pub(crate) use step::{
    NativeCommandAction, NativeCommandCompletion, NativeTickContinuation, NativeTickProgress,
};
mod water_owner;
mod team_membership;
#[cfg(test)]
mod team_membership_owner_tests;
mod teams;

#[cfg(test)]
mod temporary_move_owner_tests;

#[cfg(test)]
mod jet_aim_owner_tests;
#[cfg(test)]
mod alt_turret_tick_owner_tests;

impl super::GameLogic {
    /// Finalize this step's presentation receipts, even without an observer.
    /// EVA and construction stay pending for direct presentation consumption.
    pub(crate) fn complete_presentation_observations(&mut self) {
        let _ = self.health_events.drain_for_host_boundary();
        self.attack_events.complete_boundary();
        #[cfg(test)]
        let _ = crate::game_logic::host_destroy_log::drain();
        self.combat_system.fire_sound_loop_events.complete_boundary();
        let _ = self.movement_events.drain_for_host_boundary();
        self.ownership_events.complete_boundary();
        self.production_events.complete_boundary();
        #[cfg(test)]
        let _ = crate::game_logic::host_production_progress_log::drain();
        self.economy_events.complete_boundary();
    }
}
