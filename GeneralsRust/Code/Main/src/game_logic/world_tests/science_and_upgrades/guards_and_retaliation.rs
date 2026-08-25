//! Behavior suite extracted from `science_and_upgrades`.
use super::*;

#[test]
fn enter_guard_does_not_shoot_enemies() {
    use crate::game_logic::{AIState, KindOf, Object, ObjectId, Team, ThingTemplate, Weapon};
    let mut logic = GameLogic::new();
    let mut ht = ThingTemplate::new("Terrorist");
    ht.add_kind_of(KindOf::Infantry);
    ht.add_kind_of(KindOf::Attackable);
    ht.enter_guard = true;
    let hid = ObjectId(4710);
    let mut h = Object::new(ht, hid, Team::GLA);
    h.set_position(glam::Vec3::ZERO);
    h.guard_position = Some(glam::Vec3::ZERO);
    h.vision_range = 150.0;
    h.weapon = Some(Weapon {
        range: 80.0,
        ..Default::default()
    });
    h.set_ai_state(AIState::GuardingArea);
    logic.objects.insert(hid, h);

    let mut et = ThingTemplate::new("Ranger");
    et.add_kind_of(KindOf::Infantry);
    et.add_kind_of(KindOf::Attackable);
    let eid = ObjectId(4711);
    let mut e = Object::new(et, eid, Team::USA);
    e.set_position(glam::Vec3::new(40.0, 0.0, 0.0));
    logic.objects.insert(eid, e);

    logic.update_support_states(&[hid, eid], 1.0 / 30.0);
    let h = &logic.objects[&hid];
    assert!(
        h.target.is_none(),
        "EnterGuard must not shoot; got target {:?}",
        h.target
    );
    assert_eq!(h.ai_state, AIState::GuardingArea);
}

#[test]
fn hijack_guard_boards_enemy_vehicle() {
    use crate::game_logic::{
        AIState, KindOf, Object, ObjectId, PendingSpecialAbility, Team, ThingTemplate, Weapon,
    };
    let mut logic = GameLogic::new();
    let mut ht = ThingTemplate::new("Hijacker");
    ht.add_kind_of(KindOf::Infantry);
    ht.add_kind_of(KindOf::Attackable);
    ht.enter_guard = true;
    ht.hijack_guard = true;
    let hid = ObjectId(4720);
    let mut h = Object::new(ht, hid, Team::GLA);
    h.set_position(glam::Vec3::ZERO);
    h.guard_position = Some(glam::Vec3::ZERO);
    h.vision_range = 150.0;
    h.weapon = Some(Weapon {
        range: 20.0,
        ..Default::default()
    });
    h.set_ai_state(AIState::GuardingArea);
    logic.objects.insert(hid, h);

    let mut vt = ThingTemplate::new("Humvee");
    vt.add_kind_of(KindOf::Vehicle);
    vt.add_kind_of(KindOf::Attackable);
    let vid = ObjectId(4721);
    let mut v = Object::new(vt, vid, Team::USA);
    v.set_position(glam::Vec3::new(30.0, 0.0, 0.0));
    logic.objects.insert(vid, v);

    mark_guard_scan_due(&mut logic, hid);
    logic.update_support_states(&[hid, vid], 1.0 / 30.0);
    let h = &logic.objects[&hid];
    assert_eq!(h.target, Some(vid), "HijackGuard must pick the vehicle");
    assert_eq!(h.ai_state, AIState::SpecialAbility);
    match logic.pending_special_abilities.get(&hid) {
        Some(PendingSpecialAbility::Hijack { target_id }) => {
            assert_eq!(*target_id, vid);
        }
        other => panic!("expected Hijack, got {other:?}"),
    }
}

#[test]
fn hijack_guard_inner_scan_skips_aircraft_and_drone() {
    // C++ ActionManager::canHijackVehicle: not KINDOF_AIRCRAFT, not KINDOF_DRONE.
    // Closest Comanche/drone must lose to a farther legal tank.
    use crate::game_logic::{KindOf, Object, ObjectId, Team, ThingTemplate};
    let mut logic = GameLogic::new();
    let mut ht = ThingTemplate::new("Hijacker");
    ht.add_kind_of(KindOf::Infantry);
    ht.add_kind_of(KindOf::Attackable);
    ht.enter_guard = true;
    ht.hijack_guard = true;
    let hid = ObjectId(4722);
    let mut h = Object::new(ht, hid, Team::GLA);
    h.set_position(glam::Vec3::ZERO);
    logic.objects.insert(hid, h);

    let mut at = ThingTemplate::new("Comanche");
    at.add_kind_of(KindOf::Vehicle);
    at.add_kind_of(KindOf::Aircraft);
    at.add_kind_of(KindOf::Attackable);
    let aid = ObjectId(4723);
    let mut a = Object::new(at, aid, Team::USA);
    a.set_position(glam::Vec3::new(15.0, 0.0, 0.0));
    logic.objects.insert(aid, a);

    let mut dt = ThingTemplate::new("BattleDrone");
    dt.add_kind_of(KindOf::Vehicle);
    dt.add_kind_of(KindOf::Drone);
    dt.add_kind_of(KindOf::Attackable);
    let did = ObjectId(4724);
    let mut d = Object::new(dt, did, Team::USA);
    d.set_position(glam::Vec3::new(20.0, 0.0, 0.0));
    logic.objects.insert(did, d);

    let mut vt = ThingTemplate::new("Humvee");
    vt.add_kind_of(KindOf::Vehicle);
    vt.add_kind_of(KindOf::Attackable);
    let vid = ObjectId(4725);
    let mut v = Object::new(vt, vid, Team::USA);
    v.set_position(glam::Vec3::new(40.0, 0.0, 0.0));
    logic.objects.insert(vid, v);

    let found = logic.scan_guard_inner_target_for_test(
        hid,
        Team::GLA,
        glam::Vec3::ZERO,
        200.0,
        false,
        true,
        true,
        None,
    );
    assert_eq!(
        found,
        Some(vid),
        "HijackGuard must skip aircraft/drone and pick the legal tank"
    );
}

#[test]
fn sleep_guard_range_is_zero_not_hardcoded_80() {
    use crate::game_logic::{AIState, KindOf, Object, ObjectId, Team, ThingTemplate, Weapon};
    let mut logic = GameLogic::new();
    let mut gt = ThingTemplate::new("Sleeper");
    gt.add_kind_of(KindOf::Infantry);
    gt.add_kind_of(KindOf::Attackable);
    let gid = ObjectId(4730);
    let mut g = Object::new(gt, gid, Team::China);
    g.set_position(glam::Vec3::ZERO);
    g.guard_position = Some(glam::Vec3::ZERO);
    g.guard_radius = 80.0;
    g.vision_range = 100.0;
    g.ai_attitude = -2;
    g.weapon = Some(Weapon {
        range: 80.0,
        ..Default::default()
    });
    g.set_ai_state(AIState::GuardingArea);
    logic.objects.insert(gid, g);

    assert_eq!(logic.host_std_guard_ranges(gid), (0.0, 0.0));

    let mut et = ThingTemplate::new("Intruder");
    et.add_kind_of(KindOf::Infantry);
    et.add_kind_of(KindOf::Attackable);
    let eid = ObjectId(4731);
    let mut e = Object::new(et, eid, Team::USA);
    e.set_position(glam::Vec3::new(40.0, 0.0, 0.0));
    logic.objects.insert(eid, e);

    logic.update_support_states(&[gid, eid], 1.0 / 30.0);
    let g = &logic.objects[&gid];
    assert!(
        g.target.is_none(),
        "Sleep guard must not acquire inside leftover 80 bubble"
    );
}

#[test]
fn aggressive_guard_range_is_mood_widened() {
    use crate::game_logic::{KindOf, Object, ObjectId, Team, ThingTemplate};
    let mut logic = GameLogic::new();
    let mut nt = ThingTemplate::new("NormalG");
    nt.add_kind_of(KindOf::Infantry);
    let nid = ObjectId(4740);
    let mut n = Object::new(nt, nid, Team::China);
    n.vision_range = 100.0;
    n.ai_attitude = 0;
    logic.objects.insert(nid, n);

    let mut at = ThingTemplate::new("AggroG");
    at.add_kind_of(KindOf::Infantry);
    let aid = ObjectId(4741);
    let mut a = Object::new(at, aid, Team::China);
    a.vision_range = 100.0;
    a.ai_attitude = 2;
    logic.objects.insert(aid, a);

    let (n_in, n_out) = logic.host_std_guard_ranges(nid);
    let (a_in, a_out) = logic.host_std_guard_ranges(aid);
    assert!(
        n_in > 0.0 && n_out > n_in,
        "normal inner/outer {n_in}/{n_out}"
    );
    assert!(
        a_in > n_in && a_out > n_out,
        "aggressive {a_in}/{a_out} must exceed normal {n_in}/{n_out}"
    );
}

