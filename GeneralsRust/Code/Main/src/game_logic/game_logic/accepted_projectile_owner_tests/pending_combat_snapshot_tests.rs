//! Real authored accepted fire -> owned materialization -> snapshot continuation.
//! The parent fixture binds parsed named weapons through actual world admission.
use super::*;
use crate::game_logic::combat::live_projectileless_delayed_count_for_test;
use crate::save_load::snapshot::{
    SnapshotBuilder, WORLD_SNAPSHOT_BINCODE_VERSION, WORLD_SNAPSHOT_DIRECT_XFER_VERSION,
    WorldSnapshot,
};

fn encoded_snapshot(world: &GameLogic) -> WorldSnapshot {
    let captured = SnapshotBuilder::new()
        .create_world_snapshot(world)
        .expect("capture actual driving world");
    let wire = bincode_legacy::serialize(&captured).expect("serialize actual schema");
    bincode_legacy::deserialize(&wire).expect("decode actual schema")
}

fn apply_due(world: &mut GameLogic) {
    apply_ready_projectileless_delayed_damage(
        &mut world.combat_system,
        &mut world.objects,
        world.frame,
        Some(&world.players),
        &mut world.health_events,
        &mut world.ownership_events,
    );
}

fn fire_children(world: &GameLogic) -> Vec<(ObjectId, String, Option<ObjectId>)> {
    let mut children: Vec<_> = world
        .objects
        .registered()
        .filter(|(_, child)| {
            matches!(
                child.template_name.as_str(),
                "AcceptedFireFirstChild" | "AcceptedFireSecondChild"
            )
        })
        .map(|(id, child)| {
            (
                *id,
                child.template_name.clone(),
                child
                    .producer_id
                    .filter(|producer| world.objects.contains_key(producer)),
            )
        })
        .collect();
    children.sort_by_key(|(id, _, _)| id.0);
    children
}

#[test]
fn pending_combat_snapshot_accepted_fifo_replaces_in_place_queue_and_keeps_retired_source() {
    let (mut source_world, source, target) = world(FLIGHT_FIRST);
    {
        let (source_object, fixture_partition_source_object) =
            source_world.host_object_and_partition_mut(source).unwrap();
        source_object.set_orientation_with_partition_owner(1.25, fixture_partition_source_object);
        source_object.movement.velocity = Vec3::new(2.0, 0.0, 3.0);
    }
    let other = admit(
        &mut source_world,
        "SnapshotOtherShooter",
        Some(FLIGHT_SECOND),
        Team::USA,
        Vec3::new(0.0, 0.0, 1.0),
    );
    let victim_pos = source_world.objects[&target].get_position();
    source_world
        .objects
        .get_mut(&other)
        .unwrap()
        .prev_victim_pos = Some(victim_pos);
    accept(&mut source_world, source, target, 10.0);
    accept(&mut source_world, other, target, 10.0);
    accept(&mut source_world, source, target, 12.0);
    assert_eq!(source_world.combat_system.projectile_count(), 3);
    let children = fire_children(&source_world);
    assert_eq!(
        children
            .iter()
            .map(|(_, name, producer)| (name.as_str(), *producer))
            .collect::<Vec<_>>(),
        vec![
            ("AcceptedFireFirstChild", Some(source)),
            ("AcceptedFireSecondChild", Some(other)),
            ("AcceptedFireFirstChild", Some(source))
        ]
    );
    assert!(source_world.combat_system.take_fire_ocl().is_empty());
    source_world.destroy_object(source);
    source_world.process_destroy_list_if_needed();
    assert!(!source_world.objects.contains_key(&source));
    // C++ getProducer resolves through the current roster. The original
    // producer was present at fire time and is absent at the save boundary.
    let children = fire_children(&source_world);
    assert_eq!(source_world.objects[&target].health.current, 100.0);
    let captured = encoded_snapshot(&source_world);
    assert!(!captured.objects.contains_key(&source));
    assert_eq!(
        source_world.combat_system.projectile_count(),
        3,
        "snapshot capture must not consume real flight"
    );
    let (mut destination, reused_source, reused_target) = world(FLIGHT_SECOND);
    assert_eq!((source, target), (reused_source, reused_target));
    accept(&mut destination, reused_source, reused_target, 10.0);
    assert_eq!(destination.combat_system.projectile_count(), 1);
    finish_accepted_flight(&mut source_world);
    assert_eq!(source_world.objects[&target].health.current, 53.0);
    destination.templates = source_world.templates.clone();
    SnapshotBuilder::new()
        .restore_from_snapshot(&captured, &mut destination)
        .expect("replace receiving destination flight, not append its prior shot");
    assert!(!destination.objects.contains_key(&source));
    assert_eq!(destination.objects[&target].health.current, 100.0);
    assert_eq!(destination.combat_system.projectile_count(), 3);
    assert_eq!(
        fire_children(&destination),
        children,
        "acceptance effects were already created and saved, never replayed"
    );
    finish_accepted_flight(&mut destination);
    assert_eq!(destination.objects[&target].health.current, 53.0);
    assert_eq!(source_world.objects[&target].health.current, 53.0);
    finish_accepted_flight(&mut destination);
    assert_eq!(destination.objects[&target].health.current, 53.0);
    assert_eq!(fire_children(&destination), children);
    assert!(destination.combat_system.take_fire_ocl().is_empty());
}

