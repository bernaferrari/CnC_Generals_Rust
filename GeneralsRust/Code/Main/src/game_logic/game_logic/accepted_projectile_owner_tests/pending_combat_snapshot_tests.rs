//! Real authored accepted fire -> owned materialization -> snapshot continuation.
//! The parent fixture binds parsed named weapons through actual world admission.
use super::*;
use crate::game_logic::combat::{WeaponFireOcl, live_projectileless_delayed_count_for_test};
use crate::save_load::snapshot::{SnapshotBuilder, WorldSnapshot};

fn encoded_snapshot(world: &GameLogic) -> WorldSnapshot {
    let captured = SnapshotBuilder::new()
        .create_world_snapshot(world)
        .expect("capture actual driving world");
    let wire = bincode_legacy::serialize(&captured).expect("serialize actual schema");
    bincode_legacy::deserialize(&wire).expect("decode actual schema")
}

fn materialize(world: &mut GameLogic) -> Vec<WeaponFireOcl> {
    drain_pending_projectiles(&mut world.combat_system, &world.objects, world.frame);
    let effects = world.combat_system.take_fire_ocl();
    apply_due(world);
    effects
}

fn apply_due(world: &mut GameLogic) {
    apply_ready_projectileless_delayed_damage(
        &mut world.combat_system,
        &mut world.objects,
        world.frame,
        Some(&world.players),
        &mut world.health_events,
    );
}

fn assert_fifo_effects(effects: &[WeaponFireOcl], source: ObjectId, other: ObjectId) {
    assert_eq!(effects.len(), 3);
    assert_eq!(
        effects
            .iter()
            .map(|event| event.shooter_id)
            .collect::<Vec<_>>(),
        vec![source, other, source]
    );
    assert_eq!(
        effects
            .iter()
            .map(|event| event.fire_ocl_name.as_str())
            .collect::<Vec<_>>(),
        vec![
            "OCL_AcceptedQueueFirst",
            "OCL_AcceptedQueueSecond",
            "OCL_AcceptedQueueFirst"
        ]
    );
    assert_eq!(effects[0].source_team, Team::USA);
    assert_eq!(effects[2].source_team, Team::USA);
    assert_eq!(
        effects[0].source_veterancy,
        crate::game_logic::VeterancyLevel::Rookie
    );
    assert_eq!(effects[0].source_orientation, 1.25);
    assert_eq!(effects[2].source_orientation, 1.25);
    assert_eq!(effects[0].source_velocity, Vec3::new(2.0, 0.0, 3.0));
    assert_eq!(effects[2].source_velocity, Vec3::new(2.0, 0.0, 3.0));
}

