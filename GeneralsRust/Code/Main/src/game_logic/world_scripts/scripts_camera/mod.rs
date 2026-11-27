//! Host scripts `impl GameLogic` — `scripts_camera`.
//! Child of `world_scripts` (itself a child of `game_logic.rs`).
//! script eval / EVA process / camera path / script camera
#![allow(unused_imports, non_snake_case)]
use super::super::*;
use std::collections::HashMap;

/// C++ KINDOF_INERT from leftover ThingTemplate when the factory is already loaded.
/// Never calls TheThingFactory::find_template (that lazy-inits Object INI).
fn leftover_host_template_is_inert(template_name: &str) -> bool {
    if template_name.is_empty() {
        return false;
    }
    game_engine::common::thing::thing_factory::try_get_thing_factory()
        .and_then(|guard| {
            guard
                .as_ref()
                .and_then(|factory| factory.find_template(template_name, false))
        })
        .is_some_and(|template| {
            template.is_kind_of_mask(game_engine::common::system::kind_of::KindOfMask::INERT.bits())
        })
}

/// A present empty list is distinct from an absent list. The receiving
/// template lookup takes precedence in the caller (ScriptActions5158/5253).
fn host_script_object_type_names(
    interpreter: &gamelogic::scripting::engine::ScriptEngine,
    name: &str,
) -> Option<Vec<String>> {
    interpreter.get_object_types(name).map(|list| {
        list.iter().map(|name| name.as_str().to_owned()).collect()
    })
}

/// C++ Object::getShroudedStatus CLEAR|PARTIAL_CLEAR per player (no stealth filter).
fn host_discovered_by_player_names(
    logic: &crate::game_logic::GameLogic,
    shroud: Option<&gamelogic::system::shroud_manager::ShroudManager>,
    object_id: u32,
    obj: &crate::game_logic::object::Object,
) -> Vec<String> {
    use generals_sim::object::ObjectShroudStatus;
    if obj.status.disabled_held {
        return Vec::new();
    }
    let mut names = Vec::new();
    for player in logic.players.values() {
        let status =
            shroud.and_then(|mgr| mgr.get_host_object_shroud_status(player.id, object_id));
        let visible = match status {
            Some(ObjectShroudStatus::Clear | ObjectShroudStatus::PartialClear) => true,
            Some(_) => false,
            None => obj.owner_player_id == Some(player.id),
        };
        if visible && !player.name.is_empty() {
            names.push(player.name.clone());
        }
    }
    names
}

/// C++ Object::getStatusBits: packed OBJECT_STATUS_* from live host flags.
fn host_query_object_status_bits(obj: &crate::game_logic::object::Object) -> u64 {
    use game_engine::common::system::object_status_types::ObjectStatusMaskType as Mask;
    let s = &obj.status;
    let mut bits = obj.object_status_mask_for_queries();
    if s.destroyed {
        bits |= Mask::DESTROYED.bits();
    }
    if s.under_construction {
        bits |= Mask::UNDER_CONSTRUCTION.bits();
    }
    if s.unselectable {
        bits |= Mask::UNSELECTABLE.bits();
    }
    if s.no_collisions {
        bits |= Mask::NO_COLLISIONS.bits();
    }
    if s.airborne_target {
        bits |= Mask::AIRBORNE_TARGET.bits();
    }
    if s.parachuting {
        bits |= Mask::PARACHUTING.bits();
    }
    if s.repulsor {
        bits |= Mask::REPULSOR.bits();
    }
    if s.hijacked {
        bits |= Mask::HIJACKED.bits();
    }
    if s.wet {
        bits |= Mask::WET.bits();
    }
    if s.is_firing_weapon {
        bits |= Mask::IS_FIRING_WEAPON.bits();
    }
    if s.sold {
        bits |= Mask::SOLD.bits();
    }
    if s.reconstructing {
        bits |= Mask::RECONSTRUCTING.bits();
    }
    if s.masked {
        bits |= Mask::MASKED.bits();
    }
    if s.attacking {
        bits |= Mask::IS_ATTACKING.bits();
    }
    if s.using_ability {
        bits |= Mask::USING_ABILITY.bits();
    }
    if s.is_aiming_weapon {
        bits |= Mask::IS_AIMING_WEAPON.bits();
    }
    if s.ignoring_stealth {
        bits |= Mask::IGNORING_STEALTH.bits();
    }
    if s.is_carbomb {
        bits |= Mask::IS_CARBOMB.bits();
    }
    if s.deck_height_offset {
        bits |= Mask::DECK_HEIGHT_OFFSET.bits();
    }
    if s.faerie_fire {
        bits |= Mask::FAERIE_FIRE.bits();
    }
    if s.disguised {
        bits |= Mask::DISGUISED.bits();
    }
    if s.deployed {
        bits |= Mask::DEPLOYED.bits();
    }
    bits
}

