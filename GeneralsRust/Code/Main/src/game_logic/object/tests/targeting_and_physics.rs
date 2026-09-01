//! Behavior suite extracted from the original test module.
use super::*;

#[test]
fn return_to_base_blocks_fire_until_rearm() {
    use crate::game_logic::{KindOf, Team, ThingTemplate, Weapon};
    use glam::Vec3;
    let mut tmpl = ThingTemplate::new("AmericaJetRaptor");
    // Seed-only name so store cannot peel YES over RETURN_TO_BASE.
    tmpl.primary_weapon_name = Some("HostTestRaptorJetMissileWeapon".into());
    tmpl.add_kind_of(KindOf::Aircraft);
    tmpl.add_kind_of(KindOf::Attackable);
    let mut jet = Object::new(tmpl, ObjectId(1), Team::USA);
    jet.set_position(Vec3::ZERO);
    jet.weapon = Some(Weapon {
        damage: 100.0,
        range: 200.0,
        reload_time: 0.0,
        last_fire_time: -100.0,
        ammo: Some(2),
        clip_size: 2,
        can_target_air: true,
        can_target_ground: true,
        ..Weapon::default()
    });
    let tgt = ObjectId(9);
    assert!(jet.fire_at(tgt, 1.0));
    assert_eq!(jet.weapon.as_ref().unwrap().ammo, Some(1));
    assert!(jet.fire_at(tgt, 1.0));
    assert_eq!(jet.weapon.as_ref().unwrap().ammo, Some(0));
    assert!(jet.needs_return_to_base_rearm());
    assert!(!jet.fire_at(tgt, 2.0));
    assert!(!Object::weapon_ready_named(
        jet.weapon.as_ref().unwrap(),
        2.0,
        Some("HostTestRaptorJetMissileWeapon"),
        jet.weapon.as_ref().unwrap().reload_time,
    ));
    assert!(jet.rearm_return_to_base_weapons());
    assert_eq!(jet.weapon.as_ref().unwrap().ammo, Some(2));
    assert!(jet.fire_at(tgt, 3.0));
    assert_eq!(jet.weapon.as_ref().unwrap().ammo, Some(1));
}

#[test]
fn auto_reload_still_refills_clip() {
    use crate::game_logic::Weapon;
    let mut w = Weapon {
        damage: 10.0,
        range: 100.0,
        reload_time: 0.1,
        ammo: Some(1),
        clip_size: 2,
        clip_reload_time: 1.0,
        last_fire_time: -100.0,
        ..Weapon::default()
    };
    let t0 = 5.0;
    assert!(Object::weapon_ready(&w, t0));
    Object::consume_ammo_on_fire(&mut w, t0);
    assert_eq!(w.ammo, Some(0));
    // After clip reload gap, ready again and refill on fire.
    assert!(
        Object::weapon_ready(&w, t0 + 1.05),
        "last_fire={} reload={}",
        w.last_fire_time,
        w.reload_time
    );
    Object::consume_ammo_on_fire(&mut w, t0 + 1.05);
    assert_eq!(w.ammo, Some(1)); // refilled to 2, spent 1
}

#[test]
fn out_of_ammo_damage_ticks_empty_rtb_jet() {
    use crate::game_logic::{KindOf, Team, ThingTemplate, Weapon};
    use glam::Vec3;
    let mut tmpl = ThingTemplate::new("AmericaJetRaptor");
    tmpl.primary_weapon_name = Some("HostTestRaptorJetMissileWeapon".into());
    tmpl.add_kind_of(KindOf::Aircraft);
    tmpl.add_kind_of(KindOf::Attackable);
    tmpl.set_health(100.0);
    let mut jet = Object::new(tmpl, ObjectId(1), Team::USA);
    jet.set_position(Vec3::new(0.0, 50.0, 0.0));
    jet.status.airborne_target = true;
    jet.weapon = Some(Weapon {
        damage: 100.0,
        range: 200.0,
        reload_time: 0.0,
        last_fire_time: -100.0,
        ammo: Some(0),
        clip_size: 2,
        can_target_air: true,
        can_target_ground: true,
        ..Weapon::default()
    });
    assert!(jet.needs_return_to_base_rearm());
    let hp0 = jet.health.current;
    let dmg = jet.apply_out_of_ammo_damage_frame();
    // 10% / sec * 1/30 * 100 = 10/30 ≈ 0.333
    assert!((dmg - (0.10 / 30.0) * 100.0).abs() < 1e-3, "dmg={dmg}");
    assert!((hp0 - jet.health.current - dmg).abs() < 1e-3);
    // Docked: no damage.
    jet.health.current = 100.0;
    jet.set_ai_state(AIState::Docked);
    assert_eq!(jet.apply_out_of_ammo_damage_frame(), 0.0);
    // Rearmed: no damage.
    jet.set_ai_state(AIState::Idle);
    jet.rearm_return_to_base_weapons();
    assert_eq!(jet.apply_out_of_ammo_damage_frame(), 0.0);
}

#[test]
fn airfield_rearm_duration_is_remaining_biased() {
    use crate::game_logic::{KindOf, Team, ThingTemplate, Weapon};
    let mut tmpl = ThingTemplate::new("AmericaJetRaptor");
    tmpl.primary_weapon_name = Some("HostTestRaptorJetMissileWeapon".into());
    tmpl.add_kind_of(KindOf::Aircraft);
    let mut jet = Object::new(tmpl, ObjectId(1), Team::USA);
    jet.weapon = Some(Weapon {
        damage: 50.0,
        range: 200.0,
        reload_time: 0.0,
        last_fire_time: -100.0,
        ammo: Some(2),
        clip_size: 4,
        clip_reload_time: 8.0,
        can_target_air: true,
        can_target_ground: true,
        ..Weapon::default()
    });
    // C++ (rt * needed) / clipSize = (240 * 2) / 4 = 120.
    assert_eq!(jet.airfield_rearm_clip_reload_frames(), 120);
    jet.weapon.as_mut().unwrap().ammo = Some(0);
    assert_eq!(jet.airfield_rearm_clip_reload_frames(), 240);
}

#[test]
fn parked_rearm_fills_clip_percent_over_time() {
    use crate::game_logic::{KindOf, Team, ThingTemplate, Weapon};
    let mut tmpl = ThingTemplate::new("AmericaJetRaptor");
    tmpl.primary_weapon_name = Some("HostTestRaptorJetMissileWeapon".into());
    tmpl.add_kind_of(KindOf::Aircraft);
    let mut jet = Object::new(tmpl, ObjectId(1), Team::USA);
    jet.weapon = Some(Weapon {
        damage: 50.0,
        range: 200.0,
        reload_time: 0.0,
        last_fire_time: -100.0,
        ammo: Some(0),
        clip_size: 4,
        clip_reload_time: 8.0,
        can_target_air: true,
        can_target_ground: true,
        ..Weapon::default()
    });
    jet.begin_parked_airfield_rearm(10);
    assert_eq!(jet.airfield_rearm_ready_frame, Some(250));
    assert!(!jet.tick_parked_airfield_rearm(10));
    assert_eq!(jet.weapon.as_ref().unwrap().ammo, Some(0));
    assert!(!jet.tick_parked_airfield_rearm(70));
    assert_eq!(jet.weapon.as_ref().unwrap().ammo, Some(1));
    assert!(!jet.tick_parked_airfield_rearm(130));
    assert_eq!(jet.weapon.as_ref().unwrap().ammo, Some(2));
    assert!(jet.tick_parked_airfield_rearm(250));
    assert_eq!(jet.weapon.as_ref().unwrap().ammo, Some(4));
    assert!(jet.airfield_rearm_ready_frame.is_none());
}

#[test]
fn empty_jet_circles_last_airfield_not_own_pos() {
    use crate::game_logic::{KindOf, Team, ThingTemplate, Weapon};
    use glam::Vec3;
    let mut tmpl = ThingTemplate::new("AmericaJetRaptor");
    tmpl.primary_weapon_name = Some("HostTestRaptorJetMissileWeapon".into());
    tmpl.add_kind_of(KindOf::Aircraft);
    tmpl.set_health(100.0);
    let mut jet = Object::new(tmpl, ObjectId(1), Team::USA);
    jet.set_position(Vec3::new(2000.0, 50.0, 0.0));
    jet.weapon = Some(Weapon {
        damage: 50.0,
        range: 200.0,
        ammo: Some(0),
        clip_size: 4,
        can_target_air: true,
        can_target_ground: true,
        ..Weapon::default()
    });
    jet.capture_jet_producer_location(Some(Vec3::ZERO));
    assert!(!jet.is_at_jet_producer_location(80.0));
    assert!(jet.enter_circling_dead_airfield(1));
    assert!(jet.jet_circling_dead_airfield);
    jet.leave_circling_dead_airfield();
    jet.set_position(Vec3::new(10.0, 50.0, 0.0));
    assert!(jet.is_at_jet_producer_location(80.0));
}

#[test]
fn parked_jet_takeoff_on_attack_and_move() {
    use crate::game_logic::host_dock_contain_exit_heal_residual::PARKING_PLACE_AIRFIELD_APPROACH_HEIGHT;
    use crate::game_logic::{KindOf, Team, ThingTemplate, Weapon};
    use glam::Vec3;
    let mut tmpl = ThingTemplate::new("AmericaJetRaptor");
    tmpl.primary_weapon_name = Some("HostTestRaptorJetMissileWeapon".into());
    tmpl.add_kind_of(KindOf::Aircraft);
    tmpl.add_kind_of(KindOf::Attackable);
    tmpl.set_health(100.0);
    let mut jet = Object::new(tmpl, ObjectId(1), Team::USA);
    jet.set_position(Vec3::new(0.0, 0.0, 0.0));
    jet.weapon = Some(Weapon {
        damage: 50.0,
        range: 200.0,
        reload_time: 0.0,
        last_fire_time: -100.0,
        ammo: Some(4),
        clip_size: 4,
        can_target_air: true,
        can_target_ground: true,
        ..Weapon::default()
    });
    jet.contained_by = Some(ObjectId(99));
    jet.set_ai_state(AIState::Docked);
    jet.status.airborne_target = false;
    assert!(jet.is_parked_at_airfield());
    assert!(jet.can_attack()); // parked aircraft may sortie
    jet.attack_target(ObjectId(7));
    assert!(jet.contained_by.is_none());
    assert_ne!(jet.ai_state, AIState::Docked);
    assert!(jet.status.airborne_target);
    assert!(jet.get_position().y >= PARKING_PLACE_AIRFIELD_APPROACH_HEIGHT - 1e-3);
    assert_eq!(jet.target, Some(ObjectId(7)));
    assert_eq!(jet.ai_state, AIState::Attacking);

    // Re-dock and move.
    jet.contained_by = Some(ObjectId(99));
    jet.set_ai_state(AIState::Docked);
    jet.status.airborne_target = false;
    jet.set_position(Vec3::new(10.0, 0.0, 0.0));
    jet.set_destination(Vec3::new(100.0, 0.0, 0.0));
    assert!(jet.contained_by.is_none());
    assert!(jet.status.airborne_target || jet.ai_state != AIState::Docked);
    assert!(jet.get_position().y >= PARKING_PLACE_AIRFIELD_APPROACH_HEIGHT - 1e-3);
}

#[test]
fn fire_at_scatter_vs_infantry_only_when_flagged() {
    use crate::game_logic::weapon_bootstrap::host_effective_scatter_radius;
    // Crusader gun: base 0 + ScatterRadiusVsInfantry 10.
    let vs_inf = host_effective_scatter_radius("AmericaTankCrusaderGun", true);
    let vs_veh = host_effective_scatter_radius("AmericaTankCrusaderGun", false);
    assert!(vs_inf >= 10.0 - 1e-3, "vs infantry {vs_inf}");
    assert!(vs_veh < 1e-3, "vs vehicle base {vs_veh}");
    // fire_at_ex is the KindOf-aware entry; fire_at defaults infantry=false (base only).
    let src = crate::game_logic::object::OBJECT_SRC;
    assert!(src.contains("fn fire_at_ex"));
    assert!(src.contains("target_is_infantry"));
    assert!(
        src.contains("host_effective_scatter_radius"),
        "fire path must peel scatter"
    );
}

#[test]
fn shock_wave_impulse_knocks_ground_units() {
    use crate::game_logic::host_enum_table_residual::{
        MC_BIT_STUNNED, MC_BIT_STUNNED_FLAILING, host_model_condition_has,
    };
    let mut tmpl = ThingTemplate::new("ShockVic");
    tmpl.add_kind_of(KindOf::Vehicle);
    let mut o = Object::new(tmpl, ObjectId(1), Team::USA);
    o.movement.velocity = glam::Vec3::ZERO;
    assert!(o.apply_shock_wave_impulse(glam::Vec3::new(20.0, 10.0, 0.0)));
    assert!(o.movement.velocity.length() > 0.0);
    assert!(o.is_shock_stunned());
    assert!(host_model_condition_has(
        o.model_condition_bits,
        MC_BIT_STUNNED_FLAILING
    ));
    // After flail window: STUNNED bit.
    o.shock_stun_frames = 10;
    o.refresh_model_condition_bits();
    assert!(host_model_condition_has(
        o.model_condition_bits,
        MC_BIT_STUNNED
    ));
    assert!(!host_model_condition_has(
        o.model_condition_bits,
        MC_BIT_STUNNED_FLAILING
    ));
    // Aircraft immune.
    let mut at = ThingTemplate::new("ShockAir");
    at.add_kind_of(KindOf::Aircraft);
    let mut a = Object::new(at, ObjectId(2), Team::USA);
    a.status.airborne_target = true;
    assert!(!a.apply_shock_wave_impulse(glam::Vec3::new(20.0, 10.0, 0.0)));
}

