use generals_main::game_logic::{GameLogic, KindOf, ObjectId, Player, Team};
use generals_main::save_load::SnapshotBuilder;
use generals_main::save_load::snapshot::{
    apply_lifecycle_tail_to_host, capture_lifecycle_tail, decode_lifecycle_tail,
    encode_lifecycle_tail,
};
use glam::Vec3;


fn world(child_template: &str) -> GameLogic {
    let mut world = GameLogic::new();
    world
        .admit_object_ini_sources([(
            "SpawnBehaviorRoster.ini".into(),
            format!(
                "Object OwnedRosterNest\n KindOf = STRUCTURE SELECTABLE\n Body = ActiveBody BodyTag\n MaxHealth = 500\n InitialHealth = 500\n End\n Behavior = SpawnBehavior SpawnTag\n OneShot = Yes\n SpawnNumber = 2\n SpawnTemplateName = {child_template}\n End\n Behavior = DefaultProductionExitUpdate ExitTag\n UnitCreatePoint = X:0 Y:0 Z:0\n NaturalRallyPoint = X:0 Y:0 Z:0\n End\nEnd\nObject {child_template}\n KindOf = INFANTRY SELECTABLE\n Body = ActiveBody BodyTag\n MaxHealth = 100\n InitialHealth = 100\n End\nEnd\n"
            ),
        )])
        .expect("receiving SpawnBehavior definitions are valid");
    world.add_player(Player::new(1, Team::GLA, "Owner", true));
    world
}

fn advance_spawn_updates(world: &mut GameLogic) {
    for _ in 0..32 {
        world.update();
    }
}


fn roster(world: &GameLogic, producer: ObjectId) -> Vec<u32> {
    capture_lifecycle_tail(world)
        .expect("capture lifecycle tail")
        .spawn_behavior_rosters
        .expect("new-format save carries the roster section")
        .into_iter()
        .find(|roster| roster.producer_id == producer.0)
        .expect("supported SpawnBehavior producer is persisted, including its ordered children")
        .children
        .into_iter()
        .map(|child| child.child_id)
        .collect()
}

#[test]
fn spawn_behavior_without_an_exit_creates_no_children() {
    let mut world = GameLogic::new();
    world
        .admit_object_ini_sources([(
            "NoExitSpawn.ini".into(),
            "Object NoExitNest\n KindOf = STRUCTURE SELECTABLE\n Body = ActiveBody BodyTag\n MaxHealth = 500\n InitialHealth = 500\n End\n Behavior = SpawnBehavior SpawnTag\n OneShot = Yes\n SpawnNumber = 2\n SpawnTemplateName = NoExitChild\n End\nEnd\nObject NoExitChild\n KindOf = INFANTRY SELECTABLE\n Body = ActiveBody BodyTag\n MaxHealth = 100\n InitialHealth = 100\n End\nEnd\n".into(),
        )])
        .unwrap();
    world.add_player(Player::new(1, Team::GLA, "Owner", true));
    let nest = world
        .create_object_for_player("NoExitNest", 1, Vec3::ZERO)
        .unwrap();
    advance_spawn_updates(&mut world);
    assert!(
        !world.host_objects().registered_values().any(|object| object.producer_id == Some(nest)),
        "SpawnBehavior requires an authored ExitInterface"
    );
}

