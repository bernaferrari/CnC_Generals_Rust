//! ID-first special-power owner resolution.
//!
//! Call sites use `with_special_power_owner` so the registry checkout
//! never escapes as an `Arc`.

use crate::common::types::{INVALID_ID, Int, ObjectID};
use crate::object::registry::OBJECT_REGISTRY;
use crate::player::player_list;

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
        if OBJECT_REGISTRY.with_object(owner_object_id, |_| ()).is_some() {
            return Some(owner_object_id);
        }
    }

    let player_id = owner_player_id?;
    let owned = crate::player::with_player(player_id as crate::player::PlayerIndex, |player_guard| {
        player_guard.get_all_objects()
    })?;

    for object_id in owned {
        if OBJECT_REGISTRY.with_object(object_id, |_| ()).is_some() {
            return Some(object_id);
        }
    }
    None
}

/// Run `f` against the resolved owning object. Wave 433: empty dual-world → None.
pub fn with_special_power_owner<R>(
    owner_object_id: ObjectID,
    owner_player_id: Option<ObjectID>,
    f: impl FnOnce(&crate::object::Object) -> R,
) -> Option<R> {
    if dual_world_registry_unavailable() {
        return None;
    }
    let id = resolve_special_power_owner_id(owner_object_id, owner_player_id)?;
    OBJECT_REGISTRY.with_object(id, f)
}

/// Mutable checkout of the resolved owning object.
pub fn with_special_power_owner_mut<R>(
    owner_object_id: ObjectID,
    owner_player_id: Option<ObjectID>,
    f: impl FnOnce(&mut crate::object::Object) -> R,
) -> Option<R> {
    if dual_world_registry_unavailable() {
        return None;
    }
    let id = resolve_special_power_owner_id(owner_object_id, owner_player_id)?;
    OBJECT_REGISTRY.with_object_mut(id, f)
}