#[test]
fn shock_stun_ticks_clear_model_bits() {
    use crate::game_logic::host_enum_table_residual::{
        MC_BIT_STUNNED, MC_BIT_STUNNED_FLAILING, host_model_condition_has,
    };
    let mut tmpl = ThingTemplate::new("StunTick");
    tmpl.add_kind_of(KindOf::Infantry);
    let mut o = Object::new(tmpl, ObjectId(3), Team::USA);
    assert!(o.apply_shock_wave_impulse(glam::Vec3::new(1.5, 3.0, 0.0)));
    // C++ setStunned(true) arms no duration (Object.cpp:1832); IS_STUNNED
    // clears via relief only — |vel| < STUN_RELIEF_EPSILON or
    // !isSignificantlyAboveTerrain (PhysicsUpdate.cpp:671-683). The host
    // carries that as the u32::MAX no-duration sentinel in
    // shock_stun_frames, so tick until relief lands (bounce arcs decay
    // geometrically) instead of counting down a bounded timer.
    assert!(o.shock_stun_frames > 0);
    for _ in 0..1200 {
        if o.shock_stun_frames == 0 {
            break;
        }
        o.tick_shock_stun();
    }
    assert_eq!(o.shock_stun_frames, 0);
    assert!(!host_model_condition_has(
        o.model_condition_bits,
        MC_BIT_STUNNED_FLAILING
    ));
    assert!(!host_model_condition_has(
        o.model_condition_bits,
        MC_BIT_STUNNED
    ));
}

#[test]
fn ignore_collisions_and_overlap_helpers() {
    let mut a = Object::new(
        {
            let mut t = ThingTemplate::new("IgnA");
            t.add_kind_of(KindOf::Vehicle);
            t
        },
        ObjectId(301),
        Team::USA,
    );
    let b_id = ObjectId(302);
    assert!(!a.is_ignoring_collisions_with(b_id));
    a.set_ignore_collisions_with(Some(b_id));
    assert!(a.is_ignoring_collisions_with(b_id));
    a.set_ignore_collisions_with(None);
    assert!(!a.is_ignoring_collisions_with(b_id));

    a.add_physics_overlap(b_id);
    assert!(a.is_currently_overlapped(b_id));
    assert!(!a.was_previously_overlapped(b_id));
    a.advance_physics_overlap_frame();
    assert!(!a.is_currently_overlapped(b_id));
    assert!(a.was_previously_overlapped(b_id));
    a.last_collidee = Some(b_id);
    assert_eq!(a.last_collidee, Some(b_id));
}

#[test]
fn crush_selects_front_or_back_by_approach() {
    use crate::game_logic::host_partition_collision_physics_residual::{
        CrushTarget, select_crush_target_by_perp_residual,
    };
    // Sanity on residual selector.
    assert_eq!(
        select_crush_target_by_perp_residual(
            false,
            false,
            (4.0, 0.5),
            (0.0, 0.0),
            (1.0, 0.0),
            (1.0, 0.0),
            5.0,
        ),
        CrushTarget::FrontEndCrush
    );
    // Approach front of infantry: tank past front point only → front_crushed first.
    let mut vt = ThingTemplate::new("FrontCrushTank");
    vt.add_kind_of(KindOf::Vehicle);
    let mut tank = Object::new(vt, ObjectId(201), Team::USA);
    tank.crusher_level = 1;
    tank.set_orientation(0.0);
    tank.movement.velocity = glam::Vec3::new(5.0, 0.0, 0.0);
    // Front of inf at x≈5 (offset 5, facing +X): tank just past front.
    tank.set_position(glam::Vec3::new(5.5, 0.0, 0.2));

    let mut it = ThingTemplate::new("FrontCrushInf");
    it.add_kind_of(KindOf::Infantry);
    let mut inf = Object::new(it, ObjectId(202), Team::GLA);
    inf.crushable_level = 0;
    inf.selection_radius = 10.0;
    inf.set_orientation(0.0);
    inf.set_position(glam::Vec3::new(0.0, 0.0, 0.0));
    inf.health.current = 999999.0; // survive first non-total if needed
    inf.health.maximum = 999999.0;

    // With front selection + past front point, front_crushed set.
    // Use huge HP so we can observe flags before death if total.
    assert!(tank.check_for_overlap_collision(&mut inf, false));
    // Either front crushed or total (if selector picked total and killed).
    assert!(
        inf.front_crushed || inf.back_crushed || inf.status.destroyed,
        "front={} back={} dead={}",
        inf.front_crushed,
        inf.back_crushed,
        inf.status.destroyed
    );
}

#[test]
fn crush_overlap_collision_kills_infantry() {
    use crate::game_logic::host_usa_pilot::HostDeathType;
    let mut vt = ThingTemplate::new("CrusherTank");
    vt.add_kind_of(KindOf::Vehicle);
    let mut tank = Object::new(vt, ObjectId(91), Team::USA);
    tank.crusher_level = 1;
    tank.set_orientation(0.0); // faces +X
    tank.movement.velocity = glam::Vec3::new(5.0, 0.0, 0.0); // moving +X

    let mut it = ThingTemplate::new("CrushableInf");
    it.add_kind_of(KindOf::Infantry);
    let mut inf = Object::new(it, ObjectId(92), Team::GLA);
    inf.crushable_level = 0;
    inf.has_squish_collide = true;
    inf.selection_radius = 10.0;
    // Tank past infantry center along +X.
    inf.set_position(glam::Vec3::new(5.0, 0.0, 0.0));
    tank.set_position(glam::Vec3::new(6.0, 0.0, 0.0));

    assert!(tank.can_crush_only(&inf, false));
    assert!(tank.check_for_overlap_collision(&mut inf, false));
    assert!(inf.status.destroyed || inf.health.current <= 0.0);
    if inf.status.destroyed {
        assert_eq!(inf.status.death_type, HostDeathType::Crushed);
    }
    // Allies do not crush.
    let mut a = Object::new(
        {
            let mut t = ThingTemplate::new("AllyInf");
            t.add_kind_of(KindOf::Infantry);
            t
        },
        ObjectId(93),
        Team::USA,
    );
    a.crushable_level = 0;
    a.set_position(glam::Vec3::new(5.0, 0.0, 0.0));
    tank.physics_current_overlap = None;
    tank.physics_previous_overlap = None;
    assert!(!tank.can_crush_only(&a, true));
    assert!(!tank.check_for_overlap_collision(&mut a, true));
}

#[test]
fn own_tank_is_blocked_by_own_infantry() {
    // C++ AIUpdate.cpp:1289-1290: canCrushOrSquish is false for ALLIES,
    // so blockedBy stays true. hq-8y2zz.
    let mut vt = ThingTemplate::new("OwnCrushTank");
    vt.add_kind_of(KindOf::Vehicle);
    let mut tank = Object::new(vt, ObjectId(501), Team::USA);
    tank.crusher_level = 1;
    tank.owner_player_id = Some(0);
    tank.set_orientation(0.0);
    tank.movement.velocity = glam::Vec3::new(5.0, 0.0, 0.0);
    tank.set_position(glam::Vec3::new(0.0, 0.0, 0.0));
    tank.selection_radius = 8.0;

    let mut it = ThingTemplate::new("OwnCrushInf");
    it.add_kind_of(KindOf::Infantry);
    let mut inf = Object::new(it, ObjectId(502), Team::USA);
    inf.crushable_level = 0;
    inf.owner_player_id = Some(0);
    inf.set_position(glam::Vec3::new(4.0, 0.0, 0.0));
    inf.set_orientation(0.0);
    inf.selection_radius = 5.0;

    assert!(
        !tank.can_crush_only(&inf, true),
        "ALLIES must not crush (Object.cpp:1096)"
    );
    assert!(
        tank.ai_blocked_by(&inf, true),
        "own infantry still blocks the tank"
    );
    assert!(
        !tank.ai_blocked_by(&inf, false),
        "enemy infantry with lower crushable is crush-through"
    );
}

#[test]
fn crushable_car_uses_front_back_not_instant_squish() {
    // C++ PhysicsUpdate.cpp:1466-1743 TEST_CRUSH_ONLY: cars use crush points,
    // not SquishCollide HUGE. hq-y3ueg.
    let mut vt = ThingTemplate::new("CarCrushTank");
    vt.add_kind_of(KindOf::Vehicle);
    let mut tank = Object::new(vt, ObjectId(511), Team::USA);
    tank.crusher_level = 2;
    tank.set_orientation(0.0);
    tank.movement.velocity = glam::Vec3::new(5.0, 0.0, 0.0);
    tank.set_position(glam::Vec3::new(0.0, 0.0, 0.0));
    tank.selection_radius = 8.0;

    let mut ct = ThingTemplate::new("CivilianCar");
    ct.add_kind_of(KindOf::Vehicle);
    let mut car = Object::new(ct, ObjectId(512), Team::Neutral);
    car.crushable_level = 1;
    car.crusher_level = 0;
    car.selection_radius = 10.0;
    car.set_orientation(0.0);
    car.set_position(glam::Vec3::new(10.0, 0.0, 0.0));
    car.health.current = 200.0;
    car.health.maximum = 200.0;

    assert!(tank.can_crush_only(&car, false));
    assert!(tank.check_for_overlap_collision(&mut car, false));
    assert!(
        car.is_alive() && car.health.current > 0.0,
        "first overlap is 0-damage; car must not instant-squish"
    );
    assert!(
        !(car.front_crushed && car.back_crushed),
        "cars use front/back crush points, not both flags at first contact"
    );
}

#[test]
fn squish_module_crushes_default_crushable_level() {
    // C++ TEST_SQUISH / SquishCollide: crushableLevel 255 still dies.
    use crate::game_logic::host_usa_pilot::HostDeathType;
    let mut vt = ThingTemplate::new("SquishTank");
    vt.add_kind_of(KindOf::Vehicle);
    let mut tank = Object::new(vt, ObjectId(521), Team::USA);
    tank.crusher_level = 1;
    tank.set_orientation(0.0);
    tank.movement.velocity = glam::Vec3::new(5.0, 0.0, 0.0);
    // C++ SquishCollide.cpp:93-99 squishes only when to·vel > 0, i.e. the
    // crusher must sit behind the victim moving toward it.
    tank.set_position(glam::Vec3::new(4.0, 0.0, 0.0));
    tank.selection_radius = 8.0;

    let mut it = ThingTemplate::new("SquishInf");
    it.add_kind_of(KindOf::Infantry);
    let mut inf = Object::new(it, ObjectId(522), Team::GLA);
    inf.crushable_level = 255;
    inf.has_squish_collide = true;
    inf.selection_radius = 8.0;
    inf.set_position(glam::Vec3::new(5.0, 0.0, 0.0));

    assert!(
        !tank.can_crush_only(&inf, false),
        "TEST_CRUSH_ONLY is levels only"
    );
    assert!(
        tank.can_crush_or_squish(&inf, false),
        "TEST_CRUSH_OR_SQUISH includes SquishCollide"
    );
    assert!(!tank.ai_blocked_by(&inf, false));
    assert!(tank.check_for_overlap_collision(&mut inf, false));
    assert!(inf.status.destroyed || inf.health.current <= 0.0);
    if inf.status.destroyed {
        assert_eq!(inf.status.death_type, HostDeathType::Crushed);
    }
}

#[test]
fn crush_points_use_authored_major_radius() {
    // PhysicsUpdate.cpp:1490 majorRadius/2, not selection/bounding circle.
    let mut vt = ThingTemplate::new("MajorTank");
    vt.add_kind_of(KindOf::Vehicle);
    let mut tank = Object::new(vt, ObjectId(531), Team::USA);
    tank.crusher_level = 2;
    tank.set_orientation(0.0);
    tank.movement.velocity = glam::Vec3::new(5.0, 0.0, 0.0);
    tank.set_position(glam::Vec3::new(15.0, 0.0, 0.0));
    tank.selection_radius = 8.0;

    let mut ct = ThingTemplate::new("MajorCar");
    ct.geometry_info = crate::game_logic::HostGeometryInfo {
        geom_type: crate::game_logic::HostGeometryType::Box,
        is_small: true,
        height: 8.0,
        major_radius: 6.0,
        minor_radius: 4.0,
        authored: true,
    };
    let mut car = Object::new(ct, ObjectId(532), Team::Neutral);
    car.crushable_level = 1;
    car.crusher_level = 0;
    car.selection_radius = 20.0;
    // Crosswise crushee: front/back points sit ±offset off the crusher's ray
    // (perp 3 each) so the on-axis center wins as TOTAL_CRUSH target
    // (PhysicsUpdate.cpp:1588-1655 center-perp branch).
    car.set_orientation(std::f32::consts::FRAC_PI_2);
    car.set_position(glam::Vec3::new(10.0, 0.0, 0.0));
    car.health.current = 200.0;
    car.health.maximum = 200.0;

    assert!(tank.can_crush_only(&car, false));
    assert!(tank.check_for_overlap_collision(&mut car, false));
    assert!(
        car.is_alive() && car.health.current > 0.0,
        "center is 5wu behind; major/2 window is 4.5wu so no HUGE crush"
    );
}

#[test]
fn overlap_crush_aims_with_facing_not_velocity() {
    // C++ PhysicsUpdate.cpp:1488 uses getUnitDirectionVector2D(), not velocity.
    // A tank facing +X but sliding backward must still crush along facing.
    let mut vt = ThingTemplate::new("FacingCrushTank");
    vt.add_kind_of(KindOf::Vehicle);
    let mut tank = Object::new(vt, ObjectId(541), Team::USA);
    tank.crusher_level = 2;
    tank.set_orientation(0.0);
    tank.movement.velocity = glam::Vec3::new(-5.0, 0.0, 0.0);
    tank.set_position(glam::Vec3::new(15.0, 0.0, 0.2));

    let mut ct = ThingTemplate::new("FacingCrushCar");
    ct.add_kind_of(KindOf::Vehicle);
    ct.geometry_info = crate::game_logic::HostGeometryInfo {
        geom_type: crate::game_logic::HostGeometryType::Box,
        is_small: true,
        height: 8.0,
        major_radius: 6.0,
        minor_radius: 4.0,
        authored: true,
    };
    let mut car = Object::new(ct, ObjectId(542), Team::Neutral);
    car.crushable_level = 1;
    car.crusher_level = 0;
    car.selection_radius = 10.0;
    car.set_orientation(0.0);
    car.set_position(glam::Vec3::new(10.0, 0.0, 0.0));
    car.health.current = 200.0;
    car.health.maximum = 200.0;

    assert!(tank.can_crush_only(&car, false));
    assert!(tank.check_for_overlap_collision(&mut car, false));
    assert!(
        car.status.destroyed || car.health.current <= 0.0,
        "facing past-point must crush even when velocity points the other way"
    );
}

