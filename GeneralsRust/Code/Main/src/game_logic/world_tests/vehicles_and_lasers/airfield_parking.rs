//! Behavior tests extracted from the original hero/weapons/airfields suite.
use super::*;

#[test]
fn airfield_parking_rearm_docks_and_heals() {
    use crate::game_logic::host_dock_contain_exit_heal_residual::{
        PARKING_PLACE_AIRFIELD_HEAL_AMOUNT_PER_SEC, parking_place_heal_per_frame,
    };
    use crate::game_logic::{KindOf, ParkingPlaceMetadata, Team, ThingTemplate, Weapon};
    use glam::Vec3;

    let mut logic = GameLogic::new();
    // C++ findSuitableAirfield ally check resolves ownerless objects through
    // the unique faction-team player (Player.cpp getRelationship).
    ensure_test_player_for_team(&mut logic, Team::USA);
    let mut af_tmpl = ThingTemplate::new("AmericaAirfield");
    af_tmpl
        .add_kind_of(KindOf::Structure)
        .add_kind_of(KindOf::FSAirfield)
        .add_kind_of(KindOf::Attackable)
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
    logic.templates.insert("AmericaAirfield".into(), af_tmpl);

    let mut jet_tmpl = ThingTemplate::new("AmericaJetRaptor");
    jet_tmpl.primary_weapon_name = Some("HostTestRaptorJetMissileWeapon".into());
    jet_tmpl
        .add_kind_of(KindOf::Aircraft)
        .add_kind_of(KindOf::Attackable)
        .set_health(100.0);
    logic.templates.insert("AmericaJetRaptor".into(), jet_tmpl);

    let af_id = logic
        .create_object("AmericaAirfield", Team::USA, Vec3::new(0.0, 0.0, 0.0))
        .expect("af");
    let jet_id = logic
        .create_object("AmericaJetRaptor", Team::USA, Vec3::new(50.0, 40.0, 0.0))
        .expect("jet");

    {
        let jet = logic.objects.get_mut(&jet_id).unwrap();
        jet.weapon = Some(Weapon {
            damage: 50.0,
            range: 200.0,
            reload_time: 0.0,
            last_fire_time: -100.0,
            ammo: Some(0),
            clip_size: 4,
            can_target_air: true,
            can_target_ground: true,
            ..Weapon::default()
        });
        jet.health.current = 40.0;
        jet.status.airborne_target = false;
        jet.jet_ai.rtb_landing_phase = crate::game_logic::object::JET_RTB_PHASE_TAXI;
        jet.set_position(Vec3::ZERO);
    }

    crate::game_logic::host_ai_decision_log::clear();
    assert!(logic.try_return_to_base_rearm(jet_id));
    {
        let jet = logic.objects.get(&jet_id).unwrap();
        assert_eq!(jet.weapon.as_ref().unwrap().ammo, Some(0));
        assert_eq!(jet.contained_by, Some(af_id));
        assert!(jet.needs_return_to_base_rearm());
        // Docked AI state last-write under AI_DECISION_AUTHORITY (default on).
        if crate::gameworld_shadow::gameworld_ai_decision_authority_live() {
            assert_eq!(jet.ai_state, AIState::Idle);
            let docked =
                crate::gameworld_shadow::GameWorldShadow::host_ai_state_ordinal(&AIState::Docked);
            let events = crate::game_logic::host_ai_decision_log::snapshot();
            assert!(
                events.iter().any(|e| {
                    e.host_object == jet_id
                        && e.kind == crate::game_logic::host_ai_decision_log::AI_DECISION_SET_STATE
                        && e.ai_state_ordinal == docked
                }),
                "RTB rearm must log SetAIState(Docked) under decision authority"
            );
        } else {
            assert_eq!(jet.ai_state, AIState::Docked);
        }
    }
    assert!(
        logic
            .objects
            .get(&af_id)
            .unwrap()
            .contained_units()
            .contains(&jet_id),
        "airfield must list parked jet"
    );

    let hp_before = logic.objects.get(&jet_id).unwrap().health.current;
    logic.health_events.clear_heal();
    logic.tick_airfield_parking_heal();
    // C++ first pulse is HEAL_RATE_FRAMES after setHealee.
    let expected = 6.0 * PARKING_PLACE_AIRFIELD_HEAL_AMOUNT_PER_SEC / 30.0;
    for _ in 0..6 {
        logic.frame = logic.frame.saturating_add(1);
        logic.tick_airfield_parking_heal();
    }
    let hp_after = logic.objects.get(&jet_id).unwrap().health.current;
    if crate::gameworld_shadow::gameworld_damage_authority_live() {
        let heals = logic.health_events.snapshot_heal();
        let logged = heals
            .iter()
            .any(|e| e.target == jet_id && (e.health - (hp_before + expected)).abs() < 1e-2);
        assert!(
            logged || (hp_after - hp_before - expected).abs() < 1e-3,
            "parking heal must log absolute HP under damage authority (hp {hp_before}->{hp_after}, heals={heals:?}, expect +{expected})"
        );
    } else {
        assert!(
            (hp_after - hp_before - expected).abs() < 1e-3,
            "heal {hp_before} -> {hp_after}, want +{expected}"
        );
    }

    assert_eq!(
        logic
            .objects
            .get(&jet_id)
            .unwrap()
            .weapon
            .as_ref()
            .unwrap()
            .ammo,
        Some(4),
        "rearm completes across heal-rate frames"
    );
}

