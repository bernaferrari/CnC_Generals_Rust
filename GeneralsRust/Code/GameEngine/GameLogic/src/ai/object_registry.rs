use crate::common::ObjectID;
use crate::object::Object;
use std::collections::HashMap;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, RwLock, Weak};

#[derive(Default)]
struct LegacyObjectRegistry {
    objects: HashMap<ObjectID, Weak<RwLock<Object>>>,
}

impl LegacyObjectRegistry {
    fn register(&mut self, id: ObjectID, object: &Arc<RwLock<Object>>) {
        self.objects.insert(id, Arc::downgrade(object));
    }

    fn unregister(&mut self, id: ObjectID) {
        self.objects.remove(&id);
    }

    /// Read-only upgrade without retaining dead weaks (retain deferred to write path).
    fn get_readonly(&self, id: ObjectID) -> Option<Arc<RwLock<Object>>> {
        self.objects.get(&id).and_then(|entry| entry.upgrade())
    }

    fn get_and_prune(&mut self, id: ObjectID) -> Option<Arc<RwLock<Object>>> {
        if let Some(entry) = self.objects.get(&id) {
            if let Some(obj) = entry.upgrade() {
                return Some(obj);
            }
        }
        self.objects.retain(|_, handle| handle.strong_count() > 0);
        None
    }

    fn clear(&mut self) {
        self.objects.clear();
    }

    fn len(&self) -> usize {
        self.objects.len()
    }
}

pub(crate) struct LegacyObjectRegistryFacade {
    store: RwLock<LegacyObjectRegistry>,
    /// Wave 248: lock-free empty short-circuit for host path (Main does not populate legacy).
    live_count: AtomicUsize,
}

impl Default for LegacyObjectRegistryFacade {
    fn default() -> Self {
        Self {
            store: RwLock::new(LegacyObjectRegistry::default()),
            live_count: AtomicUsize::new(0),
        }
    }
}

impl LegacyObjectRegistryFacade {
    #[inline]
    fn set_live_count(&self, n: usize) {
        self.live_count.store(n, Ordering::Release);
    }

    #[inline]
    fn is_empty(&self) -> bool {
        self.live_count.load(Ordering::Acquire) == 0
    }
}

pub(crate) fn new_legacy_object_registry() -> Arc<LegacyObjectRegistryFacade> {
    Arc::new(LegacyObjectRegistryFacade::default())
}

fn legacy() -> Arc<LegacyObjectRegistryFacade> {
    Arc::clone(
        crate::system::engine_stores::active_services()
            .legacy_object_registry(),
    )
}

pub fn register_legacy_object(object: &Arc<RwLock<Object>>) {
    let registry = legacy();
    if let Ok(mut guard) = registry.store.write() {
        if let Ok(obj_guard) = object.read() {
            guard.register(obj_guard.get_id(), object);
            registry.set_live_count(guard.len());
        }
    }
}

pub fn unregister_legacy_object(object_id: ObjectID) {
    let registry = legacy();
    if let Ok(mut guard) = registry.store.write() {
        guard.unregister(object_id);
        registry.set_live_count(guard.len());
    }
}

/// Wave 248: prefer read lock; host empty path skips locks entirely.
pub fn get_legacy_object(object_id: ObjectID) -> Option<Arc<RwLock<Object>>> {
    let registry = legacy();
    if registry.is_empty() {
        return None;
    }
    // Fast path: read lock + upgrade.
    if let Ok(guard) = registry.store.read() {
        if let Some(obj) = guard.get_readonly(object_id) {
            return Some(obj);
        }
    } else {
        return None;
    }
    // Slow path: dead weak — prune under write lock.
    if let Ok(mut guard) = registry.store.write() {
        let obj = guard.get_and_prune(object_id);
        registry.set_live_count(guard.len());
        obj
    } else {
        None
    }
}

pub fn clear_legacy_objects() {
    let registry = legacy();
    if let Ok(mut guard) = registry.store.write() {
        guard.clear();
        registry.set_live_count(0);
    }
}

/// Wave 248: host/presentation empty probe (lock-free).
pub fn legacy_object_registry_is_empty() -> bool {
    legacy().is_empty()
}