#[test]
fn spawner_death_stops_surviving_child_slaved_effects_in_its_own_session() {
    fn session() -> (GameLogic, ObjectId) {
        let mut world = GameLogic::new();
        world.admit_object_ini_sources([("SlavedSpawn.ini".into(),
            "Object Nest\n KindOf = STRUCTURE SELECTABLE\n Body = ActiveBody Body\n MaxHealth = 500\n InitialHealth = 500\n End\n Behavior = SpawnBehavior Spawn\n OneShot = Yes\n SpawnNumber = 1\n SpawnTemplateName = Child\n SpawnedRequireSpawner = No\n End\n Behavior = DefaultProductionExitUpdate ExitTag\n UnitCreatePoint = X:0 Y:0 Z:0\n NaturalRallyPoint = X:0 Y:0 Z:0\n End\nEnd\nObject Child\n KindOf = VEHICLE SELECTABLE\n Body = ActiveBody Body\n MaxHealth = 100\n InitialHealth = 100\n End\n Behavior = SlavedUpdate Slave\n GuardMaxRange = 35\n End\nEnd\n".into())]).unwrap();
        world.add_player(Player::new(1, Team::GLA, "Owner", true));
        let parent = world
            .create_object_for_player("Nest", 1, Vec3::ZERO)
            .unwrap();
        advance_spawn_updates(&mut world);
        (world, parent)
    }
    let (mut owner, parent) = session();
    let (peer, peer_parent) = session();
    let child = ObjectId(roster(&owner, parent)[0]);
    assert_eq!(roster(&peer, peer_parent), vec![child.0]);
    for world in [&owner, &peer] {
        let object = world.host_object(child).unwrap();
        assert!(object.is_kind_of(KindOf::Vehicle));
        assert!(object.status.unselectable, "onEnslave ran at creation");
        assert!(!object.status.disabled_held, "an ordinary exit does not hold the child");
    }
    let snapshot = SnapshotBuilder::new()
        .create_world_snapshot(&owner)
        .unwrap();
    let (mut restored, _) = session();
    SnapshotBuilder::new()
        .restore_from_snapshot(&snapshot, &mut restored)
        .unwrap();
    owner.apply_host_damage(parent, 1000.0);
    restored.apply_host_damage(parent, 1000.0);
    let survivor = owner.host_object(child).unwrap();
    assert!(!survivor.status.unselectable);
    assert!(!survivor.status.disabled_held);
    assert_eq!(survivor.producer_id, None);
    let restored_child = restored.host_object(child).unwrap();
    assert!(!restored_child.status.unselectable);
    assert!(!restored_child.status.disabled_held);
    assert_eq!(restored_child.producer_id, None);
    let peer_child = peer.host_object(child).unwrap();
    assert!(peer_child.status.unselectable);
    assert!(!peer_child.status.disabled_held);
    assert_eq!(peer_child.producer_id, Some(peer_parent));
}

#[test]
fn required_spawn_children_run_their_own_slow_death_in_the_receiving_world() {
    fn session() -> (GameLogic, ObjectId) {
        let mut world = GameLogic::new();
        world.admit_object_ini_sources([("RequiredSpawn.ini".into(),
            "Object Nest\n KindOf = STRUCTURE SELECTABLE\n Body = ActiveBody Body\n MaxHealth = 500\n InitialHealth = 500\n End\n Behavior = SpawnBehavior Spawn\n OneShot = Yes\n SpawnNumber = 2\n SpawnTemplateName = Child\n SpawnedRequireSpawner = Yes\n End\n Behavior = DefaultProductionExitUpdate ExitTag\n UnitCreatePoint = X:0 Y:0 Z:0\n NaturalRallyPoint = X:0 Y:0 Z:0\n End\nEnd\nObject Child\n KindOf = INFANTRY SELECTABLE\n Body = ActiveBody Body\n MaxHealth = 100\n InitialHealth = 100\n End\n Behavior = SlowDeathBehavior Death\n DestructionDelay = 3000\n End\nEnd\n".into())]).unwrap();
        world.add_player(Player::new(1, Team::GLA, "Owner", true));
        let parent = world
            .create_object_for_player("Nest", 1, Vec3::ZERO)
            .unwrap();
        advance_spawn_updates(&mut world);
        (world, parent)
    }
    let (mut owner, parent) = session();
    let (peer, peer_parent) = session();
    let children = roster(&owner, parent);
    assert_eq!(children.len(), 2);
    assert_eq!(children, roster(&peer, peer_parent));
    let snapshot = SnapshotBuilder::new()
        .create_world_snapshot(&owner)
        .unwrap();
    let (mut restored, _) = session();
    SnapshotBuilder::new()
        .restore_from_snapshot(&snapshot, &mut restored)
        .unwrap();
    assert_eq!(roster(&restored, parent), children);
    owner.apply_host_damage(parent, 1000.0);
    restored.apply_host_damage(parent, 1000.0);
    for child in children {
        let owner_child = owner
            .host_object(ObjectId(child))
            .expect("slow death retains the child");
        assert!(owner_child.slow_death.is_some(), "child death behavior ran");
        assert!(owner_child.producer_id.is_none());
        let restored_child = restored.host_object(ObjectId(child)).unwrap();
        assert!(restored_child.slow_death.is_some());
        assert!(restored_child.producer_id.is_none());
        let peer_child = peer.host_object(ObjectId(child)).unwrap();
        assert!(peer_child.slow_death.is_none());
        assert_eq!(peer_child.producer_id, Some(peer_parent));
    }
}

