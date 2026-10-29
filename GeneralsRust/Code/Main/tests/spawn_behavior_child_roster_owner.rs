use generals_main::game_logic::{GameLogic, ObjectId, Player, Team};
use generals_main::save_load::SnapshotBuilder;
use generals_main::save_load::snapshot::{
    capture_lifecycle_tail, decode_lifecycle_tail, encode_lifecycle_tail,
};
use glam::Vec3;

fn world(child_template: &str) -> GameLogic {
    let mut world = GameLogic::new();
    world
        .admit_object_ini_sources([(
            "SpawnBehaviorRoster.ini".into(),
            format!(
                "Object OwnedRosterNest\n KindOf = STRUCTURE SELECTABLE\n Body = ActiveBody BodyTag\n MaxHealth = 500\n InitialHealth = 500\n End\n Behavior = SpawnBehavior SpawnTag\n OneShot = Yes\n SpawnNumber = 2\n SpawnTemplateName = {child_template}\n End\nEnd\nObject {child_template}\n KindOf = INFANTRY SELECTABLE\n Body = ActiveBody BodyTag\n MaxHealth = 100\n InitialHealth = 100\n End\nEnd\n"
            ),
        )])
        .expect("receiving SpawnBehavior definitions are valid");
    world.add_player(Player::new(1, Team::GLA, "Owner", true));
    world
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
fn spawn_behavior_children_are_admitted_in_order_and_restored_per_receiving_world() {
    let mut owner = world("OwnerRosterSoldier");
    let mut peer = world("PeerRosterSoldier");
    let owner_nest = owner
        .create_object_for_player("OwnedRosterNest", 1, Vec3::ZERO)
        .expect("owner SpawnBehavior admitted");
    let peer_nest = peer
        .create_object_for_player("OwnedRosterNest", 1, Vec3::ZERO)
        .expect("peer SpawnBehavior admitted");
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
    let saved_order = roster(&owner, nest);
    let stale_id = ObjectId(saved_order[0]);
    owner
        .objects
        .remove(&stale_id)
        .expect("simulate removal without onSpawnDeath callback");
    let saved_tail = capture_lifecycle_tail(&owner).expect("capture stale child as tombstone");
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

    // The direct HashMap removal above intentionally bypasses the native
    // partition teardown, so only the lifecycle tail is a valid witness for
    // this synthetic stale-link setup. Full SnapshotBuilder round trips of
    // serialized tombstones are exercised by the main test's crafted entry.
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
}

#[test]
fn producer_cleared_surviving_spawn_remains_live_in_roster_after_restore() {
    let mut owner = world("OwnerRosterSoldier");
    let nest = owner
        .create_object_for_player("OwnedRosterNest", 1, Vec3::ZERO)
        .expect("owner SpawnBehavior admitted");
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
