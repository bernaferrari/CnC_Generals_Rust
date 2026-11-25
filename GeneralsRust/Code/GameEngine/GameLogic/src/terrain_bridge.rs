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
use std::sync::{Arc, RwLock};

pub(crate) enum BridgeDamage {
    Collapsed,
    Reopened,
}

/// C++ `Bridge::updateDamageState`.
pub fn update_damage_state(bridge: &mut Bridge) {
    let id = bridge.get_bridge_info().bridge_object_id;
    let actor = TheGameLogic::find_object_by_id(id);
    if actor.is_none() && OBJECT_REGISTRY.store_is_empty() {
        bridge.bridge_info_mut().damage_state_changed = false;
        return;
    }
    match observe_damage(bridge, actor.as_ref()) {
        Some(BridgeDamage::Collapsed) => {
            change_bridge_state(bridge.get_layer(), false);
            for id in OBJECT_REGISTRY.get_all_object_ids() {
                if let Some(actor) = TheGameLogic::find_object_by_id(id) {
                    splat_unit(bridge, &actor);
                }
            }
        }
        Some(BridgeDamage::Reopened) => change_bridge_state(bridge.get_layer(), true),
        None => {}
    }
}

/// Observe the bound actor before dispatching effects. The owner can release
/// its terrain borrow before applying falling damage.
pub(crate) fn observe_damage(
    bridge: &mut Bridge,
    actor: Option<&Arc<RwLock<Object>>>,
) -> Option<BridgeDamage> {
    let info = bridge.bridge_info_mut();
    info.damage_state_changed = false;
    if info.bridge_object_id == crate::common::INVALID_ID {
        return None;
    }
    let Some(actor) = actor else {
        info.bridge_object_id = crate::common::INVALID_ID;
        log::error!("Bridge object disappeared from its world");
        return None;
    };
    let actor = actor.read().ok()?;
    let body = actor.get_body_module()?;
    let state = body.lock().ok()?.get_damage_state();
    let previous = info.cur_damage_state;
    if previous == state {
        return None;
    }

    info.cur_damage_state = state;
    info.damage_state_changed = previous == BodyDamageType::Rubble || state == BodyDamageType::Rubble;
    if state == BodyDamageType::Rubble {
        Some(BridgeDamage::Collapsed)
    } else if previous == BodyDamageType::Rubble && !bridge_has_scaffold(&actor) {
        Some(BridgeDamage::Reopened)
    } else {
        None
    }
}

fn change_bridge_state(layer: PathfindLayerEnum, repaired: bool) {
    let ai_store = the_ai();
    if let Ok(mut ai) = ai_store.write() {
        ai.change_bridge_state(layer, repaired);
    }
}

fn bridge_has_scaffold(bridge_obj: &Object) -> bool {
    for mut module in bridge_obj.get_behavior_modules() {
        let Ok(mut guard) = module.access() else {
            continue;
        };
        if let Some(bbi) = guard.get_bridge_behavior_interface() {
            return bbi.is_scaffold_present();
        }
    }
    false
}

pub(crate) fn splat_unit(bridge: &Bridge, actor: &Arc<RwLock<Object>>) {
    let Ok(mut obj) = actor.write() else {
        return;
    };
    // C++ `considerBridgeHealth = false` — the bridge is already rubble.
    if !layers_match(obj.get_layer(), bridge.get_layer()) || !object_on_this_bridge(bridge, &obj) {
        return;
    }
    let mut damage = DamageInfo::with_simple(
        HUGE_DAMAGE_AMOUNT,
        obj.get_id(),
        DamageType::Falling,
        DeathType::Splatted,
    );
    let _ = obj.attempt_damage(&mut damage);
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
