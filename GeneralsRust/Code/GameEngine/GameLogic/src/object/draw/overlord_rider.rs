//! Shared Overlord / Helix rider draw path.
//!
//! C++ `W3DOverlordTankDraw::doDrawModule` / `setHidden` (and the truck/aircraft
//! twins) look up `contain->friend_getRider()` on the live object, copy the
//! carrier tint envelope, clear the rider's dependency latch, and draw it.
//!
//! C++: W3DOverlordTankDraw.cpp:50-85, W3DOverlordTruckDraw.cpp:50-85,
//! W3DOverlordAircraftDraw.cpp:64-106. Helix uses W3DOverlordAircraftDraw.

use crate::common::*;
use crate::drawable::Drawable as DrawableTrait;
use crate::helpers::{TheGameClient, TheGameLogic};
use crate::object::drawable::{Drawable, DrawableArcExt};
use crate::object::registry::OBJECT_REGISTRY;
use std::sync::{Arc, RwLock};

fn find_object(id: ObjectID) -> Option<Arc<RwLock<crate::object::Object>>> {
    TheGameLogic::find_object_by_id(id).or_else(|| OBJECT_REGISTRY.get_object(id))
}

/// C++ OverlordContain::friend_getRider returns `m_containList.front()`.
/// HelixContain::friend_getRider returns `TheGameLogic->findObjectByID(m_portableStructureID)`.
fn rider_id_of(owner_id: ObjectID) -> Option<ObjectID> {
    let owner = find_object(owner_id)?;
    let owner_guard = owner.read().ok()?;
    let contain = owner_guard.get_contain()?;
    let cg = contain.lock().ok()?;
    if let Some(id) = cg.friend_get_rider().filter(|id| *id != INVALID_ID) {
        return Some(id);
    }
    // C++ Overlord: first contained is the portable-structure rider.
    cg.get_contained_objects()
        .iter()
        .copied()
        .find(|id| *id != INVALID_ID)
        .and_then(|id| {
            let rider = find_object(id)?;
            let guard = rider.read().ok()?;
            guard
                .is_kind_of(crate::common::KindOf::PortableStructure)
                .then_some(id)
        })
}

fn drawable_of(object_id: ObjectID) -> Option<Arc<RwLock<Drawable>>> {
    let object = find_object(object_id)?;
    let guard = object.read().ok()?;
    guard.get_drawable()
}

/// After the rider's own `Drawable::draw` commits under the rider object id,
/// fold those W3D records onto the carrier so container draw (the live host
/// / GameClient object_model_draws path) actually presents the rider.
fn republish_rider_on_container(owner_id: ObjectID, rider_id: ObjectID) {
    let Some(client) = TheGameClient::get() else {
        return;
    };
    let rider_draws = client.object_model_draws(rider_id);
    if rider_draws.is_empty() {
        return;
    }
    let logic_drawable_id = drawable_of(owner_id)
        .and_then(|drawable| drawable.read().ok().map(|guard| guard.get_drawable_id()))
        .unwrap_or(0);
    // Commit the carrier body that `do_draw_module` already published, then
    // leave the last rider record active for the outer Drawable::draw commit.
    client.commit_active_object_model_draw(owner_id, logic_drawable_id);
    let last = rider_draws.len() - 1;
    for (index, state) in rider_draws.into_iter().enumerate() {
        client.begin_active_object_model_draw(owner_id, state.source.clone());
        client.set_active_object_model_draw(owner_id, state);
        if index != last {
            client.commit_active_object_model_draw(owner_id, logic_drawable_id);
        }
    }
}

/// C++ `riderDraw->setColorTintEnvelope(*getDrawable()->getColorTintEnvelope())`
/// then `notifyDrawableDependencyCleared()` + `draw(NULL)`.
pub fn draw_overlord_rider(owner_id: ObjectID) {
    let Some(rider_id) = rider_id_of(owner_id) else {
        return;
    };
    let Some(owner_drawable) = drawable_of(owner_id) else {
        return;
    };
    let Some(rider_drawable) = drawable_of(rider_id) else {
        return;
    };

    {
        let Ok(owner_guard) = owner_drawable.read() else {
            return;
        };
        let Ok(mut rider_guard) = rider_drawable.write() else {
            return;
        };
        rider_guard.copy_color_tint_envelope_from(&owner_guard);
        rider_guard.notify_drawable_dependency_cleared();
    }

    // C++ portable-structure riders are not enclosing-hidden. If a prior hide
    // left them hidden while the carrier is drawing, unhide so Drawable::draw
    // reaches W3DDependencyModelDraw.
    if rider_drawable.is_drawable_effectively_hidden() {
        rider_drawable.set_drawable_hidden(false);
    }

    // Drop every drawable lock before draw(). W3DDependencyModelDraw re-enters
    // the rider drawable for stealth / bone attach; holding write here deadlocks
    // std::sync::RwLock and makes the live path a no-op.
    if let Ok(mut rider_guard) = rider_drawable.write() {
        rider_guard.draw(None);
    }

    if let (Ok(owner_guard), Ok(mut rider_guard)) = (owner_drawable.read(), rider_drawable.write())
    {
        rider_guard.set_stealth_look(owner_guard.get_stealth_look());
    }

    republish_rider_on_container(owner_id, rider_id);
}

/// C++ `setHidden` propagates `setDrawableHidden(h)` to the rider.
pub fn set_overlord_rider_hidden(owner_id: ObjectID, hidden: bool) {
    let Some(rider_id) = rider_id_of(owner_id) else {
        return;
    };
    if let Some(drawable) = drawable_of(rider_id) {
        drawable.set_drawable_hidden(hidden);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn missing_owner_is_noop() {
        draw_overlord_rider(INVALID_ID);
        set_overlord_rider_hidden(INVALID_ID, true);
    }
}
