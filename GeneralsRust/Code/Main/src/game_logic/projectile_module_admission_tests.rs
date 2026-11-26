//! Actual receiving archive admission, not a second projectile runtime.
use super::*;
use crate::game_logic::{GameLogic, Player, Team};

fn definitions(fields: &str) -> Vec<(String, ObjectDefinition)> {
    let text = format!(
        "Object AuthoredProjectile\n Behavior = DestroyDie First\n End\n Behavior = DumbProjectileBehavior Flight\n FirstHeight = 12\n End\n Behavior = PhysicsBehavior Physics\n {fields}\n End\n Behavior = LifetimeUpdate Last\n MinLifetime = 3000\n MaxLifetime = 3000\n End\nEnd\n"
    );
    let mut parser = crate::assets::ini_parser::IniParser::new();
    parser
        .parse_ini_content(&text, "owned-projectile.ini")
        .unwrap();
    parser
        .get_all_definitions()
        .iter()
        .map(|(name, row)| (name.clone(), row.clone()))
        .collect()
}

#[test]
fn missile_launch_uses_receiving_locomotor_and_physics_content() {
    let text = "Object OwnedMissile\n Locomotor = SET_NORMAL OwnedMissileLoco\n Behavior = MissileAIUpdate Flight\n InitialVelocity = 90\n UseWeaponSpeed = Yes\n DistanceToTravelBeforeTurning = 25\n DistanceToTargetBeforeDiving = 40\n End\n Behavior = PhysicsBehavior Physics\n Mass = 7\n End\nEnd\n";
    let mut parser = crate::assets::ini_parser::IniParser::new();
    parser.parse_ini_content(text, "missile.ini").unwrap();
    let rows: Vec<_> = parser.get_all_definitions().iter()
        .map(|(name, row)| (name.clone(), row.clone())).collect();
    let mut first = GameLogic::new();
    let mut second = GameLogic::new();
    for (world, speed) in [(&mut first, 60), (&mut second, 120)] {
        let mut store = game_engine::common::ini::ini_locomotor::LocomotorStore::new_owned();
        store.apply_source(&format!("Locomotor OwnedMissileLoco\n Surfaces = AIR\n Speed = {speed}\n Appearance = THRUST\nEnd\n")).unwrap();
        world.admit_locomotor_definitions(store).unwrap();
        world.admit_weapon_content_snapshot(Vec::new(), &rows).unwrap();
    }
    let a = first.projectile_definitions.get("OwnedMissile").unwrap();
    let b = second.projectile_definitions.get("OwnedMissile").unwrap();
    assert_eq!(a.locomotor.unwrap().movement.max_speed, 60.0);
    assert_eq!(b.locomotor.unwrap().movement.max_speed, 120.0);
    assert_eq!(a.behaviors.iter().find_map(|module| module.physics.as_ref()).unwrap().mass, 7.0);
    let launch = a.missile_launch.unwrap();
    assert_eq!(launch.initial_velocity_per_frame, 3.0);
    assert!(launch.use_weapon_speed);
    assert_eq!(launch.distance_before_turning, 25.0);
    assert_eq!(launch.distance_before_diving, 40.0);
    let mut absent = GameLogic::new();
    absent.admit_weapon_content_snapshot(Vec::new(), &rows).unwrap();
    assert!(absent.projectile_definitions.get("OwnedMissile").unwrap().locomotor.is_none());
}

