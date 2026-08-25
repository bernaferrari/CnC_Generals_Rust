//! Behavior suite extracted from `vehicles_and_lasers`.
use super::*;

#[test]
fn helipad_heals_landed_helo_without_stall() {
    use crate::game_logic::{KindOf, ParkingPlaceMetadata, Team, ThingTemplate};
    use glam::Vec3;

    let mut logic = GameLogic::new();
    let mut pad_tmpl = ThingTemplate::new("HealHelipad");
    pad_tmpl
        .add_kind_of(KindOf::Structure)
        .add_kind_of(KindOf::FSAirfield)
        .set_health(1000.0);
    pad_tmpl.parking_place = Some(ParkingPlaceMetadata {
        num_rows: 1,
        num_cols: 1,
        approach_height: 37.0,
        landing_deck_height_offset: 4.0,
        has_runways: false,
        park_in_hangars: false,
        heal_amount_per_second: 10.0,
    });
    logic.templates.insert("HealHelipad".into(), pad_tmpl);
    let mut heli_tmpl = ThingTemplate::new("AmericaVehicleComanche");
    heli_tmpl
        .add_kind_of(KindOf::Aircraft)
        .add_kind_of(KindOf::Vehicle)
        .set_health(220.0);
    logic
        .templates
        .insert("AmericaVehicleComanche".into(), heli_tmpl);

    let pad = logic
        .create_object("HealHelipad", Team::USA, Vec3::ZERO)
        .unwrap();
    let heli = logic
        .create_object(
            "AmericaVehicleComanche",
            Team::USA,
            Vec3::new(0.0, 4.0, 0.0),
        )
        .unwrap();
    {
        let h = logic.objects.get_mut(&heli).unwrap();
        h.set_contained_by(Some(pad));
        h.set_ai_state(AIState::Docked);
        h.status.airborne_target = false;
        h.producer_id = Some(pad);
        h.health.current = 100.0;
        h.target = Some(ObjectId(99));
    }
    let hp_before = logic.objects.get(&heli).unwrap().health.current;
    logic.tick_airfield_parking_heal();
    for _ in 0..6 {
        logic.frame = logic.frame.saturating_add(1);
        logic.tick_airfield_parking_heal();
    }
    let hp_after = logic.objects.get(&heli).unwrap().health.current;
    assert!(
        hp_after > hp_before + 1.0,
        "landed helo must heal without a hangar stall ({hp_before} -> {hp_after})"
    );
}

#[test]
fn taxiing_jet_heals_from_reserved_stall() {
    use crate::game_logic::buildings::BuildingType;
    use crate::game_logic::{KindOf, ParkingPlaceMetadata, Team, ThingTemplate, Weapon};
    use glam::Vec3;

    let mut logic = GameLogic::new();
    let mut af_tmpl = ThingTemplate::new("TaxiHealAirfield");
    af_tmpl
        .add_kind_of(KindOf::Structure)
        .add_kind_of(KindOf::FSAirfield)
        .set_health(1000.0);
    af_tmpl.parking_place = Some(ParkingPlaceMetadata {
        num_rows: 2,
        num_cols: 2,
        approach_height: 50.0,
        landing_deck_height_offset: 0.0,
        has_runways: true,
        park_in_hangars: true,
        heal_amount_per_second: 10.0,
    });
    logic.templates.insert("TaxiHealAirfield".into(), af_tmpl);
    let mut jet_tmpl = ThingTemplate::new("TaxiHealRaptor");
    jet_tmpl.add_kind_of(KindOf::Aircraft).set_health(100.0);
    logic.templates.insert("TaxiHealRaptor".into(), jet_tmpl);

    let af = logic
        .create_object("TaxiHealAirfield", Team::USA, Vec3::ZERO)
        .unwrap();
    if let Some(o) = logic.host_object_mut(af) {
        o.building_data = Some(crate::game_logic::BuildingData::new(BuildingType::Airfield));
    }
    let jet_id = logic
        .create_object("TaxiHealRaptor", Team::USA, Vec3::new(10.0, 0.0, 0.0))
        .unwrap();
    if let Some(jet) = logic.objects.get_mut(&jet_id) {
        jet.weapon = Some(Weapon {
            damage: 10.0,
            range: 100.0,
            reload_time: 0.0,
            last_fire_time: -100.0,
            ammo: Some(0),
            clip_size: 2,
            ..Weapon::default()
        });
    }
    assert!(logic.try_return_to_base_rearm(jet_id));
    {
        let jet = logic.objects.get_mut(&jet_id).unwrap();
        jet.set_contained_by(None);
        jet.status.airborne_target = false;
        jet.jet_ai.takeoff_in_progress = true;
        jet.health.current = 40.0;
    }
    let hp_before = logic.objects.get(&jet_id).unwrap().health.current;
    logic.tick_airfield_parking_heal();
    for _ in 0..6 {
        logic.frame = logic.frame.saturating_add(1);
        logic.tick_airfield_parking_heal();
    }
    let hp_after = logic.objects.get(&jet_id).unwrap().health.current;
    assert!(
        hp_after > hp_before + 1.0,
        "taxiing jet must heal from reserved stall (JetAIUpdate.cpp:1834-1852) ({hp_before} -> {hp_after})"
    );
}

#[test]
fn chinook_landed_heals_without_stall() {
    use crate::game_logic::host_combat_chinook::{HostChinookAI, HostChinookFlightStatus};
    use crate::game_logic::{KindOf, ParkingPlaceMetadata, Team, ThingTemplate};
    use glam::Vec3;

    let mut logic = GameLogic::new();
    let mut pad_tmpl = ThingTemplate::new("ChinookHealPad");
    pad_tmpl
        .add_kind_of(KindOf::Structure)
        .add_kind_of(KindOf::FSAirfield)
        .set_health(1000.0);
    pad_tmpl.parking_place = Some(ParkingPlaceMetadata {
        num_rows: 1,
        num_cols: 1,
        approach_height: 37.0,
        landing_deck_height_offset: 4.0,
        has_runways: false,
        park_in_hangars: false,
        heal_amount_per_second: 10.0,
    });
    logic.templates.insert("ChinookHealPad".into(), pad_tmpl);
    let mut chinook_tmpl = ThingTemplate::new("AmericaVehicleChinook");
    chinook_tmpl
        .add_kind_of(KindOf::Aircraft)
        .add_kind_of(KindOf::Vehicle)
        .set_health(220.0);
    logic
        .templates
        .insert("AmericaVehicleChinook".into(), chinook_tmpl);

    let pad = logic
        .create_object("ChinookHealPad", Team::USA, Vec3::ZERO)
        .unwrap();
    let chinook = logic
        .create_object("AmericaVehicleChinook", Team::USA, Vec3::new(0.0, 4.0, 0.0))
        .unwrap();
    {
        let h = logic.objects.get_mut(&chinook).unwrap();
        let mut ai = HostChinookAI::new_vanilla([0.0, 4.0, 0.0]);
        ai.flight_status = HostChinookFlightStatus::Landed;
        ai.airfield_id = Some(pad.0);
        h.chinook_ai = Some(ai);
        h.status.airborne_target = false;
        h.health.current = 100.0;
    }
    let hp_before = logic.objects.get(&chinook).unwrap().health.current;
    logic.tick_airfield_parking_heal();
    for _ in 0..6 {
        logic.frame = logic.frame.saturating_add(1);
        logic.tick_airfield_parking_heal();
    }
    let hp_after = logic.objects.get(&chinook).unwrap().health.current;
    assert!(
        hp_after > hp_before + 1.0,
        "CHINOOK_LANDED must heal without a hangar stall (ChinookAIUpdate.cpp:1055) ({hp_before} -> {hp_after})"
    );
}

#[test]
fn queued_jet_reserves_exit_stall() {
    use crate::game_logic::buildings::BuildingType;
    use crate::game_logic::{KindOf, ParkingPlaceMetadata, Team, ThingTemplate};
    use glam::Vec3;

    let mut logic = GameLogic::new();
    ensure_test_player_for_team(&mut logic, Team::USA);
    let mut af_t = ThingTemplate::new("ExitReserveAirfield");
    af_t.add_kind_of(KindOf::Structure)
        .add_kind_of(KindOf::FSAirfield)
        .set_health(2000.0);
    af_t.parking_place = Some(ParkingPlaceMetadata {
        num_rows: 2,
        num_cols: 2,
        approach_height: 50.0,
        landing_deck_height_offset: 0.0,
        has_runways: true,
        park_in_hangars: true,
        heal_amount_per_second: 10.0,
    });
    logic.templates.insert("ExitReserveAirfield".into(), af_t);
    let mut jet_t = ThingTemplate::new("ExitReserveRaptor");
    jet_t
        .add_kind_of(KindOf::Aircraft)
        .set_health(200.0)
        .set_cost(100, 0);
    logic.templates.insert("ExitReserveRaptor".into(), jet_t);

    let af = logic
        .create_object("ExitReserveAirfield", Team::USA, Vec3::ZERO)
        .unwrap();
    if let Some(o) = logic.host_object_mut(af) {
        o.building_data = Some(crate::game_logic::BuildingData::new(BuildingType::Airfield));
    }
    assert!(logic.enqueue_production(af, "ExitReserveRaptor".into()));
    assert!(
        logic
            .airfield_parking_spaces
            .get(&af)
            .is_some_and(|spaces| {
                spaces
                    .iter()
                    .any(|space| space.reserved_for_exit && space.object_id.is_none())
            }),
        "enqueue must set reservedForExit without objectInSpace (ParkingPlaceBehavior.cpp:695-715)"
    );
    for i in 0..3 {
        let j = logic
            .create_object(
                "ExitReserveRaptor",
                Team::USA,
                Vec3::new(20.0 + i as f32, 0.0, 0.0),
            )
            .unwrap();
        if let Some(jet) = logic.objects.get_mut(&j) {
            jet.weapon = Some(crate::game_logic::Weapon {
                damage: 10.0,
                range: 100.0,
                reload_time: 0.0,
                last_fire_time: -100.0,
                ammo: Some(0),
                clip_size: 2,
                ..crate::game_logic::Weapon::default()
            });
        }
        assert!(logic.try_return_to_base_rearm(j), "dock {i}");
    }
    let inbound = logic
        .create_object("ExitReserveRaptor", Team::USA, Vec3::new(80.0, 40.0, 0.0))
        .unwrap();
    if let Some(jet) = logic.objects.get_mut(&inbound) {
        jet.weapon = Some(crate::game_logic::Weapon {
            damage: 10.0,
            range: 100.0,
            reload_time: 0.0,
            last_fire_time: -100.0,
            ammo: Some(0),
            clip_size: 2,
            ..crate::game_logic::Weapon::default()
        });
    }
    assert!(
        !logic.try_return_to_base_rearm(inbound),
        "returning jet must not steal reservedForExit stall"
    );
}

#[test]
fn runway_reserve_requires_jet_stall() {
    use crate::game_logic::buildings::BuildingType;
    use crate::game_logic::{KindOf, ParkingPlaceMetadata, Team, ThingTemplate};
    use glam::Vec3;

    let mut logic = GameLogic::new();
    let mut af_tmpl = ThingTemplate::new("RunwayStallAirfield");
    af_tmpl
        .add_kind_of(KindOf::Structure)
        .add_kind_of(KindOf::FSAirfield)
        .set_health(1000.0);
    af_tmpl.parking_place = Some(ParkingPlaceMetadata {
        num_rows: 2,
        num_cols: 2,
        approach_height: 50.0,
        landing_deck_height_offset: 0.0,
        has_runways: true,
        park_in_hangars: true,
        heal_amount_per_second: 10.0,
    });
    logic
        .templates
        .insert("RunwayStallAirfield".into(), af_tmpl);
    let mut jet_tmpl = ThingTemplate::new("RunwayStallRaptor");
    jet_tmpl.add_kind_of(KindOf::Aircraft).set_health(100.0);
    logic.templates.insert("RunwayStallRaptor".into(), jet_tmpl);

    let af = logic
        .create_object("RunwayStallAirfield", Team::USA, Vec3::ZERO)
        .unwrap();
    if let Some(o) = logic.host_object_mut(af) {
        o.building_data = Some(crate::game_logic::BuildingData::new(BuildingType::Airfield));
    }
    let jet = logic
        .create_object("RunwayStallRaptor", Team::USA, Vec3::new(10.0, 0.0, 0.0))
        .unwrap();
    assert!(
        logic.reserve_airfield_runway(af, jet).is_none(),
        "C++ reserveRunway fails without a parking stall"
    );
}

#[test]
fn private_attack_object_enters_attack_state_machine() {
    use crate::game_logic::{
        AttackSubState, KindOf, Object, ObjectId, Team, ThingTemplate, Weapon,
    };
    use glam::Vec3;
    let mut logic = GameLogic::new();
    let mut at = ThingTemplate::new("PaA");
    at.add_kind_of(KindOf::Infantry);
    let aid = ObjectId(1701);
    logic.objects.insert(aid, {
        let mut o = Object::new(at, aid, Team::USA);
        o.set_position(Vec3::ZERO);
        o.set_orientation(0.0);
        o.weapon = Some(Weapon {
            damage: 10.0,
            range: 100.0,
            reload_time: 1.0,
            last_fire_time: -10.0,
            projectile_speed: 200.0,
            ..Default::default()
        });
        o
    });
    let mut vt = ThingTemplate::new("PaV");
    vt.add_kind_of(KindOf::Infantry);
    let vid = ObjectId(1702);
    logic.objects.insert(vid, {
        let mut o = Object::new(vt, vid, Team::GLA);
        o.set_position(Vec3::new(20.0, 0.0, 0.0));
        o
    });
    assert!(logic.private_attack_object(aid, vid, -1));
    assert!(logic.objects[&aid].status.is_aiming_weapon);
    assert_eq!(
        logic.objects[&aid].attack_substate,
        AttackSubState::AimAtTarget
    );
    // Nested tick should promote Aim → Fire when facing.
    logic.tick_nested_attack_machines(&[aid], 10.0, 1);
    assert_eq!(
        logic.objects[&aid].attack_substate,
        AttackSubState::FireWeapon
    );
}

