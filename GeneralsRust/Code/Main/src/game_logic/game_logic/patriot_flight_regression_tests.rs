//! Retail Patriot weapon and projectile through actual accepted fire and normal saves.
//! The carrier and unarmored target are deliberately small test objects; weapon,
//! MissileAIUpdate, and locomotor rules are read from the retail INIs.
use super::*;

fn retail_ini(name: &str) -> String {
    super::patriot_retail_battery_tests::base_ini(name)
}

fn single_block(source: &str, header: &str) -> String {
    let mut lines = source.lines().skip_while(|line| !line.trim().eq_ignore_ascii_case(header));
    let first = lines.next().expect("retail definition exists");
    let mut block = format!("{first}\n");
    for line in lines {
        block.push_str(line);
        block.push('\n');
        if line.trim_end().eq_ignore_ascii_case("End") {
            return block;
        }
    }
    panic!("unterminated retail definition: {header}");
}

fn world() -> (GameLogic, ObjectId, ObjectId) {
    world_for("PatriotMissileWeapon", "PatriotMissile", "PatriotMissileLocomotor")
}

fn world_for(weapon_name: &str, projectile_name: &str, locomotor_name: &str) -> (GameLogic, ObjectId, ObjectId) {
    use crate::assets::ini_template_loader::{parse_weapon_definitions_ini_into, WeaponDefinitionLoadType};
    let weapon_ini = single_block(&retail_ini("Weapon.ini"), &format!("Weapon {weapon_name}"));
    let mut weapons = HashMap::new();
    assert_eq!(parse_weapon_definitions_ini_into(&mut weapons, &weapon_ini, WeaponDefinitionLoadType::Normal), 1);
    assert_eq!(weapons[&weapon_name.to_ascii_lowercase()].projectile_name, projectile_name);

    let projectile_ini = single_block(&retail_ini("Object/WeaponObjects.ini"), &format!("Object {projectile_name}"));
    let mut parser = crate::assets::IniParser::new();
    parser.parse_ini_content(
        &projectile_ini,
        "WeaponObjects.ini",
    ).unwrap();
    let projectile = parser.get_definition(projectile_name).expect("retail missile").clone();
    let objects = vec![(projectile_name.to_string(), projectile)];
    let loco_ini = single_block(&retail_ini("Locomotor.ini"), &format!("Locomotor {locomotor_name}"));
    let mut locomotors = game_engine::common::ini::ini_locomotor::LocomotorStore::new_owned();
    locomotors.apply_source(&loco_ini).unwrap();

    let mut world = GameLogic::new();
    world.admit_locomotor_definitions(locomotors).unwrap();
    let mut map = gamelogic::system::map_loader::MapData::new();
    map.width = 32;
    map.height = 32;
    map.heightmap = vec![0; 1024];
    map.boundaries = vec![gamelogic::common::ICoord2D::new(32, 32)];
    world.world_services.terrain().write().unwrap().try_load_map_geometry(map).unwrap();
    world.initialize_receiving_partition_grid().unwrap();
    for (id, team) in [(0, Team::USA), (1, Team::China)] {
        let mut player = Player::new(id, team, "Patriot regression", id == 0);
        player.alliance_team = id as i32;
        world.add_player(player);
    }
    world.admit_receiving_module_content(crate::assets::ReceivingObjectContent::from_sources([
        ("missile-regression.ini".into(), format!("Object PatriotRegressionCarrier\nKindOf = VEHICLE SELECTABLE CAN_ATTACK\nWeaponSet\nConditions = None\nWeapon = PRIMARY {weapon_name}\nEnd\nBody = ActiveBody Body\nMaxHealth = 1000\nInitialHealth = 1000\nEnd\nBehavior = AIUpdateInterface AI\nEnd\nEnd\nObject PatriotRegressionTarget\nKindOf = VEHICLE SELECTABLE\nBody = ActiveBody Body\nMaxHealth = 1000\nInitialHealth = 1000\nEnd\nBehavior = AIUpdateInterface AI\nEnd\nGeometry = Sphere\nGeometryMajorRadius = 0\nEnd\n")),
        ("WeaponObjects.ini".into(), projectile_ini),
    ]).unwrap()).unwrap();
    world.admit_weapon_content_snapshot(weapons.into_values().map(|w| (w.name.clone(), w)).collect(), &objects).unwrap();
    let source = world.create_object_for_player("PatriotRegressionCarrier", 0, Vec3::new(100.0, 30.0, 100.0)).unwrap();
    let target = world.create_object_for_player("PatriotRegressionTarget", 1, Vec3::new(280.0, 30.0, 100.0)).unwrap();
    world.frame = 100;
    let target_pos = world.objects[&target].get_position();
    world.objects.get_mut(&source).unwrap().set_prev_victim_pos(Some(target_pos));
    (world, source, target)
}

