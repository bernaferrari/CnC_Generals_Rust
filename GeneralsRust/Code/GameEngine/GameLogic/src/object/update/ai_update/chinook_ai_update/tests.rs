//! Chinook configuration, supply-counter, flight-predicate and runtime Xfer regressions.

use super::module_data::CHINOOK_AI_UPDATE_FIELDS;
use super::{
    CHINOOK_ARRIVE_THRESH_SQR, ChinookAIUpdate, ChinookAIUpdateData, ChinookAIUpdateModuleData,
    ChinookFlightStatus, chinook_attack_allowed_by_kind_of, chinook_dist_sqr,
    chinook_evac_and_exit_pipeline, chinook_evac_needs_takeoff_first, chinook_evac_pipeline,
    chinook_free_to_exit, chinook_move_to_bldg_arrived, chinook_move_to_bldg_preferred_height,
    chinook_passenger_should_follow_attack, chinook_should_auto_land, chinook_should_auto_takeoff,
};
use crate::ai::{AiCommandParams, AiCommandType, CommandSourceType};
use crate::common::{Coord3D, INVALID_ID, LocomotorSetType};
use crate::modules::SupplyTruckAIInterface;
use game_engine::common::ini::INI;
use game_engine::common::system::Snapshotable;
use game_engine::system::xfer_load::XferLoad;
use game_engine::system::xfer_save::XferSave;
use std::io::Cursor;

fn parse_field(data: &mut ChinookAIUpdateModuleData, token: &str, values: &[&str]) {
    let field = CHINOOK_AI_UPDATE_FIELDS
        .iter()
        .find(|field| field.token == token)
        .expect("field exists");
    let mut ini = INI::new();
    (field.parse)(&mut ini, data, values).expect("field parses");
}

#[test]
fn chinook_fields_accept_ini_equals_token() {
    let mut data = ChinookAIUpdateModuleData::default();

    parse_field(
        &mut data,
        "AutoAcquireEnemiesWhenIdle",
        &["=", "YES", "ATTACK_BUILDINGS"],
    );
    parse_field(
        &mut data,
        "Locomotor",
        &["=", "SET_NORMAL", "ChinookLocomotor"],
    );
    parse_field(&mut data, "MoodAttackCheckRate", &["=", "2000"]);
    parse_field(&mut data, "SurrenderDuration", &["=", "3000"]);
    parse_field(&mut data, "ForbidPlayerCommands", &["=", "Yes"]);
    parse_field(&mut data, "TurretsLinked", &["=", "Yes"]);
    parse_field(&mut data, "MaxBoxes", &["=", "6"]);
    parse_field(&mut data, "SupplyCenterActionDelay", &["=", "1200"]);
    parse_field(&mut data, "SupplyWarehouseActionDelay", &["=", "900"]);
    parse_field(&mut data, "SupplyWarehouseScanDistance", &["=", "275.5"]);
    parse_field(
        &mut data,
        "SuppliesDepletedVoice",
        &["=", "ChinookSupplyEmpty"],
    );
    parse_field(&mut data, "RappelSpeed", &["=", "55.5"]);
    parse_field(&mut data, "RopeDropSpeed", &["=", "12.5"]);
    parse_field(&mut data, "RopeName", &["=", "CombatDropRope"]);
    parse_field(&mut data, "RopeFinalHeight", &["=", "35.0"]);
    parse_field(&mut data, "RopeWidth", &["=", "0.75"]);
    parse_field(&mut data, "RopeWobbleLen", &["=", "16.0"]);
    parse_field(&mut data, "RopeWobbleAmplitude", &["=", "2.5"]);
    parse_field(&mut data, "RopeWobbleRate", &["=", "0.35"]);
    parse_field(&mut data, "RopeColor", &["=", "R:51", "G:102", "B:153"]);
    parse_field(&mut data, "NumRopes", &["=", "5"]);
    parse_field(&mut data, "PerRopeDelayMin", &["=", "600"]);
    parse_field(&mut data, "PerRopeDelayMax", &["=", "900"]);
    parse_field(&mut data, "MinDropHeight", &["=", "40.0"]);
    parse_field(&mut data, "WaitForRopesToDrop", &["=", "No"]);
    parse_field(&mut data, "RotorWashParticleSystem", &["=", "ChinookDust"]);
    parse_field(&mut data, "UpgradedSupplyBoost", &["=", "4"]);

    assert_ne!(data.base.auto_acquire_enemies_when_idle(), 0);
    assert!(data.base.has_locomotor_set(LocomotorSetType::Normal));
    assert_eq!(data.base.mood_attack_check_rate(), 60);
    assert_eq!(data.base.surrender_duration_frames(), 90);
    assert!(data.base.forbid_player_commands());
    assert!(data.base.turrets_linked());
    assert_eq!(data.max_boxes_data, 6);
    assert_eq!(data.center_delay, 36);
    assert_eq!(data.warehouse_delay, 27);
    assert_eq!(data.warehouse_scan_distance, 275.5);
    assert_eq!(data.supplies_depleted_voice.as_str(), "ChinookSupplyEmpty");
    assert_eq!(data.rappel_speed, 55.5);
    assert_eq!(data.rope_drop_speed, 12.5);
    assert_eq!(data.rope_name.as_str(), "CombatDropRope");
    assert_eq!(data.rope_final_height, 35.0);
    assert_eq!(data.rope_width, 0.75);
    assert_eq!(data.rope_wobble_len, 16.0);
    assert_eq!(data.rope_wobble_amp, 2.5);
    assert_eq!(
        data.rope_wobble_rate,
        INI::parse_angular_velocity_real("0.35").unwrap()
    );
    assert_eq!(data.rope_color.r, 51);
    assert_eq!(data.rope_color.g, 102);
    assert_eq!(data.rope_color.b, 153);
    assert_eq!(data.num_ropes, 5);
    assert_eq!(data.per_rope_delay_min, 18);
    assert_eq!(data.per_rope_delay_max, 27);
    assert_eq!(data.min_drop_height, 40.0);
    assert!(!data.wait_for_ropes_to_drop);
    assert_eq!(data.rotor_wash_particle_system.as_str(), "ChinookDust");
    assert_eq!(data.upgraded_supply_boost, 4);
}