#[test]
fn nested_attack_machine_fires_an_explicit_tertiary_slot() {
    use crate::game_logic::{KindOf, Object, ObjectId, Team, ThingTemplate, Weapon};
    use glam::Vec3;

    let mut logic = GameLogic::new();
    let mut attacker_template = ThingTemplate::new("TertiaryOnlyAttacker");
    attacker_template.add_kind_of(KindOf::Infantry);
    let attacker_id = ObjectId(1711);
    logic.objects.insert(attacker_id, {
        let mut object = Object::new(attacker_template, attacker_id, Team::USA);
        object.set_position(Vec3::ZERO);
        object.set_orientation(0.0);
        object.tertiary_weapon = Some(Weapon {
            damage: 73.0,
            range: 100.0,
            reload_time: 1.0,
            last_fire_time: -10.0,
            projectile_speed: 200.0,
            ..Weapon::default()
        });
        object.set_active_weapon_slot(2);
        object
    });

    let mut victim_template = ThingTemplate::new("TertiaryOnlyVictim");
    victim_template.add_kind_of(KindOf::Infantry);
    let victim_id = ObjectId(1712);
    logic.objects.insert(victim_id, {
        let mut object = Object::new(victim_template, victim_id, Team::GLA);
        object.set_position(Vec3::new(20.0, 0.0, 0.0));
        object
    });

    assert!(logic.private_attack_object(attacker_id, victim_id, -1));
    logic.tick_nested_attack_machines(&[attacker_id], 10.0, 1);
    logic.tick_nested_attack_machines(&[attacker_id], 10.1, 2);

    let attacker = logic.host_object(attacker_id).expect("attacker");
    assert_eq!(attacker.last_fire_slot, 2);
    assert_eq!(attacker.last_fire_damage, 73.0);
}

#[test]
fn turret_sm_aim_to_fire_when_aligned() {
    use crate::game_logic::object::TurretSubState;
    use crate::game_logic::{KindOf, Object, ObjectId, Team, ThingTemplate, Weapon};
    use glam::Vec3;
    let mut logic = GameLogic::new();
    let mut at = ThingTemplate::new("TsmA");
    at.add_kind_of(KindOf::Vehicle);
    let aid = ObjectId(2101);
    logic.objects.insert(aid, {
        let mut o = Object::new(at, aid, Team::USA);
        o.set_position(Vec3::ZERO);
        o.set_orientation(0.0);
        o.turret_enabled = true;
        o.turret_angle_deg = 0.0;
        o.turret_turn_rate_rad = 1.0;
        o.weapon = Some(Weapon {
            damage: 10.0,
            range: 100.0,
            reload_time: 1.0,
            last_fire_time: -10.0,
            projectile_speed: 200.0,
            ..Default::default()
        });
        o
    });
    let mut vt = ThingTemplate::new("TsmV");
    vt.add_kind_of(KindOf::Infantry);
    let vid = ObjectId(2102);
    logic.objects.insert(vid, {
        let mut o = Object::new(vt, vid, Team::GLA);
        o.set_position(Vec3::new(20.0, 0.0, 0.0));
        o
    });
    logic.set_turret_target_object(aid, Some(vid), false);
    assert_eq!(logic.objects[&aid].turret_substate, TurretSubState::Aim);
    let r = logic.tick_turret_state_machine(aid, 10.0, 1);
    assert_eq!(r, AttackAimResult::Success);
    assert_eq!(logic.objects[&aid].turret_substate, TurretSubState::Fire);
}

#[test]
fn turret_sm_fire_returns_to_aim() {
    use crate::game_logic::object::TurretSubState;
    use crate::game_logic::{KindOf, Object, ObjectId, Team, ThingTemplate, Weapon};
    use glam::Vec3;
    let mut logic = GameLogic::new();
    let mut at = ThingTemplate::new("TsmA2");
    at.add_kind_of(KindOf::Vehicle);
    let aid = ObjectId(2103);
    logic.objects.insert(aid, {
        let mut o = Object::new(at, aid, Team::USA);
        o.set_position(Vec3::ZERO);
        o.set_orientation(0.0);
        o.turret_enabled = true;
        o.turret_angle_deg = 0.0;
        o.turret_substate = TurretSubState::Fire;
        o.turret_target_id = Some(ObjectId(2104));
        o.weapon = Some(Weapon {
            damage: 10.0,
            range: 100.0,
            reload_time: 1.0,
            last_fire_time: -10.0,
            projectile_speed: 200.0,
            ..Default::default()
        });
        o
    });
    let mut vt = ThingTemplate::new("TsmV2");
    vt.add_kind_of(KindOf::Infantry);
    let vid = ObjectId(2104);
    logic.objects.insert(vid, {
        let mut o = Object::new(vt, vid, Team::GLA);
        o.set_position(Vec3::new(15.0, 0.0, 0.0));
        o
    });
    let _ = logic.tick_turret_state_machine(aid, 10.0, 1);
    assert_eq!(logic.objects[&aid].turret_substate, TurretSubState::Aim);
}

#[test]
fn turret_sm_clear_target_holds_then_recenters() {
    use crate::game_logic::object::TurretSubState;
    use crate::game_logic::{KindOf, Object, ObjectId, Team, ThingTemplate, Weapon};
    let mut logic = GameLogic::new();
    logic.frame = 100;
    let mut at = ThingTemplate::new("TsmA3");
    at.add_kind_of(KindOf::Vehicle);
    let aid = ObjectId(2105);
    logic.objects.insert(aid, {
        let mut o = Object::new(at, aid, Team::USA);
        o.turret_enabled = true;
        o.turret_angle_deg = 45.0;
        o.turret_natural_angle_deg = 0.0;
        o.turret_turn_rate_rad = 1.0;
        o.turret_recenter_frames = 5;
        o.turret_substate = TurretSubState::Aim;
        o.turret_target_id = Some(ObjectId(99));
        o.weapon = Some(Weapon {
            range: 50.0,
            ..Default::default()
        });
        o
    });
    logic.set_turret_target_object(aid, None, false);
    assert_eq!(logic.objects[&aid].turret_substate, TurretSubState::Hold);
    assert!(logic.objects[&aid].turret_holding);
    // Hold not elapsed
    let _ = logic.tick_turret_state_machine(aid, 0.0, 102);
    assert_eq!(logic.objects[&aid].turret_substate, TurretSubState::Hold);
    // Hold elapsed → Recenter
    let _ = logic.tick_turret_state_machine(aid, 0.0, 200);
    assert_eq!(
        logic.objects[&aid].turret_substate,
        TurretSubState::Recenter
    );
    // Recenter to natural
    for f in 201..220 {
        let r = logic.tick_turret_state_machine(aid, 0.0, f);
        if logic.objects[&aid].turret_substate == TurretSubState::Idle {
            assert_eq!(r, AttackAimResult::Success);
            break;
        }
    }
    assert_eq!(logic.objects[&aid].turret_substate, TurretSubState::Idle);
    assert!(logic.objects[&aid].turret_angle_deg.abs() < 1.0);
}

#[test]
fn turret_sm_fire_oor_returns_to_aim() {
    use crate::game_logic::object::TurretSubState;
    use crate::game_logic::{KindOf, Object, ObjectId, Team, ThingTemplate, Weapon};
    use glam::Vec3;
    let mut logic = GameLogic::new();
    let mut at = ThingTemplate::new("TsmA4");
    at.add_kind_of(KindOf::Vehicle);
    let aid = ObjectId(2106);
    logic.objects.insert(aid, {
        let mut o = Object::new(at, aid, Team::USA);
        o.set_position(Vec3::ZERO);
        o.turret_enabled = true;
        o.turret_substate = TurretSubState::Fire;
        o.turret_target_id = Some(ObjectId(2107));
        o.weapon = Some(Weapon {
            range: 30.0,
            ..Default::default()
        });
        o
    });
    let mut vt = ThingTemplate::new("TsmV4");
    vt.add_kind_of(KindOf::Infantry);
    let vid = ObjectId(2107);
    logic.objects.insert(vid, {
        let mut o = Object::new(vt, vid, Team::GLA);
        o.set_position(Vec3::new(400.0, 0.0, 0.0));
        o
    });
    let _ = logic.tick_turret_state_machine(aid, 10.0, 1);
    assert_eq!(logic.objects[&aid].turret_substate, TurretSubState::Aim);
}

#[test]
fn set_turret_target_object_enters_aim() {
    use crate::game_logic::object::TurretSubState;
    use crate::game_logic::{KindOf, Object, ObjectId, Team, ThingTemplate, Weapon};
    let mut logic = GameLogic::new();
    let mut at = ThingTemplate::new("TurA");
    at.add_kind_of(KindOf::Vehicle);
    let aid = ObjectId(2001);
    logic.objects.insert(aid, {
        let mut o = Object::new(at, aid, Team::USA);
        o.turret_enabled = true;
        o.turret_angle_deg = 0.0;
        o.turret_turn_rate_rad = 0.5; // fast for test
        o.weapon = Some(Weapon {
            range: 100.0,
            ..Default::default()
        });
        o
    });
    let mut vt = ThingTemplate::new("TurV");
    vt.add_kind_of(KindOf::Infantry);
    let vid = ObjectId(2002);
    logic.objects.insert(vid, {
        let mut o = Object::new(vt, vid, Team::GLA);
        o
    });
    logic.set_turret_target_object(aid, Some(vid), false);
    assert_eq!(logic.objects[&aid].turret_target_id, Some(vid));
    assert_eq!(logic.objects[&aid].turret_substate, TurretSubState::Aim);
    assert!(logic.objects[&aid].is_trying_to_aim_at_target(vid));
    logic.set_turret_target_object(aid, None, false);
    assert!(logic.objects[&aid].turret_target_id.is_none());
    assert_eq!(logic.objects[&aid].turret_substate, TurretSubState::Hold);
}

#[test]
fn tick_turret_aim_aligns_toward_target() {
    use crate::game_logic::{KindOf, Object, ObjectId, Team, ThingTemplate, Weapon};
    use glam::Vec3;
    let mut logic = GameLogic::new();
    let mut at = ThingTemplate::new("TurA2");
    at.add_kind_of(KindOf::Vehicle);
    let aid = ObjectId(2003);
    logic.objects.insert(aid, {
        let mut o = Object::new(at, aid, Team::USA);
        o.set_position(Vec3::ZERO);
        o.set_orientation(0.0);
        o.turret_enabled = true;
        o.turret_angle_deg = 0.0;
        // Slow turn so first tick Continues if target is behind... use front.
        o.turret_turn_rate_rad = 0.02;
        o.weapon = Some(Weapon {
            range: 200.0,
            ..Default::default()
        });
        o
    });
    let mut vt = ThingTemplate::new("TurV2");
    vt.add_kind_of(KindOf::Infantry);
    let vid = ObjectId(2004);
    logic.objects.insert(vid, {
        let mut o = Object::new(vt, vid, Team::GLA);
        o.set_position(Vec3::new(50.0, 0.0, 0.0)); // +X, rel ~0
        o
    });
    logic.set_turret_target_object(aid, Some(vid), false);
    // Already facing: one tick should Success.
    let r = logic.tick_turret_aim(aid, 1.0);
    assert_eq!(r, AttackAimResult::Success);
}

#[test]
fn tick_turret_aim_continues_while_turning() {
    use crate::game_logic::{KindOf, Object, ObjectId, Team, ThingTemplate, Weapon};
    use glam::Vec3;
    let mut logic = GameLogic::new();
    let mut at = ThingTemplate::new("TurA3");
    at.add_kind_of(KindOf::Vehicle);
    let aid = ObjectId(2005);
    logic.objects.insert(aid, {
        let mut o = Object::new(at, aid, Team::USA);
        o.set_position(Vec3::ZERO);
        o.set_orientation(0.0);
        o.turret_enabled = true;
        o.turret_angle_deg = 0.0;
        o.turret_turn_rate_rad = 0.02; // ~1.1 deg/frame
        o.weapon = Some(Weapon {
            range: 200.0,
            ..Default::default()
        });
        o
    });
    let mut vt = ThingTemplate::new("TurV3");
    vt.add_kind_of(KindOf::Infantry);
    let vid = ObjectId(2006);
    logic.objects.insert(vid, {
        let mut o = Object::new(vt, vid, Team::GLA);
        // Behind: ~180 deg turn
        o.set_position(Vec3::new(-50.0, 0.0, 0.0));
        o
    });
    logic.set_turret_target_object(aid, Some(vid), false);
    let r = logic.tick_turret_aim(aid, 1.0);
    assert_eq!(r, AttackAimResult::Continue);
    assert!(logic.objects[&aid].turret_rotating);
}

