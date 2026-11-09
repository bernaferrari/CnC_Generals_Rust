//! The retail Patriot battery, weapon, missile and locomotor at the accepted-fire seam.
//! The one-shot budget is the C++ Weapon max-shot-count admission.
use super::*;

fn base_ini(name: &str) -> String {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../windows_game/extracted_big_files_v2/INI");
    std::fs::read_to_string(root.join(name)).expect("retail base Generals INI")
}

// Top-level definitions in the retail files end with an unindented End;
// nested Draw, WeaponSet and Behavior blocks must remain intact.
fn definition(source: &str, header: &str) -> String {
    let mut lines = source.lines().skip_while(|line| !line.trim().eq_ignore_ascii_case(header));
    let first = lines.next().expect("retail definition exists");
    let mut result = format!("{first}\n");
    for line in lines {
        result.push_str(line);
        result.push('\n');
        if line.trim_end().eq_ignore_ascii_case("End") {
            return result;
        }
    }
    panic!("unterminated retail definition: {header}");
}

pub(in crate::game_logic) fn world() -> (GameLogic, ObjectId, ObjectId) {
    use crate::assets::ini_template_loader::{parse_weapon_definitions_ini_into, WeaponDefinitionLoadType};
    let weapon_text = base_ini("Weapon.ini");
    let mut weapons = HashMap::new();
    for name in ["PatriotMissileWeapon", "PatriotMissileAssistWeapon", "PatriotMissileWeaponAir"] {
        let block = definition(&weapon_text, &format!("Weapon {name}"));
        assert_eq!(parse_weapon_definitions_ini_into(&mut weapons, &block, WeaponDefinitionLoadType::Normal), 1);
    }
    assert_eq!(weapons["patriotmissileweapon"].primary_damage, 30.0);
    assert_eq!(weapons["patriotmissileweapon"].projectile_name, "PatriotMissile");

    let missile_text = base_ini("Object/WeaponObjects.ini");
    let missile = definition(&missile_text, "Object PatriotMissile");
    let mut parser = crate::assets::IniParser::new();
    parser.parse_ini_content(&missile, "WeaponObjects.ini").unwrap();
    let projectile = parser.get_definition("PatriotMissile").expect("retail missile").clone();
    let objects = vec![("PatriotMissile".to_string(), projectile)];
    let locomotor = definition(&base_ini("Locomotor.ini"), "Locomotor PatriotMissileLocomotor");
    let mut locomotors = game_engine::common::ini::ini_locomotor::LocomotorStore::new_owned();
    locomotors.apply_source(&locomotor).unwrap();

    let mut assets = crate::assets::AssetManager::new().unwrap();
    assets.archive_file_system_mut().set_local_search_roots(Vec::new());
    let archive = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../../windows_game/Command & Conquer Generals/W3D.big");
    futures::executor::block_on(assets.archive_file_system_mut().load_big_file(archive)).unwrap();
    let mut world = GameLogic::new();
    world.bind_application_assets(std::sync::Arc::new(std::sync::Mutex::new(assets)));
    world.admit_locomotor_definitions(locomotors).unwrap();
    let mut map = gamelogic::system::map_loader::MapData::new();
    map.width = 32;
    map.height = 32;
    map.heightmap = vec![0; 1024];
    map.boundaries = vec![gamelogic::common::ICoord2D::new(32, 32)];
    world.world_services.terrain().write().unwrap().try_load_map_geometry(map).unwrap();
    world.initialize_receiving_partition_grid().unwrap();
    for (id, team) in [(0, Team::USA), (1, Team::China)] {
        let mut player = Player::new(id, team, "retail battery", id == 0);
        player.alliance_team = id as i32;
        world.add_player(player);
    }
    let buildings = base_ini("Object/FactionBuilding.ini");
    let battery = definition(&buildings, "Object AmericaPatriotBattery");
    // The retail battery's Prerequisites block references this real template.
    let power_plant = definition(&buildings, "Object AmericaPowerPlant");
    let target = "Object RetailPatriotTarget\nKindOf = VEHICLE SELECTABLE\nBody = ActiveBody Body\nMaxHealth = 1000\nInitialHealth = 1000\nEnd\nBehavior = AIUpdateInterface AI\nEnd\nGeometry = Sphere\nGeometryMajorRadius = 0\nEnd\n";
    world.admit_receiving_module_content(crate::assets::ReceivingObjectContent::from_sources([
        ("AmericaPowerPlant.ini".into(), power_plant),
        ("FactionBuilding.ini".into(), battery),
        ("retail-patriot-target.ini".into(), target.into()),
    ]).unwrap()).unwrap();
    world.admit_weapon_content_snapshot(weapons.into_values().map(|w| (w.name.clone(), w)).collect(), &objects).unwrap();
    // POWERED structures consume Energy immediately on admission. The retail
    // Patriot consumes 3, so its authored 5-Energy plant must exist first.
    world.create_object_for_player("AmericaPowerPlant", 0, Vec3::new(10.0, 0.0, 10.0)).unwrap();
    let source = world.create_object_for_player("AmericaPatriotBattery", 0, Vec3::new(100.0, 30.0, 100.0)).unwrap();
    assert!(!world.objects[&source].is_disabled(), "retail battery must have power before firing");
    let target = world.create_object_for_player("RetailPatriotTarget", 1, Vec3::new(280.0, 30.0, 100.0)).unwrap();
    world.frame = 100;
    let target_pos = world.objects[&target].get_position();
    world.objects.get_mut(&source).unwrap().set_prev_victim_pos(Some(target_pos));
    (world, source, target)
}