#[test]
fn weapon_speed_caps_authored_missile_locomotor_without_raising_either_condition() {
    let source = "Object SpeedCapMissile\n\
KindOf = PROJECTILE\n\
Body = ActiveBody Body\nMaxHealth = 1\nInitialHealth = 1\nEnd\n\
Locomotor = SET_NORMAL SpeedCapLocomotor\n\
Behavior = MissileAIUpdate Flight\nUseWeaponSpeed = Yes\nEnd\n\
Behavior = PhysicsBehavior Physics\nMass = 1\nEnd\nEnd\n";
    let mut parser = crate::assets::ini_parser::IniParser::new();
    parser.parse_ini_content(source, "speed-cap.ini").unwrap();
    let rows: Vec<_> = parser.get_all_definitions().iter()
        .map(|(name, row)| (name.clone(), row.clone())).collect();
    let mut world = GameLogic::new();
    let mut player = Player::new(0, Team::China, "missile owner", false);
    player.alliance_team = 0;
    world.add_player(player);
    let mut locomotors = game_engine::common::ini::ini_locomotor::LocomotorStore::new_owned();
    locomotors.apply_source("Locomotor SpeedCapLocomotor\nSurfaces = AIR\nSpeed = 60\nSpeedDamaged = 45\nAcceleration = 1200\nAccelerationDamaged = 600\nAppearance = THRUST\nEnd\n").unwrap();
    world.admit_locomotor_definitions(locomotors).unwrap();
    world.admit_receiving_module_content(crate::assets::ReceivingObjectContent::from_sources([
        ("speed-cap.ini".into(), source.into()),
    ]).unwrap()).unwrap();
    world.admit_weapon_content_snapshot(Vec::new(), &rows).unwrap();

    for (weapon_speed, expected) in [
        (100.0, (60.0, 45.0, 1200.0, 600.0)),
        (10.0, (10.0, 10.0, 300.0, 300.0)),
    ] {
        let mut shot = crate::game_logic::combat::tests::lifecycle_test_pending_projectile(
            "SpeedCapMissile", None, glam::Vec3::new(100.0, 30.0, 0.0),
        );
        shot.speed = weapon_speed;
        let context = shot.source_context.as_mut().unwrap();
        context.source_owner_player_id = Some(0);
        context.launch_transform = Some(glam::Mat4::from_translation(glam::Vec3::new(0.0, 30.0, 0.0)));
        world.materialize_owned_passive_shot(shot, None).unwrap();
        let projectile = world.combat_system.projectiles_snapshot().into_iter()
            .find(|shot| (shot.speed - weapon_speed).abs() < 0.001).unwrap();
        let actor = world.host_object(projectile.id).expect("factory-created missile Object");
        let actual = (actor.movement.max_speed, actor.movement.max_speed_damaged,
            actor.movement.acceleration, actor.movement.acceleration_damaged);
        for (actual, expected) in [actual.0, actual.1, actual.2, actual.3]
            .into_iter().zip([expected.0, expected.1, expected.2, expected.3]) {
            assert!((actual - expected).abs() < 0.01, "actual {actual}, expected {expected}");
        }
    }
}

