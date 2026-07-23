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
use std::sync::{Arc, RwLock};

/// Internal storage for the registry.
#[derive(Default)]
struct RegistryStore {
    /// Strong handles: registry is the ID→Object authority until unregister/destroy.
    objects: HashMap<ObjectID, Arc<RwLock<Object>>>,
}

impl RegistryStore {
    fn register(&mut self, id: ObjectID, object: &Arc<RwLock<Object>>) {
        self.objects.insert(id, Arc::clone(object));
    }

    fn unregister(&mut self, id: ObjectID) {
        self.objects.remove(&id);
    }

    fn get(&self, id: ObjectID) -> Option<Arc<RwLock<Object>>> {
        self.objects.get(&id).cloned()
    }

    fn contains(&self, id: ObjectID) -> bool {
        self.objects.contains_key(&id)
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
        if let Ok(guard) = self.store.read() {
            guard.get(id)
        } else {
            None
        }
    }

    /// True when `id` is currently registered (no Arc clone).
    pub fn contains(&self, id: ObjectID) -> bool {
        if let Ok(guard) = self.store.read() {
            guard.contains(id)
        } else {
            false
        }
    }

    /// Borrow-first object access without keeping an Arc at the call site.
    /// Prefer this over `get_object(id).read()` when the registry handle need
    /// not outlive the callback. Intermediate step toward retiring Arc stores.
    pub fn with_object<R>(&self, id: ObjectID, f: impl FnOnce(&Object) -> R) -> Option<R> {
        let arc = self.get_object(id)?;
        let guard = arc.read().ok()?;
        Some(f(&guard))
    }

    /// Mutable borrow-first object access without keeping an Arc at the call site.
    pub fn with_object_mut<R>(&self, id: ObjectID, f: impl FnOnce(&mut Object) -> R) -> Option<R> {
        let arc = self.get_object(id)?;
        let mut guard = arc.write().ok()?;
        Some(f(&mut guard))
    }

    /// Host/presentation path: true when no dual-world factory objects are registered.
    pub fn is_empty(&self) -> bool {
        if let Ok(guard) = self.store.read() {
            guard.objects.is_empty()
        } else {
            true
        }
    }

    /// Retrieve all registered objects.
    pub fn get_all_objects(&self) -> Vec<Arc<RwLock<Object>>> {
        if let Ok(guard) = self.store.read() {
            let mut result: Vec<Arc<RwLock<Object>>> = guard.objects.values().cloned().collect();
            result.sort_by_key(|obj| obj.read().map(|o| o.get_id()).unwrap_or(0));
            result
        } else {
            Vec::new()
        }
    }

    /// Object IDs currently registered (no Arc clones).
    pub fn get_all_object_ids(&self) -> Vec<ObjectID> {
        if let Ok(guard) = self.store.read() {
            guard.objects.keys().copied().collect()
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
    /// No-op with strong registry storage (kept for call-site compatibility).
    pub fn cleanup_dead_references(&self) -> usize {
        0
    }
}

/// Global instance mirroring the legacy singleton.
pub static OBJECT_REGISTRY: Lazy<ObjectRegistry> = Lazy::new(ObjectRegistry::default);

/// Process-wide mutex for tests that clear/register objects on the shared
/// [`OBJECT_REGISTRY`] / GameLogic singleton. Parallel weapon collision tests
/// otherwise clobber each other mid-assertion.
pub fn test_isolation_lock() -> &'static std::sync::Mutex<()> {
    use std::sync::{Mutex, OnceLock};
    static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
    LOCK.get_or_init(|| Mutex::new(()))
}
