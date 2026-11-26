//! The retail Patriot battery, weapon, missile and locomotor at the accepted-fire seam.
//! The one-shot budget is the C++ Weapon max-shot-count admission.
use super::*;

use crate::fixture_content::{retail_base_w3d_archive, retail_content_root};

pub(in crate::game_logic) fn base_ini(name: &str) -> String {
    let archive = retail_content_root().join("Command & Conquer Generals Zero Hour/Data/INIZH.big");
    let mut files = crate::assets::ArchiveFileSystem::new();
    files.set_local_search_roots(Vec::new());
    futures::executor::block_on(files.load_big_file(archive)).expect("retail INI archive");
    let bytes = files.read_optional_definition_source(&format!("Data/INI/{name}"))
        .expect("retail INI read").expect("retail INI entry");
    String::from_utf8_lossy(&bytes).into_owned()
}

// Top-level definitions in the retail files end with an unindented End;
// nested Draw, WeaponSet and Behavior blocks must remain intact.
pub(in crate::game_logic) fn definition(source: &str, header: &str) -> String {
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
    world_for_target(RetailTarget::Vehicle, false)
}

pub(in crate::game_logic) fn world_with_air_target() -> (GameLogic, ObjectId, ObjectId) {
    world_for_target(RetailTarget::Aircraft, false)
}