#[test]
fn vanished_queued_target_does_not_run_missile_factory_callbacks() {
    let source = "Object CallbackMissile\n\
KindOf = PROJECTILE\n\
Body = ActiveBody Body\nMaxHealth = 1\nInitialHealth = 1\nEnd\n\
Locomotor = SET_NORMAL CallbackLocomotor\n\
Behavior = MissileAIUpdate Flight\nTryToFollowTarget = Yes\nEnd\n\
Behavior = PhysicsBehavior Physics\nMass = 1\nEnd\n\
Behavior = GrantUpgradeCreate Grant\nUpgradeToGrant = Upgrade_CallbackRadar\nExemptStatus = UNDER_CONSTRUCTION\nEnd\nEnd\n\
Object CallbackTarget\nKindOf = VEHICLE\nBody = ActiveBody Body\nMaxHealth = 100\nInitialHealth = 100\nEnd\nEnd\n";
    let mut parser = crate::assets::ini_parser::IniParser::new();
    parser.parse_ini_content(source, "callback-missile.ini").unwrap();
    let rows: Vec<_> = parser.get_all_definitions().iter()
        .map(|(name, row)| (name.clone(), row.clone())).collect();
    let mut world = GameLogic::new();
    world.add_player(Player::new(0, Team::China, "owner", false));
    crate::game_logic::host_upgrade_rules::register_test_upgrade(
        &mut world, "Upgrade_CallbackRadar", "PLAYER", 0, 1,
    );
    let mut locomotors = game_engine::common::ini::ini_locomotor::LocomotorStore::new_owned();
    locomotors.apply_source("Locomotor CallbackLocomotor\nSurfaces = AIR\nSpeed = 60\nAppearance = THRUST\nEnd\n").unwrap();
    world.admit_locomotor_definitions(locomotors).unwrap();
    world.admit_receiving_module_content(crate::assets::ReceivingObjectContent::from_sources([
        ("callback-missile.ini".into(), source.into()),
    ]).unwrap()).unwrap();
    world.admit_weapon_content_snapshot(Vec::new(), &rows).unwrap();
    let target = world.create_object_for_player("CallbackTarget", 0, glam::Vec3::X * 100.0).unwrap();
    let mut shot = crate::game_logic::combat::tests::lifecycle_test_pending_projectile(
        "CallbackMissile", Some(target), glam::Vec3::X * 100.0,
    );
    shot.target_pos = None;
    shot.fire_fx_name = "FX_AcceptedBeforeTargetLoss".into();
    let context = shot.source_context.as_mut().unwrap();
    context.source_owner_player_id = Some(0);
    context.launch_transform = Some(glam::Mat4::from_translation(glam::Vec3::Y * 30.0));
    crate::game_logic::combat::queue_projectile(&mut world.combat_system, shot.clone());
    world.destroy_object(target).unwrap();
    world.process_destroy_list_if_needed_with_effects_checked(&mut Default::default()).unwrap();
    assert!(world.host_object(target).is_none(), "queued target is physically removed");
    let before_count = world.host_objects().registered_len();
    // Observe the accepted FX at the queued materialization boundary, before
    // the normal drain dispatches and consumes that presentation event.
    world.materialize_owned_queued_projectiles_with_effects(&mut Default::default()).unwrap();
    assert!(!world.get_player(0).unwrap().completed_upgrades.contains("Upgrade_CallbackRadar"),
        "missile onCreate must not grant its player upgrade");
    assert_eq!(world.host_objects().registered_len(), before_count, "no projectile factory admission");
    assert_eq!(world.combat_system.projectile_count(), 0);
    let fire_effects = world.combat_system.take_fire_ocl();
    assert_eq!(fire_effects.len(), 1);
    assert_eq!(fire_effects[0].fire_fx_name, "FX_AcceptedBeforeTargetLoss");

    // A captured position remains a valid aim even though its object is gone.
    shot.target_pos = Some(glam::Vec3::X * 100.0);
    world.materialize_owned_passive_shot(shot, None).unwrap();
    assert_eq!(world.combat_system.projectile_count(), 1);
    assert!(world.get_player(0).unwrap().completed_upgrades.contains("Upgrade_CallbackRadar"),
        "valid shot ran onCreate");

    let mut coordinate = crate::game_logic::combat::tests::lifecycle_test_pending_projectile(
        "CallbackMissile", None, glam::Vec3::X * 120.0,
    );
    let context = coordinate.source_context.as_mut().unwrap();
    context.source_owner_player_id = Some(0);
    context.launch_transform = Some(glam::Mat4::from_translation(glam::Vec3::Y * 30.0));
    world.materialize_owned_passive_shot(coordinate, None).unwrap();
    assert_eq!(world.combat_system.projectile_count(), 2);

    let live_target = world.create_object_for_player(
        "CallbackTarget", 0, glam::Vec3::X * 140.0,
    ).unwrap();
    let mut tracked = crate::game_logic::combat::tests::lifecycle_test_pending_projectile(
        "CallbackMissile", Some(live_target), glam::Vec3::ZERO,
    );
    tracked.target_pos = None;
    let context = tracked.source_context.as_mut().unwrap();
    context.source_owner_player_id = Some(0);
    context.launch_transform = Some(glam::Mat4::from_translation(glam::Vec3::Y * 30.0));
    world.materialize_owned_passive_shot(tracked, None).unwrap();
    assert_eq!(world.combat_system.projectile_count(), 3);
}

#[test]
fn receiving_profile_keeps_all_authored_modules_tags_and_physics_after_flight() {
    let mut owner = GameLogic::new();
    let rows = definitions("Mass = 7\n ForwardFriction = 6\n AllowBouncing = Yes");
    owner
        .admit_weapon_content_snapshot(Vec::new(), &rows)
        .unwrap();
    let profile = owner
        .projectile_definitions
        .get("AuthoredProjectile")
        .unwrap();
    let tags: Vec<_> = profile
        .behaviors
        .iter()
        .map(|module| {
            (
                module.authored.class_name.as_str(),
                module.authored.module_tag.as_deref(),
            )
        })
        .collect();
    assert_eq!(
        tags,
        vec![
            ("DestroyDie", Some("First")),
            ("DumbProjectileBehavior", Some("Flight")),
            ("PhysicsBehavior", Some("Physics")),
            ("LifetimeUpdate", Some("Last"))
        ]
    );
    let physics = profile.behaviors[2]
        .physics
        .as_ref()
        .expect("actual later Physics definition");
    assert_eq!(physics.mass, 7.0);
    assert_eq!(physics.forward_friction, 6.0 * (1.0 / 30.0));
    assert_eq!(physics.lateral_friction, 0.15, "default already per frame");
    assert!(physics.allow_bouncing);
    assert!(profile.behaviors[3].physics.is_none());
    assert_eq!(
        profile.behaviors[3].authored.attribute("MinLifetime"),
        Some("3000")
    );
}

