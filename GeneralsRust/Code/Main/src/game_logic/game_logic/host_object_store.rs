//! Application adapters for the receiving simulation's live Object roster.

use super::prelude::*;
use generals_sim::object::{ConstructorLookupError, ConstructorLookupReceipt};
use generals_sim::object::{ObjectIdentity, ObjectRoster, RosterEntry};

/// Application access to the match's roster. Membership changes stay internal.
#[derive(Debug, Default)]
pub struct HostObjectStore(ObjectRoster<Object>);

impl RosterEntry for Object {
    fn roster_identity(&self) -> ObjectIdentity<'_> {
        ObjectIdentity {
            id: self.id,
            generation: self.visual_object_generation,
            token: &self.visual_draw_identity_token,
        }
    }
}

impl std::ops::Index<&ObjectId> for HostObjectStore {
    type Output = Object;

    fn index(&self, id: &ObjectId) -> &Self::Output {
        &self.0[id]
    }
}

impl std::ops::IndexMut<&ObjectId> for HostObjectStore {
    fn index_mut(&mut self, id: &ObjectId) -> &mut Self::Output {
        &mut self.0[id]
    }
}

impl HostObjectStore {
    pub fn new() -> Self {
        Self::default()
    }

    pub(in crate::game_logic) fn new_for_world(world: &std::sync::Arc<()>) -> Self {
        Self(ObjectRoster::new_for_world(world))
    }

    pub(crate) fn admit(&mut self, id: ObjectId, object: Object) {
        Self::assert_detached_drawable(&object);
        self.0.admit(id, object);
    }

    /// Remove outgoing partition stamps from live and retained retired actors.
    pub(in crate::game_logic) fn retire_owned_scalar_stamps(
        &mut self,
        partition: &mut crate::game_logic::partition_manager::PartitionManager,
    ) {
        self.0.for_each_registered_mut(|object| {
            object.unstamp_partition_value_threat_with_partition_owner(partition);
        });
        if let Some(object) = self.0.failed_retirement_object_mut() {
            object.unstamp_partition_value_threat_with_partition_owner(partition);
        }
    }