pub(in crate::game_logic) fn world_with_infantry_target() -> (GameLogic, ObjectId, ObjectId) {
    world_for_target(RetailTarget::Infantry, false)
}
pub(in crate::game_logic) fn superweapon_world(airborne: bool) -> (GameLogic, ObjectId, ObjectId) {
    let target = if airborne {
        RetailTarget::Aircraft
    } else {
        RetailTarget::Vehicle
    };
    world_for_target(target, true)
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum RetailTarget {
    Vehicle,
    Aircraft,
    Infantry,
}

fn world_for_target(target_kind: RetailTarget, superweapon: bool) -> (GameLogic, ObjectId, ObjectId) {
    let air_target = target_kind == RetailTarget::Aircraft;
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
    if superweapon {
        locomotors
            .apply_source("Locomotor EmpFixtureFlight\n Surfaces = AIR\n Speed = 20\n Acceleration = 20\n Braking = 20\n PreferredHeight = 80\n AirborneTargetingHeight = 5\nEnd\n")
            .unwrap();
    }

    let mut assets = crate::assets::AssetManager::new().unwrap();
    assets.archive_file_system_mut().set_local_search_roots(Vec::new());
    if !superweapon {
        let base_archive = retail_base_w3d_archive();
        futures::executor::block_on(assets.archive_file_system_mut().load_big_file(base_archive))
            .expect("mount genuine base-game W3D.big");
    }
    let archive = retail_content_root().join("Command & Conquer Generals Zero Hour/Data/W3DZH.big");
    futures::executor::block_on(assets.archive_file_system_mut().load_big_file(archive))
        .expect("mount genuine Zero Hour W3DZH.big");
    let mut world = GameLogic::new();
    world.bind_application_assets(std::sync::Arc::new(std::sync::Mutex::new(assets)));
    world.admit_locomotor_definitions(locomotors).unwrap();
    let mut map = gamelogic::system::map_loader::MapData::new();
    let size = if superweapon { 64 } else { 32 };
    map.width = size;
    map.height = size;
    map.heightmap = vec![0; (size * size) as usize];
    map.boundaries = vec![gamelogic::common::ICoord2D::new(size as i32, size as i32)];
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
    let (kind, hp) = match target_kind {
        RetailTarget::Aircraft => ("AIRCRAFT VEHICLE", 200),
        RetailTarget::Vehicle => ("VEHICLE", 1000),
        RetailTarget::Infantry => ("INFANTRY", 100),
    };
    let motion_and_geometry = if superweapon && air_target {
        // Preserve the mounted warhead's tight 3D pulse while the air target
        // continues moving between impact and the fade. Use the stock bounds.
        let raptor = definition(&base_ini("Object/AmericaAir.ini"), "Object AmericaJetRaptor");
        let mut fields = String::from("Locomotor = SET_NORMAL EmpFixtureFlight\n");
        for line in raptor.lines().filter(|line| line.trim_start().starts_with("Geometry")) {
            fields.push_str(line);
            fields.push('\n');
        }
        fields
    } else {
        "Geometry = Sphere\nGeometryMajorRadius = 0\n".into()
    };
    let target = format!("Object RetailPatriotTarget\nKindOf = {kind} SELECTABLE\nBody = ActiveBody Body\nMaxHealth = {hp}\nInitialHealth = {hp}\nEnd\nBehavior = AIUpdateInterface AI\nEnd\n{motion_and_geometry}End\n");
    let mut object_sources = vec![
        ("AmericaPowerPlant.ini".into(), power_plant),
        ("FactionBuilding.ini".into(), battery),
        ("WeaponObjects.ini".into(), missile),
        ("retail-patriot-target.ini".into(), target),
    ];
    if superweapon {
        world
            .admit_object_creation_list_sources([definition(
                &base_ini("ObjectCreationList.ini"),
                "ObjectCreationList SupW_OCL_PatriotEMPEffectSpheroid",
            )])
            .unwrap();
        object_sources.push((
            "System.ini".into(),
            definition(&base_ini("Object/System.ini"), "Object EMPPatriotEffectSpheroid"),
        ));
        object_sources.push((
            "emp-test-hardened.ini".into(),
            "Object EmpTestHardened\nKindOf = VEHICLE AIRCRAFT EMP_HARDENED\nBody = ActiveBody Body\n  MaxHealth = 200\n  InitialHealth = 200\nEnd\nBehavior = AIUpdateInterface AI\nEnd\nLocomotor = SET_NORMAL EmpFixtureFlight\nGeometry = Sphere\nGeometryMajorRadius = 2\nEnd\n".into(),
        ));
    }
    world.admit_receiving_module_content(crate::assets::ReceivingObjectContent::from_sources(object_sources).unwrap()).unwrap();
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
    assert_eq!(world.objects[&projectiles[0].id].template_name, projectiles[0].projectile_object_name);
    assert_eq!(world.objects[&projectiles[0].id].get_position(), projectiles[0].position);
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

fn assert_missile_object_state_eq(a: &crate::game_logic::object::Object, b: &crate::game_logic::object::Object) {
    assert_eq!(a.get_transform_matrix(), b.get_transform_matrix());
    assert_eq!(a.movement.velocity, b.movement.velocity);
    assert_eq!(a.cur_locomotor_name, b.cur_locomotor_name);
    assert_eq!(a.locomotor_set_names, b.locomotor_set_names);
    assert_eq!(a.locomotor_goal_type, b.locomotor_goal_type);
    assert_eq!(a.locomotor_goal_angle, b.locomotor_goal_angle);
    assert_eq!(a.is_braking, b.is_braking);
    assert_eq!(a.braking_factor, b.braking_factor);
    assert_eq!(a.physics_mass, b.physics_mass);
    assert_eq!(a.physics_accel, b.physics_accel);
    assert_eq!(a.physics_previous_accel, b.physics_previous_accel);
    assert_eq!(a.motive_force_expires, b.motive_force_expires);
    assert_eq!(a.physics_turning, b.physics_turning);
    assert_eq!(a.is_in_freefall, b.is_in_freefall);
    assert_eq!(a.was_airborne_last_frame, b.was_airborne_last_frame);
    assert_eq!(a.pending_ground_collide, b.pending_ground_collide);
}

#[test]
fn superweapon_general_retail_patriot_uses_its_own_model_and_missile() {
    let (mut world, source, target) = world_for_target(RetailTarget::Vehicle, true);
    launch_one(&mut world, source, target);
    let position = world.combat_system.projectiles_snapshot()[0].position;
    let _ = world.update_with_dt(1.0 / 30.0);
    assert_eq!(world.combat_system.projectile_count(), 1);
    assert_ne!(world.combat_system.projectiles_snapshot()[0].position, position);
    assert_eq!(world.objects[&target].health.current, 1000.0);
}

#[test]
fn superweapon_missile_save_continues_the_same_owned_flight() {
    let (mut original, source, target) = world_for_target(RetailTarget::Vehicle, true);
    launch_one(&mut original, source, target);
    let moved_target = Vec3::new(300.0, 30.0, 125.0);
    original.set_fixture_object_position(target, moved_target);
    let _ = original.update_with_dt(1.0 / 30.0);
    let id = original.combat_system.projectiles_snapshot()[0].id;
    assert_eq!(original.combat_system.projectiles_snapshot()[0].target_position, original.objects[&target].get_position());
    let (mut peer, peer_source, peer_target) = world_for_target(RetailTarget::Vehicle, true);
    launch_one(&mut peer, peer_source, peer_target);
    assert_eq!(peer.combat_system.projectiles_snapshot()[0].id, id);
    let peer_before = peer.objects[&id].clone();
    let builder = crate::save_load::SnapshotBuilder::new();
    let saved = builder.create_world_snapshot(&original).unwrap();
    let bytes = bincode_legacy::serialize(&saved).unwrap();
    let decoded = crate::save_load::snapshot::decode_bincode_world_snapshot(&bytes).unwrap();
    let (mut restored, _, _) = world_for_target(RetailTarget::Vehicle, true);
    builder.restore_from_snapshot(&decoded, &mut restored).unwrap();
    assert_eq!(restored.combat_system.projectiles_snapshot()[0].id, id);
    assert_eq!(restored.objects[&id].template_name, "PatriotMissileEMP");
    assert_missile_object_state_eq(&original.objects[&id], &restored.objects[&id]);
    assert_missile_object_state_eq(&peer_before, &peer.objects[&id]);

    for _ in 0..12 {
        let a = original.combat_system.projectiles_snapshot();
        let b = restored.combat_system.projectiles_snapshot();
        assert_eq!(a.len(), b.len());
        if a.is_empty() { break; }
        assert_eq!(a[0].position, b[0].position);
        assert_eq!(a[0].velocity, b[0].velocity);
        assert_eq!(original.objects[&id].get_position(), a[0].position);
        assert_eq!(restored.objects[&id].get_position(), b[0].position);
        assert_eq!(original.objects[&id].movement.velocity, restored.objects[&id].movement.velocity);
        assert_missile_object_state_eq(&original.objects[&id], &restored.objects[&id]);
        assert_missile_object_state_eq(&peer_before, &peer.objects[&id]);
        let _ = original.update_with_dt(1.0 / 30.0);
        let _ = restored.update_with_dt(1.0 / 30.0);
    }
    assert_missile_object_state_eq(&peer_before, &peer.objects[&id]);
}

#[test]
fn detonated_retail_missile_hides_only_its_model_and_stays_hidden_after_save() {
    use crate::presentation_frame::PresentationFrame;
    // The stock regular Patriot weapon leaves its missile alive through KILL_SELF.
    let (mut original, source, target) = world();
    launch_one(&mut original, source, target);
    let shot = &original.combat_system.projectiles_snapshot()[0];
    assert!(!shot.die_on_detonate);
    let id = shot.id;
    let initial = PresentationFrame::try_build_from_logic(&mut original, 0).unwrap();
    assert!(initial.projectile_render_inputs().iter().any(|row| row.id == id));
    for _ in 0..300 {
        let _ = original.update_with_dt(1.0 / 30.0);
        if original.combat_system.projectiles_snapshot().iter()
            .any(|projectile| projectile.id == id && projectile.missile_drawable_hidden()) {
            break;
        }
    }
    let projectile = original.combat_system.projectiles_snapshot().into_iter()
        .find(|projectile| projectile.id == id).expect("detonated missile is retained");
    assert!(projectile.missile_drawable_hidden());
    assert_eq!(original.objects[&id].template_name, "PatriotMissile");
    assert!(original.objects[&id].drawable_hidden);
    assert!(projectile.is_missile_kill_self_holding());
    let frame = PresentationFrame::try_build_from_logic(&mut original, 0).unwrap();
    assert!(frame.projectiles.iter().any(|row| row.id == id && row.model_hidden));
    assert!(!frame.projectile_render_inputs().iter().any(|row| row.id == id));
    let health_after_impact = original.objects[&target].health.current;
    assert!(health_after_impact < 1000.0, "retail missile actually damaged its target");

    let builder = crate::save_load::SnapshotBuilder::new();
    let saved = builder.create_world_snapshot(&original).unwrap();
    let bytes = bincode_legacy::serialize(&saved).unwrap();
    let decoded = crate::save_load::snapshot::decode_bincode_world_snapshot(&bytes).unwrap();
    let (mut restored, _, _) = world();
    builder.restore_from_snapshot(&decoded, &mut restored).unwrap();
    assert_eq!(restored.objects[&id].template_name, "PatriotMissile");
    assert!(restored.objects[&id].drawable_hidden);
    let restored_frame = PresentationFrame::try_build_from_logic(&mut restored, 0).unwrap();
    assert!(restored_frame.projectiles.iter().any(|row| row.id == id && row.model_hidden));
    assert!(!restored_frame.projectile_render_inputs().iter().any(|row| row.id == id));
    assert_eq!(restored.objects[&target].health.current, health_after_impact);
    for _ in 0..10 {
        if restored.combat_system.projectile_count() == 0 { break; }
        let _ = original.update_with_dt(1.0 / 30.0);
        let _ = restored.update_with_dt(1.0 / 30.0);
        assert_eq!(original.objects[&target].health.current, health_after_impact,
            "retained missile must not repeat its impact");
        assert_eq!(restored.objects[&target].health.current, health_after_impact,
            "restored retained missile must not repeat its impact");
        assert_eq!(original.combat_system.projectile_count(), restored.combat_system.projectile_count());
    }
    assert_eq!(restored.combat_system.projectile_count(), 0, "restored missile completes KILL_SELF");
}

#[test]
fn detonated_retail_emp_missile_retires_flight_with_its_instant_death_object() {
    use crate::presentation_frame::PresentationFrame;
    let (mut world, source, target) = world_for_target(RetailTarget::Vehicle, true);
    launch_one(&mut world, source, target);
    let shot = &world.combat_system.projectiles_snapshot()[0];
    assert!(shot.die_on_detonate, "SupW_PatriotMissileWeapon sets MissileCallsOnDie");
    let id = shot.id;

    for _ in 0..300 {
        let _ = world.update_with_dt(1.0 / 30.0);
        if world.objects[&target].health.current < 1000.0 {
            break;
        }
    }
    let health_after_impact = world.objects[&target].health.current;
    assert!(health_after_impact < 1000.0, "retail EMP warhead reached the target");
    assert!(!world.objects.contains_key(&id), "InstantDeathBehavior removed the missile");
    assert!(!world.combat_system.get_projectiles().contains_key(&id),
        "flight must retire within the same frame as its Object");
    let frame = PresentationFrame::try_build_from_logic(&mut world, 0).unwrap();
    assert!(!frame.projectiles.iter().any(|row| row.id == id));
    crate::save_load::SnapshotBuilder::new().create_world_snapshot(&world)
        .expect("impact frame can be saved without a missing projectile Object");

    for _ in 0..4 {
        let _ = world.update_with_dt(1.0 / 30.0);
        assert_eq!(world.objects[&target].health.current, health_after_impact);
    }
}

#[test]
fn lost_target_kill_self_retains_the_visible_retail_missile() {
    use crate::presentation_frame::PresentationFrame;
    let (mut world, source, target) = world_for_target(RetailTarget::Vehicle, true);
    launch_one(&mut world, source, target);
    let id = world.combat_system.projectiles_snapshot()[0].id;
    for _ in 0..5 {
        let _ = world.update_with_dt(1.0 / 30.0);
    }
    assert!(world.objects[&id].movement.velocity.length() > 0.0);
    world.destroy_object(target).unwrap();
    world.process_destroy_list();
    assert!(world.host_object(target).is_none());
    for _ in 0..10 {
        let _ = world.update_with_dt(1.0 / 30.0);
        if world.combat_system.projectiles_snapshot().iter()
            .any(|projectile| projectile.id == id && projectile.is_missile_kill_self_holding()) {
            break;
        }
    }
    let projectile = world.combat_system.projectiles_snapshot().into_iter()
        .find(|projectile| projectile.id == id).expect("target-loss missile is retained");
    assert!(projectile.is_missile_kill_self_holding());
    assert_eq!(world.objects[&id].template_name, "PatriotMissileEMP");
    assert_eq!(world.objects[&id].get_position(), projectile.position);
    assert!(!projectile.missile_drawable_hidden());
    let frame = PresentationFrame::try_build_from_logic(&mut world, 0).unwrap();
    assert!(frame.projectiles.iter().any(|row| row.id == id && !row.model_hidden));
    assert!(frame.projectile_render_inputs().iter().any(|row| row.id == id));

    let builder = crate::save_load::SnapshotBuilder::new();
    let saved = builder.create_world_snapshot(&world).unwrap();
    let bytes = bincode_legacy::serialize(&saved).unwrap();
    let decoded = crate::save_load::snapshot::decode_bincode_world_snapshot(&bytes).unwrap();
    let (mut restored, _, _) = world_for_target(RetailTarget::Vehicle, true);
    builder.restore_from_snapshot(&decoded, &mut restored).unwrap();
    let before = world.objects[&id].get_position();
    assert_eq!(restored.objects[&id].get_position(), before);
    assert!(restored.combat_system.projectiles_snapshot()[0].is_missile_kill_self_holding());
    let _ = world.update_with_dt(1.0 / 30.0);
    let _ = restored.update_with_dt(1.0 / 30.0);
    assert!(world.combat_system.projectiles_snapshot()[0].is_missile_kill_self_holding());
    assert!(restored.combat_system.projectiles_snapshot()[0].is_missile_kill_self_holding());
    assert_ne!(world.objects[&id].get_position(), before, "KILL_SELF retains moving world physics");
    assert_eq!(restored.objects[&id].get_position(), world.objects[&id].get_position());
    assert_eq!(restored.objects[&id].movement.velocity, world.objects[&id].movement.velocity);
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
    let id = original.combat_system.projectiles_snapshot()[0].id;
    let (mut peer, peer_source, peer_target) = world();
    launch_one(&mut peer, peer_source, peer_target);
    assert_eq!(peer.combat_system.projectiles_snapshot()[0].id, id);
    let peer_before = peer.objects[&id].clone();
    let builder = crate::save_load::SnapshotBuilder::new();
    let saved = builder.create_world_snapshot(&original).unwrap();
    let bytes = bincode_legacy::serialize(&saved).unwrap();
    let decoded = crate::save_load::snapshot::decode_bincode_world_snapshot(&bytes).unwrap();
    let (mut restored, _, _) = world();
    builder.restore_from_snapshot(&decoded, &mut restored).unwrap();
    assert_eq!(restored.combat_system.projectile_count(), 1);
    assert_eq!(restored.combat_system.projectiles_snapshot()[0].position, position);
    assert_eq!(restored.combat_system.projectiles_snapshot()[0].id, id);
    assert_eq!(restored.objects[&id].get_position(), position);
    assert_missile_object_state_eq(&original.objects[&id], &restored.objects[&id]);
    assert_missile_object_state_eq(&peer_before, &peer.objects[&id]);
    let _ = original.update_with_dt(1.0 / 30.0);
    let _ = restored.update_with_dt(1.0 / 30.0);
    assert_eq!(original.combat_system.projectiles_snapshot()[0].position, restored.combat_system.projectiles_snapshot()[0].position);
    assert_missile_object_state_eq(&original.objects[&id], &restored.objects[&id]);
    assert_missile_object_state_eq(&peer_before, &peer.objects[&id]);
    impact(&mut original, target);
    impact(&mut restored, target);
    assert_missile_object_state_eq(&peer_before, &peer.objects[&id]);
}

#[test]
fn superweapon_retail_missile_motion_is_owned_by_its_world_even_with_matching_ids() {
    let (mut first, source, target) = world_for_target(RetailTarget::Vehicle, true);
    let (mut second, other_source, other_target) = world_for_target(RetailTarget::Vehicle, true);
    assert_eq!((source, target), (other_source, other_target));
    launch_one(&mut first, source, target);
    launch_one(&mut second, other_source, other_target);
    let first_id = first.combat_system.projectiles_snapshot()[0].id;
    let second_id = second.combat_system.projectiles_snapshot()[0].id;
    assert_eq!(first_id, second_id);
    assert_eq!(first.objects[&first_id].template_name, "PatriotMissileEMP");
    assert_eq!(second.objects[&second_id].template_name, "PatriotMissileEMP");
    let second_position = second.combat_system.projectiles_snapshot()[0].position;
    let _ = first.update_with_dt(1.0 / 30.0);
    assert_ne!(first.combat_system.projectiles_snapshot()[0].position, second_position);
    assert_ne!(first.objects[&first_id].get_position(), second_position);
    assert_eq!(second.objects[&second_id].get_position(), second_position);
    assert_eq!(second.combat_system.projectiles_snapshot()[0].position, second_position);
    assert_eq!(second.objects[&other_target].health.current, 1000.0);
    let _ = second.update_with_dt(1.0 / 30.0);
    assert_eq!(
        first.combat_system.projectiles_snapshot()[0].position,
        second.combat_system.projectiles_snapshot()[0].position
    );
    assert_eq!(first.objects[&first_id].get_position(), second.objects[&second_id].get_position());
}
