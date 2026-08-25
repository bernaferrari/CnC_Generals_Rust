//! Host special-power / strike / OCL modules.
//!
//! `#[path]` keeps the `.rs` files in this directory. Parent types and a
//! few sibling modules are imported so existing `use super::ObjectId`
//! (and similar) paths in those files keep resolving.

#![allow(unused_imports)]

use super::ObjectId;
use super::Team;
use super::VeterancyLevel;
use super::Weapon;
use super::combat;

#[path = "host_a10_strike_flight.rs"]
pub mod host_a10_strike_flight;

#[path = "host_anthrax_bomb_flight.rs"]
pub mod host_anthrax_bomb_flight;

#[path = "host_artillery_barrage_flight.rs"]
pub mod host_artillery_barrage_flight;

#[path = "host_carpet_bomb_flight.rs"]
pub mod host_carpet_bomb_flight;

#[path = "host_cluster_mines_flight.rs"]
pub mod host_cluster_mines_flight;

#[path = "host_daisy_cutter_flight.rs"]
pub mod host_daisy_cutter_flight;

#[path = "host_emp_pulse_flight.rs"]
pub mod host_emp_pulse_flight;

#[path = "host_fuel_air_gas_slow_death.rs"]
pub mod host_fuel_air_gas_slow_death;

#[path = "host_neutron_missile_update.rs"]
pub mod host_neutron_missile_update;

#[path = "host_ocl_apply_random_force.rs"]
pub mod host_ocl_apply_random_force;

#[path = "host_ocl_create_debris.rs"]
pub mod host_ocl_create_debris;

#[path = "host_ocl_fire_weapon_attack.rs"]
pub mod host_ocl_fire_weapon_attack;

#[path = "host_ocl_special_power.rs"]
pub mod host_ocl_special_power;

#[path = "host_scud_storm_missile_flight.rs"]
pub mod host_scud_storm_missile_flight;

#[path = "host_missile_launcher_building_update.rs"]
pub mod host_missile_launcher_building_update;

#[path = "host_special_power_update_module.rs"]
pub mod host_special_power_update_module;

#[path = "host_spectre_gunship_deployment.rs"]
pub mod host_spectre_gunship_deployment;

#[path = "host_spectre_gunship_update.rs"]
pub mod host_spectre_gunship_update;

#[path = "host_ambush.rs"]
pub mod host_ambush;

#[path = "host_baikonur_launch.rs"]
pub(super) mod host_baikonur_launch;

#[path = "host_cash_bounty.rs"]
pub mod host_cash_bounty;

#[path = "host_cia_intelligence.rs"]
pub mod host_cia_intelligence;

#[path = "host_satellite_hack.rs"]
pub mod host_satellite_hack;

#[path = "host_cleanup_area.rs"]
pub mod host_cleanup_area;

#[path = "host_defector_special_power.rs"]
pub(super) mod host_defector_special_power;

#[path = "host_deliver_payload.rs"]
pub mod host_deliver_payload;

#[path = "host_emergency_repair.rs"]
pub mod host_emergency_repair;

#[path = "host_emp_pulse.rs"]
pub mod host_emp_pulse;

#[path = "host_frenzy.rs"]
pub mod host_frenzy;

#[path = "host_gps_scrambler.rs"]
pub mod host_gps_scrambler;

#[path = "host_hero_abilities.rs"]
pub mod host_hero_abilities;

#[path = "host_historic_bonus.rs"]
pub mod host_historic_bonus;

#[path = "host_leaflet_drop.rs"]
pub mod host_leaflet_drop;

#[path = "host_neutron_missile_slow_death.rs"]
pub mod host_neutron_missile_slow_death;

#[path = "host_neutron_shell.rs"]
pub mod host_neutron_shell;

#[path = "host_nuclear_tanks.rs"]
pub mod host_nuclear_tanks;

#[path = "host_nuke_cannon.rs"]
pub mod host_nuke_cannon;

#[path = "host_paradrop.rs"]
pub mod host_paradrop;

#[path = "host_point_defense.rs"]
pub mod host_point_defense;

#[path = "host_radar_scan.rs"]
pub mod host_radar_scan;

#[path = "host_science_rank.rs"]
pub mod host_science_rank;

#[path = "host_sneak_attack.rs"]
pub mod host_sneak_attack;

#[path = "host_special_power_completion_die.rs"]
pub(super) mod host_special_power_completion_die;

#[path = "host_spy_drone.rs"]
pub mod host_spy_drone;

#[path = "host_spy_satellite.rs"]
pub mod host_spy_satellite;

#[path = "host_superweapon_kindof.rs"]
pub mod host_superweapon_kindof;

#[path = "host_unit_training.rs"]
pub mod host_unit_training;
