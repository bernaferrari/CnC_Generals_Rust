//! ID-keyed unit handles for AI resolve.

#![allow(unused_imports)]

use super::identity::Unit;
use super::imports::*;

#[inline]
pub(super) fn dual_world_registry_unavailable() -> bool {
    crate::object::registry::OBJECT_REGISTRY.is_empty()
}

/// ID-keyed unit handles for AI resolve (borrow for the duration of an op).
/// Transitional: factory/tests register Arcs; not a second gameplay authority.
#[derive(Debug)]
#[allow(dead_code)]
pub struct UnitRegistry {
    pub(super) units: std::collections::HashMap<ObjectID, Weak<RwLock<Unit>>>,
}

impl UnitRegistry {
    pub(super) fn register(&mut self, id: ObjectID, unit: &Arc<RwLock<Unit>>) {
        if id != INVALID_ID {
            self.units.insert(id, Arc::downgrade(unit));
        }
    }

    pub(super) fn unregister(&mut self, id: ObjectID) {
        self.units.remove(&id);
    }

    pub(super) fn get(&mut self, id: ObjectID) -> Option<Arc<RwLock<Unit>>> {
        if id == INVALID_ID {
            return None;
        }
        if let Some(weak) = self.units.get(&id) {
            if let Some(arc) = weak.upgrade() {
                return Some(arc);
            }
        }
        self.units.retain(|_, w| w.strong_count() > 0);
        None
    }
}

static UNIT_REGISTRY: Lazy<StdRwLock<UnitRegistry>> = Lazy::new(|| {
    StdRwLock::new(UnitRegistry {
        units: std::collections::HashMap::new(),
    })
});

pub fn register_unit(id: ObjectID, unit: &Arc<RwLock<Unit>>) {
    if let Ok(mut g) = UNIT_REGISTRY.write() {
        g.register(id, unit);
    }
}

pub fn unregister_unit(id: ObjectID) {
    if let Ok(mut g) = UNIT_REGISTRY.write() {
        g.unregister(id);
    }
}

pub(super) fn get_unit_arc(id: ObjectID) -> Option<Arc<RwLock<Unit>>> {
    UNIT_REGISTRY.write().ok().and_then(|mut g| g.get(id))
}

/// Borrow unit by id: factory-owned first, then test/registry Arc.
pub(super) fn with_unit_ref<R>(id: ObjectID, f: impl FnOnce(&Unit) -> R) -> Option<R> {
    if id == INVALID_ID {
        return None;
    }
    if let Ok(factory) = crate::object::object_factory::get_object_factory().read() {
        if let Some(crate::object::object_factory::GameObjectInstance::Unit(unit)) =
            factory.get_object(id)
        {
            return Some(f(unit));
        }
    }
    let arc = get_unit_arc(id)?;
    let guard = arc.read().ok()?;
    Some(f(&guard))
}

pub(super) fn with_unit_mut<R>(id: ObjectID, f: impl FnOnce(&mut Unit) -> R) -> Option<R> {
    if id == INVALID_ID {
        return None;
    }
    if let Ok(mut factory) = crate::object::object_factory::get_object_factory().write() {
        if let Some(crate::object::object_factory::GameObjectInstance::Unit(unit)) =
            factory.get_object_mut(id)
        {
            return Some(f(unit));
        }
    }
    let arc = get_unit_arc(id)?;
    let mut guard = arc.write().ok()?;
    Some(f(&mut guard))
}
