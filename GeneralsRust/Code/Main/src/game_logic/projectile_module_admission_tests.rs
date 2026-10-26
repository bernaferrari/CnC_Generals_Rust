//! Actual receiving archive admission, not a second projectile runtime.
use super::*;
use crate::game_logic::GameLogic;

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
    assert!(owner.apply_receiving_module_overrides(&["Object MapProjectile\nReplaceModule FlightTag\nBehavior = MissileAIUpdate BrokenFlight\nFuelLifetime = invalid\nEnd\nEnd\nEnd".into()]).is_err());
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