#[test]
fn notify_computer_killer_only() {
    use crate::game_logic::{KindOf, Object, ObjectId, Team, ThingTemplate};
    let mut logic = GameLogic::new();
    logic
        .players
        .insert(0, Player::new(0, Team::USA, "Human", true));
    logic
        .players
        .insert(1, Player::new(1, Team::China, "AI", false));

    let mut ht = ThingTemplate::new("Hum");
    ht.add_kind_of(KindOf::Infantry);
    let hid = ObjectId(4620);
    logic.objects.insert(hid, Object::new(ht, hid, Team::USA));

    let mut at = ThingTemplate::new("AiK");
    at.add_kind_of(KindOf::Infantry);
    let aid = ObjectId(4621);
    logic.objects.insert(aid, Object::new(at, aid, Team::China));

    let cid = ObjectId(4622);
    assert!(!logic.notify_computer_killer_of_crate(hid, cid));
    assert!(logic.objects[&hid].crate_created.is_none());
    assert!(logic.notify_computer_killer_of_crate(aid, cid));
    assert_eq!(logic.objects[&aid].crate_created, Some(cid));
}

#[test]
fn begin_guard_retaliate_sets_state_and_anchor() {
    use crate::game_logic::{AIState, KindOf, Object, ObjectId, Team, ThingTemplate, Weapon};
    let mut logic = GameLogic::new();
    let mut t = ThingTemplate::new("GR");
    t.add_kind_of(KindOf::Infantry);
    t.add_kind_of(KindOf::Attackable);
    let id = ObjectId(4501);
    let mut o = Object::new(t, id, Team::USA);
    o.set_position(glam::Vec3::new(10.0, 0.0, 20.0));
    o.weapon = Some(Weapon {
        range: 50.0,
        ..Default::default()
    });
    logic.objects.insert(id, o);
    let victim = ObjectId(4502);
    logic.objects.get_mut(&id).unwrap().begin_guard_retaliate(
        victim,
        Some(glam::Vec3::new(10.0, 0.0, 20.0)),
        Some(5),
    );
    let o = &logic.objects[&id];
    assert_eq!(o.ai_state, AIState::GuardRetaliating);
    assert_eq!(o.guard_retaliate_victim, Some(victim));
    assert_eq!(o.target, Some(victim));
    assert_eq!(o.max_shots_to_fire, 5);
}

#[test]
fn guard_retaliate_ends_when_victim_dead() {
    use crate::game_logic::{AIState, KindOf, Object, ObjectId, Team, ThingTemplate, Weapon};
    let mut logic = GameLogic::new();
    let mut t = ThingTemplate::new("GR2");
    t.add_kind_of(KindOf::Infantry);
    let id = ObjectId(4510);
    let mut o = Object::new(t, id, Team::USA);
    o.set_position(glam::Vec3::ZERO);
    o.weapon = Some(Weapon {
        range: 40.0,
        ..Default::default()
    });
    logic.objects.insert(id, o);
    let vid = ObjectId(4511);
    let mut et = ThingTemplate::new("EV");
    et.add_kind_of(KindOf::Infantry);
    logic.objects.insert(vid, {
        let mut e = Object::new(et, vid, Team::GLA);
        e.set_position(glam::Vec3::new(15.0, 0.0, 0.0));
        e
    });
    logic
        .objects
        .get_mut(&id)
        .unwrap()
        .begin_guard_retaliate(vid, Some(glam::Vec3::ZERO), None);
    // Kill victim
    if let Some(e) = logic.objects.get_mut(&vid) {
        e.status.destroyed = true;
        e.health.current = 0.0;
    }
    logic.tick_guard_retaliate_states();
    let o = &logic.objects[&id];
    // Victim dead near anchor → end_guard_retaliate → GuardingArea (anchor preserved)
    assert!(
        matches!(
            o.ai_state,
            AIState::GuardingArea | AIState::Idle | AIState::Moving
        ),
        "got {:?}",
        o.ai_state
    );
    assert!(o.guard_retaliate_victim.is_none() || matches!(o.ai_state, AIState::Moving));
}

#[test]
fn friends_retaliate_against_nearby_aggressor() {
    use crate::game_logic::{AIState, KindOf, Object, ObjectId, Team, ThingTemplate, Weapon};
    let mut logic = GameLogic::new();
    // Human local player USA
    logic
        .players
        .insert(0, Player::new(0, Team::USA, "P0", true));
    logic.set_logical_retaliation_mode(0, true);

    // Victim
    let mut vt = ThingTemplate::new("Vic");
    vt.add_kind_of(KindOf::Infantry);
    vt.add_kind_of(KindOf::Attackable);
    let vid = ObjectId(4401);
    let mut victim = Object::new(vt, vid, Team::USA);
    victim.set_position(glam::Vec3::new(0.0, 0.0, 0.0));
    victim.health.current = 100.0;
    victim.health.maximum = 100.0;
    logic.objects.insert(vid, victim);

    // Friend idle nearby
    let mut ft = ThingTemplate::new("Friend");
    ft.add_kind_of(KindOf::Infantry);
    ft.add_kind_of(KindOf::Attackable);
    let fid = ObjectId(4402);
    let mut friend = Object::new(ft, fid, Team::USA);
    friend.set_position(glam::Vec3::new(30.0, 0.0, 0.0));
    friend.set_ai_state(AIState::Idle);
    friend.weapon = Some(Weapon {
        range: 100.0,
        damage: 10.0,
        ..Default::default()
    });
    logic.objects.insert(fid, friend);

    // Enemy damager within max retaliate distance
    let mut et = ThingTemplate::new("Aggr");
    et.add_kind_of(KindOf::Infantry);
    et.add_kind_of(KindOf::Attackable);
    let eid = ObjectId(4403);
    let mut enemy = Object::new(et, eid, Team::GLA);
    enemy.set_position(glam::Vec3::new(50.0, 0.0, 0.0));
    enemy.health.current = 100.0;
    enemy.health.maximum = 100.0;
    logic.objects.insert(eid, enemy);

    let n = logic.try_friends_retaliate(vid, eid);
    assert!(n >= 1, "friend should retaliate, got {n}");
    let f = &logic.objects[&fid];
    assert_eq!(f.target, Some(eid));
    assert_eq!(f.ai_state, AIState::GuardRetaliating);
    assert_eq!(f.guard_retaliate_victim, Some(eid));
    assert!(f.guard_retaliate_anchor.is_some());
}

#[test]
fn friends_retaliate_skipped_when_mode_off() {
    use crate::game_logic::{AIState, KindOf, Object, ObjectId, Team, ThingTemplate, Weapon};
    let mut logic = GameLogic::new();
    logic
        .players
        .insert(0, Player::new(0, Team::USA, "P0", true));
    // mode off
    let mut vt = ThingTemplate::new("Vic2");
    vt.add_kind_of(KindOf::Infantry);
    let vid = ObjectId(4411);
    logic.objects.insert(vid, {
        let mut o = Object::new(vt, vid, Team::USA);
        o.set_position(glam::Vec3::ZERO);
        o
    });
    let mut ft = ThingTemplate::new("Fr2");
    ft.add_kind_of(KindOf::Infantry);
    ft.add_kind_of(KindOf::Attackable);
    let fid = ObjectId(4412);
    logic.objects.insert(fid, {
        let mut o = Object::new(ft, fid, Team::USA);
        o.set_position(glam::Vec3::new(20.0, 0.0, 0.0));
        o.set_ai_state(AIState::Idle);
        o.weapon = Some(Weapon {
            range: 80.0,
            ..Default::default()
        });
        o
    });
    let mut et = ThingTemplate::new("En2");
    et.add_kind_of(KindOf::Infantry);
    let eid = ObjectId(4413);
    logic.objects.insert(eid, {
        let mut o = Object::new(et, eid, Team::China);
        o.set_position(glam::Vec3::new(40.0, 0.0, 0.0));
        o
    });
    assert_eq!(logic.try_friends_retaliate(vid, eid), 0);
    assert!(logic.objects[&fid].target.is_none());
}