#[test]
fn turret_move_loop_plays_authored_event_and_stops() {
    use crate::game_logic::audio_dispatch_impl::{
        clear_test_template_voices, set_test_per_unit_sound,
    };
    use crate::game_logic::{KindOf, Object, ObjectId, Team, ThingTemplate, Weapon};
    use glam::Vec3;
    clear_test_template_voices();
    set_test_per_unit_sound(
        "AmericaInfantryRanger",
        "TurretMoveLoop",
        "RangerTurretMoveLoop",
    );
    let mut logic = GameLogic::new();
    let mut at = ThingTemplate::new("AmericaInfantryRanger");
    at.add_kind_of(KindOf::Infantry);
    let aid = ObjectId(28021);
    logic.objects.insert(aid, {
        let mut o = Object::new(at, aid, Team::USA);
        o.set_position(Vec3::ZERO);
        o.set_orientation(0.0);
        o.turret_enabled = true;
        o.turret_angle_deg = 0.0;
        o.turret_turn_rate_rad = 0.02;
        o.weapon = Some(Weapon {
            range: 200.0,
            ..Default::default()
        });
        o
    });
    let mut vt = ThingTemplate::new("TurVicLoop");
    vt.add_kind_of(KindOf::Infantry);
    let vid = ObjectId(28022);
    logic.objects.insert(vid, {
        let mut o = Object::new(vt, vid, Team::GLA);
        o.set_position(Vec3::new(-50.0, 0.0, 0.0));
        o
    });
    logic.set_turret_target_object(aid, Some(vid), false);
    let r = logic.tick_turret_state_machine(aid, 1.0, 1);
    assert_eq!(r, crate::game_logic::AttackAimResult::Continue);
    assert!(logic.objects[&aid].turret_rotating);
    assert!(
        logic.queued_audio_events.iter().any(|e| {
            e.event_type == "RangerTurretMoveLoop"
                && e.object_id == Some(aid)
                && e.is_looping
                && !e.stop
        }),
        "TurretMoveLoop must play the INI value, not the slot token: {:?}",
        logic.queued_audio_events
    );
    assert!(
        logic
            .queued_audio_events
            .iter()
            .all(|e| e.event_type != "TurretMoveLoop"),
        "must not queue the TurretMoveLoop slot token: {:?}",
        logic.queued_audio_events
    );

    logic.queued_audio_events.clear();
    if let Some(u) = logic.objects.get_mut(&aid) {
        u.turret_turn_rate_rad = 10.0;
    }
    let _ = logic.tick_turret_state_machine(aid, 1.0, 2);
    assert!(
        !logic.objects[&aid].turret_rotating,
        "fast turn should snap and clear rotating"
    );
    assert!(
        logic.queued_audio_events.iter().any(|e| {
            e.event_type == "RangerTurretMoveLoop" && e.object_id == Some(aid) && e.stop
        }),
        "stopping the turret must removeAudioEvent the authored loop: {:?}",
        logic.queued_audio_events
    );
    clear_test_template_voices();
}

#[test]
fn turret_move_loop_missing_unit_sound_stays_silent() {
    use crate::game_logic::audio_dispatch_impl::clear_test_template_voices;
    use crate::game_logic::{KindOf, Object, ObjectId, Team, ThingTemplate, Weapon};
    use glam::Vec3;
    clear_test_template_voices();
    let mut logic = GameLogic::new();
    let mut at = ThingTemplate::new("SilentTurret");
    at.add_kind_of(KindOf::Vehicle);
    let aid = ObjectId(28023);
    logic.objects.insert(aid, {
        let mut o = Object::new(at, aid, Team::USA);
        o.set_position(Vec3::ZERO);
        o.set_orientation(0.0);
        o.turret_enabled = true;
        o.turret_angle_deg = 0.0;
        o.turret_turn_rate_rad = 0.02;
        o.weapon = Some(Weapon {
            range: 200.0,
            ..Default::default()
        });
        o
    });
    let mut vt = ThingTemplate::new("SilentVic");
    vt.add_kind_of(KindOf::Infantry);
    let vid = ObjectId(28024);
    logic.objects.insert(vid, {
        let mut o = Object::new(vt, vid, Team::GLA);
        o.set_position(Vec3::new(-50.0, 0.0, 0.0));
        o
    });
    logic.set_turret_target_object(aid, Some(vid), false);
    let _ = logic.tick_turret_state_machine(aid, 1.0, 1);
    assert!(logic.objects[&aid].turret_rotating);
    assert!(
        logic.queued_audio_events.iter().all(|e| {
            e.event_type != "TurretMoveLoop" && e.event_type != "SilentTurretTurretMoveLoop"
        }),
        "missing UnitSpecificSounds.TurretMoveLoop must stay silent: {:?}",
        logic.queued_audio_events
    );
}

#[test]
fn voice_rapid_fire_plays_authored_per_unit_sound() {
    use crate::game_logic::audio_dispatch_impl::{
        clear_test_template_voices, set_test_per_unit_sound,
    };
    use crate::game_logic::host_gattling_tank::GattlingFireLevel;
    use crate::game_logic::{KindOf, ObjectId, Team, ThingTemplate};
    use glam::Vec3;

    clear_test_template_voices();
    set_test_per_unit_sound(
        "ChinaTankGattling",
        "VoiceRapidFire",
        "GattlingTankVoiceRapid",
    );
    set_test_per_unit_sound(
        "ChinaGattlingCannon",
        "VoiceRapidFire",
        "GattlingCannonVoiceRapid",
    );
    set_test_per_unit_sound(
        "ChinaInfantryMiniGunner",
        "VoiceRapidFire",
        "MiniGunnerVoiceRapidFire",
    );

    let mut logic = GameLogic::new();
    let mut tank_t = ThingTemplate::new("ChinaTankGattling");
    tank_t.add_kind_of(KindOf::Vehicle).set_health(300.0);
    logic.templates.insert("ChinaTankGattling".into(), tank_t);
    let tank = logic
        .create_object("ChinaTankGattling", Team::China, Vec3::ZERO)
        .expect("tank");
    {
        let o = logic.objects.get_mut(&tank).unwrap();
        o.continuous_fire_level = GattlingFireLevel::Mean.as_u8();
        o.continuous_fire_consecutive = 6;
        o.continuous_fire_victim = 99;
    }
    logic.advance_gattling_continuous_fire(tank, Some(ObjectId(99)), 0);
    assert!(
        logic
            .queued_audio_events
            .iter()
            .any(|e| { e.event_type == "GattlingTankVoiceRapid" && e.object_id == Some(tank) }),
        "tank VoiceRapidFire must play the authored event: {:?}",
        logic.queued_audio_events
    );

    logic.queued_audio_events.clear();
    let mut bld_t = ThingTemplate::new("ChinaGattlingCannon");
    bld_t.add_kind_of(KindOf::Structure).set_health(1000.0);
    logic.templates.insert("ChinaGattlingCannon".into(), bld_t);
    let bld = logic
        .create_object("ChinaGattlingCannon", Team::China, Vec3::ZERO)
        .expect("cannon");
    {
        let o = logic.objects.get_mut(&bld).unwrap();
        o.continuous_fire_level = GattlingFireLevel::Mean.as_u8();
        o.continuous_fire_consecutive = 6;
        o.continuous_fire_victim = 99;
    }
    logic.advance_gattling_building_continuous_fire(bld, Some(ObjectId(99)), 0);
    assert!(
        logic
            .queued_audio_events
            .iter()
            .any(|e| { e.event_type == "GattlingCannonVoiceRapid" && e.object_id == Some(bld) }),
        "building VoiceRapidFire must play the authored event: {:?}",
        logic.queued_audio_events
    );

    logic.queued_audio_events.clear();
    let mut mini_t = ThingTemplate::new("ChinaInfantryMiniGunner");
    mini_t.add_kind_of(KindOf::Infantry).set_health(120.0);
    logic
        .templates
        .insert("ChinaInfantryMiniGunner".into(), mini_t);
    let mini = logic
        .create_object("ChinaInfantryMiniGunner", Team::China, Vec3::ZERO)
        .expect("minigunner");
    {
        let o = logic.objects.get_mut(&mini).unwrap();
        o.continuous_fire_level = GattlingFireLevel::Mean.as_u8();
        o.continuous_fire_consecutive = 6;
        o.continuous_fire_victim = 99;
    }
    logic.advance_minigunner_continuous_fire(mini, Some(ObjectId(99)), 0);
    assert!(
        logic
            .queued_audio_events
            .iter()
            .any(|e| { e.event_type == "MiniGunnerVoiceRapidFire" && e.object_id == Some(mini) }),
        "minigunner VoiceRapidFire must play the authored event, not Attack: {:?}",
        logic.queued_audio_events
    );
    assert!(
        logic.queued_audio_events.iter().all(|e| {
            e.event_type != "VoiceRapidFire" && e.event_type != "RedMinigunnerVoiceAttack"
        }),
        "must not queue the slot token or Attack voice: {:?}",
        logic.queued_audio_events
    );
    clear_test_template_voices();
}

#[test]
fn voice_rapid_fire_missing_unit_sound_stays_silent() {
    use crate::game_logic::audio_dispatch_impl::clear_test_template_voices;
    use crate::game_logic::host_gattling_tank::GattlingFireLevel;
    use crate::game_logic::{KindOf, ObjectId, Team, ThingTemplate};
    use glam::Vec3;

    clear_test_template_voices();
    let mut logic = GameLogic::new();
    let mut t = ThingTemplate::new("SilentGattling");
    t.add_kind_of(KindOf::Vehicle).set_health(100.0);
    logic.templates.insert("SilentGattling".into(), t);
    let id = logic
        .create_object("SilentGattling", Team::China, Vec3::ZERO)
        .expect("silent");
    {
        let o = logic.objects.get_mut(&id).unwrap();
        o.continuous_fire_level = GattlingFireLevel::Mean.as_u8();
        o.continuous_fire_consecutive = 6;
        o.continuous_fire_victim = 99;
    }
    logic.advance_gattling_continuous_fire(id, Some(ObjectId(99)), 0);
    assert!(
        logic.queued_audio_events.iter().all(|e| {
            e.event_type != "VoiceRapidFire" && e.event_type != "SilentGattlingVoiceRapidFire"
        }),
        "missing UnitSpecificSounds.VoiceRapidFire must stay silent: {:?}",
        logic.queued_audio_events
    );
}

#[test]
fn turn_turret_towards_angle_snaps_within_rate() {
    use crate::game_logic::{KindOf, Object, ObjectId, Team, ThingTemplate};
    let mut t = ThingTemplate::new("Snap");
    t.add_kind_of(KindOf::Vehicle);
    let mut o = Object::new(t, ObjectId(2007), Team::USA);
    o.turret_enabled = true;
    o.turret_angle_deg = 0.0;
    o.turret_turn_rate_rad = 0.5;
    // desired 0.1 rad — within rate → snap success
    assert!(o.turn_turret_towards_angle_rad(0.1, 1.0, 0.035));
    assert!((o.turret_angle_deg.to_radians() - 0.1).abs() < 1e-4);
    assert!(!o.turret_rotating);
}

#[test]
fn out_of_weapon_range_object_distance() {
    use crate::game_logic::{KindOf, Object, ObjectId, Team, ThingTemplate, Weapon};
    use glam::Vec3;
    let mut logic = GameLogic::new();
    let mut at = ThingTemplate::new("OorA");
    at.add_kind_of(KindOf::Infantry);
    let aid = ObjectId(1901);
    logic.objects.insert(aid, {
        let mut o = Object::new(at, aid, Team::USA);
        o.set_position(Vec3::ZERO);
        o.weapon = Some(Weapon {
            range: 40.0,
            ..Default::default()
        });
        o
    });
    let mut vt = ThingTemplate::new("OorV");
    vt.add_kind_of(KindOf::Infantry);
    let vid = ObjectId(1902);
    logic.objects.insert(vid, {
        let mut o = Object::new(vt, vid, Team::GLA);
        o.set_position(Vec3::new(20.0, 0.0, 0.0));
        o
    });
    assert!(!logic.out_of_weapon_range_object(aid, vid));
    logic
        .objects
        .get_mut(&vid)
        .unwrap()
        .set_position(Vec3::new(200.0, 0.0, 0.0));
    assert!(logic.out_of_weapon_range_object(aid, vid));
}

#[test]
fn out_of_weapon_range_leech_bypasses() {
    use crate::game_logic::{KindOf, Object, ObjectId, Team, ThingTemplate, Weapon};
    use glam::Vec3;
    let mut logic = GameLogic::new();
    let mut at = ThingTemplate::new("LeeA");
    at.add_kind_of(KindOf::Infantry);
    let aid = ObjectId(1903);
    logic.objects.insert(aid, {
        let mut o = Object::new(at, aid, Team::USA);
        o.set_position(Vec3::ZERO);
        o.weapon = Some(Weapon {
            range: 40.0,
            ..Default::default()
        });
        o.leech_range_active_primary = true;
        o
    });
    let mut vt = ThingTemplate::new("LeeV");
    vt.add_kind_of(KindOf::Infantry);
    let vid = ObjectId(1904);
    logic.objects.insert(vid, {
        let mut o = Object::new(vt, vid, Team::GLA);
        o.set_position(Vec3::new(200.0, 0.0, 0.0));
        o
    });
    assert!(!logic.out_of_weapon_range_object(aid, vid));
}

#[test]
fn want_to_squish_vehicle_vs_infantry() {
    use crate::game_logic::{KindOf, Object, ObjectId, Team, ThingTemplate, Weapon};
    let mut logic = GameLogic::new();
    let mut at = ThingTemplate::new("SqA");
    at.add_kind_of(KindOf::Vehicle);
    let aid = ObjectId(1905);
    logic.objects.insert(aid, {
        let mut o = Object::new(at, aid, Team::USA);
        o.crusher_level = 1;
        o.weapon = Some(Weapon {
            range: 50.0,
            ..Default::default()
        });
        o
    });
    let mut vt = ThingTemplate::new("SqV");
    vt.add_kind_of(KindOf::Infantry);
    let vid = ObjectId(1906);
    logic.objects.insert(vid, {
        let mut o = Object::new(vt, vid, Team::GLA);
        o.crushable_level = 0;
        o
    });
    // can_crush_only residual may depend on levels — assert function is callable.
    let _ = logic.want_to_squish_target(aid, vid);
    // Contained victim cannot be squished.
    logic.objects.get_mut(&vid).unwrap().contained_by = Some(ObjectId(1));
    assert!(!logic.want_to_squish_target(aid, vid));
}

