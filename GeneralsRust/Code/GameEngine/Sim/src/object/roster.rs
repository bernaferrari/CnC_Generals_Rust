//! The receiving simulation's authoritative ObjectID lookup and list order.
//!
//! C++ GameLogic::registerObject prepends to m_objList. The lookup table also
//! contains constructors before registration and is cleared before reset walks
//! the remaining list. Both paths borrow the same allocations here.

use generals_game_domain::ObjectId;
use rustc_hash::FxHashMap;
use std::collections::{HashMap, HashSet};
use std::hash::{BuildHasher, Hash};
use std::sync::{Arc, Weak};

pub trait RosterEntry {
    fn roster_identity(&self) -> ObjectIdentity<'_>;
}

/// One allocation lifetime, independent of its reusable numeric ObjectID.
pub struct ObjectIdentity<'a> {
    pub id: ObjectId,
    pub generation: u64,
    pub token: &'a Arc<()>,
}

#[derive(Debug, PartialEq, Eq, thiserror::Error)]
pub enum ConstructorLookupError {
    #[error("constructor lookup requires its live receiving world")]
    UnboundWorld,
    #[error("constructor lookup admission is occupied, invalid or terminal")]
    UnavailableId,
    #[error("constructor registration is terminal")]
    RetirementFailed,
    #[error("constructor receipt belongs to another world")]
    ForeignWorld,
    #[error("constructor lookup lifetime is absent")]
    MissingActor,
    #[error("constructor lookup lifetime changed")]
    ChangedActor,
    #[error("constructor lifetime already registered")]
    AlreadyRegistered,
}

/// Stable per-match allocations with separate ID lookup and linked visitation.
#[derive(Debug)]
pub struct ObjectRoster<T> {
    // Keep the ID table compact and retire each Object in its original allocation.
    map: FxHashMap<ObjectId, Box<T>>,
    registration_order: Vec<ObjectId>,
    // Test instrumentation for passes that must avoid mutable Object loans.
    #[cfg(any(test, feature = "test-object-roster"))]
    mutation_count: u64,
    // Namespace is a weak loan of the EXISTING GameLogic identity. Raw standalone
    // stores remain usable for registered fixtures, but cannot admit constructors.
    constructor_world: Weak<()>,
    // A failed destructor cannot put its Object back into the live lookup.
    // Retain the exact owned value for diagnostics until this session is
    // discarded with its native owner. Clearing a live roster is not an
    // acknowledgement that failed native resource cleanup succeeded.
    failed_retirement: Option<FailedObjectRetirement<T>>,
    // C++ reset clears m_objVector before destroying the still-linked Object
    // list. Keep canonical storage/ordered visitation available to the owner,
    // while ordinary ID lookup observes the cleared vector during that phase.
    ordinary_lookup_hidden_ids: Option<HashSet<ObjectId>>,
}

#[derive(Debug)]
struct FailedObjectRetirement<T> {
    object: Box<T>,
    reason: String,
}

impl<T> Default for ObjectRoster<T> {
    fn default() -> Self {
        Self {
            map: FxHashMap::default(),
            registration_order: Vec::new(),
            #[cfg(any(test, feature = "test-object-roster"))]
            mutation_count: 0,
            constructor_world: Weak::new(),
            failed_retirement: None,
            ordinary_lookup_hidden_ids: None,
        }
    }
}

/// Immutable proof of one lookup-visible, not-yet-registered lifetime.
/// Both tokens already exist on GameLogic/Object. No additional shared owner,
/// mutable publication slot or synchronization is introduced.
#[derive(Debug)]
pub struct ConstructorLookupReceipt {
    id: ObjectId,
    generation: u64,
    world: Weak<()>,
    actor: Weak<()>,
}

impl<T: RosterEntry> ObjectRoster<T> {
    #[inline]
    fn mark_objects_mutably_accessed(&mut self) {
        #[cfg(any(test, feature = "test-object-roster"))]
        {
            self.mutation_count = self.mutation_count.wrapping_add(1);
        }
    }