#[cfg(test)]
mod booby_status_observation_tests {
    use super::*;

    #[test]
    fn script_status_query_projects_canonical_flags_and_retains_raw_bits() {
        use crate::game_logic::host_status_bits_upgrade::object_status_mask_from_names;

        let mut object = crate::game_logic::object::Object::new(
            crate::game_logic::ThingTemplate::new("ScriptObservedStatusTarget"),
            crate::game_logic::ObjectId(1),
            crate::game_logic::Team::GLA,
        );
        let raw = object_status_mask_from_names(&["STATUS_RIDER4", "NO_ATTACK"]);
        object.object_status_bits = raw | object_status_mask_from_names(&["STEALTHED"]);
        object.status.stealthed = false;
        object.status.detected = true;
        object.status.under_construction = true;
        object.status.using_ability = true;
        object.status.ignoring_stealth = true;
        object.status.deployed = true;

        assert_eq!(
            host_query_object_status_bits(&object),
            raw | object_status_mask_from_names(&[
                "DETECTED",
                "UNDER_CONSTRUCTION",
                "USING_ABILITY",
                "IGNORING_STEALTH",
                "DEPLOYED",
            ])
        );
    }

    #[test]
    fn script_status_query_observes_mask_mutation_through_canonical_status() {
        let mut object = crate::game_logic::object::Object::new(
            crate::game_logic::ThingTemplate::new("ScriptObservedTrapTarget"),
            crate::game_logic::ObjectId(1),
            crate::game_logic::Team::GLA,
        );
        object.apply_status_bits_upgrade_masks(&["BOOBY_TRAPPED"], &[]);
        let observed = host_query_object_status_bits(&object);
        assert!(
            crate::game_logic::host_status_bits_upgrade::status_bits_has(observed, "BOOBY_TRAPPED")
        );
        object.apply_status_bits_upgrade_masks(&[], &["BOOBY_TRAPPED"]);
        let observed = host_query_object_status_bits(&object);
        assert!(
            !crate::game_logic::host_status_bits_upgrade::status_bits_has(
                observed,
                "BOOBY_TRAPPED"
            )
        );
    }
}

/// C++ OpenContain::getPlayerWhoEntered — SIDE name, one-frame pulse after enter.
fn host_query_player_who_entered(
    logic: &GameLogic,
    obj: &crate::game_logic::object::Object,
) -> String {
    obj.player_who_entered
        .and_then(|id| logic.player_name(id))
        .unwrap_or_default()
}

mod camera_motion;
mod camera_reset;
mod named_commands;
#[cfg(test)]
mod named_lifecycle_owner_tests;
mod nearest_object_move;
#[cfg(test)]
mod nearest_object_move_tests;
mod owned_team_state;
mod path_admission;
#[cfg(test)]
mod prerequisite_owner_tests;
mod script_action_effects;
mod script_execution_driver;
mod script_runtime_camera;
mod script_state;
mod script_team_actions;
mod script_unit_actions;
mod team_guard;
mod team_stop;

#[cfg(test)]
mod sequential_actor_tests;

#[cfg(test)]
mod script_action_order_tests;

#[cfg(test)]
mod script_activation_owner_tests;

#[cfg(test)]
mod script_display_owner_tests;

#[cfg(test)]
mod script_camera_owner_tests;