#[test]
fn chinook_xfer_preserves_cpp_runtime_fields() {
    let data = ChinookAIUpdateData::default();
    let mut original = ChinookAIUpdate::new(data.clone(), 77, 1);
    original.base.set_preferred_dock(1234);
    original.base.set_force_wanting_state(true);
    original.flight_status = ChinookFlightStatus::Landing;
    original.airfield_for_healing = 5678;
    original.original_pos = Coord3D::new(10.0, 20.0, 30.0);

    let mut command = AiCommandParams::new(AiCommandType::CombatDrop, CommandSourceType::FromAi);
    command.pos = Coord3D::new(40.0, 50.0, 60.0);
    command.obj = Some(4321);
    command.other_obj = Some(8765);
    command.team = Some("TeamAlpha".to_string());
    command.coords = vec![Coord3D::new(1.0, 2.0, 3.0)];
    command.int_value = 99;
    original.pending_command = Some(command);

    let mut bytes = Vec::new();
    {
        let cursor = Cursor::new(&mut bytes);
        let mut save = XferSave::new(cursor, 1);
        original.xfer(&mut save).unwrap();
    }

    let mut loaded = ChinookAIUpdate::new(data, 77, 1);
    {
        let cursor = Cursor::new(bytes.as_slice());
        let mut load = XferLoad::new(cursor, 1);
        loaded.xfer(&mut load).unwrap();
    }

    assert_eq!(loaded.base.get_preferred_dock(), Some(1234));
    assert!(loaded.base.is_forced_into_wanting_state());
    assert_eq!(loaded.flight_status, ChinookFlightStatus::Landing);
    assert_eq!(loaded.airfield_for_healing, 5678);
    assert_eq!(loaded.original_pos, Coord3D::new(10.0, 20.0, 30.0));

    let pending = loaded.pending_command.expect("pending command restored");
    assert_eq!(pending.cmd, AiCommandType::CombatDrop);
    assert_eq!(pending.cmd_source, CommandSourceType::FromAi);
    assert_eq!(pending.pos, Coord3D::new(40.0, 50.0, 60.0));
    assert_eq!(pending.obj, Some(4321));
    assert_eq!(pending.other_obj, Some(8765));
    assert_eq!(pending.team.as_deref(), Some("TeamAlpha"));
    assert_eq!(pending.coords, vec![Coord3D::new(1.0, 2.0, 3.0)]);
    assert_eq!(pending.int_value, 99);
}

#[test]
fn chinook_arrival_uses_three_unit_3d_threshold() {
    let a = Coord3D::new(0.0, 0.0, 0.0);
    let inside = Coord3D::new(2.0, 2.0, 1.0);
    let outside = Coord3D::new(3.0, 0.0, 1.0);
    assert!(chinook_dist_sqr(&a, &inside) <= CHINOOK_ARRIVE_THRESH_SQR);
    assert!(chinook_dist_sqr(&a, &outside) > CHINOOK_ARRIVE_THRESH_SQR);
}