const SLOW_FIRST: &str = "SnapshotFiniteFirstWeapon";
const SLOW_SECOND: &str = "SnapshotFiniteSecondWeapon";

#[test]
fn pending_combat_snapshot_finite_deadline_survives_fresh_and_in_place_same_id_loads() {
    let (mut first, source, target) = world(SLOW_FIRST);
    let (mut second, other_source, other_target) = world(SLOW_SECOND);
    assert_eq!((source, target), (other_source, other_target));
    assert_eq!(first.frame, 100);
    assert_eq!(
        first.objects[&source]
            .selected_native_weapon_rule(0)
            .unwrap()
            .weapon_speed
            .to_bits(),
        0x4120_0001
    );
    accept(&mut first, source, target, 10.0);
    accept(&mut second, other_source, other_target, 10.0);
    first.drain_pending_projectiles_into_combat();
    second.drain_pending_projectiles_into_combat();
    assert_eq!(
        live_projectileless_delayed_count_for_test(&first.combat_system),
        1
    );
    assert_eq!(
        live_projectileless_delayed_count_for_test(&second.combat_system),
        1
    );
    assert_eq!(first.objects[&target].health.current, 100.0);
    assert_eq!(second.objects[&other_target].health.current, 100.0);
    assert_eq!(first.combat_system.projectile_count(), 0);
    first.destroy_object(source);
    first.process_destroy_list_if_needed();
    assert!(!first.objects.contains_key(&source));
    let first_save = encoded_snapshot(&first);
    let second_save = encoded_snapshot(&second);

    let mut fresh = new_owned_world();
    fresh.templates = first.templates.clone();
    SnapshotBuilder::new()
        .restore_from_snapshot(&first_save, &mut fresh)
        .expect("fresh continuation");
    SnapshotBuilder::new()
        .restore_from_snapshot(&first_save, &mut first)
        .expect("same-owner in-place continuation");
    SnapshotBuilder::new()
        .restore_from_snapshot(&second_save, &mut second)
        .expect("other same-ID in-place continuation");
    for world in [&mut fresh, &mut first, &mut second] {
        assert_eq!(
            live_projectileless_delayed_count_for_test(&world.combat_system),
            1
        );
        world.set_current_frame(102);
        apply_due(world);
        assert_eq!(
            world.objects[&target].health.current, 100.0,
            "absolute frame103 deadline must still wait"
        );
    }
    fresh.set_current_frame(103);
    apply_due(&mut fresh);
    assert_eq!(fresh.objects[&target].health.current, 83.0);
    assert_eq!(first.objects[&target].health.current, 100.0);
    assert_eq!(second.objects[&other_target].health.current, 100.0);
    first.set_current_frame(103);
    second.set_current_frame(103);
    apply_due(&mut first);
    apply_due(&mut second);
    assert_eq!(first.objects[&target].health.current, 83.0);
    assert_eq!(second.objects[&other_target].health.current, 87.0);
    for world in [&mut fresh, &mut first, &mut second] {
        assert_eq!(
            live_projectileless_delayed_count_for_test(&world.combat_system),
            0
        );
        apply_due(world);
    }
    assert_eq!(fresh.objects[&target].health.current, 83.0);
    assert_eq!(first.objects[&target].health.current, 83.0);
    assert_eq!(second.objects[&other_target].health.current, 87.0);
}