#[test]
fn pending_combat_snapshot_accepted_fifo_replaces_in_place_queue_and_keeps_retired_source() {
    let _serial = combat_test_guard();
    let _restore = RestoreInputs::install();
    let (mut source_world, source, target) = world(FIRST_WEAPON);
    {
        let source_object = source_world.objects.get_mut(&source).unwrap();
        source_object.set_orientation(1.25);
        source_object.movement.velocity = Vec3::new(2.0, 0.0, 3.0);
    }
    let other = admit(
        &mut source_world,
        "SnapshotOtherShooter",
        Some(SECOND_WEAPON),
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
    source_world.destroy_object(source);
    source_world.process_destroy_list_if_needed();
    assert!(!source_world.objects.contains_key(&source));
    assert_eq!(source_world.objects[&target].health.current, 100.0);

    let captured = encoded_snapshot(&source_world);
    assert!(!captured.objects.contains_key(&source));
    let (mut destination, reused_source, reused_target) = world(SECOND_WEAPON);
    assert_eq!((source, target), (reused_source, reused_target));
    accept(&mut destination, reused_source, reused_target, 10.0);

    // Capture is observational: the original still owns all three accepted shots.
    let original_effects = materialize(&mut source_world);
    assert_fifo_effects(&original_effects, source, other);
    assert_eq!(source_world.objects[&target].health.current, 53.0);

    destination.templates = source_world.templates.clone();
    SnapshotBuilder::new()
        .restore_from_snapshot(&captured, &mut destination)
        .expect("restore into actual same-ID destination with its own pending shot");
    assert!(!destination.objects.contains_key(&source));
    assert_eq!(destination.objects[&target].health.current, 100.0);
    let restored_effects = materialize(&mut destination);
    assert_fifo_effects(&restored_effects, source, other);
    assert_eq!(destination.objects[&target].health.current, 53.0);
    assert_eq!(source_world.objects[&target].health.current, 53.0);
    assert!(materialize(&mut destination).is_empty());
    assert_eq!(destination.objects[&target].health.current, 53.0);
}

const SLOW_FIRST: &str = "SnapshotFiniteFirstWeapon";
const SLOW_SECOND: &str = "SnapshotFiniteSecondWeapon";

fn install_finite_weapons() {
    let source = r#"
Weapon SnapshotFiniteFirstWeapon
  PrimaryDamage = 17
  PrimaryDamageRadius = 0
  AttackRange = 200
  DamageType = SMALL_ARMS
  ProjectileObject = NONE
  WeaponSpeed = 300
  DelayBetweenShots = 1000
  ClipSize = 8
  ClipReloadTime = 1000
  PreAttackDelay = 0
  AntiGround = Yes
End
Weapon SnapshotFiniteSecondWeapon
  PrimaryDamage = 13
  PrimaryDamageRadius = 0
  AttackRange = 200
  DamageType = SMALL_ARMS
  ProjectileObject = NONE
  WeaponSpeed = 300
  DelayBetweenShots = 1000
  ClipSize = 8
  ClipReloadTime = 1000
  PreAttackDelay = 0
  AntiGround = Yes
End
"#;
    assert_eq!(
        crate::assets::ini_template_loader::register_weapons_from_ini_text(source),
        2
    );
    gamelogic::weapon::with_weapon_store(|store| {
        for (name, damage) in [(SLOW_FIRST, 17.0), (SLOW_SECOND, 13.0)] {
            let authored = store
                .find_weapon_template(name)
                .expect("real authored finite weapon");
            assert_eq!(authored.primary_damage, damage);
            assert_eq!(authored.weapon_speed, 10.0);
            assert_eq!(authored.attack_range, 200.0);
            assert!(authored.projectile_name.eq_ignore_ascii_case("NONE"));
            assert_eq!(authored.clip_size, 8);
        }
    })
    .expect("actual registered finite rules");
}

#[test]
fn pending_combat_snapshot_finite_deadline_survives_fresh_and_in_place_same_id_loads() {
    let _serial = combat_test_guard();
    let _restore = RestoreInputs::install();
    install_finite_weapons();
    let (mut first, source, target) = world(SLOW_FIRST);
    let (mut second, other_source, other_target) = world(SLOW_SECOND);
    assert_eq!((source, target), (other_source, other_target));
    assert_eq!(first.frame, 100);
    assert_eq!(
        first.objects[&source]
            .weapon
            .as_ref()
            .unwrap()
            .projectile_speed,
        10.0
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

    let mut fresh = GameLogic::new();
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
    let _serial = combat_test_guard();
    let _restore = RestoreInputs::install();
    install_finite_weapons();
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
    assert!(materialize(&mut destination).is_empty());
    assert_eq!(destination.objects[&target].health.current, 100.0);
}

#[test]
fn pending_combat_snapshot_rejects_old_schema_before_mutating_live_owner() {
    let _serial = combat_test_guard();
    let _restore = RestoreInputs::install();
    let (mut source, shooter, target) = world(FIRST_WEAPON);
    accept(&mut source, shooter, target, 10.0);
    let mut captured = encoded_snapshot(&source);
    captured.version = 22;
    captured.frame_number = 999;
    let (mut destination, own_shooter, own_target) = world(SECOND_WEAPON);
    accept(&mut destination, own_shooter, own_target, 10.0);
    let result = SnapshotBuilder::new().restore_from_snapshot(&captured, &mut destination);
    assert!(matches!(
        result,
        Err(crate::save_load::SaveLoadError::VersionMismatch {
            expected: 24,
            actual: 22
        })
    ));
    assert_eq!(destination.frame, 100);
    assert_eq!(destination.objects[&own_target].health.current, 100.0);
    destination.drain_pending_projectiles_into_combat();
    assert_eq!(destination.objects[&own_target].health.current, 87.0);
    source.drain_pending_projectiles_into_combat();
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
    let _serial = combat_test_guard();
    let _restore = RestoreInputs::install();
    install_finite_weapons();
    let (mut source, shooter, target) = world(SLOW_FIRST);
    accept(&mut source, shooter, target, 10.0);
    source.drain_pending_projectiles_into_combat();
    assert_eq!(
        live_projectileless_delayed_count_for_test(&source.combat_system),
        1
    );
    let fast = admit(
        &mut source,
        "SnapshotFastShooter",
        Some(SECOND_WEAPON),
        Team::USA,
        Vec3::ZERO,
    );
    let victim_pos = source.objects[&target].get_position();
    source.objects.get_mut(&fast).unwrap().prev_victim_pos = Some(victim_pos);
    accept(&mut source, fast, target, 10.0);
    let dir = tempfile::TempDir::new().expect("owned save directory");
    let mut manager = crate::save_load::SaveFileManager::with_save_directory(dir.path());
    manager.init().expect("real manager init");
    manager
        .save_game("both_queues", &source, &save_info("both_queues"))
        .expect("actual Common named-chunk writer");
    let (decoded, _) = manager
        .load_game_snapshot("both_queues")
        .expect("actual current decoder");
    let mut restored = GameLogic::new();
    restored.templates = source.templates.clone();
    manager
        .restore_game_snapshot(&decoded, &mut restored)
        .expect("actual staging restore route");
    assert_eq!(
        live_projectileless_delayed_count_for_test(&restored.combat_system),
        1
    );
    restored.drain_pending_projectiles_into_combat();
    assert_eq!(restored.objects[&target].health.current, 87.0);
    assert_eq!(restored.combat_system.projectile_count(), 0);
    restored.set_current_frame(103);
    apply_due(&mut restored);
    assert_eq!(restored.objects[&target].health.current, 70.0);
    restored.drain_pending_projectiles_into_combat();
    apply_due(&mut restored);
    assert_eq!(restored.objects[&target].health.current, 70.0);
    assert_eq!(source.objects[&target].health.current, 100.0);
    assert_eq!(
        live_projectileless_delayed_count_for_test(&source.combat_system),
        1
    );
}

#[test]
fn pending_combat_snapshot_direct_xfer_continues_real_queue_and_following_record() {
    use crate::save_load::{Snapshot, Xfer, XferLoad, XferSave};
    use std::io::Cursor;
    let _serial = combat_test_guard();
    let _restore = RestoreInputs::install();
    let (mut source, shooter, target) = world(FIRST_WEAPON);
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
    let mut restored = GameLogic::new();
    restored.templates = source.templates.clone();
    SnapshotBuilder::new()
        .restore_from_snapshot(&decoded, &mut restored)
        .unwrap();
    restored.drain_pending_projectiles_into_combat();
    assert_eq!(restored.objects[&target].health.current, 83.0);
    restored.drain_pending_projectiles_into_combat();
    assert_eq!(restored.objects[&target].health.current, 83.0);
    assert_eq!(source.objects[&target].health.current, 100.0);
}

#[test]
fn pending_combat_snapshot_current_wire_has_exact_schema_and_end_boundary() {
    let _serial = combat_test_guard();
    let _restore = RestoreInputs::install();
    let (mut source, shooter, target) = world(FIRST_WEAPON);
    accept(&mut source, shooter, target, 10.0);
    let snapshot = SnapshotBuilder::new()
        .create_world_snapshot(&source)
        .unwrap();
    assert_eq!(
        snapshot.version, 24,
        "current Rust envelope requires the alliance capability; body schema stays 23"
    );
    let wire = bincode_legacy::serialize(&snapshot).unwrap();
    // The previous envelope remains readable with the identical typed body.
    // Pin its admission independently of the current writer version.
    for version in [23u32, 24] {
        let mut supported = wire.clone();
        supported[..4].copy_from_slice(&version.to_le_bytes());
        let decoded =
            crate::save_load::snapshot::decode_bincode_world_snapshot(&supported).unwrap();
        assert_eq!(decoded.version, version);
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
    for version in [0u32, 1, 22, 25, u32::MAX] {
        let mut incompatible = wire.clone();
        incompatible[..4].copy_from_slice(&version.to_le_bytes());
        assert!(crate::save_load::snapshot::decode_bincode_world_snapshot(&incompatible).is_err());
    }
    assert_eq!(source.objects[&target].health.current, 100.0);
    source.drain_pending_projectiles_into_combat();
    assert_eq!(source.objects[&target].health.current, 83.0);
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
                expected: 24,
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
            expected: 24,
            actual: 22
        })
    ));
    let mut following = 0;
    reader.xfer_u32(&mut following).unwrap();
    assert_eq!(following, 0xAF17_0815);
}