fn launch(world: &mut GameLogic, source: ObjectId, target: ObjectId) {
    // C++ attack orders can specify a shot budget (Weapon::m_maxShotCount).
    // A normal unlimited attack keeps firing while the missile is in flight.
    world.objects.get_mut(&source).unwrap().set_max_shots_to_fire(1);
    assert_eq!(world.attack_fire_weapon_update(source, target, 10.0), AttackFireResult::Success);
    assert_eq!(world.objects[&source].max_shots_to_fire, 0);
    world.drain_pending_projectiles_into_combat();
    let projectiles = world.combat_system.projectiles_snapshot();
    assert_eq!(projectiles.len(), 1);
    let shot = projectiles[0];
    assert_eq!(shot.projectile_object_name, "PatriotMissile");
    assert_eq!(shot.target_id, Some(target));
    assert!(shot.missile_motion.is_some(), "retail missile must bind its owned physics and locomotor");
    assert_eq!(world.objects[&shot.id].template_name, "PatriotMissile");
    assert_eq!(world.objects[&shot.id].get_position(), shot.position);
    assert_eq!(shot.damage, 30.0);
    assert_eq!(world.objects[&target].health.current, 1000.0);
}

fn advance_to_impact(world: &mut GameLogic, target: ObjectId) {
    let first_id = world.combat_system.projectiles_snapshot()[0].id;
    for _ in 0..300 {
        if world.combat_system.projectile_count() == 0 { break; }
        let _ = world.update_with_dt(1.0 / 30.0);
        assert!(world.combat_system.projectiles_snapshot().iter().all(|p| p.id == first_id),
            "one-shot budget must not produce a second projectile");
    }
    assert_eq!(world.combat_system.projectile_count(), 0, "retail missile reaches its target");
    assert_eq!(world.objects[&target].health.current, 970.0, "retail warhead damages the target once");
}

#[test]
fn retail_patriot_fires_flies_and_impacts() {
    let (mut world, source, target) = world();
    launch(&mut world, source, target);
    let before = world.combat_system.projectiles_snapshot()[0].position;
    let _ = world.update_with_dt(1.0 / 30.0);
    assert_eq!(world.combat_system.projectile_count(), 1, "missile does not hit at launch");
    assert_ne!(world.combat_system.projectiles_snapshot()[0].position, before, "missile actually flies");
    let speed = world.combat_system.projectiles_snapshot()[0].velocity.length();
    assert!(speed > 40.0 && speed < 400.0,
        "InitialVelocity and locomotor acceleration must drive the shot, not WeaponSpeed=1: {speed}");
    assert_eq!(world.objects[&target].health.current, 1000.0);
    advance_to_impact(&mut world, target);
}