#[test]
fn same_name_receiving_physics_defaults_and_height_use_each_content_owner() {
    let rows = definitions("MinFallHeightForDamage = 80");
    let mut owner = GameLogic::new();
    let mut peer = GameLogic::new();
    for (world, gravity) in [(&mut owner, -2.0), (&mut peer, -5.0)] {
        let mut rules = world.physics_rules_baseline();
        rules.gravity = gravity;
        world.admit_physics_rules_base(rules).unwrap();
        world
            .admit_weapon_content_snapshot(Vec::new(), &rows)
            .unwrap();
    }
    let speed = |world: &GameLogic| {
        world
            .projectile_definitions
            .get("AuthoredProjectile")
            .unwrap()
            .behaviors[2]
            .physics
            .as_ref()
            .unwrap()
            .min_fall_speed_for_damage
    };
    assert_eq!(speed(&owner), 320.0_f32.sqrt());
    assert_eq!(speed(&peer), 800.0_f32.sqrt());
    peer.reset();
    assert_eq!(speed(&owner), 320.0_f32.sqrt());
    assert_eq!(speed(&peer), 800.0_f32.sqrt());
    owner
        .admit_weapon_content_snapshot(Vec::new(), &definitions(""))
        .unwrap();
    assert_eq!(speed(&owner), 160.0_f32.sqrt(), "default C++ height is40");
}

#[test]
fn invalid_later_physics_cannot_replace_admitted_archive_catalog() {
    let mut owner = GameLogic::new();
    owner
        .admit_weapon_content_snapshot(Vec::new(), &definitions("Mass = 7"))
        .unwrap();
    for invalid in [
        "Mass = 0",
        "Mass = NaN",
        "ForwardFriction = inf",
        "UnknownPhysicsField = 1",
    ] {
        assert!(
            owner
                .admit_weapon_content_snapshot(Vec::new(), &definitions(invalid))
                .is_err(),
            "later authored Physics must reject {invalid}"
        );
        let profile = owner
            .projectile_definitions
            .get("AuthoredProjectile")
            .unwrap();
        assert_eq!(profile.behaviors[2].physics.as_ref().unwrap().mass, 7.0);
        assert_eq!(
            profile.behaviors[2].authored.module_tag.as_deref(),
            Some("Physics")
        );
    }
}

#[test]
fn mixed_case_physics_scalar_uses_last_authored_value() {
    let rows = definitions("Mass = 7\n mass = 9\n ForwardFriction = 6\n forwardfriction = 3");
    let row = rows
        .iter()
        .find(|(name, _)| name == "AuthoredProjectile")
        .unwrap();
    assert_eq!(row.1.behavior_modules[2].attributes.len(), 2);
    let mut world = GameLogic::new();
    world
        .admit_weapon_content_snapshot(Vec::new(), &rows)
        .unwrap();
    let data = world
        .projectile_definitions
        .get("AuthoredProjectile")
        .unwrap()
        .behaviors[2]
        .physics
        .as_ref()
        .unwrap();
    assert_eq!(data.mass, 9.0);
    assert_eq!(data.forward_friction, 3.0 * (1.0 / 30.0));
}

#[test]
fn mixed_case_repeated_fx_fields_retain_source_order() {
    let text = "Object OrderedFX\n Behavior = SlowDeathBehavior Death\n FX = INITIAL First\n fx = FINAL Last\n End\nEnd\n";
    let mut parser = crate::assets::ini_parser::IniParser::new();
    parser.parse_ini_content(text, "ordered.ini").unwrap();
    let row = &parser.get_all_definitions()["OrderedFX"];
    assert_eq!(row.behavior_modules[0].attributes.len(), 1);
    assert_eq!(
        row.behavior_modules[0].attribute("FX"),
        Some("INITIAL First\nFINAL Last")
    );
}

#[test]
fn projectile_physics_rebinds_only_with_its_receiving_validated_gravity() {
    let rows = definitions("MinFallHeightForDamage = 50");
    let mut owner = GameLogic::new();
    let mut peer = GameLogic::new();
    owner
        .admit_weapon_content_snapshot(Vec::new(), &rows)
        .unwrap();
    peer.admit_weapon_content_snapshot(Vec::new(), &rows)
        .unwrap();
    let speed = |world: &GameLogic| {
        world
            .projectile_definitions
            .get("AuthoredProjectile")
            .unwrap()
            .behaviors
            .iter()
            .find_map(|module| module.physics.as_ref())
            .unwrap()
            .min_fall_speed_for_damage
    };
    let baseline = speed(&peer);
    let mut rules = owner.physics_rules_baseline();
    rules.gravity *= 4.0;
    owner.admit_physics_map_rules(rules).unwrap();
    assert!((speed(&owner) - baseline * 2.0).abs() < 0.00001);
    assert_eq!(speed(&peer), baseline);
    let before = speed(&owner);
    let before_rules = owner.physics_definitions.effective;
    rules.gravity = f32::NAN;
    assert!(owner.admit_physics_map_rules(rules).is_err());
    assert_eq!(speed(&owner), before);
    assert_eq!(
        owner.physics_definitions.effective.gravity,
        before_rules.gravity
    );
    assert_eq!(speed(&peer), baseline);
}

