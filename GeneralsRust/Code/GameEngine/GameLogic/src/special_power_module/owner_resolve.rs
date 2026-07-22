//! ID-first special-power owner resolution.
//!
//! Prefer `resolve_special_power_owner_id` and only materialize an Arc for the
//! duration of a call site that still needs a handle.

use crate::common::types::{Int, ObjectID, INVALID_ID};
use crate::object::registry::OBJECT_REGISTRY;
use crate::player::player_list;
use std::sync::{Arc, RwLock};

/// Resolve the owning object id for a special power.
pub fn resolve_special_power_owner_id(
    owner_object_id: ObjectID,
    owner_player_id: Option<ObjectID>,
) -> Option<ObjectID> {
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
    resolve_special_power_owner_id(owner_object_id, owner_player_id)
        .and_then(|id| OBJECT_REGISTRY.get_object(id))
}