#[test]
fn retail_tomahawk_accepted_shot_uses_authored_missile_flight() {
    let (mut world, source, target) = world_for(
        "TomahawkMissileWeapon", "TomahawkMissile", "TomahawkMissileLocomotor",
    );
    world.objects.get_mut(&source).unwrap().set_max_shots_to_fire(1);
    assert_eq!(world.attack_fire_weapon_update(source, target, 10.0), AttackFireResult::Continue);
    world.frame += 8;
    assert_eq!(world.attack_fire_weapon_update(source, target, 10.3), AttackFireResult::Success);
    world.drain_pending_projectiles_into_combat();
    let shots = world.combat_system.projectiles_snapshot();
    assert_eq!(shots.len(), 1);
    assert_eq!(shots[0].projectile_object_name, "TomahawkMissile");
    assert!(shots[0].missile_motion.is_some());
    let id = shots[0].id;
    assert_eq!(world.objects[&id].template_name, "TomahawkMissile");
    assert_eq!(world.objects[&id].get_position(), shots[0].position);
    assert!(shots[0].is_homing, "zero WeaponSpeed must not erase MissileAI's tracked victim");
    let start = shots[0].position;
    let _ = world.update_with_dt(1.0 / 30.0);
    assert_ne!(world.combat_system.projectiles_snapshot()[0].position, start);
    assert_eq!(world.objects[&id].get_position(), world.combat_system.projectiles_snapshot()[0].position);
    let mut last_flight = None;
    for _ in 0..300 {
        let snapshot = world.combat_system.projectiles_snapshot();
        if snapshot.is_empty() { break; }
        let p = snapshot[0];
        last_flight = Some((p.position, p.velocity, p.target_position, p.is_homing,
            p.flight_runtime.missile_phase, p.lifetime, p.missile_kill_self_started_frame));
        let _ = world.update_with_dt(1.0 / 30.0);
    }
    assert_eq!(world.combat_system.projectile_count(), 0, "last flight: {last_flight:?}");
    assert!(world.objects[&target].health.current < 1000.0,
        "target {:?}, last flight: {last_flight:?}", world.objects[&target].get_position());
}
#[test]
fn accepted_missile_uses_launcher_pose_and_velocity_at_delivery() {
    let (mut world, source, target) = world();
    let (_, shot) = world.objects.get_mut(&source).unwrap()
        .prepare_passive_weapon_shot(
            0, target, 10.0, false, false, 100,
            Vec3::new(100.0, 30.0, 100.0),
            &mut world.combat_system.fire_sound_loop_events,
            &mut world.attack_events,
            Some(&world.projectile_definitions),
            &mut crate::game_logic::ObjectAudio::record(&mut Vec::new()),
        ).unwrap();
    // Model a source moved by an accepted effect between preparation and
    // delivery. The unchanged target keeps the shot otherwise identical.
    let position = Vec3::new(110.0, 32.0, 105.0);
    let velocity = Vec3::new(9.0, 3.0, -6.0);
    world.set_fixture_object_position(source, position);
    world.objects.get_mut(&source).unwrap().movement.velocity = velocity;
    world.deliver_owned_weapon_shot(0, shot, None).unwrap();
    let projectiles = world.combat_system.projectiles_snapshot();
    assert_eq!(projectiles.len(), 1);
    assert!(projectiles[0].missile_motion.is_some());
    assert_eq!(projectiles[0].position, position);
    assert_eq!(projectiles[0].velocity, velocity);
    assert_eq!(world.objects[&projectiles[0].id].get_position(), position);
    assert_eq!(world.objects[&projectiles[0].id].movement.velocity, velocity);
}