#[test]
fn first_crush_of_car_is_not_always_total() {
    // C++ PhysicsBehavior does not stamp body flags. CrushDie::onDie then
    // crushLocationCheck against both-false writes FRONT or BACK, not TOTAL.
    let mut vt = ThingTemplate::new("HalfWreckTank");
    vt.add_kind_of(KindOf::Vehicle);
    let mut tank = Object::new(vt, ObjectId(551), Team::USA);
    tank.crusher_level = 2;
    tank.set_orientation(0.0);
    tank.movement.velocity = glam::Vec3::new(5.0, 0.0, 0.0);
    tank.set_position(glam::Vec3::new(15.0, 0.0, 0.2));

    let mut ct = ThingTemplate::new("HalfWreckCar");
    ct.add_kind_of(KindOf::Vehicle);
    ct.geometry_info = crate::game_logic::HostGeometryInfo {
        geom_type: crate::game_logic::HostGeometryType::Box,
        is_small: true,
        height: 8.0,
        major_radius: 6.0,
        minor_radius: 4.0,
        authored: true,
    };
    let mut car = Object::new(ct, ObjectId(552), Team::Neutral);
    car.crushable_level = 1;
    car.crusher_level = 0;
    car.selection_radius = 10.0;
    car.set_orientation(0.0);
    car.set_position(glam::Vec3::new(10.0, 0.0, 0.0));
    car.health.current = 200.0;
    car.health.maximum = 200.0;

    assert!(tank.check_for_overlap_collision(&mut car, false));
    assert!(car.status.destroyed || car.health.current <= 0.0);
    assert!(
        car.front_crushed && !car.back_crushed,
        "first crush must be FRONT wreck, not TOTAL (front={} back={})",
        car.front_crushed,
        car.back_crushed
    );
}

#[test]
fn scrub_velocity_and_structure_stiffness_bounce() {
    use crate::game_logic::host_partition_collision_physics_residual::{
        PHYSICS_STRUCTURE_STIFFNESS_DEFAULT_RESIDUAL, clamp_structure_stiffness,
        parachute_bounce_out_distance,
    };
    let mut tmpl = ThingTemplate::new("ScrubVic");
    tmpl.add_kind_of(KindOf::Vehicle);
    let mut o = Object::new(tmpl, ObjectId(71), Team::USA);
    o.movement.velocity = glam::Vec3::new(10.0, 0.0, 0.0);
    o.scrub_velocity_2d(5.0);
    assert!((o.movement.velocity.x - 5.0).abs() < 1e-3);
    assert!(o.movement.velocity.z.abs() < 1e-5);
    o.scrub_velocity_2d(0.0);
    assert_eq!(o.movement.velocity.x, 0.0);

    o.movement.velocity = glam::Vec3::new(0.0, -8.0, 0.0);
    o.scrub_velocity_vertical(-3.0);
    assert!((o.movement.velocity.y - (-3.0)).abs() < 1e-5);

    // Parachute bounce out.
    o.set_position(glam::Vec3::new(0.0, 5.0, 0.0));
    o.movement.velocity = glam::Vec3::new(4.0, -1.0, 0.0);
    o.apply_parachute_building_bounce_out(glam::Vec3::new(10.0, 5.0, 0.0), 20.0);
    assert!(o.get_position().x < 0.0, "pushed away from building +X");
    assert_eq!(o.movement.velocity.x, 0.0);
    assert_eq!(o.movement.velocity.z, 0.0);
    assert!((parachute_bounce_out_distance(20.0) - 2.0).abs() < 1e-6);

    // Structure stiffness bounce.
    o.set_position(glam::Vec3::new(0.0, 2.0, 0.0));
    o.movement.velocity = glam::Vec3::new(6.0, -2.0, 0.0);
    let f = o.apply_structure_stiffness_bounce(
        glam::Vec3::new(5.0, 2.0, 0.0),
        PHYSICS_STRUCTURE_STIFFNESS_DEFAULT_RESIDUAL,
        1.0,
    );
    assert!(f.x < 0.0, "push back -X force={f:?}");
    assert!(
        (o.movement.velocity.x - f.x).abs() < 1e-4,
        "zero-then-apply vel={:?} force={f:?}",
        o.movement.velocity
    );
    assert!((o.movement.velocity.y - f.y).abs() < 1e-4);
    assert!((clamp_structure_stiffness(0.5) - 0.5).abs() < 1e-6);
}

#[test]
fn vehicle_crash_into_structure_residual() {
    use crate::game_logic::host_partition_collision_physics_residual::{
        PHYSICS_VEHICLE_CRASHES_INTO_BUILDING_WEAPON, VehicleCrashImmobileOutcome,
        vehicle_crash_destroys_vehicle, vehicle_crash_weapon_name,
    };
    let mut vt = ThingTemplate::new("CrashVic");
    vt.add_kind_of(KindOf::Vehicle);
    let mut v = Object::new(vt, ObjectId(51), Team::USA);
    v.set_position(glam::Vec3::new(0.0, 5.0, 0.0));
    v.movement.velocity = glam::Vec3::new(0.0, -3.0, 0.0);

    let mut st = ThingTemplate::new("CrashBldg");
    st.add_kind_of(KindOf::Structure);
    st.add_kind_of(KindOf::Immobile);
    let s = Object::new(st, ObjectId(52), Team::China);

    let o = v.evaluate_vehicle_crash_into(&s);
    assert_eq!(o, VehicleCrashImmobileOutcome::DestroyWithBuildingWeapon);
    assert!(vehicle_crash_destroys_vehicle(o));
    assert_eq!(
        vehicle_crash_weapon_name(o),
        Some(PHYSICS_VEHICLE_CRASHES_INTO_BUILDING_WEAPON)
    );

    // Rising vehicle: no crash.
    v.movement.velocity.y = 2.0;
    assert_eq!(
        v.evaluate_vehicle_crash_into(&s),
        VehicleCrashImmobileOutcome::None
    );

    // Tossed infantry/debris: destroyObject, no crash weapon (hq-w78f4).
    let mut it = ThingTemplate::new("TossedRanger");
    it.add_kind_of(KindOf::Infantry);
    let mut inf = Object::new(it, ObjectId(53), Team::USA);
    inf.set_position(glam::Vec3::new(0.0, 5.0, 0.0));
    inf.movement.velocity = glam::Vec3::new(0.0, -3.0, 0.0);
    let io = inf.evaluate_vehicle_crash_into(&s);
    assert_eq!(io, VehicleCrashImmobileOutcome::DestroyWithoutWeapon);
    assert!(vehicle_crash_destroys_vehicle(io));
    assert!(vehicle_crash_weapon_name(io).is_none());
}

#[test]
fn kill_when_resting_and_bounce_land_residual() {
    let mut tmpl = ThingTemplate::new("RestKillVic");
    tmpl.add_kind_of(KindOf::Vehicle);
    let mut o = Object::new(tmpl, ObjectId(41), Team::USA);
    o.kill_when_resting_on_ground = true;
    o.shock_stun_frames = 5;
    o.set_position(glam::Vec3::ZERO);
    o.movement.velocity = glam::Vec3::ZERO;
    assert!(o.maybe_kill_when_resting_on_ground());
    assert!(o.status.destroyed);

    // Drone alive with KINDOF_DRONE does not kill (name substring is not the gate).
    let mut td = ThingTemplate::new("CombatDrone");
    td.add_kind_of(KindOf::Vehicle);
    td.add_kind_of(KindOf::Drone);
    let mut d = Object::new(td, ObjectId(42), Team::USA);
    d.kill_when_resting_on_ground = true;
    d.shock_stun_frames = 5;
    d.set_position(glam::Vec3::ZERO);
    d.movement.velocity = glam::Vec3::ZERO;
    assert!(!d.maybe_kill_when_resting_on_ground());
    assert!(!d.status.destroyed);
    // Unmanned drone does kill.
    d.status.disabled_unmanned = true;
    assert!(d.maybe_kill_when_resting_on_ground());
    assert!(d.status.destroyed);

    // KINDOF_DRONE without "drone" in the name is still spared.
    let mut tu = ThingTemplate::new("AmericaVehicleComanche");
    tu.add_kind_of(KindOf::Drone);
    let mut u = Object::new(tu, ObjectId(44), Team::USA);
    u.kill_when_resting_on_ground = true;
    u.set_position(glam::Vec3::ZERO);
    u.movement.velocity = glam::Vec3::ZERO;
    assert!(!u.maybe_kill_when_resting_on_ground());
    // Name contains "drone" but no KINDOF_DRONE → kill.
    let mut tn = ThingTemplate::new("FakeDroneProp");
    tn.add_kind_of(KindOf::Vehicle);
    let mut n = Object::new(tn, ObjectId(45), Team::USA);
    n.kill_when_resting_on_ground = true;
    n.set_position(glam::Vec3::ZERO);
    n.movement.velocity = glam::Vec3::ZERO;
    assert!(n.maybe_kill_when_resting_on_ground());

    // Bounce land event on airborne ground hit.
    let mut tb = ThingTemplate::new("BounceSnd");
    tb.add_kind_of(KindOf::Vehicle);
    let mut b = Object::new(tb, ObjectId(43), Team::USA);
    b.shock_stun_frames = 30;
    b.shock_allow_bounce = false;
    b.shock_was_airborne = true;
    b.set_position(glam::Vec3::new(0.0, 3.0, 0.0));
    b.movement.velocity = glam::Vec3::new(0.0, -5.0, 0.0);
    b.immune_to_falling_damage = true; // isolate bounce event
    for _ in 0..20 {
        b.tick_shock_stun();
        if b.bounce_land_events > 0 {
            break;
        }
    }
    assert!(
        b.bounce_land_events > 0,
        "landing records bounce sound residual"
    );
    assert!(b.last_bounce_fall_dy > 0.0);
    assert!(b.last_bounce_volume >= 0.25 && b.last_bounce_volume <= 1.0);
    // C++ doBounceSound no-ops unless BounceSound was authored.
    assert_eq!(b.bounce_audio_pending, 0);
    assert!(b.take_bounce_audio_pending().is_none());
    b.set_bounce_sound(BOUNCE_SOUND_DEFAULT);
    b.record_bounce_land(2.0);
    assert!(b.bounce_audio_pending > 0);
    let (name, vol) = b.take_bounce_audio_pending().expect("pending");
    assert_eq!(name, BOUNCE_SOUND_DEFAULT);
    assert!((vol - b.last_bounce_volume).abs() < 1e-5);
    let v_small = bounce_sound_volume_residual(0.05, 1.0);
    let v_big = bounce_sound_volume_residual(0.25, 50.0);
    assert!(v_big >= v_small);

    // Immune falling takes no damage.
    let mut ti = ThingTemplate::new("ImmuneFall");
    ti.add_kind_of(KindOf::Vehicle);
    let mut i = Object::new(ti, ObjectId(44), Team::USA);
    i.health.current = 100.0;
    i.immune_to_falling_damage = true;
    assert_eq!(i.apply_shock_fall_damage(-30.0), 0.0);
    assert_eq!(i.health.current, 100.0);
}

#[test]
fn physics_wave10_held_wreck_friction_stun_shock() {
    use crate::game_logic::{
        KindOf, MIN_NON_AERO_FRICTION_RESIDUAL, Object, ObjectId, Team, ThingTemplate,
    };
    use glam::Vec3;

    // HELD/contained skips Euler.
    let mut th = ThingTemplate::new("HeldInf");
    th.add_kind_of(KindOf::Infantry);
    let mut held = Object::new(th, ObjectId(501), Team::USA);
    held.set_contained_by(Some(ObjectId(99)));
    held.set_position(Vec3::new(0.0, 2.0, 0.0));
    held.movement.velocity = Vec3::new(4.0, -1.0, 0.0);
    let _ = held.tick_physics_motion_step(0.0);
    assert!(
        (held.get_position().x).abs() < 1e-4,
        "HELD must not integrate pos+=vel; x={}",
        held.get_position().x
    );

    // Dead wrecks keep Euler (fall from mid-air).
    let mut tw = ThingTemplate::new("DeadTank");
    tw.add_kind_of(KindOf::Vehicle);
    let mut wreck = Object::new(tw, ObjectId(502), Team::USA);
    wreck.health.current = 0.0;
    wreck.status.effectively_dead = true;
    wreck.set_position(Vec3::new(0.0, 5.0, 0.0));
    wreck.movement.velocity = Vec3::new(0.0, -2.0, 0.0);
    wreck.allow_to_fall = true;
    wreck.immune_to_falling_damage = true;
    let _ = wreck.tick_physics_motion_step(0.0);
    assert!(
        wreck.get_position().y < 5.0,
        "dead wreck must keep Euler; y={}",
        wreck.get_position().y
    );

    // 5cm hop still uses ground friction (not aero=0).
    let mut tf = ThingTemplate::new("Hopper");
    tf.add_kind_of(KindOf::Vehicle);
    let mut hop = Object::new(tf, ObjectId(503), Team::USA);
    hop.set_position(Vec3::new(0.0, 0.08, 0.0));
    hop.ground_height = 0.0;
    hop.status.airborne_target = true;
    hop.movement.velocity = Vec3::new(0.0, 0.0, 10.0);
    hop.physics_mass = 1.0;
    hop.lateral_friction = 0.15;
    hop.apply_frictional_forces();
    hop.integrate_physics_accel();
    assert!(
        hop.movement.velocity.z.abs() < 10.0,
        "5cm hop must scrub with ground friction, vz={}",
        hop.movement.velocity.z
    );

    // MIN_NON_AERO floor 0.01.
    hop.forward_friction = 0.0;
    hop.extra_friction = -1.0;
    assert!((hop.get_forward_friction() - MIN_NON_AERO_FRICTION_RESIDUAL).abs() < 1e-6);
    assert!((hop.get_lateral_friction() - MIN_NON_AERO_FRICTION_RESIDUAL).abs() < 1e-6);
    // Stun relief at 3-frame height (9 with g=-1), not 5cm.
    let mut ts = ThingTemplate::new("Tossed");
    ts.add_kind_of(KindOf::Infantry);
    let mut stun = Object::new(ts, ObjectId(504), Team::USA);
    // C++ relief gate: |vel|<STUN_RELIEF_EPSILON (PhysicsUpdate.cpp:42,674-677)
    // OR !isSignificantlyAboveTerrain (Thing.cpp:308-311, height > 9*|gravity|;
    // retail GameData.ini Gravity=-64/900 per frame² → 0.64wu band).
    stun.shock_stun_frames = 20;
    stun.set_position(Vec3::new(0.0, 4.0, 0.0));
    stun.ground_height = 0.0;
    stun.movement.velocity = Vec3::new(2.0, -1.0, 0.0);
    stun.tick_shock_stun();
    assert_eq!(
        stun.shock_stun_frames, 20,
        "height 4 (2.93 after tick) is significantly airborne: stun persists"
    );
    // Just under the 0.64wu 3-frame band (not the old 5cm rule): relief clears.
    stun.set_position(Vec3::new(0.0, 0.5, 0.0));
    stun.movement.velocity = Vec3::new(2.0, -1.0, 0.0);
    stun.tick_shock_stun();
    assert_eq!(
        stun.shock_stun_frames, 0,
        "under the 3-frame height band relief clears"
    );

    // Shock toss has no invented 80 cap.
    let mut tk = ThingTemplate::new("MoabVic");
    tk.add_kind_of(KindOf::Vehicle);
    let mut tossed = Object::new(tk, ObjectId(505), Team::USA);
    tossed.physics_mass = 1.0;
    tossed.shock_resistance = 0.0;
    tossed.status.airborne_target = false;
    let applied = tossed.apply_shock_wave_impulse(Vec3::new(200.0, 20.0, 0.0));
    assert!(applied);
    assert!(
        tossed.movement.velocity.length() > 80.0,
        "shock must not cap |v| at 80; |v|={}",
        tossed.movement.velocity.length()
    );

    // Non-stun landing records bounce + pending ground collide.
    let mut tl = ThingTemplate::new("Lander");
    tl.add_kind_of(KindOf::Vehicle);
    let mut land = Object::new(tl, ObjectId(506), Team::USA);
    land.set_position(Vec3::new(0.0, 3.0, 0.0));
    land.movement.velocity = Vec3::new(0.0, -4.0, 0.0);
    land.health.current = 10_000.0;
    land.was_airborne_last_frame = true;
    land.immune_to_falling_damage = false;
    let _ = land.tick_physics_motion_step(0.0);
    assert!(land.bounce_land_events > 0);
    assert_eq!(land.bounce_audio_pending, 0);
    assert!(land.pending_ground_collide);
}

