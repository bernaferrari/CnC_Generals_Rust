//! Shared Overlord / Helix rider draw path.
//!
//! C++ `W3DOverlordTankDraw::doDrawModule` / `setHidden` (and the truck/aircraft
//! twins) look up `contain->friend_getRider()` on the live object, copy the
//! carrier tint envelope, clear the rider's dependency latch, and draw it.

use crate::common::*;
use crate::helpers::TheGameLogic;
use crate::object::drawable::{Drawable, DrawableArcExt};
use crate::object::registry::OBJECT_REGISTRY;
use std::sync::{Arc, RwLock};

fn find_object(id: ObjectID) -> Option<Arc<RwLock<crate::object::Object>>> {
    TheGameLogic::find_object_by_id(id).or_else(|| OBJECT_REGISTRY.get_object(id))
}

fn rider_id_of(owner_id: ObjectID) -> Option<ObjectID> {
    let owner = find_object(owner_id)?;
    let owner_guard = owner.read().ok()?;
    owner_guard
        .get_contain()
        .and_then(|contain| contain.lock().ok().and_then(|cg| cg.friend_get_rider()))
}

fn drawable_of(object_id: ObjectID) -> Option<Arc<RwLock<Drawable>>> {
    let object = find_object(object_id)?;
    let guard = object.read().ok()?;
    guard.get_drawable()
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
    let Ok(owner_guard) = owner_drawable.read() else {
        return;
    };
    let Ok(mut rider_guard) = rider_drawable.write() else {
        return;
    };
    rider_guard.copy_color_tint_envelope_from(&owner_guard);
    rider_guard.notify_drawable_dependency_cleared();
    rider_guard.draw(None);
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