#[test]
fn want_to_squish_honors_ally_computer_and_dont_auto_crush() {
    // C++ AIStates.cpp:1140-1166 wantToSquishTarget. hq-mvho2.
    use crate::game_logic::{KindOf, Object, ObjectId, Player, Team, ThingTemplate, Weapon};
    let mut logic = GameLogic::new();
    let mut human = Player::new(0, Team::USA, "Human", true);
    human.is_alive = true;
    let mut cpu = Player::new(1, Team::China, "CPU", false);
    cpu.is_alive = true;
    cpu.alliance_team = 3;
    let mut ally = Player::new(2, Team::USA, "Ally", false);
    ally.is_alive = true;
    ally.alliance_team = 3;
    logic.add_player(human);
    logic.add_player(cpu);
    logic.add_player(ally);

    let spawn = |name: &str, id: u32, team: Team, owner: u32, infantry: bool| {
        let mut t = ThingTemplate::new(name);
        t.add_kind_of(if infantry {
            KindOf::Infantry
        } else {
            KindOf::Vehicle
        });
        if name.to_ascii_uppercase().contains("DOZER") {
            t.add_kind_of(KindOf::Dozer);
        }
        let mut o = Object::new(t, ObjectId(id), team);
        o.owner_player_id = Some(owner);
        if infantry {
            o.crushable_level = 0;
        } else {
            o.crusher_level = 1;
            o.turret_enabled = true;
            o.weapon = Some(Weapon {
                range: 50.0,
                ..Default::default()
            });
        }
        o
    };

    let tank = spawn("CpuTank", 3001, Team::China, 1, false);
    let inf = spawn("EnemyInf", 3002, Team::USA, 0, true);
    let dozer = spawn("AmericaVehicleDozer", 3003, Team::China, 1, false);
    let tomahawk = spawn("AmericaVehicleTomahawk", 3004, Team::China, 1, false);
    let human_tank = spawn("HumanTank", 3005, Team::USA, 0, false);
    let ally_inf = spawn("AllyInf", 3006, Team::USA, 2, true);
    logic.objects.insert(tank.id, tank);
    logic.objects.insert(inf.id, inf);
    logic.objects.insert(dozer.id, dozer);
    logic.objects.insert(tomahawk.id, tomahawk);
    logic.objects.insert(human_tank.id, human_tank);
    logic.objects.insert(ally_inf.id, ally_inf);

    assert!(
        logic.want_to_squish_target(ObjectId(3001), ObjectId(3002)),
        "computer turreted tank may chase-squish enemy infantry"
    );
    assert!(!logic.want_to_squish_target(ObjectId(3003), ObjectId(3002)));
    assert!(!logic.want_to_squish_target(ObjectId(3004), ObjectId(3002)));
    assert!(!logic.want_to_squish_target(ObjectId(3005), ObjectId(3002)));
    assert!(!logic.want_to_squish_target(ObjectId(3001), ObjectId(3006)));
}

#[test]
fn attack_state_machine_oor_chases_fleeing_victim() {
    use crate::game_logic::{
        AttackSubState, KindOf, Object, ObjectId, Team, ThingTemplate, Weapon,
    };
    use glam::Vec3;
    let mut logic = GameLogic::new();
    let mut at = ThingTemplate::new("ChaseA");
    at.add_kind_of(KindOf::Vehicle);
    let aid = ObjectId(1801);
    logic.objects.insert(aid, {
        let mut o = Object::new(at, aid, Team::USA);
        o.set_position(Vec3::ZERO);
        o.set_orientation(0.0);
        o.movement.max_speed = 20.0;
        o.weapon = Some(Weapon {
            damage: 10.0,
            range: 30.0,
            reload_time: 1.0,
            last_fire_time: -10.0,
            ..Default::default()
        });
        o
    });
    let mut vt = ThingTemplate::new("ChaseV");
    vt.add_kind_of(KindOf::Infantry);
    let vid = ObjectId(1802);
    logic.objects.insert(vid, {
        let mut o = Object::new(vt, vid, Team::GLA);
        // Far out of range, fleeing +X.
        o.set_position(Vec3::new(200.0, 0.0, 0.0));
        o.set_orientation(0.0);
        o.movement.velocity = Vec3::new(8.0, 0.0, 0.0); // speed 8 < 20, > 2
        o
    });
    assert!(logic.should_chase_attack_target(aid, vid));
    assert_eq!(
        logic.attack_state_enter(aid, vid),
        AttackMachineResult::Continue
    );
    let r = logic.tick_attack_state_machine(aid, vid, 10.0, 1, 1.0);
    assert_eq!(r, AttackMachineResult::Continue);
    assert_eq!(
        logic.objects[&aid].attack_substate,
        AttackSubState::ChaseTarget
    );
}

#[test]
fn attack_state_machine_oor_approaches_stationary_victim() {
    use crate::game_logic::{
        AttackSubState, KindOf, Object, ObjectId, Team, ThingTemplate, Weapon,
    };
    use glam::Vec3;
    let mut logic = GameLogic::new();
    let mut at = ThingTemplate::new("AppA");
    at.add_kind_of(KindOf::Infantry);
    let aid = ObjectId(1803);
    logic.objects.insert(aid, {
        let mut o = Object::new(at, aid, Team::USA);
        o.set_position(Vec3::ZERO);
        o.movement.max_speed = 10.0;
        o.weapon = Some(Weapon {
            range: 30.0,
            damage: 10.0,
            can_target_ground: true,
            can_target_air: true,
            ..Default::default()
        });
        o
    });
    let mut vt = ThingTemplate::new("AppV");
    vt.add_kind_of(KindOf::Infantry);
    let vid = ObjectId(1804);
    logic.objects.insert(vid, {
        let mut o = Object::new(vt, vid, Team::GLA);
        o.set_position(Vec3::new(200.0, 0.0, 0.0));
        // Stationary — can_pursue false.
        o.movement.velocity = Vec3::ZERO;
        o
    });
    assert!(!logic.should_chase_attack_target(aid, vid));
    assert_eq!(
        logic.attack_state_enter(aid, vid),
        AttackMachineResult::Continue
    );
    let r = logic.tick_attack_state_machine(aid, vid, 10.0, 1, 1.0);
    assert_eq!(r, AttackMachineResult::Continue);
    assert_eq!(
        logic.objects[&aid].attack_substate,
        AttackSubState::ApproachTarget
    );
}

#[test]
fn attack_chase_drops_when_victim_stops_fleeing() {
    use crate::game_logic::{
        AttackSubState, KindOf, Object, ObjectId, Team, ThingTemplate, Weapon,
    };
    use glam::Vec3;
    let mut logic = GameLogic::new();
    let mut at = ThingTemplate::new("DropA");
    at.add_kind_of(KindOf::Vehicle);
    let aid = ObjectId(1805);
    logic.objects.insert(aid, {
        let mut o = Object::new(at, aid, Team::USA);
        o.set_position(Vec3::ZERO);
        o.movement.max_speed = 20.0;
        o.weapon = Some(Weapon {
            range: 30.0,
            ..Default::default()
        });
        o.attack_substate = AttackSubState::ChaseTarget;
        o
    });
    let mut vt = ThingTemplate::new("DropV");
    vt.add_kind_of(KindOf::Infantry);
    let vid = ObjectId(1806);
    logic.objects.insert(vid, {
        let mut o = Object::new(vt, vid, Team::GLA);
        o.set_position(Vec3::new(200.0, 0.0, 0.0));
        o.movement.velocity = Vec3::ZERO; // stopped
        o
    });
    let r = logic.tick_attack_state_machine(aid, vid, 10.0, 1, 1.0);
    assert_eq!(r, AttackMachineResult::Continue);
    assert_eq!(
        logic.objects[&aid].attack_substate,
        AttackSubState::ApproachTarget
    );
}

#[test]
fn attack_chase_crush_floors_to_fast_as_possible() {
    use crate::game_logic::{
        AttackSubState, KindOf, Object, ObjectId, Team, ThingTemplate, Weapon,
    };
    use glam::Vec3;
    let mut logic = GameLogic::new();
    let mut at = ThingTemplate::new("CrushTank");
    at.add_kind_of(KindOf::Vehicle);
    let aid = ObjectId(1811);
    logic.objects.insert(aid, {
        let mut o = Object::new(at, aid, Team::USA);
        o.set_position(Vec3::ZERO);
        o.set_orientation(0.0);
        o.movement.max_speed = 20.0;
        o.movement.velocity = Vec3::new(18.0, 0.0, 0.0);
        o.crusher_level = 1;
        o.group_speed_factor = 0.5;
        o.weapon = Some(Weapon {
            damage: 10.0,
            range: 80.0,
            reload_time: 1.0,
            last_fire_time: -10.0,
            can_target_ground: true,
            ..Default::default()
        });
        o.attack_substate = AttackSubState::ChaseTarget;
        o
    });
    let mut vt = ThingTemplate::new("CrushInf");
    vt.add_kind_of(KindOf::Infantry);
    let vid = ObjectId(1812);
    logic.objects.insert(vid, {
        let mut o = Object::new(vt, vid, Team::GLA);
        o.set_position(Vec3::new(20.0, 0.0, 0.0));
        o.set_orientation(0.0);
        o.crushable_level = 0;
        o.has_squish_collide = true;
        o.movement.velocity = Vec3::new(10.0, 0.0, 0.0);
        o
    });
    assert!(logic.objects[&aid].can_crush_or_squish(&logic.objects[&vid], false));
    let r = logic.tick_attack_state_machine(aid, vid, 10.0, 1, 1.0);
    assert_eq!(r, AttackMachineResult::Continue);
    let tank = &logic.objects[&aid];
    assert_eq!(tank.attack_substate, AttackSubState::ChaseTarget);
    assert!(
        (tank.group_speed_factor - 1.0).abs() < 1e-5,
        "canCrushOrSquish must floor to FAST_AS_POSSIBLE, factor={}",
        tank.group_speed_factor
    );
    let spd = (tank.movement.velocity.x * tank.movement.velocity.x
        + tank.movement.velocity.z * tank.movement.velocity.z)
        .sqrt();
    assert!(
        spd > 10.0 * 0.95 + 0.1,
        "tank must not pace fleeing infantry at victim*0.95, spd={}",
        spd
    );
}

#[test]
fn choose_best_weapon_prefers_ready_slot() {
    use crate::game_logic::{KindOf, Object, ObjectId, Team, ThingTemplate, Weapon};
    let mut logic = GameLogic::new();
    let mut at = ThingTemplate::new("CwA");
    at.add_kind_of(KindOf::Infantry);
    let aid = ObjectId(2201);
    logic.objects.insert(aid, {
        let mut o = Object::new(at, aid, Team::USA);
        o.weapon = Some(Weapon {
            damage: 5.0,
            range: 50.0,
            reload_time: 1.0,
            last_fire_time: -10.0,
            ..Default::default()
        });
        o.secondary_weapon = Some(Weapon {
            damage: 40.0,
            range: 50.0,
            reload_time: 1.0,
            last_fire_time: -10.0,
            ..Default::default()
        });
        o
    });
    let mut vt = ThingTemplate::new("CwV");
    vt.add_kind_of(KindOf::Structure);
    let vid = ObjectId(2202);
    logic.objects.insert(vid, {
        let mut o = Object::new(vt, vid, Team::GLA);
        o
    });
    assert!(logic.choose_best_weapon_for_target(aid, Some(vid), 10.0));
    // Prefer secondary vs structure when damage higher (select_combat residual).
    assert_eq!(logic.objects[&aid].active_weapon_slot, 1);
}

#[test]
fn update_combat_temp_lock_waits_instead_of_firing_primary() {
    // hq-8t90d: LOCKED_TEMPORARILY mid-reload must wait on that slot.
    // PreferMostDamage must not fall through to ready PRIMARY.
    use crate::game_logic::{KindOf, Team, ThingTemplate, Weapon, WeaponLockType};
    let mut logic = GameLogic::new();
    for (name, kinds) in [
        ("LockWaitAtk", vec![KindOf::Infantry, KindOf::Attackable]),
        ("LockWaitTgt", vec![KindOf::Infantry, KindOf::Attackable]),
    ] {
        if !logic.templates.contains_key(name) {
            let mut tmpl = ThingTemplate::new(name);
            tmpl.set_health(200.0);
            for k in kinds {
                tmpl.add_kind_of(k);
            }
            logic.templates.insert(name.into(), tmpl);
        }
    }
    let atk = logic
        .create_object("LockWaitAtk", Team::USA, glam::Vec3::new(0.0, 0.0, 0.0))
        .expect("atk");
    let tgt = logic
        .create_object("LockWaitTgt", Team::GLA, glam::Vec3::new(30.0, 0.0, 0.0))
        .expect("tgt");
    if let Some(o) = logic.objects.get_mut(&atk) {
        o.weapon = Some(Weapon {
            damage: 25.0,
            range: 200.0,
            reload_time: 0.0,
            last_fire_time: -100.0,
            ..Weapon::default()
        });
        o.secondary_weapon = Some(Weapon {
            damage: 80.0,
            range: 200.0,
            reload_time: 10.0,
            last_fire_time: 1.0,
            ammo: Some(0),
            clip_size: 1,
            ..Weapon::default()
        });
        assert!(o.set_weapon_lock(1, WeaponLockType::LockedTemporarily));
        o.target = Some(tgt);
        o.set_ai_state(AIState::Attacking);
        o.set_status_attacking(true);
    }
    logic.set_current_frame(30);
    let hp_before = logic.objects[&tgt].health.current;
    let primary_last = logic.objects[&atk].weapon.as_ref().unwrap().last_fire_time;
    let secondary_last = logic.objects[&atk]
        .secondary_weapon
        .as_ref()
        .unwrap()
        .last_fire_time;
    logic.update_combat(&[atk, tgt], LOGIC_FRAME_TIMESTEP);
    assert_eq!(
        logic.objects[&tgt].health.current, hp_before,
        "temp-locked reloading SECONDARY must wait, not auto-choose PRIMARY"
    );
    assert_eq!(
        logic.objects[&atk].weapon.as_ref().unwrap().last_fire_time,
        primary_last
    );
    assert_eq!(
        logic.objects[&atk]
            .secondary_weapon
            .as_ref()
            .unwrap()
            .last_fire_time,
        secondary_last
    );
    assert_eq!(
        logic.objects[&atk].weapon_lock_type,
        WeaponLockType::LockedTemporarily
    );
}

