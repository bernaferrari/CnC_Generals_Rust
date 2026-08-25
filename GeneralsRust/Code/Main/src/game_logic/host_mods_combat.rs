//! Host combat / projectile / death modules.
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

use super::host_gamedata_lobby_residual;
use super::host_money_crate;
use super::host_rng_residual;
use super::host_slave_drones;

#[path = "host_animation_steering.rs"]
pub mod host_animation_steering;

#[path = "host_checkpoint_update.rs"]
pub mod host_checkpoint_update;

#[path = "host_create_crate_die.rs"]
pub mod host_create_crate_die;

#[path = "host_float_update.rs"]
pub mod host_float_update;

#[path = "host_projectile_stream.rs"]
pub mod host_projectile_stream;

#[path = "host_prone_update.rs"]
pub mod host_prone_update;

#[path = "host_radius_decal_update.rs"]
pub mod host_radius_decal_update;

#[path = "host_repulsor_gate.rs"]
pub mod host_repulsor_gate;

#[path = "host_smart_bomb_target_homing.rs"]
pub mod host_smart_bomb_target_homing;

#[path = "host_aurora_bomb.rs"]
pub mod host_aurora_bomb;

#[path = "host_bomb_truck_detonate.rs"]
pub mod host_bomb_truck_detonate;

#[path = "host_bomb_truck_disguise.rs"]
pub mod host_bomb_truck_disguise;

#[path = "host_bone_fx_damage.rs"]
pub(super) mod host_bone_fx_damage;

#[path = "host_booby_trap.rs"]
pub mod host_booby_trap;

#[path = "host_bunker_buster.rs"]
pub mod host_bunker_buster;

#[path = "host_car_bomb.rs"]
pub mod host_car_bomb;

#[path = "host_comanche_rocket_pods.rs"]
pub mod host_comanche_rocket_pods;

#[path = "host_combat_chinook.rs"]
pub mod host_combat_chinook;

#[path = "host_combat_cycle.rs"]
pub mod host_combat_cycle;

#[path = "host_command_button_hunt.rs"]
pub(super) mod host_command_button_hunt;

#[path = "host_countermeasures.rs"]
pub mod host_countermeasures;

#[path = "host_create_object_die.rs"]
pub(super) mod host_create_object_die;

#[path = "host_crush_die.rs"]
pub(super) mod host_crush_die;

#[path = "host_dam_die.rs"]
pub(super) mod host_dam_die;

#[path = "host_defection_helper.rs"]
pub mod host_defection_helper;

#[path = "host_demo_suicide_bomb.rs"]
pub mod host_demo_suicide_bomb;

#[path = "host_deploy_style.rs"]
pub(super) mod host_deploy_style;

#[path = "host_enemy_near.rs"]
pub mod host_enemy_near;

#[path = "host_fire_base.rs"]
pub mod host_fire_base;

#[path = "host_fire_spread.rs"]
pub mod host_fire_spread;

#[path = "host_fire_weapon_power.rs"]
pub(super) mod host_fire_weapon_power;

#[path = "host_fire_weapon_when_damaged.rs"]
pub mod host_fire_weapon_when_damaged;

#[path = "host_fire_weapon_when_dead.rs"]
pub(super) mod host_fire_weapon_when_dead;

#[path = "host_temporary_weapon_behavior.rs"]
pub mod host_temporary_weapon_behavior;

#[path = "host_fx_list_die.rs"]
pub(super) mod host_fx_list_die;

#[path = "host_fx_list_dispatch.rs"]
pub(super) mod host_fx_list_dispatch;
pub use host_fx_list_dispatch::{
    dispatch_fx_list_at_object, dispatch_fx_list_at_pos, dispatch_fx_list_at_pos_ex,
    dispatch_fx_list_at_pos_oriented, host_to_leftover_mat4, is_authored_fx_list_name,
    particle_template_names_for_fx_list, publish_host_fx_object, publish_host_fx_object_ex,
    refresh_host_fx_object_poses_from_presentation, resolve_audio_event_names,
    sound_names_for_fx_list,
};

#[path = "host_heal.rs"]
pub mod host_heal;

#[path = "host_helicopter_slow_death.rs"]
pub mod host_helicopter_slow_death;

#[path = "host_helix_minigun.rs"]
pub mod host_helix_minigun;

#[path = "host_helix_napalm.rs"]
pub mod host_helix_napalm;

#[path = "host_highlander_body.rs"]
pub(super) mod host_highlander_body;

#[path = "host_inferno_cannon.rs"]
pub mod host_inferno_cannon;

#[path = "host_instant_death.rs"]
pub(super) mod host_instant_death;

#[path = "host_jet_slow_death.rs"]
pub mod host_jet_slow_death;

#[path = "host_keep_object_die.rs"]
pub(super) mod host_keep_object_die;

#[path = "host_mines.rs"]
pub mod host_mines;

#[path = "host_missile_defender.rs"]
pub mod host_missile_defender;

#[path = "host_passengers_fire_upgrade.rs"]
pub mod host_passengers_fire_upgrade;

#[path = "host_poisoned_behavior.rs"]
pub mod host_poisoned_behavior;

#[path = "host_red_guard.rs"]
pub mod host_red_guard;

#[path = "host_slow_death.rs"]
pub mod host_slow_death;

#[path = "host_squish_collide.rs"]
pub(super) mod host_squish_collide;

#[path = "host_status_damage.rs"]
pub(super) mod host_status_damage;

#[path = "host_stealth_fighter.rs"]
pub mod host_stealth_fighter;

#[path = "host_structure_topple.rs"]
pub mod host_structure_topple;

#[path = "host_topple.rs"]
pub mod host_topple;

#[path = "host_toxin_tractor.rs"]
pub mod host_toxin_tractor;

#[path = "host_transition_damage_fx.rs"]
pub(super) mod host_transition_damage_fx;

#[path = "host_upgrade_die.rs"]
pub(super) mod host_upgrade_die;

#[path = "host_wave_guide.rs"]
pub(super) mod host_wave_guide;

#[path = "host_weapon_laser.rs"]
pub mod host_weapon_laser;
