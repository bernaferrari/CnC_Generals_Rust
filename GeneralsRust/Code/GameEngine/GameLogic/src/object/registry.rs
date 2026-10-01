//! Global object registry used by legacy gameplay systems.
//!
//! The original C++ code relied on a singleton registry to look up objects
//! quickly from behaviours that only had an `ObjectID`.  The modern port keeps
//! the interface so the remaining legacy modules (crate collide logic, factory
//! helpers, etc.) can continue to function while the ownership model migrates
//! towards explicit handles.
//!
//! This registry is the sole owner of `Object` values. Callers borrow through
//! `with_object` / `with_object_mut`, which check the value out of the map and
//! drop the map lock before the callback runs.

use crate::common::ObjectID;
use crate::object::Object;
use crate::scripting::engine::get_script_engine;
use std::collections::HashMap;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{LazyLock, RwLock};

/// Internal storage for the registry.
#[derive(Default)]
struct RegistryStore {
    /// Owned objects. The registry is the ID→Object authority until unregister/destroy.
    objects: HashMap<ObjectID, Object>,
}

/// Public façade matching the legacy `ObjectRegistry` API.
#[derive(Default)]
pub struct ObjectRegistry {
    store: RwLock<RegistryStore>,
    /// Wave 247: lock-free empty short-circuit for host/presentation path.
    /// Kept in sync under the same write lock as map mutations.
    live_count: AtomicUsize,
}

/// Puts a checked-out object back when `with_object*` returns or unwinds.
/// The map lock is never held across the callback, and `Object` is not dropped
/// while that lock is held.
struct CheckedOut<'a> {
    registry: &'a ObjectRegistry,
    id: ObjectID,
    object: Option<Object>,
}

impl Drop for CheckedOut<'_> {
    fn drop(&mut self) {
        if let Some(object) = self.object.take() {
            self.registry.reinsert(self.id, object);
        }
    }
}

impl ObjectRegistry {
    #[inline]
    fn set_live_count(&self, n: usize) {
        self.live_count.store(n, Ordering::Release);
    }

    /// Insert under the write lock. The displaced object is dropped only after
    /// the guard is released (`Object::drop` re-enters the registry).
    fn insert_replacing(&self, id: ObjectID, object: Object) -> Option<Object> {
        if let Ok(mut guard) = self.store.write() {
            let displaced = guard.objects.insert(id, object);
            self.set_live_count(guard.objects.len());
            displaced
        } else {
            // Poisoned: we are not holding the lock, so dropping here is safe.
            drop(object);
            None
        }
    }

    fn reinsert(&self, id: ObjectID, object: Object) {
        let displaced = self.insert_replacing(id, object);
        drop(displaced);
    }

    /// Remove `id` without touching `live_count`. The id is absent until reinsert,
    /// but an empty live_count would make `is_empty` hide every other id.
    fn checkout(&self, id: ObjectID) -> Option<Object> {
        let mut guard = self.store.write().ok()?;
        guard.objects.remove(&id)
    }

    /// Register a live object. The registry takes ownership.
    pub fn register_object(&self, id: ObjectID, object: Object) {
        let displaced = self.insert_replacing(id, object);
        // Object::drop can query the registry through pathfinder cleanup.
        // Never run it while this registry's write lock is held.
        drop(displaced);
    }

    /// Remove an object from the registry and drop it after the write lock.
    pub fn unregister_object(&self, id: ObjectID) {
        let removed = if let Ok(mut guard) = self.store.write() {
            let removed = guard.objects.remove(&id);
            self.set_live_count(guard.objects.len());
            removed
        } else {
            None
        };
        // Preserve the old callback order: destruction completes before the
        // script engine's attack-priority entry is cleared below.
        drop(removed);
        if let Ok(mut engine_guard) = get_script_engine().try_write() {
            if let Some(engine) = engine_guard.as_mut() {
                engine.clear_object_attack_priority_set(id);
            }
        }
    }