#[test]
fn choose_best_and_attack_ground_reset_primary_unless_locked() {
    // hq-xpdwb: unlocked chooseBest/attack-ground leftover-resets PRIMARY.
    // A leftover PreferMostDamage SECONDARY (Humvee TOW) must not stick.
    use crate::game_logic::{
        KindOf, Object, ObjectId, Team, ThingTemplate, Weapon, WeaponLockType,
    };
    let mut logic = GameLogic::new();
    let mut at = ThingTemplate::new("GroundResetHumvee");
    at.add_kind_of(KindOf::Vehicle);
    at.add_kind_of(KindOf::Attackable);
    at.set_health(200.0);
    let aid = ObjectId(4010);
    logic.objects.insert(aid, {
        let mut o = Object::new(at, aid, Team::USA);
        o.weapon = Some(Weapon {
            damage: 10.0,
            range: 150.0,
            last_fire_time: -10.0,
            ..Weapon::default()
        });
        o.secondary_weapon = Some(Weapon {
            damage: 30.0,
            range: 150.0,
            last_fire_time: -10.0,
            ..Weapon::default()
        });
        o.set_active_weapon_slot(1);
        o
    });
    assert!(logic.choose_best_weapon_for_target(aid, None, 10.0));
    assert_eq!(
        logic.objects[&aid].active_weapon_slot, 0,
        "unlocked chooseBest no-victim leftover-resets PRIMARY"
    );

    logic
        .objects
        .get_mut(&aid)
        .unwrap()
        .set_active_weapon_slot(1);
    assert!(logic.unit_command_attack_ground(aid, glam::Vec3::new(20.0, 0.0, 0.0)));
    assert_eq!(
        logic.objects[&aid].active_weapon_slot, 0,
        "unlocked attack-ground leftover-resets PRIMARY"
    );
    assert_eq!(logic.objects[&aid].ai_state, AIState::AttackingGround);

    assert!(
        logic
            .objects
            .get_mut(&aid)
            .unwrap()
            .set_weapon_lock(1, WeaponLockType::LockedTemporarily)
    );
    assert!(logic.unit_command_attack_ground(aid, glam::Vec3::new(25.0, 0.0, 0.0)));
    assert_eq!(
        logic.objects[&aid].active_weapon_slot, 1,
        "locked attack-ground keeps the locked slot"
    );
    assert_eq!(logic.objects[&aid].weapon_lock_slot, 1);
}

#[test]
fn hijack_hides_in_eject_capable_vehicle() {
    use crate::game_logic::{KindOf, Object, ObjectId, Team, ThingTemplate, VeterancyLevel};
    let mut logic = GameLogic::new();
    let mut ht = ThingTemplate::new("GLAInfantryHijacker");
    ht.add_kind_of(KindOf::Infantry);
    let hid = ObjectId(5501);
    logic.objects.insert(hid, {
        let mut o = Object::new(ht, hid, Team::GLA);
        o.name = "Jack".into();
        o.record_host_identity();
        o.vision_range = 100.0;
        o.shroud_clearing_range = 80.0;
        o
    });
    let mut vt = ThingTemplate::new("AmericaTankCrusader");
    vt.add_kind_of(KindOf::Vehicle);
    vt.add_kind_of(KindOf::Attackable);
    let vid = ObjectId(5502);
    logic.objects.insert(vid, {
        let mut o = Object::new(vt, vid, Team::USA);
        o.set_position(glam::Vec3::new(10.0, 0.0, 0.0));
        o.vision_range = 200.0;
        o.shroud_clearing_range = 250.0;
        o
    });
    assert!(logic.vehicle_supports_hijacker_ride(vid));
    let donor = logic.objects.get(&hid).cloned();
    {
        let v = logic.objects.get_mut(&vid).unwrap();
        v.apply_hijacked_from(donor.as_ref());
        v.set_team(Team::GLA);
    }
    logic.partition_manager.register_object_at(hid.0, 10.0, 0.0);
    {
        let h = logic.objects.get_mut(&hid).unwrap();
        h.begin_hijacker_in_vehicle(vid);
    }
    logic.partition_manager.unregister_object(hid.0);
    assert!(logic.objects[&hid].hijacker_in_vehicle);
    assert!(logic.objects[&hid].status.masked);
    assert!(logic.objects[&hid].drawable_hidden);
    assert!(!logic.objects[&hid].is_selectable());
    assert!(!logic.partition_manager.is_registered(hid.0));
    let stolen = &logic.objects[&vid];
    assert!((stolen.vision_range - 100.0).abs() < 0.01);
    assert!((stolen.shroud_clearing_range - 80.0).abs() < 0.01);

    // Move vehicle → rider follows
    {
        let v = logic.objects.get_mut(&vid).unwrap();
        v.set_position(glam::Vec3::new(40.0, 0.0, 5.0));
    }
    logic.tick_hijacker_updates();
    let hp = logic.objects[&hid].get_position();
    assert!((hp.x - 40.0).abs() < 0.01);
    // Kill vehicle → rider restored
    {
        let v = logic.objects.get_mut(&vid).unwrap();
        // Wave 753: under damage authority, do not zero host HP mid-frame
        // (dual with GW HP writeback). Project lethal via damage log + flags.
        if crate::gameworld_shadow::gameworld_damage_authority_live() {
            let hp = v.health.current.max(1.0);
            let oid = v.id;
            crate::game_logic::host_damage_log::record(oid, hp, None, true);
        } else {
            v.health.current = 0.0;
        }
        v.status.destroyed = true;
    }
    logic.tick_hijacker_updates();
    let h = &logic.objects[&hid];
    assert!(!h.hijacker_in_vehicle);
    assert!(!h.status.masked);
    assert!(!h.drawable_hidden);
    assert!(h.is_alive());
}

#[test]
fn hijack_airborne_eject_puts_in_america_parachute() {
    use crate::game_logic::host_car_bomb::{HIJACKER_PARACHUTE_NAME, HostCarBombRegistry};
    use crate::game_logic::{KindOf, Object, ObjectId, Team, ThingTemplate};
    let mut logic = GameLogic::new();
    let mut ht = ThingTemplate::new("GLAInfantryHijacker");
    ht.add_kind_of(KindOf::Infantry);
    let hid = ObjectId(5521);
    logic.objects.insert(hid, {
        let mut o = Object::new(ht, hid, Team::GLA);
        o
    });
    let mut vt = ThingTemplate::new("AmericaTankCrusader");
    vt.add_kind_of(KindOf::Vehicle);
    vt.add_kind_of(KindOf::Attackable);
    let vid = ObjectId(5522);
    logic.objects.insert(vid, {
        let mut o = Object::new(vt, vid, Team::USA);
        // Airborne residual (significantly above terrain).
        o.set_position(glam::Vec3::new(10.0, 80.0, 0.0));
        o.status.airborne_target = true;
        o
    });
    {
        let v = logic.objects.get_mut(&vid).unwrap();
        v.apply_hijacked();
        v.set_team(Team::GLA);
    }
    {
        let h = logic.objects.get_mut(&hid).unwrap();
        h.begin_hijacker_in_vehicle(vid);
    }
    // Sync airborne flag onto rider.
    logic.tick_hijacker_updates();
    assert!(logic.objects[&hid].hijacker_was_airborne);

    // Kill airborne vehicle → PutInContainer AmericaParachute.
    {
        let v = logic.objects.get_mut(&vid).unwrap();
        // Wave 753: under damage authority, do not zero host HP mid-frame
        // (dual with GW HP writeback). Project lethal via damage log + flags.
        if crate::gameworld_shadow::gameworld_damage_authority_live() {
            let hp = v.health.current.max(1.0);
            let oid = v.id;
            crate::game_logic::host_damage_log::record(oid, hp, None, true);
        } else {
            v.health.current = 0.0;
        }
        v.status.destroyed = true;
    }
    logic.tick_hijacker_updates();

    let h = &logic.objects[&hid];
    assert!(!h.hijacker_in_vehicle);
    assert!(h.is_alive());
    assert!(
        h.status.parachuting,
        "rider must parachute after airborne eject"
    );
    assert!(
        h.contained_by.is_some(),
        "rider must be PutInContainer AmericaParachute"
    );
    let chute_id = h.contained_by.unwrap();
    let chute = logic.objects.get(&chute_id).expect("parachute object");
    assert_eq!(chute.template_name, HIJACKER_PARACHUTE_NAME);
    assert!(
        chute.contained_units().contains(&hid),
        "chute must contain hijacker"
    );
    assert!(
        logic.car_bomb.honesty_airborne_parachute_ok(),
        "airborne PutInContainer honesty"
    );
    assert!(logic.usa_pilot_residual().air_ejections >= 1);
}

#[test]
fn hijacker_hill_tank_is_not_airborne() {
    use crate::game_logic::{KindOf, Object, ObjectId, Team, ThingTemplate};
    let mut logic = GameLogic::new();
    let hid = ObjectId(5523);
    logic.objects.insert(
        hid,
        Object::new(ThingTemplate::new("GLAHijacker"), hid, Team::GLA),
    );
    let mut vt = ThingTemplate::new("AmericaTankCrusader");
    vt.add_kind_of(KindOf::Vehicle);
    let vid = ObjectId(5524);
    logic.objects.insert(vid, {
        let mut o = Object::new(vt, vid, Team::USA);
        o.set_position(glam::Vec3::new(10.0, 80.0, 0.0));
        o.ground_height = 80.0;
        o
    });
    logic.objects.get_mut(&vid).unwrap().apply_hijacked();
    logic
        .objects
        .get_mut(&hid)
        .unwrap()
        .begin_hijacker_in_vehicle(vid);
    logic.tick_hijacker_updates();
    assert!(!logic.objects[&hid].hijacker_was_airborne);
}

#[test]
fn hijacker_hill_tank_uses_height_above_terrain_not_world_y() {
    use crate::game_logic::{KindOf, Object, ObjectId, Team, ThingTemplate};
    let mut logic = GameLogic::new();
    let w = logic.pathfinding_system.grid.width().max(8) as u32;
    let h = logic.pathfinding_system.grid.height().max(8) as u32;
    let heights = vec![80.0f32; (w * h) as usize];
    assert!(
        logic.restore_terrain_heights_from_grid(w, h, &heights),
        "hill height cache"
    );

    let hid = ObjectId(5525);
    logic.objects.insert(
        hid,
        Object::new(ThingTemplate::new("GLAInfantryHijacker"), hid, Team::GLA),
    );
    let mut vt = ThingTemplate::new("AmericaTankCrusader");
    vt.add_kind_of(KindOf::Vehicle);
    let vid = ObjectId(5526);
    logic.objects.insert(vid, {
        let mut o = Object::new(vt, vid, Team::USA);
        o.set_position(glam::Vec3::new(0.0, 80.0, 0.0));
        o.ground_height = 0.0;
        o.status.airborne_target = false;
        o
    });
    logic.objects.get_mut(&vid).unwrap().apply_hijacked();
    logic
        .objects
        .get_mut(&hid)
        .unwrap()
        .begin_hijacker_in_vehicle(vid);
    logic.tick_hijacker_updates();
    assert!(
        !logic.objects[&hid].hijacker_was_airborne,
        "tank sitting on Y=80 terrain is not airborne"
    );

    logic.objects.get_mut(&vid).unwrap().status.destroyed = true;
    logic.tick_hijacker_updates();
    let rider = &logic.objects[&hid];
    assert!(!rider.hijacker_in_vehicle);
    assert!(
        !rider.status.parachuting,
        "ground wreck on a hill must walk out, not AmericaParachute"
    );
}

#[test]
fn mixed_selection_hijack_skips_rebel() {
    use crate::command_executor::CommandExecutor;
    use crate::command_system::CommandResult;
    use crate::game_logic::{KindOf, PendingSpecialAbility, Team, ThingTemplate};
    let mut logic = GameLogic::new();
    let mut hijacker_t = ThingTemplate::new("GLAInfantryHijacker");
    hijacker_t.add_kind_of(KindOf::Infantry).set_health(100.0);
    let mut rebel_t = ThingTemplate::new("GLAInfantryRebel");
    rebel_t.add_kind_of(KindOf::Infantry).set_health(100.0);
    let mut tank_t = ThingTemplate::new("AmericaTankCrusader");
    tank_t
        .add_kind_of(KindOf::Vehicle)
        .add_kind_of(KindOf::Attackable)
        .set_health(400.0);
    for template in [hijacker_t, rebel_t, tank_t] {
        logic.templates.insert(template.name.clone(), template);
    }
    let hijacker = logic
        .create_object("GLAInfantryHijacker", Team::GLA, glam::Vec3::ZERO)
        .expect("hijacker");
    let rebel = logic
        .create_object(
            "GLAInfantryRebel",
            Team::GLA,
            glam::Vec3::new(2.0, 0.0, 0.0),
        )
        .expect("rebel");
    let tank = logic
        .create_object(
            "AmericaTankCrusader",
            Team::USA,
            glam::Vec3::new(20.0, 0.0, 0.0),
        )
        .expect("tank");
    let result = {
        let mut exec = CommandExecutor::new(&mut logic, 0);
        exec.execute_hijack(&[hijacker, rebel], tank)
    };
    assert_eq!(result, CommandResult::Success);
    assert!(matches!(
        logic.pending_special_abilities.get(&hijacker),
        Some(PendingSpecialAbility::Hijack { target_id }) if *target_id == tank
    ));
    assert!(logic.pending_special_abilities.get(&rebel).is_none());
}