    /// Borrow the receiving roster without borrowing mutable simulation state.
    pub fn read(&self) -> HostObjectRead<'_> {
        HostObjectRead {
            store: &self.0,
            players: None,
            team_factory: None,
        }
    }

    pub(crate) fn get(&self, id: &ObjectId) -> Option<&Object> {
        self.0.get(id)
    }

    pub(in crate::game_logic) fn get_registered(&self, id: &ObjectId) -> Option<&Object> {
        self.0.get_registered(id)
    }

    /// Resolve an ID using the ordinary lookup visible to callbacks.
    pub fn lookup(&self, id: &ObjectId) -> Option<&Object> {
        self.0.lookup(id)
    }

    pub(crate) fn contains_key(&self, id: &ObjectId) -> bool {
        self.0.contains_key(id)
    }

    pub(crate) fn suspend_ordinary_lookup_for_reset(&mut self) {
        self.0.suspend_ordinary_lookup_for_reset()
    }

    pub fn registered(
        &self,
    ) -> impl DoubleEndedIterator<Item = (&ObjectId, &Object)> + ExactSizeIterator {
        self.0.registered()
    }

    pub fn registered_values(
        &self,
    ) -> impl DoubleEndedIterator<Item = &Object> + ExactSizeIterator {
        self.0.registered_values()
    }

    pub fn registered_keys(
        &self,
    ) -> impl DoubleEndedIterator<Item = &ObjectId> + ExactSizeIterator {
        self.0.registered_keys()
    }

    pub fn registered_len(&self) -> usize {
        self.0.registered_len()
    }

    pub fn registered_is_empty(&self) -> bool {
        self.0.registered_is_empty()
    }

    pub fn registered_values_mut(&mut self) -> impl Iterator<Item = &mut Object> {
        self.0.registered_values_mut()
    }

    pub(in crate::game_logic) fn for_each_registered_mut(
        &mut self,
        visit: impl FnMut(&mut Object),
    ) {
        self.0.for_each_registered_mut(visit)
    }

    pub(in crate::game_logic) fn for_each_registered_entry_mut(
        &mut self,
        visit: impl FnMut(ObjectId, &mut Object),
    ) {
        self.0.for_each_registered_entry_mut(visit)
    }

    /// IDs in the original GameLogic linked-list order.
    pub fn ordered_ids(&self) -> &[ObjectId] {
        self.0.ordered_ids()
    }

    #[cfg(test)]
    pub fn insert(&mut self, id: ObjectId, object: Object) -> Option<Object> {
        self.0.insert(id, object)
    }

    #[cfg(test)]
    pub(crate) fn remove(&mut self, id: &ObjectId) -> Option<Object> {
        self.0.remove(id)
    }

    pub(crate) fn remove_owned(&mut self, id: &ObjectId) -> Option<Box<Object>> {
        self.0.remove_owned(id)
    }

    pub(crate) fn clear(&mut self) {
        self.0.clear()
    }

    pub(crate) fn failed_retirement_reason(&self) -> Option<&str> {
        self.0.failed_retirement_reason()
    }

    pub(crate) fn failed_retirement_object(&self) -> Option<&Object> {
        self.0.failed_retirement_object()
    }

    pub(crate) fn retain_failed_retirement(&mut self, object: Box<Object>, reason: String) {
        self.0.retain_failed_retirement(object, reason)
    }

    pub(crate) fn get_mut(&mut self, id: &ObjectId) -> Option<&mut Object> {
        self.0.get_mut(id)
    }

    pub(crate) fn lookup_mut(&mut self, id: &ObjectId) -> Option<&mut Object> {
        self.0.lookup_mut(id)
    }

    /// Borrow distinct values. Duplicate keys panic as in `HashMap::get_disjoint_mut`.
    pub fn get_disjoint_mut<const N: usize>(
        &mut self,
        keys: [&ObjectId; N],
    ) -> [Option<&mut Object>; N] {
        self.0.get_disjoint_mut(keys)
    }

    pub(in crate::game_logic) fn admit_constructor_lookup(
        &mut self,
        object: Object,
    ) -> Result<ConstructorLookupReceipt, (Object, ConstructorLookupError)> {
        Self::assert_detached_drawable(&object);
        self.0.admit_constructor_lookup(object)
    }

    fn assert_detached_drawable(_object: &Object) {
        #[cfg(feature = "game_client")]
        assert!(
            _object.jet_ai.lockon_runtime.is_none(),
            "new Object admission cannot inherit a live auxiliary Drawable receipt"
        );
    }

    pub(in crate::game_logic) fn register_constructor_lookup(
        &mut self,
        receipt: &ConstructorLookupReceipt,
    ) -> Result<(), ConstructorLookupError> {
        self.0.register_constructor_lookup(receipt)
    }

    pub(in crate::game_logic) fn retain_failed_constructor_lookup(
        &mut self,
        receipt: &ConstructorLookupReceipt,
    ) -> Result<(), ConstructorLookupError> {
        self.0.retain_failed_constructor_lookup(receipt)
    }

    pub(in crate::game_logic) fn has_unregistered_lookup(&self) -> bool {
        self.0.has_unregistered_lookup()
    }

    /// Visit mutable actors in registration order.
    pub fn registered_mut(&mut self) -> impl Iterator<Item = (&ObjectId, &mut Object)> {
        self.0.registered_mut()
    }
}

/// Explicit receiving read capability: lookup and registered visitation have
/// different C++ meanings even while both borrow the same canonical values.
#[derive(Clone, Copy)]
pub struct HostObjectRead<'a> {
    store: &'a ObjectRoster<Object>,
    players: Option<&'a HashMap<u32, Player>>,
    team_factory: Option<&'a gamelogic::team::TeamFactoryHandle>,
}

impl std::ops::Index<&ObjectId> for HostObjectRead<'_> {
    type Output = Object;

    fn index(&self, id: &ObjectId) -> &Self::Output {
        self.store.lookup(id).expect("Object ID is not visible")
    }
}

impl<'a> HostObjectRead<'a> {
    /// Borrow the controller table belonging to the same session as this store.
    pub(crate) fn with_players(mut self, players: &'a HashMap<u32, Player>) -> Self {
        self.players = Some(players);
        self
    }
    pub(crate) fn with_team_factory(
        mut self,
        team_factory: &'a gamelogic::team::TeamFactoryHandle,
    ) -> Self {
        self.team_factory = Some(team_factory);
        self
    }

    pub(crate) fn relationship(
        self,
        source: &Object,
        target: &Object,
    ) -> Option<game_engine::common::game_common::Relationship> {
        Some(
            crate::game_logic::GameLogic::object_relationship_with_world(
                self.team_factory?,
                self.players?,
                source,
                target,
            ),
        )
    }

    pub(crate) fn controller_is_human(self, id: u32) -> Option<bool> {
        self.players?.get(&id).map(|player| player.is_human)
    }