#[test]
fn guard_idle_acquire_uses_inner_not_outer() {
    use crate::game_logic::{AIState, KindOf, Object, ObjectId, Team, ThingTemplate};
    let mut logic = GameLogic::new();
    let mut gt = ThingTemplate::new("InnerGuard");
    gt.add_kind_of(KindOf::Infantry);
    gt.add_kind_of(KindOf::Attackable);
    let gid = ObjectId(6101);
    let mut g = Object::new(gt, gid, Team::China);
    g.set_position(glam::Vec3::ZERO);
    g.guard_position = Some(glam::Vec3::ZERO);
    g.vision_range = 100.0;
    g.weapon = Some(wave21_guard_weapon());
    g.set_ai_state(AIState::GuardingArea);
    logic.objects.insert(gid, g);

    let (inner, outer) = logic.host_std_guard_ranges(gid);
    assert!(outer > inner && inner > 0.0, "inner={inner} outer={outer}");
    let mid = (inner + outer) * 0.5;

    let mut et = ThingTemplate::new("OuterRing");
    et.add_kind_of(KindOf::Infantry);
    et.add_kind_of(KindOf::Attackable);
    let eid = ObjectId(6102);
    let mut e = Object::new(et, eid, Team::USA);
    e.set_position(glam::Vec3::new(mid, 0.0, 0.0));
    logic.objects.insert(eid, e);

    logic.update_support_states(&[gid, eid], 1.0 / 30.0);
    assert!(
        logic.objects[&gid].target.is_none(),
        "Normal guard must not acquire between inner and outer"
    );
}

#[test]
fn guarding_object_flying_only_skips_ground() {
    use crate::game_logic::{AIState, GuardMode, KindOf, Object, ObjectId, Team, ThingTemplate};
    let mut logic = GameLogic::new();
    let mut ht = ThingTemplate::new("AAGuard");
    ht.add_kind_of(KindOf::Infantry);
    ht.add_kind_of(KindOf::Attackable);
    let hid = ObjectId(6110);
    let mut h = Object::new(ht, hid, Team::USA);
    h.set_position(glam::Vec3::ZERO);
    h.vision_range = 200.0;
    h.weapon = Some(wave21_guard_weapon());
    h.guard_mode = GuardMode::FlyingUnitsOnly;
    h.set_ai_state(AIState::GuardingObject);
    logic.objects.insert(hid, h);

    let mut bt = ThingTemplate::new("Convoy");
    bt.add_kind_of(KindOf::Vehicle);
    bt.add_kind_of(KindOf::Attackable);
    let bid = ObjectId(6111);
    let mut b = Object::new(bt, bid, Team::USA);
    b.set_position(glam::Vec3::ZERO);
    logic.objects.insert(bid, b);
    logic.objects.get_mut(&hid).unwrap().guard_target = Some(bid);

    let mut gt = ThingTemplate::new("Tank");
    gt.add_kind_of(KindOf::Vehicle);
    gt.add_kind_of(KindOf::Attackable);
    let tid = ObjectId(6112);
    let mut tank = Object::new(gt, tid, Team::GLA);
    tank.set_position(glam::Vec3::new(30.0, 0.0, 0.0));
    logic.objects.insert(tid, tank);

    let mut at = ThingTemplate::new("Raptor");
    at.add_kind_of(KindOf::Aircraft);
    at.add_kind_of(KindOf::Attackable);
    let aid = ObjectId(6113);
    let mut air = Object::new(at, aid, Team::GLA);
    air.set_position(glam::Vec3::new(80.0, 20.0, 0.0));
    air.status.airborne_target = true;
    logic.objects.insert(aid, air);

    mark_guard_scan_due(&mut logic, hid);
    logic.update_support_states(&[hid, bid, tid, aid], 1.0 / 30.0);
    assert_eq!(
        logic.objects[&hid].target,
        Some(aid),
        "FlyingUnitsOnly object guard must ignore the closer tank"
    );
}

#[test]
fn guard_area_polygon_rejects_outside_and_covers_far_corner() {
    use crate::game_logic::{AIState, KindOf, Object, ObjectId, Team, ThingTemplate};
    use gamelogic::common::{AsciiString, ICoord3D};
    use gamelogic::polygon_trigger::PolygonTrigger;

    let trigger = PolygonTrigger::new(
        6120,
        AsciiString::from("Wave21GuardAreaPoly"),
        vec![
            ICoord3D::new(0, 0, 0),
            ICoord3D::new(400, 0, 0),
            ICoord3D::new(400, 40, 0),
            ICoord3D::new(0, 40, 0),
        ],
    );
    gamelogic::terrain::get_terrain_logic()
        .write()
        .expect("terrain")
        .add_trigger_area(trigger);

    let mut logic = GameLogic::new();
    let mut gt = ThingTemplate::new("PolyGuard");
    gt.add_kind_of(KindOf::Infantry);
    gt.add_kind_of(KindOf::Attackable);
    let gid = ObjectId(6121);
    let mut g = Object::new(gt, gid, Team::China);
    g.set_position(glam::Vec3::new(200.0, 0.0, 20.0));
    g.guard_position = Some(glam::Vec3::new(200.0, 0.0, 20.0));
    g.guard_area_trigger = Some("Wave21GuardAreaPoly".into());
    g.vision_range = 100.0;
    g.weapon = Some(wave21_guard_weapon());
    g.set_ai_state(AIState::GuardingArea);
    logic.objects.insert(gid, g);

    let mut ot = ThingTemplate::new("OutsidePoly");
    ot.add_kind_of(KindOf::Infantry);
    ot.add_kind_of(KindOf::Attackable);
    let oid = ObjectId(6122);
    let mut outside = Object::new(ot, oid, Team::USA);
    outside.set_position(glam::Vec3::new(200.0, 0.0, 150.0));
    logic.objects.insert(oid, outside);

    mark_guard_scan_due(&mut logic, gid);
    logic.update_support_states(&[gid, oid], 1.0 / 30.0);
    assert!(
        logic.objects[&gid].target.is_none(),
        "enemy outside the polygon must not be acquired"
    );

    let mut it = ThingTemplate::new("InsideCorner");
    it.add_kind_of(KindOf::Infantry);
    it.add_kind_of(KindOf::Attackable);
    let iid = ObjectId(6123);
    let mut inside = Object::new(it, iid, Team::USA);
    inside.set_position(glam::Vec3::new(380.0, 0.0, 20.0));
    logic.objects.insert(iid, inside);

    mark_guard_scan_due(&mut logic, gid);
    logic.update_support_states(&[gid, oid, iid], 1.0 / 30.0);
    assert_eq!(
        logic.objects[&gid].target,
        Some(iid),
        "far polygon corner must be covered"
    );
}

#[test]
fn guard_retaliate_chase_gives_up_on_timer() {
    use crate::game_logic::{
        AIState, GUARD_CHASE_PHASE_RETALIATE, KindOf, Object, ObjectId, Team, ThingTemplate,
    };
    let mut logic = GameLogic::new();
    let mut t = ThingTemplate::new("ChaseGiver");
    t.add_kind_of(KindOf::Infantry);
    t.add_kind_of(KindOf::Attackable);
    let id = ObjectId(6130);
    let mut o = Object::new(t, id, Team::USA);
    o.set_position(glam::Vec3::ZERO);
    o.vision_range = 100.0;
    o.weapon = Some(wave21_guard_weapon());
    logic.objects.insert(id, o);

    let vid = ObjectId(6131);
    let mut et = ThingTemplate::new("ChaseVictim");
    et.add_kind_of(KindOf::Infantry);
    et.add_kind_of(KindOf::Attackable);
    logic.objects.insert(vid, {
        let mut e = Object::new(et, vid, Team::GLA);
        e.set_position(glam::Vec3::new(20.0, 0.0, 0.0));
        e
    });

    logic
        .objects
        .get_mut(&id)
        .unwrap()
        .begin_guard_retaliate(vid, Some(glam::Vec3::ZERO), None);
    assert_eq!(
        logic.objects[&id].guard_chase_phase,
        GUARD_CHASE_PHASE_RETALIATE
    );

    logic.tick_guard_retaliate_states();
    let give = logic.objects[&id].guard_chase_give_up_frame;
    assert!(give > 0, "first tick must stamp give-up frame");
    assert_eq!(logic.objects[&id].guard_retaliate_victim, Some(vid));

    logic.frame = give;
    logic.tick_guard_retaliate_states();
    let o = &logic.objects[&id];
    assert!(
        o.guard_retaliate_victim.is_none(),
        "timer must drop the live victim"
    );
    assert!(
        matches!(
            o.ai_state,
            AIState::GuardingArea | AIState::Idle | AIState::Moving
        ),
        "got {:?}",
        o.ai_state
    );
}