#[cfg(test)]
mod script_completion_owner_tests;

#[cfg(test)]
mod script_variable_owner_tests;

#[cfg(test)]
mod science_edge_tests;

#[cfg(test)]
mod remaining_camera_owner_tests;

#[cfg(test)]
mod camera_view_drain_tests;

#[cfg(test)]
mod script_ai_player_owner_tests;

#[cfg(test)]
mod bridge_condition_owner_tests;

#[cfg(test)]
mod tech_building_owner_tests;

#[cfg(test)]
mod named_command_owner_tests;

#[cfg(test)]
mod player_event_owner_tests;

#[cfg(test)]
mod named_command_test_support;
#[cfg(test)]
mod named_guard_owner_tests;
#[cfg(test)]
mod named_hunt_owner_tests;
#[cfg(test)]
mod named_identity_owner_tests;

#[cfg(test)]
mod guard_phase_owner_tests;

#[cfg(test)]
mod guard_return_rate_control_tests;

#[cfg(test)]
mod guard_quick_exit_owner_tests;

#[cfg(test)]
mod team_guard_owner_tests;

#[cfg(test)]
mod team_save_owner_tests;

#[cfg(test)]
mod team_context_owner_tests;

#[cfg(test)]
mod team_condition_loop_control_tests;

#[cfg(test)]
mod condition_cursor_control_tests;
#[cfg(test)]
mod condition_cursor_owner_tests;

#[cfg(test)]
mod sighting_owner_tests;

#[cfg(test)]
mod condition_body_census_tests;

#[cfg(test)]
mod eva_script_owner_tests;

#[cfg(test)]
mod scoring_owner_tests;

#[cfg(test)]
mod containment_command_owner_tests;

#[cfg(test)]
mod named_attack_team_owner_tests;

mod script_ai_build;
#[cfg(test)]
mod script_ai_build_owner_tests;

#[cfg(test)]
mod named_attack_area_owner_tests;

#[cfg(test)]
mod script_idle_owner_tests;

#[cfg(test)]
mod player_census_owner_tests;

#[cfg(test)]
mod hulk_lifetime_owner_tests;
#[cfg(test)]
mod owned_runtime_command_tests;
mod owned_runtime_commands;
#[cfg(test)]
mod owned_world_effect_tests;

#[cfg(test)]
mod qualified_waypoint_owner_tests;

mod team_attack_named;
#[cfg(test)]
mod team_attack_named_forwarding_tests;

#[cfg(test)]
mod named_entry_condition_owner_tests;

#[cfg(test)]
mod team_presence_owner_tests;

#[cfg(test)]
mod named_ownership_condition_tests;

mod containment_observation;

#[cfg(test)]
mod containment_condition_owner_tests;

#[cfg(test)]
mod money_owner_tests;

#[cfg(test)]
mod overlord_containment_owner_tests;

#[cfg(test)]
mod player_area_owner_tests;

mod script_lifecycle;
#[cfg(test)]
mod script_lifecycle_owner_tests;

#[cfg(test)]
mod team_containment_owner_tests;

#[cfg(test)]
mod named_frame_command_owner_tests;

#[cfg(test)]
mod selection_owner_tests;

mod script_nearest_button;
#[cfg(test)]
mod script_nearest_button_owner_tests;

#[cfg(all(test, feature = "game_client"))]
mod camera_completion_cpp_tests;

#[cfg(test)]
mod camera_admission_cpp_tests;

mod debug_freeze;

#[cfg(all(test, feature = "game_client"))]
mod camera_fade_owner_tests;

#[cfg(test)]
mod behind_building_marker_owner_tests;

#[cfg(test)]
mod draw_icon_ui_owner_tests;

mod waypoint_routes;

#[cfg(test)]
mod waypoint_identity_owner_tests;

#[cfg(test)]
mod victory_condition_owner_tests;

#[cfg(test)]
mod buildability_owner_tests;

#[cfg(test)]
mod special_power_notification_owner_tests;

#[cfg(test)]
mod team_capture_nearest_owner_tests;
