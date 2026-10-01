use crate::common::ObjectID;
use crate::object::Object;
use crate::object::registry::OBJECT_REGISTRY;
use std::collections::HashSet;
use std::sync::LazyLock;
use std::sync::RwLock;
use std::sync::atomic::{AtomicUsize, Ordering};

/// Legacy AI object map. Live objects live in `OBJECT_REGISTRY`; this set only
/// records which ids the legacy AI path registered. Resolution always goes
/// through `OBJECT_REGISTRY.with_object` / `with_object_mut`, which check the
/// object out and drop the registry lock before the callback.
#[derive(Default)]
struct LegacyObjectRegistry {
    objects: HashSet<ObjectID>,
}

impl LegacyObjectRegistry {
    fn register(&mut self, id: ObjectID) {
        self.objects.insert(id);
    }

    fn unregister(&mut self, id: ObjectID) {
        self.objects.remove(&id);
    }

    fn contains(&self, id: ObjectID) -> bool {
        self.objects.contains(&id)
    }

    fn clear(&mut self) {
        self.objects.clear();
    }

    fn len(&self) -> usize {
        self.objects.len()
    }
}

struct LegacyObjectRegistryFacade {
    store: RwLock<LegacyObjectRegistry>,
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
    fn is_empty(&self) -> bool {
        self.live_count.load(Ordering::Acquire) == 0
    }

    fn set_live_count(&self, n: usize) {
        self.live_count.store(n, Ordering::Release);
    }
}

static LEGACY_OBJECT_REGISTRY: LazyLock<LegacyObjectRegistryFacade> =
    LazyLock::new(LegacyObjectRegistryFacade::default);

/// Register an id already present in `OBJECT_REGISTRY`. Does not retain an Arc.
pub fn register_legacy_object(object_id: ObjectID) {
    if let Ok(mut guard) = LEGACY_OBJECT_REGISTRY.store.write() {
        guard.register(object_id);
        LEGACY_OBJECT_REGISTRY.set_live_count(guard.len());
    }
}

pub fn unregister_legacy_object(object_id: ObjectID) {
    if let Ok(mut guard) = LEGACY_OBJECT_REGISTRY.store.write() {
        guard.unregister(object_id);
        LEGACY_OBJECT_REGISTRY.set_live_count(guard.len());
    }
}

fn legacy_contains(object_id: ObjectID) -> bool {
    if LEGACY_OBJECT_REGISTRY.is_empty() {
        return false;
    }
    LEGACY_OBJECT_REGISTRY
        .store
        .read()
        .map(|guard| guard.contains(object_id))
        .unwrap_or(false)
}

/// Wave 248: prefer read lock; host empty path skips locks entirely.
/// Returns the id when the legacy set still tracks it and the live registry can resolve it.
/// Does not return an object handle.
pub fn get_legacy_object(object_id: ObjectID) -> Option<ObjectID> {
    if !legacy_contains(object_id) {
        return None;
    }
    OBJECT_REGISTRY
        .with_object(object_id, |_| ())
        .map(|_| object_id)
}

/// Resolve a legacy-registered object without holding the registry lock inside `f`.
/// Same-id re-entry of `OBJECT_REGISTRY` inside `f` returns None.
pub fn with_legacy_object<R>(object_id: ObjectID, f: impl FnOnce(&Object) -> R) -> Option<R> {
    if !legacy_contains(object_id) {
        return None;
    }
    OBJECT_REGISTRY.with_object(object_id, f)
}

pub fn with_legacy_object_mut<R>(
    object_id: ObjectID,
    f: impl FnOnce(&mut Object) -> R,
) -> Option<R> {
    if !legacy_contains(object_id) {
        return None;
    }
    OBJECT_REGISTRY.with_object_mut(object_id, f)
}

pub fn clear_legacy_objects() {
    if let Ok(mut guard) = LEGACY_OBJECT_REGISTRY.store.write() {
        guard.clear();
        LEGACY_OBJECT_REGISTRY.set_live_count(0);
    }
}

/// Wave 248: host/presentation empty probe (lock-free).
pub fn legacy_object_registry_is_empty() -> bool {
    LEGACY_OBJECT_REGISTRY.is_empty()
}
