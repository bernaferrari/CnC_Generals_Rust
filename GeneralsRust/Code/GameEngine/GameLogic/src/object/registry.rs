//! Global object registry used by legacy gameplay systems.
//!
//! The original C++ code relied on a singleton registry to look up objects
//! quickly from behaviours that only had an `ObjectID`.  The modern port keeps
//! the interface so the remaining legacy modules (crate collide logic, factory
//! helpers, etc.) can continue to function while the ownership model migrates
//! towards explicit handles.

use crate::common::ObjectID;
use crate::object::Object;
use crate::scripting::engine::get_script_engine;
use once_cell::sync::Lazy;
use std::collections::HashMap;
use std::sync::{Arc, RwLock, Weak};

/// Internal storage for the registry.
#[derive(Default)]
struct RegistryStore {
    objects: HashMap<ObjectID, Weak<RwLock<Object>>>,
}

impl RegistryStore {
    fn register(&mut self, id: ObjectID, object: &Arc<RwLock<Object>>) {
        self.objects.insert(id, Arc::downgrade(object));
    }

    fn unregister(&mut self, id: ObjectID) {
        self.objects.remove(&id);
    }

    fn get(&mut self, id: ObjectID) -> Option<Arc<RwLock<Object>>> {
        if let Some(entry) = self.objects.get(&id) {
            if let Some(obj) = entry.upgrade() {
                return Some(obj);
            }
        }
        // Drop dead weak references so the map cannot grow unbounded.
        self.objects.retain(|_, handle| handle.strong_count() > 0);
        None
    }

    fn clear(&mut self) {
        self.objects.clear();
    }
}

/// Public façade matching the legacy `ObjectRegistry` API.
#[derive(Default)]
pub struct ObjectRegistry {
    store: RwLock<RegistryStore>,
}

impl ObjectRegistry {
    /// Register a live object handle.
    pub fn register_object(&self, id: ObjectID, object: &Arc<RwLock<Object>>) {
        if let Ok(mut guard) = self.store.write() {
            guard.register(id, object);
        }
    }

    /// Remove a handle from the registry.
    pub fn unregister_object(&self, id: ObjectID) {
        if let Ok(mut guard) = self.store.write() {
            guard.unregister(id);
        }
        if let Ok(mut engine_guard) = get_script_engine().try_write() {
            if let Some(engine) = engine_guard.as_mut() {
                engine.clear_object_attack_priority_set(id);
            }
        }
    }

    /// Retrieve a strong reference to an object by identifier.
    pub fn get_object(&self, id: ObjectID) -> Option<Arc<RwLock<Object>>> {
        if let Ok(mut guard) = self.store.write() {
            guard.get(id)
        } else {
            None
        }
    }

    /// Retrieve all live objects (dropping stale weak refs on the way).
    pub fn get_all_objects(&self) -> Vec<Arc<RwLock<Object>>> {
        if let Ok(mut guard) = self.store.write() {
            guard.objects.retain(|_, handle| handle.strong_count() > 0);
            guard
                .objects
                .values()
                .filter_map(|weak| weak.upgrade())
                .collect()
        } else {
            Vec::new()
        }
    }

    /// Clear all registered handles.
    pub fn clear(&self) {
        if let Ok(mut guard) = self.store.write() {
            guard.clear();
        }
    }

    /// Remove dead weak references from the registry.
    ///
    /// The registry already drops stale entries opportunistically when `get()`
    /// or `get_all_objects()` is called.  This method allows the game loop to
    /// periodically sweep the table so that objects which are looked up
    /// infrequently (or never) do not accumulate as dead entries.
    ///
    /// Returns the number of entries that were removed.
    pub fn cleanup_dead_references(&self) -> usize {
        if let Ok(mut guard) = self.store.write() {
            let before = guard.objects.len();
            guard
                .objects
                .retain(|_, handle| handle.strong_count() > 0);
            before.saturating_sub(guard.objects.len())
        } else {
            0
        }
    }
}

/// Global instance mirroring the legacy singleton.
pub static OBJECT_REGISTRY: Lazy<ObjectRegistry> = Lazy::new(ObjectRegistry::default);
