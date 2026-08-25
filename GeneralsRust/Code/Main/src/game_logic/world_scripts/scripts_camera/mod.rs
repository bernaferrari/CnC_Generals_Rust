//! Host scripts `impl GameLogic` — `scripts_camera`.
//! Child of `world_scripts` (itself a child of `game_logic.rs`).
//! script eval / EVA process / camera path / script camera
#![allow(unused_imports, non_snake_case)]
use super::super::*;
use std::cell::RefCell;
use std::collections::HashMap;

thread_local! {
    static HOST_PREV_BRIDGE_BROKEN: RefCell<HashMap<String, bool>> =
        RefCell::new(HashMap::new());
}

fn merge_host_bridge_states(
    world: &GameLogic,
    snap: &mut gamelogic::scripting::HostScriptQuerySnapshot,
) {
    use crate::game_logic::host_bridge_behavior::is_bridge_span_template;
    let mut current = HashMap::new();
    for obj in world.host_objects().values() {
        if obj.name.is_empty() || !is_bridge_span_template(&obj.template_name) {
            continue;
        }
        let broken = !obj.is_alive() || obj.status.destroyed || obj.health.current <= 0.0;
        current.insert(obj.name.clone(), broken);
        snap.named_bridge_broken.insert(obj.name.clone(), broken);
        snap.named_bridge_repaired.insert(obj.name.clone(), !broken);
    }
    snap.any_bridges_damage_states_changed = HOST_PREV_BRIDGE_BROKEN.with(|prev| {
        let mut prev = prev.borrow_mut();
        let changed = !prev.is_empty()
            && current
                .iter()
                .any(|(name, broken)| prev.get(name) != Some(broken));
        *prev = current;
        changed
    });
}

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

/// C++ `objectTypesFromParam` / leftover `resolve_object_types_for_action`.
fn host_script_object_type_names(object_type: &str) -> Vec<String> {
    if object_type.is_empty() {
        return Vec::new();
    }
    if let Some(Some(list)) = gamelogic::scripting::engine::with_script_engine_ref(|engine| {
        engine.get_object_types(object_type)
    }) {
        let mut names = Vec::new();
        for i in 0..list.list_size() as i32 {
            if let Some(name) = list.nth_in_list(i) {
                names.push(name.as_str().to_string());
            }
        }
        if !names.is_empty() {
            return names;
        }
    }
    vec![object_type.to_string()]
}

/// C++ Object::getShroudedStatus CLEAR|PARTIAL_CLEAR per player (no stealth filter).
fn host_discovered_by_player_names(
    logic: &crate::game_logic::GameLogic,
    object_id: u32,
    obj: &crate::game_logic::object::Object,
) -> Vec<String> {
    use gamelogic::common::ObjectShroudStatus;
    if obj.status.disabled_held {
        return Vec::new();
    }
    let mut names = Vec::new();
    let shroud = gamelogic::system::shroud_manager::get_shroud_manager()
        .lock()
        .ok();
    for player in logic.players.values() {
        let status = shroud
            .as_ref()
            .and_then(|mgr| mgr.get_host_object_shroud_status(player.id, object_id));
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

fn leftover_waypoint_path_labels(path_label: &str, last: glam::Vec3) -> Vec<String> {
    let mut labels = Vec::new();
    if !path_label.is_empty() {
        labels.push(path_label.to_string());
    }
    let Ok(terrain) = gamelogic::terrain::get_terrain_logic().read() else {
        return labels;
    };
    let pos = gamelogic::common::Coord3D::new(last.x, last.z, last.y);
    if let Some(wp) = terrain.get_closest_waypoint_on_path(&pos, path_label) {
        for label in [
            wp.get_path_label1().as_str(),
            wp.get_path_label2().as_str(),
            wp.get_path_label3().as_str(),
        ] {
            if !label.is_empty() && !labels.iter().any(|existing| existing == label) {
                labels.push(label.to_string());
            }
        }
    }
    labels
}

/// C++ Object::getStatusBits: packed OBJECT_STATUS_* from live host flags.
fn host_query_object_status_bits(obj: &crate::game_logic::object::Object) -> u64 {
    use crate::game_logic::host_status_bits_upgrade::object_status_mask_from_names;
    let s = &obj.status;
    let mut names: Vec<&str> = Vec::new();
    if s.destroyed {
        names.push("DESTROYED");
    }
    if s.under_construction {
        names.push("UNDER_CONSTRUCTION");
    }
    if s.unselectable {
        names.push("UNSELECTABLE");
    }
    if s.no_collisions {
        names.push("NO_COLLISIONS");
    }
    if s.airborne_target {
        names.push("AIRBORNE_TARGET");
    }
    if s.parachuting {
        names.push("PARACHUTING");
    }
    if s.repulsor {
        names.push("REPULSOR");
    }
    if s.hijacked {
        names.push("HIJACKED");
    }
    if s.wet {
        names.push("WET");
    }
    if s.is_firing_weapon {
        names.push("IS_FIRING_WEAPON");
    }
    if s.stealthed {
        names.push("STEALTHED");
    }
    if s.detected {
        names.push("DETECTED");
    }
    if s.sold {
        names.push("SOLD");
    }
    if s.reconstructing {
        names.push("RECONSTRUCTING");
    }
    if s.masked {
        names.push("MASKED");
    }
    if s.attacking {
        names.push("IS_ATTACKING");
    }
    if s.using_ability {
        names.push("USING_ABILITY");
    }
    if s.is_aiming_weapon {
        names.push("IS_AIMING_WEAPON");
    }
    if s.ignoring_stealth {
        names.push("IGNORING_STEALTH");
    }
    if s.is_carbomb {
        names.push("IS_CARBOMB");
    }
    if s.deck_height_offset {
        names.push("DECK_HEIGHT_OFFSET");
    }
    if s.faerie_fire {
        names.push("FAERIE_FIRE");
    }
    if s.booby_trapped {
        names.push("BOOBY_TRAPPED");
    }
    if s.disguised {
        names.push("DISGUISED");
    }
    if s.deployed {
        names.push("DEPLOYED");
    }
    obj.object_status_bits | object_status_mask_from_names(&names)
}

/// C++ OpenContain::getPlayerWhoEntered — SIDE name, one-frame pulse after enter.
fn host_query_player_who_entered(
    _logic: &GameLogic,
    obj: &crate::game_logic::object::Object,
) -> String {
    obj.player_who_entered.clone()
}

fn panel_flag_is_indestructible(flag: &str) -> bool {
    flag.chars()
        .filter(|c| !c.is_ascii_whitespace() && *c != '_')
        .collect::<String>()
        .eq_ignore_ascii_case("indestructible")
}

mod script_runtime_camera;
mod script_state;
mod script_team_actions;
mod script_unit_actions;