#[test]
fn missing_draw_launch_still_rotates_by_the_selected_turret() {
    let (mut world, source, target) = world();
    let carrier = world.objects.get_mut(&source).unwrap();
    carrier.turret_runtime.has_authored_turret = true;
    carrier.turret_runtime.controlled_weapon_slots = 1;
    carrier.turret_ai.turret_angle_deg = 90.0;
    carrier.turret_ai.turret_pitch_deg = 30.0;
    let (_, shot) = carrier.prepare_passive_weapon_shot(
        0, target, 10.0, false, false, 100,
        Vec3::new(100.0, 30.0, 100.0),
        &mut world.combat_system.fire_sound_loop_events,
        &mut world.attack_events,
        Some(&world.projectile_definitions),
        &mut crate::game_logic::ObjectAudio::record(&mut Vec::new()),
    ).unwrap();
    let transform = shot.source_context.as_ref().unwrap().launch_transform.unwrap();
    let direction = transform.x_axis.truncate();
    assert!((direction - Vec3::new(0.0, 0.5, 0.866_025_4)).length() < 0.001);
    world.deliver_owned_weapon_shot(0, shot, None).unwrap();
    let before = world.combat_system.projectiles_snapshot()[0].position;
    let _ = world.update_with_dt(1.0 / 30.0);
    let displacement = world.combat_system.projectiles_snapshot()[0].position - before;
    assert!(displacement.z > 0.0, "missile inherits the rotated launch direction");
}

#[test]
fn unlimited_attack_schedules_distinct_missiles_during_flight() {
    let (mut world, source, target) = world();
    assert_eq!(world.attack_fire_weapon_update(source, target, 10.0), AttackFireResult::Success);
    world.drain_pending_projectiles_into_combat();
    let mut ids = std::collections::HashSet::new();
    for _ in 0..30 {
        ids.extend(world.combat_system.projectiles_snapshot().iter().map(|p| p.id));
        let _ = world.update_with_dt(1.0 / 30.0);
    }
    assert!(ids.len() > 1, "an unlimited attack continues firing separate missiles");
}


#[test]
fn retail_patriot_continues_in_flight_after_encoded_world_save() {
    let (mut original, source, target) = world();
    launch(&mut original, source, target);
    let _ = original.update_with_dt(1.0 / 30.0);
    assert_eq!(original.combat_system.projectile_count(), 1);
    let position = original.combat_system.projectiles_snapshot()[0].position;
    let id = original.combat_system.projectiles_snapshot()[0].id;
    assert_eq!(original.objects[&id].get_position(), position);
    let builder = crate::save_load::SnapshotBuilder::new();
    let saved = builder.create_world_snapshot(&original).unwrap();
    let bytes = bincode_legacy::serialize(&saved).unwrap();
    let decoded = crate::save_load::snapshot::decode_bincode_world_snapshot(&bytes).unwrap();
    let (mut restored, _, _) = world();
    builder.restore_from_snapshot(&decoded, &mut restored).unwrap();
    assert_eq!(restored.combat_system.projectile_count(), 1);
    assert_eq!(restored.combat_system.projectiles_snapshot()[0].position, position);
    assert_eq!(restored.combat_system.projectiles_snapshot()[0].id, id);
    assert_eq!(restored.objects[&id].template_name, "PatriotMissile");
    assert_eq!(restored.objects[&id].get_position(), position);
    let _ = original.update_with_dt(1.0 / 30.0);
    let _ = restored.update_with_dt(1.0 / 30.0);
    assert_eq!(original.combat_system.projectiles_snapshot()[0].position, restored.combat_system.projectiles_snapshot()[0].position);
    assert_eq!(original.objects[&id].get_position(), restored.objects[&id].get_position());
    advance_to_impact(&mut original, target);
    advance_to_impact(&mut restored, target);
}