#[test]
fn pending_combat_snapshot_explicit_empty_state_replaces_both_dirty_destination_queues() {
    let (empty, source, target) = world(SLOW_FIRST);
    let captured = encoded_snapshot(&empty);
    let (mut destination, same_source, same_target) = world(SLOW_SECOND);
    assert_eq!((source, target), (same_source, same_target));
    accept(&mut destination, same_source, same_target, 10.0);
    destination.drain_pending_projectiles_into_combat();
    assert_eq!(
        live_projectileless_delayed_count_for_test(&destination.combat_system),
        1
    );
    accept(&mut destination, same_source, same_target, 12.0);
    SnapshotBuilder::new()
        .restore_from_snapshot(&captured, &mut destination)
        .expect("replace exact empty captured state");
    assert_eq!(
        live_projectileless_delayed_count_for_test(&destination.combat_system),
        0
    );
    destination.set_current_frame(110);
    destination.drain_pending_projectiles_into_combat();
    assert_eq!(destination.objects[&target].health.current, 100.0);
    assert_eq!(destination.combat_system.projectile_count(), 0);
    assert!(destination.combat_system.take_fire_ocl().is_empty());
    assert_eq!(destination.objects[&target].health.current, 100.0);
}

#[test]
fn pending_combat_snapshot_rejects_old_schema_before_mutating_live_owner() {
    let (mut source, shooter, target) = world(FLIGHT_FIRST);
    accept(&mut source, shooter, target, 10.0);
    let mut captured = encoded_snapshot(&source);
    captured.version = 22;
    captured.frame_number = 999;
    let (mut destination, own_shooter, own_target) = world(FLIGHT_SECOND);
    accept(&mut destination, own_shooter, own_target, 10.0);
    let observe = |world: &GameLogic| {
        let mut snapshot = encoded_snapshot(world);
        snapshot.timestamp = std::time::UNIX_EPOCH;
        serde_json::to_value(snapshot).expect("canonical serialized simulation observation")
    };
    let before = observe(&destination);
    let result = SnapshotBuilder::new().restore_from_snapshot(&captured, &mut destination);
    assert!(matches!(
        result,
        Err(crate::save_load::SaveLoadError::VersionMismatch {
            expected: WORLD_SNAPSHOT_DIRECT_XFER_VERSION,
            actual: 22
        })
    ));
    assert_eq!(
        observe(&destination),
        before,
        "failed version admission must leave actual receiving state untouched"
    );
    assert_eq!(destination.frame, 100);
    assert_eq!(destination.objects[&own_target].health.current, 100.0);
    assert_eq!(destination.combat_system.projectile_count(), 1);
    finish_accepted_flight(&mut destination);
    assert_eq!(destination.objects[&own_target].health.current, 87.0);
    assert_eq!(source.objects[&target].health.current, 100.0);
    assert_eq!(source.combat_system.projectile_count(), 1);
    finish_accepted_flight(&mut source);
    assert_eq!(source.objects[&target].health.current, 83.0);
}

fn save_info(slot: &str) -> crate::save_load::SaveGameInfo {
    crate::save_load::SaveGameInfo {
        pristine_map_name: None,
        filename: slot.to_string(),
        display_name: "Typed pending combat".to_string(),
        description: "actual authored accepted and finite damage continuation".to_string(),
        map_name: "SnapshotCombatMap".to_string(),
        campaign_side: None,
        mission_number: None,
        save_date: std::time::SystemTime::now(),
        game_version: env!("CARGO_PKG_VERSION").to_string(),
        play_time: std::time::Duration::ZERO,
        difficulty: crate::save_load::GameDifficulty::Medium,
        save_type: crate::save_load::SaveFileType::Normal,
    }
}