#[test]
fn guard_retaliate_inner_scan_allows_base_defense() {
    use crate::game_logic::{AIState, KindOf, Object, ObjectId, Player, Team, ThingTemplate};
    let mut logic = GameLogic::new();
    logic
        .players
        .insert(0, Player::new(0, Team::USA, "Human", true));

    let mut t = ThingTemplate::new("Retaliator");
    t.add_kind_of(KindOf::Infantry);
    t.add_kind_of(KindOf::Attackable);
    let id = ObjectId(6140);
    let mut o = Object::new(t, id, Team::USA);
    o.set_position(glam::Vec3::ZERO);
    o.owner_player_id = Some(0);
    o.vision_range = 200.0;
    o.weapon = Some(wave21_guard_weapon());
    logic.objects.insert(id, o);

    let vid = ObjectId(6141);
    let mut vt = ThingTemplate::new("DeadAggr");
    vt.add_kind_of(KindOf::Infantry);
    logic.objects.insert(vid, {
        let mut e = Object::new(vt, vid, Team::GLA);
        e.set_position(glam::Vec3::new(10.0, 0.0, 0.0));
        e
    });

    let mut wt = ThingTemplate::new("Warehouse");
    wt.add_kind_of(KindOf::Structure);
    wt.add_kind_of(KindOf::Attackable);
    let wid = ObjectId(6142);
    let mut warehouse = Object::new(wt, wid, Team::GLA);
    warehouse.set_position(glam::Vec3::new(20.0, 0.0, 0.0));
    logic.objects.insert(wid, warehouse);

    let mut pt = ThingTemplate::new("Patriot");
    pt.add_kind_of(KindOf::Structure);
    pt.add_kind_of(KindOf::FSBaseDefense);
    pt.add_kind_of(KindOf::Attackable);
    let pid = ObjectId(6143);
    let mut patriot = Object::new(pt, pid, Team::GLA);
    patriot.set_position(glam::Vec3::new(40.0, 0.0, 0.0));
    logic.objects.insert(pid, patriot);

    logic
        .objects
        .get_mut(&id)
        .unwrap()
        .begin_guard_retaliate(vid, Some(glam::Vec3::ZERO), None);
    logic.objects.get_mut(&vid).unwrap().status.destroyed = true;
    logic.objects.get_mut(&vid).unwrap().health.current = 0.0;
    mark_guard_scan_due(&mut logic, id);
    logic.tick_guard_retaliate_states();
    assert_eq!(
        logic.objects[&id].guard_retaliate_victim,
        Some(pid),
        "human retaliate rescan must pick Patriot, not warehouse"
    );
}

#[test]
fn guard_retaliate_computer_scan_allows_any_enemy_structure() {
    use crate::game_logic::{KindOf, Object, ObjectId, Player, Team, ThingTemplate};
    let mut logic = GameLogic::new();
    logic
        .players
        .insert(1, Player::new(1, Team::China, "AI", false));

    let mut t = ThingTemplate::new("AiRet");
    t.add_kind_of(KindOf::Infantry);
    t.add_kind_of(KindOf::Attackable);
    let id = ObjectId(6150);
    let mut o = Object::new(t, id, Team::China);
    o.set_position(glam::Vec3::ZERO);
    o.owner_player_id = Some(1);
    o.vision_range = 200.0;
    o.weapon = Some(wave21_guard_weapon());
    logic.objects.insert(id, o);

    let vid = ObjectId(6151);
    let mut vt = ThingTemplate::new("Dead2");
    vt.add_kind_of(KindOf::Infantry);
    logic.objects.insert(vid, {
        let mut e = Object::new(vt, vid, Team::USA);
        e.set_position(glam::Vec3::new(8.0, 0.0, 0.0));
        e
    });

    let mut wt = ThingTemplate::new("Warfact");
    wt.add_kind_of(KindOf::Structure);
    wt.add_kind_of(KindOf::Attackable);
    let wid = ObjectId(6152);
    let mut warehouse = Object::new(wt, wid, Team::USA);
    warehouse.set_position(glam::Vec3::new(25.0, 0.0, 0.0));
    logic.objects.insert(wid, warehouse);

    logic
        .objects
        .get_mut(&id)
        .unwrap()
        .begin_guard_retaliate(vid, Some(glam::Vec3::ZERO), None);
    logic.objects.get_mut(&vid).unwrap().status.destroyed = true;
    logic.objects.get_mut(&vid).unwrap().health.current = 0.0;
    mark_guard_scan_due(&mut logic, id);
    logic.tick_guard_retaliate_states();
    assert_eq!(
        logic.objects[&id].guard_retaliate_victim,
        Some(wid),
        "computer retaliate rescan must acquire enemy structures"
    );
}

#[test]
fn host_guardee_follow_is_per_axis_two_cells_not_inner_radius() {
    use crate::game_logic::host_repair::PATHFIND_CELL_SIZE_F;
    use crate::game_logic::{AIState, KindOf, Object, ObjectId, Team, ThingTemplate};

    // C++ AIGuard.cpp:722-730 — 2.5 cells on X (25wu) is still well inside
    // inner vision (~80+). Pre-fix live only followed when farther than inner.

    let mut logic = GameLogic::new();
    let mut gt = ThingTemplate::new("RangerFollow");
    gt.add_kind_of(KindOf::Infantry);
    gt.add_kind_of(KindOf::Attackable);
    let gid = ObjectId(6201);
    let mut g = Object::new(gt, gid, Team::USA);
    g.set_position(glam::Vec3::ZERO);
    g.vision_range = 200.0;
    g.weapon = Some(wave21_guard_weapon());
    g.set_ai_state(AIState::GuardingObject);
    logic.objects.insert(gid, g);

    let mut ct = ThingTemplate::new("CrusaderFollow");
    ct.add_kind_of(KindOf::Vehicle);
    ct.add_kind_of(KindOf::Attackable);
    let cid = ObjectId(6202);
    let mut crusader = Object::new(ct, cid, Team::USA);
    crusader.set_position(glam::Vec3::ZERO);
    logic.objects.insert(cid, crusader);
    logic.objects.get_mut(&gid).unwrap().guard_target = Some(cid);

    logic.update_support_states(&[gid, cid], 1.0 / 30.0);
    assert!(
        logic.objects[&gid].movement.target_position.is_none(),
        "first idle tick only stamps m_guardeePos"
    );

    logic
        .objects
        .get_mut(&cid)
        .unwrap()
        .set_position(glam::Vec3::new(PATHFIND_CELL_SIZE_F * 2.5, 0.0, 0.0));
    logic.update_support_states(&[gid, cid], 1.0 / 30.0);
    let dest = logic.objects[&gid].movement.target_position;
    assert!(
        dest.is_some_and(|p| (p.x - PATHFIND_CELL_SIZE_F * 2.5).abs() < 1.0),
        "2.5-cell guardee walk must path even while still inside inner vision; dest={dest:?}"
    );
}

#[test]
fn inner_guard_attack_switches_to_new_last_attacker() {
    use crate::game_logic::{AIState, KindOf, Object, ObjectId, Team, ThingTemplate};
    const INNER: u8 = 1;
    const AGGRESSOR: u8 = 3;

    let mut logic = GameLogic::new();
    let mut gt = ThingTemplate::new("InnerSwitch");
    gt.add_kind_of(KindOf::Infantry);
    gt.add_kind_of(KindOf::Attackable);
    let gid = ObjectId(6210);
    let mut g = Object::new(gt, gid, Team::USA);
    g.set_position(glam::Vec3::ZERO);
    g.guard_position = Some(glam::Vec3::ZERO);
    g.guard_radius = 200.0;
    g.vision_range = 200.0;
    g.weapon = Some(wave21_guard_weapon());
    g.set_ai_state(AIState::Attacking);
    g.target = Some(ObjectId(6211));
    g.guard_chase_phase = INNER;
    g.status.attacking = true;
    logic.objects.insert(gid, g);

    let mut at = ThingTemplate::new("EnemyA");
    at.add_kind_of(KindOf::Infantry);
    at.add_kind_of(KindOf::Attackable);
    let aid = ObjectId(6211);
    let mut a = Object::new(at, aid, Team::GLA);
    a.set_position(glam::Vec3::new(20.0, 0.0, 0.0));
    logic.objects.insert(aid, a);

    let mut bt = ThingTemplate::new("EnemyB");
    bt.add_kind_of(KindOf::Infantry);
    bt.add_kind_of(KindOf::Attackable);
    let bid = ObjectId(6212);
    let mut b = Object::new(bt, bid, Team::GLA);
    b.set_position(glam::Vec3::new(25.0, 0.0, 0.0));
    logic.objects.insert(bid, b);

    logic.objects.get_mut(&gid).unwrap().last_damage_source = Some(bid);
    logic.update_support_states(&[gid, aid, bid], 1.0 / 30.0);
    let g = &logic.objects[&gid];
    assert_eq!(
        g.target,
        Some(bid),
        "INNER attack must switch to new last attacker"
    );
    assert_eq!(
        g.guard_chase_phase, AGGRESSOR,
        "switch must enter AttackAggressor, not stay INNER"
    );
}

