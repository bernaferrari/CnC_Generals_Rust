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