    pub fn lookup(self, id: &ObjectId) -> Option<&'a Object> {
        self.store.lookup(id)
    }

    /// Resolve an Object still on the receiving registration list, including
    /// the interval where C++ reset hides ordinary ObjectID lookup.
    #[cfg(feature = "game_client")]
    pub(crate) fn lookup_registered(self, id: &ObjectId) -> Option<&'a Object> {
        self.store.get_registered(id)
    }
    pub fn registered(
        self,
    ) -> impl DoubleEndedIterator<Item = (&'a ObjectId, &'a Object)> + ExactSizeIterator + 'a {
        self.store.registered()
    }
    pub fn registered_values(
        self,
    ) -> impl DoubleEndedIterator<Item = &'a Object> + ExactSizeIterator + 'a {
        self.registered().map(|(_, object)| object)
    }
    pub fn registered_keys(
        self,
    ) -> impl DoubleEndedIterator<Item = &'a ObjectId> + ExactSizeIterator + 'a {
        self.registered_ids().iter()
    }
    /// Counter for verifying which passes took mutable Object loans.
    #[cfg(test)]
    pub(crate) fn mutation_count(self) -> u64 {
        self.store.mutation_count()
    }

    pub(crate) fn registered_ids(self) -> &'a [ObjectId] {
        self.store.ordered_ids()
    }
    pub fn registered_is_empty(self) -> bool {
        self.store.registered_is_empty()
    }
    pub fn registered_len(self) -> usize {
        self.store.registered_len()
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
        assert_eq!(store.registered_len(), 3);
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
        let rejected = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            store.admit(ObjectId(500), object(500));
        }));
        assert!(rejected.is_err());
        assert_eq!(
            store.ordered_ids(),
            &[ObjectId(2), ObjectId(90), ObjectId(500)]
        );
        store.remove(&ObjectId(500)).unwrap();
        store.admit(ObjectId(500), object(500));
        assert_eq!(
            store.ordered_ids(),
            &[ObjectId(500), ObjectId(2), ObjectId(90)]
        );
        store.clear();
        assert!(store.registered_is_empty());
        assert!(store.ordered_ids().is_empty());
        store.insert(ObjectId(90), object(90));
        assert_eq!(store.ordered_ids(), &[ObjectId(90)]);
    }

    #[test]
    fn retirement_keeps_the_object_allocation_through_index_growth() {
        let mut store = HostObjectStore::new();
        let original = ObjectId(2);
        store.admit(original, object(original.0));
        let actor = std::ptr::from_ref(store.get(&original).unwrap());
        for id in 3..48 {
            store.admit(ObjectId(id), object(id));
        }

        assert_eq!(actor, std::ptr::from_ref(store.get(&original).unwrap()));
        let retired = store.remove_owned(&original).unwrap();
        assert_eq!(actor, std::ptr::from_ref(retired.as_ref()));
        assert!(store.lookup(&original).is_none());
        assert_eq!(store.ordered_ids().first(), Some(&ObjectId(47)));
        assert_eq!(store.ordered_ids().last(), Some(&ObjectId(3)));
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
        for (_, object) in owner.registered_mut() {
            object.health.current += 1.0;
        }
        for object in owner.registered_values_mut() {
            object.health.current += 1.0;
        }
        owner[&ObjectId(9)].health.current += 1.0;
        assert_eq!(owner[&ObjectId(9)].health.current, 10.0);
        assert_ne!(peer[&ObjectId(9)].health.current, 10.0);
        assert_eq!(owner.ordered_ids(), &[ObjectId(9)]);
    }
}

