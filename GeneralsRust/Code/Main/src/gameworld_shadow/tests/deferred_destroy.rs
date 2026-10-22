//! Deferred-destroy lockstep + Phase 0 probe extensions.

use super::*;

#[test]
fn destroy_marks_then_process_removes_in_lockstep() {
    let mut logic = GameLogic::new();
    let cfg = golden_skirmish_config("DefDest");
    apply_skirmish_config(&mut logic, &cfg).expect("cfg");
    ensure_template(&mut logic, "Victim", 80.0);
    let id = logic
        .create_object("Victim", Team::USA, Vec3::new(20.0, 0.0, 20.0))
        .expect("id");

    let mut shadow = GameWorldShadow::new(64);
    shadow.sync_from_host(&logic);
    let eid = *shadow.host_to_entity.get(&id.0).expect("map");
    logic.destroy_object(id);
    assert!(logic.host_object(id).unwrap().status.destroyed);
    shadow.sync_from_host(&logic);
    let ent = shadow.world().entity(eid).expect("marked-visible");
    assert!(ent.destroyed);
    assert!(!ent.is_eligible_for_targeting());
    assert!(logic.has_pending_destroy_work());

    let probe = shadow.probe(&mut logic);
    assert!(probe.destroy_visibility_match, "{}", probe.format_report());

    logic.process_destroy_list();
    shadow.sync_from_host(&logic);
    assert!(shadow.world().entity(eid).is_none());
    assert!(shadow.entity_for_host(id).is_none());
}

#[test]
fn host_to_entity_invalidates_on_deferred_destroy_events() {
    let mut logic = GameLogic::new();
    let cfg = golden_skirmish_config("MapInv");
    apply_skirmish_config(&mut logic, &cfg).expect("cfg");
    ensure_template(&mut logic, "Victim", 80.0);
    let id = logic
        .create_object("Victim", Team::USA, Vec3::new(20.0, 0.0, 20.0))
        .expect("id");

    let mut shadow = GameWorldShadow::new(64);
    shadow.sync_from_host(&logic);
    let eid = shadow.entity_for_host(id).expect("mapped");
    logic.destroy_object(id);
    logic.process_destroy_list();
    shadow.sync_from_host(&logic);
    assert!(shadow.entity_for_host(id).is_none());
    assert!(shadow.host_for_entity(eid).is_none());
}

#[test]
fn phase0_probe_pose_target_weapon_contain_fields() {
    let mut logic = GameLogic::new();
    let cfg = golden_skirmish_config("ProbeExt");
    apply_skirmish_config(&mut logic, &cfg).expect("cfg");
    ensure_template(&mut logic, "Scout", 50.0);
    ensure_template(&mut logic, "Mark", 50.0);
    let a = logic
        .create_object("Scout", Team::USA, Vec3::new(10.0, 0.0, 10.0))
        .expect("a");
    let b = logic
        .create_object("Mark", Team::China, Vec3::new(40.0, 0.0, 10.0))
        .expect("b");
    {
        let (o, attack_events) = logic.host_object_and_attack_events_mut(a).expect("o");
        o.attack_target(
            b,
            attack_events,
            &mut crate::game_logic::ObjectAudio::queue(&mut Vec::new()),
        );
        o.movement.target_position = Some(Vec3::new(12.0, 0.0, 11.0));
        if let Some(w) = o.weapon.as_mut() {
            w.clip_size = 5;
            w.ammo = Some(3);
        }
    }

    let mut shadow = GameWorldShadow::new(64);
    shadow.sync_from_host(&logic);
    let probe = shadow.probe(&mut logic);
    assert!(probe.pose_match, "{}", probe.format_report());
    assert!(probe.attack_target_match, "{}", probe.format_report());
    assert!(probe.move_target_match, "{}", probe.format_report());
    assert!(probe.weapon_match, "{}", probe.format_report());
    assert!(probe.contain_match, "{}", probe.format_report());
    assert!(probe.destroy_visibility_match, "{}", probe.format_report());
}