fn launch_one(world: &mut GameLogic, source: ObjectId, target: ObjectId) {
    let assets = world.application_assets().expect("world-owned retail asset manager");
    let mut effects = crate::game_logic::ScriptPresentationEffects {
        pristine_bones: Some(assets.as_ref()),
        ..Default::default()
    };
    world.objects.get_mut(&source).unwrap().set_max_shots_to_fire(1);
    assert_eq!(world.attack_fire_weapon_update_with_effects(source, target, 10.0, &mut effects), AttackFireResult::Success);
    assert_eq!(world.objects[&source].max_shots_to_fire, 0);
    world.drain_pending_projectiles_into_combat();
    let projectiles = world.combat_system.projectiles_snapshot();
    assert_eq!(projectiles.len(), 1, "one accepted retail shot");
    assert_eq!(projectiles[0].projectile_object_name, "PatriotMissile");
    assert_eq!(projectiles[0].target_id, Some(target));
    assert_eq!(projectiles[0].damage, 30.0);
    assert!(projectiles[0].missile_motion.is_some(), "retail missile binds locomotor physics");
    assert_ne!(projectiles[0].position, world.objects[&source].get_position(), "retail W3D launch bone supplies the launch position");
    assert_eq!(world.objects[&target].health.current, 1000.0);
}

fn impact(world: &mut GameLogic, target: ObjectId) {
    for _ in 0..300 {
        if world.combat_system.projectile_count() == 0 { break; }
        let _ = world.update_with_dt(1.0 / 30.0);
        assert!(world.combat_system.projectile_count() <= 1, "no additional accepted shots");
    }
    assert_eq!(world.combat_system.projectile_count(), 0, "retail missile reaches its target");
    assert_eq!(world.objects[&target].health.current, 970.0, "one retail warhead impacts once");
}

#[test]
fn retail_battery_accepted_shot_flies_and_impacts() {
    let (mut world, source, target) = world();
    launch_one(&mut world, source, target);
    let position = world.combat_system.projectiles_snapshot()[0].position;
    let _ = world.update_with_dt(1.0 / 30.0);
    assert_eq!(world.combat_system.projectile_count(), 1);
    assert_ne!(world.combat_system.projectiles_snapshot()[0].position, position);
    assert_eq!(world.objects[&target].health.current, 1000.0);
    impact(&mut world, target);
}

#[test]
fn retail_battery_shot_continues_after_encoded_world_save() {
    let (mut original, source, target) = world();
    launch_one(&mut original, source, target);
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
    impact(&mut original, target);
    impact(&mut restored, target);
}