#[test]
fn airfield_parking_capacity_blocks_fifth_jet() {
    use crate::game_logic::{KindOf, ParkingPlaceMetadata, Team, ThingTemplate, Weapon};
    use glam::Vec3;

    let mut logic = GameLogic::new();
    ensure_test_player_for_team(&mut logic, Team::USA);
    let mut af_tmpl = ThingTemplate::new("AmericaAirfield");
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
    logic.templates.insert("AmericaAirfield".into(), af_tmpl);
    let mut jet_tmpl = ThingTemplate::new("AmericaJetRaptor");
    jet_tmpl.primary_weapon_name = Some("HostTestRaptorJetMissileWeapon".into());
    jet_tmpl.add_kind_of(KindOf::Aircraft).set_health(100.0);
    logic.templates.insert("AmericaJetRaptor".into(), jet_tmpl);

    let _af = logic
        .create_object("AmericaAirfield", Team::USA, Vec3::ZERO)
        .unwrap();
    let mut jet_ids = Vec::new();
    for i in 0..5 {
        let id = logic
            .create_object(
                "AmericaJetRaptor",
                Team::USA,
                Vec3::new(10.0 + i as f32, 20.0, 0.0),
            )
            .unwrap();
        if let Some(jet) = logic.objects.get_mut(&id) {
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
        jet_ids.push(id);
    }
    // First 4 dock; 5th capacity-blocked (NumRows*NumCols=4).
    for (i, &id) in jet_ids.iter().enumerate() {
        let ok = logic.try_return_to_base_rearm(id);
        if i < 4 {
            assert!(ok, "jet {i} should dock");
        } else {
            assert!(!ok, "5th jet must hit parking capacity");
        }
    }
}

#[test]
fn airfield_takeoff_keeps_parking_stall_for_airborne_jet() {
    use crate::game_logic::buildings::BuildingType;
    use crate::game_logic::{KindOf, ParkingPlaceMetadata, Team, ThingTemplate, Weapon};
    use glam::Vec3;

    let mut logic = GameLogic::new();
    ensure_test_player_for_team(&mut logic, Team::USA);
    let mut af_tmpl = ThingTemplate::new("KeepStallAirfield");
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
    logic.templates.insert("KeepStallAirfield".into(), af_tmpl);
    let mut jet_tmpl = ThingTemplate::new("KeepStallRaptor");
    jet_tmpl.primary_weapon_name = Some("HostTestRaptorJetMissileWeapon".into());
    jet_tmpl.add_kind_of(KindOf::Aircraft).set_health(100.0);
    logic.templates.insert("KeepStallRaptor".into(), jet_tmpl);
    let af = logic
        .create_object("KeepStallAirfield", Team::USA, Vec3::ZERO)
        .unwrap();
    if let Some(o) = logic.host_object_mut(af) {
        o.building_data = Some(crate::game_logic::BuildingData::new(BuildingType::Airfield));
    }
    let jet_id = logic
        .create_object("KeepStallRaptor", Team::USA, Vec3::new(10.0, 0.0, 0.0))
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
    assert!(logic.try_runway_takeoff_from_airfield(jet_id));
    {
        let jet = logic.objects.get(&jet_id).unwrap();
        assert!(jet.contained_by.is_none());
        assert!(jet.airfield_parking_space_index.is_some());
    }
    assert!(
        logic
            .airfield_parking_spaces
            .get(&af)
            .is_some_and(|spaces| spaces.iter().any(|space| space.object_id == Some(jet_id))),
        "airborne jet must keep hangar stall (JetAIUpdate.cpp:897-900, 1630)"
    );
}