#[test]
fn guard_idle_acquire_uses_scan_rate_cadence() {
    use crate::game_logic::{AIState, KindOf, Object, ObjectId, Team, ThingTemplate};

    let mut logic = GameLogic::new();
    let mut gt = ThingTemplate::new("ScanCadence");
    gt.add_kind_of(KindOf::Infantry);
    gt.add_kind_of(KindOf::Attackable);
    let gid = ObjectId(6220);
    let mut g = Object::new(gt, gid, Team::China);
    g.set_position(glam::Vec3::ZERO);
    g.guard_position = Some(glam::Vec3::ZERO);
    g.vision_range = 200.0;
    g.weapon = Some(wave21_guard_weapon());
    g.set_ai_state(AIState::GuardingArea);
    logic.objects.insert(gid, g);

    let mut et = ThingTemplate::new("ScanPrey");
    et.add_kind_of(KindOf::Infantry);
    et.add_kind_of(KindOf::Attackable);
    let eid = ObjectId(6221);
    let mut e = Object::new(et, eid, Team::USA);
    e.set_position(glam::Vec3::new(20.0, 0.0, 0.0));
    logic.objects.insert(eid, e);

    let rate = logic.host_guard_enemy_scan_rate().max(1);
    logic
        .guard_next_enemy_scan
        .insert(gid, logic.frame.saturating_add(rate));
    logic.update_support_states(&[gid, eid], 1.0 / 30.0);
    assert!(
        logic.objects[&gid].target.is_none(),
        "idle must not acquire before GuardEnemyScanRate"
    );

    logic.frame = logic.frame.saturating_add(rate);
    mark_guard_scan_due(&mut logic, gid);
    logic.update_support_states(&[gid, eid], 1.0 / 30.0);
    assert_eq!(
        logic.objects[&gid].target,
        Some(eid),
        "idle acquire must fire when scan time is due"
    );

    // Return to idle; same-frame rescan must wait the rate.
    {
        let g = logic.objects.get_mut(&gid).unwrap();
        g.target = None;
        g.status.attacking = false;
        g.clear_guard_chase();
        g.set_ai_state(AIState::GuardingArea);
        g.last_damage_source = None;
    }
    logic.update_support_states(&[gid, eid], 1.0 / 30.0);
    assert!(
        logic.objects[&gid].target.is_none(),
        "second idle look in the same frame must wait GuardEnemyScanRate"
    );
}

#[test]
fn end_guard_chase_attack_clears_team_attack_common_target() {
    // C++ AIGuardInnerState::onExit / AttackAggressor::onExit:
    // getTeam()->setTeamTargetObject(NULL) so the next lookForInnerTarget
    // re-scans instead of re-pulling the same kite.
    use crate::game_logic::{AIState, KindOf, Object, ObjectId, Player, Team, ThingTemplate};
    const INNER: u8 = 1;

    let mut logic = GameLogic::new();
    logic.add_player(Player::new(1, Team::China, "China AI", false));

    let mut gt = ThingTemplate::new("W26ChaseClear");
    gt.add_kind_of(KindOf::Infantry);
    gt.add_kind_of(KindOf::Attackable);
    let gid = ObjectId(6301);
    let mut g = Object::new(gt, gid, Team::China);
    g.set_position(glam::Vec3::ZERO);
    g.guard_position = Some(glam::Vec3::ZERO);
    g.vision_range = 200.0;
    g.weapon = Some(wave21_guard_weapon());
    g.set_ai_state(AIState::Attacking);
    g.guard_chase_phase = INNER;
    g.status.attacking = true;
    g.owner_player_id = Some(1);
    g.team_instance_name = "China_GuardSquad".into();
    logic.objects.insert(gid, g);

    let mut et = ThingTemplate::new("W26Kite");
    et.add_kind_of(KindOf::Infantry);
    et.add_kind_of(KindOf::Attackable);
    let eid = ObjectId(6302);
    let mut e = Object::new(et, eid, Team::USA);
    e.set_position(glam::Vec3::new(20.0, 0.0, 0.0));
    logic.objects.insert(eid, e);

    logic.objects.get_mut(&gid).unwrap().target = Some(eid);
    logic
        .team_common_attack_targets
        .insert("China_GuardSquad".into(), eid);

    // Victim left the inner ring: chase-exit must drop the shared victim.
    logic
        .objects
        .get_mut(&eid)
        .unwrap()
        .set_position(glam::Vec3::new(400.0, 0.0, 0.0));
    logic.update_support_states(&[gid, eid], 1.0 / 30.0);
    assert!(
        !logic
            .team_common_attack_targets
            .contains_key("China_GuardSquad"),
        "inner/aggressor exit must setTeamTargetObject(NULL); map={:?}",
        logic.team_common_attack_targets
    );
}

#[test]
fn retaliate_chase_exit_clears_team_attack_common_target() {
    // C++ AIGuardRetaliateInner/Aggressor onExit: setTeamTargetObject(NULL).
    use crate::game_logic::{
        AIState, GUARD_CHASE_PHASE_RETALIATE, KindOf, Object, ObjectId, Player, Team, ThingTemplate,
    };

    let mut logic = GameLogic::new();
    logic.add_player(Player::new(1, Team::USA, "USA AI", false));

    let mut gt = ThingTemplate::new("W26RetClear");
    gt.add_kind_of(KindOf::Infantry);
    gt.add_kind_of(KindOf::Attackable);
    let gid = ObjectId(6310);
    let mut g = Object::new(gt, gid, Team::USA);
    g.set_position(glam::Vec3::ZERO);
    g.vision_range = 200.0;
    g.weapon = Some(wave21_guard_weapon());
    g.owner_player_id = Some(1);
    g.team_instance_name = "USA_RetSquad".into();
    logic.objects.insert(gid, g);

    let mut et = ThingTemplate::new("W26RetKite");
    et.add_kind_of(KindOf::Infantry);
    et.add_kind_of(KindOf::Attackable);
    let eid = ObjectId(6311);
    let mut e = Object::new(et, eid, Team::GLA);
    e.set_position(glam::Vec3::new(20.0, 0.0, 0.0));
    logic.objects.insert(eid, e);

    {
        let o = logic.objects.get_mut(&gid).unwrap();
        o.begin_guard_retaliate(eid, Some(glam::Vec3::ZERO), None);
        o.guard_chase_phase = GUARD_CHASE_PHASE_RETALIATE;
        o.guard_chase_give_up_frame = 1;
        o.target = Some(eid);
    }
    logic.frame = 10;
    logic
        .team_common_attack_targets
        .insert("USA_RetSquad".into(), eid);

    logic.tick_guard_retaliate_states();
    assert!(
        !logic
            .team_common_attack_targets
            .contains_key("USA_RetSquad"),
        "retaliate chase-exit must setTeamTargetObject(NULL); map={:?}",
        logic.team_common_attack_targets
    );
}

