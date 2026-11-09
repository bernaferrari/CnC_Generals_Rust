//! Retail Patriot weapon and projectile through actual accepted fire and normal saves.
//! The carrier and unarmored target are deliberately small test objects; weapon,
//! MissileAIUpdate, and locomotor rules are read from the retail INIs.
use super::*;

fn retail_ini(name: &str) -> String {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../../windows_game/extracted_big_files_v2/INIZH/Data/INI");
    std::fs::read_to_string(root.join(name)).expect("retail Zero Hour INI")
}

fn single_block(source: &str, header: &str) -> String {
    let mut lines = source.lines().skip_while(|line| !line.trim().eq_ignore_ascii_case(header));
    let first = lines.next().expect("retail definition exists");
    let mut block = format!("{first}\n");
    for line in lines {
        block.push_str(line);
        block.push('\n');
        if line.trim().eq_ignore_ascii_case("End") {
            return block;
        }
    }
    panic!("unterminated retail definition: {header}");
}

fn world() -> (GameLogic, ObjectId, ObjectId) {
    use crate::assets::ini_template_loader::{parse_weapon_definitions_ini_into, WeaponDefinitionLoadType};
    let weapon_ini = single_block(&retail_ini("Weapon.ini"), "Weapon PatriotMissileWeapon");
    let mut weapons = HashMap::new();
    assert_eq!(parse_weapon_definitions_ini_into(&mut weapons, &weapon_ini, WeaponDefinitionLoadType::Normal), 1);
    let weapon = &weapons["patriotmissileweapon"];
    assert_eq!(weapon.primary_damage, 30.0);
    assert_eq!(weapon.projectile_name, "PatriotMissile");

    let mut parser = crate::assets::IniParser::new();
    parser.parse_ini_content(&retail_ini("Object/WeaponObjects.ini"), "WeaponObjects.ini").unwrap();
    let projectile = parser.get_definition("PatriotMissile").expect("retail missile").clone();
    let objects = vec![("PatriotMissile".to_string(), projectile)];
    let loco_ini = single_block(&retail_ini("Locomotor.ini"), "Locomotor PatriotMissileLocomotor");
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
        ("patriot-regression.ini".into(), "Object PatriotRegressionCarrier\nKindOf = VEHICLE SELECTABLE CAN_ATTACK\nWeaponSet\nConditions = None\nWeapon = PRIMARY PatriotMissileWeapon\nEnd\nBody = ActiveBody Body\nMaxHealth = 1000\nInitialHealth = 1000\nEnd\nBehavior = AIUpdateInterface AI\nEnd\nEnd\nObject PatriotRegressionTarget\nKindOf = VEHICLE SELECTABLE\nBody = ActiveBody Body\nMaxHealth = 1000\nInitialHealth = 1000\nEnd\nBehavior = AIUpdateInterface AI\nEnd\nGeometry = Sphere\nGeometryMajorRadius = 0\nEnd\n".into()),
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
    let builder = crate::save_load::SnapshotBuilder::new();
    let saved = builder.create_world_snapshot(&original).unwrap();
    let bytes = bincode_legacy::serialize(&saved).unwrap();
    let decoded = crate::save_load::snapshot::decode_bincode_world_snapshot(&bytes).unwrap();
    let (mut restored, _, _) = world();
    builder.restore_from_snapshot(&decoded, &mut restored).unwrap();
    assert_eq!(restored.combat_system.projectile_count(), 1);
    assert_eq!(restored.combat_system.projectiles_snapshot()[0].position, position);
    let _ = original.update_with_dt(1.0 / 30.0);
    let _ = restored.update_with_dt(1.0 / 30.0);
    assert_eq!(original.combat_system.projectiles_snapshot()[0].position, restored.combat_system.projectiles_snapshot()[0].position);
    advance_to_impact(&mut original, target);
    advance_to_impact(&mut restored, target);
}