#[cfg(test)]
mod lookup_registration_tests {
    use super::*;
    fn actor(id: u32) -> Object {
        Object::new(
            ThingTemplate::new("LookupRegistration"),
            ObjectId(id),
            Team::USA,
        )
    }
    #[test]
    fn lookup_visible_parent_registers_after_child_without_moving_or_rebinding() {
        let world = std::sync::Arc::new(());
        let mut store = HostObjectStore::new_for_world(&world);
        let parent = actor(10);
        let token = std::sync::Arc::downgrade(&parent.visual_draw_identity_token);
        let parent = store.admit_constructor_lookup(parent).unwrap();
        assert!(store.read().lookup(&ObjectId(10)).is_some());
        assert_eq!(store.read().registered_len(), 0);
        assert!(store.has_unregistered_lookup());
        let child = store.admit_constructor_lookup(actor(11)).unwrap();
        store.register_constructor_lookup(&child).unwrap();
        assert_eq!(
            store
                .read()
                .registered()
                .map(|(id, _)| *id)
                .collect::<Vec<_>>(),
            [ObjectId(11)]
        );
        store.register_constructor_lookup(&parent).unwrap();
        assert_eq!(store.ordered_ids(), [ObjectId(10), ObjectId(11)]);
        assert!(!store.has_unregistered_lookup());
        assert!(token.ptr_eq(&std::sync::Arc::downgrade(
            &store[&ObjectId(10)].visual_draw_identity_token
        )));
        let before = store.ordered_ids().to_vec();
        assert!(store.register_constructor_lookup(&parent).is_err());
        assert_eq!(store.ordered_ids(), before);
    }
    #[test]
    fn stale_foreign_or_changed_receipt_cannot_partially_register() {
        let owner = std::sync::Arc::new(());
        let peer = std::sync::Arc::new(());
        let mut store = HostObjectStore::new_for_world(&owner);
        let receipt = store.admit_constructor_lookup(actor(9)).unwrap();
        let mut peer_store = HostObjectStore::new_for_world(&peer);
        let _peer_receipt = peer_store.admit_constructor_lookup(actor(9)).unwrap();
        assert!(peer_store.register_constructor_lookup(&receipt).is_err());
        assert!(peer_store.ordered_ids().is_empty());
        assert!(store.ordered_ids().is_empty());
        store
            .get_mut(&ObjectId(9))
            .unwrap()
            .visual_object_generation += 1;
        assert!(store.register_constructor_lookup(&receipt).is_err());
        assert!(store.ordered_ids().is_empty());
        store.remove(&ObjectId(9));
        let replacement = store.admit_constructor_lookup(actor(9)).unwrap();
        assert!(store.register_constructor_lookup(&receipt).is_err());
        assert!(store.ordered_ids().is_empty());
        store.register_constructor_lookup(&replacement).unwrap();
        assert_eq!(store.ordered_ids(), [ObjectId(9)]);
    }
    #[test]
    fn registered_mut_uses_list_order_and_omits_unfinished_lookup() {
        let world = std::sync::Arc::new(());
        let mut store = HostObjectStore::new_for_world(&world);
        store.admit(ObjectId(3), actor(3));
        store.admit(ObjectId(1), actor(1));
        let _pending = store.admit_constructor_lookup(actor(7)).unwrap();
        let mut seen = Vec::new();
        for (id, actor) in store.registered_mut() {
            seen.push(*id);
            actor.health.current = 4.0;
        }
        assert_eq!(seen, [ObjectId(1), ObjectId(3)]);
        seen.clear();
        store.for_each_registered_mut(|actor| {
            seen.push(actor.id);
            actor.health.current = 3.0;
        });
        assert_eq!(seen, [ObjectId(1), ObjectId(3)]);
        assert_ne!(store[&ObjectId(7)].health.current, 4.0);
        assert_ne!(store[&ObjectId(7)].health.current, 3.0);
    }
    #[test]
    fn unfinished_world_constructor_rejects_save_reset_and_map_replacement_without_mutation() {
        let mut world = GameLogic::new();
        world.frame = 42;
        let receipt = world.objects.admit_constructor_lookup(actor(10)).unwrap();
        assert!(world.host_object(ObjectId(10)).is_some());
        assert_eq!(world.host_objects().registered_len(), 0);
        let error = crate::save_load::snapshot::SnapshotBuilder::new()
            .create_world_snapshot(&world)
            .unwrap_err();
        assert!(error.to_string().contains("unfinished Object constructor"));
        assert!(
            world
                .validate_map_replacement_preflight()
                .unwrap_err()
                .contains("unfinished Object constructor")
        );
        assert!(std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| world.reset())).is_err());
        assert_eq!(world.frame, 42);
        assert!(world.host_object(ObjectId(10)).is_some());
        assert!(world.objects.ordered_ids().is_empty());
        world.objects.register_constructor_lookup(&receipt).unwrap();
        assert!(!world.has_unregistered_host_objects());
        assert_eq!(world.objects.ordered_ids(), &[ObjectId(10)]);
    }
    fn canonical_save_fixture(with_actors: bool) -> GameLogic {
        let mut world = GameLogic::new();
        world.admit_receiving_module_content(
            crate::assets::ReceivingObjectContent::from_sources([(
                "read-capability-save.ini".into(),
                "Object ReadCapabilityActor\nKindOf = VEHICLE\nBody = ActiveBody BodyTag\nMaxHealth = 100\nInitialHealth = 100\nEnd\nEnd\n".into(),
            )]).unwrap(),
        ).unwrap();
        world.add_player(crate::game_logic::Player::new(
            1,
            Team::USA,
            "ReadOwner",
            true,
        ));
        if with_actors {
            for id in [90, 2, 500] {
                world.set_next_object_id_for_restore(ObjectId(id));
                let actual = world
                    .create_object_for_player(
                        "ReadCapabilityActor",
                        1,
                        glam::Vec3::new(id as f32, 0., 0.),
                    )
                    .unwrap();
                assert_eq!(actual, ObjectId(id));
            }
        }
        world.frame = 77;
        world
    }

    fn canonical_save_info() -> crate::save_load::SaveGameInfo {
        crate::save_load::SaveGameInfo {
            filename: "read-capability".into(),
            display_name: "Read capability save".into(),
            description: "Actual owner admission".into(),
            map_name: "ReadCapabilityAbsentMap".into(),
            pristine_map_name: None,
            campaign_side: None,
            mission_number: None,
            save_date: std::time::UNIX_EPOCH,
            game_version: "test".into(),
            play_time: std::time::Duration::ZERO,
            difficulty: crate::save_load::GameDifficulty::Medium,
            save_type: crate::save_load::SaveFileType::Normal,
        }
    }

    #[test]
    fn actual_save_writer_roundtrip_retains_nonnumeric_list_and_cpp_load_prepend_order() {
        let source = canonical_save_fixture(true);
        let directory = tempfile::tempdir().unwrap();
        let mut manager = crate::save_load::SaveFileManager::with_save_directory(directory.path());
        manager.init().unwrap();
        manager
            .save_game("ordered", &source, &canonical_save_info())
            .unwrap();
        let decoded = manager.load_game_snapshot("ordered").unwrap();
        assert_eq!(
            source
                .host_objects()
                .registered_keys()
                .copied()
                .collect::<Vec<_>>(),
            [ObjectId(500), ObjectId(2), ObjectId(90)]
        );
        assert_eq!(decoded.snapshot.objects.len(), 3);
        let mut restored = canonical_save_fixture(false);
        crate::save_load::SnapshotBuilder::new()
            .restore_from_snapshot(&decoded.snapshot, &mut restored)
            .unwrap();
        // Original Xfer loads head→next and registerObject prepends each row.
        // This exact reverse distinguishes preserved OLST from sorted-ID fallback.
        assert_eq!(
            restored
                .host_objects()
                .registered_keys()
                .copied()
                .collect::<Vec<_>>(),
            [ObjectId(90), ObjectId(2), ObjectId(500)]
        );
        assert_eq!(restored.get_frame(), 77);
        for id in [500, 2, 90] {
            let actor = restored.host_objects().lookup(&ObjectId(id)).unwrap();
            assert_eq!(actor.owner_player_id, Some(1));
            assert_eq!(actor.health.current, 100.);
            assert_eq!(actor.get_position(), glam::Vec3::new(id as f32, 0., 0.));
        }
        assert_eq!(
            source
                .host_objects()
                .registered_keys()
                .copied()
                .collect::<Vec<_>>(),
            [ObjectId(500), ObjectId(2), ObjectId(90)]
        );
    }

    #[test]
    fn unfinished_constructor_public_read_aliases_and_writer_refusal_preserve_existing_save() {
        let mut world = canonical_save_fixture(true);
        let directory = tempfile::tempdir().unwrap();
        let mut manager = crate::save_load::SaveFileManager::with_save_directory(directory.path());
        manager.init().unwrap();
        manager
            .save_game("existing", &world, &canonical_save_info())
            .unwrap();
        let path = manager.get_save_path("existing");
        let bytes = std::fs::read(&path).unwrap();
        let id = ObjectId(2);
        let actor = world.objects.remove(&id).unwrap();
        let generation = actor.visual_object_generation;
        let token = std::sync::Arc::downgrade(&actor.visual_draw_identity_token);
        let receipt = world.objects.admit_constructor_lookup(actor).unwrap();
        {
            let host = world.host_objects().lookup(&id).unwrap();
            let alias = world.get_objects().lookup(&id).unwrap();
            assert!(std::ptr::eq(host, alias));
            assert_eq!(host.visual_object_generation, generation);
            assert!(token.ptr_eq(&std::sync::Arc::downgrade(&host.visual_draw_identity_token)));
            assert_eq!(
                world
                    .host_objects()
                    .registered_keys()
                    .copied()
                    .collect::<Vec<_>>(),
                [ObjectId(500), ObjectId(90)]
            );
            assert_eq!(world.get_objects().registered_len(), 2);
            assert!(
                world
                    .get_objects()
                    .registered_values()
                    .all(|actor| actor.id != id)
            );
        }
        for name in ["existing", "not-created"] {
            let error = manager
                .save_game(name, &world, &canonical_save_info())
                .unwrap_err();
            assert!(
                error.to_string().contains("unfinished Object constructor"),
                "{error}"
            );
        }
        assert_eq!(std::fs::read(&path).unwrap(), bytes);
        assert!(!manager.get_save_path("not-created").exists());
        assert_eq!(
            std::fs::read_dir(directory.path().join("temp"))
                .unwrap()
                .count(),
            0
        );
        assert_eq!(world.get_frame(), 77);
        assert_eq!(
            world
                .host_objects()
                .registered_keys()
                .copied()
                .collect::<Vec<_>>(),
            [ObjectId(500), ObjectId(90)]
        );
        let actor = world.get_objects().lookup(&id).unwrap();
        assert_eq!(actor.visual_object_generation, generation);
        assert_eq!(actor.owner_player_id, Some(1));
        assert_eq!(actor.health.current, 100.);
        assert!(token.ptr_eq(&std::sync::Arc::downgrade(
            &actor.visual_draw_identity_token
        )));
        world.objects.register_constructor_lookup(&receipt).unwrap();
        assert_eq!(
            world
                .get_objects()
                .registered_keys()
                .copied()
                .collect::<Vec<_>>(),
            [ObjectId(2), ObjectId(500), ObjectId(90)]
        );
        assert!(std::ptr::eq(
            world.host_objects().lookup(&id).unwrap(),
            world.get_objects().lookup(&id).unwrap()
        ));
    }
    #[test]
    fn moved_constructor_actor_rejects_original_world_receipt_before_registration() {
        let first_world = std::sync::Arc::new(());
        let second_world = std::sync::Arc::new(());
        let mut first = HostObjectStore::new_for_world(&first_world);
        let mut second = HostObjectStore::new_for_world(&second_world);
        let original = first.admit_constructor_lookup(actor(9)).unwrap();
        let canonical = first.remove(&ObjectId(9)).unwrap();
        let generation = canonical.visual_object_generation;
        let identity = std::sync::Arc::downgrade(&canonical.visual_draw_identity_token);
        let receiving = second.admit_constructor_lookup(canonical).unwrap();
        assert!(second.register_constructor_lookup(&original).is_err());
        assert!(second.ordered_ids().is_empty());
        let retained = second.get(&ObjectId(9)).unwrap();
        assert_eq!(retained.visual_object_generation, generation);
        assert!(identity.ptr_eq(&std::sync::Arc::downgrade(
            &retained.visual_draw_identity_token
        )));
        second.register_constructor_lookup(&receiving).unwrap();
        assert_eq!(second.ordered_ids(), &[ObjectId(9)]);
        assert!(first.ordered_ids().is_empty());
    }

    #[test]
    fn unbound_store_returns_unchanged_constructor_actor_without_admission() {
        let mut store = HostObjectStore::new();
        let object = actor(9);
        let identity = std::sync::Arc::downgrade(&object.visual_draw_identity_token);
        let (rejected, reason) = store.admit_constructor_lookup(object).unwrap_err();
        assert_eq!(
            reason,
            generals_sim::object::ConstructorLookupError::UnboundWorld
        );
        assert_eq!(rejected.id, ObjectId(9));
        assert!(identity.ptr_eq(&std::sync::Arc::downgrade(
            &rejected.visual_draw_identity_token
        )));
        assert!(store.get(&ObjectId(9)).is_none());
        assert!(store.ordered_ids().is_empty());
    }
}

impl super::GameLogic {
    /// The pathfinder together with the player-aware Object read its queries
    /// need: C++ Pathfinder reads `obj->getControllingPlayer()` (human vs
    /// computer logical-extent rules, AIPathfind.cpp:5338-5341), so a path
    /// request must never see a controller-less Object view.
    pub(crate) fn pathfinder_with_host_objects(
        &mut self,
    ) -> (&mut PathfindingSystem, HostObjectRead<'_>) {
        (
            &mut self.pathfinding_system,
            self.objects
                .read()
                .with_players(&self.players)
                .with_team_factory(&self.team_factory),
        )
    }
}
