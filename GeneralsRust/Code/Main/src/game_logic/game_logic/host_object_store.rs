//! Match-owned lookup and original Object-list registration order.
//!
//! C++ `GameLogic::registerObject` prepends to `m_objList` independently of its
//! ID lookup table. Hash iteration and numerical IDs cannot represent that order.

use super::prelude::*;

#[derive(Debug, Default)]
pub struct HostObjectStore {
    map: HashMap<ObjectId, Object>,
    registration_order: Vec<ObjectId>,
    // A failed destructor cannot put its Object back into the live lookup.
    // Retain the exact owned value for diagnostics until this session is
    // discarded with its native owner. Clearing a live roster is not an
    // acknowledgement that failed native resource cleanup succeeded.
    failed_retirement: Option<FailedObjectRetirement>,
}

#[derive(Debug)]
struct FailedObjectRetirement {
    object: Box<Object>,
    reason: String,
}

impl HostObjectStore {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn map(&self) -> &HashMap<ObjectId, Object> {
        &self.map
    }

    /// IDs in C++ `getFirstObject` / `getNextObject` order.
    pub fn ordered_ids(&self) -> &[ObjectId] {
        &self.registration_order
    }

    /// Admit a new object lifetime, even when replacing a reused ID.
    pub fn admit(&mut self, id: ObjectId, object: Object) -> Option<Object> {
        self.registration_order.retain(|existing| *existing != id);
        self.registration_order.insert(0, id);
        self.map.insert(id, object)
    }

    /// Replace an existing value without changing its registered position.
    /// A previously absent ID is a new admission and is prepended.
    pub fn insert(&mut self, id: ObjectId, object: Object) -> Option<Object> {
        if !self.map.contains_key(&id) {
            self.registration_order.insert(0, id);
        }
        self.map.insert(id, object)
    }

    pub fn remove(&mut self, id: &ObjectId) -> Option<Object> {
        let object = self.map.remove(id)?;
        self.registration_order.retain(|existing| existing != id);
        Some(object)
    }

    pub fn clear(&mut self) {
        self.map.clear();
        self.registration_order.clear();
    }

    pub(crate) fn failed_retirement_reason(&self) -> Option<&str> {
        self.failed_retirement
            .as_ref()
            .map(|failure| failure.reason.as_str())
    }

    pub(crate) fn failed_retirement_object(&self) -> Option<&Object> {
        self.failed_retirement
            .as_ref()
            .map(|failure| failure.object.as_ref())
    }

    pub(crate) fn retain_failed_retirement(&mut self, object: Object, reason: String) {
        assert!(
            self.failed_retirement.is_none(),
            "a failed retirement is terminal"
        );
        assert!(
            !self.map.contains_key(&object.id),
            "a retiring Object is not live"
        );
        self.failed_retirement = Some(FailedObjectRetirement {
            object: Box::new(object),
            reason,
        });
    }

    pub fn get_mut(&mut self, id: &ObjectId) -> Option<&mut Object> {
        self.map.get_mut(id)
    }

    pub fn iter_mut(&mut self) -> std::collections::hash_map::IterMut<'_, ObjectId, Object> {
        self.map.iter_mut()
    }

    pub fn values_mut(&mut self) -> std::collections::hash_map::ValuesMut<'_, ObjectId, Object> {
        self.map.values_mut()
    }

    /// Borrow distinct object values without exposing membership mutation.
    /// Like `HashMap::get_disjoint_mut`, duplicate keys are rejected by panic.
    pub fn get_disjoint_mut<const N: usize>(
        &mut self,
        keys: [&ObjectId; N],
    ) -> [Option<&mut Object>; N] {
        self.map.get_disjoint_mut(keys)
    }
}

// Immutable map access retains lookup compatibility without allowing callers
// to mutate membership behind the registration-order owner.
impl std::ops::Deref for HostObjectStore {
    type Target = HashMap<ObjectId, Object>;
    fn deref(&self) -> &Self::Target {
        &self.map
    }
}

impl std::ops::Index<&ObjectId> for HostObjectStore {
    type Output = Object;
    fn index(&self, id: &ObjectId) -> &Self::Output {
        &self.map[id]
    }
}

impl std::ops::IndexMut<&ObjectId> for HostObjectStore {
    fn index_mut(&mut self, id: &ObjectId) -> &mut Object {
        self.map.get_mut(id).expect("Object ID is not registered")
    }
}