#[test]
fn spawn_behavior_children_are_admitted_in_order_and_restored_per_receiving_world() {
    let mut owner = world("OwnerRosterSoldier");
    let mut peer = world("PeerRosterSoldier");
    let owner_nest = owner
        .create_object_for_player("OwnedRosterNest", 1, Vec3::ZERO)
        .expect("owner SpawnBehavior admitted");
    let peer_nest = peer
        .create_object_for_player("OwnedRosterNest", 1, Vec3::ZERO)
        .expect("peer SpawnBehavior admitted");
    advance_spawn_updates(&mut owner);
    advance_spawn_updates(&mut peer);
    assert_eq!(owner_nest, peer_nest, "sessions deliberately reuse IDs");

    let owner_children = roster(&owner, owner_nest);
    let peer_children = roster(&peer, peer_nest);
    assert_eq!(owner_children.len(), 2);
    assert_eq!(peer_children.len(), 2);
    assert_eq!(owner_children, peer_children);
    assert!(owner_children[0] < owner_children[1]);
    assert_eq!(
        owner
            .host_object(ObjectId(owner_children[0]))
            .unwrap()
            .template_name,
        "OwnerRosterSoldier"
    );
    assert_eq!(
        peer.host_object(ObjectId(peer_children[0]))
            .unwrap()
            .template_name,
        "PeerRosterSoldier"
    );

    // A normal production/OCL object can share both the producer link and
    // template name without being a SpawnBehavior child. New-format roster
    // membership is authoritative; do not infer membership from that link.
    let ordinary = owner
        .create_object_for_player("OwnerRosterSoldier", 1, Vec3::ZERO)
        .expect("ordinary same-template object admitted");
    owner.host_object_mut(ordinary).unwrap().producer_id = Some(owner_nest);
    assert!(!roster(&owner, owner_nest).contains(&ordinary.0));

    let mut snapshot = SnapshotBuilder::new()
        .create_world_snapshot(&owner)
        .expect("capture complete world snapshot with ordered roster");
    // Exercise a valid stale ObjectId whose little-endian bytes equal a
    // suffix tag used elsewhere in lifecycle persistence. SBRO hex framing
    // keeps those bytes opaque to the older magic-window suffix scanners.
    let mut framed_tail = decode_lifecycle_tail(&snapshot.lifecycle_tail).unwrap();
    let modern_prefix = encode_lifecycle_tail(&framed_tail);
    let suffix = snapshot.lifecycle_tail[modern_prefix.len()..].to_vec();
    let spawn_roster = framed_tail
        .spawn_behavior_rosters
        .as_mut()
        .unwrap()
        .iter_mut()
        .find(|roster| roster.producer_id == owner_nest.0)
        .unwrap();
    let mut magic_id = spawn_roster.children[0].clone();
    magic_id.child_id = u32::from_le_bytes(*b"WSFL");
    magic_id.tombstone = true;
    spawn_roster.children.push(magic_id.clone());
    let mut encoded_prefix = encode_lifecycle_tail(&framed_tail);
    assert!(!encoded_prefix.windows(4).any(|window| window == b"WSFL"));
    encoded_prefix.extend_from_slice(&suffix);
    snapshot.lifecycle_tail = encoded_prefix;
    let mut restored = world("OwnerRosterSoldier");
    SnapshotBuilder::new()
        .restore_from_snapshot(&snapshot, &mut restored)
        .expect("restore complete world snapshot against matching catalog");
    let restored_children = roster(&restored, owner_nest);
    assert_eq!(&restored_children[..owner_children.len()], &owner_children);
    assert_eq!(restored_children.last(), Some(&magic_id.child_id));
    assert_eq!(
        restored.host_object(ordinary).unwrap().producer_id,
        Some(owner_nest)
    );
    assert_eq!(roster(&peer, peer_nest), peer_children);
}

