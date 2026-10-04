//! Actual admitted normal FIRE path: driving frame remains distinct from f32 time.
//! CPP Weapon.cpp:2573,2617; FiringTracker.cpp:62,104,148.
use super::*;

fn assert_one_owned_cue(world: &mut GameLogic, source: ObjectId, frame: u32) {
    let object = &world.objects[&source];
    assert_eq!(object.weapon.as_ref().unwrap().ammo, Some(7));
    let marker = object.weapon_discharge_marker();
    assert_eq!(marker.sequence, 1);
    assert_eq!(marker.weapon_slot, 0);
    assert_eq!(marker.logic_frame, frame);
    let cues = world.take_weapon_discharges_for_presentation();
    assert_eq!(cues.len(), 1);
    assert_eq!(cues[0].source, source);
    assert_eq!(cues[0].sequence, marker.sequence);
    assert_eq!(cues[0].weapon_slot, marker.weapon_slot);
    assert_eq!(cues[0].fired_barrel, marker.fired_barrel);
    assert_eq!(cues[0].logic_frame, frame);
    assert!(world.take_weapon_discharges_for_presentation().is_empty());
}

fn materialize_each_world_once(
    first: &mut GameLogic,
    second: &mut GameLogic,
    target: ObjectId,
    other_target: ObjectId,
) {
    assert_eq!(first.objects[&target].health.current, 100.0);
    assert_eq!(second.objects[&other_target].health.current, 100.0);
    first.drain_pending_projectiles_into_combat();
    assert_eq!(first.objects[&target].health.current, 83.0);
    assert_eq!(second.objects[&other_target].health.current, 100.0);
    second.drain_pending_projectiles_into_combat();
    assert_eq!(second.objects[&other_target].health.current, 87.0);
    assert_eq!(first.objects[&target].health.current, 83.0);
    assert_eq!(first.combat_system.projectile_count(), 0);
    assert_eq!(second.combat_system.projectile_count(), 0);
    first.drain_pending_projectiles_into_combat();
    second.drain_pending_projectiles_into_combat();
    assert_eq!(first.objects[&target].health.current, 83.0);
    assert_eq!(second.objects[&other_target].health.current, 87.0);
}

#[test]
fn accepted_fire_frame_foreign_drain_cannot_stamp_other_worlds_normal_shot() {
    let _serial = combat_test_guard();
    let _restore = RestoreInputs::install();
    let (mut first, source, target) = world(FIRST_WEAPON);
    let (mut second, other_source, other_target) = world(SECOND_WEAPON);
    assert_eq!((source, target), (other_source, other_target));
    assert_eq!(first.get_current_frame(), 100);
    second.set_current_frame(300);

    // Ordinary production materialization publishes the second world's frame;
    // no test writes the compatibility clock to manufacture the defect.
    second.drain_pending_projectiles_into_combat();
    accept(&mut first, source, target, 10.0);
    accept(&mut second, other_source, other_target, 10.0);
    assert_one_owned_cue(&mut first, source, 100);
    assert_one_owned_cue(&mut second, other_source, 300);
    materialize_each_world_once(&mut first, &mut second, target, other_target);
    assert_eq!(first.objects[&source].last_fire_sim_time, 10.0);
    assert_eq!(second.objects[&other_source].last_fire_sim_time, 10.0);
    assert_eq!(second.objects[&other_source].last_fire_frame, 300);
    // All real acceptance, ammo, cue and damage controls precede the OLD
    // witness: current Object acceptance incorrectly stamps foreign frame 300.
    assert_eq!(
        first.objects[&source].last_fire_frame, 100,
        "normal accepted fire must use its driving world, after a foreign ordinary drain"
    );
}

#[test]
fn accepted_fire_frame_own_drain_control_keeps_timestamps_ammo_and_damage_order() {
    let _serial = combat_test_guard();
    let _restore = RestoreInputs::install();
    let (mut first, source, target) = world(FIRST_WEAPON);
    let (mut second, other_source, other_target) = world(SECOND_WEAPON);
    assert_eq!((source, target), (other_source, other_target));
    second.set_current_frame(300);

    first.drain_pending_projectiles_into_combat();
    accept(&mut first, source, target, 10.0);
    // First's accepted shot remains owned while second publishes its frame.
    second.drain_pending_projectiles_into_combat();
    accept(&mut second, other_source, other_target, 10.0);
    assert_one_owned_cue(&mut first, source, 100);
    assert_one_owned_cue(&mut second, other_source, 300);
    materialize_each_world_once(&mut first, &mut second, target, other_target);
    assert_eq!(first.objects[&source].last_fire_frame, 100);
    assert_eq!(second.objects[&other_source].last_fire_frame, 300);
    // Same f32 time deliberately accompanies two distinct actual frames.
    assert_eq!(first.objects[&source].last_fire_sim_time, 10.0);
    assert_eq!(second.objects[&other_source].last_fire_sim_time, 10.0);
}