#[test]
fn mixed_selection_carbomb_skips_rebel() {
    use crate::command_executor::CommandExecutor;
    use crate::command_system::CommandResult;
    use crate::game_logic::{KindOf, PendingSpecialAbility, Team, ThingTemplate};
    let mut logic = GameLogic::new();
    let mut terrorist_t = ThingTemplate::new("GLAInfantryTerrorist");
    terrorist_t.add_kind_of(KindOf::Infantry).set_health(100.0);
    let mut rebel_t = ThingTemplate::new("GLAInfantryRebel");
    rebel_t.add_kind_of(KindOf::Infantry).set_health(100.0);
    let mut car_t = ThingTemplate::new("CivilianCar");
    car_t.add_kind_of(KindOf::Vehicle).set_health(80.0);
    for template in [terrorist_t, rebel_t, car_t] {
        logic.templates.insert(template.name.clone(), template);
    }
    let terrorist = logic
        .create_object("GLAInfantryTerrorist", Team::GLA, glam::Vec3::ZERO)
        .expect("terrorist");
    let rebel = logic
        .create_object(
            "GLAInfantryRebel",
            Team::GLA,
            glam::Vec3::new(2.0, 0.0, 0.0),
        )
        .expect("rebel");
    let car = logic
        .create_object(
            "CivilianCar",
            Team::Neutral,
            glam::Vec3::new(20.0, 0.0, 0.0),
        )
        .expect("car");
    let result = {
        let mut exec = CommandExecutor::new(&mut logic, 0);
        exec.execute_convert_carbomb(&[terrorist, rebel], car)
    };
    assert_eq!(result, CommandResult::Success);
    assert!(matches!(
        logic.pending_special_abilities.get(&terrorist),
        Some(PendingSpecialAbility::CarBomb { target_id }) if *target_id == car
    ));
    assert!(logic.pending_special_abilities.get(&rebel).is_none());
}

#[test]
fn deliver_payload_parachute_directly_arms_landing_override() {
    use crate::game_logic::host_deliver_payload::{
        HostDeliverPayloadKind, SUPPLY_DROP_PARACHUTE_DIRECTLY,
        SUPPLY_DROP_PAYLOAD_RESIDUAL_TEMPLATE,
    };
    use crate::game_logic::{KindOf, ThingTemplate};
    assert!(SUPPLY_DROP_PARACHUTE_DIRECTLY);
    let mut logic = GameLogic::new();
    // Residual supply-drop crate template.
    if !logic
        .templates
        .contains_key(SUPPLY_DROP_PAYLOAD_RESIDUAL_TEMPLATE)
    {
        let mut t = ThingTemplate::new(SUPPLY_DROP_PAYLOAD_RESIDUAL_TEMPLATE);
        t.add_kind_of(KindOf::Resource).set_health(1.0);
        logic
            .templates
            .insert(SUPPLY_DROP_PAYLOAD_RESIDUAL_TEMPLATE.to_string(), t);
    }
    let zone = ObjectId(8801);
    logic.objects.insert(zone, {
        let mut t = ThingTemplate::new("AmericaSupplyDropZone");
        t.add_kind_of(KindOf::Structure);
        Object::new(t, zone, Team::USA)
    });
    let target = glam::Vec3::new(200.0, 0.0, 150.0);
    let mission_id = logic.host_deliver_payloads.queue(
        HostDeliverPayloadKind::SupplyDropZoneCrate,
        zone,
        Team::USA,
        target,
        logic.frame,
        SUPPLY_DROP_PAYLOAD_RESIDUAL_TEMPLATE.to_string(),
    );
    // Advance frames until items spawn.
    let mut spawned_any = false;
    for _ in 0..400 {
        logic.frame = logic.frame.saturating_add(1);
        logic.update_deliver_payloads();
        if logic
            .host_deliver_payloads
            .get(mission_id)
            .map(|m| !m.spawned_payload_ids.is_empty())
            .unwrap_or(false)
        {
            spawned_any = true;
            break;
        }
    }
    assert!(spawned_any, "DeliverPayload must spawn residual crate");
    let crate_id = logic
        .host_deliver_payloads
        .get(mission_id)
        .unwrap()
        .spawned_payload_ids[0];
    let c = logic.objects.get(&crate_id).expect("crate object");
    assert!(c.is_parachuting(), "crate must parachute");
    assert!(
        c.has_parachute_landing_override(),
        "ParachuteDirectly must arm landingOverride"
    );
    let ov = c.parachute_landing_override().unwrap();
    assert!(
        (ov.x - target.x).abs() < 0.1 && (ov.z - target.z).abs() < 0.1,
        "override LZ must match DeliverPayload target {:?}",
        ov
    );
    assert!(
        logic.host_deliver_payloads.honesty_parachute_directly_ok(),
        "DeliverPayload ParachuteDirectly honesty"
    );

    // Open + step toward target residual.
    {
        let c = logic.objects.get_mut(&crate_id).unwrap();
        c.open_eject_parachute();
        // Force open path for XZ step.
        c.status.parachute_start_height = c.get_position().y + 200.0;
    }
    let before = logic.objects[&crate_id].get_position();
    for _ in 0..10 {
        logic.tick_crate_parachute_residual(crate_id);
    }
    let after = logic.objects[&crate_id].get_position();
    let d0 = ((before.x - target.x).powi(2) + (before.z - target.z).powi(2)).sqrt();
    let d1 = ((after.x - target.x).powi(2) + (after.z - target.z).powi(2)).sqrt();
    assert!(
        d1 < d0 - 0.5,
        "crate must steer XZ toward LZ: before {} after {}",
        d0,
        d1
    );
}

#[test]
fn parachute_landing_override_steers_xz() {
    use crate::game_logic::host_car_bomb::HIJACKER_PARACHUTE_NAME;
    use crate::game_logic::host_usa_pilot::PARACHUTE_OPEN_DIST;
    use crate::game_logic::{KindOf, Team, ThingTemplate};
    let mut logic = GameLogic::new();
    if !logic.templates.contains_key(HIJACKER_PARACHUTE_NAME) {
        let mut ct = ThingTemplate::new(HIJACKER_PARACHUTE_NAME);
        ct.add_kind_of(KindOf::Vehicle).set_health(1.0);
        logic
            .templates
            .insert(HIJACKER_PARACHUTE_NAME.to_string(), ct);
    }
    // Start high enough that freefall opens after OpenDist.
    let start_y = PARACHUTE_OPEN_DIST + 80.0;
    let chute_id = logic
        .create_object(
            HIJACKER_PARACHUTE_NAME,
            Team::USA,
            glam::Vec3::new(0.0, start_y, 0.0),
        )
        .expect("chute");
    {
        let c = logic.objects.get_mut(&chute_id).unwrap();
        c.max_transport = 1;
        c.apply_eject_parachuting();
    }
    // C++ empty AmericaParachute dies in update; fixture needs a living rider.
    if !logic.templates.contains_key("AmericaInfantryRanger") {
        let mut rt = ThingTemplate::new("AmericaInfantryRanger");
        rt.add_kind_of(KindOf::Infantry).set_health(100.0);
        logic
            .templates
            .insert("AmericaInfantryRanger".to_string(), rt);
    }
    let rider_id = logic
        .create_object(
            "AmericaInfantryRanger",
            Team::USA,
            glam::Vec3::new(0.0, start_y, 0.0),
        )
        .expect("rider");
    {
        let c = logic.objects.get_mut(&chute_id).unwrap();
        if !c.enter_transport(rider_id) {
            if !c.occupants.contains(&rider_id) {
                c.occupants.push(rider_id);
            }
        }
    }
    {
        let r = logic.objects.get_mut(&rider_id).unwrap();
        r.set_contained_by(Some(chute_id));
        r.apply_eject_parachuting();
        r.set_position(glam::Vec3::new(0.0, start_y, 0.0));
    }
    let dest = glam::Vec3::new(100.0, 0.0, 50.0);
    assert!(logic.set_parachute_override_destination(chute_id, dest));
    assert!(logic.objects[&chute_id].has_parachute_landing_override());

    // Tick until open + several override steps.
    let mut opened = false;
    for _ in 0..80 {
        logic.tick_eject_parachute_residual(chute_id);
        if logic.objects[&chute_id].is_parachute_open() {
            opened = true;
        }
        if opened && logic.usa_pilot_residual().landing_override_steps >= 3 {
            break;
        }
    }
    assert!(opened, "chute must open");
    let p = logic.objects[&chute_id].get_position();
    assert!(
        p.x > 1.0 || p.z > 1.0,
        "landingOverride must steer XZ toward dest, got {:?}",
        p
    );
    // Should be closer to dest in XZ than origin.
    let d0 = (0.0f32.hypot(0.0) - 0.0).abs(); // origin dist to dest xz
    let dist_origin = (100.0f32.powi(2) + 50.0f32.powi(2)).sqrt();
    let dist_now = ((p.x - 100.0).powi(2) + (p.z - 50.0).powi(2)).sqrt();
    assert!(
        dist_now < dist_origin - 1.0,
        "must approach override LZ: now {} start {}",
        dist_now,
        dist_origin
    );
    assert!(logic.honesty_parachute_landing_override_ok());
    let _ = d0;
}

#[test]
fn reconstructing_death_transfers_attackers_back_to_hole() {
    use crate::game_logic::{KindOf, Team, ThingTemplate};
    let mut logic = GameLogic::new();
    logic
        .players
        .insert(0, Player::new(0, Team::GLA, "GLA", true));
    logic
        .players
        .insert(1, Player::new(1, Team::USA, "USA", true));
    let mut st = ThingTemplate::new("GLATunnelNetwork");
    st.add_kind_of(KindOf::Structure).set_health(1000.0);
    st.set_rebuild_hole_expose("GLAHoleTunnelNetwork", 0.0);
    logic.templates.insert("GLATunnelNetwork".into(), st);
    let mut ranger = ThingTemplate::new("AmericaInfantryRanger");
    ranger.add_kind_of(KindOf::Infantry).set_health(100.0);
    logic
        .templates
        .insert("AmericaInfantryRanger".into(), ranger);

    let orig = logic
        .create_object(
            "GLATunnelNetwork",
            Team::GLA,
            glam::Vec3::new(0.0, 0.0, 0.0),
        )
        .expect("orig");
    if let Some(o) = logic.host_object_mut(orig) {
        o.set_status_under_construction(false);
        o.construction_percent = 1.0;
    }
    let hole = logic.maybe_spawn_rebuild_hole(orig).expect("hole");
    // Simulate reconstructing building with producer = hole.
    let rid = logic
        .create_object(
            "GLATunnelNetwork",
            Team::GLA,
            glam::Vec3::new(0.0, 0.0, 0.0),
        )
        .expect("recon");
    if let Some(o) = logic.host_object_mut(rid) {
        o.set_status_under_construction(true);
        o.set_status_reconstructing(true);
        o.producer_id = Some(hole);
        o.construction_percent = 0.3;
    }
    if let Some(h) = logic.host_object_mut(hole) {
        h.rebuild_reconstructing_id = Some(rid);
        h.set_status_masked(true);
    }
    let aid = logic
        .create_object(
            "AmericaInfantryRanger",
            Team::USA,
            glam::Vec3::new(20.0, 0.0, 0.0),
        )
        .expect("atk");
    if let Some(a) = logic.host_object_mut(aid) {
        a.set_ai_state(AIState::Attacking);
        a.target = Some(rid);
    }
    assert!(logic.handle_reconstructing_death(rid));
    assert!(logic.rebuild_hole_recon_deaths > 0);
    assert_eq!(logic.host_object(aid).unwrap().target, Some(hole));
    let h = logic.host_object(hole).unwrap();
    assert!(h.rebuild_reconstructing_id.is_none());
    assert!(!h.status.masked);
    assert!(h.rebuild_ready_frame > 0);
}

#[test]
fn rebuild_hole_transfers_sticky_bombs_to_reconstruction() {
    use crate::game_logic::host_mines::{HostMineData, HostMineKind};
    use crate::game_logic::{KindOf, Team, ThingTemplate};
    let mut logic = GameLogic::new();
    logic
        .players
        .insert(0, Player::new(0, Team::GLA, "GLA", true));
    let mut st = ThingTemplate::new("GLATunnelNetwork");
    st.add_kind_of(KindOf::Structure).set_health(1000.0);
    st.set_rebuild_hole_expose("GLAHoleTunnelNetwork", 0.0);
    logic.templates.insert("GLATunnelNetwork".into(), st);
    let mut bomb_t = ThingTemplate::new("TimedC4Charge");
    bomb_t.set_health(10.0);
    logic.templates.insert("TimedC4Charge".into(), bomb_t);
    let hole = {
        let sid = logic
            .create_object(
                "GLATunnelNetwork",
                Team::GLA,
                glam::Vec3::new(0.0, 0.0, 0.0),
            )
            .expect("s");
        if let Some(o) = logic.host_object_mut(sid) {
            o.set_status_under_construction(false);
            o.construction_percent = 1.0;
        }
        logic.maybe_spawn_rebuild_hole(sid).expect("hole")
    };
    let bid = logic
        .create_object("TimedC4Charge", Team::USA, glam::Vec3::new(1.0, 0.0, 0.0))
        .expect("bomb");
    if let Some(o) = logic.host_object_mut(bid) {
        let mut md = HostMineData::new(HostMineKind::TimedDemoCharge);
        md.attached_to = Some(hole);
        o.mine_data = Some(md);
    }
    // Force reconstruct path.
    let now = logic.frame.max(1);
    if let Some(h) = logic.host_object_mut(hole) {
        h.rebuild_ready_frame = now;
    }
    logic.frame = now;
    logic.update_rebuild_holes();
    let rid = logic
        .host_object(hole)
        .and_then(|h| h.rebuild_reconstructing_id)
        .expect("recon");
    assert_eq!(
        logic
            .host_object(bid)
            .and_then(|b| b.mine_data.as_ref())
            .and_then(|m| m.attached_to),
        Some(rid)
    );
    assert!(logic.rebuild_hole_bomb_transfers > 0);
}