#[test]
fn tick_physics_motion_step_destroys_nan_position() {
    use glam::Vec3;
    let mut tmpl = ThingTemplate::new("NanWreck");
    tmpl.add_kind_of(KindOf::Vehicle);
    let mut o = Object::new(tmpl, ObjectId(9201), Team::USA);
    o.set_position(Vec3::new(3.0, 4.0, 5.0));
    o.ground_height = 0.0;
    o.allow_to_fall = true;
    o.movement.target_position = None;
    o.movement.path.clear();
    o.movement.velocity = Vec3::new(f32::NAN, 0.0, 0.0);
    let _ = o.tick_physics_motion_step(0.0);
    assert!(o.status.destroyed, "NaN translation must destroyObject");
    let p = o.get_position();
    assert!(
        p.x.is_finite() && p.y.is_finite() && p.z.is_finite(),
        "must not write NaN pose; pos={p:?}"
    );
}

#[test]
fn airborne_target_uses_airborne_targeting_height_not_5cm() {
    let mut tmpl = ThingTemplate::new("TossedTank");
    tmpl.add_kind_of(KindOf::Vehicle);
    let mut tank = Object::new(tmpl, ObjectId(9101), Team::USA);
    tank.set_position(Vec3::new(0.0, 10.0, 0.0));
    tank.ground_height = 0.0;
    tank.status.airborne_target = true;
    tank.loco_appearance = LocomotorAppearance::Hover;
    tank.stamp_airborne_target_from_locomotor();
    assert!(
        !tank.status.airborne_target,
        "default INT_MAX must not flag tossed tanks as AA victims"
    );

    tank.set_position(Vec3::new(0.0, 1.0, 0.0));
    tank.movement.target_position = Some(Vec3::new(10.0, 1.0, 0.0));
    tank.movement.velocity = Vec3::ZERO;
    tank.was_airborne_last_frame = false;
    let _ = tank.tick_physics_motion_step(0.0);
    assert!(
        !tank.status.airborne_target,
        "physics 5cm airborne must not set AIRBORNE_TARGET"
    );
    assert!(
        tank.was_airborne_last_frame,
        "1m above terrain is still physically airborne"
    );

    let mut air_t = ThingTemplate::new("AirLocoProbe");
    air_t.add_kind_of(KindOf::Aircraft);
    let mut air = Object::new(air_t, ObjectId(9102), Team::USA);
    air.airborne_targeting_height = 30;
    air.ground_height = 0.0;
    air.set_position(Vec3::new(0.0, 30.0, 0.0));
    air.stamp_airborne_target_from_locomotor();
    assert!(
        !air.status.airborne_target,
        "C++ AIUpdate uses strictly greater than AirborneTargetingHeight"
    );
    air.set_position(Vec3::new(0.0, 31.0, 0.0));
    air.stamp_airborne_target_from_locomotor();
    assert!(air.status.airborne_target);
}

#[test]
fn stunned_off_map_cliff_water_kills_without_loco() {
    use crate::game_logic::host_deliver_payload::{
        RESIDUAL_MAP_EXTENT_MAX_X, is_off_map_default_residual,
    };
    let mut tmpl = ThingTemplate::new("GroundTank");
    tmpl.add_kind_of(KindOf::Vehicle);
    let mut o = Object::new(tmpl, ObjectId(31), Team::USA);
    o.shock_stun_frames = 30;
    o.ensure_locomotor_surfaces();
    assert!(o.has_locomotor_for_surface(LOCO_SURFACE_GROUND));
    assert!(!o.has_locomotor_for_surface(LOCO_SURFACE_CLIFF));
    assert!(!o.has_locomotor_for_surface(LOCO_SURFACE_WATER));
    o.set_position(glam::Vec3::new(RESIDUAL_MAP_EXTENT_MAX_X + 50.0, 0.0, 0.0));
    assert!(is_off_map_default_residual(o.get_position()));
    assert!(o.test_stunned_unit_for_destruction());
    assert!(o.status.destroyed);
    assert_eq!(
        o.status.death_type,
        crate::game_logic::host_usa_pilot::HostDeathType::Normal
    );
    assert!(o.health.current <= 0.0);

    let mut t2 = ThingTemplate::new("CliffVictim");
    t2.add_kind_of(KindOf::Infantry);
    let mut c = Object::new(t2, ObjectId(32), Team::USA);
    c.shock_stun_frames = 20;
    c.cell_is_cliff = true;
    c.set_position(glam::Vec3::ZERO);
    assert!(c.test_stunned_unit_for_destruction());
    assert!(c.status.destroyed);

    let mut t3 = ThingTemplate::new("WaterVictim");
    t3.add_kind_of(KindOf::Vehicle);
    let mut w = Object::new(t3, ObjectId(33), Team::USA);
    w.shock_stun_frames = 20;
    w.cell_is_underwater = true;
    w.set_position(glam::Vec3::ZERO);
    assert!(w.test_stunned_unit_for_destruction());
    assert!(w.status.destroyed);

    let mut th = ThingTemplate::new("AmphibHover");
    th.add_kind_of(KindOf::Vehicle);
    let mut h = Object::new(th, ObjectId(34), Team::USA);
    h.shock_stun_frames = 20;
    h.locomotor_surfaces = LOCO_SURFACE_GROUND | LOCO_SURFACE_WATER;
    h.cell_is_underwater = true;
    h.set_position(glam::Vec3::ZERO);
    assert!(!h.test_stunned_unit_for_destruction());
    assert!(!h.status.destroyed);
    h.cell_is_underwater = false;
    h.cell_is_cliff = true;
    h.locomotor_surfaces |= LOCO_SURFACE_CLIFF;
    assert!(!h.test_stunned_unit_for_destruction());
}

#[test]
fn stunned_ai_less_debris_keeps_tumbling_on_cliff_water() {
    use crate::game_logic::host_deliver_payload::RESIDUAL_MAP_EXTENT_MAX_X;

    let mut debris_t = ThingTemplate::new("GenericDebris");
    let mut debris = Object::new(debris_t, ObjectId(351), Team::Neutral);
    debris.shock_stun_frames = 20;
    debris.cell_is_cliff = true;
    debris.set_position(glam::Vec3::ZERO);
    assert!(!debris.has_ai_update_interface());
    assert!(!debris.test_stunned_unit_for_destruction());
    assert!(!debris.status.destroyed);

    debris.cell_is_cliff = false;
    debris.cell_is_underwater = true;
    assert!(!debris.test_stunned_unit_for_destruction());
    assert!(!debris.status.destroyed);

    let mut crate_t = ThingTemplate::new("SalvageCrate");
    crate_t.add_kind_of(KindOf::Crate);
    let mut crate_obj = Object::new(crate_t, ObjectId(352), Team::Neutral);
    crate_obj.shock_stun_frames = 20;
    crate_obj.cell_is_cliff = true;
    crate_obj.cell_is_underwater = true;
    crate_obj.set_position(glam::Vec3::ZERO);
    assert!(!crate_obj.has_ai_update_interface());
    assert!(!crate_obj.test_stunned_unit_for_destruction());
    assert!(!crate_obj.status.destroyed);

    // C++ still kills AI-less stunned debris when upside-down or off-map.
    debris.cell_is_underwater = false;
    debris.apply_physics_ypr(0.0, 0.0, std::f32::consts::PI);
    assert!(debris.test_stunned_unit_for_destruction());
    assert!(debris.status.destroyed);

    let mut off = Object::new(
        ThingTemplate::new("GenericDebris"),
        ObjectId(353),
        Team::Neutral,
    );
    off.shock_stun_frames = 20;
    off.set_position(glam::Vec3::new(RESIDUAL_MAP_EXTENT_MAX_X + 50.0, 0.0, 0.0));
    assert!(off.test_stunned_unit_for_destruction());
    assert!(off.status.destroyed);
}

#[test]
fn stunned_center_of_mass_offset_scales_pitch() {
    let mut tmpl = ThingTemplate::new("ComTruck");
    tmpl.add_kind_of(KindOf::Vehicle);
    let mut o = Object::new(tmpl, ObjectId(35), Team::USA);
    o.center_of_mass_offset = 2.0;
    o.pitch_roll_yaw_factor = 1.0;
    o.shock_stun_frames = 20;
    o.shock_pitch_rate = 0.2;
    o.shock_yaw_rate = 0.0;
    o.shock_roll_rate = 0.0;
    o.set_position(glam::Vec3::new(0.0, 2.0, 0.0));
    o.ground_height = 0.0;
    let mut raw = Object::new(ThingTemplate::new("ComRaw"), ObjectId(36), Team::USA);
    raw.center_of_mass_offset = 0.0;
    raw.pitch_roll_yaw_factor = 1.0;
    raw.shock_stun_frames = 20;
    raw.shock_pitch_rate = 0.2;
    raw.shock_yaw_rate = 0.0;
    raw.shock_roll_rate = 0.0;
    raw.set_position(glam::Vec3::new(0.0, 2.0, 0.0));
    raw.ground_height = 0.0;
    // Nose-up so remaining = π/2 - π/4 = π/4, sin < 1.
    o.apply_physics_ypr(0.0, std::f32::consts::FRAC_PI_4, 0.0);
    raw.apply_physics_ypr(0.0, std::f32::consts::FRAC_PI_4, 0.0);
    let pitch = |m: glam::Mat4| {
        let f = m.x_axis;
        f.y.atan2((f.x * f.x + f.z * f.z).sqrt())
    };
    let o0 = pitch(o.get_transform_matrix());
    let r0 = pitch(raw.get_transform_matrix());
    o.tick_shock_stun();
    raw.tick_shock_stun();
    let o_dpitch = (pitch(o.get_transform_matrix()) - o0).abs();
    let r_dpitch = (pitch(raw.get_transform_matrix()) - r0).abs();
    assert!(
        o_dpitch + 1e-5 < r_dpitch,
        "stunned COM offset must damp pitch vs raw rate ({o_dpitch} vs {r_dpitch})"
    );
}

#[test]
fn motion_step_bounce_rights_tilted_not_just_flipped() {
    let mut tmpl = ThingTemplate::new("TiltWreck");
    tmpl.add_kind_of(KindOf::Vehicle);
    let mut o = Object::new(tmpl, ObjectId(24), Team::USA);
    o.set_position(glam::Vec3::new(0.0, 2.0, 0.0));
    o.apply_physics_ypr(0.0, std::f32::consts::FRAC_PI_4, 0.0);
    let up_before = o.physics_transform_up_y();
    assert!(
        up_before > 0.0 && up_before < 0.99,
        "tilted but not flipped: {up_before}"
    );
    o.shock_allow_bounce = true;
    o.original_allow_bounce = false;
    o.movement.velocity = glam::Vec3::new(0.0, -4.0, 0.0);
    o.immune_to_falling_damage = true;
    let force = o.compute_ground_bounce_force(2.0, -0.1, 0.0);
    assert!(force.is_some(), "vy<0 ground contact must bounce");
    assert!(
        o.physics_transform_up_y() > 0.99,
        "tilted wreck must right on bounce, up={}",
        o.physics_transform_up_y()
    );

    // Flip is preserved through handleBounce so stun-kill still sees up<0.
    let mut ftmpl = ThingTemplate::new("FlipKeep");
    ftmpl.add_kind_of(KindOf::Vehicle);
    let mut f = Object::new(ftmpl, ObjectId(25), Team::USA);
    f.set_position(glam::Vec3::new(0.0, 2.0, 0.0));
    f.apply_physics_ypr(0.0, 0.0, std::f32::consts::PI);
    assert!(f.physics_transform_up_y() < 0.0);
    f.shock_allow_bounce = true;
    f.movement.velocity = glam::Vec3::new(0.0, -4.0, 0.0);
    assert!(f.compute_ground_bounce_force(2.0, -0.1, 0.0).is_some());
    assert!(
        f.physics_transform_up_y() < 0.0,
        "flipped pose must survive first-righting for stun kill"
    );
}

