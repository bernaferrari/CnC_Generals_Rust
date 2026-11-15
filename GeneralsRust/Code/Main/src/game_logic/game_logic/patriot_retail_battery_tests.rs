//! The retail Patriot battery, weapon, missile and locomotor at the accepted-fire seam.
//! The one-shot budget is the C++ Weapon max-shot-count admission.
use super::*;

fn retail_root() -> std::path::PathBuf {
    std::env::var_os("GENERALS_RETAIL_CONTENT_ROOT")
        .map(std::path::PathBuf::from)
        .unwrap_or_else(|| std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../../windows_game"))
}

fn base_w3d_archive() -> std::path::PathBuf {
    let explicit = std::env::var_os("GENERALS_BASE_W3D_BIG").map(std::path::PathBuf::from);
    let root = retail_root();
    let candidates = [
        explicit,
        Some(root.join("Command & Conquer Generals/W3D.big")),
        Some(root.join("Command & Conquer Generals/Data/W3D.big")),
        Some(root.join("Command and Conquer Generals/W3D.big")),
        Some(root.join("Command and Conquer Generals/Data/W3D.big")),
    ];
    candidates.into_iter().flatten()
        .find(|path| path.metadata().is_ok_and(|metadata| metadata.is_file() && metadata.len() > 0))
        .expect("retail vanilla Patriot requires the nonempty base-game W3D.big containing ABPatriot; set GENERALS_BASE_W3D_BIG")
}

pub(in crate::game_logic) fn base_ini(name: &str) -> String {
    let archive = retail_root().join("Command & Conquer Generals Zero Hour/Data/INIZH.big");
    let mut files = crate::assets::ArchiveFileSystem::new();
    files.set_local_search_roots(Vec::new());
    futures::executor::block_on(files.load_big_file(archive)).expect("retail INI archive");
    let bytes = files.read_optional_definition_source(&format!("Data/INI/{name}"))
        .expect("retail INI read").expect("retail INI entry");
    String::from_utf8_lossy(&bytes).into_owned()
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
    world_for_target(false, false)
}

pub(in crate::game_logic) fn world_with_air_target() -> (GameLogic, ObjectId, ObjectId) {
    world_for_target(true, false)
}

fn world_for_target(air_target: bool, superweapon: bool) -> (GameLogic, ObjectId, ObjectId) {
    use crate::assets::ini_template_loader::{parse_weapon_definitions_ini_into, WeaponDefinitionLoadType};
    let weapon_text = base_ini("Weapon.ini");
    let mut weapons = HashMap::new();
    let weapon_names = if superweapon {
        ["SupW_PatriotMissileWeapon", "SupW_PatriotMissileAssistWeapon", "SupW_PatriotMissileWeaponAir"]
    } else {
        ["PatriotMissileWeapon", "PatriotMissileAssistWeapon", "PatriotMissileWeaponAir"]
    };
    for name in weapon_names {
        let block = definition(&weapon_text, &format!("Weapon {name}"));
        assert_eq!(parse_weapon_definitions_ini_into(&mut weapons, &block, WeaponDefinitionLoadType::Normal), 1);
    }
    let projectile_name = if superweapon { "PatriotMissileEMP" } else { "PatriotMissile" };
    let primary_name = weapon_names[0].to_ascii_lowercase();
    assert_eq!(weapons[&primary_name].primary_damage, if superweapon { 15.0 } else { 30.0 });
    assert_eq!(weapons[&primary_name].projectile_name, projectile_name);

    let missile_text = base_ini("Object/WeaponObjects.ini");
    let missile = definition(&missile_text, &format!("Object {projectile_name}"));
    let mut parser = crate::assets::IniParser::new();
    parser.parse_ini_content(&missile, "WeaponObjects.ini").unwrap();
    let projectile = parser.get_definition(projectile_name).expect("retail missile").clone();
    let objects = vec![(projectile_name.to_string(), projectile)];
    let locomotor = definition(&base_ini("Locomotor.ini"), "Locomotor PatriotMissileLocomotor");
    let mut locomotors = game_engine::common::ini::ini_locomotor::LocomotorStore::new_owned();
    locomotors.apply_source(&locomotor).unwrap();

    let mut assets = crate::assets::AssetManager::new().unwrap();
    assets.archive_file_system_mut().set_local_search_roots(Vec::new());
    if !superweapon {
        let base_archive = base_w3d_archive();
        futures::executor::block_on(assets.archive_file_system_mut().load_big_file(base_archive))
            .expect("mount genuine base-game W3D.big");
    }
    let archive = retail_root().join("Command & Conquer Generals Zero Hour/Data/W3DZH.big");
    futures::executor::block_on(assets.archive_file_system_mut().load_big_file(archive))
        .expect("mount genuine Zero Hour W3DZH.big");
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
    let (battery_name, plant_name, building_file) = if superweapon {
        ("SupW_AmericaPatriotBattery", "SupW_AmericaPowerPlant", "Object/SuperWeaponGeneral.ini")
    } else {
        ("AmericaPatriotBattery", "AmericaPowerPlant", "Object/FactionBuilding.ini")
    };
    let buildings = base_ini(building_file);
    let battery = definition(&buildings, &format!("Object {battery_name}"));
    let power_plant = definition(&buildings, &format!("Object {plant_name}"));
    let target = if air_target {
        "Object RetailPatriotTarget\nKindOf = AIRCRAFT SELECTABLE\nBody = ActiveBody Body\nMaxHealth = 200\nInitialHealth = 200\nEnd\nBehavior = AIUpdateInterface AI\nEnd\nGeometry = Sphere\nGeometryMajorRadius = 0\nEnd\n"
    } else {
        "Object RetailPatriotTarget\nKindOf = VEHICLE SELECTABLE\nBody = ActiveBody Body\nMaxHealth = 1000\nInitialHealth = 1000\nEnd\nBehavior = AIUpdateInterface AI\nEnd\nGeometry = Sphere\nGeometryMajorRadius = 0\nEnd\n"
    };
    world.admit_receiving_module_content(crate::assets::ReceivingObjectContent::from_sources([
        ("AmericaPowerPlant.ini".into(), power_plant),
        ("FactionBuilding.ini".into(), battery),
        ("retail-patriot-target.ini".into(), target.into()),
    ]).unwrap()).unwrap();
    world.admit_weapon_content_snapshot(weapons.into_values().map(|w| (w.name.clone(), w)).collect(), &objects).unwrap();
    let profile = world.projectile_definitions.get(projectile_name).expect("retail missile profile");
    assert!(profile.locomotor.is_some(), "retail PatriotMissileLocomotor must bind in this world");
    assert!(profile.behaviors.iter().any(|module| module.physics.is_some()), "retail missile must bind PhysicsBehavior");
    // POWERED structures consume Energy immediately on admission. The retail
    // Patriot consumes 3, so its authored 5-Energy plant must exist first.
    world.create_object_for_player(plant_name, 0, Vec3::new(10.0, 0.0, 10.0)).unwrap();
    let source = world.create_object_for_player(battery_name, 0, Vec3::new(100.0, 30.0, 100.0)).unwrap();
    assert!(!world.objects[&source].is_disabled(), "retail battery must have power before firing");
    let target_position = if air_target {
        Vec3::new(350.0, 80.0, 100.0)
    } else {
        Vec3::new(280.0, 30.0, 100.0)
    };
    let target = world.create_object_for_player("RetailPatriotTarget", 1, target_position).unwrap();
    if air_target {
        world.objects.get_mut(&target).unwrap().status.airborne_target = true;
    }
    world.frame = 100;
    let target_pos = world.objects[&target].get_position();
    world.objects.get_mut(&source).unwrap().set_prev_victim_pos(Some(target_pos));
    (world, source, target)
}

fn launch_one(world: &mut GameLogic, source: ObjectId, target: ObjectId) {
    let assets = world.application_assets().expect("world-owned retail asset manager");
    let superweapon = world.objects[&source].template_name.eq_ignore_ascii_case("SupW_AmericaPatriotBattery");
    let draw = crate::assets::AuthoredDrawModel {
        model_key: if superweapon { "ABPatriotSW" } else { "ABPatriot" }.into(),
        ..Default::default()
    };
    let bones = crate::assets::pristine_bones::PristineBoneAssets::pristine_bones_for_selected_draw(
        assets.as_ref(), &draw, 1.0, "WeaponA", 1, 1,
    ).expect("retail W3D archive must supply the selected Patriot launch model");
    assert_eq!(bones.len(), 1, "selected retail Patriot model must contain WeaponA01");
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
    assert_eq!(projectiles[0].projectile_object_name, if superweapon { "PatriotMissileEMP" } else { "PatriotMissile" });
    assert_eq!(projectiles[0].target_id, Some(target));
    assert_eq!(projectiles[0].damage, if superweapon { 15.0 } else { 30.0 });
    assert!(projectiles[0].missile_motion.is_some(), "retail missile binds locomotor physics");
    assert_ne!(projectiles[0].position, world.objects[&source].get_position(), "retail W3D launch bone supplies the launch position");
    let expected_health = if world.objects[&target].status.airborne_target { 200.0 } else { 1000.0 };
    assert_eq!(world.objects[&target].health.current, expected_health);
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
fn superweapon_general_retail_patriot_uses_its_own_model_and_missile() {
    let (mut world, source, target) = world_for_target(false, true);
    launch_one(&mut world, source, target);
    let position = world.combat_system.projectiles_snapshot()[0].position;
    let _ = world.update_with_dt(1.0 / 30.0);
    assert_eq!(world.combat_system.projectile_count(), 1);
    assert_ne!(world.combat_system.projectiles_snapshot()[0].position, position);
    assert_eq!(world.objects[&target].health.current, 1000.0);
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

#[test]
fn superweapon_retail_missile_motion_is_owned_by_its_world_even_with_matching_ids() {
    let (mut first, source, target) = world_for_target(false, true);
    let (mut second, other_source, other_target) = world_for_target(false, true);
    assert_eq!((source, target), (other_source, other_target));
    launch_one(&mut first, source, target);
    launch_one(&mut second, other_source, other_target);
    let first_shot = first.combat_system.projectiles_snapshot()[0];
    let second_shot = second.combat_system.projectiles_snapshot()[0];
    assert_eq!(first_shot.id, second_shot.id);
    let second_position = second_shot.position;
    let _ = first.update_with_dt(1.0 / 30.0);
    assert_ne!(first.combat_system.projectiles_snapshot()[0].position, second_position);
    assert_eq!(second.combat_system.projectiles_snapshot()[0].position, second_position);
    assert_eq!(second.objects[&other_target].health.current, 1000.0);
    let _ = second.update_with_dt(1.0 / 30.0);
    assert_eq!(
        first.combat_system.projectiles_snapshot()[0].position,
        second.combat_system.projectiles_snapshot()[0].position
    );
}