    pub fn new() -> Self {
        Self::default()
    }

    pub fn new_for_world(world: &Arc<()>) -> Self {
        Self {
            constructor_world: Arc::downgrade(world),
            ..Self::default()
        }
    }

    /// Stable owner storage access. Unlike `lookup`, this intentionally
    /// ignores reset's ordinary-ID-lookup mask so internal Object lifecycle
    /// callbacks can keep operating on the linked teardown roster.
    pub fn get(&self, id: &ObjectId) -> Option<&T> {
        self.map.get(id).map(Box::as_ref)
    }

    /// Random access to the linked Object list, including during lookup reset.
    pub fn get_registered(&self, id: &ObjectId) -> Option<&T> {
        let object = self.map.get(id)?;
        if self.has_unregistered_lookup() && !self.registration_order.contains(id) {
            return None;
        }
        Some(object.as_ref())
    }

    pub fn lookup(&self, id: &ObjectId) -> Option<&T> {
        if self
            .ordinary_lookup_hidden_ids
            .as_ref()
            .is_some_and(|hidden| hidden.contains(id))
        {
            None
        } else {
            self.get(id)
        }
    }

    pub fn contains_key(&self, id: &ObjectId) -> bool {
        self.map.contains_key(id)
    }

    pub fn suspend_ordinary_lookup_for_reset(&mut self) {
        self.mark_objects_mutably_accessed();
        self.ordinary_lookup_hidden_ids = Some(self.registration_order.iter().copied().collect());
    }

    pub fn registered(
        &self,
    ) -> impl DoubleEndedIterator<Item = (&ObjectId, &T)> + ExactSizeIterator {
        self.registration_order.iter().map(|id| {
            (
                id,
                self.map
                    .get(id)
                    .expect("registered Object missing from lookup")
                    .as_ref(),
            )
        })
    }

    pub fn registered_values(&self) -> impl DoubleEndedIterator<Item = &T> + ExactSizeIterator {
        self.registered().map(|(_, object)| object)
    }

    pub fn registered_keys(
        &self,
    ) -> impl DoubleEndedIterator<Item = &ObjectId> + ExactSizeIterator {
        self.registration_order.iter()
    }

    pub fn registered_len(&self) -> usize {
        self.registration_order.len()
    }
    pub fn registered_is_empty(&self) -> bool {
        self.registration_order.is_empty()
    }

    pub fn registered_values_mut(&mut self) -> impl Iterator<Item = &mut T> {
        self.registered_mut().map(|(_, object)| object)
    }

    /// Visit registered Objects in linked-list order with one mutable loan at a time.
    pub fn for_each_registered_mut(&mut self, mut visit: impl FnMut(&mut T)) {
        self.for_each_registered_entry_mut(|_, object| visit(object));
    }

    pub fn for_each_registered_entry_mut(&mut self, mut visit: impl FnMut(ObjectId, &mut T)) {
        self.mark_objects_mutably_accessed();
        for &id in &self.registration_order {
            visit(id, self.map.get_mut(&id).expect("registered Object exists"));
        }
    }

    /// IDs in C++ `getFirstObject` / `getNextObject` order.
    pub fn ordered_ids(&self) -> &[ObjectId] {
        &self.registration_order
    }

    /// Internal storage operation; callers must validate the new lifetime first.
    pub fn admit(&mut self, id: ObjectId, object: T) {
        assert!(
            !self.map.contains_key(&id),
            "new Object admission cannot replace a live lifetime"
        );
        self.mark_objects_mutably_accessed();
        self.registration_order.insert(0, id);
        if let Some(hidden) = self.ordinary_lookup_hidden_ids.as_mut() {
            hidden.remove(&id);
        }
        self.map.insert(id, Box::new(object));
    }

    /// Fixture-only permissive insertion; production births use checked admission.
    /// Restoration commits only validated, already registered lifetimes in place.
    #[cfg(any(test, feature = "test-object-roster"))]
    pub fn insert(&mut self, id: ObjectId, object: T) -> Option<T> {
        self.mark_objects_mutably_accessed();
        if !self.map.contains_key(&id) {
            self.registration_order.insert(0, id);
        }
        self.map.insert(id, Box::new(object)).map(|object| *object)
    }

