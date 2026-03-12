use crate::common::ObjectID;
use crate::object::Object;
use once_cell::sync::Lazy;
use std::collections::HashMap;
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

    fn get(&mut self, id: ObjectID) -> Option<Arc<RwLock<Object>>> {
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
}

static LEGACY_OBJECT_REGISTRY: Lazy<RwLock<LegacyObjectRegistry>> =
    Lazy::new(|| RwLock::new(LegacyObjectRegistry::default()));

pub fn register_legacy_object(object: &Arc<RwLock<Object>>) {
    if let Ok(mut guard) = LEGACY_OBJECT_REGISTRY.write() {
        if let Ok(obj_guard) = object.read() {
            guard.register(obj_guard.get_id(), object);
        }
    }
}

pub fn unregister_legacy_object(object_id: ObjectID) {
    if let Ok(mut guard) = LEGACY_OBJECT_REGISTRY.write() {
        guard.unregister(object_id);
    }
}

pub fn get_legacy_object(object_id: ObjectID) -> Option<Arc<RwLock<Object>>> {
    if let Ok(mut guard) = LEGACY_OBJECT_REGISTRY.write() {
        guard.get(object_id)
    } else {
        None
    }
}

pub fn clear_legacy_objects() {
    if let Ok(mut guard) = LEGACY_OBJECT_REGISTRY.write() {
        guard.clear();
    }
}