#[test]
fn guarding_object_prefers_team_attack_common_target() {
    // C++ lookForInnerTarget returns getTeamTargetObject first for Guard Object
    // as well as Area. A squad guarding a dozer must focus-fire the shared victim.
    use crate::game_logic::{AIState, KindOf, Object, ObjectId, Player, Team, ThingTemplate};

    let mut logic = GameLogic::new();
    logic.add_player(Player::new(1, Team::USA, "USA AI", false));

    let mut gt = ThingTemplate::new("W26ObjGuard");
    gt.add_kind_of(KindOf::Infantry);
    gt.add_kind_of(KindOf::Attackable);
    let gid = ObjectId(6320);
    let mut g = Object::new(gt, gid, Team::USA);
    g.set_position(glam::Vec3::ZERO);
    g.vision_range = 200.0;
    g.weapon = Some(wave21_guard_weapon());
    g.set_ai_state(AIState::GuardingObject);
    g.owner_player_id = Some(1);
    g.team_instance_name = "USA_DozerGuard".into();
    logic.objects.insert(gid, g);

    let mut dt = ThingTemplate::new("W26Dozer");
    dt.add_kind_of(KindOf::Vehicle);
    dt.add_kind_of(KindOf::Dozer);
    dt.add_kind_of(KindOf::Attackable);
    let did = ObjectId(6321);
    let mut d = Object::new(dt, did, Team::USA);
    d.set_position(glam::Vec3::ZERO);
    logic.objects.insert(did, d);
    logic.objects.get_mut(&gid).unwrap().guard_target = Some(did);

    let mut close_t = ThingTemplate::new("W26CloseEnemy");
    close_t.add_kind_of(KindOf::Infantry);
    close_t.add_kind_of(KindOf::Attackable);
    let close_id = ObjectId(6322);
    let mut close = Object::new(close_t, close_id, Team::GLA);
    close.set_position(glam::Vec3::new(15.0, 0.0, 0.0));
    logic.objects.insert(close_id, close);

    let mut team_t = ThingTemplate::new("W26TeamVictim");
    team_t.add_kind_of(KindOf::Infantry);
    team_t.add_kind_of(KindOf::Attackable);
    let team_id = ObjectId(6323);
    let mut team_v = Object::new(team_t, team_id, Team::GLA);
    team_v.set_position(glam::Vec3::new(50.0, 0.0, 0.0));
    logic.objects.insert(team_id, team_v);

    logic
        .team_common_attack_targets
        .insert("USA_DozerGuard".into(), team_id);
    mark_guard_scan_due(&mut logic, gid);
    logic.update_support_states(&[gid, did, close_id, team_id], 1.0 / 30.0);
    assert_eq!(
        logic.objects[&gid].target,
        Some(team_id),
        "GuardingObject must prefer team attackCommonTarget over closer scan"
    );
}

#[test]
fn enter_guard_inner_scan_requires_can_enter_object() {
    // C++ PartitionFilterPossibleToEnter + ALLOW_NEUTRAL: closest *enterable*
    // Neutral wins. A closer civilian/prop must not beat a garrison building.
    use crate::game_logic::{
        ContainAdmission, ContainModuleKind, ContainModuleMetadata, KindOf, Object, ObjectId, Team,
        ThingTemplate,
    };

    let mut logic = GameLogic::new();
    let mut ht = ThingTemplate::new("W26Terrorist");
    ht.add_kind_of(KindOf::Infantry);
    ht.add_kind_of(KindOf::Attackable);
    ht.enter_guard = true;
    ht.transport_slot_count = Some(1);
    let hid = ObjectId(6330);
    let mut h = Object::new(ht, hid, Team::GLA);
    h.set_position(glam::Vec3::ZERO);
    h.vision_range = 200.0;
    logic.objects.insert(hid, h);

    let mut civ_t = ThingTemplate::new("W26Civilian");
    civ_t.add_kind_of(KindOf::Infantry);
    let civ_id = ObjectId(6331);
    let mut civ = Object::new(civ_t, civ_id, Team::Neutral);
    civ.set_position(glam::Vec3::new(10.0, 0.0, 0.0));
    logic.objects.insert(civ_id, civ);

    let mut bunk_t = ThingTemplate::new("W26NeutralBunker");
    bunk_t.add_kind_of(KindOf::Structure);
    bunk_t.add_kind_of(KindOf::Attackable);
    bunk_t.contain_module = ContainModuleMetadata {
        kind: ContainModuleKind::Garrison,
        slots: Some(5),
        admission: ContainAdmission::InfantryOnly,
        allow_allies_inside: true,
        allow_enemies_inside: true,
        allow_neutral_inside: true,
        ..ContainModuleMetadata::default()
    };
    let bunk_id = ObjectId(6332);
    let mut bunk = Object::new(bunk_t, bunk_id, Team::Neutral);
    bunk.set_position(glam::Vec3::new(40.0, 0.0, 0.0));
    if let Some(bd) = bunk.building_data.as_mut() {
        bd.max_garrison = 5;
    } else {
        let mut bd = crate::game_logic::BuildingData::new(crate::game_logic::BuildingType::Bunker);
        bd.max_garrison = 5;
        bunk.building_data = Some(bd);
    }
    logic.objects.insert(bunk_id, bunk);

    assert!(
        logic.can_unit_enter_normal_target(hid, bunk_id),
        "bunker must be enterable"
    );
    assert!(
        !logic.can_unit_enter_normal_target(hid, civ_id),
        "civilian must not be enterable"
    );

    let found = logic.scan_guard_inner_target_for_test(
        hid,
        Team::GLA,
        glam::Vec3::ZERO,
        200.0,
        false,
        true,
        false,
        None,
    );
    assert_eq!(
        found,
        Some(bunk_id),
        "EnterGuard scan must pick closest enterable Neutral, not closer civilian"
    );
}

#[test]
fn area_guard_outer_chase_uses_polygon_radius() {
    use crate::game_logic::{AIState, KindOf, Object, ObjectId, Team, ThingTemplate};
    use gamelogic::common::{AsciiString, ICoord3D};
    use gamelogic::polygon_trigger::PolygonTrigger;

    let trigger = PolygonTrigger::new(
        6300,
        AsciiString::from("Wave26GuardOuterPoly"),
        vec![
            ICoord3D::new(0, 0, 0),
            ICoord3D::new(600, 0, 0),
            ICoord3D::new(600, 600, 0),
            ICoord3D::new(0, 600, 0),
        ],
    );
    let poly_r = trigger.get_radius();
    gamelogic::terrain::get_terrain_logic()
        .write()
        .expect("terrain")
        .add_trigger_area(trigger);

    let mut logic = GameLogic::new();
    let mut gt = ThingTemplate::new("PolyOuterGuard");
    gt.add_kind_of(KindOf::Infantry);
    gt.add_kind_of(KindOf::Attackable);
    let gid = ObjectId(6301);
    let mut g = Object::new(gt, gid, Team::China);
    let center = glam::Vec3::new(300.0, 0.0, 300.0);
    g.set_position(center);
    g.guard_position = Some(center);
    g.guard_area_trigger = Some("Wave26GuardOuterPoly".into());
    g.vision_range = 100.0;
    g.weapon = Some(wave21_guard_weapon());
    g.set_ai_state(AIState::Attacking);
    g.guard_chase_phase = 2; // OUTER
    g.guard_chase_give_up_frame = logic.frame.saturating_add(10_000);
    g.status.attacking = true;
    logic.objects.insert(gid, g);

    let (inner, outer) = logic.host_std_guard_ranges(gid);
    assert!(
        poly_r > outer && outer > 0.0,
        "polygon radius {poly_r} must exceed vision outer {outer}"
    );
    let mid = (outer + poly_r) * 0.5;
    assert!(mid > inner, "chase sample must sit outside inner {inner}");

    let mut et = ThingTemplate::new("PolyOuterPrey");
    et.add_kind_of(KindOf::Infantry);
    et.add_kind_of(KindOf::Attackable);
    let eid = ObjectId(6302);
    let mut e = Object::new(et, eid, Team::USA);
    e.set_position(glam::Vec3::new(center.x + mid, 0.0, center.z));
    logic.objects.insert(eid, e);
    logic.objects.get_mut(&gid).unwrap().target = Some(eid);

    logic.update_support_states(&[gid, eid], 1.0 / 30.0);
    assert_eq!(
        logic.objects[&gid].target,
        Some(eid),
        "area-guard outer must keep a victim inside polygon radius ({poly_r}) even past vision ({outer})"
    );
}