    #[cfg(any(test, feature = "test-object-roster"))]
    pub fn remove(&mut self, id: &ObjectId) -> Option<T> {
        self.remove_owned(id).map(|object| *object)
    }

    pub fn remove_owned(&mut self, id: &ObjectId) -> Option<Box<T>> {
        let object = self.map.remove(id)?;
        self.mark_objects_mutably_accessed();
        self.registration_order.retain(|existing| existing != id);
        Some(object)
    }

    pub fn clear(&mut self) {
        self.mark_objects_mutably_accessed();
        self.map.clear();
        self.registration_order.clear();
        self.ordinary_lookup_hidden_ids = None;
    }

    pub fn failed_retirement_reason(&self) -> Option<&str> {
        self.failed_retirement
            .as_ref()
            .map(|failure| failure.reason.as_str())
    }

    pub fn failed_retirement_object(&self) -> Option<&T> {
        self.failed_retirement
            .as_ref()
            .map(|failure| failure.object.as_ref())
    }

    pub fn retain_failed_retirement(&mut self, object: Box<T>, reason: String) {
        assert!(
            self.failed_retirement.is_none(),
            "a failed retirement is terminal"
        );
        assert!(
            !self.map.contains_key(&object.roster_identity().id),
            "a retiring Object is not live"
        );
        self.failed_retirement = Some(FailedObjectRetirement { object, reason });
    }

    pub fn failed_retirement_object_mut(&mut self) -> Option<&mut T> {
        self.failed_retirement
            .as_mut()
            .map(|failure| failure.object.as_mut())
    }

    #[cfg(any(test, feature = "test-object-roster"))]
    pub fn mutation_count(&self) -> u64 {
        self.mutation_count
    }

    /// Mutable stable owner storage access; ordinary ID resolution must use
    /// `lookup_mut` so reset callback visibility follows C++ `m_objVector`.
    pub fn get_mut(&mut self, id: &ObjectId) -> Option<&mut T> {
        self.mark_objects_mutably_accessed();
        self.map.get_mut(id).map(Box::as_mut)
    }

    pub fn lookup_mut(&mut self, id: &ObjectId) -> Option<&mut T> {
        if self
            .ordinary_lookup_hidden_ids
            .as_ref()
            .is_some_and(|hidden| hidden.contains(id))
        {
            None
        } else {
            self.get_mut(id)
        }
    }

    /// Borrow distinct object values without exposing membership mutation.
    /// Like `HashMap::get_disjoint_mut`, duplicate keys are rejected by panic.
    pub fn get_disjoint_mut<const N: usize>(
        &mut self,
        keys: [&ObjectId; N],
    ) -> [Option<&mut T>; N] {
        self.mark_objects_mutably_accessed();
        self.map
            .get_disjoint_mut(keys)
            .map(|object| object.map(Box::as_mut))
    }

    /// Install a constructor's allocation for ID lookup. It joins traversal
    /// when its receipt is registered.
    pub fn admit_constructor_lookup(
        &mut self,
        object: T,
    ) -> Result<ConstructorLookupReceipt, (T, ConstructorLookupError)> {
        if self.constructor_world.upgrade().is_none() {
            return Err((object, ConstructorLookupError::UnboundWorld));
        }
        let identity = object.roster_identity();
        if self.failed_retirement.is_some()
            || identity.id.0 == 0
            || self.map.contains_key(&identity.id)
        {
            return Err((object, ConstructorLookupError::UnavailableId));
        }
        let receipt = ConstructorLookupReceipt {
            id: identity.id,
            generation: identity.generation,
            world: self.constructor_world.clone(),
            actor: Arc::downgrade(identity.token),
        };
        self.mark_objects_mutably_accessed();
        if let Some(hidden) = self.ordinary_lookup_hidden_ids.as_mut() {
            hidden.remove(&receipt.id);
        }
        self.map.insert(receipt.id, Box::new(object));
        Ok(receipt)
    }