#[test]
fn motion_step_bounce_keeps_inverted_roll_for_stun_kill() {
    // hq-p6amn: leftover handle_bounce 0-or-PI must survive the live Euler.
    // Inverted stunned wrecks die; inverted non-stunned poses stay flipped.
    let mut st = ThingTemplate::new("StunFlipStep");
    st.add_kind_of(KindOf::Vehicle);
    st.max_health = 100.0;
    let mut stunned = Object::new(st, ObjectId(26), Team::USA);
    stunned.health.current = 100.0;
    stunned.set_position(glam::Vec3::new(0.0, 2.0, 0.0));
    stunned.apply_physics_ypr(0.0, 0.0, std::f32::consts::PI);
    assert!(stunned.physics_transform_up_y() < 0.0);
    stunned.shock_allow_bounce = true;
    stunned.shock_stun_frames = 40;
    stunned.movement.velocity = glam::Vec3::new(0.0, -4.0, 0.0);
    stunned.immune_to_falling_damage = true;
    let _ = stunned.tick_physics_motion_step(0.0);
    assert!(
        stunned.status.destroyed,
        "hq-p6amn: inverted stunned must die on motion-step bounce"
    );

    let mut wt = ThingTemplate::new("WreckFlipStep");
    wt.add_kind_of(KindOf::Vehicle);
    let mut wreck = Object::new(wt, ObjectId(27), Team::USA);
    wreck.set_position(glam::Vec3::new(0.0, 2.0, 0.0));
    wreck.apply_physics_ypr(0.0, 0.0, std::f32::consts::PI);
    wreck.shock_allow_bounce = true;
    wreck.movement.velocity = glam::Vec3::new(0.0, -4.0, 0.0);
    wreck.immune_to_falling_damage = true;
    wreck.kill_when_resting_on_ground = false;
    let bounced = wreck.tick_physics_motion_step(0.0);
    assert!(bounced, "hq-p6amn: inverted wreck must bounce");
    assert!(
        wreck.physics_transform_up_y() < 0.0,
        "hq-p6amn: inverted wreck pose must survive motion-step, up={}",
        wreck.physics_transform_up_y()
    );
    assert!(!wreck.status.destroyed);
}

#[test]
fn shock_bounce_keep_flip_before_stun_test() {
    // hq-p6amn: handle_shock_ground_bounce must 0-or-PI right before
    // testStunned so a flip discretized from up-Y still kills.
    let mut tmpl = ThingTemplate::new("ShockFlip");
    tmpl.add_kind_of(KindOf::Vehicle);
    tmpl.max_health = 100.0;
    let mut o = Object::new(tmpl, ObjectId(28), Team::USA);
    o.health.current = 100.0;
    o.set_position(glam::Vec3::new(0.0, 2.0, 0.0));
    o.apply_physics_ypr(0.0, 0.0, std::f32::consts::PI);
    assert!(o.physics_transform_up_y() < 0.0);
    o.shock_allow_bounce = true;
    o.shock_stun_frames = 40;
    o.movement.velocity = glam::Vec3::new(0.0, -4.0, 0.0);
    let bounced = o.handle_shock_ground_bounce(2.0, -0.1, 0.0);
    assert!(
        o.status.destroyed,
        "inverted stunned must die after keep_flip"
    );
    assert_eq!(bounced, 0.0);

    let mut wtmpl = ThingTemplate::new("ShockWreckFlip");
    wtmpl.add_kind_of(KindOf::Vehicle);
    let mut w = Object::new(wtmpl, ObjectId(29), Team::USA);
    w.set_position(glam::Vec3::new(0.0, 2.0, 0.0));
    w.apply_physics_ypr(0.0, 0.0, std::f32::consts::PI);
    w.shock_allow_bounce = true;
    w.movement.velocity = glam::Vec3::new(0.0, -4.0, 0.0);
    assert!(w.handle_shock_ground_bounce(2.0, -0.1, 0.0) > 0.0);
    assert!(
        w.physics_transform_up_y() < 0.0,
        "hq-p6amn: shock bounce must keep inverted roll, up={}",
        w.physics_transform_up_y()
    );
}

#[test]
fn stunned_upside_down_bounce_kills_and_freefall_disables() {
    let mut tmpl = ThingTemplate::new("StunKill");
    tmpl.add_kind_of(KindOf::Vehicle);
    tmpl.max_health = 100.0;
    let mut o = Object::new(tmpl, ObjectId(21), Team::USA);
    o.health.current = 100.0;
    assert!(o.apply_shock_wave_impulse(glam::Vec3::new(5.0, 30.0, 0.0)));
    o.shock_allow_bounce = true;
    o.shock_stun_frames = 40;
    // Simulate bounce path with downward impact from above ground.
    o.set_position(glam::Vec3::new(0.0, 2.0, 0.0));
    // Invert integrated pose (C++ Get_Z_Vector().Z < 0) after set_position.
    o.apply_physics_ypr(0.0, 0.0, std::f32::consts::PI);
    o.movement.velocity = glam::Vec3::new(0.0, -4.0, 0.0);
    let bounced = o.handle_shock_ground_bounce(2.0, -0.1, 0.0);
    assert!(o.status.destroyed, "upside-down stunned must die on bounce");
    assert_eq!(bounced, 0.0);
    // Freefall disable residual while airborne.
    let mut t2 = ThingTemplate::new("FreeFallDis");
    t2.add_kind_of(KindOf::Vehicle);
    let mut a = Object::new(t2, ObjectId(22), Team::USA);
    assert!(a.apply_shock_wave_impulse(glam::Vec3::new(0.0, 50.0, 0.0)));
    a.set_position(glam::Vec3::ZERO);
    // Climb a few frames.
    for _ in 0..5 {
        if a.get_position().y > 0.2 {
            break;
        }
        a.tick_shock_stun();
    }
    if a.get_position().y > 0.05 {
        assert!(a.status.disabled_freefall || a.is_disabled());
        assert!(a.is_freefall_disabled() || a.is_disabled());
    }
    // Land fully.
    for _ in 0..80 {
        a.tick_shock_stun();
        if a.shock_stun_frames == 0 && a.get_position().y <= 0.01 {
            break;
        }
    }
    if a.get_position().y <= 0.01 && !a.status.destroyed {
        assert!(
            !a.status.disabled_freefall,
            "grounded clears DISABLED_FREEFALL"
        );
    }
}

#[test]
fn shock_fall_damage_splats_on_hard_landing() {
    use crate::game_logic::combat::DamageType;
    use crate::game_logic::host_enum_table_residual::{MC_BIT_SPLATTED, host_model_condition_has};
    use crate::game_logic::host_usa_pilot::HostDeathType;
    // leftover height_to_speed(40) with retail |g|=64/900 → ~2.385, not sqrt(80).
    let leftover_min = Object::min_fall_speed_for_damage();
    assert!((leftover_min - Object::height_to_fall_speed(40.0)).abs() < 1e-3);
    assert!(leftover_min > 2.0 && leftover_min < 3.0);
    assert!((leftover_min - (80.0f32).sqrt()).abs() > 1.0);
    let mut tmpl = ThingTemplate::new("SplatVic");
    tmpl.add_kind_of(KindOf::Vehicle);
    tmpl.max_health = 50.0;
    let mut o = Object::new(tmpl, ObjectId(11), Team::USA);
    o.health.current = 50.0;
    o.health.maximum = 50.0;
    o.set_position(glam::Vec3::new(0.0, 5.0, 0.0));
    o.shock_was_airborne = true;
    o.shock_allow_bounce = false;
    o.shock_stun_frames = 20;
    // Hard downward impact residual (steep fall, no lateral).
    o.movement.velocity = glam::Vec3::new(0.0, -20.0, 0.0);
    let dmg = o.apply_shock_fall_damage(-20.0);
    assert!(dmg > 0.0, "expected fall damage, got {dmg}");
    // net = 20 - leftover ~2.385 ≈ 17.6 → wounds 50hp unit with mass1 factor1
    assert!(o.health.current < 50.0);
    // Stronger impact to splat.
    o.health.current = 5.0;
    o.status.destroyed = false;
    let dmg2 = o.apply_shock_fall_damage(-30.0);
    assert!(dmg2 > 5.0);
    assert!(o.status.destroyed || o.health.current <= 0.0);
    if o.status.destroyed {
        assert_eq!(o.status.death_type, HostDeathType::Splatted);
        assert!(host_model_condition_has(
            o.model_condition_bits,
            MC_BIT_SPLATTED
        ));
    }
    // Shallow slope residual: large lateral vs vertical → no damage.
    let mut s = Object::new(
        {
            let mut t = ThingTemplate::new("SlopeVic");
            t.add_kind_of(KindOf::Vehicle);
            t
        },
        ObjectId(12),
        Team::USA,
    );
    s.health.current = 100.0;
    s.movement.velocity = glam::Vec3::new(50.0, -5.0, 0.0);
    let d0 = s.apply_shock_fall_damage(-5.0);
    assert_eq!(d0, 0.0, "below min fall speed");
    // Above min speed but shallow angle.
    let d1 = s.apply_shock_fall_damage(-20.0);
    // |20/50|=0.4 < 3 → not steep
    assert_eq!(d1, 0.0, "shallow fall must not damage");
    let _ = DamageType::Falling;
}

#[test]
fn shock_bounce_settles_freefall_and_switches_to_stunned() {
    use crate::game_logic::host_enum_table_residual::{
        MC_BIT_FREEFALL, MC_BIT_STUNNED, MC_BIT_STUNNED_FLAILING, host_model_condition_has,
    };
    let mut tmpl = ThingTemplate::new("BounceVic");
    tmpl.add_kind_of(KindOf::Vehicle);
    let mut o = Object::new(tmpl, ObjectId(9), Team::USA);
    o.set_position(glam::Vec3::new(0.0, 0.0, 0.0));
    // Short, near-vertical toss. A tall 40vy arc carries the hull far outside
    // the default map extent / upside-down by the first bounce, so C++
    // testStunnedUnitForDestruction (PhysicsUpdate.cpp:505-517) kills it before
    // the grounded STUNNED flip can be observed. Keep the arc low and lateral
    // drift tiny so the first ground hit lands a living, stunned unit.
    assert!(o.apply_shock_wave_impulse(glam::Vec3::new(0.5, 6.0, 0.0)));
    assert!(o.shock_allow_bounce);
    // Climb while velocity positive.
    let mut saw_air = false;
    let mut saw_bounce = false;
    let mut max_y = 0.0f32;
    let mut saw_stunned_after_ground = false;
    // Live frame order (world_tick/step.rs:802 tick_shock_stun_all +
    // tick_physics_collisions_all): shock tick runs while stunned, motion step
    // runs for every physics-active body. The motion step owns gravity once
    // relief clears mid-air (its gravity gate is frames==0) and clamps the
    // landing + first-ground-hit latch (PhysicsUpdate.cpp:765); driving
    // tick_shock_stun alone freezes the fall at the relief band.
    for i in 0..2000 {
        if o.shock_stun_frames > 0 || o.bounce_audio_pending > 0 {
            o.tick_shock_stun();
        }
        let _ = o.tick_physics_motion_step(0.0);
        let y = o.get_position().y;
        max_y = max_y.max(y);
        if y > 0.5 {
            saw_air = true;
        }
        if o.shock_grounded_once {
            saw_bounce = true;
            // While still stunned after first ground hit: STUNNED, not FLAILING.
            if o.shock_stun_frames > 0 {
                assert!(
                    host_model_condition_has(o.model_condition_bits, MC_BIT_STUNNED),
                    "frames={} bits={:#x}",
                    o.shock_stun_frames,
                    o.model_condition_bits
                );
                assert!(!host_model_condition_has(
                    o.model_condition_bits,
                    MC_BIT_STUNNED_FLAILING
                ));
                saw_stunned_after_ground = true;
            }
        }
        if o.shock_stun_frames == 0 && o.get_position().y <= 0.01 {
            break;
        }
    }
    assert!(saw_air || max_y > 0.0, "shock lift should leave ground");
    assert!(saw_bounce || o.shock_grounded_once, "must hit ground");
    assert!(
        saw_stunned_after_ground,
        "must observe STUNNED bit after ground while stun active"
    );
    // Settled: no freefall bit when grounded.
    if o.get_position().y <= 0.01 && o.movement.velocity.y.abs() < 0.5 {
        assert!(!host_model_condition_has(
            o.model_condition_bits,
            MC_BIT_FREEFALL
        ));
    }
    assert!(o.get_position().y >= -0.01, "must not sink below ground");
}