#[test]
fn retail_tomahawk_tracks_moving_target_and_continues_after_save_and_target_loss() {
    let (mut original, source, target) = world_for(
        "TomahawkMissileWeapon", "TomahawkMissile", "TomahawkMissileLocomotor",
    );
    original.objects.get_mut(&source).unwrap().set_max_shots_to_fire(1);
    assert_eq!(original.attack_fire_weapon_update(source, target, 10.0), AttackFireResult::Continue);
    original.frame += 8;
    assert_eq!(original.attack_fire_weapon_update(source, target, 10.3), AttackFireResult::Success);
    original.drain_pending_projectiles_into_combat();
    let id = original.combat_system.projectiles_snapshot()[0].id;
    let moved_target = Vec3::new(300.0, 30.0, 125.0);
    original.set_fixture_object_position(target, moved_target);
    let _ = original.update_with_dt(1.0 / 30.0);
    let shot = original.combat_system.projectiles_snapshot()[0];
    let live_target = original.objects[&target].get_position();
    assert_eq!((live_target.x, live_target.z), (moved_target.x, moved_target.z));
    assert_eq!(shot.target_position, live_target,
        "missile observes the target after this frame's world physics");
    assert_eq!(original.objects[&id].template_name, "TomahawkMissile");
    assert_eq!(original.objects[&id].get_position(), shot.position);

    let builder = crate::save_load::SnapshotBuilder::new();
    let saved = builder.create_world_snapshot(&original).unwrap();
    let bytes = bincode_legacy::serialize(&saved).unwrap();
    let decoded = crate::save_load::snapshot::decode_bincode_world_snapshot(&bytes).unwrap();
    let (mut restored, _, _) = world_for(
        "TomahawkMissileWeapon", "TomahawkMissile", "TomahawkMissileLocomotor",
    );
    builder.restore_from_snapshot(&decoded, &mut restored).unwrap();
    assert_eq!(restored.combat_system.projectiles_snapshot()[0].id, id);
    assert_eq!(restored.objects[&id].template_name, "TomahawkMissile");
    assert_eq!(restored.objects[&id].get_position(), original.objects[&id].get_position());
    for _ in 0..5 {
        let _ = original.update_with_dt(1.0 / 30.0);
        let _ = restored.update_with_dt(1.0 / 30.0);
        assert_eq!(original.objects[&id].get_position(), restored.objects[&id].get_position());
        assert_eq!(original.objects[&id].movement.velocity, restored.objects[&id].movement.velocity);
        let original_target = original.objects[&target].get_position();
        let restored_target = restored.objects[&target].get_position();
        assert_eq!(original_target, restored_target);
        assert_eq!(original.combat_system.projectiles_snapshot()[0].target_position, original_target);
        assert_eq!(restored.combat_system.projectiles_snapshot()[0].target_position, restored_target);
    }
    original.destroy_object(target).unwrap();
    restored.destroy_object(target).unwrap();
    original.process_destroy_list();
    restored.process_destroy_list();
    assert!(original.host_object(target).is_none());
    assert!(restored.host_object(target).is_none());
    for _ in 0..30 {
        let _ = original.update_with_dt(1.0 / 30.0);
        let _ = restored.update_with_dt(1.0 / 30.0);
        let a = original.combat_system.projectiles_snapshot();
        let b = restored.combat_system.projectiles_snapshot();
        assert_eq!(a.len(), b.len());
        if a.is_empty() { break; }
        assert_eq!(a[0].id, id);
        assert_eq!(b[0].id, id);
        assert_eq!(a[0].is_missile_kill_self_holding(), b[0].is_missile_kill_self_holding());
        assert_eq!(original.objects[&id].get_position(), restored.objects[&id].get_position());
    }
}

#[test]
fn queued_retail_missile_flight_uses_receiving_object_pose() {
    let (mut moved, source, target) = world();
    let (mut peer, peer_source, peer_target) = world();
    launch(&mut moved, source, target);
    launch(&mut peer, peer_source, peer_target);
    let id = moved.combat_system.projectiles_snapshot()[0].id;
    assert_eq!(peer.combat_system.projectiles_snapshot()[0].id, id);
    let peer_position = peer.objects[&id].get_position();
    moved.set_fixture_object_position(id, peer_position + Vec3::new(0.0, 0.0, 20.0));
    let _ = moved.update_with_dt(1.0 / 30.0);
    let _ = peer.update_with_dt(1.0 / 30.0);
    assert_ne!(moved.objects[&id].get_position(), peer.objects[&id].get_position());
    assert_eq!(moved.objects[&id].get_position(), moved.combat_system.projectiles_snapshot()[0].position);
    assert_eq!(peer.objects[&id].get_position(), peer.combat_system.projectiles_snapshot()[0].position);
}
