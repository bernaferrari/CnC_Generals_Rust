//! ID-first special-power owner resolution.
//!
//! Prefer `resolve_special_power_owner_id` and only materialize an Arc for the
//! duration of a call site that still needs a handle.

use crate::common::types::{INVALID_ID, Int, ObjectID};
use crate::object::registry::OBJECT_REGISTRY;
use crate::player::player_list;
use std::sync::{Arc, RwLock};

/// Wave 433: host-only path has no dual-world factory objects.
#[inline]
fn dual_world_registry_unavailable() -> bool {
    crate::object::registry::OBJECT_REGISTRY.is_empty()
}

/// Resolve the owning object id for a special power.
pub fn resolve_special_power_owner_id(
    owner_object_id: ObjectID,
    owner_player_id: Option<ObjectID>,
) -> Option<ObjectID> {
    // Wave 433: empty dual-world → None.
    if dual_world_registry_unavailable() {
        return None;
    }

    if owner_object_id != INVALID_ID {
        if OBJECT_REGISTRY.get_object(owner_object_id).is_some() {
            return Some(owner_object_id);
        }
    }

    let player_id = owner_player_id?;
    let list = player_list().read().ok()?;
    let player = list.get_player(player_id as Int).cloned()?;
    let player_guard = player.read().ok()?;
    let owned = player_guard.get_all_objects();
    drop(player_guard);

    for object_id in owned {
        if OBJECT_REGISTRY.get_object(object_id).is_some() {
            return Some(object_id);
        }
    }
    None
}

/// Legacy Arc handle helper. Prefer `resolve_special_power_owner_id` + `with_object`.
pub fn resolve_special_power_owner(
    owner_object_id: ObjectID,
    owner_player_id: Option<ObjectID>,
) -> Option<Arc<RwLock<crate::object::Object>>> {
    // Wave 433: empty dual-world → None.
    if dual_world_registry_unavailable() {
        return None;
    }

    resolve_special_power_owner_id(owner_object_id, owner_player_id)
        .and_then(|id| OBJECT_REGISTRY.get_object(id))
}