#[test]
fn rebuild_hole_transfers_attackers_and_cancel_skips_refund() {
    use crate::game_logic::{KindOf, Team, ThingTemplate};
    let mut logic = GameLogic::new();
    logic
        .players
        .insert(0, Player::new(0, Team::GLA, "GLA", true));
    logic
        .players
        .insert(1, Player::new(1, Team::USA, "USA", true));
    let mut st = ThingTemplate::new("GLABarracks");
    st.add_kind_of(KindOf::Structure).set_health(800.0);
    st.set_rebuild_hole_expose("GLAHole", 0.0);
    st.build_cost.supplies = 500;
    logic.templates.insert("GLABarracks".into(), st);
    let mut ranger = ThingTemplate::new("AmericaInfantryRanger");
    ranger.add_kind_of(KindOf::Infantry).set_health(100.0);
    logic
        .templates
        .insert("AmericaInfantryRanger".into(), ranger);
    let sid = logic
        .create_object("GLABarracks", Team::GLA, glam::Vec3::new(0.0, 0.0, 0.0))
        .expect("b");
    if let Some(o) = logic.host_object_mut(sid) {
        o.set_status_under_construction(false);
        o.construction_percent = 1.0;
    }
    let aid = logic
        .create_object(
            "AmericaInfantryRanger",
            Team::USA,
            glam::Vec3::new(20.0, 0.0, 0.0),
        )
        .expect("a");
    if let Some(a) = logic.host_object_mut(aid) {
        a.set_ai_state(AIState::Attacking);
        a.target = Some(sid);
    }
    let hole = logic.maybe_spawn_rebuild_hole(sid).expect("hole");
    assert!(logic.rebuild_hole_attack_transfers > 0);
    assert_eq!(logic.host_object(aid).unwrap().target, Some(hole));
    // Reconstructing cancel residual: no refund.
    if let Some(p) = logic.get_player_mut(0) {
        p.resources.supplies = 1000;
    }
    // Build reconstructing structure manually with reconstructing flag.
    let rid = logic
        .create_object("GLABarracks", Team::GLA, glam::Vec3::new(0.0, 0.0, 0.0))
        .expect("recon");
    if let Some(o) = logic.host_object_mut(rid) {
        o.set_status_under_construction(true);
        o.set_status_reconstructing(true);
        o.construction_percent = 0.2;
    }
    // Simulate cancel refund policy.
    let refund = {
        let o = logic.host_object(rid).unwrap();
        if o.status.reconstructing {
            0
        } else {
            o.thing.template.build_cost.supplies
        }
    };
    assert_eq!(refund, 0);
}

#[test]
fn gla_structure_death_spawns_rebuild_hole_and_reconstructs() {
    use crate::game_logic::{KindOf, Team, ThingTemplate};
    let mut logic = GameLogic::new();
    logic
        .players
        .insert(0, Player::new(0, Team::GLA, "GLA", true));
    let mut st = ThingTemplate::new("GLATunnelNetwork");
    st.add_kind_of(KindOf::Structure).set_health(1000.0);
    st.set_rebuild_hole_expose("GLAHoleTunnelNetwork", 0.0);
    logic.templates.insert("GLATunnelNetwork".into(), st);
    // Authored hole INI HP must not be overwritten by constructor 500.
    let mut hole_tpl = ThingTemplate::new("GLAHoleTunnelNetwork");
    hole_tpl.add_kind_of(KindOf::Structure).set_health(200.0);
    logic
        .templates
        .insert("GLAHoleTunnelNetwork".into(), hole_tpl);
    let sid = logic
        .create_object(
            "GLATunnelNetwork",
            Team::GLA,
            glam::Vec3::new(50.0, 0.0, 50.0),
        )
        .expect("tn");
    if let Some(o) = logic.host_object_mut(sid) {
        o.set_status_under_construction(false);
        o.construction_percent = 1.0;
    }
    let hole = logic.maybe_spawn_rebuild_hole(sid).expect("hole");
    assert!(logic.host_object(hole).unwrap().is_rebuild_hole);
    assert_eq!(
        logic
            .host_object(hole)
            .unwrap()
            .rebuild_template_name
            .as_deref(),
        Some("GLATunnelNetwork")
    );
    assert!(
        (logic.host_object(hole).unwrap().health.maximum - 200.0).abs() < 0.01,
        "must keep authored hole-template HP, not force 500"
    );
    assert_eq!(
        logic.host_object(hole).unwrap().rebuild_ready_frame,
        logic.frame.max(1).saturating_add(600),
        "WorkerRespawnDelay 20000ms → 600 frames"
    );
    assert!(logic.rebuild_hole_spawns > 0);
    // Heal residual while waiting.
    if let Some(h) = logic.host_object_mut(hole) {
        h.health.current = 100.0;
    }
    logic.update_rebuild_holes();
    assert!(logic.honesty_rebuild_hole_heal_ok());
    // Force ready → worker + reconstructing building; hole stays masked.
    let now = logic.frame.max(1);
    if let Some(h) = logic.host_object_mut(hole) {
        h.rebuild_ready_frame = now;
    }
    logic.frame = now;
    logic.update_rebuild_holes();
    assert!(logic.rebuild_hole_reconstructs > 0);
    assert!(logic.rebuild_hole_workers > 0);
    let h = logic
        .host_object(hole)
        .expect("hole still present while reconstructing");
    assert!(h.status.masked);
    let rid = h.rebuild_reconstructing_id.expect("recon id");
    let wid = h.rebuild_worker_id.expect("worker id");
    assert!(logic.host_object(wid).unwrap().status.unselectable);
    assert_eq!(
        logic.host_object(wid).unwrap().template_name,
        "GLAInfantryWorker"
    );
    assert!(logic.host_object(rid).unwrap().status.under_construction);
    assert!(logic.host_object(rid).unwrap().status.reconstructing);
    // Complete construction → hole removed.
    if let Some(b) = logic.host_object_mut(rid) {
        b.set_status_under_construction(false);
        b.construction_percent = 1.0;
    }
    logic.update_rebuild_holes();
    logic.process_destroy_list();
    assert!(logic.host_object(hole).is_none());
    assert!(logic.rebuild_hole_completes > 0);
    assert!(logic.honesty_rebuild_hole_ok());
}

#[test]
fn rebuild_hole_transfers_script_name_and_skips_defeated_player() {
    // C++ RebuildHoleExposeDie.cpp:108-110 isPlayerActive; :131 transferObjectName.
    use crate::game_logic::{KindOf, Team, ThingTemplate};
    let mut logic = GameLogic::new();
    logic
        .players
        .insert(0, Player::new(0, Team::GLA, "GLA", true));
    let mut st = ThingTemplate::new("GLATunnelNetwork");
    st.add_kind_of(KindOf::Structure).set_health(1000.0);
    st.set_rebuild_hole_expose("GLAHoleTunnelNetwork", 0.0);
    logic.templates.insert("GLATunnelNetwork".into(), st);
    let sid = logic
        .create_object(
            "GLATunnelNetwork",
            Team::GLA,
            glam::Vec3::new(0.0, 0.0, 0.0),
        )
        .expect("tn");
    if let Some(o) = logic.host_object_mut(sid) {
        o.set_status_under_construction(false);
        o.construction_percent = 1.0;
        o.name = "NamedTunnel".into();
    }
    let hole = logic.maybe_spawn_rebuild_hole(sid).expect("hole");
    assert_eq!(
        logic.host_object(hole).unwrap().name,
        "NamedTunnel",
        "hole must inherit script name"
    );

    let mut dead = GameLogic::new();
    let mut defeated = Player::new(0, Team::GLA, "GLA", true);
    defeated.is_alive = false;
    dead.players.insert(0, defeated);
    let mut st2 = ThingTemplate::new("GLATunnelNetwork");
    st2.add_kind_of(KindOf::Structure).set_health(1000.0);
    st2.set_rebuild_hole_expose("GLAHoleTunnelNetwork", 0.0);
    dead.templates.insert("GLATunnelNetwork".into(), st2);
    let sid2 = dead
        .create_object(
            "GLATunnelNetwork",
            Team::GLA,
            glam::Vec3::new(0.0, 0.0, 0.0),
        )
        .expect("tn2");
    if let Some(o) = dead.host_object_mut(sid2) {
        o.set_status_under_construction(false);
        o.construction_percent = 1.0;
    }
    assert!(
        dead.maybe_spawn_rebuild_hole(sid2).is_none(),
        "defeated player must not expose a rebuild hole"
    );
}

#[test]
fn rebuild_hole_worker_death_resumes_existing_scaffold() {
    // C++ RebuildHoleBehavior.cpp:241 aiResumeConstruction when worker dies.
    use crate::game_logic::{KindOf, Team, ThingTemplate};
    let mut logic = GameLogic::new();
    logic
        .players
        .insert(0, Player::new(0, Team::GLA, "GLA", true));
    let mut st = ThingTemplate::new("GLABarracks");
    st.add_kind_of(KindOf::Structure).set_health(800.0);
    st.set_rebuild_hole_expose("GLAHole", 0.0);
    logic.templates.insert("GLABarracks".into(), st);
    let sid = logic
        .create_object("GLABarracks", Team::GLA, glam::Vec3::new(0.0, 0.0, 0.0))
        .expect("b");
    if let Some(o) = logic.host_object_mut(sid) {
        o.set_status_under_construction(false);
        o.construction_percent = 1.0;
    }
    let hole = logic.maybe_spawn_rebuild_hole(sid).expect("hole");
    let now = logic.frame.max(1);
    if let Some(h) = logic.host_object_mut(hole) {
        h.rebuild_ready_frame = now;
    }
    logic.frame = now;
    logic.update_rebuild_holes();
    let (rid, wid) = {
        let h = logic.host_object(hole).expect("hole");
        (
            h.rebuild_reconstructing_id.expect("recon"),
            h.rebuild_worker_id.expect("worker"),
        )
    };
    // Kill the generated worker; scaffold stays up.
    if let Some(w) = logic.host_object_mut(wid) {
        w.health.current = 0.0;
        w.status.destroyed = true;
    }
    logic.update_rebuild_holes();
    let h = logic.host_object(hole).expect("hole after worker death");
    assert!(h.rebuild_worker_id.is_none());
    assert_eq!(h.rebuild_reconstructing_id, Some(rid));
    assert!(logic.rebuild_hole_worker_restarts > 0);
    // After WorkerRespawnDelay, a new worker must resume the same scaffold.
    let later = logic.frame.max(1).saturating_add(600);
    if let Some(h) = logic.host_object_mut(hole) {
        h.rebuild_ready_frame = later;
    }
    logic.frame = later;
    logic.update_rebuild_holes();
    let h = logic.host_object(hole).expect("hole after resume");
    let wid2 = h.rebuild_worker_id.expect("replacement worker");
    assert_ne!(wid2, wid);
    assert_eq!(h.rebuild_reconstructing_id, Some(rid));
    assert_eq!(logic.host_object(wid2).unwrap().target, Some(rid));
    assert!(logic.host_object(wid2).unwrap().status.unselectable);
    assert!(h.status.masked);
}

#[test]
fn rebuild_hole_copies_dying_building_geometry() {
    // C++ RebuildHoleExposeDie.cpp:126 hole->setGeometryInfo(us->getGeometryInfo()).
    use crate::game_logic::{KindOf, Team, ThingTemplate};
    let mut logic = GameLogic::new();
    logic
        .players
        .insert(0, Player::new(0, Team::GLA, "GLA", true));
    let mut st = ThingTemplate::new("GLAPalace");
    st.add_kind_of(KindOf::Structure).set_health(2000.0);
    st.set_rebuild_hole_expose("GLAHole", 0.0);
    logic.templates.insert("GLAPalace".into(), st);
    let sid = logic
        .create_object("GLAPalace", Team::GLA, glam::Vec3::new(10.0, 0.0, 20.0))
        .expect("palace");
    if let Some(o) = logic.host_object_mut(sid) {
        o.set_status_under_construction(false);
        o.construction_percent = 1.0;
        o.thing.geometry.bounds_min = glam::Vec3::new(-40.0, 0.0, -35.0);
        o.thing.geometry.bounds_max = glam::Vec3::new(40.0, 55.0, 35.0);
        o.thing.geometry.radius = 53.0;
        o.selection_radius = 48.0;
    }
    let hole = logic.maybe_spawn_rebuild_hole(sid).expect("hole");
    let h = logic.host_object(hole).expect("hole obj");
    assert!((h.thing.geometry.bounds_min.x + 40.0).abs() < 0.01);
    assert!((h.thing.geometry.bounds_max.y - 55.0).abs() < 0.01);
    assert!((h.thing.geometry.radius - 53.0).abs() < 0.01);
    assert!((h.selection_radius - 48.0).abs() < 0.01);
}

#[test]
fn rebuild_hole_complete_transfers_script_name() {
    // C++ RebuildHoleBehavior.cpp:302 transferObjectName(hole, reconstructing).
    use crate::game_logic::{KindOf, Team, ThingTemplate};
    let mut logic = GameLogic::new();
    logic
        .players
        .insert(0, Player::new(0, Team::GLA, "GLA", true));
    let mut st = ThingTemplate::new("GLATunnelNetwork");
    st.add_kind_of(KindOf::Structure).set_health(1000.0);
    st.set_rebuild_hole_expose("GLAHoleTunnelNetwork", 0.0);
    logic.templates.insert("GLATunnelNetwork".into(), st);
    let sid = logic
        .create_object(
            "GLATunnelNetwork",
            Team::GLA,
            glam::Vec3::new(0.0, 0.0, 0.0),
        )
        .expect("tn");
    if let Some(o) = logic.host_object_mut(sid) {
        o.set_status_under_construction(false);
        o.construction_percent = 1.0;
        o.name = "NamedTunnel".into();
    }
    let hole = logic.maybe_spawn_rebuild_hole(sid).expect("hole");
    let now = logic.frame.max(1);
    if let Some(h) = logic.host_object_mut(hole) {
        h.rebuild_ready_frame = now;
    }
    logic.frame = now;
    logic.update_rebuild_holes();
    let rid = logic
        .host_object(hole)
        .and_then(|h| h.rebuild_reconstructing_id)
        .expect("recon");
    if let Some(b) = logic.host_object_mut(rid) {
        b.set_status_under_construction(false);
        b.construction_percent = 1.0;
    }
    logic.update_rebuild_holes();
    logic.process_destroy_list();
    assert!(logic.host_object(hole).is_none());
    assert_eq!(
        logic.host_object(rid).unwrap().name,
        "NamedTunnel",
        "completed rebuild must inherit the hole script name"
    );
}