#[test]
fn chinook_passenger_follow_skips_engaged_riders() {
    assert!(chinook_passenger_should_follow_attack(false));
    assert!(!chinook_passenger_should_follow_attack(true));
}

/// C++ ChinookAIUpdate.cpp:1408 KindOf CAN_ATTACK, not ObjectStatus.
#[test]
fn chinook_attack_gate_is_kind_of_not_status() {
    assert!(chinook_attack_allowed_by_kind_of(true));
    assert!(!chinook_attack_allowed_by_kind_of(false));
}

/// C++ ChinookAIUpdate.cpp:1067-1087 idle + want enter/exit auto-lands.
#[test]
fn chinook_idle_want_enter_exit_auto_lands() {
    assert!(chinook_should_auto_land(true, true, false));
    assert!(!chinook_should_auto_land(true, true, true));
    assert!(!chinook_should_auto_land(false, true, false));
    assert!(!chinook_should_auto_land(true, false, false));
    assert!(chinook_should_auto_takeoff(true, false, true, false));
    assert!(!chinook_should_auto_takeoff(true, false, true, true));
    assert!(!chinook_free_to_exit(false, false, false));
    assert!(chinook_free_to_exit(true, false, false));
    assert!(chinook_free_to_exit(false, true, true));
    assert!(!chinook_free_to_exit(false, true, false));
}

/// C++ ChinookAIStateMachine.cpp:817-833 evac land/dump/takeoff/HeadOffMap.
#[test]
fn chinook_evac_is_land_dump_takeoff_headoffmap() {
    assert_eq!(
        chinook_evac_pipeline(),
        [
            "MoveToAndEvac",
            "LandAndEvac",
            "EvacAndTakeoff",
            "TakingOff",
        ]
    );
    assert_eq!(
        chinook_evac_and_exit_pipeline(),
        [
            "MoveToAndEvacAndExitInit",
            "MoveToAndEvacAndExit",
            "LandAndEvacAndExit",
            "EvacAndExit",
            "TakeoffAndExit",
            "HeadOffMap",
        ]
    );
    assert!(chinook_evac_needs_takeoff_first(true, 16.0));
    assert!(!chinook_evac_needs_takeoff_first(true, 4.0));
    assert!(!chinook_evac_needs_takeoff_first(false, 100.0));
}

/// C++ ChinookMoveToBldgState.cpp:692-745 height before DO_COMBAT_DROP ropes.
#[test]
fn chinook_combat_drop_waits_for_move_to_bldg_height() {
    assert_eq!(
        chinook_move_to_bldg_preferred_height(100.0, true, 50.0, 40.0),
        100.0
    );
    assert_eq!(
        chinook_move_to_bldg_preferred_height(100.0, true, 80.0, 40.0),
        120.0
    );
    assert_eq!(
        chinook_move_to_bldg_preferred_height(100.0, false, 80.0, 40.0),
        100.0
    );
    assert!(!chinook_move_to_bldg_arrived(true, 10.0, 140.0));
    assert!(chinook_move_to_bldg_arrived(true, 140.0, 141.0));
    assert!(!chinook_move_to_bldg_arrived(false, 140.0, 140.0));
}

/// C++ SupplyTruckAIUpdate.h:111 defaults capacity to zero; authored MaxBoxes
/// controls gainOneBox (SupplyTruckAIUpdate.cpp:132–136). This exercises the
/// Chinook supply interface's counter contract, not a landing/combat-drop flow.
#[test]
fn chinook_box_counter_honors_authored_capacity() {
    let mut default_ai = ChinookAIUpdate::new(ChinookAIUpdateData::default(), INVALID_ID, 0);
    assert_eq!(default_ai.get_number_boxes(), 0);
    assert!(!default_ai.gain_one_box(2));
    assert!(!default_ai.lose_one_box());

    let mut module = ChinookAIUpdateModuleData::default();
    parse_field(&mut module, "MaxBoxes", &["=", "2"]);
    let data = ChinookAIUpdateData::from_module(&module);
    assert_eq!(data.supply.max_boxes, 2);
    let mut ai = ChinookAIUpdate::new(data, INVALID_ID, 0);
    assert!(ai.gain_one_box(2));
    assert!(ai.gain_one_box(1));
    assert_eq!(ai.get_number_boxes(), 2);
    assert!(!ai.gain_one_box(1));
    assert_eq!(ai.get_number_boxes(), 2);

    let mut released = 0;
    while ai.lose_one_box() {
        released += 1;
        assert!(released <= 2, "a box loss must decrease the counter");
    }
    assert_eq!(released, 2);
    assert_eq!(ai.get_number_boxes(), 0);
    assert!(!ai.lose_one_box());
}
