//! Driving-clock evidence at the real accepted-shot -> materialization ->
//! ready-consumer boundary. Normal GameLogic drain still publishes an ambient
//! compatibility clock; this tests the actual consumer's supplied frame.
//! C++ ActiveBody.cpp:547-572,817-820. No private shot/timestamp injection.
use super::*;

fn materialize_only(world: &mut GameLogic) {
    drain_pending_projectiles(&mut world.combat_system, &world.objects, world.frame);
    assert_eq!(world.combat_system.projectile_count(), 0);
}

fn consume_only(world: &mut GameLogic) {
    apply_ready_projectileless_delayed_damage(
        &mut world.combat_system,
        &mut world.objects,
        world.frame,
        Some(&world.players),
        &mut world.health_events,
    );
}

#[test]
fn accepted_impact_timestamp_uses_driving_frame_after_foreign_normal_drain() {
    let _serial = combat_test_guard();
    let _restore = RestoreInputs::install();
    let (mut first, source, target) = world(FIRST_WEAPON);
    let (mut second, other_source, other_target) = world(SECOND_WEAPON);
    second.set_current_frame(300);
    assert_eq!((source, target), (other_source, other_target));
    accept(&mut first, source, target, 10.0);
    materialize_only(&mut first);
    assert_eq!(first.objects[&target].health.current, 100.0);
    // A genuine ordinary second-world drain publishes frame300. The first
    // ready consumer already accepts its own frame100 and must retain it.
    second.drain_pending_projectiles_into_combat();
    assert_eq!(crate::game_logic::host_historic_bonus::logic_frame(), 300);
    consume_only(&mut first);
    assert_eq!(first.objects[&target].health.current, 83.0);
    assert_eq!(first.objects[&target].last_damage_source, Some(source));
    assert_eq!(
        first.objects[&target].last_damage_info_type,
        Some(crate::game_logic::combat::DamageType::Bullet)
    );
    assert_eq!(second.objects[&other_target].health.current, 100.0);
    assert_eq!(second.objects[&other_target].last_damage_timestamp, None);
    consume_only(&mut first);
    assert_eq!(
        first.objects[&target].health.current, 83.0,
        "consumed impact never repeats"
    );
    assert_eq!(
        first.objects[&target].last_damage_timestamp,
        Some(100),
        "actual damage uses consumer frame, not foreign published300"
    );
}

#[test]
fn accepted_impact_same_owner_clock_control_preserves_hp_source_and_timestamp() {
    let _serial = combat_test_guard();
    let _restore = RestoreInputs::install();
    let (mut first, source, target) = world(FIRST_WEAPON);
    first.drain_pending_projectiles_into_combat(); // actual own empty drain publishes100
    assert_eq!(crate::game_logic::host_historic_bonus::logic_frame(), 100);
    accept(&mut first, source, target, 10.0);
    materialize_only(&mut first);
    assert_eq!(first.objects[&target].health.current, 100.0);
    consume_only(&mut first);
    assert_eq!(first.objects[&target].health.current, 83.0);
    assert_eq!(first.objects[&target].last_damage_source, Some(source));
    assert_eq!(first.objects[&target].last_damage_timestamp, Some(100));
    assert_eq!(first.objects[&target].last_healing_timestamp, None);
    consume_only(&mut first);
    assert_eq!(first.objects[&target].health.current, 83.0);
}

#[test]
fn accepted_healing_timestamp_uses_driving_frame_after_foreign_normal_drain() {
    let _serial = combat_test_guard();
    let _restore = RestoreInputs::install();
    assert_eq!(
        crate::assets::ini_template_loader::register_weapons_from_ini_text(
            r#"
Weapon AcceptedImpactFrameHealing
  PrimaryDamage = 17
  PrimaryDamageRadius = 0
  AttackRange = 200
  DamageType = HEALING
  ProjectileObject = NONE
  WeaponSpeed = 30000000
  DelayBetweenShots = 1000
  ClipSize = 8
  ClipReloadTime = 1000
  PreAttackDelay = 0
  AntiGround = Yes
End
"#
        ),
        1
    );
    gamelogic::weapon::with_weapon_store(|store| {
        let rules = store
            .find_weapon_template("AcceptedImpactFrameHealing")
            .unwrap();
        assert_eq!(rules.primary_damage, 17.0);
        assert_eq!(rules.damage_type, gamelogic::damage::DamageType::Healing);
        assert!(rules.projectile_name.eq_ignore_ascii_case("NONE"));
    })
    .unwrap();
    let (mut first, source, target) = world(FIRST_WEAPON);
    let (mut second, other_source, other_target) = world(SECOND_WEAPON);
    second.set_current_frame(300);
    assert_eq!((source, target), (other_source, other_target));
    // Damage creates the actual healing opportunity, without assigning HP.
    accept(&mut first, source, target, 10.0);
    first.drain_pending_projectiles_into_combat();
    assert_eq!(first.objects[&target].health.current, 83.0);
    let healer = admit(
        &mut first,
        "AcceptedImpactFrameHealer",
        Some("AcceptedImpactFrameHealing"),
        Team::USA,
        Vec3::ZERO,
    );
    let target_pos = first.objects[&target].get_position();
    first.objects.get_mut(&healer).unwrap().prev_victim_pos = Some(target_pos);
    accept(&mut first, healer, target, 11.0);
    materialize_only(&mut first);
    assert_eq!(first.objects[&target].health.current, 83.0);
    second.drain_pending_projectiles_into_combat();
    assert_eq!(crate::game_logic::host_historic_bonus::logic_frame(), 300);
    consume_only(&mut first);
    assert_eq!(first.objects[&target].health.current, 100.0);
    assert_eq!(first.objects[&target].last_damage_source, Some(healer));
    assert_eq!(
        first.objects[&target].last_damage_info_type,
        Some(crate::game_logic::combat::DamageType::Healing)
    );
    assert_eq!(second.objects[&other_target].health.current, 100.0);
    consume_only(&mut first);
    assert_eq!(first.objects[&target].health.current, 100.0);
    assert_eq!(
        first.objects[&target].last_healing_timestamp,
        Some(100),
        "CPP healing stamps this game's frame100"
    );
    assert_eq!(first.objects[&target].last_damage_timestamp, Some(100));
}
