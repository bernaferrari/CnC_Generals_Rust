//! Real parsed catalog rules, not name-seeded status inference.
//! CPP Weapon.cpp161,303,1259,1383: default NONE remains NONE in DamageInfo.
use super::*;
use crate::game_logic::{GameLogic, KindOf, Team, ThingTemplate};

fn parsed_rule(name: &str, status: &str, expected: gamelogic::common::ObjectStatusTypes) {
    let ini = format!(
        "Weapon {name}\n  PrimaryDamage = 200\n  PrimaryDamageRadius = 0\n  AttackRange = 200\n  DamageType = STATUS\n{status}  WeaponSpeed = 999999\n  DelayBetweenShots = 200\n  ClipSize = 0\n  PreAttackDelay = 0\n  AntiGround = Yes\n  ProjectileObject = NONE\nEnd\n"
    );
    assert_eq!(
        crate::assets::ini_template_loader::register_weapons_from_ini_text(&ini),
        1
    );
    gamelogic::weapon::with_weapon_store(|store| {
        let rule = store.find_weapon_template(name).expect("real parsed rule");
        let actual: gamelogic::common::ObjectStatusTypes = rule.damage_status_type.into();
        assert_eq!(
            actual, expected,
            "verify author admission before regression assertion"
        );
        assert_eq!(rule.damage_type, gamelogic::damage::DamageType::Status);
    })
    .unwrap();
    // This small actual catalog is intentionally complete. No disk loading or
    // golden seed can replace these admitted unique-name definitions.
    gamelogic::weapon::with_weapon_store_mut(|store| store.mark_host_bootstrap_complete()).unwrap();
}

#[test]
fn authored_status_default_none_overrides_avenger_name_fallback() {
    isolated(
        "status_definition_tests::authored_status_default_none_overrides_avenger_name_fallback",
        || {
            let _restore = RestoreStore::install();
            const NAME: &str = "AuthoredAvengerTargetDefaultNone";
            parsed_rule(NAME, "", gamelogic::common::ObjectStatusTypes::None);
            assert!(
                host_weapon_is_status_damage(NAME),
                "witness actually exercises legacy name fallback"
            );
            assert_eq!(
                host_damage_status_type_for_weapon_name(NAME),
                None,
                "CPP default NONE is authored information, not a missing definition"
            );
        },
    );
}

#[test]
fn authored_status_explicit_none_overrides_designator_name_fallback() {
    isolated(
        "status_definition_tests::authored_status_explicit_none_overrides_designator_name_fallback",
        || {
            let _restore = RestoreStore::install();
            const NAME: &str = "AuthoredTargetDesignatorExplicitNone";
            parsed_rule(
                NAME,
                "  DamageStatusType = NONE\n",
                gamelogic::common::ObjectStatusTypes::None,
            );
            assert!(host_weapon_is_status_damage(NAME));
            assert_eq!(
                host_damage_status_type_for_weapon_name(NAME),
                None,
                "explicit NONE must not manufacture FAERIE_FIRE"
            );
        },
    );
}

#[test]
fn authored_faerie_fire_without_seed_name_is_retained() {
    isolated(
        "status_definition_tests::authored_faerie_fire_without_seed_name_is_retained",
        || {
            let _restore = RestoreStore::install();
            const NAME: &str = "AuthoredPaintRule";
            parsed_rule(
                NAME,
                "  DamageStatusType = FAERIE_FIRE\n",
                gamelogic::common::ObjectStatusTypes::FaerieFire,
            );
            assert!(
                !host_weapon_is_status_damage(NAME),
                "authored rule cannot rely on seed recognition"
            );
            assert_eq!(
                host_damage_status_type_for_weapon_name(NAME),
                Some("FAERIE_FIRE")
            );
        },
    );
}

#[test]
fn avenger_fallback_seed_contains_retail_faerie_fire_metadata() {
    isolated(
        "status_definition_tests::avenger_fallback_seed_contains_retail_faerie_fire_metadata",
        || {
            let _restore = RestoreStore::install();
            assert!(crate::game_logic::weapon_bootstrap::store::seed_known_host_weapons() > 0);
            gamelogic::weapon::with_weapon_store(|store| {
            let rule = store.find_weapon_template(AVENGER_TARGET_DESIGNATOR).expect("actual seed rule");
            let status: gamelogic::common::ObjectStatusTypes = rule.damage_status_type.into();
            assert_eq!(rule.damage_type, gamelogic::damage::DamageType::Status);
            assert_eq!(status, gamelogic::common::ObjectStatusTypes::FaerieFire,
                "retail Avenger seed must encode its known status instead of relying on helper inference");
        }).unwrap();
        },
    );
}