#[test]
fn pending_combat_snapshot_common_save_load_continues_both_real_queues() {
    let (mut source, shooter, target) = world(SLOW_FIRST);
    accept(&mut source, shooter, target, 10.0);
    assert_eq!(
        live_projectileless_delayed_count_for_test(&source.combat_system),
        1
    );
    let fast = admit(
        &mut source,
        "SnapshotFastShooter",
        Some(FLIGHT_SECOND),
        Team::USA,
        Vec3::ZERO,
    );
    let victim_pos = source.objects[&target].get_position();
    source.objects.get_mut(&fast).unwrap().prev_victim_pos = Some(victim_pos);
    accept(&mut source, fast, target, 10.0);
    assert_eq!(source.combat_system.projectile_count(), 1);
    assert_eq!(source.objects[&target].health.current, 100.0);
    let children = fire_children(&source);
    assert_eq!(children.len(), 1);
    let dir = tempfile::TempDir::new().expect("owned save directory");
    let mut manager = crate::save_load::SaveFileManager::with_save_directory(dir.path());
    manager.init().expect("real manager init");
    manager
        .save_game("both_queues", &source, &save_info("both_queues"))
        .expect("actual Common named-chunk writer");
    let crate::save_load::DecodedGameSave {
        snapshot: decoded, ..
    } = manager
        .load_game_snapshot("both_queues")
        .expect("actual current decoder");
    let mut restored = new_owned_world();
    restored.templates = source.templates.clone();
    manager
        .restore_game_snapshot(&decoded, &mut restored)
        .expect("actual staging restore route");
    assert_eq!(restored.combat_system.projectile_count(), 1);
    assert_eq!(
        live_projectileless_delayed_count_for_test(&restored.combat_system),
        1
    );
    assert_eq!(fire_children(&restored), children);
    for _ in 0..40 {
        source.frame += 1;
        restored.frame += 1;
        source.update_owned_projectile_impacts(LOGIC_FRAME_TIMESTEP);
        restored.update_owned_projectile_impacts(LOGIC_FRAME_TIMESTEP);
        source.apply_owned_projectileless_delayed_damage();
        restored.apply_owned_projectileless_delayed_damage();
        assert_eq!(restored.frame, source.frame);
        assert_eq!(
            restored.objects[&target].health.current,
            source.objects[&target].health.current
        );
        assert_eq!(
            restored.combat_system.projectile_count(),
            source.combat_system.projectile_count()
        );
        assert_eq!(
            live_projectileless_delayed_count_for_test(&restored.combat_system),
            live_projectileless_delayed_count_for_test(&source.combat_system)
        );
        assert_eq!(fire_children(&restored), children);
        assert!(restored.combat_system.take_fire_ocl().is_empty());
        assert!(source.combat_system.take_fire_ocl().is_empty());
        if source.combat_system.projectile_count() == 0
            && live_projectileless_delayed_count_for_test(&source.combat_system) == 0
        {
            break;
        }
    }
    assert_eq!(source.objects[&target].health.current, 70.0);
    assert_eq!(restored.objects[&target].health.current, 70.0);
    assert_eq!(restored.combat_system.projectile_count(), 0);
    assert_eq!(
        live_projectileless_delayed_count_for_test(&restored.combat_system),
        0
    );
    finish_accepted_flight(&mut restored);
    restored.apply_owned_projectileless_delayed_damage();
    assert_eq!(restored.objects[&target].health.current, 70.0);
    assert_eq!(fire_children(&restored), children);
}

#[test]
fn pending_combat_snapshot_direct_xfer_continues_real_queue_and_following_record() {
    use crate::save_load::{Snapshot, Xfer, XferLoad, XferSave};
    use std::io::Cursor;
    let (mut source, shooter, target) = world(SLOW_FIRST);
    accept(&mut source, shooter, target, 10.0);
    let mut captured = SnapshotBuilder::new()
        .create_world_snapshot(&source)
        .unwrap();
    let mut wire = Cursor::new(Vec::new());
    {
        let mut writer = XferSave::new(&mut wire);
        captured
            .xfer(&mut writer)
            .expect("actual direct world writer");
        let mut following = 0xF19E_0711u32;
        writer.xfer_u32(&mut following).unwrap();
    }
    let mut decoded = WorldSnapshot::default();
    let mut reader = XferLoad::new(Cursor::new(wire.into_inner()));
    decoded
        .xfer(&mut reader)
        .expect("actual direct world reader");
    let mut following = 0;
    reader.xfer_u32(&mut following).unwrap();
    assert_eq!(following, 0xF19E_0711);
    let mut restored = new_owned_world();
    restored.templates = source.templates.clone();
    SnapshotBuilder::new()
        .restore_from_snapshot(&decoded, &mut restored)
        .unwrap();
    assert_eq!(
        live_projectileless_delayed_count_for_test(&source.combat_system),
        1,
        "direct capture is observational"
    );
    assert_eq!(
        live_projectileless_delayed_count_for_test(&restored.combat_system),
        1
    );
    restored.frame = 102;
    restored.apply_owned_projectileless_delayed_damage();
    assert_eq!(restored.objects[&target].health.current, 100.0);
    restored.frame = 103;
    restored.apply_owned_projectileless_delayed_damage();
    assert_eq!(restored.objects[&target].health.current, 83.0);
    restored.apply_owned_projectileless_delayed_damage();
    assert_eq!(restored.objects[&target].health.current, 83.0);
    assert_eq!(
        live_projectileless_delayed_count_for_test(&restored.combat_system),
        0
    );
    assert_eq!(source.objects[&target].health.current, 100.0);
}

