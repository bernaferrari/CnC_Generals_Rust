//! Bridge damage-state transitions.
//!
//! C++ `Bridge::updateDamageState` (`TerrainLogic.cpp` 852-909):
//! body-module damage state (not health-percent buckets), `changeBridgeState`
//! on rubble, `DAMAGE_FALLING` / `DEATH_SPLATTED` for units on the layer, and
//! a scaffold gate when leaving rubble.

use crate::ai::the_ai;
use crate::common::Region2D;
use crate::common::{BodyDamageType, PathfindLayerEnum as CommonLayer};
use crate::damage::{DamageInfo, DamageType, DeathType, HUGE_DAMAGE_AMOUNT};
use crate::helpers::TheGameLogic;
use crate::object::Object;
use crate::object::registry::OBJECT_REGISTRY;
use crate::path::{LAYER_Z_CLOSE_ENOUGH_F, PATHFIND_CELL_SIZE_F, PathfindLayerEnum};
use crate::terrain::Bridge;

/// C++ `Bridge::updateDamageState`.
pub fn update_damage_state(bridge: &mut Bridge) {
    bridge.bridge_info_mut().damage_state_changed = false;
    if bridge.get_bridge_info().bridge_object_id == crate::common::INVALID_ID {
        return;
    }

    let object_id = bridge.get_bridge_info().bridge_object_id;
    let Some(obj_arc) = TheGameLogic::find_object_by_id(object_id) else {
        // Wave 341 host path: empty factory store and no GameLogic object.
        // C++ clears the id when the object is actually gone.
        if OBJECT_REGISTRY.store_is_empty() {
            return;
        }
        bridge.bridge_info_mut().bridge_object_id = crate::common::INVALID_ID;
        log::error!("Bridge object disappeared - unexpected. jba.");
        return;
    };

    let Ok(obj_guard) = obj_arc.read() else {
        return;
    };

    let Some(body) = obj_guard.get_body_module() else {
        return;
    };
    let Ok(body_guard) = body.lock() else {
        return;
    };
    let damage_state = body_guard.get_damage_state();
    drop(body_guard);

    let cur_state = bridge.get_bridge_info().cur_damage_state;
    if damage_state == cur_state {
        return;
    }

    bridge.bridge_info_mut().cur_damage_state = damage_state;
    let layer = bridge.get_layer();

    if damage_state == BodyDamageType::Rubble {
        change_bridge_state(layer, false);
        bridge.bridge_info_mut().damage_state_changed = true;
        splat_units_on_bridge(bridge, &obj_guard);
    }

    if cur_state == BodyDamageType::Rubble {
        // C++: do not re-enable the layer while scaffolding is up.
        if !bridge_has_scaffold(&obj_guard) {
            change_bridge_state(layer, true);
        }
        bridge.bridge_info_mut().damage_state_changed = true;
    }
}

fn change_bridge_state(layer: PathfindLayerEnum, repaired: bool) {
    let ai_store = the_ai(); if let Ok(mut ai) = ai_store.write() {
        ai.change_bridge_state(layer, repaired);
    }
}

fn bridge_has_scaffold(bridge_obj: &Object) -> bool {
    for module in bridge_obj.get_behavior_modules() {
        let Ok(mut guard) = module.lock() else {
            continue;
        };
        if let Some(bbi) = guard.get_bridge_behavior_interface() {
            return bbi.is_scaffold_present();
        }
    }
    false
}

fn splat_units_on_bridge(bridge: &Bridge, _bridge_obj: &Object) {
    let layer = bridge.get_layer();
    let ids = OBJECT_REGISTRY.get_all_object_ids();
    for id in ids {
        let Some(obj_arc) = TheGameLogic::find_object_by_id(id) else {
            continue;
        };
        let Ok(mut obj) = obj_arc.write() else {
            continue;
        };
        if !layers_match(obj.get_layer(), layer) {
            continue;
        }
        // C++ `considerBridgeHealth = false` — the bridge is already rubble.
        if !object_on_this_bridge(bridge, &obj) {
            continue;
        }
        let mut extra = DamageInfo::with_simple(
            HUGE_DAMAGE_AMOUNT,
            obj.get_id(),
            DamageType::Falling,
            DeathType::Splatted,
        );
        let _ = obj.attempt_damage(&mut extra);
    }
}

fn layers_match(obj_layer: CommonLayer, bridge_layer: PathfindLayerEnum) -> bool {
    obj_layer as u8 == bridge_layer as u8
}

/// Subset of `TerrainLogic::objectInteractsWithBridgeLayer` for this bridge
/// with `considerBridgeHealth = false`.
fn object_on_this_bridge(bridge: &Bridge, obj: &Object) -> bool {
    let mut matches = bridge.is_point_on_bridge(obj.get_position());
    let mut radius = obj.get_geometry_info().get_minor_radius();
    radius += PATHFIND_CELL_SIZE_F * 0.5;
    let mut bounds = Region2D::default();
    bounds.lo.x = obj.get_position().x - radius;
    bounds.lo.y = obj.get_position().y - radius;
    bounds.hi.x = obj.get_position().x + radius;
    bounds.hi.y = obj.get_position().y + radius;
    if bridge.is_cell_on_end(&bounds) {
        matches = true;
    }
    if !matches {
        return false;
    }
    let bridge_height = bridge.get_bridge_height(obj.get_position(), None);
    let delta = (obj.get_position().z - bridge_height).abs();
    delta <= LAYER_Z_CLOSE_ENOUGH_F
}