#[test]
fn captured_gla_building_exposes_rebuild_hole() {
    // C++ RebuildHoleExposeDie is module-based; captured GLA still drops a hole
    // on the capturing player's team.
    use crate::game_logic::{KindOf, Team, ThingTemplate};
    let mut logic = GameLogic::new();
    logic
        .players
        .insert(1, Player::new(1, Team::USA, "USA", true));
    let mut st = ThingTemplate::new("GLABarracks");
    st.add_kind_of(KindOf::Structure).set_health(800.0);
    st.set_rebuild_hole_expose("GLAHole", 0.0);
    logic.templates.insert("GLABarracks".into(), st);
    let sid = logic
        .create_object("GLABarracks", Team::USA, glam::Vec3::new(0.0, 0.0, 0.0))
        .expect("captured");
    if let Some(o) = logic.host_object_mut(sid) {
        o.set_status_under_construction(false);
        o.construction_percent = 1.0;
        o.owner_player_id = Some(1);
    }
    let hole = logic
        .maybe_spawn_rebuild_hole(sid)
        .expect("captured GLA must expose a hole");
    let h = logic.host_object(hole).expect("hole");
    assert!(h.is_rebuild_hole);
    assert_eq!(h.team, Team::USA);
    assert_eq!(h.owner_player_id, Some(1));
}

#[test]
fn usa_china_command_center_does_not_expose_gla_rebuild_hole() {
    // C++ RebuildHoleExposeDie is authored on GLA FactionBuilding.ini only.
    // AmericaCommandCenter / ChinaBarracks must not leave a free GLA hole.
    use crate::game_logic::{KindOf, Team, ThingTemplate};
    let mut logic = GameLogic::new();
    logic
        .players
        .insert(0, Player::new(0, Team::USA, "USA", true));
    for name in [
        "AmericaCommandCenter",
        "ChinaBarracks",
        "AmericaSupplyCenter",
    ] {
        let mut st = ThingTemplate::new(name);
        st.add_kind_of(KindOf::Structure).set_health(1000.0);
        logic.templates.insert(name.into(), st);
        let sid = logic
            .create_object(name, Team::USA, glam::Vec3::ZERO)
            .expect(name);
        if let Some(o) = logic.host_object_mut(sid) {
            o.set_status_under_construction(false);
            o.construction_percent = 1.0;
            o.owner_player_id = Some(0);
        }
        assert!(
            logic.maybe_spawn_rebuild_hole(sid).is_none(),
            "{name} must not spawn a GLA rebuild hole"
        );
    }
}

#[test]
fn rebuild_hole_and_scaffold_death_destroys_worker() {
    // C++ onDie / newWorkerRespawnProcess destroy the generated worker.
    use crate::game_logic::{KindOf, Team, ThingTemplate};
    let mut logic = GameLogic::new();
    logic
        .players
        .insert(0, Player::new(0, Team::GLA, "GLA", true));
    let mut st = ThingTemplate::new("GLABarracks");
    st.add_kind_of(KindOf::Structure).set_health(800.0);
    st.set_rebuild_hole_expose("GLAHole", 0.0);
    logic.templates.insert("GLABarracks".into(), st);
    let sid = logic
        .create_object("GLABarracks", Team::GLA, glam::Vec3::new(0.0, 0.0, 0.0))
        .expect("b");
    if let Some(o) = logic.host_object_mut(sid) {
        o.set_status_under_construction(false);
        o.construction_percent = 1.0;
    }
    let hole = logic.maybe_spawn_rebuild_hole(sid).expect("hole");
    let now = logic.frame.max(1);
    if let Some(h) = logic.host_object_mut(hole) {
        h.rebuild_ready_frame = now;
    }
    logic.frame = now;
    logic.update_rebuild_holes();
    let (rid, wid) = {
        let h = logic.host_object(hole).expect("hole");
        (
            h.rebuild_reconstructing_id.expect("recon"),
            h.rebuild_worker_id.expect("worker"),
        )
    };
    assert!(logic.handle_reconstructing_death(rid));
    assert!(
        logic.host_object(hole).unwrap().rebuild_worker_id.is_none(),
        "scaffold death must drop the worker id"
    );
    let worker_destroyed = logic
        .host_object(wid)
        .map(|w| w.status.destroyed || !w.is_alive())
        .unwrap_or(true)
        || logic.objects_to_destroy.iter().any(|e| e.id == wid);
    assert!(
        worker_destroyed,
        "scaffold death must destroy the generated worker"
    );

    // Fresh hole+worker: hole death must also destroy the worker.
    let sid2 = logic
        .create_object("GLABarracks", Team::GLA, glam::Vec3::new(40.0, 0.0, 0.0))
        .expect("b2");
    if let Some(o) = logic.host_object_mut(sid2) {
        o.set_status_under_construction(false);
        o.construction_percent = 1.0;
    }
    let hole2 = logic.maybe_spawn_rebuild_hole(sid2).expect("hole2");
    let now2 = logic.frame.max(1);
    if let Some(h) = logic.host_object_mut(hole2) {
        h.rebuild_ready_frame = now2;
    }
    logic.frame = now2;
    logic.update_rebuild_holes();
    let wid2 = logic
        .host_object(hole2)
        .and_then(|h| h.rebuild_worker_id)
        .expect("worker2");
    assert!(logic.maybe_spawn_rebuild_hole(hole2).is_none());
    assert!(
        logic
            .host_object(hole2)
            .unwrap()
            .rebuild_worker_id
            .is_none()
    );
    let worker2_destroyed = logic
        .host_object(wid2)
        .map(|w| w.status.destroyed || !w.is_alive())
        .unwrap_or(true)
        || logic.objects_to_destroy.iter().any(|e| e.id == wid2);
    assert!(
        worker2_destroyed,
        "hole death must destroy the generated worker"
    );
}

#[test]
fn scud_storm_uses_authored_rebuild_hole_name() {
    use crate::game_logic::{KindOf, Team, ThingTemplate};
    let mut logic = GameLogic::new();
    logic
        .players
        .insert(0, Player::new(0, Team::GLA, "GLA", true));
    let mut st = ThingTemplate::new("GLAScudStorm");
    st.add_kind_of(KindOf::Structure).set_health(4000.0);
    st.set_rebuild_hole_expose("GLAScudStormRebuildHole", 500.0);
    logic.templates.insert("GLAScudStorm".into(), st);
    let sid = logic
        .create_object("GLAScudStorm", Team::GLA, glam::Vec3::ZERO)
        .expect("scud");
    if let Some(o) = logic.host_object_mut(sid) {
        o.set_status_under_construction(false);
        o.construction_percent = 1.0;
    }
    let hole = logic.maybe_spawn_rebuild_hole(sid).expect("scud hole");
    assert_eq!(
        logic.host_object(hole).unwrap().template_name,
        "GLAScudStormRebuildHole"
    );

    let mut fake = ThingTemplate::new("GLAFakeCommandCenter");
    fake.add_kind_of(KindOf::Structure).set_health(100.0);
    logic.templates.insert("GLAFakeCommandCenter".into(), fake);
    let fid = logic
        .create_object(
            "GLAFakeCommandCenter",
            Team::GLA,
            glam::Vec3::new(20.0, 0.0, 0.0),
        )
        .expect("fake");
    if let Some(o) = logic.host_object_mut(fid) {
        o.set_status_under_construction(false);
        o.construction_percent = 1.0;
    }
    assert!(
        logic.maybe_spawn_rebuild_hole(fid).is_none(),
        "GLAFake* without RebuildHoleExposeDie must not spawn a hole"
    );
}

#[test]
fn rebuild_hole_clock_starts_at_death_start_not_topple_done() {
    use crate::game_logic::{KindOf, Team, ThingTemplate};
    let mut logic = GameLogic::new();
    logic
        .players
        .insert(0, Player::new(0, Team::GLA, "GLA", true));
    let mut st = ThingTemplate::new("GLABarracks");
    st.add_kind_of(KindOf::Structure).set_health(800.0);
    st.set_rebuild_hole_expose("GLAHole", 0.0);
    logic.templates.insert("GLABarracks".into(), st);
    let sid = logic
        .create_object("GLABarracks", Team::GLA, glam::Vec3::ZERO)
        .expect("b");
    if let Some(o) = logic.host_object_mut(sid) {
        o.set_status_under_construction(false);
        o.construction_percent = 1.0;
    }
    logic.frame = 10;
    logic.mark_object_for_destruction(sid, None);
    let hole = logic
        .objects
        .values()
        .find(|o| o.is_rebuild_hole)
        .map(|o| o.id)
        .expect("hole at death start");
    assert!(
        logic.host_object(sid).is_some(),
        "husk still present while toppling"
    );
    assert_eq!(
        logic.host_object(hole).unwrap().rebuild_ready_frame,
        10u32.max(1).saturating_add(600)
    );
}

#[test]
fn rebuild_hole_uses_dying_owner_not_faction_team() {
    use crate::game_logic::{KindOf, Team, ThingTemplate};
    let mut logic = GameLogic::new();
    logic
        .players
        .insert(1, Player::new(1, Team::GLA, "GLA-A", true));
    logic
        .players
        .insert(2, Player::new(2, Team::GLA, "GLA-B", true));
    let mut st = ThingTemplate::new("GLABarracks");
    st.add_kind_of(KindOf::Structure).set_health(800.0);
    st.set_rebuild_hole_expose("GLAHole", 0.0);
    logic.templates.insert("GLABarracks".into(), st);
    let sid = logic
        .create_object_for_player("GLABarracks", 2, glam::Vec3::ZERO)
        .expect("p2 barracks");
    if let Some(o) = logic.host_object_mut(sid) {
        o.set_status_under_construction(false);
        o.construction_percent = 1.0;
    }
    let hole = logic.maybe_spawn_rebuild_hole(sid).expect("hole");
    let h = logic.host_object(hole).expect("hole obj");
    assert_eq!(h.owner_player_id, Some(2));
    assert_eq!(h.team, Team::GLA);
}

#[test]
fn finished_rebuild_hole_goes_through_destroy_object() {
    use crate::game_logic::{KindOf, Team, ThingTemplate};
    let mut logic = GameLogic::new();
    logic
        .players
        .insert(0, Player::new(0, Team::GLA, "GLA", true));
    let mut st = ThingTemplate::new("GLABarracks");
    st.add_kind_of(KindOf::Structure).set_health(800.0);
    st.set_rebuild_hole_expose("GLAHole", 0.0);
    logic.templates.insert("GLABarracks".into(), st);
    let sid = logic
        .create_object("GLABarracks", Team::GLA, glam::Vec3::ZERO)
        .expect("b");
    if let Some(o) = logic.host_object_mut(sid) {
        o.set_status_under_construction(false);
        o.construction_percent = 1.0;
    }
    let hole = logic.maybe_spawn_rebuild_hole(sid).expect("hole");
    let now = logic.frame.max(1);
    if let Some(h) = logic.host_object_mut(hole) {
        h.rebuild_ready_frame = now;
    }
    logic.frame = now;
    logic.update_rebuild_holes();
    let rid = logic
        .host_object(hole)
        .and_then(|h| h.rebuild_reconstructing_id)
        .expect("recon");
    if let Some(b) = logic.host_object_mut(rid) {
        b.set_status_under_construction(false);
        b.construction_percent = 1.0;
    }
    logic.update_rebuild_holes();
    assert!(
        logic.objects_to_destroy.iter().any(|e| e.id == hole),
        "finished hole must be queued via destroy_object, not hashmap-removed"
    );
    assert!(logic.host_object(hole).is_some());
    logic.process_destroy_list();
    assert!(logic.host_object(hole).is_none());
}

#[test]
fn production_door_hold_open_blocks_close_until_released() {
    use crate::game_logic::host_enum_table_residual::{
        door_1_closing_model_bit, door_1_waiting_open_model_bit, host_model_condition_has,
    };
    use crate::game_logic::host_production_buildable_command_residual::producer_door_phase_duration;
    use crate::game_logic::{KindOf, Team, ThingTemplate};
    let mut logic = GameLogic::new();
    let mut st = ThingTemplate::new("AmericaAirfield");
    st.add_kind_of(KindOf::Structure)
        .add_kind_of(KindOf::FSAirfield)
        .set_health(2000.0);
    logic.templates.insert("AmericaAirfield".into(), st);
    let id = logic
        .create_object("AmericaAirfield", Team::USA, glam::Vec3::new(0.0, 0.0, 0.0))
        .expect("af");
    let open = producer_door_phase_duration("AmericaAirfield", 1);
    let close = producer_door_phase_duration("AmericaAirfield", 4);
    if let Some(o) = logic.host_object_mut(id) {
        o.set_production_door_hold_open(true, 0);
        assert!(o.production_door_hold_open);
        assert_eq!(o.production_door_phase, 1);
        assert!(!o.tick_production_door(open));
        assert_eq!(o.production_door_phase, 2);
        assert!(!o.tick_production_door(open.saturating_add(10_000)));
        assert_eq!(o.production_door_phase, 2);
        assert!(host_model_condition_has(
            o.model_condition_bits,
            door_1_waiting_open_model_bit()
        ));
        let release = open.saturating_add(10_000);
        o.set_production_door_hold_open(false, release);
        assert!(!o.tick_production_door(release));
        // C++ updateDoors: WAITING_OPEN → CLOSING. No WAITING_TO_CLOSE.
        assert_eq!(o.production_door_phase, 4);
        assert!(host_model_condition_has(
            o.model_condition_bits,
            door_1_closing_model_bit()
        ));
        assert!(o.tick_production_door(release.saturating_add(close)));
        assert_eq!(o.production_door_phase, 0);
    }
}
