//! Game logic, owned runtime adapters, and executable behavioral regressions.

#[cfg(test)]
mod runtime_regressions;

pub(crate) mod armor_definitions;
pub mod audio_dispatch_impl;
pub(crate) mod body_damage_rules;
pub mod buildings;
pub mod combat;
pub mod combat_particles;
pub(crate) mod crate_object_bridge;
pub mod game_logic;

pub mod host_transport_return;
pub(crate) mod lighting_rules;
pub(crate) mod native_die_definition;
pub(crate) mod native_lifecycle;
pub(crate) mod native_lifecycle_definition;
pub(crate) mod physics_rules;
pub(crate) use game_logic::kill_credit;
pub(crate) mod production_energy;
pub(crate) mod weapon_bonus_owner;
mod script_ui;
mod session_audio;
pub use session_audio::SessionAudio;
pub(crate) use session_audio::SessionAudioSelection;
pub(crate) mod terrain_definitions;
pub(crate) use game_logic::PathfindingHeightSamples;
pub(crate) use script_ui::ScriptUiEffects;
mod script_presentation;
#[cfg(feature = "game_client")]
pub(crate) use script_presentation::NativeAuxiliarySource;
pub(crate) use script_presentation::{
    NativeAudioEffects, NativeMapAdmission, NativeSlaveSource, NativeTerrainEffects,
    NativeViewEffects, ScriptPresentationEffects,
};
#[cfg(feature = "game_client")]
pub(crate) use script_presentation::{NativeBodyVisualChange, NativeProjectedRadiusAllocator};
#[cfg(feature = "game_client")]
pub(crate) use script_presentation::{NativeFactoryDrawableSource, NativeObjectRetirement};

#[cfg(test)]
pub(crate) fn evaluate_and_execute_scripts_for_test(logic: &mut game_logic::GameLogic, dt: f32) {
    logic.evaluate_and_execute_scripts(dt);
}

mod host_types;
pub(crate) mod host_upgrade_rules;
mod special_power_notifications;
pub use host_types::*;
pub(crate) use special_power_notifications::ScriptEventRecipient;
// Child modules (`use super::*`) historically saw these parent imports.
use glam::{Vec2, Vec3};
use std::collections::HashMap;

mod host_mods_combat;
pub use host_mods_combat::*;
mod host_mods_special_powers;
pub use host_mods_special_powers::*;
mod host_mods_structures;
pub use host_mods_structures::*;
mod host_mods_residuals_on;
pub use host_mods_residuals_on::presentation_command_filters::presentation_selected_sellable_structure_ids;
pub use host_mods_residuals_on::presentation_mouse_input::mouse_game_logic_for_process_mouse_input;
pub use host_mods_residuals_on::presentation_selection::host_ui_selected_ids_from_residuals;
pub use host_mods_residuals_on::retail_map_resolution::{
    DEFAULT_SKIRMISH_MAP_WAVE169, LONE_EAGLE_MAP_WAVE169, resolve_retail_map_path,
};
pub use host_mods_residuals_on::retail_map_resolution::{
    honesty_default_skirmish_map_resolves, honesty_lone_eagle_map_resolves,
};
pub use host_mods_residuals_on::*;
mod host_mods_units;
pub use host_mods_units::*;
mod host_mods_logs_a;
pub use host_mods_logs_a::*;
pub(crate) mod host_health_log;
pub use host_health_log::HostHealthEvents;
mod host_mods_logs_b;
pub use host_mods_logs_b::*;
mod host_mods_logs_c;
pub use host_mods_logs_c::*;

use host_mods_combat::host_bone_fx_damage;
use host_mods_combat::host_command_button_hunt;
pub(crate) use host_mods_combat::host_command_button_hunt::{
    HUNT_CMD_FROM_AI, HUNT_CMD_FROM_PLAYER, HUNT_CMD_FROM_SCRIPT,
};
use host_mods_combat::host_create_object_die;
use host_mods_combat::host_crush_die;
use host_mods_combat::host_dam_die;
use host_mods_combat::host_deploy_style;
use host_mods_combat::host_fire_weapon_power;
use host_mods_combat::host_fire_weapon_when_dead;
use host_mods_combat::host_fx_list_die;
use host_mods_combat::host_highlander_body;
use host_mods_combat::host_instant_death;
use host_mods_combat::host_keep_object_die;
use host_mods_combat::host_squish_collide;
use host_mods_combat::host_status_damage;
use host_mods_combat::host_transition_damage_fx;
use host_mods_combat::host_upgrade_die;
use host_mods_combat::host_wave_guide;
pub(crate) use host_mods_residuals_on::host_upgrade_module_residuals;
use host_mods_special_powers::host_baikonur_launch;
use host_mods_special_powers::host_defector_special_power;
use host_mods_special_powers::host_special_power_completion_die;
use host_mods_structures::host_black_market;
use host_mods_structures::host_model_condition_upgrade;
use host_mods_structures::host_preorder_create;
use host_mods_structures::host_replace_object_upgrade;
pub use host_mods_structures::host_sub_objects_upgrade;