fn accepted_status_hit(name: &str, status: &str, expected: bool) {
    let _restore = RestoreStore::install();
    parsed_rule(
        name,
        status,
        if expected {
            gamelogic::common::ObjectStatusTypes::FaerieFire
        } else {
            gamelogic::common::ObjectStatusTypes::None
        },
    );
    let mut world = GameLogic::new();
    let mut shooter = ThingTemplate::new("AuthoredStatusShooter");
    shooter
        .add_kind_of(KindOf::Vehicle)
        .add_kind_of(KindOf::Attackable)
        .set_health(100.0)
        .set_primary_weapon_name(name);
    let mut victim = ThingTemplate::new("AuthoredStatusVictim");
    victim
        .add_kind_of(KindOf::Vehicle)
        .add_kind_of(KindOf::Attackable)
        .set_health(100.0);
    world.templates.insert(shooter.name.clone(), shooter);
    world.templates.insert(victim.name.clone(), victim);
    let source = world
        .create_object("AuthoredStatusShooter", Team::USA, Vec3::ZERO)
        .unwrap();
    let target = world
        .create_object("AuthoredStatusVictim", Team::GLA, Vec3::new(20.0, 0.0, 0.0))
        .unwrap();
    world.set_current_frame(100);
    let source_obj = world.objects.get_mut(&source).unwrap();
    assert_eq!(source_obj.weapon_name_for_slot(0), Some(name));
    assert_eq!(source_obj.weapon.as_ref().unwrap().damage, 200.0);
    let before_fire = source_obj.weapon.as_ref().unwrap().last_fire_time;
    source_obj.attack_target(target);
    assert_eq!(
        source_obj.target,
        Some(target),
        "public attack order accepted"
    );
    world.update_combat(&[source], 1.0 / 30.0);
    assert!(
        world.objects[&source]
            .weapon
            .as_ref()
            .unwrap()
            .last_fire_time
            > before_fire,
        "real combat pass must commit a shot before checking status"
    );
    assert_eq!(
        world.objects[&target].health.current, 100.0,
        "STATUS changes no HP"
    );
    assert_eq!(
        world.objects[&target].is_faerie_fire(),
        expected,
        "live accepted hit must use the parsed definition's exact status"
    );
}

#[test]
fn live_authored_default_none_does_not_paint_from_weapon_name() {
    isolated(
        "status_definition_tests::live_authored_default_none_does_not_paint_from_weapon_name",
        || {
            accepted_status_hit("LiveAuthoredAvengerTargetNone", "", false);
        },
    );
}