#[test]
fn pending_combat_snapshot_current_wire_has_exact_schema_and_end_boundary() {
    let (mut source, shooter, target) = world(SLOW_FIRST);
    accept(&mut source, shooter, target, 10.0);
    source.drain_pending_projectiles_into_combat();
    assert_eq!(
        live_projectileless_delayed_count_for_test(&source.combat_system),
        1
    );
    let snapshot = SnapshotBuilder::new()
        .create_world_snapshot(&source)
        .unwrap();
    assert_eq!(
        snapshot.version, WORLD_SNAPSHOT_BINCODE_VERSION,
        "current Rust envelope advertises admission and projectile continuation capabilities; body schema stays 23"
    );
    let wire = bincode_legacy::serialize(&snapshot).unwrap();
    // types.rs and world_codec.rs explicitly define an identical positional
    // body for envelopes 23 through 28. Their differing admission and projectile
    // continuation capabilities belong to the complete save bundle, not this typed world
    // body. Pin body admission only; this is not an old-bundle restore claim.
    for version in 23u32..=28 {
        let mut supported = wire.clone();
        supported[..4].copy_from_slice(&version.to_le_bytes());
        let decoded =
            crate::save_load::snapshot::decode_bincode_world_snapshot(&supported).unwrap();
        assert_eq!(decoded.version, version);
        let mut decoded_combat = crate::game_logic::combat::CombatSystem::new();
        decoded_combat.restore_pending_combat(&decoded.pending_combat);
        assert_eq!(
            live_projectileless_delayed_count_for_test(&decoded_combat),
            1
        );
        assert_eq!(
            bincode_legacy::serialize(&decoded.pending_combat).unwrap(),
            bincode_legacy::serialize(&snapshot.pending_combat).unwrap(),
            "typed queues retain their body under envelope {version}"
        );
        for cut in [0, 3, 4, supported.len() - 1] {
            assert!(
                crate::save_load::snapshot::decode_bincode_world_snapshot(&supported[..cut])
                    .is_err(),
                "truncated positional record {cut}, envelope {version}"
            );
        }
        supported.extend_from_slice(b"unframed data");
        assert!(crate::save_load::snapshot::decode_bincode_world_snapshot(&supported).is_err());
    }
    for version in [0u32, 1, 22, 29, u32::MAX] {
        let mut incompatible = wire.clone();
        incompatible[..4].copy_from_slice(&version.to_le_bytes());
        assert!(matches!(
            crate::save_load::snapshot::decode_bincode_world_snapshot(&incompatible),
            Err(crate::save_load::SaveLoadError::VersionMismatch {
                expected: WORLD_SNAPSHOT_DIRECT_XFER_VERSION,
                actual
            }) if actual == version
        ));
    }
    assert_eq!(source.objects[&target].health.current, 100.0);
    // C++ ceil(30 / (300 * rounded_f32(1/30))) schedules frame 103.
    source.frame = 102;
    apply_due(&mut source);
    assert_eq!(source.objects[&target].health.current, 100.0);
    source.frame = 103;
    apply_due(&mut source);
    assert_eq!(source.objects[&target].health.current, 83.0);
    assert_eq!(
        live_projectileless_delayed_count_for_test(&source.combat_system),
        0
    );
}

#[test]
fn pending_combat_snapshot_old_direct_reader_and_writer_stop_before_body() {
    use crate::save_load::{SaveLoadError, Snapshot, Xfer, XferLoad, XferSave};
    use std::io::Cursor;
    let mut old = WorldSnapshot::default();
    old.version = 22;
    let mut output = Cursor::new(Vec::new());
    {
        let mut writer = XferSave::new(&mut output);
        assert!(matches!(
            old.xfer(&mut writer),
            Err(SaveLoadError::VersionMismatch {
                expected: WORLD_SNAPSHOT_DIRECT_XFER_VERSION,
                actual: 22
            })
        ));
    }
    assert!(output.into_inner().is_empty());
    let mut bytes = Cursor::new(Vec::new());
    {
        let mut writer = XferSave::new(&mut bytes);
        let mut version = 22;
        writer.xfer_u32(&mut version).unwrap();
        let mut following = 0xAF17_0815u32;
        writer.xfer_u32(&mut following).unwrap();
    }
    let mut reader = XferLoad::new(Cursor::new(bytes.into_inner()));
    let mut target = WorldSnapshot::default();
    assert!(matches!(
        target.xfer(&mut reader),
        Err(SaveLoadError::VersionMismatch {
            expected: WORLD_SNAPSHOT_DIRECT_XFER_VERSION,
            actual: 22
        })
    ));
    let mut following = 0;
    reader.xfer_u32(&mut following).unwrap();
    assert_eq!(following, 0xAF17_0815);
}