#[test]
fn shock_applies_random_rotation_and_optional_freefall_bit() {
    use crate::game_logic::host_enum_table_residual::{MC_BIT_FREEFALL, host_model_condition_has};
    let mut tmpl = ThingTemplate::new("RotVic");
    tmpl.add_kind_of(KindOf::Vehicle);
    let mut o = Object::new(tmpl, ObjectId(7), Team::USA);
    let ori0 = o.get_orientation();
    o.shock_yaw_rate = 0.0;
    assert!(o.apply_shock_wave_impulse(glam::Vec3::new(30.0, 20.0, 10.0)));
    // Random rotation residual should change rates and/or orientation.
    let rotated = (o.get_orientation() - ori0).abs() > 1e-6
        || o.shock_yaw_rate.abs() > 1e-6
        || o.shock_pitch_rate.abs() > 1e-6;
    assert!(rotated, "shock applies rotation residual");
    // Strong up velocity may set FREEFALL while stunned.
    if o.movement.velocity.y > 8.0 {
        assert!(host_model_condition_has(
            o.model_condition_bits,
            MC_BIT_FREEFALL
        ));
    }
    // C++ applyRandomRotation: only STICK_TO_GROUND skips tumble.
    let mut st = ThingTemplate::new("RotStruct");
    st.add_kind_of(KindOf::Structure);
    let mut s = Object::new(st, ObjectId(8), Team::USA);
    let s0 = s.get_orientation();
    s.apply_shock_random_rotation(123);
    let struct_tumbled = (s.get_orientation() - s0).abs() > 1e-6
        || s.shock_yaw_rate.abs() > 1e-6
        || s.shock_pitch_rate.abs() > 1e-6;
    assert!(struct_tumbled, "structure without stick must tumble");
    assert!(
        s.apply_shock_wave_impulse(glam::Vec3::new(4.0, 8.0, 0.0)),
        "structure with PhysicsBehavior still takes shock"
    );

    let mut it = ThingTemplate::new("RotInf");
    it.add_kind_of(KindOf::Infantry);
    let mut inf = Object::new(it, ObjectId(81), Team::USA);
    inf.stick_to_ground = false;
    let i0 = inf.get_orientation();
    inf.apply_shock_random_rotation(123);
    let inf_tumbled = (inf.get_orientation() - i0).abs() > 1e-6
        || inf.shock_yaw_rate.abs() > 1e-6
        || inf.shock_pitch_rate.abs() > 1e-6;
    assert!(inf_tumbled, "infantry without stick must tumble");

    let mut stuck_t = ThingTemplate::new("RotStuck");
    stuck_t.add_kind_of(KindOf::Infantry);
    let mut stuck = Object::new(stuck_t, ObjectId(82), Team::USA);
    stuck.stick_to_ground = true;
    let st0 = stuck.get_orientation();
    stuck.apply_shock_random_rotation(123);
    assert!((stuck.get_orientation() - st0).abs() < 1e-6);
    assert_eq!(stuck.shock_yaw_rate, 0.0);
}

#[test]
fn handle_shock_ground_bounce_restores_original_allow_bounce() {
    let mut tmpl = ThingTemplate::new("StunBounceAllow");
    tmpl.add_kind_of(KindOf::Vehicle);
    let mut o = Object::new(tmpl, ObjectId(91), Team::USA);
    o.shock_allow_bounce = true;
    o.original_allow_bounce = true;
    o.movement.velocity = glam::Vec3::ZERO;
    let bounced = o.handle_shock_ground_bounce(0.0, -0.1, 0.0);
    assert_eq!(bounced, 0.0);
    assert!(
        o.shock_allow_bounce,
        "zero-force stun bounce must restore authored AllowBouncing"
    );

    let mut authored_off = Object::new(
        {
            let mut t = ThingTemplate::new("StunBounceOff");
            t.add_kind_of(KindOf::Vehicle);
            t
        },
        ObjectId(92),
        Team::USA,
    );
    authored_off.shock_allow_bounce = true;
    authored_off.original_allow_bounce = false;
    authored_off.movement.velocity = glam::Vec3::ZERO;
    let bounced_off = authored_off.handle_shock_ground_bounce(0.0, -0.1, 0.0);
    assert_eq!(bounced_off, 0.0);
    assert!(
        !authored_off.shock_allow_bounce,
        "zero-force stun bounce must restore authored AllowBouncing=No"
    );
}

#[test]
fn bounce_damps_pitch_roll_rates_not_zero() {
    let mut tmpl = ThingTemplate::new("BounceDamp");
    tmpl.add_kind_of(KindOf::Vehicle);
    let mut o = Object::new(tmpl, ObjectId(93), Team::USA);
    o.set_position(glam::Vec3::new(0.0, 2.0, 0.0));
    o.shock_allow_bounce = true;
    o.shock_yaw_rate = 1.0;
    o.shock_pitch_rate = 1.0;
    o.shock_roll_rate = 1.0;
    o.movement.velocity = glam::Vec3::new(0.0, -4.0, 0.0);
    o.immune_to_falling_damage = true;
    assert!(o.compute_ground_bounce_force(2.0, -0.1, 0.0).is_some());
    assert!((o.shock_yaw_rate - 0.7).abs() < 1e-5);
    assert!((o.shock_pitch_rate - 0.7).abs() < 1e-5);
    assert!((o.shock_roll_rate - 0.7).abs() < 1e-5);

    let mut s = Object::new(
        {
            let mut t = ThingTemplate::new("StunDamp");
            t.add_kind_of(KindOf::Vehicle);
            t
        },
        ObjectId(94),
        Team::USA,
    );
    s.set_position(glam::Vec3::new(0.0, 2.0, 0.0));
    s.shock_allow_bounce = true;
    s.shock_yaw_rate = 1.0;
    s.shock_pitch_rate = 1.0;
    s.shock_roll_rate = 1.0;
    s.movement.velocity = glam::Vec3::new(0.0, -4.0, 0.0);
    s.immune_to_falling_damage = true;
    assert!(s.handle_shock_ground_bounce(2.0, -0.1, 0.0) > 0.0);
    assert!((s.shock_yaw_rate - 0.7).abs() < 1e-5);
    assert!((s.shock_pitch_rate - 0.7).abs() < 1e-5);
    assert!((s.shock_roll_rate - 0.7).abs() < 1e-5);
}

#[test]
fn shock_stun_blocks_attack_fire_and_flail_move() {
    let mut tmpl = ThingTemplate::new("StunBlock");
    tmpl.add_kind_of(KindOf::Vehicle);
    let mut o = Object::new(tmpl, ObjectId(42), Team::USA);
    o.weapon = Some(Weapon {
        damage: 10.0,
        range: 100.0,
        reload_time: 0.0,
        last_fire_time: -100.0,
        can_target_ground: true,
        ..Weapon::default()
    });
    assert!(o.can_attack());
    assert!(o.can_fire(0.0));
    assert!(o.can_move());
    assert!(o.apply_shock_wave_impulse(glam::Vec3::new(10.0, 5.0, 0.0)));
    assert!(o.is_shock_stunned());
    assert!(!o.can_attack(), "stunned cannot attack");
    assert!(!o.can_fire(0.0), "stunned cannot fire");
    // Flailing phase blocks commanded move.
    assert!(o.shock_stun_frames > 15);
    assert!(!o.can_move(), "flailing cannot take move orders");
    // Settled stunned phase: move orders allowed (stagger), still no fire.
    o.shock_stun_frames = 10;
    o.refresh_model_condition_bits();
    assert!(!o.can_attack());
    assert!(!o.can_fire(1.0));
    assert!(o.can_move(), "settled stun may stagger-move");
    // attack_target ignored while stunned.
    o.shock_stun_frames = 20;
    o.attack_target(ObjectId(99));
    assert!(o.target.is_none() || o.ai_state != AIState::Attacking || !o.can_attack());
    // After stun clears, combat again.
    o.shock_stun_frames = 0;
    o.refresh_model_condition_bits();
    assert!(o.can_attack());
    assert!(o.can_fire(2.0));
    assert!(o.can_move());
}

#[test]
fn jet_stop_idle_timer_sneaky_and_lockon() {
    use crate::game_logic::object::{
        HostJetPendingResume, JetAiTickAction, STEALTH_FIGHTER_LOCKON_TIME_FRAMES,
    };
    use crate::game_logic::{KindOf, Team, ThingTemplate};
    use glam::Vec3;

    let mut aurora_t = ThingTemplate::new("AmericaJetAurora");
    aurora_t.add_kind_of(KindOf::Aircraft);
    let mut aurora = Object::new(aurora_t, ObjectId(1), Team::USA);
    aurora.set_position(Vec3::new(0.0, 50.0, 0.0));
    aurora.status.airborne_target = true;
    aurora.set_ai_state(AIState::Idle);
    aurora.status.attacking = true;
    let persist = aurora.tick_jet_ai_update(10);
    assert_eq!(persist, JetAiTickAction::None);
    aurora.status.attacking = false;
    aurora.set_ai_state(AIState::Idle);
    let _ = aurora.tick_jet_ai_update(11);
    assert_eq!(
        aurora.jet_ai.return_to_base_frame,
        11 + crate::game_logic::host_aurora_bomb::AURORA_JET_RETURN_TO_BASE_IDLE_FRAMES
    );
    aurora.jet_ai.return_to_base_frame = 12;
    assert_eq!(aurora.tick_jet_ai_update(12), JetAiTickAction::ReturnToBase);
    aurora.notify_jet_victim_is_dead(40);
    assert_eq!(aurora.jet_ai.return_to_base_frame, 40);

    aurora.status.attacking = true;
    let _ = aurora.tick_jet_ai_update(50);
    let off = aurora.get_sneaky_targeting_offset(50).expect("sneaky");
    assert!((off.length() - 20.0).abs() < 0.01);
    assert_eq!(
        aurora.jet_ai.cur_locomotor_set.as_deref(),
        Some("SET_SUPERSONIC")
    );
    assert!(
        crate::game_logic::host_countermeasures::victim_locomotor_is_supersonic(
            aurora.get_cur_locomotor_set_token()
        )
    );
    assert!(
        aurora.movement.max_speed > 200.0,
        "SET_SUPERSONIC must dash faster than cruise, got {}",
        aurora.movement.max_speed
    );

    aurora.status.attacking = false;
    let persist_frames = aurora.jet_attack_loco_persist_frames();
    let _ = aurora.tick_jet_ai_update(50 + persist_frames);
    assert_eq!(aurora.jet_ai.attack_loco_expire_frame, 0);
    assert_eq!(
        aurora.jet_ai.cur_locomotor_set.as_deref(),
        Some("SET_NORMAL")
    );
    aurora.status.attacking = true;
    let _ = aurora.tick_jet_ai_update(50);

    let mut sf_t = ThingTemplate::new("AmericaJetStealthFighter");
    sf_t.add_kind_of(KindOf::Aircraft);
    let mut sf = Object::new(sf_t, ObjectId(2), Team::USA);
    sf.set_position(Vec3::new(10.0, 40.0, 0.0));
    sf.add_jet_targeter(ObjectId(9), true, 100);
    assert!(sf.is_temporarily_preventing_aim_success(100));
    assert!(!sf.is_temporarily_preventing_aim_success(100 + STEALTH_FIGHTER_LOCKON_TIME_FRAMES));
    let _ = sf.tick_jet_ai_update(101);
    assert!(sf.jet_ai.lockon_pos.is_some());

    let mut raptor_t = ThingTemplate::new("AmericaJetRaptor");
    raptor_t.add_kind_of(KindOf::Aircraft);
    raptor_t.primary_weapon_name = Some("HostTestRaptorJetMissileWeapon".into());
    let mut raptor = Object::new(raptor_t, ObjectId(3), Team::USA);
    raptor.status.airborne_target = true;
    raptor.set_ai_state(AIState::GuardingArea);
    raptor.mark_jet_command_for_reload_interrupt(true);
    raptor.weapon = Some(crate::game_logic::Weapon {
        ammo: Some(0),
        clip_size: 2,
        ..crate::game_logic::Weapon::default()
    });

    assert!(raptor.needs_return_to_base_rearm());
    assert!(raptor.jet_empty_clip_should_auto_rtb());
    assert_eq!(raptor.tick_jet_ai_update(3), JetAiTickAction::ReturnToBase);
    assert!(raptor.jet_ai.has_pending_command);
    assert_eq!(
        raptor.jet_ai.pending_resume,
        HostJetPendingResume::GuardArea
    );

    raptor.set_ai_state(AIState::Attacking);
    raptor.target = Some(ObjectId(9));
    raptor.jet_ai.allow_interrupt_for_reload = false;
    raptor.jet_ai.has_pending_command = false;
    assert!(!raptor.jet_empty_clip_should_auto_rtb());
    raptor.begin_guard_retaliate(ObjectId(9), Some(Vec3::ZERO), None);
    assert!(
        raptor.jet_ai.allow_interrupt_for_reload,
        "C++ GUARD_RETALIATE sets ALLOW_INTERRUPT_AND_RESUME_OF_CUR_STATE_FOR_RELOAD"
    );
    assert_eq!(raptor.tick_jet_ai_update(4), JetAiTickAction::ReturnToBase);
}

#[test]
fn jet_takeoff_pause_afterburner_and_lift_ramp() {
    use crate::game_logic::{KindOf, Team, ThingTemplate};
    use glam::Vec3;
    let mut t = ThingTemplate::new("AmericaJetRaptor");
    t.add_kind_of(KindOf::Aircraft);
    let mut jet = Object::new(t, ObjectId(4), Team::USA);
    jet.max_lift = 8.0;
    jet.set_position(Vec3::new(0.0, 0.0, 0.0));
    jet.apply_taxiing_locomotor_set();
    assert_eq!(jet.jet_ai.cur_locomotor_set.as_deref(), Some("SET_TAXIING"));
    assert!((jet.movement.max_speed - 25.0).abs() < 0.05);

    jet.begin_jet_runway_takeoff(0, Vec3::new(100.0, 0.0, 0.0), 100.0, false);
    assert!(jet.jet_ai.afterburners_on);
    assert!(jet.jet_ai.takeoff_in_progress);
    assert_eq!(jet.max_lift, 0.0);
    assert!(!jet.jet_should_transfer_runway(0));
    assert!(jet.jet_should_transfer_runway(1));
    let _ = jet.tick_jet_takeoff_lift(1);
    jet.set_position(Vec3::new(50.0, 0.0, 0.0));
    let _ = jet.tick_jet_takeoff_lift(jet.jet_ai.takeoff_pause_until);
    assert!(
        jet.max_lift > 0.0 && jet.max_lift < 8.0,
        "lift={}",
        jet.max_lift
    );
}

