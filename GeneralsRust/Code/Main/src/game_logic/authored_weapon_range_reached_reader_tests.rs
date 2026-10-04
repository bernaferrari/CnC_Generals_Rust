//! Existing garrison distance policy, actual named admission and actual enter/fire seams.
//! Zero authored sphere radii make that policy equal the C++ geometry distance here.
use super::*;
use crate::game_logic::{
    AIState, ContainAdmission, ContainModuleKind, ContainModuleMetadata, Player,
};

fn garrison_world(
    victim_distance: f32,
) -> (
    GameLogic,
    crate::game_logic::ObjectId,
    crate::game_logic::ObjectId,
) {
    let mut logic = GameLogic::new();
    logic.add_player(Player::new(0, Team::USA, "USA", true));
    logic.add_player(Player::new(1, Team::GLA, "GLA", true));
    let mut bunker = ThingTemplate::new("AuthoredRangeBunker");
    bunker.set_health(1000.0);
    bunker
        .add_kind_of(KindOf::Structure)
        .add_kind_of(KindOf::Attackable);
    bunker.contain_module = ContainModuleMetadata {
        kind: ContainModuleKind::Garrison,
        slots: Some(1),
        admission: ContainAdmission::InfantryOnly,
        is_enclosing_container: true,
        allow_allies_inside: true,
        ..ContainModuleMetadata::default()
    };
    logic.templates.insert(bunker.name.clone(), bunker);
    let mut shooter = ThingTemplate::new("AuthoredRangeGarrisonInfantry");
    shooter.set_health(100.0);
    shooter
        .add_kind_of(KindOf::Infantry)
        .add_kind_of(KindOf::Attackable);
    shooter.set_primary_weapon_name("AuthoredRangeBonus");
    shooter.transport_slot_count = Some(1);
    shooter.geometry_info.major_radius = 0.0;
    shooter.geometry_info.authored = true;
    assert!(shooter.primary_weapon.is_none());
    logic.templates.insert(shooter.name.clone(), shooter);
    let mut victim = ThingTemplate::new("AuthoredRangeGarrisonVictim");
    victim.set_health(100.0);
    victim
        .add_kind_of(KindOf::Vehicle)
        .add_kind_of(KindOf::Attackable);
    victim.geometry_info.major_radius = 0.0;
    victim.geometry_info.authored = true;
    logic.templates.insert(victim.name.clone(), victim);
    let bunker = logic
        .create_object("AuthoredRangeBunker", Team::USA, Vec3::ZERO)
        .unwrap();
    let source = logic
        .create_object("AuthoredRangeGarrisonInfantry", Team::USA, Vec3::ZERO)
        .unwrap();
    assert!(
        logic.can_unit_enter_normal_target(source, bunker),
        "actual authored admission must allow this infantry"
    );
    assert!(logic.unit_command_order_enter(source, bunker));
    assert_eq!(logic.objects[&source].ai_state, AIState::Entering);
    assert_eq!(logic.objects[&source].target, Some(bunker));
    let before_enter_frame = logic.frame;
    logic.update();
    assert_eq!(
        logic.frame,
        before_enter_frame + 1,
        "the normal public tick must advance"
    );
    assert_eq!(
        logic.objects[&source].contained_by,
        Some(bunker),
        "actual enter must complete"
    );
    assert_eq!(logic.objects[&source].ai_state, AIState::Garrisoned);
    assert!(logic.objects[&bunker].contained_units().contains(&source));
    assert_eq!(
        logic.objects[&source]
            .thing
            .template
            .primary_weapon_name
            .as_deref(),
        Some("AuthoredRangeBonus")
    );
    assert_eq!(logic.objects[&source].weapon_slot(0).unwrap().damage, 13.0);
    assert_eq!(
        logic.objects[&source].weapon_bonus_fields().1,
        1.33,
        "actual garrison condition activates RANGE 133%"
    );
    assert_eq!(
        logic.objects[&source]
            .thing
            .template
            .geometry_info
            .bounding_circle_radius(),
        0.0
    );
    let victim = logic
        .create_object(
            "AuthoredRangeGarrisonVictim",
            Team::GLA,
            Vec3::new(victim_distance, 0.0, 0.0),
        )
        .unwrap();
    assert_eq!(
        logic.objects[&victim]
            .thing
            .template
            .geometry_info
            .bounding_circle_radius(),
        0.0
    );
    assert_eq!(
        logic.objects[&source].distance_to_object(&logic.objects[&victim]),
        victim_distance
    );
    logic.frame = 300;
    assert!(crate::game_logic::Object::weapon_ready(
        logic.objects[&source].weapon_slot(0).unwrap(),
        10.0
    ));
    (logic, source, victim)
}