#[test]
fn guard_retaliate_scan_victim_uses_inner_1_5x_not_aggressor() {
    use crate::game_logic::{KindOf, Object, ObjectId, Team, ThingTemplate};
    let mut logic = GameLogic::new();
    let mut t = ThingTemplate::new("ScanLeash");
    t.add_kind_of(KindOf::Infantry);
    t.add_kind_of(KindOf::Attackable);
    let id = ObjectId(6310);
    let mut o = Object::new(t, id, Team::USA);
    o.set_position(glam::Vec3::ZERO);
    o.vision_range = 180.0;
    o.weapon = Some(wave21_guard_weapon());
    logic.objects.insert(id, o);

    let (inner, outer) = logic.host_std_guard_ranges(id);
    assert!(inner > 0.0 && outer > 0.0);
    let inner_1_5 = 1.5 * inner;
    let aggressor = outer + inner;
    assert!(
        aggressor > inner_1_5,
        "aggressor {aggressor} must exceed 1.5x inner {inner_1_5}"
    );
    let between = (inner_1_5 + aggressor) * 0.5;

    let vid = ObjectId(6311);
    let mut vt = ThingTemplate::new("DeadAggr26");
    vt.add_kind_of(KindOf::Infantry);
    logic.objects.insert(vid, {
        let mut e = Object::new(vt, vid, Team::GLA);
        e.set_position(glam::Vec3::new(10.0, 0.0, 0.0));
        e
    });

    let sid = ObjectId(6312);
    let mut st = ThingTemplate::new("ScanPrey26");
    st.add_kind_of(KindOf::Infantry);
    st.add_kind_of(KindOf::Attackable);
    logic.objects.insert(sid, {
        let mut e = Object::new(st, sid, Team::GLA);
        e.set_position(glam::Vec3::new(between, 0.0, 0.0));
        e
    });

    logic
        .objects
        .get_mut(&id)
        .unwrap()
        .begin_guard_retaliate(vid, Some(glam::Vec3::ZERO), None);
    logic.objects.get_mut(&vid).unwrap().status.destroyed = true;
    logic.objects.get_mut(&vid).unwrap().health.current = 0.0;
    // Place the scan victim inside acquire (inner) so lookForInner finds it,
    // then walk it out to the 1.5x..aggressor band.
    logic
        .objects
        .get_mut(&sid)
        .unwrap()
        .set_position(glam::Vec3::new(inner * 0.5, 0.0, 0.0));
    mark_guard_scan_due(&mut logic, id);
    logic.tick_guard_retaliate_states();
    assert_eq!(
        logic.objects[&id].guard_retaliate_victim,
        Some(sid),
        "dead aggressor must re-acquire the scan victim"
    );
    assert_eq!(
        logic.objects[&id].guard_chase_phase, 1,
        "scan re-acquire must enter Inner (no aggressor timer)"
    );
    assert_eq!(
        logic.objects[&id].guard_chase_give_up_frame, 0,
        "Inner scan victims are timer-free"
    );

    logic
        .objects
        .get_mut(&sid)
        .unwrap()
        .set_position(glam::Vec3::new(between, 0.0, 0.0));
    logic.tick_guard_retaliate_states();
    assert_eq!(
        logic.objects[&id].guard_chase_phase, 2,
        "INNER leash fail must escalate to OUTER (C++ success AND failure → OUTER)"
    );
    assert_eq!(
        logic.objects[&id].guard_retaliate_victim,
        Some(sid),
        "OUTER onEnter re-attacks the same nemesis"
    );
    assert!(
        logic.objects[&id].guard_chase_give_up_frame > 0,
        "OUTER must stamp GuardChaseUnitsDuration"
    );

    logic.tick_guard_retaliate_states();
    assert!(
        logic.objects[&id].guard_retaliate_victim.is_none(),
        "scan victim past OUTER 0.67*(vision+std) must then drop"
    );
}

#[test]
fn guard_walks_back_to_post_after_chase() {
    use crate::game_logic::{AIState, KindOf, Object, ObjectId, Team, ThingTemplate};
    let mut logic = GameLogic::new();
    let mut gt = ThingTemplate::new("ReturnGuard");
    gt.add_kind_of(KindOf::Infantry);
    gt.add_kind_of(KindOf::Attackable);
    let gid = ObjectId(6320);
    let mut g = Object::new(gt, gid, Team::China);
    g.set_position(glam::Vec3::new(50.0, 0.0, 0.0));
    g.guard_position = Some(glam::Vec3::ZERO);
    g.vision_range = 200.0;
    g.weapon = Some(wave21_guard_weapon());
    g.set_ai_state(AIState::Attacking);
    g.guard_chase_phase = 1;
    g.status.attacking = true;
    logic.objects.insert(gid, g);

    let eid = ObjectId(6321);
    let mut et = ThingTemplate::new("DeadInsideRing");
    et.add_kind_of(KindOf::Infantry);
    et.add_kind_of(KindOf::Attackable);
    logic.objects.insert(eid, {
        let mut e = Object::new(et, eid, Team::USA);
        e.set_position(glam::Vec3::new(40.0, 0.0, 0.0));
        e.status.destroyed = true;
        e.health.current = 0.0;
        e
    });
    logic.objects.get_mut(&gid).unwrap().target = Some(eid);

    let (inner, _) = logic.host_std_guard_ranges(gid);
    assert!(50.0 < inner, "kill site must sit inside the inner ring");

    logic.update_support_states(&[gid, eid], 1.0 / 30.0);
    let g = &logic.objects[&gid];
    let dest = g
        .movement
        .target_position
        .or_else(|| g.movement.path.last().copied())
        .expect("return-to-post must issue a destination");
    assert!(
        dest.x.abs() < 15.0 && dest.z.abs() < 15.0,
        "must walk back to the post, got {dest:?}"
    );
    assert!(
        matches!(g.ai_state, AIState::GuardingArea),
        "return must restore GuardingArea, got {:?}",
        g.ai_state
    );
}

#[test]
fn without_pursuit_acquires_while_guarder_outside_ring() {
    use crate::game_logic::{AIState, GuardMode, KindOf, Object, ObjectId, Team, ThingTemplate};
    let mut logic = GameLogic::new();
    let mut gt = ThingTemplate::new("NoPursuitGuard");
    gt.add_kind_of(KindOf::Infantry);
    gt.add_kind_of(KindOf::Attackable);
    let gid = ObjectId(6330);
    let mut g = Object::new(gt, gid, Team::China);
    g.set_position(glam::Vec3::new(250.0, 0.0, 0.0));
    g.guard_position = Some(glam::Vec3::ZERO);
    g.guard_mode = GuardMode::WithoutPursuit;
    g.vision_range = 180.0;
    g.weapon = Some(wave21_guard_weapon());
    g.set_ai_state(AIState::GuardingArea);
    logic.objects.insert(gid, g);

    let (inner, _) = logic.host_std_guard_ranges(gid);
    assert!(250.0 > inner && inner > 20.0);

    let mut et = ThingTemplate::new("InsideRing");
    et.add_kind_of(KindOf::Infantry);
    et.add_kind_of(KindOf::Attackable);
    let eid = ObjectId(6331);
    let mut e = Object::new(et, eid, Team::USA);
    e.set_position(glam::Vec3::new(20.0, 0.0, 0.0));
    logic.objects.insert(eid, e);

    mark_guard_scan_due(&mut logic, gid);
    logic.update_support_states(&[gid, eid], 1.0 / 30.0);
    assert_eq!(
        logic.objects[&gid].target,
        Some(eid),
        "WithoutPursuit must still acquire a target inside the ring while the guarder walks back"
    );
}

#[test]
fn guard_retaliate_outer_refreshes_timer_while_victim_in_std_guard() {
    // C++ AIGuardRetaliateOuterState::update: if goal is within stdGuardRange
    // of the center, m_attackGiveUpFrame = now + GuardChaseUnitsDuration.
    use crate::game_logic::{KindOf, Object, ObjectId, Team, ThingTemplate};
    let mut logic = GameLogic::new();
    let mut t = ThingTemplate::new("OuterRefresh");
    t.add_kind_of(KindOf::Infantry);
    t.add_kind_of(KindOf::Attackable);
    let id = ObjectId(6401);
    let mut o = Object::new(t, id, Team::USA);
    o.set_position(glam::Vec3::ZERO);
    o.vision_range = 180.0;
    o.weapon = Some(wave21_guard_weapon());
    logic.objects.insert(id, o);

    let vid = ObjectId(6402);
    let mut vt = ThingTemplate::new("OuterPrey");
    vt.add_kind_of(KindOf::Infantry);
    vt.add_kind_of(KindOf::Attackable);
    logic.objects.insert(vid, {
        let mut e = Object::new(vt, vid, Team::GLA);
        e.set_position(glam::Vec3::new(10.0, 0.0, 0.0));
        e
    });

    {
        let o = logic.objects.get_mut(&id).unwrap();
        o.begin_guard_retaliate(vid, Some(glam::Vec3::ZERO), None);
        o.guard_chase_phase = 2;
        o.guard_chase_give_up_frame = logic.frame.saturating_add(1);
        o.target = Some(vid);
    }
    let frames = logic.host_guard_chase_unit_frames();
    logic.tick_guard_retaliate_states();
    assert_eq!(
        logic.objects[&id].guard_chase_give_up_frame,
        logic.frame.saturating_add(frames),
        "OUTER must refresh give-up while the victim stays inside stdGuard"
    );
    assert_eq!(
        logic.objects[&id].guard_retaliate_victim,
        Some(vid),
        "in-range OUTER victim must not be dropped"
    );
}

