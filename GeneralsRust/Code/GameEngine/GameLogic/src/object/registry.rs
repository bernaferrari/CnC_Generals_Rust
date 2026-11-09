//! Global object registry used by legacy gameplay systems.
//!
//! The original C++ code relied on a singleton registry to look up objects
//! quickly from behaviours that only had an `ObjectID`.  The modern port keeps
//! the interface so the remaining legacy modules (crate collide logic, factory
//! helpers, etc.) can continue to function while the ownership model migrates
//! towards explicit handles.

use crate::common::ObjectID;
use crate::object::Object;
use crate::scripting::engine::{ScriptEngineHandle, WeakScriptEngineHandle};
use std::collections::HashMap;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex, OnceLock, RwLock};

/// Shared handle to a registered object, resolved transiently by id. The
/// registry is the owner; callers must not store these handles (host-only runs
/// have an empty registry, so id-keyed lookups fail closed there — see the
/// Wave 276 gates in `ai/turret.rs`).
pub type SharedObjectHandle = Arc<RwLock<Object>>;

/// Internal storage for the registry.
#[derive(Default)]
struct RegistryStore {
    /// Strong handles: registry is the ID→Object authority until unregister/destroy.
    objects: HashMap<ObjectID, Arc<RwLock<Object>>>,
}

impl RegistryStore {
    fn register(
        &mut self,
        id: ObjectID,
        object: &Arc<RwLock<Object>>,
    ) -> Option<Arc<RwLock<Object>>> {
        self.objects.insert(id, Arc::clone(object))
    }

    fn unregister(&mut self, id: ObjectID) -> Option<Arc<RwLock<Object>>> {
        self.objects.remove(&id)
    }

    fn get(&self, id: ObjectID) -> Option<Arc<RwLock<Object>>> {
        self.objects.get(&id).cloned()
    }

    fn contains(&self, id: ObjectID) -> bool {
        self.objects.contains_key(&id)
    }

    fn clear(&mut self) -> HashMap<ObjectID, Arc<RwLock<Object>>> {
        std::mem::take(&mut self.objects)
    }
}

/// Public façade matching the legacy `ObjectRegistry` API.
#[derive(Default)]
pub struct ObjectRegistry {
    store: RwLock<RegistryStore>,
    /// Wave 247: lock-free empty short-circuit for host/presentation path.
    /// Kept in sync under the same write lock as map mutations.
    live_count: AtomicUsize,
    /// GameLogic of the world that owns this registry (C++ `TheGameLogic` of
    /// that engine). Store misses consult only this logic, never whichever
    /// world is published on the calling thread.
    owning_logic: OnceLock<&'static Mutex<crate::system::game_logic::GameLogic>>,
    /// The owning world's `TheScriptEngine` slot (weak: the engine holds this
    /// registry for cave actions).
    owning_scripts: OnceLock<WeakScriptEngineHandle>,
}