#[test]
fn stale_spawn_behavior_ids_survive_save_restore_as_unbound_tombstones() {
    let mut owner = world("OwnerRosterSoldier");
    let nest = owner
        .create_object_for_player("OwnedRosterNest", 1, Vec3::ZERO)
        .expect("owner SpawnBehavior admitted");
    advance_spawn_updates(&mut owner);
    let saved_order = roster(&owner, nest);
    let stale_id = ObjectId(saved_order[0]);
    let missing_id = ObjectId(0x1234_5678);
    let mut detached = capture_lifecycle_tail(&owner).expect("capture source roster");
    let detached_roster = detached
        .spawn_behavior_rosters
        .as_mut()
        .unwrap()
        .iter_mut()
        .find(|roster| roster.producer_id == nest.0)
        .unwrap();
    let entry = detached_roster
        .children
        .iter_mut()
        .find(|child| child.child_id == stale_id.0)
        .unwrap();
    entry.tombstone = true;
    let mut missing = entry.clone();
    missing.child_id = missing_id.0;
    detached_roster.children.push(missing);

    let mut receiver = world("OwnerRosterSoldier");
    let receiver_nest = receiver
        .create_object_for_player("OwnedRosterNest", 1, Vec3::ZERO)
        .unwrap();
    advance_spawn_updates(&mut receiver);
    assert_eq!(receiver_nest, nest);
    assert_eq!(roster(&receiver, receiver_nest), saved_order);
    apply_lifecycle_tail_to_host(&detached, &mut receiver).expect("apply detached tombstone");
    assert!(
        receiver.host_object(stale_id).is_some(),
        "same-ID live birth remains intact"
    );
    assert!(
        !capture_lifecycle_tail(&owner)
            .unwrap()
            .spawn_behavior_rosters
            .unwrap()[0]
            .children
            .iter()
            .find(|child| child.child_id == stale_id.0)
            .unwrap()
            .tombstone
    );
    let saved_tail = capture_lifecycle_tail(&receiver).expect("capture stale child as tombstone");
    assert!(receiver.host_object(missing_id).is_none());
    assert!(
        saved_tail
            .spawn_behavior_rosters
            .as_ref()
            .unwrap()
            .iter()
            .find(|roster| roster.producer_id == nest.0)
            .unwrap()
            .children
            .iter()
            .find(|child| child.child_id == stale_id.0)
            .unwrap()
            .tombstone
    );

    // A tombstone must stay unbound even when a same-ID live object exists.
    let encoded = encode_lifecycle_tail(&saved_tail);
    assert!(
        decode_lifecycle_tail(&encoded)
            .unwrap()
            .spawn_behavior_rosters
            .unwrap()[0]
            .children
            .iter()
            .find(|child| child.child_id == stale_id.0)
            .unwrap()
            .tombstone
    );
    let snapshot = SnapshotBuilder::new()
        .create_world_snapshot(&receiver)
        .unwrap();
    let mut restored = world("OwnerRosterSoldier");
    SnapshotBuilder::new()
        .restore_from_snapshot(&snapshot, &mut restored)
        .unwrap();
    let restored_tail = capture_lifecycle_tail(&restored).unwrap();
    assert!(restored.host_object(stale_id).is_some());
    assert!(restored.host_object(missing_id).is_none());
    assert!(
        restored_tail
            .spawn_behavior_rosters
            .as_ref()
            .unwrap()
            .iter()
            .find(|roster| roster.producer_id == nest.0)
            .unwrap()
            .children
            .iter()
            .find(|child| child.child_id == missing_id.0)
            .unwrap()
            .tombstone
    );
    assert!(
        restored_tail
            .spawn_behavior_rosters
            .unwrap()
            .iter()
            .find(|roster| roster.producer_id == nest.0)
            .unwrap()
            .children
            .iter()
            .find(|child| child.child_id == stale_id.0)
            .unwrap()
            .tombstone
    );
}

#[test]
fn producer_cleared_surviving_spawn_remains_live_in_roster_after_restore() {
    let mut owner = world("OwnerRosterSoldier");
    let nest = owner
        .create_object_for_player("OwnedRosterNest", 1, Vec3::ZERO)
        .expect("owner SpawnBehavior admitted");
    advance_spawn_updates(&mut owner);
    let children = roster(&owner, nest);

    // C++ SpawnBehavior::onDie clears each child's producer but retains its
    // m_spawnIDs entry when SpawnedRequireSpawner is No. Model the state at
    // the save boundary after that callback.
    for child in &children {
        owner.host_object_mut(ObjectId(*child)).unwrap().producer_id = None;
    }
    let captured = capture_lifecycle_tail(&owner).expect("capture retained roster entries");
    let records = &captured.spawn_behavior_rosters.as_ref().unwrap()[0].children;
    assert!(records.iter().all(|child| !child.tombstone));

    let snapshot = SnapshotBuilder::new()
        .create_world_snapshot(&owner)
        .expect("capture full snapshot with producer-cleared survivors");
    let mut restored = world("OwnerRosterSoldier");
    SnapshotBuilder::new()
        .restore_from_snapshot(&snapshot, &mut restored)
        .expect("restore uses roster birth identity rather than producer link");
    assert_eq!(roster(&restored, nest), children);
    for child in children {
        assert_eq!(
            restored.host_object(ObjectId(child)).unwrap().producer_id,
            None
        );
    }
}