#[test]
fn garrison_named_range_admits_exact_native_bonus_boundary() {
    let _serial = crate::game_logic::combat::tests::combat_test_guard();
    let _restore = RestoreInputs::install();
    let expected = gamelogic::weapon::with_weapon_store(|store| {
        let mut bonus = gamelogic::weapon::WeaponBonus::new();
        bonus.set_field(gamelogic::weapon::WeaponBonusField::Range, 1.33);
        store
            .find_weapon_template("AuthoredRangeBonus")
            .unwrap()
            .get_attack_range(&bonus)
    })
    .unwrap();
    assert_eq!(expected, 130.5);
    let (mut logic, source, victim) = garrison_world(expected);
    // This is the live production garrison fire seam, after the real enter path;
    // it preserves current firepoint fallback and center-distance policies.
    logic.try_garrison_residual_fire(source);
    assert_eq!(
        logic.objects[&victim].health.current, 87.0,
        "one admitted native-boundary shot"
    );
    assert!(logic.honesty_garrison_fire_ok());
    assert_eq!(logic.objects[&source].ai_state, AIState::Garrisoned);
}

#[test]
fn garrison_named_range_rejects_beyond_native_bonus_boundary() {
    let _serial = crate::game_logic::combat::tests::combat_test_guard();
    let _restore = RestoreInputs::install();
    let (mut logic, source, victim) = garrison_world(130.51);
    logic.try_garrison_residual_fire(source);
    assert_eq!(
        logic.objects[&victim].health.current, 100.0,
        "raw100 * RANGE1.33 cannot admit beyond native130.5"
    );
    assert!(!logic.honesty_garrison_fire_ok());
    assert_eq!(logic.objects[&source].ai_state, AIState::Garrisoned);
}

fn base_defense_world(
    victim_distance: f32,
) -> (
    GameLogic,
    crate::game_logic::ObjectId,
    crate::game_logic::ObjectId,
) {
    let mut logic = GameLogic::new();
    let mut defense = ThingTemplate::new("AuthoredRangeCustomDefense");
    defense.set_health(100.0);
    defense
        .add_kind_of(KindOf::Structure)
        .add_kind_of(KindOf::FSBaseDefense)
        .add_kind_of(KindOf::Attackable);
    defense.set_primary_weapon_name("AuthoredRangeBonus");
    defense.geometry_info.major_radius = 0.0;
    defense.geometry_info.authored = true;
    assert!(defense.primary_weapon.is_none());
    logic.templates.insert(defense.name.clone(), defense);
    let mut victim = ThingTemplate::new("AuthoredRangeDefenseVictim");
    victim.set_health(100.0);
    victim
        .add_kind_of(KindOf::Vehicle)
        .add_kind_of(KindOf::Attackable);
    victim.geometry_info.major_radius = 0.0;
    victim.geometry_info.authored = true;
    logic.templates.insert(victim.name.clone(), victim);
    let source = logic
        .create_object("AuthoredRangeCustomDefense", Team::USA, Vec3::ZERO)
        .unwrap();
    let victim = logic
        .create_object(
            "AuthoredRangeDefenseVictim",
            Team::GLA,
            Vec3::new(victim_distance, 0.0, 0.0),
        )
        .unwrap();
    assert!(logic.objects[&source].is_kind_of(KindOf::FSBaseDefense));
    assert_eq!(
        logic.objects[&source]
            .thing
            .template
            .primary_weapon_name
            .as_deref(),
        Some("AuthoredRangeBonus")
    );
    assert_eq!(logic.objects[&source].weapon_slot(0).unwrap().damage, 13.0);
    assert_eq!(logic.objects[&source].weapon_bonus_fields().1, 1.0);
    assert!(
        !logic.objects[&source].turret_enabled,
        "no authored turret: existing direct acquisition/fire lane"
    );
    assert_eq!(
        logic.objects[&source].distance_to_object(&logic.objects[&victim]),
        victim_distance
    );
    logic.frame = 300;
    assert!(crate::game_logic::Object::weapon_ready(
        logic.objects[&source].weapon_slot(0).unwrap(),
        10.0
    ));
    (logic, source, victim)
}

#[test]
fn custom_authored_base_defense_admits_exact_native_range_boundary() {
    let _serial = crate::game_logic::combat::tests::combat_test_guard();
    let _restore = RestoreInputs::install();
    let (mut logic, source, victim) = base_defense_world(97.5);
    logic.update_combat(&[source], 1.0 / 30.0);
    assert_eq!(
        logic.objects[&victim].health.current, 87.0,
        "normal FSBaseDefense dispatch admits native max97.5"
    );
}

#[test]
fn custom_authored_base_defense_rejects_beyond_native_range_boundary() {
    let _serial = crate::game_logic::combat::tests::combat_test_guard();
    let _restore = RestoreInputs::install();
    let (mut logic, source, victim) = base_defense_world(97.51);
    logic.update_combat(&[source], 1.0 / 30.0);
    assert_eq!(
        logic.objects[&victim].health.current, 100.0,
        "normal FSBaseDefense picker must not use raw100 as firing range"
    );
}

#[path = "authored_weapon_range_slot_tests.rs"]
mod slot_contract_tests;
