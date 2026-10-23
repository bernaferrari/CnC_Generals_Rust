//! Driving-clock evidence at the real accepted-shot -> materialization ->
//! ready-consumer boundary. Finite NONE shots reach frame103 from frame100;
//! a foreign ordinary drain cannot change the receiving body clock.
//! C++ ActiveBody.cpp:547-572,817-820. No private shot/timestamp injection.
use super::*;

const FINITE_DAMAGE: &str = "AcceptedImpactFrameDamage";
const FINITE_HEALING: &str = "AcceptedImpactFrameHealing";

fn assert_finite_weapon(world: &GameLogic, name: &str, damage_type: &str) {
    let catalog = world
        .weapon_definitions
        .as_ref()
        .expect("receiving immutable rules");
    let rules = &catalog[&name.to_ascii_lowercase()];
    assert_eq!(rules.primary_damage, 17.0);
    assert_eq!(rules.weapon_speed.to_bits(), 0x4120_0001);
    assert!(rules.projectile_name.eq_ignore_ascii_case("NONE"));
    assert_eq!(rules.clip_size, 8);
    assert_eq!(
        rules.damage_type,
        if damage_type == "HEALING" {
            gamelogic::damage::DamageType::Healing
        } else {
            gamelogic::damage::DamageType::SmallArms
        }
    );
}

fn consume_before_due(world: &mut GameLogic, target: ObjectId, hp: f32) {
    world.set_current_frame(102);
    consume_only(world);
    assert_eq!(world.objects[&target].health.current, hp);
    world.set_current_frame(103);
}

fn materialize_only(world: &mut GameLogic) {
    drain_pending_projectiles(&mut world.combat_system, world.objects.read(), world.frame);
    assert_eq!(world.combat_system.projectile_count(), 0);
}

fn consume_only(world: &mut GameLogic) {
    apply_ready_projectileless_delayed_damage(
        &mut world.combat_system,
        &mut world.objects,
        world.frame,
        Some(&world.players),
        &mut world.health_events,
        &mut world.ownership_events,
    );
}

#[test]
fn accepted_impact_timestamp_uses_driving_frame_after_foreign_normal_drain() {
    let (mut first, source, target) = world(FINITE_DAMAGE);
    assert_finite_weapon(&first, FINITE_DAMAGE, "SMALL_ARMS");
    let (mut second, other_source, other_target) = world(SECOND_WEAPON);
    second.set_current_frame(300);
    assert_eq!((source, target), (other_source, other_target));
    accept(&mut first, source, target, 10.0);
    materialize_only(&mut first);
    assert_eq!(first.objects[&target].health.current, 100.0);
    // A genuine ordinary second-world drain publishes frame300. The first
    // ready consumer receives its own due frame103, independently of that drain.
    second.drain_pending_projectiles_into_combat();
    consume_before_due(&mut first, target, 100.0);
    assert_eq!(first.objects[&target].last_damage_timestamp, None);
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
        Some(103),
        "actual damage uses the receiving consumer frame after the foreign drain"
    );
}

#[test]
fn accepted_impact_same_owner_clock_control_preserves_hp_source_and_timestamp() {
    let (mut first, source, target) = world(FINITE_DAMAGE);
    assert_finite_weapon(&first, FINITE_DAMAGE, "SMALL_ARMS");
    first.drain_pending_projectiles_into_combat();
    accept(&mut first, source, target, 10.0);
    materialize_only(&mut first);
    assert_eq!(first.objects[&target].health.current, 100.0);
    consume_before_due(&mut first, target, 100.0);
    assert_eq!(first.objects[&target].last_damage_timestamp, None);
    consume_only(&mut first);
    assert_eq!(first.objects[&target].health.current, 83.0);
    assert_eq!(first.objects[&target].last_damage_source, Some(source));
    assert_eq!(first.objects[&target].last_damage_timestamp, Some(103));
    assert_eq!(first.objects[&target].last_healing_timestamp, None);
    consume_only(&mut first);
    assert_eq!(first.objects[&target].health.current, 83.0);
}

#[test]
fn accepted_healing_timestamp_uses_driving_frame_after_foreign_normal_drain() {
    let (mut first, source, target) = world(FIRST_WEAPON);
    assert_finite_weapon(&first, FINITE_HEALING, "HEALING");
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
    consume_before_due(&mut first, target, 83.0);
    assert_eq!(first.objects[&target].last_damage_timestamp, Some(100));
    assert_eq!(first.objects[&target].last_healing_timestamp, None);
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
        Some(103),
        "CPP healing stamps the actual receiving frame103"
    );
    assert_eq!(first.objects[&target].last_damage_timestamp, Some(103));
}