fn no_spawner_world() -> GameLogic {
    let mut world = GameLogic::new();
    world
        .admit_object_ini_sources([(
            "PlainSnapshot.ini".into(),
            "Object PlainSnapshotObject\n KindOf = INFANTRY SELECTABLE\n Body = ActiveBody BodyTag\n MaxHealth = 100\n InitialHealth = 100\n End\nEnd\n".into(),
        )])
        .expect("plain receiving object valid");
    world.add_player(Player::new(1, Team::GLA, "Plain", true));
    world
        .create_object_for_player("PlainSnapshotObject", 1, Vec3::ZERO)
        .unwrap();
    world
}


#[test]
fn no_spawner_snapshot_and_legacy_no_roster_restore_remain_supported() {
    let source = no_spawner_world();
    let builder = SnapshotBuilder::new();
    let modern = builder
        .create_world_snapshot(&source)
        .expect("capture no-spawner complete snapshot");
    let mut restored = no_spawner_world();
    builder
        .restore_from_snapshot(&modern, &mut restored)
        .expect("restore no-spawner complete snapshot");

    let decoded = decode_lifecycle_tail(&modern.lifecycle_tail).expect("decode modern tail");
    let mut old_tail = decoded.clone();
    old_tail.spawn_behavior_rosters = None;
    let old_prefix = encode_lifecycle_tail(&old_tail);
    let modern_prefix = encode_lifecycle_tail(&decoded);
    let mut legacy = old_prefix;
    legacy.extend_from_slice(&modern.lifecycle_tail[modern_prefix.len()..]);
    let mut legacy_snapshot = modern;
    legacy_snapshot.lifecycle_tail = legacy;
    let mut legacy_receiver = no_spawner_world();
    builder
        .restore_from_snapshot(&legacy_snapshot, &mut legacy_receiver)
        .expect("legacy tails without a supported spawner remain accepted");
}

#[test]
fn legacy_supported_spawner_restore_fails_before_replacing_live_objects() {
    let mut owner = world("OwnerRosterSoldier");
    let owner_nest = owner
        .create_object_for_player("OwnedRosterNest", 1, Vec3::ZERO)
        .expect("owner SpawnBehavior admitted");
    advance_spawn_updates(&mut owner);
    let builder = SnapshotBuilder::new();
    let modern = builder
        .create_world_snapshot(&owner)
        .expect("capture source snapshot");
    let decoded = decode_lifecycle_tail(&modern.lifecycle_tail).expect("decode modern tail");
    let mut old_tail = decoded.clone();
    old_tail.spawn_behavior_rosters = None;
    let old_prefix = encode_lifecycle_tail(&old_tail);
    let modern_prefix = encode_lifecycle_tail(&decoded);
    let mut legacy = old_prefix;
    legacy.extend_from_slice(&modern.lifecycle_tail[modern_prefix.len()..]);
    let mut legacy_snapshot = modern;
    legacy_snapshot.lifecycle_tail = legacy;

    let mut receiver = world("OwnerRosterSoldier");
    let receiver_nest = receiver
        .create_object_for_player("OwnedRosterNest", 1, Vec3::ZERO)
        .expect("peer SpawnBehavior admitted");
    advance_spawn_updates(&mut receiver);
    assert_eq!(receiver_nest, owner_nest);
    let receiver_child = ObjectId(roster(&receiver, receiver_nest)[0]);
    receiver
        .host_object_mut(receiver_child)
        .unwrap()
        .health
        .current = 83.0;
    let receiver_health = receiver.host_object(receiver_child).unwrap().health.current;
    let error = builder
        .restore_from_snapshot(&legacy_snapshot, &mut receiver)
        .expect_err("supported spawner cannot be guessed from legacy save");
    assert!(
        format!("{error}").contains("no ordered SpawnBehavior roster"),
        "unexpected preflight error: {error}"
    );
    assert_eq!(
        receiver.host_object(receiver_child).unwrap().health.current,
        receiver_health,
        "legacy preflight must fail before replacing receiving children"
    );
}