#[test]
fn jet_lockon_rearms_after_targeter_removed() {
    use crate::game_logic::object::STEALTH_FIGHTER_LOCKON_TIME_FRAMES;
    use crate::game_logic::{KindOf, Team, ThingTemplate};
    let mut sf_t = ThingTemplate::new("AmericaJetStealthFighter");
    sf_t.add_kind_of(KindOf::Aircraft);
    let mut sf = Object::new(sf_t, ObjectId(2), Team::USA);
    sf.add_jet_targeter(ObjectId(9), true, 100);
    assert!(sf.is_temporarily_preventing_aim_success(100));
    assert!(!sf.is_temporarily_preventing_aim_success(100 + STEALTH_FIGHTER_LOCKON_TIME_FRAMES));
    sf.add_jet_targeter(ObjectId(9), false, 200);
    assert!(sf.jet_ai.targeted_by.is_empty());
    assert_eq!(sf.jet_ai.untargetable_expire_frame, 0);
    sf.add_jet_targeter(ObjectId(11), true, 200);
    assert!(
        sf.is_temporarily_preventing_aim_success(200),
        "new targeting episode must re-arm LockonTime"
    );
    assert!(!sf.is_temporarily_preventing_aim_success(200 + STEALTH_FIGHTER_LOCKON_TIME_FRAMES));
}

#[test]
fn jet_taxi_to_takeoff_does_not_enable_afterburners() {
    use crate::game_logic::host_enum_table_residual::{
        jetafterburner_model_bit, jetexhaust_model_bit,
    };
    use crate::game_logic::{KindOf, Team, ThingTemplate};
    use glam::Vec3;
    let mut t = ThingTemplate::new("AmericaJetRaptor");
    t.add_kind_of(KindOf::Aircraft);
    let mut jet = Object::new(t, ObjectId(5), Team::USA);
    jet.apply_taxiing_locomotor_set();
    jet.movement.velocity = Vec3::new(8.0, 0.0, 0.0);
    jet.arm_jet_taxi_to_takeoff(
        Vec3::new(40.0, 0.0, 0.0),
        Vec3::new(120.0, 0.0, 0.0),
        80.0,
        false,
    );
    let _ = jet.tick_jet_ai_update(1);
    assert!(!jet.jet_ai.afterburners_on);
    assert!(jet.jet_ai.taxi_to_takeoff);
    let ab = 1u128 << jetafterburner_model_bit();
    let ex = 1u128 << jetexhaust_model_bit();
    assert_eq!(
        jet.model_condition_bits & ab,
        0,
        "no takeoff afterburner on taxi"
    );
    assert_eq!(
        jet.model_condition_bits & ex,
        0,
        "no JETEXHAUST on ground taxi"
    );
    assert!(!jet.jet_reached_runway_head());
    jet.set_position(Vec3::new(40.0, 0.0, 0.0));
    assert!(jet.jet_reached_runway_head());
    jet.begin_jet_runway_takeoff(10, Vec3::new(120.0, 0.0, 0.0), 80.0, false);
    assert!(jet.jet_ai.afterburners_on);
    jet.finish_jet_takeoff();
    assert!(!jet.jet_ai.afterburners_on);
}

#[test]
fn jet_taxi_takeoff_sets_precise_z_and_ultra_accurate() {
    use crate::game_logic::{KindOf, Team, ThingTemplate};
    use glam::Vec3;
    let mut t = ThingTemplate::new("AmericaJetRaptor");
    t.add_kind_of(KindOf::Aircraft);
    let mut jet = Object::new(t, ObjectId(15), Team::USA);
    jet.arm_jet_taxi_to_takeoff(
        Vec3::new(40.0, 0.0, 0.0),
        Vec3::new(120.0, 0.0, 0.0),
        80.0,
        false,
    );
    assert!(jet.precise_z_pos, "JetTaxi PRECISE_Z_POS");
    assert!(jet.ultra_accurate, "JetTaxi ULTRA_ACCURATE");
    assert!(jet.allow_invalid_position, "JetTaxi ALLOW_INVALID_POSITION");
    jet.begin_jet_runway_takeoff(10, Vec3::new(120.0, 50.0, 0.0), 80.0, false);
    assert!(jet.precise_z_pos, "JetTakeoff PRECISE_Z_POS");
    assert!(jet.ultra_accurate, "JetTakeoff ULTRA_ACCURATE");
    jet.finish_jet_takeoff();
    assert!(!jet.precise_z_pos, "takeoff onExit clears PRECISE_Z_POS");
    assert!(!jet.ultra_accurate, "takeoff onExit clears ULTRA_ACCURATE");
    assert!(
        !jet.allow_invalid_position,
        "takeoff onExit clears ALLOW_INVALID_POSITION"
    );
}

#[test]
fn jet_exhaust_only_in_forward_flight() {
    use crate::game_logic::host_enum_table_residual::jetexhaust_model_bit;
    use crate::game_logic::{KindOf, Team, ThingTemplate};
    use glam::Vec3;
    let mut t = ThingTemplate::new("AmericaJetRaptor");
    t.add_kind_of(KindOf::Aircraft);
    let mut jet = Object::new(t, ObjectId(6), Team::USA);
    let ex = 1u128 << jetexhaust_model_bit();
    jet.apply_airborne_locomotor_set();
    jet.status.airborne_target = true;
    jet.movement.velocity = Vec3::ZERO;
    let _ = jet.tick_jet_ai_update(1);
    assert_eq!(jet.model_condition_bits & ex, 0, "hover has no exhaust");
    jet.movement.velocity = Vec3::new(40.0, 0.0, 0.0);
    let _ = jet.tick_jet_ai_update(2);
    assert_ne!(
        jet.model_condition_bits & ex,
        0,
        "forward flight shows exhaust"
    );
}

#[test]
fn jet_stop_and_enter_airfield_land() {
    use crate::game_logic::{GameLogic, KindOf, ParkingPlaceMetadata, Team, ThingTemplate};
    use glam::Vec3;
    let mut logic = GameLogic::new();
    let mut af_t = ThingTemplate::new("AmericaAirfield");
    af_t.add_kind_of(KindOf::Structure)
        .add_kind_of(KindOf::FSAirfield)
        .set_health(1000.0);
    af_t.parking_place = Some(ParkingPlaceMetadata {
        num_rows: 2,
        num_cols: 2,
        approach_height: 50.0,
        landing_deck_height_offset: 0.0,
        has_runways: true,
        park_in_hangars: false,
        heal_amount_per_second: 10.0,
    });
    logic.templates.insert("AmericaAirfield".into(), af_t);
    let mut jet_t = ThingTemplate::new("AmericaJetRaptor");
    jet_t.add_kind_of(KindOf::Aircraft).set_health(80.0);
    logic.templates.insert("AmericaJetRaptor".into(), jet_t);
    let af = logic
        .create_object("AmericaAirfield", Team::USA, Vec3::ZERO)
        .expect("af");
    let jet = logic
        .create_object("AmericaJetRaptor", Team::USA, Vec3::new(80.0, 40.0, 0.0))
        .expect("jet");
    {
        let j = logic.objects.get_mut(&jet).unwrap();
        j.status.airborne_target = true;
        j.owner_player_id = Some(0);
        j.producer_id = Some(af);
        j.set_ai_state(AIState::Attacking);
        j.target = Some(ObjectId(99));
    }
    assert!(logic.unit_command_stop(jet));
    let j = logic.objects.get(&jet).unwrap();
    assert!(
        j.return_to_base_requested || j.contained_by == Some(af) || j.ai_state == AIState::Moving
    );

    let jet2 = logic
        .create_object("AmericaJetRaptor", Team::USA, Vec3::new(10.0, 40.0, 0.0))
        .expect("jet2");
    {
        let j = logic.objects.get_mut(&jet2).unwrap();
        j.status.airborne_target = true;
        j.health.current = 20.0;
        j.owner_player_id = Some(0);
    }
    assert!(logic.do_jet_landing_command(jet2, af));
}

#[test]
fn jet_hangar_taxi_then_afterburner_at_runway_head_and_rtb_approach() {
    use crate::game_logic::audio_dispatch_impl::{
        clear_test_template_voices, set_test_per_unit_sound,
    };
    use crate::game_logic::object::JET_AFTERBURNER_SOUND_STOP;
    use crate::game_logic::{GameLogic, KindOf, ParkingPlaceMetadata, Team, ThingTemplate};
    use glam::Vec3;
    clear_test_template_voices();
    const AFTERBURNER_EVENT: &str = "RaptorAfterburner";
    set_test_per_unit_sound("AmericaJetRaptor", "Afterburner", AFTERBURNER_EVENT);
    let mut logic = GameLogic::new();
    let mut af_t = ThingTemplate::new("AmericaAirfield");
    af_t.add_kind_of(KindOf::Structure)
        .add_kind_of(KindOf::FSAirfield)
        .set_health(1000.0);
    // Ownerless legacy objects need a unique alive player per team for the
    // (None, None) relationship branch to read Allies (same-team USA).
    if logic.get_player(0).is_none() {
        logic.add_player(crate::game_logic::Player::new(0, Team::USA, "TestPlayer", true));
    }
    af_t.parking_place = Some(ParkingPlaceMetadata {
        num_rows: 2,
        num_cols: 2,
        approach_height: 50.0,
        landing_deck_height_offset: 0.0,
        has_runways: true,
        park_in_hangars: false,
        heal_amount_per_second: 10.0,
    });
    logic.templates.insert("AmericaAirfield".into(), af_t);
    let mut jet_t = ThingTemplate::new("AmericaJetRaptor");
    jet_t.add_kind_of(KindOf::Aircraft).set_health(80.0);
    logic.templates.insert("AmericaJetRaptor".into(), jet_t);
    let af = logic
        .create_object("AmericaAirfield", Team::USA, Vec3::ZERO)
        .expect("af");
    let jet = logic
        .create_object("AmericaJetRaptor", Team::USA, Vec3::new(5.0, 0.0, 0.0))
        .expect("jet");
    {
        let j = logic.objects.get_mut(&jet).unwrap();
        j.status.airborne_target = false;
        j.jet_ai.rtb_landing_phase = crate::game_logic::object::JET_RTB_PHASE_TAXI;
        j.set_position(Vec3::ZERO);
    }
    assert!(logic.try_return_to_base_rearm(jet));
    {
        let j = logic.objects.get_mut(&jet).unwrap();
        assert_eq!(j.contained_by, Some(af));
        let p = j.get_position();
        j.set_position(Vec3::new(p.x - 50.0, p.y, p.z));
    }
    assert!(logic.try_runway_takeoff_from_airfield(jet));
    {
        let j = logic.objects.get(&jet).unwrap();
        assert!(j.contained_by.is_none());
        assert!(j.jet_ai.taxi_to_takeoff || j.jet_ai.takeoff_in_progress);
        assert!(
            !j.jet_ai.afterburners_on,
            "afterburners stay off during hangar/parking taxi"
        );
        assert!(
            !logic
                .queued_audio_events
                .iter()
                .any(|e| e.event_type == AFTERBURNER_EVENT || e.event_type == "Afterburner"),
            "Afterburner sound must not start at taxi-out"
        );
        assert!(
            j.movement.path.len() >= 2,
            "taxi must include hangar/parking intermediate, not just runway end"
        );
    }
    let start = logic
        .objects
        .get(&jet)
        .and_then(|j| j.jet_ai.takeoff_runway_start)
        .map(|p| Vec3::new(p[0], p[1], p[2]))
        .expect("runway start");
    if let Some(j) = logic.objects.get_mut(&jet) {
        j.set_position(start);
    }
    logic.tick_jet_ai_update_all();
    {
        let j = logic.objects.get(&jet).unwrap();
        assert!(
            j.jet_ai.afterburners_on,
            "afterburners at runway-head pause"
        );
        assert!(j.jet_ai.takeoff_in_progress);
    }
    assert!(
        logic
            .queued_audio_events
            .iter()
            .any(|e| e.event_type == AFTERBURNER_EVENT && e.is_looping && !e.stop),
        "Afterburner must queue the per-unit event, not the slot token"
    );
    assert!(
        !logic
            .queued_audio_events
            .iter()
            .any(|e| e.event_type == "Afterburner"),
        "must not queue the Afterburner slot token"
    );
    if let Some(j) = logic.objects.get_mut(&jet) {
        j.finish_jet_takeoff();
    }
    logic.tick_jet_ai_update_all();
    assert!(
        logic
            .queued_audio_events
            .iter()
            .any(|e| e.event_type == JET_AFTERBURNER_SOUND_STOP || e.stop),
        "Afterburner sound must stop when afterburners clear"
    );

    let inbound = logic
        .create_object("AmericaJetRaptor", Team::USA, Vec3::new(2000.0, 40.0, 0.0))
        .expect("inbound");
    if let Some(j) = logic.objects.get_mut(&inbound) {
        j.status.airborne_target = true;
        j.weapon = Some(crate::game_logic::Weapon {
            ammo: Some(0),
            clip_size: 2,
            ..crate::game_logic::Weapon::default()
        });
    }
    // C++ order: the physical RTB command stamps the return request first
    // (doLandingCommand, JetAIUpdate.cpp:2277-2312); the rearm tick then
    // installs the approach leg. The unnamed auto-reload clip is not an
    // isOutOfSpecialReloadAmmo trigger by itself.
    if let Some(j) = logic.objects.get_mut(&inbound) {
        j.return_to_base_requested = true;
    }
    assert!(logic.try_return_to_base_rearm(inbound));
    {
        let j = logic.objects.get(&inbound).unwrap();
        assert!(j.contained_by.is_none(), "distant RTB must not snap-dock");
        let dest = j
            .movement
            .target_position
            .or_else(|| j.movement.path.last().copied())
            .expect("approach dest");
        assert!(
            dest.length() > 20.0,
            "RTB flies to runway approach, not airfield center ({dest:?})"
        );
        assert!(j.jet_allows_air_loco() || j.jet_ai.allow_air_loco);
    }
}