#[test]
fn receiving_map_solo_and_reset_rebuild_only_the_effective_projectile_catalog() {
    fn world(fuel: u32) -> GameLogic {
        let text = format!(
            "Object MapProjectile\nKindOf = PROJECTILE SMALL_MISSILE\nBody = ActiveBody BodyTag\nMaxHealth = 1\nInitialHealth = 1\nEnd\nBehavior = MissileAIUpdate FlightTag\nFuelLifetime = {fuel}\nEnd\nBehavior = PhysicsBehavior PhysicsTag\nMass = 1\nEnd\nEnd"
        );
        let mut parser = crate::assets::IniParser::new();
        parser
            .parse_ini_content(&text, "receiving-projectile.ini")
            .unwrap();
        let rows: Vec<_> = parser
            .get_all_definitions()
            .iter()
            .map(|(name, row)| (name.clone(), row.clone()))
            .collect();
        let mut world = GameLogic::new();
        world
            .admit_weapon_content_snapshot(Vec::new(), &rows)
            .unwrap();
        world.seed_asset_definition_templates_from_snapshot(rows);
        world
            .admit_receiving_module_content(
                crate::assets::ReceivingObjectContent::from_sources([(
                    "receiving-projectile.ini".into(),
                    text,
                )])
                .unwrap(),
            )
            .unwrap();
        world
    }
    let mut owner = world(1000);
    let peer = world(10000);
    let fuel = |world: &GameLogic| match world
        .projectile_definitions
        .get("MapProjectile")
        .unwrap()
        .lifecycle
    {
        HostProjectileLifecycle::Missile {
            fuel_lifetime_frames,
            ..
        } => fuel_lifetime_frames,
        _ => panic!("actual receiving Missile"),
    };
    assert_eq!(fuel(&owner), 30);
    assert_eq!(fuel(&peer), 300);
    owner.apply_receiving_module_overrides(&[
        "Object MapProjectile\nReplaceModule FlightTag\nBehavior = MissileAIUpdate MapFlight\nFuelLifetime = 2000\nEnd\nEnd\nEnd".into(),
        "Object MapProjectile\nReplaceModule MapFlight\nBehavior = MissileAIUpdate SoloFlight\nFuelLifetime = 3000\nEnd\nEnd\nEnd".into(),
    ]).unwrap();
    assert_eq!(fuel(&owner), 90);
    assert_eq!(fuel(&peer), 300);
    let profile = owner.projectile_definitions.get("MapProjectile").unwrap();
    let effective = owner
        .receiving_object_definitions
        .as_ref()
        .unwrap()
        .get("MapProjectile")
        .unwrap();
    assert_eq!(
        profile
            .behaviors
            .iter()
            .map(|m| m.authored.module_tag.as_deref())
            .collect::<Vec<_>>(),
        effective
            .behavior_modules
            .iter()
            .map(|m| m.module_tag.as_deref())
            .collect::<Vec<_>>()
    );
    let error = owner.apply_receiving_module_overrides(&["Object MapProjectile\nReplaceModule FlightTag\nBehavior = MissileAIUpdate BrokenFlight\nFuelLifetime = invalid\nEnd\nEnd\nEnd".into()]).unwrap_err();
    use game_engine::common::thing::module_factory::ReceivingModuleError;
    assert!(error.to_string().contains("Object MapProjectile at line 1"));
    let mut cause = &error;
    while let ReceivingModuleError::Context(_, inner) = cause {
        cause = inner;
    }
    assert!(matches!(
        cause,
        ReceivingModuleError::Ini(game_engine::common::ini::INIError::InvalidData)
    ));
    assert_eq!(fuel(&owner), 90);
    assert_eq!(fuel(&peer), 300);
    owner.apply_receiving_module_overrides(&[]).unwrap();
    assert_eq!(fuel(&owner), 30);
    assert_eq!(fuel(&peer), 300);
    owner.apply_receiving_module_overrides(&["Object MapProjectile\nReplaceModule FlightTag\nBehavior = MissileAIUpdate FinalMapFlight\nFuelLifetime = 2000\nEnd\nEnd\nEnd".into()]).unwrap();
    assert_eq!(fuel(&owner), 60);
    owner.reset();
    assert_eq!(fuel(&owner), 30);
    assert_eq!(fuel(&peer), 300);
}