pub mod locomotor_bootstrap;
pub mod mission_scripts;
mod object_audio;
pub(crate) mod promotion_definitions;
mod veterancy_feedback;
pub use veterancy_feedback::VeterancyFeedback;
pub mod object;
pub use object::{
    AttackSubState, DEFAULT_AERO_FRICTION_RESIDUAL, DEFAULT_FORWARD_FRICTION_RESIDUAL,
    DEFAULT_LATERAL_FRICTION_RESIDUAL, DEFAULT_Z_FRICTION_RESIDUAL, LocomotorAppearance,
    LocomotorBehaviorZ, MAX_FRICTION_RESIDUAL, MIN_AERO_FRICTION_RESIDUAL,
    MIN_NON_AERO_FRICTION_RESIDUAL, MIN_RECOMPUTE_TIME_RESIDUAL, MOTIVE_FRAMES_RESIDUAL,
    PATHFIND_CELL_SIZE_F_RESIDUAL, PhysicsTurningType, calc_slow_down_dist,
    is_same_position_residual,
};
pub use object_audio::ObjectAudio;
pub(crate) mod ai_definitions;
pub mod host_radar;
pub mod partition_coi;
mod partition_geometry;
pub mod partition_manager;
pub mod pathfinding;
pub(crate) mod projectile_definitions;
pub mod radar_notifications;
pub mod resources;
pub mod script_events;
pub mod script_loader;
pub mod special_power_strikes;
pub mod terrain;
pub mod thing;
pub mod units;
pub mod victory;
pub mod victory_conditions;
pub mod weapon_bootstrap;

pub use buildings::*;
pub use combat::*;
pub use combat_particles::{CombatParticleKind, CombatParticleRegistry, CombatParticleSystemEntry};
pub(crate) use game_logic::LogicCameraContinuation;
pub use game_logic::*;
pub(crate) use game_logic::{
    NativeCommandAction, NativeCommandCompletion, NativeTickContinuation, NativeTickProgress,
};
pub use locomotor_bootstrap::honesty_locomotor_residual_expand_wave92;
pub use locomotor_bootstrap::honesty_locomotor_residual_expand_wave103;
pub use locomotor_bootstrap::{
    BASIC_HUMAN_LOCOMOTOR, BATTLE_MASTER_LOCOMOTOR, CRUSADER_LOCOMOTOR, HUMVEE_LOCOMOTOR,
    REDGUARD_LOCOMOTOR, SCORPION_LOCOMOTOR, TECHNICAL_LOCOMOTOR, ensure_host_locomotor_store,
    locomotor_name_for_unit, resolve_host_movement,
};
pub use mission_scripts::*;
pub use object::*;
pub use partition_manager::*;
pub use pathfinding::*;
pub use radar_notifications::*;
pub use resources::*;
pub use script_events::*;
pub use script_loader::*;
pub use special_power_strikes::{
    HostRadiationField, HostSpecialPowerStrike, HostSpecialPowerStrikeRegistry, HostStrikePhase,
    HostSuperweaponKind, NUKE_RADIATION_DAMAGE_PER_TICK, NUKE_RADIATION_DURATION_FRAMES,
    NUKE_RADIATION_RADIUS, NUKE_RADIATION_TICK_INTERVAL_FRAMES,
};
pub use terrain::*;
pub use thing::*;
pub use units::*;
pub use victory::*;
pub use victory_conditions::*;
pub use weapon_bootstrap::honesty_weapon_store_deepen_residual_wave92;
pub use weapon_bootstrap::honesty_weapon_store_deepen_residual_wave103;
pub use weapon_bootstrap::{
    GATTLING_BUILDING_PRIMARY_WEAPON as HOST_GATTLING_BUILDING_PRIMARY_WEAPON,
    GLA_REBEL_PRIMARY_WEAPON, HOST_WEAPON_STORE_CORE_SEED_NAMES, HUMVEE_PRIMARY_WEAPON,
    HUMVEE_SECONDARY_WEAPON, PATRIOT_PRIMARY_WEAPON as HOST_PATRIOT_PRIMARY_WEAPON,
    RANGER_PRIMARY_WEAPON, RANGER_SECONDARY_WEAPON, REDGUARD_PRIMARY_WEAPON,
    ensure_host_weapon_store, honesty_weapon_store_host_seed_residual_wave77,
    primary_weapon_name_for_unit, secondary_weapon_name_for_unit,
};

#[cfg(test)]
mod production_construction_rules_tests;

#[cfg(test)]
pub(crate) mod containment_audio_test_support;

#[cfg(test)]
pub(crate) use game_logic::containment_audio_owner_tests;

pub(crate) mod player_controllers;

mod host_reexports;
pub use host_reexports::*;