#[test]
fn extra_friction_overlap_force_and_rest_kill() {
    use crate::game_logic::{
        KindOf, MIN_NON_AERO_FRICTION_RESIDUAL, Object, ObjectId, Team, ThingTemplate,
    };
    use glam::Vec3;

    // OCL ExtraFriction sticks on non-loco debris (disabled / !can_move).
    let mut td = ThingTemplate::new("Chunk");
    td.add_kind_of(KindOf::Projectile);
    let mut debris = Object::new(td, ObjectId(7001), Team::USA);
    debris.status.disabled_unmanned = true;
    debris.set_extra_friction(-0.01);
    debris.set_locomotor_physics_options();
    assert!((debris.extra_friction + 0.01).abs() < 1e-6);
    debris.forward_friction = 0.15;
    assert!((debris.get_forward_friction() - 0.14).abs() < 1e-5);

    // ExtraFriction floor still applies.
    debris.forward_friction = 0.0;
    debris.set_extra_friction(-1.0);
    assert!((debris.get_forward_friction() - MIN_NON_AERO_FRICTION_RESIDUAL).abs() < 1e-6);

    // Mobile collide: -min(overlap,5) * delta/dist via applyForce (accel).
    let mut tm = ThingTemplate::new("PanicInf");
    tm.add_kind_of(KindOf::Infantry);
    let mut inf = Object::new(tm, ObjectId(7002), Team::USA);
    inf.set_position(Vec3::new(0.0, 0.0, 0.0));
    inf.physics_mass = 1.0;
    inf.physics_accel = Vec3::ZERO;
    inf.apply_overlap_collide_force(Vec3::new(2.0, 0.0, 0.0), 4.0);
    // force = -4 * (2,0,0)/2 = (-4, 0, 0); accel = force/mass.
    assert!((inf.physics_accel.x + 4.0).abs() < 1e-4);
    assert!(inf.physics_accel.z.abs() < 1e-5);

    // Overlap cap 5.
    inf.physics_accel = Vec3::ZERO;
    inf.apply_overlap_collide_force(Vec3::new(1.0, 0.0, 0.0), 9.0);
    assert!((inf.physics_accel.x + 5.0).abs() < 1e-4);

    // Airborne: 3D delta includes host Y (hq-xvcet).
    inf.set_position(Vec3::new(0.0, 10.0, 0.0));
    inf.ground_height = 0.0;
    inf.physics_accel = Vec3::ZERO;
    inf.apply_overlap_collide_force(Vec3::new(0.0, 2.0, 0.0), 4.0);
    // force = -4 * (0,-8,0)/8 = (0, 4, 0); accel = force/mass (mass 1).
    assert!((inf.physics_accel.y - 4.0).abs() < 1e-4);
    assert!(inf.physics_accel.z.abs() < 1e-5);

    // KillWhenResting uses Object::kill (UNRESISTABLE), not stun-destroy only.
    let mut tr = ThingTemplate::new("RestProp");
    tr.add_kind_of(KindOf::Vehicle);
    tr.set_health(50.0);
    let mut prop = Object::new(tr, ObjectId(7003), Team::USA);
    prop.kill_when_resting_on_ground = true;
    prop.health.current = 50.0;
    prop.health.maximum = 50.0;
    prop.set_position(Vec3::ZERO);
    prop.ground_height = 0.0;
    prop.movement.velocity = Vec3::ZERO;
    assert!(prop.maybe_kill_when_resting_on_ground());
    assert!(prop.status.destroyed);
    assert!(prop.health.current <= 0.0);

    // Height > 0 is airborne (isAboveTerrain); 0.04 no longer counts as resting.
    let mut ta = ThingTemplate::new("RestProp2");
    ta.add_kind_of(KindOf::Vehicle);
    let mut air = Object::new(ta, ObjectId(7004), Team::USA);
    air.kill_when_resting_on_ground = true;
    air.set_position(Vec3::new(0.0, 0.04, 0.0));
    air.ground_height = 0.0;
    air.movement.velocity = Vec3::ZERO;
    assert!(!air.maybe_kill_when_resting_on_ground());
    air.set_position(Vec3::ZERO);
    assert!(air.maybe_kill_when_resting_on_ground());
}

#[test]
fn script_emoticon_flash_color_match_cpp() {
    let mut obj = make_test_object();
    obj.set_emoticon("EmoticonAlert", 60);
    assert_eq!(obj.emoticon_name, "EmoticonAlert");
    assert_eq!(obj.emoticon_frames_left, 60);
    obj.set_emoticon("EmoticonCheer", -30);
    assert_eq!(obj.emoticon_name, "EmoticonCheer");
    assert_eq!(obj.emoticon_frames_left, i32::MAX);
    obj.set_emoticon("Gone", 0);
    assert!(obj.emoticon_name.is_empty());
    assert_eq!(obj.emoticon_frames_left, 0);

    obj.set_script_flash(2, 0x00FF_FFFF);
    assert_eq!(obj.flash_count, 4, "2s * 30fps / 15 frames-per-flash");
    assert_eq!(obj.flash_color, 0x00FF_FFFF);
    obj.set_script_flash(0, 0x00FF_0000);
    assert_eq!(obj.flash_count, 4, "C++ named flash ignores seconds <= 0");

    obj.set_custom_indicator_color_raw(0xFFFF_0000);
    assert_eq!(obj.custom_indicator_color, Some(0xFFFF_0000));
    obj.set_custom_indicator_color_raw(0);
    assert_eq!(obj.custom_indicator_color, None);
}

#[test]
fn live_host_script_visual_status_apply() {
    use crate::game_logic::{GameLogic, KindOf, Player, Team, ThingTemplate};
    use gamelogic::object::registry::OBJECT_REGISTRY;
    use gamelogic::scripting::{
        HostScriptCustomColorRequest, HostScriptEmoticonRequest, HostScriptFlashRequest,
        HostScriptHeldRequest, HostScriptRepulsorRequest, request_host_script_custom_color,
        request_host_script_emoticon, request_host_script_flash, request_host_script_held,
        request_host_script_repulsor,
    };
    use glam::Vec3;

    OBJECT_REGISTRY.clear();
    let _ = gamelogic::scripting::take_host_script_flash_requests();
    let _ = gamelogic::scripting::take_host_script_emoticon_requests();
    let _ = gamelogic::scripting::take_host_script_held_requests();
    let _ = gamelogic::scripting::take_host_script_custom_color_requests();
    let _ = gamelogic::scripting::take_host_script_repulsor_requests();

    let mut logic = GameLogic::new();
    logic.scripts_loaded = true;
    let mut p = Player::new(0, Team::USA, "PlyrAmerica", true);
    p.color_rgb = (0x12, 0x34, 0x56);
    logic.add_player(p);
    let mut tmpl = ThingTemplate::new("AmericaInfantryRanger");
    tmpl.add_kind_of(KindOf::Infantry);
    tmpl.set_health(100.0);
    logic.templates.insert("AmericaInfantryRanger".into(), tmpl);
    let id = logic
        .create_object(
            "AmericaInfantryRanger",
            Team::USA,
            Vec3::new(10.0, 0.0, 20.0),
        )
        .expect("ranger");
    if let Some(o) = logic.host_object_mut(id) {
        o.name = "FlashRanger".into();
        o.team_instance_name = "teamAmerica".into();
        o.owner_player_id = Some(0);
    }

    request_host_script_flash(HostScriptFlashRequest::Named {
        unit: "FlashRanger".into(),
        seconds: 2,
        white: false,
    });
    request_host_script_flash(HostScriptFlashRequest::Team {
        team: "teamAmerica".into(),
        seconds: 3,
        white: true,
    });
    request_host_script_emoticon(HostScriptEmoticonRequest::Named {
        unit: "FlashRanger".into(),
        emoticon: "EmoticonAlert".into(),
        duration_frames: -30,
    });
    request_host_script_held(HostScriptHeldRequest {
        unit: "FlashRanger".into(),
        held: true,
    });
    request_host_script_custom_color(HostScriptCustomColorRequest {
        unit: "FlashRanger".into(),
        color_raw: 0xFFFF_0000,
    });
    request_host_script_repulsor(HostScriptRepulsorRequest::Named {
        unit: "FlashRanger".into(),
        enabled: true,
    });
    logic.evaluate_and_execute_scripts(0.0);

    let obj = logic.host_object(id).expect("applied");
    assert_eq!(
        obj.flash_count, 6,
        "team white 3s * 30 / 15 overwrites named 2s"
    );
    assert_eq!(
        obj.flash_color, 0x00FF_FFFF,
        "FLASH_WHITE RGBColor(1,1,1).getAsInt"
    );
    assert_eq!(obj.emoticon_name, "EmoticonAlert");
    assert_eq!(
        obj.emoticon_frames_left,
        i32::MAX,
        "duration < 0 is FOREVER"
    );
    assert!(obj.status.disabled_held, "NAMED_SET_HELD DISABLED_HELD");
    assert!(obj.is_physics_held());
    assert!(!obj.can_move());
    assert_eq!(obj.custom_indicator_color, Some(0xFFFF_0000));
    assert!(obj.status.repulsor, "OBJECT_STATUS_REPULSOR");
    assert_eq!(obj.repulsor_until_frame, 0, "script repulsor is permanent");

    request_host_script_held(HostScriptHeldRequest {
        unit: "FlashRanger".into(),
        held: false,
    });
    request_host_script_custom_color(HostScriptCustomColorRequest {
        unit: "FlashRanger".into(),
        color_raw: 0,
    });
    request_host_script_repulsor(HostScriptRepulsorRequest::Team {
        team: "teamAmerica".into(),
        enabled: false,
    });
    request_host_script_emoticon(HostScriptEmoticonRequest::Team {
        team: "teamAmerica".into(),
        emoticon: "EmoticonCheer".into(),
        duration_frames: 45,
    });
    request_host_script_flash(HostScriptFlashRequest::Named {
        unit: "FlashRanger".into(),
        seconds: 2,
        white: false,
    });
    logic.evaluate_and_execute_scripts(0.0);

    let obj = logic.host_object(id).expect("toggled");
    assert!(!obj.status.disabled_held);
    assert_eq!(obj.custom_indicator_color, None, "color 0 removes custom");
    assert!(!obj.status.repulsor);
    assert_eq!(obj.emoticon_name, "EmoticonCheer");
    assert_eq!(obj.emoticon_frames_left, 45);
    assert_eq!(obj.flash_count, 4);
    assert_eq!(
        obj.flash_color,
        crate::game_logic::host_radar::pack_player_color_argb((0x12, 0x34, 0x56)),
        "NAMED_FLASH uses getIndicatorColor"
    );
    assert!(OBJECT_REGISTRY.is_empty());
}

#[test]
fn dead_without_works_when_dead_skips_locomotor() {
    let mut jet = make_ground_unit("DeadSkipJet", 6101, KindOf::Aircraft);
    jet.health.current = 0.0;
    assert!(jet.host_skip_dead_locomotor());
    jet.locomotor_works_when_dead = true;
    assert!(!jet.host_skip_dead_locomotor());
}

#[test]
fn blocked_by_ignores_near_goal_and_reverse() {
    let mut tank = make_ground_unit("NearGoalTank", 6102, KindOf::Vehicle);
    tank.movement.target_position = Some(glam::Vec3::new(5.0, 0.0, 0.0));
    let mut inf = make_ground_unit("NearGoalInf", 6103, KindOf::Infantry);
    inf.set_position(glam::Vec3::new(2.0, 0.0, 0.0));
    assert!(
        !tank.ai_blocked_by(&inf, true),
        "within one pathfind cell of the goal is not blocked"
    );

    tank.movement.target_position = Some(glam::Vec3::new(40.0, 0.0, 0.0));
    tank.moving_backwards = true;
    assert!(
        !tank.ai_blocked_by(&inf, true),
        "reversing units skip blockedBy"
    );
}

#[test]
fn blocked_by_same_cell_uses_path_priority() {
    let mut dozer = make_ground_unit("PrioDozer", 6104, KindOf::Dozer);
    let mut inf = make_ground_unit("PrioInf", 6105, KindOf::Infantry);
    // Same cell (dsqr ~ 0).
    dozer.set_position(glam::Vec3::new(0.0, 0.0, 0.0));
    inf.set_position(glam::Vec3::new(0.0, 0.0, 0.0));
    assert!(
        dozer.ai_blocked_by(&inf, true),
        "dozer has higher path priority so is blocked (lowest priority wins)"
    );
    assert!(
        !inf.ai_blocked_by(&dozer, true),
        "infantry yields the stacked cell"
    );
}

#[test]
fn blocked_by_off_angle_higher_priority_yields() {
    let mut dozer = make_ground_unit("OffAngleDozer", 6106, KindOf::Dozer);
    let mut truck = make_ground_unit("OffAngleTruck", 6107, KindOf::Vehicle);
    dozer.set_position(glam::Vec3::ZERO);
    dozer.set_orientation(0.0);
    dozer.movement.velocity = glam::Vec3::new(8.0, 0.0, 0.0);
    // Off-angle closer pair: headings still overlap (dot>0) and distance shrinks.
    truck.set_position(glam::Vec3::new(4.0, 0.0, 8.0));
    // Engine movement convention: yaw = (-dz).atan2(dx) (unit_direction_xz),
    // so heading (0.8,-0.6) is +atan2(0.6,0.8), not plain atan2(-0.6,0.8).
    truck.set_orientation(0.6f32.atan2(0.8));
    truck.movement.velocity = glam::Vec3::new(6.4, 0.0, -4.8);
    assert!(
        !dozer.ai_blocked_by(&truck, true),
        "higher-priority dozer yields leftover off-angle heading"
    );
    assert!(
        truck.ai_blocked_by(&dozer, true),
        "lower-priority truck stays blocked on leftover off-angle close"
    );
}
#[test]
fn blocked_speed_applies_formation_crowd_factor() {
    let mut a = make_ground_unit("FormA", 6108, KindOf::Vehicle);
    let mut b = make_ground_unit("FormB", 6109, KindOf::Vehicle);
    a.set_position(glam::Vec3::ZERO);
    a.set_orientation(0.0);
    b.set_position(glam::Vec3::new(10.0, 0.0, 0.0));
    b.set_orientation(0.0);
    b.movement.velocity = glam::Vec3::new(10.0, 0.0, 0.0);
    let raw = a.calculate_max_blocked_speed(&b);
    a.formation_id = 7;
    b.formation_id = 7;
    let crowded = a.calculate_max_blocked_speed(&b);
    assert!(
        (raw - 10.0).abs() < 1e-4,
        "unformed blocked speed is away_speed, got {raw}"
    );
    assert!(
        (crowded - 5.5).abs() < 1e-4,
        "same formation scales blocked speed by 0.55, got {crowded}"
    );
}