#[test]
fn hive_body_without_spawn_behavior_forwards_to_closest_contained_rider() {
    use generals_main::game_logic::host_base_defense::HostHiveDamageClass;
    // C++ HiveStructureBody.cpp:79-100: no SpawnBehavior interface but a
    // Contain module forwards to getClosestRider (FROM_CENTER_2D, first wins
    // ties) and swallows only when no rider remains. Selection is by the
    // authored Body, not the template name.
    let ini = "Object HiveRiderBunker\n KindOf = STRUCTURE SELECTABLE\n TransportSlotCount = 1\n Body = HiveStructureBody BunkerBody\n PropagateDamageTypesToSlavesWhenExisting = SMALL_ARMS\n SwallowDamageTypesIfSlavesNotExisting = SMALL_ARMS\n MaxHealth = 1000\n InitialHealth = 1000\n End\n Behavior = TransportContain BunkerContain\n Slots = 5\n End\nEnd\nObject HiveRider\n KindOf = INFANTRY SELECTABLE\n TransportSlotCount = 1\n Body = ActiveBody RiderBody\n MaxHealth = 100\n InitialHealth = 100\n End\nEnd\nObject HiveShooter\n KindOf = VEHICLE SELECTABLE\n Body = ActiveBody ShooterBody\n MaxHealth = 100\n InitialHealth = 100\n End\nEnd\n";
    let mut world = GameLogic::new();
    world
        .admit_object_ini_sources([("HiveRider.ini".into(), ini.to_string())])
        .unwrap();
    world.add_player(Player::new(1, Team::GLA, "Owner", true));
    let bunker = world
        .create_object_for_player("HiveRiderBunker", 1, Vec3::ZERO)
        .unwrap();
    let far = world
        .create_object_for_player("HiveRider", 1, Vec3::new(-50.0, 0.0, 0.0))
        .unwrap();
    let near = world
        .create_object_for_player("HiveRider", 1, Vec3::new(40.0, 0.0, 0.0))
        .unwrap();
    let shooter = world
        .create_object_for_player("HiveShooter", 1, Vec3::new(100.0, 0.0, 0.0))
        .unwrap();
    for rider in [far, near] {
        assert!(world.host_object_mut(bunker).unwrap().add_occupant(rider));
    }
    world.apply_host_hive_damage_from(
        bunker,
        30.0,
        HostHiveDamageClass::PropagateToSlaves,
        None,
        Some(shooter),
    );
    assert_eq!(world.host_object(bunker).unwrap().health.current, 1000.0);
    assert_eq!(world.host_object(near).unwrap().health.current, 70.0);
    assert_eq!(world.host_object(far).unwrap().health.current, 100.0);
    // Missing source falls through to the structure.
    world.apply_host_hive_damage_from(
        bunker,
        10.0,
        HostHiveDamageClass::PropagateToSlaves,
        None,
        None,
    );
    assert_eq!(world.host_object(bunker).unwrap().health.current, 990.0);
    // An empty Contain with a resolved source swallows.
    let empty = world
        .create_object_for_player("HiveRiderBunker", 1, Vec3::ZERO)
        .unwrap();
    world.apply_host_hive_damage_from(
        empty,
        10.0,
        HostHiveDamageClass::PropagateToSlaves,
        None,
        Some(shooter),
    );
    assert_eq!(world.host_object(empty).unwrap().health.current, 1000.0);
}