#[test]
fn live_authored_faerie_fire_control_paints_without_seed_name() {
    isolated(
        "status_definition_tests::live_authored_faerie_fire_control_paints_without_seed_name",
        || {
            accepted_status_hit(
                "LiveAuthoredPaintRule",
                "  DamageStatusType = FAERIE_FIRE\n",
                true,
            );
        },
    );
}
// Actual Avenger class admission reaches the special primary branch, unlike
// the generic custom-source controls above. Every witness commits a real shot.
fn accepted_actual_avenger_status_hit(name: &str, damage: f32, expected_paint: bool) {
    use crate::game_logic::host_avenger::{
        AVENGER_FAERIE_FIRE_DURATION_FRAMES, AVENGER_PAINT_AUDIO, is_avenger_template,
    };
    let mut world = GameLogic::new();
    let mut shooter = ThingTemplate::new("AmericaTankAvenger");
    shooter
        .add_kind_of(KindOf::Vehicle)
        .add_kind_of(KindOf::Attackable)
        .set_health(100.0)
        .set_primary_weapon_name(name);
    let mut victim = ThingTemplate::new("AvengerAuthoredStatusVictim");
    victim
        .add_kind_of(KindOf::Vehicle)
        .add_kind_of(KindOf::Attackable)
        .set_health(100.0);
    world.templates.insert(shooter.name.clone(), shooter);
    world.templates.insert(victim.name.clone(), victim);
    let source = world
        .create_object("AmericaTankAvenger", Team::USA, Vec3::ZERO)
        .expect("normal factory admits actual Avenger source name");
    let target = world
        .create_object(
            "AvengerAuthoredStatusVictim",
            Team::GLA,
            Vec3::new(20.0, 0.0, 0.0),
        )
        .unwrap();
    world.set_current_frame(100);
    let source_obj = world.objects.get_mut(&source).unwrap();
    assert!(is_avenger_template(&source_obj.template_name));
    assert_eq!(source_obj.weapon_name_for_slot(0), Some(name));
    // Existing normal Avenger factory replaces the authored numeric weapon stats
    // with its zero-HP residual. This witness isolates the separate status gate.
    assert_eq!(source_obj.weapon.as_ref().unwrap().damage, damage);
    let before_fire = source_obj.weapon.as_ref().unwrap().last_fire_time;
    source_obj.attack_target(target);
    assert_eq!(source_obj.target, Some(target));
    assert!(!world.honesty_avenger_paint_ok());
    world.update_combat(&[source], 1.0 / 30.0);
    assert!(
        world.objects[&source]
            .weapon
            .as_ref()
            .unwrap()
            .last_fire_time
            > before_fire,
        "actual default combat must commit the primary shot before assertions"
    );
    assert_eq!(
        world.objects[&target].health.current, 100.0,
        "STATUS changes no HP"
    );
    assert_eq!(
        world.objects[&target].is_faerie_fire(),
        expected_paint,
        "Avenger class cannot override the authored DamageStatusType"
    );
    assert_eq!(world.honesty_avenger_paint_ok(), expected_paint);
    assert_eq!(
        world
            .queued_audio_events
            .iter()
            .any(|event| event.event_type == AVENGER_PAINT_AUDIO),
        expected_paint,
        "paint-specific audio follows the actual admitted status"
    );
    assert_eq!(
        world.objects[&target].faerie_fire_until_frame,
        if expected_paint {
            100 + AVENGER_FAERIE_FIRE_DURATION_FRAMES
        } else {
            0
        },
        "retain existing special FAERIE_FIRE timing without claiming generic duration parity"
    );
}

#[test]
fn actual_avenger_primary_authored_default_none_does_not_paint() {
    isolated(
        "status_definition_tests::actual_avenger_primary_authored_default_none_does_not_paint",
        || {
            let _restore = RestoreStore::install();
            const NAME: &str = "ActualAvengerPrimaryAuthoredNone";
            parsed_rule(NAME, "", gamelogic::common::ObjectStatusTypes::None);
            accepted_actual_avenger_status_hit(NAME, 0.0, false);
        },
    );
}

#[test]
fn actual_avenger_primary_authored_other_status_does_not_manufacture_paint() {
    isolated(
        "status_definition_tests::actual_avenger_primary_authored_other_status_does_not_manufacture_paint",
        || {
            let _restore = RestoreStore::install();
            const NAME: &str = "ActualAvengerPrimaryAuthoredOtherStatus";
            parsed_rule(
                NAME,
                "  DamageStatusType = NO_ATTACK\n",
                gamelogic::common::ObjectStatusTypes::NoAttack,
            );
            // Full NO_ATTACK timer support is independent; this rejects false FAERIE_FIRE.
            accepted_actual_avenger_status_hit(NAME, 0.0, false);
        },
    );
}

#[test]
fn actual_avenger_primary_authored_faerie_retains_paint_audio_and_timing() {
    isolated(
        "status_definition_tests::actual_avenger_primary_authored_faerie_retains_paint_audio_and_timing",
        || {
            let _restore = RestoreStore::install();
            const NAME: &str = "ActualAvengerPrimaryAuthoredFaerie";
            parsed_rule(
                NAME,
                "  DamageStatusType = FAERIE_FIRE\n",
                gamelogic::common::ObjectStatusTypes::FaerieFire,
            );
            accepted_actual_avenger_status_hit(NAME, 0.0, true);
        },
    );
}

#[test]
fn actual_avenger_primary_retail_name_faerie_retains_paint_audio_and_timing() {
    isolated(
        "status_definition_tests::actual_avenger_primary_retail_name_faerie_retains_paint_audio_and_timing",
        || {
            let _restore = RestoreStore::install();
            // Actual retail name and duration metadata. Raw fallback seed enum
            // metadata is verified separately above; its zero-duration estimate
            // does not admit an unlocked ordinary shot.
            parsed_rule(
                AVENGER_TARGET_DESIGNATOR,
                "  DamageStatusType = FAERIE_FIRE\n",
                gamelogic::common::ObjectStatusTypes::FaerieFire,
            );
            accepted_actual_avenger_status_hit(AVENGER_TARGET_DESIGNATOR, 0.0, true);
        },
    );
}