    /// True when `id` is currently registered.
    ///
    /// Same-id re-entry from a `with_object*` callback is false: that object is
    /// checked out. Other ids still resolve.
    pub fn contains(&self, id: ObjectID) -> bool {
        // Wave 247: host path (empty registry) skips RwLock entirely.
        if self.is_empty() {
            return false;
        }
        if let Ok(guard) = self.store.read() {
            if guard.objects.contains_key(&id) {
                return true;
            }
        }
        // C++ GameLogic.objects is an id roster. A store miss can still be a
        // logic id while that roster is being moved onto this registry.
        match crate::system::game_logic::get_game_logic().try_lock() {
            Ok(logic) => logic.find_object_by_id(id).is_some(),
            // Lock-held ≠ missing (C++ GameLogic.h:386-397): the global
            // GameLogic mutex is held across the entire update, so this arm
            // is "store miss + update in flight". Surface it instead of a
            // silent false.
            Err(_) => {
                log::warn!(
                    "OBJECT_REGISTRY::contains({id}): GameLogic lock held; reporting false on store miss — lock-held is not missing (C++ GameLogic.h:386-397)"
                );
                false
            }
        }
    }

    /// Borrow an owned object. The map lock is dropped before `f` runs.
    ///
    /// The object is absent for the duration of `f`, so a nested lookup of the
    /// same id returns `None`. Other ids still resolve. The value is reinserted
    /// after `f` returns (and if `f` unwinds).
    pub fn with_object<R>(&self, id: ObjectID, f: impl FnOnce(&Object) -> R) -> Option<R> {
        self.with_object_mut(id, |obj| f(obj))
    }

    /// Mutable borrow. Same checkout protocol as [`with_object`].
    pub fn with_object_mut<R>(&self, id: ObjectID, f: impl FnOnce(&mut Object) -> R) -> Option<R> {
        let object = self.checkout(id)?;
        let mut checked = CheckedOut {
            registry: self,
            id,
            object: Some(object),
        };
        let obj = checked.object.as_mut()?;
        Some(f(obj))
    }

    /// Iterate owned objects. Each callback runs without the map lock held.
    pub fn with_each(&self, mut f: impl FnMut(ObjectID, &Object)) {
        let mut ids = self.owned_ids();
        ids.sort_unstable();
        for id in ids {
            let _ = self.with_object(id, |obj| f(id, obj));
        }
    }

    /// Mutable iteration. Each callback runs without the map lock held.
    pub fn with_each_mut(&self, mut f: impl FnMut(ObjectID, &mut Object)) {
        let mut ids = self.owned_ids();
        ids.sort_unstable();
        for id in ids {
            let _ = self.with_object_mut(id, |obj| f(id, obj));
        }
    }

    fn owned_ids(&self) -> Vec<ObjectID> {
        if self.live_count.load(Ordering::Acquire) == 0 {
            return Vec::new();
        }
        if let Ok(guard) = self.store.read() {
            guard.objects.keys().copied().collect()
        } else {
            Vec::new()
        }
    }

    /// Host/presentation path: true when no dual-world factory objects are registered.
    ///
    /// Wave 247: lock-free via `live_count` (updated under write lock).
    #[inline]
    pub fn is_empty(&self) -> bool {
        if self.live_count.load(Ordering::Acquire) != 0 {
            return false;
        }
        // C++ GameLogic.objects is the authority: empty registry + live logic
        // objects is not an empty world.
        match crate::system::game_logic::get_game_logic().try_lock() {
            Ok(logic) => logic.get_object_count() == 0,
            Err(_) => false,
        }
    }

    /// True when the factory registry **store** has no handles.
    ///
    /// Unlike [`is_empty`], this does **not** consult GameLogic and does **not**
    /// fail-open when the GameLogic mutex is already held. Used only by the
    /// full `GameLogic::update()` empty-noop path. Do **not** use this as a
    /// blanket skip on terrain/pathfind APIs (`dual_world_registry_unavailable`
    /// / [`is_empty`] stay fail-open under lock for those).
    #[inline]
    pub fn store_is_empty(&self) -> bool {
        self.live_count.load(Ordering::Acquire) == 0
    }

    /// Object IDs currently registered.
    pub fn get_all_object_ids(&self) -> Vec<ObjectID> {
        // Wave 247: host path short-circuit when both registry and GameLogic empty.
        if self.live_count.load(Ordering::Acquire) == 0 {
            return match crate::system::game_logic::get_game_logic().try_lock() {
                Ok(logic) => logic.get_all_object_ids().to_vec(),
                // Lock-held ≠ missing (C++ GameLogic.h:386-397): the global
                // GameLogic mutex is held across the entire update, so an
                // empty-looking store mid-update can still have live
                // GameLogic objects. Surface it instead of a silent empty
                // vec.
                Err(_) => {
                    log::warn!(
                        "OBJECT_REGISTRY::get_all_object_ids: GameLogic lock held; returning empty on empty store — lock-held is not missing (C++ GameLogic.h:386-397)"
                    );
                    Vec::new()
                }
            };
        }
        if let Ok(guard) = self.store.read() {
            guard.objects.keys().copied().collect()
        } else {
            Vec::new()
        }
    }