impl<'a> IntoIterator for &'a HostObjectStore {
    type Item = (&'a ObjectId, &'a Object);
    type IntoIter = std::collections::hash_map::Iter<'a, ObjectId, Object>;
    fn into_iter(self) -> Self::IntoIter {
        self.map.iter()
    }
}

impl<'a> IntoIterator for &'a mut HostObjectStore {
    type Item = (&'a ObjectId, &'a mut Object);
    type IntoIter = std::collections::hash_map::IterMut<'a, ObjectId, Object>;
    fn into_iter(self) -> Self::IntoIter {
        self.map.iter_mut()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn object(id: u32) -> Object {
        Object::new(
            ThingTemplate::new("OwnedStoreUnit"),
            ObjectId(id),
            Team::USA,
        )
    }

    #[test]
    fn registration_order_is_independent_of_ids_and_replacement() {
        let mut store = HostObjectStore::new();
        for id in [500, 2, 90] {
            assert!(store.insert(ObjectId(id), object(id)).is_none());
        }
        assert_eq!(
            store.ordered_ids(),
            &[ObjectId(90), ObjectId(2), ObjectId(500)]
        );
        assert!(store.insert(ObjectId(2), object(2)).is_some());
        assert_eq!(
            store.ordered_ids(),
            &[ObjectId(90), ObjectId(2), ObjectId(500)]
        );
        assert_eq!(store.get(&ObjectId(500)).unwrap().id, ObjectId(500));
        assert_eq!(store[&ObjectId(2)].id, ObjectId(2));
        assert_eq!(store.map().len(), 3);
    }

    #[test]
    fn removal_recreation_and_explicit_admission_have_distinct_lifetimes() {
        let mut store = HostObjectStore::new();
        for id in [500, 2, 90] {
            store.admit(ObjectId(id), object(id));
        }
        let removed = store.remove(&ObjectId(2)).unwrap();
        assert_eq!(store.ordered_ids(), &[ObjectId(90), ObjectId(500)]);
        assert!(store.remove(&ObjectId(2)).is_none());
        store.insert(ObjectId(2), removed);
        assert_eq!(
            store.ordered_ids(),
            &[ObjectId(2), ObjectId(90), ObjectId(500)]
        );
        store.admit(ObjectId(500), object(500));
        assert_eq!(
            store.ordered_ids(),
            &[ObjectId(500), ObjectId(2), ObjectId(90)]
        );
        store.clear();
        assert!(store.is_empty());
        assert!(store.ordered_ids().is_empty());
        store.insert(ObjectId(90), object(90));
        assert_eq!(store.ordered_ids(), &[ObjectId(90)]);
    }

    #[test]
    fn disjoint_value_borrows_preserve_membership_and_registration_order() {
        let mut store = HostObjectStore::new();
        store.insert(ObjectId(500), object(500));
        store.insert(ObjectId(2), object(2));
        let [first, second, missing] =
            store.get_disjoint_mut([&ObjectId(500), &ObjectId(2), &ObjectId(90)]);
        first.unwrap().health.current = 7.0;
        second.unwrap().health.current = 9.0;
        assert!(missing.is_none());
        assert_eq!(store[&ObjectId(500)].health.current, 7.0);
        assert_eq!(store[&ObjectId(2)].health.current, 9.0);
        assert_eq!(store.ordered_ids(), &[ObjectId(2), ObjectId(500)]);
    }

    #[test]
    fn mutable_values_preserve_order_and_peer_store_isolation() {
        let mut owner = HostObjectStore::new();
        let mut peer = HostObjectStore::new();
        owner.insert(ObjectId(9), object(9));
        peer.insert(ObjectId(9), object(9));
        owner.get_mut(&ObjectId(9)).unwrap().health.current = 7.0;
        for (_, object) in owner.iter_mut() {
            object.health.current += 1.0;
        }
        for object in owner.values_mut() {
            object.health.current += 1.0;
        }
        owner[&ObjectId(9)].health.current += 1.0;
        assert_eq!(owner[&ObjectId(9)].health.current, 10.0);
        assert_ne!(peer[&ObjectId(9)].health.current, 10.0);
        assert_eq!(owner.ordered_ids(), &[ObjectId(9)]);
    }
}