#[test]
fn tn_guard_nemesis_uses_tunnel_attack_goal() {
    // C++ AITNGuardIdleState::lookForInnerTarget: tunnel getAI()->getGoalObject()
    // ENEMIES become the shared nemesis even when the tunnel took no damage.
    use crate::game_logic::{KindOf, Player, Team, ThingTemplate};
    let mut logic = GameLogic::new();
    logic.add_player(Player::new(0, Team::USA, "USA", true));
    logic.add_player(Player::new(1, Team::GLA, "GLA AI", false));

    let tnl = create_test_tunnel_network(&mut logic, glam::Vec3::ZERO);
    if let Some(o) = logic.host_object_mut(tnl) {
        o.owner_player_id = Some(1);
    }

    let mut rebel = ThingTemplate::new("GLARebelGoal");
    rebel
        .add_kind_of(KindOf::Infantry)
        .add_kind_of(KindOf::Attackable)
        .set_health(100.0);
    logic.templates.insert("GLARebelGoal".into(), rebel);
    let uid = logic
        .create_object("GLARebelGoal", Team::GLA, glam::Vec3::new(1.0, 0.0, 0.0))
        .expect("rebel");
    if let Some(u) = logic.host_object_mut(uid) {
        u.owner_player_id = Some(1);
        u.guard_target = Some(tnl);
        u.weapon = Some(wave21_guard_weapon());
        u.team_instance_name = "GLA_TunnelGuard".into();
        u.set_contained_by(Some(tnl));
    }

    let mut et = ThingTemplate::new("USARangerGoal");
    et.add_kind_of(KindOf::Infantry)
        .add_kind_of(KindOf::Attackable)
        .set_health(100.0);
    logic.templates.insert("USARangerGoal".into(), et);
    let eid = logic
        .create_object("USARangerGoal", Team::USA, glam::Vec3::new(20.0, 0.0, 0.0))
        .expect("enemy");
    if let Some(e) = logic.host_object_mut(eid) {
        e.owner_player_id = Some(0);
    }

    let gla_key = logic
        .host_object(tnl)
        .map(|o| o.tunnel_system_key())
        .expect("tn key");
    logic.tunnel_network.on_tunnel_created(gla_key, tnl);
    assert!(logic.tunnel_network.record_enter(gla_key, uid, tnl));

    logic.objects.get_mut(&tnl).unwrap().target = Some(eid);
    logic.update_support_states(&[tnl, uid, eid], 1.0 / 30.0);
    assert_eq!(
        logic
            .tunnel_network
            .get_cur_nemesis_id(gla_key, logic.frame),
        Some(eid),
        "tunnel getGoalObject must become the shared nemesis"
    );
    assert!(
        logic.host_object(uid).unwrap().contained_by.is_none(),
        "pool must sally against the tunnel's attack victim"
    );
}

#[test]
fn tn_guard_nemesis_skips_no_effect_and_stale_scan() {
    // C++ TunnelContain::update: info->m_noEffect skip + lastDamageTimestamp
    // windowed by TheAI->getAiData()->m_guardEnemyScanRate.
    use crate::game_logic::combat::DamageType;
    use crate::game_logic::{KindOf, Player, Team, ThingTemplate};
    let mut logic = GameLogic::new();
    logic.add_player(Player::new(0, Team::USA, "USA", true));
    logic.add_player(Player::new(1, Team::GLA, "GLA AI", false));

    let tnl = create_test_tunnel_network(&mut logic, glam::Vec3::ZERO);
    if let Some(o) = logic.host_object_mut(tnl) {
        o.owner_player_id = Some(1);
    }

    let mut et = ThingTemplate::new("StatusDmgSrc");
    et.add_kind_of(KindOf::Infantry)
        .add_kind_of(KindOf::Attackable)
        .set_health(100.0);
    logic.templates.insert("StatusDmgSrc".into(), et);
    let eid = logic
        .create_object("StatusDmgSrc", Team::USA, glam::Vec3::new(20.0, 0.0, 0.0))
        .expect("enemy");
    if let Some(e) = logic.host_object_mut(eid) {
        e.owner_player_id = Some(0);
    }

    let gla_key = logic
        .host_object(tnl)
        .map(|o| o.tunnel_system_key())
        .expect("tn key");
    logic.tunnel_network.on_tunnel_created(gla_key, tnl);

    {
        let o = logic.objects.get_mut(&tnl).unwrap();
        o.last_damage_source = Some(eid);
        o.last_damage_timestamp = Some(logic.frame);
        o.last_damage_info_type = Some(DamageType::Status);
    }
    logic.update_support_states(&[tnl, eid], 1.0 / 30.0);
    assert!(
        logic
            .tunnel_network
            .get_cur_nemesis_id(gla_key, logic.frame)
            .is_none(),
        "no-effect (status) damage must not rally the tunnel-guard pool"
    );

    let rate = logic.host_guard_enemy_scan_rate().max(1);
    logic.frame = rate;
    {
        let o = logic.objects.get_mut(&tnl).unwrap();
        o.last_damage_info_type = Some(DamageType::Bullet);
        o.last_damage_timestamp = Some(0);
    }
    logic.update_support_states(&[tnl, eid], 1.0 / 30.0);
    assert!(
        logic
            .tunnel_network
            .get_cur_nemesis_id(gla_key, logic.frame)
            .is_none(),
        "stale last-damage outside GuardEnemyScanRate must not write nemesis"
    );

    {
        let o = logic.objects.get_mut(&tnl).unwrap();
        o.last_damage_timestamp = Some(1);
    }
    logic.update_support_states(&[tnl, eid], 1.0 / 30.0);
    assert_eq!(
        logic
            .tunnel_network
            .get_cur_nemesis_id(gla_key, logic.frame),
        Some(eid),
        "fresh health-damaging hit inside the scan window must write nemesis"
    );
}

#[test]
fn tn_guard_nemesis_rejects_unattackable_damager() {
    // C++ lookForInnerTarget: getAbleToAttackSpecificObject(
    // ATTACK_TUNNEL_NETWORK_GUARD) — Unattackable must not hijack the slot.
    use crate::game_logic::{KindOf, Player, Team, ThingTemplate};
    let mut logic = GameLogic::new();
    logic.add_player(Player::new(0, Team::USA, "USA", true));
    logic.add_player(Player::new(1, Team::GLA, "GLA AI", false));

    let tnl = create_test_tunnel_network(&mut logic, glam::Vec3::ZERO);
    if let Some(o) = logic.host_object_mut(tnl) {
        o.owner_player_id = Some(1);
    }

    let mut rebel = ThingTemplate::new("GLARebelGate");
    rebel
        .add_kind_of(KindOf::Infantry)
        .add_kind_of(KindOf::Attackable)
        .set_health(100.0);
    logic.templates.insert("GLARebelGate".into(), rebel);
    let uid = logic
        .create_object("GLARebelGate", Team::GLA, glam::Vec3::new(1.0, 0.0, 0.0))
        .expect("rebel");
    if let Some(u) = logic.host_object_mut(uid) {
        u.owner_player_id = Some(1);
        u.guard_target = Some(tnl);
        u.weapon = Some(wave21_guard_weapon());
        u.team_instance_name = "GLA_TunnelGate".into();
        u.set_contained_by(Some(tnl));
    }

    let mut et = ThingTemplate::new("UnattackableSrc");
    et.add_kind_of(KindOf::Infantry)
        .add_kind_of(KindOf::Attackable)
        .add_kind_of(KindOf::Unattackable)
        .set_health(100.0);
    logic.templates.insert("UnattackableSrc".into(), et);
    let eid = logic
        .create_object(
            "UnattackableSrc",
            Team::USA,
            glam::Vec3::new(20.0, 0.0, 0.0),
        )
        .expect("enemy");
    if let Some(e) = logic.host_object_mut(eid) {
        e.owner_player_id = Some(0);
    }

    let gla_key = logic
        .host_object(tnl)
        .map(|o| o.tunnel_system_key())
        .expect("tn key");
    logic.tunnel_network.on_tunnel_created(gla_key, tnl);
    assert!(logic.tunnel_network.record_enter(gla_key, uid, tnl));

    {
        let o = logic.objects.get_mut(&tnl).unwrap();
        o.last_damage_source = Some(eid);
        o.last_damage_timestamp = Some(logic.frame);
        o.last_damage_info_type = Some(crate::game_logic::combat::DamageType::Bullet);
    }
    logic.update_support_states(&[tnl, uid, eid], 1.0 / 30.0);
    assert!(
        logic
            .tunnel_network
            .get_cur_nemesis_id(gla_key, logic.frame)
            .is_none(),
        "unattackable damager must not hijack the shared nemesis slot"
    );
    assert_eq!(
        logic.host_object(uid).unwrap().contained_by,
        Some(tnl),
        "pool must stay inside when the damager is not attackable"
    );
}