    /// Clear all registered objects, dropping them after the write lock.
    pub fn clear(&self) {
        let removed = if let Ok(mut guard) = self.store.write() {
            let removed = std::mem::take(&mut guard.objects);
            self.set_live_count(0);
            removed
        } else {
            HashMap::new()
        };
        // Destructors may re-enter ObjectRegistry while unwinding their
        // world-side registrations, so release the lock before dropping them.
        drop(removed);
    }

    /// Remove dead weak references from the registry.
    ///
    /// No-op with owned storage (kept for call-site compatibility).
    ///
    /// Returns the number of entries that were removed.
    pub fn cleanup_dead_references(&self) -> usize {
        0
    }
}

/// Global instance mirroring the legacy singleton.
pub static OBJECT_REGISTRY: LazyLock<ObjectRegistry> = LazyLock::new(ObjectRegistry::default);

/// Process-wide mutex for tests that clear/register objects on the shared
/// [`OBJECT_REGISTRY`] / GameLogic singleton. Parallel weapon collision tests
/// otherwise clobber each other mid-assertion.
pub fn test_isolation_lock() -> &'static std::sync::Mutex<()> {
    static LOCK: LazyLock<std::sync::Mutex<()>> = LazyLock::new(|| std::sync::Mutex::new(()));
    &LOCK
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::common::{DefaultThingTemplate, ObjectStatusMaskType};
    use crate::object::Object;
    use std::sync::Arc;

    fn crate_test_object(id: ObjectID) -> Object {
        let template = Arc::new(DefaultThingTemplate::new(format!("CrateBind{id}")));
        Object::new_raw(template, id, ObjectStatusMaskType::none(), None)
    }

    #[test]
    fn register_object_moves_owned_value_into_store() {
        let _lock = test_isolation_lock().lock().unwrap();
        let id = 0xC0_FF_EE;
        OBJECT_REGISTRY.clear();
        assert!(
            OBJECT_REGISTRY.store_is_empty(),
            "cleared store must start empty"
        );

        OBJECT_REGISTRY.register_object(id, crate_test_object(id));

        assert!(
            !OBJECT_REGISTRY.store_is_empty(),
            "register_object must fill OBJECT_REGISTRY store"
        );
        assert!(!OBJECT_REGISTRY.is_empty());
        let found = OBJECT_REGISTRY.with_object(id, |obj| obj.get_id());
        assert_eq!(found, Some(id));

        OBJECT_REGISTRY.unregister_object(id);
        assert!(OBJECT_REGISTRY.with_object(id, |_| ()).is_none());
        OBJECT_REGISTRY.clear();
    }

    #[test]
    fn with_object_checks_out_same_id_and_keeps_other_ids() {
        let _lock = test_isolation_lock().lock().unwrap();
        let id = 0xC0_11_EC;
        let other = 0xC0_11_ED;
        OBJECT_REGISTRY.clear();
        OBJECT_REGISTRY.register_object(id, crate_test_object(id));
        OBJECT_REGISTRY.register_object(other, crate_test_object(other));

        let nested = OBJECT_REGISTRY.with_object(id, |_| {
            OBJECT_REGISTRY.with_object(id, |obj| obj.get_id())
        });
        assert_eq!(nested, Some(None), "same-id re-entry must miss");

        let seen = OBJECT_REGISTRY.with_object(id, |_| {
            OBJECT_REGISTRY.with_object(other, |obj| obj.get_id())
        });
        assert_eq!(seen, Some(Some(other)));

        let mut seen_ids = Vec::new();
        OBJECT_REGISTRY.with_each(|oid, obj| {
            assert_eq!(oid, obj.get_id());
            seen_ids.push(oid);
        });
        assert_eq!(seen_ids, vec![id, other]);

        OBJECT_REGISTRY.clear();
    }
}
