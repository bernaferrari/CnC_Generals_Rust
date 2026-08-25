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
use std::sync::atomic::{AtomicUsize, Ordering};
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
    /// Wave 247: lock-free empty short-circuit for host/presentation path.
    /// Kept in sync under the same write lock as map mutations.
    live_count: AtomicUsize,
}

impl ObjectRegistry {
    #[inline]
    fn set_live_count(&self, n: usize) {
        self.live_count.store(n, Ordering::Release);
    }

    /// Register a live object handle.
    pub fn register_object(&self, id: ObjectID, object: &Arc<RwLock<Object>>) {
        if let Ok(mut guard) = self.store.write() {
            guard.register(id, object);
            self.set_live_count(guard.objects.len());
        }
    }

    /// Remove a handle from the registry.
    pub fn unregister_object(&self, id: ObjectID) {
        if let Ok(mut guard) = self.store.write() {
            guard.unregister(id);
            self.set_live_count(guard.objects.len());
        }
        if let Ok(mut engine_guard) = get_script_engine().try_write() {
            if let Some(engine) = engine_guard.as_mut() {
                engine.clear_object_attack_priority_set(id);
            }
        }
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
        // C++ has no dual-world skip: GameLogic.objects is the authority.
        crate::system::game_logic::get_game_logic()
            .try_lock()
            .ok()
            .and_then(|logic| logic.find_object_by_id(id))
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
        crate::system::game_logic::get_game_logic()
            .try_lock()
            .ok()
            .and_then(|logic| logic.find_object_by_id(id))
            .is_some()
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

    /// Retrieve all registered objects.
    pub fn get_all_objects(&self) -> Vec<Arc<RwLock<Object>>> {
        // Wave 247: host path short-circuit when both registry and GameLogic empty.
        if self.live_count.load(Ordering::Acquire) == 0 {
            if let Ok(logic) = crate::system::game_logic::get_game_logic().try_lock() {
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
            return match crate::system::game_logic::get_game_logic().try_lock() {
                Ok(logic) => logic.get_all_object_ids().to_vec(),
                Err(_) => Vec::new(),
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
        if let Ok(mut guard) = self.store.write() {
            guard.clear();
            self.set_live_count(0);
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

    #[test]
    fn object_manager_crate_create_path_calls_bind_crate_object() {
        let src = include_str!("../object_manager.rs");
        let bind_count = src.matches("bind_crate_object(").count();
        assert!(
            bind_count >= 4,
            "crate object_manager new/from_existing/create/register must call bind_crate_object, got {bind_count}"
        );
        let manager_impl = src.find("impl ObjectManager").expect("ObjectManager impl");
        let create_rel = src[manager_impl..]
            .find("pub fn create_object(")
            .expect("ObjectManager::create_object");
        let create_idx = manager_impl + create_rel;
        let create_window = &src[create_idx..src.len().min(create_idx + 2800)];
        assert!(
            create_window.contains("bind_crate_object("),
            "ObjectManager::create_object must bind crate objects"
        );
        assert!(
            !src.contains("gameworld_shadow"),
            "crate object_manager must not be the host create/couple path"
        );
    }
}
