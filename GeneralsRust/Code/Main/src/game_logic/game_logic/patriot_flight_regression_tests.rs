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
    world_for_with(weapon_name, projectile_name, locomotor_name, |_, _, _| {})
}

fn world_for_with(
    weapon_name: &str,
    projectile_name: &str,
    locomotor_name: &str,
    configure: impl FnOnce(&mut String, &mut String, &mut String),
) -> (GameLogic, ObjectId, ObjectId) {
    use crate::assets::ini_template_loader::{parse_weapon_definitions_ini_into, WeaponDefinitionLoadType};
    let mut weapon_ini = single_block(&retail_ini("Weapon.ini"), &format!("Weapon {weapon_name}"));
    let mut projectile_ini = single_block(&retail_ini("Object/WeaponObjects.ini"), &format!("Object {projectile_name}"));
    let mut objects_ini = format!("Object PatriotRegressionCarrier\nKindOf = VEHICLE SELECTABLE CAN_ATTACK\nWeaponSet\nConditions = None\nWeapon = PRIMARY {weapon_name}\nEnd\nBody = ActiveBody Body\nMaxHealth = 1000\nInitialHealth = 1000\nEnd\nBehavior = AIUpdateInterface AI\nEnd\nEnd\nObject PatriotRegressionTarget\nKindOf = VEHICLE SELECTABLE\nBody = ActiveBody Body\nMaxHealth = 1000\nInitialHealth = 1000\nEnd\nBehavior = AIUpdateInterface AI\nEnd\nGeometry = Sphere\nGeometryMajorRadius = 0\nEnd\n");
    configure(&mut weapon_ini, &mut projectile_ini, &mut objects_ini);
    let mut weapons = HashMap::new();
    assert_eq!(parse_weapon_definitions_ini_into(&mut weapons, &weapon_ini, WeaponDefinitionLoadType::Normal), 1);
    assert_eq!(weapons[&weapon_name.to_ascii_lowercase()].projectile_name, projectile_name);

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
    world.admit_camera_application_movement(
        game_engine::common::ini::ini_game_data::CameraMovementRules {
            partition_cell_size: crate::game_logic::partition_manager::PARTITION_CELL_SIZE_RESIDUAL,
            ..Default::default()
        },
    );
    world.admit_locomotor_definitions(locomotors).unwrap();
    let mut map = gamelogic::system::map_loader::MapData::new();
    map.width = 32;
    map.height = 32;
    map.heightmap = vec![0; 1024];
    map.boundaries = vec![glam::IVec2::new(32, 32)];
    world.world_services.terrain().write().unwrap().try_load_map_geometry(map).unwrap();
    world.initialize_receiving_partition_grid().unwrap();
    for (id, team) in [(0, Team::USA), (1, Team::China)] {
        let mut player = Player::new(id, team, "Patriot regression", id == 0);
        player.alliance_team = id as i32;
        world.add_player(player);
    }
    world.admit_receiving_module_content(crate::assets::ReceivingObjectContent::from_sources([
        ("missile-regression.ini".into(), objects_ini),
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
fn retail_patriot_coordinate_kill_waits_for_the_move_callback() {
    let (mut world, source, target) = world();
    let goal = Vec3::new(240.0, 45.0, 120.0);
    world
        .fire_owned_temporary_weapon_at(
            source,
            "PatriotMissileWeapon",
            goal,
            &mut crate::game_logic::ScriptPresentationEffects::default(),
        )
        .unwrap();
    world.drain_pending_projectiles_into_combat();
    let id = advance_to_kill(&mut world);
    let shot = world.combat_system.projectiles_snapshot()[0];
    assert_eq!(shot.target_id, None);
    assert_eq!(shot.target_position, goal);
    assert!(!shot.missile_drawable_hidden());

    world.set_fixture_object_position(id, goal - 6.0 * Vec3::X);
    world.objects.get_mut(&id).unwrap().movement.velocity = 240.0 * Vec3::X;
    let _ = world.update_with_dt(1.0 / 30.0);
    let shot = world.combat_system.projectiles_snapshot()[0];
    assert_eq!(shot.position, goal, "braking reaches the coordinate");
    assert!(
        !shot.missile_drawable_hidden(),
        "KILL sees the preceding move state"
    );

    let _ = world.update_with_dt(1.0 / 30.0);
    assert!(
        !world.combat_system.projectiles_snapshot()[0].missile_drawable_hidden(),
        "the parent MoveTo callback recognizes arrival after the missile callback"
    );

    let builder = crate::save_load::SnapshotBuilder::new();
    let saved = builder.create_world_snapshot(&world).unwrap();
    let bytes = bincode_legacy::serialize(&saved).unwrap();
    let decoded = crate::save_load::snapshot::decode_bincode_world_snapshot(&bytes).unwrap();
    let (mut restored, _, _) = self::world();
    builder.restore_from_snapshot(&decoded, &mut restored).unwrap();
    for receiving in [&mut world, &mut restored] {
        let _ = receiving.update_with_dt(1.0 / 30.0);
        let shot = receiving.combat_system.projectiles_snapshot()[0];
        assert_eq!(shot.id, id);
        assert!(shot.missile_drawable_hidden());
        assert_eq!(shot.position, goal);
        assert_eq!(receiving.objects[&target].health.current, 1000.0);
    }
}

#[test]
fn retail_patriot_tracked_kill_resumes_move_or_snaps_to_target_origin() {
    let (mut world, source, target) = world_for_with(
        "PatriotMissileWeapon",
        "PatriotMissile",
        "PatriotMissileLocomotor",
        |_, _, objects| {
            *objects = objects.replace(
                "Geometry = Sphere\nGeometryMajorRadius = 0",
                "Geometry = Cylinder\nGeometryMajorRadius = 1\nGeometryHeight = 8",
            );
        },
    );
    // Hold the test target on the flat terrain instead of letting gravity
    // move the aim point between these staged arrivals.
    let target_ground = world.objects[&target].get_position().with_y(0.0);
    world.set_fixture_object_position(target, target_ground);
    launch(&mut world, source, target);
    let id = advance_to_kill(&mut world);
    // Isolate the MissileAI callback from the earlier object-contact path.
    world.objects.get_mut(&id).unwrap().set_status_no_collisions(true);
    let aim = world.combat_system.projectiles_snapshot()[0].target_position;
    assert_eq!(aim, world.objects[&target].get_position() + 8.0 * Vec3::Y);
    world.set_fixture_object_position(id, aim);
    let _ = world.update_with_dt(1.0 / 30.0);
    assert!(!world.combat_system.projectiles_snapshot()[0].missile_drawable_hidden());

    let new_origin = world.objects[&target].get_position() + 40.0 * Vec3::Z;
    world.set_fixture_object_position(target, new_origin);
    let _ = world.update_with_dt(1.0 / 30.0);
    let shot = world.combat_system.projectiles_snapshot()[0];
    assert!(!shot.missile_drawable_hidden(), "target left one frame's reach");
    assert!(shot.position.z > aim.z, "KILL issues another MoveToObject");
    let new_aim = shot.target_position;
    world.set_fixture_object_position(id, new_aim);
    let _ = world.update_with_dt(1.0 / 30.0);
    let shot = world.combat_system.projectiles_snapshot()[0];
    assert!(!shot.missile_drawable_hidden());
    assert_eq!(shot.position, new_aim);
    assert_eq!(shot.target_position, new_aim);

    let _ = world.update_with_dt(1.0 / 30.0);
    let shot = world.combat_system.projectiles_snapshot()[0];
    assert!(
        shot.missile_drawable_hidden(),
        "the idle missile at {:?} should detonate near its target at {:?}",
        shot.position,
        world.objects[&target].get_position()
    );
    assert_eq!(shot.position, new_origin);
    assert_eq!(world.objects[&id].get_position(), new_origin);
}

fn advance_to_kill(world: &mut GameLogic) -> ObjectId {
    use crate::game_logic::weapon_bootstrap::HostMissilePhase;

    for _ in 0..100 {
        let shot = world.combat_system.projectiles_snapshot()[0];
        if shot.flight_runtime.missile_phase == HostMissilePhase::Kill {
            assert!(!shot.missile_drawable_hidden());
            return shot.id;
        }
        let _ = world.update_with_dt(1.0 / 30.0);
    }
    panic!("retail missile never entered KILL");
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

#[test]
fn retail_patriot_checks_lock_before_missing_target() {
    use crate::game_logic::weapon_bootstrap::{HostMissilePhase, HostProjectileFlight};

    for (distance, enters_kill) in [(50.0, true), (100.0, false)] {
        let (mut world, source, target) = world();
        let victim_position = world.objects[&source].get_position() + 50.0 * Vec3::X;
        world.set_fixture_object_position(target, victim_position);
        world.objects.get_mut(&source).unwrap().set_prev_victim_pos(Some(victim_position));
        launch(&mut world, source, target);

        // Stock IgnitionDelay=0 arms the missile on this update. ATTACK
        // dispatches on the next one. Lose the object between those updates.
        let _ = world.update_with_dt(1.0 / 30.0);
        let (id, position, goal) = {
            let shots = world.combat_system.projectiles_snapshot();
            assert_eq!(shots.len(), 1);
            let shot = shots[0];
            assert!(shot.flight_runtime.missile_armed);
            assert_eq!(shot.flight_runtime.missile_phase, HostMissilePhase::Attack);
            let Some(HostProjectileFlight::Missile(rule)) = shot.flight.as_ref() else {
                panic!("stock Patriot must have an admitted MissileAIUpdate");
            };
            assert_eq!(rule.lock_distance, 100.0);
            assert!(shot.is_homing);
            (shot.id, shot.position, shot.target_position)
        };
        // Establish the exact pre-update distance on the receiving actor,
        // including the equality control for C++'s strict lock comparison.
        world.set_fixture_object_position(
            id,
            Vec3::new(goal.x - distance, position.y, goal.z),
        );
        world.destroy_object(target).unwrap();
        world.process_destroy_list();
        assert!(world.host_object(target).is_none());

        let _ = world.update_with_dt(1.0 / 30.0);
        let shot = world.combat_system.get_projectiles().get(&id).unwrap();
        // MissileAIUpdate.cpp:506-553 checks its retained goal first and
        // returns from ATTACK as soon as it enters KILL.
        assert_eq!(
            (shot.flight_runtime.missile_phase == HostMissilePhase::Kill,
             shot.is_missile_kill_self_holding()),
            (enters_kill, !enters_kill),
            "target removed at {distance} units",
        );
    }
}

#[test]
fn retail_patriot_lock_requires_intended_target_contact() {
    use crate::game_logic::weapon_bootstrap::HostMissilePhase;

    let (mut world, source, target) = world();
    launch(&mut world, source, target);
    let _ = world.update_with_dt(1.0 / 30.0);
    let shot = world.combat_system.projectiles_snapshot()[0];
    assert!(shot.flight_runtime.missile_armed);
    assert_eq!(shot.flight_runtime.missile_phase, HostMissilePhase::Attack);
    let missile = shot.id;
    let victim = world.objects[&target].get_position();

    // The stock missile has radius 2 and the target has radius 0.
    // MissileAIUpdate.cpp:506–529 enters KILL without running its handler here.
    world.set_fixture_object_position(missile, victim - 4.9 * Vec3::X);
    world.objects.get_mut(&missile).unwrap().movement.velocity = Vec3::ZERO;
    let _ = world.update_with_dt(1.0 / 30.0);

    assert_eq!(world.objects[&target].health.current, 1000.0);
    let shot = world.combat_system.get_projectiles().get(&missile).unwrap();
    assert_eq!(shot.flight_runtime.missile_phase, HostMissilePhase::Kill);
    assert!(!shot.missile_drawable_hidden());
    assert!(host_geom_collides(&world.objects[&missile], &world.objects[&target]).is_none());
    let age = shot.lifetime;

    let contact = victim - 1.5 * Vec3::X;
    world.set_fixture_object_position(missile, contact);
    assert!(host_geom_collides(&world.objects[&missile], &world.objects[&target]).is_some());
    // Dispatch from the partition phase, without stepping the flight again.
    world.update_owned_partition_with_effects(&mut Default::default()).unwrap();
    assert!(world.partition_manager.neighbor_ids_in_coi_order(missile.0).contains(&target.0));
    let shot = world.combat_system.get_projectiles().get(&missile).unwrap();
    assert_eq!(shot.lifetime, age);
    assert_eq!(shot.position, contact);
    assert!(shot.missile_drawable_hidden());
    assert!(world.objects[&missile].status.no_collisions);
    assert_eq!(world.objects[&target].health.current, 970.0);
    advance_to_impact(&mut world, target);
}

#[test]
fn retail_tomahawk_lock_transition_skips_attack_dive() {
    use crate::game_logic::weapon_bootstrap::{HostMissilePhase, HostProjectileFlight};

    for (distance, expected_phase, precise_z) in [
        (10.0, HostMissilePhase::Attack, true),
        (5.0, HostMissilePhase::Kill, false),
    ] {
        let (mut world, source, target) = world_for(
            "TomahawkMissileWeapon",
            "TomahawkMissile",
            "TomahawkMissileLocomotor",
        );
        world
            .objects
            .get_mut(&source)
            .unwrap()
            .set_max_shots_to_fire(1);
        assert_eq!(
            world.attack_fire_weapon_update(source, target, 10.0),
            AttackFireResult::Continue
        );
        world.frame += 8;
        assert_eq!(
            world.attack_fire_weapon_update(source, target, 10.3),
            AttackFireResult::Success
        );
        world.drain_pending_projectiles_into_combat();
        let _ = world.update_with_dt(1.0 / 30.0);
        let id = {
            let shots = world.combat_system.projectiles_snapshot();
            assert_eq!(shots.len(), 1);
            let shot = shots[0];
            let Some(HostProjectileFlight::Missile(rule)) = shot.flight.as_ref() else {
                panic!("stock Tomahawk must have an admitted MissileAIUpdate");
            };
            assert_eq!(rule.lock_distance, 10.0);
            assert!(shot.flight_runtime.missile_armed && shot.is_homing);
            assert_eq!(shot.flight_runtime.missile_phase, HostMissilePhase::Attack);
            shot.id
        };
        assert_eq!(world.objects[&id].loco_preferred_height, 120.0);
        assert!(!world.objects[&id].precise_z_pos);
        let victim = world.objects[&target].get_position();
        // Establish the first ATTACK distance before the missile can latch
        // precise Z. Vertical separation prevents a collision on this frame.
        world.set_fixture_object_position(
            id,
            Vec3::new(victim.x - distance, victim.y + 120.0, victim.z),
        );
        let _ = world.update_with_dt(1.0 / 30.0);
        let shot = world.combat_system.get_projectiles().get(&id).unwrap();
        assert_eq!(shot.flight_runtime.missile_phase, expected_phase);
        assert_eq!(world.objects[&target].health.current, 1000.0);
        // C++ MissileAIUpdate.cpp:520–541 returns from ATTACK after entering
        // KILL. At exact lock distance it still applies the in-range dive.
        assert_eq!(
            world.objects[&id].precise_z_pos, precise_z,
            "distance {distance}"
        );
    }
}

/// MissileAIUpdate.cpp:364-399 detonate() runs handleProjectileDetonation —
/// the warhead's victim Body damage — before the MissileCallsOnDie self-hit.
/// A lethal victim's CreateObjectDie child must be allocated before the
/// missile's own die child, and that child's producer is the dying missile.
#[test]
fn missile_calls_on_die_body_damage_follows_warhead_victim_die() {
    let (mut world, source, target) = world_for_with(
        "PatriotMissileWeapon",
        "PatriotMissile",
        "PatriotMissileLocomotor",
        |weapon_ini, projectile_ini, objects_ini| {
            let at = weapon_ini.rfind("End").expect("weapon End");
            weapon_ini.insert_str(at, "MissileCallsOnDie = Yes\n");
            let at = projectile_ini.rfind("End").expect("missile object End");
            projectile_ini.insert_str(
                at,
                "Behavior = CreateObjectDie MissileOrderTag\nCreationList = OCL_MissileDieOrderChild\nDeathTypes = NONE +DETONATED\nEnd\n",
            );
            let at = objects_ini.rfind("End").expect("target object End");
            objects_ini.insert_str(
                at,
                "Behavior = CreateObjectDie VictimOrderTag\nCreationList = OCL_VictimDieOrderChild\nEnd\n",
            );
            objects_ini.push_str(
                "Object MissileDieOrderChild\nKindOf = VEHICLE\nBody = ActiveBody BodyTag\nMaxHealth = 10\nInitialHealth = 10\nEnd\nEnd\nObject VictimDieOrderChild\nKindOf = VEHICLE\nBody = ActiveBody BodyTag\nMaxHealth = 10\nInitialHealth = 10\nEnd\nEnd\n",
            );
        },
    );
    world.admit_ocl_application_sources(&[
        "ObjectCreationList OCL_VictimDieOrderChild\nCreateObject\nObjectNames = VictimDieOrderChild\nCount = 1\nEnd\nEnd\n".into(),
        "ObjectCreationList OCL_MissileDieOrderChild\nCreateObject\nObjectNames = MissileDieOrderChild\nCount = 1\nEnd\nEnd\n".into(),
    ]).unwrap();

    launch(&mut world, source, target);
    let missile_id = world.combat_system.projectiles_snapshot()[0].id;
    // A 10hp victim dies to the stock 30-damage warhead.
    world.objects.get_mut(&target).unwrap().health.current = 10.0;
    for _ in 0..300 {
        if world.combat_system.projectile_count() == 0 {
            break;
        }
        let _ = world.update_with_dt(1.0 / 30.0);
    }
    assert_eq!(world.combat_system.projectile_count(), 0, "missile reaches its target");

    let born_ids = |name: &str| -> Vec<ObjectId> {
        world
            .objects
            .registered_values()
            .filter(|object| object.template_name == name)
            .map(|object| object.id)
            .collect()
    };
    let victim_children = born_ids("VictimDieOrderChild");
    let missile_children = born_ids("MissileDieOrderChild");
    assert_eq!(victim_children.len(), 1, "victim die child born exactly once");
    assert_eq!(missile_children.len(), 1, "missile die child born exactly once");
    assert!(
        victim_children[0] < missile_children[0],
        "victim child {} born before missile child {}",
        victim_children[0],
        missile_children[0],
    );
    assert_eq!(
        world.objects.get(&missile_children[0]).unwrap().producer_id,
        Some(missile_id),
        "the dying missile is the OCL source, not the launcher",
    );
}

/// MissileAIUpdate.cpp:413-430 doKillSelfState — at KILL_SELF timeout an
/// authored DetonateCallsKill picks Object::kill (forced NORMAL death, so Die
/// modules run) over destroyObject. DetonateCallsKill = No keeps raw removal.
#[test]
fn missile_detonate_calls_kill_retirement_uses_kill_not_destroy() {
    for detonate_calls_kill in [true, false] {
        let (mut world, source, target) = world_for_with(
            "PatriotMissileWeapon",
            "PatriotMissile",
            "PatriotMissileLocomotor",
            |_, projectile_ini, objects_ini| {
                let module_at = projectile_ini
                    .find("MissileAIUpdate")
                    .expect("retail MissileAIUpdate");
                let end_at = module_at
                    + projectile_ini[module_at..]
                        .find("End")
                        .expect("MissileAIUpdate End");
                let authored = if detonate_calls_kill {
                    "DetonateCallsKill = Yes\nKillSelfDelay = 200\n"
                } else {
                    "DetonateCallsKill = No\nKillSelfDelay = 200\n"
                };
                projectile_ini.insert_str(end_at, authored);
                let at = projectile_ini.rfind("End").expect("missile object End");
                projectile_ini.insert_str(
                    at,
                    "Behavior = CreateObjectDie RetirementTag\nDeathTypes = NONE +NORMAL\nCreationList = OCL_MissileRetirementChild\nEnd\n",
                );
                objects_ini.push_str(
                    "Object MissileRetirementChild\nKindOf = VEHICLE\nBody = ActiveBody BodyTag\nMaxHealth = 10\nInitialHealth = 10\nEnd\nEnd\n",
                );
            },
        );
        world.admit_ocl_application_sources(&[
            "ObjectCreationList OCL_MissileRetirementChild\nCreateObject\nObjectNames = MissileRetirementChild\nCount = 1\nEnd\nEnd\n".into(),
        ]).unwrap();

        launch(&mut world, source, target);
        let missile_id = world.combat_system.projectiles_snapshot()[0].id;
        let born_ids = |world: &GameLogic| -> Vec<ObjectId> {
            world
                .objects
                .registered_values()
                .filter(|object| object.template_name == "MissileRetirementChild")
                .map(|object| object.id)
                .collect()
        };

        // Ordinary warhead impact holds the missile in KILL_SELF; nothing dies.
        let mut hold_step = None;
        for step in 0..300 {
            if world.combat_system.projectile_count() == 0 {
                break;
            }
            let _ = world.update_with_dt(1.0 / 30.0);
            if world
                .combat_system
                .projectiles_snapshot()
                .first()
                .is_some_and(|shot| shot.is_missile_kill_self_holding())
            {
                hold_step = Some(step);
                break;
            }
        }
        let hold_step = hold_step.expect("detonated missile enters KILL_SELF hold");
        assert_eq!(world.objects[&target].health.current, 970.0);
        assert!(world.host_object(missile_id).is_some());
        assert!(born_ids(&world).is_empty(), "impact itself does not kill");

        // KILL_SELF retires the authored 200ms = 6 logic frames after the hold.
        let mut retire_step = None;
        for step in (hold_step + 1)..(hold_step + 40) {
            let _ = world.update_with_dt(1.0 / 30.0);
            if world.combat_system.projectile_count() == 0 {
                retire_step = Some(step);
                break;
            }
            assert!(
                born_ids(&world).is_empty(),
                "retirement child not born before the KILL_SELF deadline",
            );
        }
        let retire_step = retire_step.expect("KILL_SELF retires the missile");
        assert_eq!(retire_step - hold_step, 6, "KillSelfDelay = 200ms");

        let children = born_ids(&world);
        if detonate_calls_kill {
            assert_eq!(children.len(), 1, "kill runs the authored die OCL once");
            assert_eq!(
                world.objects.get(&children[0]).unwrap().producer_id,
                Some(missile_id),
            );
            for _ in 0..10 {
                let _ = world.update_with_dt(1.0 / 30.0);
            }
            assert_eq!(
                born_ids(&world).len(),
                1,
                "the forced kill fires the die OCL exactly once",
            );
        } else {
            assert!(children.is_empty(), "destroy does not run die modules");
            assert!(world.host_object(missile_id).is_none());
        }
    }
}