    /// C++ registerObject: the existing lookup lifetime joins the Object list.
    /// Validate all proof fields before touching list order; never rebind identity.
    pub fn register_constructor_lookup(
        &mut self,
        receipt: &ConstructorLookupReceipt,
    ) -> Result<(), ConstructorLookupError> {
        if self.failed_retirement.is_some() {
            return Err(ConstructorLookupError::RetirementFailed);
        }
        self.publish_constructor_lookup(receipt)
    }

    /// Settle an existing failed birth for the caller's ordinary retirement path.
    /// A terminal error in a nested callback can stop new admissions while the
    /// enclosing constructor still owns its allocation.
    pub fn retain_failed_constructor_lookup(
        &mut self,
        receipt: &ConstructorLookupReceipt,
    ) -> Result<(), ConstructorLookupError> {
        self.publish_constructor_lookup(receipt)
    }

    fn publish_constructor_lookup(
        &mut self,
        receipt: &ConstructorLookupReceipt,
    ) -> Result<(), ConstructorLookupError> {
        if self.constructor_world.upgrade().is_none()
            || !receipt.world.ptr_eq(&self.constructor_world)
        {
            return Err(ConstructorLookupError::ForeignWorld);
        }
        let actor = self
            .map
            .get(&receipt.id)
            .ok_or(ConstructorLookupError::MissingActor)?;
        let identity = actor.roster_identity();
        if identity.id != receipt.id
            || identity.generation != receipt.generation
            || !receipt.actor.ptr_eq(&Arc::downgrade(identity.token))
        {
            return Err(ConstructorLookupError::ChangedActor);
        }
        if self.registration_order.contains(&receipt.id) {
            return Err(ConstructorLookupError::AlreadyRegistered);
        }
        self.mark_objects_mutably_accessed();
        self.registration_order.insert(0, receipt.id);
        Ok(())
    }

    /// Save/reset admission can reject a live unfinished constructor, rather
    /// than silently serializing only one of its lookup/list representations.
    pub fn has_unregistered_lookup(&self) -> bool {
        self.map.len() != self.registration_order.len()
    }

    /// Bounded mutable registered traversal without unsafe or cloning Objects.
    /// This allocation holds references only, never competing gameplay state.
    pub fn registered_mut(&mut self) -> impl Iterator<Item = (&ObjectId, &mut T)> {
        self.mark_objects_mutably_accessed();
        registered_mut_in_order(&mut self.map, &self.registration_order)
            .map(|(id, object)| (id, object.as_mut()))
    }
}

impl<T: RosterEntry> std::ops::Index<&ObjectId> for ObjectRoster<T> {
    type Output = T;
    fn index(&self, id: &ObjectId) -> &Self::Output {
        &self.map[id]
    }
}

impl<T: RosterEntry> std::ops::IndexMut<&ObjectId> for ObjectRoster<T> {
    fn index_mut(&mut self, id: &ObjectId) -> &mut T {
        self.mark_objects_mutably_accessed();
        self.map.get_mut(id).expect("Object ID is not registered")
    }
}

fn registered_mut_in_order<'a, K: Copy + Eq + Hash, V, S: BuildHasher>(
    map: &'a mut HashMap<K, V, S>,
    registration_order: &[K],
) -> impl Iterator<Item = (&'a K, &'a mut V)> {
    let order: FxHashMap<_, _> = registration_order
        .iter()
        .enumerate()
        .map(|(index, id)| (*id, index))
        .collect();
    // An object may exist in the lookup map before its constructor registers
    // it in the C++ object list. Such entries are excluded from traversal.
    let mut rows: Vec<_> = std::iter::repeat_with(|| None)
        .take(registration_order.len())
        .collect();
    for (id, object) in map.iter_mut() {
        if let Some(&index) = order.get(id) {
            rows[index] = Some((id, object));
        }
    }
    rows.into_iter().flatten()
}