#[test]
fn spawn_weapon_stealth_checks_child_eligibility_and_reveals_the_pack() {
    fn session() -> (GameLogic, ObjectId, Vec<u32>) {
        let mut world = GameLogic::new();
        world.admit_object_ini_sources([("SpawnStealth.ini".into(),
            "Object StealthNest\n KindOf = STRUCTURE SELECTABLE SPAWNS_ARE_THE_WEAPONS\n Body = ActiveBody Body\n MaxHealth = 500\n InitialHealth = 500\n End\n Behavior = DefaultProductionExitUpdate Exit\n End\n Behavior = SpawnBehavior Spawn\n OneShot = Yes\n SpawnNumber = 2\n SpawnTemplateName = StealthChild\n End\n Behavior = StealthUpdate Cloak\n StealthDelay = 0\n InnateStealth = Yes\n End\nEnd\nObject StealthChild\n KindOf = INFANTRY SELECTABLE\n Body = ActiveBody Body\n MaxHealth = 100\n InitialHealth = 100\n End\n Behavior = StealthUpdate Cloak\n StealthDelay = 2000\n InnateStealth = Yes\n End\nEnd\n".into())]).unwrap();
        world.add_player(Player::new(1, Team::GLA, "Owner", true));
        let parent = world.create_object_for_player("StealthNest", 1, Vec3::ZERO).unwrap();
        advance_spawn_updates(&mut world);
        let children = roster(&world, parent);
        assert_eq!(children.len(), 2);
        (world, parent, children)
    }

    let (mut owner, parent, children) = session();
    let (mut peer, peer_parent, peer_children) = session();
    assert_eq!(parent, peer_parent);
    assert_eq!(children, peer_children);
    let first = ObjectId(children[0]);
    let second = ObjectId(children[1]);
    owner.host_object_mut(second).unwrap().innate_stealth = false;
    owner.update_stealth_and_detection();
    assert!(!owner.host_object(parent).unwrap().status.stealthed);
    assert_eq!(owner.host_object(first).unwrap().detection_expires_frame, owner.get_current_frame() as u32 + 60);
    assert_eq!(owner.host_object(second).unwrap().detection_expires_frame, owner.get_current_frame() as u32 + 60);
    assert!(!peer.host_object(first).unwrap().status.detected);

    // Eligibility, not the current cloak or detection bit, controls the nest.
    owner.host_object_mut(second).unwrap().innate_stealth = true;
    owner.host_object_mut(first).unwrap().set_status_stealthed(false);
    owner.update_stealth_and_detection();
    assert!(owner.host_object(parent).unwrap().status.stealthed);
    peer.update_stealth_and_detection();
    assert!(peer.host_object(peer_parent).unwrap().status.stealthed);
}

#[test]
fn temporary_stealth_grant_expires_after_awake_updates() {
    let mut world = GameLogic::new();
    world.admit_object_ini_sources([("TemporaryStealth.ini".into(),
        "Object TemporaryCloakUnit\n KindOf = VEHICLE SELECTABLE\n Body = ActiveBody Body\n MaxHealth = 100\n InitialHealth = 100\n End\n Behavior = AIUpdateInterface Brain\n End\n Behavior = StealthUpdate Cloak\n StealthDelay = 0\n InnateStealth = No\n GrantedBySpecialPower = Yes\n End\nEnd\n".into())]).unwrap();
    world.add_player(Player::new(1, Team::GLA, "Owner", true));
    let id = world.create_object_for_player("TemporaryCloakUnit", 1, Vec3::ZERO).unwrap();
    let frame = world.get_frame();
    let object = world.host_object_mut(id).unwrap();
    object.apply_grant_stealth(frame);
    object.stealth_frames_granted = 2;
    object.set_status_disabled_emp(true);

    world.update_stealth_and_detection();
    assert_eq!(world.host_object(id).unwrap().stealth_frames_granted, 2);
    assert!(world.host_object(id).unwrap().innate_stealth);
    world.host_object_mut(id).unwrap().set_status_disabled_emp(false);

    world.update_stealth_and_detection();
    assert_eq!(world.host_object(id).unwrap().stealth_frames_granted, 1);
    assert!(world.host_object(id).unwrap().innate_stealth);
    world.update_stealth_and_detection();
    let object = world.host_object(id).unwrap();
    assert_eq!(object.stealth_frames_granted, 0);
    assert!(!object.innate_stealth);
    assert!(!object.status.stealthed);
    assert!(object.stealth_update_awake);
    world.update_stealth_and_detection();
    assert!(!world.host_object(id).unwrap().stealth_update_awake);

    let commanded = world.create_object_for_player("TemporaryCloakUnit", 1, Vec3::ZERO).unwrap();
    let frame = world.get_frame();
    let object = world.host_object_mut(commanded).unwrap();
    assert!(object.has_ai_update_interface());
    object.apply_grant_stealth(frame);
    object.stealth_frames_granted = 10;
    object.last_command_source = 0; // CMD_FROM_PLAYER
    world.update_stealth_and_detection();
    let object = world.host_object(commanded).unwrap();
    assert_eq!(object.stealth_frames_granted, 0);
    assert!(!object.innate_stealth);
    assert!(!object.status.stealthed);
}