impl ObjectRegistry {
    /// Bind the GameLogic that owns this registry. The first owner wins; a
    /// second, different owner is rejected.
    pub(crate) fn bind_owning_logic(
        &self,
        logic: &'static Mutex<crate::system::game_logic::GameLogic>,
    ) -> Result<(), &'static str> {
        let bound = *self.owning_logic.get_or_init(|| logic);
        if std::ptr::eq(bound, logic) {
            Ok(())
        } else {
            Err("ObjectRegistry already belongs to another GameLogic")
        }
    }

    /// Bind the script engine slot of the world that owns this registry.
    pub(crate) fn bind_owning_scripts(&self, scripts: &ScriptEngineHandle) {
        let _ = self.owning_scripts.set(scripts.downgrade());
    }

    fn owning_logic(&self) -> Option<&'static Mutex<crate::system::game_logic::GameLogic>> {
        self.owning_logic.get().copied()
    }

    /// C++ `TheScriptEngine->removeObjectFromAttackPriority`-style cleanup on
    /// the owning world's engine only.
    fn clear_owned_attack_priority(&self, id: ObjectID) {
        let Some(scripts) = self.owning_scripts.get().and_then(WeakScriptEngineHandle::upgrade) else {
            return;
        };
        if let Ok(mut engine_guard) = scripts.try_write() {
            if let Some(engine) = engine_guard.as_mut() {
                engine.clear_object_attack_priority_set(id);
            }
        }
    }

    #[inline]
    fn set_live_count(&self, n: usize) {
        self.live_count.store(n, Ordering::Release);
    }

    /// Register a live object handle.
    pub fn register_object(&self, id: ObjectID, object: &Arc<RwLock<Object>>) {
        let replaced = if let Ok(mut guard) = self.store.write() {
            let replaced = guard.register(id, object);
            self.set_live_count(guard.objects.len());
            replaced
        } else {
            None
        };
        // Object::drop can query the registry through pathfinder cleanup.
        // Never run it while this registry's write lock is held.
        drop(replaced);
    }

    /// Remove a handle from the registry.
    pub fn unregister_object(&self, id: ObjectID) {
        let removed = if let Ok(mut guard) = self.store.write() {
            let removed = guard.unregister(id);
            self.set_live_count(guard.objects.len());
            removed
        } else {
            None
        };
        // Preserve the old callback order: destruction completes before the
        // script engine's attack-priority entry is cleared below.
        drop(removed);
        self.clear_owned_attack_priority(id);
    }

    /// Retire only the handle owned by the caller. Numeric IDs can repeat in
    /// separate GameLogic instances; their compatibility index is shared until
    /// remaining callers receive an explicit owner.
    pub(crate) fn unregister_object_if_same(
        &self,
        id: ObjectID,
        object: &SharedObjectHandle,
    ) -> bool {
        let removed = if let Ok(mut guard) = self.store.write() {
            if !guard
                .objects
                .get(&id)
                .is_some_and(|stored| Arc::ptr_eq(stored, object))
            {
                return false;
            }
            let removed = guard.unregister(id);
            self.set_live_count(guard.objects.len());
            removed
        } else {
            return false;
        };
        // Destructors can re-enter the registry. Preserve unregister's callback
        // order and do not clear a different handle's script entry on mismatch.
        drop(removed);
        self.clear_owned_attack_priority(id);
        true
    }

    /// Retrieve a strong reference to an object by identifier.
    pub fn get_object(&self, id: ObjectID) -> Option<Arc<RwLock<Object>>> {
        // Wave 247: host path (empty registry) skips RwLock entirely.
        if !self.is_empty() {
            if let Ok(guard) = self.store.read() {
                if let Some(arc) = guard.get(id) {
                    return Some(arc);
                }
            }
        }
        // C++ has no dual-world skip: the owning GameLogic.objects is the authority.
        match self.owning_logic()?.try_lock() {
            Ok(logic) => logic.find_object_by_id(id),
            // C++ findObjectByID (GameLogic.h:386-397) is a plain array
            // index: lock contention NEVER means the object is missing. The
            // global GameLogic mutex is held across the entire update
            // (game_logic_impl/globals.rs update_game_logic), so this arm is
            // "store miss + update in flight". Surface it instead of a
            // silent None; callers that can borrow GameLogic directly
            // (e.g. GameLogic::resolve_damage_and_physics) should use
            // find_object_by_id.
            Err(_) => {
                log::warn!(
                    "OBJECT_REGISTRY::get_object({id}): GameLogic lock held; reporting None on store miss — lock-held is not missing (C++ GameLogic.h:386-397)"
                );
                None
            }
        }
    }
    /// True when `id` is currently registered (no Arc clone).
    pub fn contains(&self, id: ObjectID) -> bool {
        // Wave 247: host path (empty registry) skips RwLock entirely.
        if self.is_empty() {
            return false;
        }
        if let Ok(guard) = self.store.read() {
            if guard.contains(id) {
                return true;
            }
        }
        // C++ GameLogic.objects is the authority when the factory registry is empty.
        let Some(owning_logic) = self.owning_logic() else {
            return false;
        };
        match owning_logic.try_lock() {
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
    ///
    /// Wave 247: lock-free via `live_count` (updated under write lock).
    #[inline]
    pub fn is_empty(&self) -> bool {
        if self.live_count.load(Ordering::Acquire) != 0 {
            return false;
        }
        // C++ GameLogic.objects is the authority: empty registry + live logic
        // objects is not an empty world.
        let Some(owning_logic) = self.owning_logic() else {
            return true;
        };
        match owning_logic.try_lock() {
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

    /// Retrieve all registered objects.
    pub fn get_all_objects(&self) -> Vec<Arc<RwLock<Object>>> {
        // Wave 247: host path short-circuit when both registry and GameLogic empty.
        if self.live_count.load(Ordering::Acquire) == 0 {
            if let Some(Ok(logic)) = self.owning_logic().map(Mutex::try_lock) {
                if logic.get_object_count() == 0 {
                    return Vec::new();
                }
                let mut result: Vec<Arc<RwLock<Object>>> = logic
                    .get_all_object_ids()
                    .iter()
                    .filter_map(|id| logic.find_object_by_id(*id))
                    .collect();
                result.sort_by_key(|obj| obj.read().map(|o| o.get_id()).unwrap_or(0));
                return result;
            }
        }
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
        // Wave 247: host path short-circuit when both registry and GameLogic empty.
        if self.live_count.load(Ordering::Acquire) == 0 {
            let Some(owning_logic) = self.owning_logic() else {
                return Vec::new();
            };
            return match owning_logic.try_lock() {
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

    /// Clear all registered handles.
    pub fn clear(&self) {
        let removed = if let Ok(mut guard) = self.store.write() {
            let removed = guard.clear();
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

/// Call-site-compatible proxy; resolves the published world's registry.
#[derive(Clone, Copy, Debug, Default)]
pub struct ObjectRegistryGlobal;

pub static OBJECT_REGISTRY: ObjectRegistryGlobal = ObjectRegistryGlobal;

impl ObjectRegistryGlobal {
    fn try_get(self) -> Option<std::sync::Arc<ObjectRegistry>> {
        crate::system::engine_stores::try_active_services()
            .map(|services| std::sync::Arc::clone(services.object_registry()))
    }

    fn get(self) -> std::sync::Arc<ObjectRegistry> {
        self.try_get().unwrap_or_else(|| {
            panic!(
                "ObjectRegistry requires with_world_services / install_services (no process bag)"
            )
        })
    }

    pub fn register_object(self, id: ObjectID, object: &Arc<RwLock<Object>>) {
        self.get().register_object(id, object)
    }

    pub fn unregister_object(self, id: ObjectID) {
        self.get().unregister_object(id)
    }

    pub(crate) fn unregister_object_if_same(
        self,
        id: ObjectID,
        object: &SharedObjectHandle,
    ) -> bool {
        self.get().unregister_object_if_same(id, object)
    }

    pub fn get_object(self, id: ObjectID) -> Option<Arc<RwLock<Object>>> {
        self.try_get()?.get_object(id)
    }

    pub fn contains(self, id: ObjectID) -> bool {
        self.try_get().is_some_and(|registry| registry.contains(id))
    }

    pub fn with_object<R>(self, id: ObjectID, f: impl FnOnce(&Object) -> R) -> Option<R> {
        self.get().with_object(id, f)
    }

    pub fn with_object_mut<R>(self, id: ObjectID, f: impl FnOnce(&mut Object) -> R) -> Option<R> {
        self.get().with_object_mut(id, f)
    }

    pub fn is_empty(self) -> bool {
        // Host matches never admit crate Objects; unbound ⇒ empty (fail closed).
        self.try_get().map(|registry| registry.is_empty()).unwrap_or(true)
    }

    pub fn store_is_empty(self) -> bool {
        self.try_get()
            .map(|registry| registry.store_is_empty())
            .unwrap_or(true)
    }

    pub fn get_all_objects(self) -> Vec<Arc<RwLock<Object>>> {
        self.get().get_all_objects()
    }

    pub fn get_all_object_ids(self) -> Vec<ObjectID> {
        self.get().get_all_object_ids()
    }

    pub fn clear(self) {
        self.get().clear()
    }

    pub fn cleanup_dead_references(self) -> usize {
        self.get().cleanup_dead_references()
    }
}

/// Process-wide mutex for tests that clear/register objects on the shared
/// [`OBJECT_REGISTRY`] / GameLogic singleton. Parallel weapon collision tests
/// otherwise clobber each other mid-assertion.
#[cfg(test)]
pub fn test_isolation_lock() -> &'static std::sync::Mutex<()> {
    test_isolation_lock_cell::LOCK.get_or_init(|| std::sync::Mutex::new(()))
}

#[cfg(test)]
mod test_isolation_lock_cell {
    use std::sync::{Mutex, OnceLock};
    pub(super) static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::common::{DefaultThingTemplate, ObjectStatusMaskType};
    use crate::object::Object;
    use crate::object::crate_registry_bind::bind_crate_object;
    use std::sync::{Arc, RwLock};

    fn crate_test_object(id: ObjectID) -> Arc<RwLock<Object>> {
        let template = Arc::new(DefaultThingTemplate::new(format!("CrateBind{id}")));
        Arc::new(RwLock::new(Object::new_raw(
            template,
            id,
            ObjectStatusMaskType::none(),
            None,
        )))
    }

    #[test]
    fn identity_checked_retirement_preserves_replacement_and_count() {
        let _lock = test_isolation_lock().lock().unwrap();
        let registry = ObjectRegistry::default();
        let original = crate_test_object(77);
        let replacement = crate_test_object(77);
        registry.register_object(77, &original);
        registry.register_object(77, &replacement);

        assert!(!registry.unregister_object_if_same(77, &original));
        assert_eq!(registry.live_count.load(Ordering::Acquire), 1);
        assert!(Arc::ptr_eq(
            &registry.store.read().unwrap().get(77).unwrap(),
            &replacement
        ));
        assert!(registry.unregister_object_if_same(77, &replacement));
        assert!(registry.store_is_empty());
        assert!(registry.store.read().unwrap().get(77).is_none());
        assert!(!registry.unregister_object_if_same(77, &replacement));
    }

    #[test]
    fn bind_crate_object_fills_object_registry_store() {
        let _lock = test_isolation_lock().lock().unwrap();
        let id = 0xC0_FF_EE;
        let object = crate_test_object(id);
        OBJECT_REGISTRY.clear();
        assert!(
            OBJECT_REGISTRY.store_is_empty(),
            "cleared store must start empty"
        );

        bind_crate_object(id, &object);

        assert!(
            !OBJECT_REGISTRY.store_is_empty(),
            "crate bind must fill OBJECT_REGISTRY store"
        );
        assert!(!OBJECT_REGISTRY.is_empty());
        let found = OBJECT_REGISTRY.with_object(id, |obj| obj.get_id());
        assert_eq!(found, Some(id));

        OBJECT_REGISTRY.unregister_object(id);
        OBJECT_REGISTRY.clear();
    }

    #[test]
    fn crate_object_manager_create_registers_in_object_registry() {
        let _lock = test_isolation_lock().lock().unwrap();
        let id = 0xC0_11_EC;
        let object = crate_test_object(id);
        OBJECT_REGISTRY.clear();

        // Same helper crate object_manager new/from_existing/create/register call.
        bind_crate_object(id, &object);

        assert!(
            !OBJECT_REGISTRY.is_empty(),
            "crate object_manager create must fill OBJECT_REGISTRY"
        );
        assert!(!OBJECT_REGISTRY.store_is_empty());
        let found = OBJECT_REGISTRY.with_object(id, |obj| obj.get_id());
        assert_eq!(found, Some(id));

        OBJECT_REGISTRY.unregister_object(id);
        OBJECT_REGISTRY.clear();
    }
}
