use game_engine::common::dict::Dict;
use game_engine::common::name_key_generator::NameKeyGenerator;
use game_engine::common::system::DataChunkOutput;
use gamelogic::scripting::core::{
    Condition, ConditionType, OrCondition, Parameter, ParameterType, Script, ScriptAction,
    ScriptActionType, ScriptList,
};
use gamelogic::scripting::engine::ScriptEngine;
use generals_main::game_logic::{GameLogic, GameMode, Player, Team};

fn map_file(root: &std::path::Path) -> std::path::PathBuf {
    let mut out = DataChunkOutput::new();
    out.open_data_chunk("HeightMapData", 3);
    for value in [41, 41, 1, 1681] {
        out.write_int(value);
    }
    for _ in 0..1681 {
        out.write_byte(0);
    }
    out.close_data_chunk();
    out.open_data_chunk("ObjectsList", 3);
    out.open_data_chunk("Object", 3);
    for value in [50.0, 50.0, 0.0, 0.0] {
        out.write_real(value);
    }
    out.write_int(0);
    out.write_ascii_string("Waypoint");
    let mut properties = Dict::new();
    properties.set_int(NameKeyGenerator::name_to_key("waypointID"), 701);
    properties.set_ascii_string(NameKeyGenerator::name_to_key("waypointName"), "ExitAllFixture");
    out.write_dict(&properties);
    out.close_data_chunk();
    out.close_data_chunk();

    let path = root.join("TeamExitAll.map");
    std::fs::write(&path, out.into_ckmp_bytes()).unwrap();
    path
}

fn world(map: &std::path::Path) -> (
    GameLogic,
    generals_main::game_logic::ObjectId,
    generals_main::game_logic::ObjectId,
) {
    let mut world = GameLogic::new();
    world
        .admit_object_ini_sources([(
            "TeamExitAll.ini".into(),
            "Object TeamExitCarrier\n KindOf = SELECTABLE VEHICLE\n Body = ActiveBody BodyTag\n MaxHealth = 200\n InitialHealth = 200\n End\n Behavior = TransportContain ContainTag\n  ContainMax = 2\n  Slots = 2\n  AllowInsideKindOf = INFANTRY\n End\n Behavior = AIUpdateInterface Brain\n End\nEnd\nObject TeamExitAircraft\n KindOf = SELECTABLE VEHICLE AIRCRAFT\n TransportSlotCount = 2\n Locomotor = SET_NORMAL BasicHumanLocomotor\n Body = ActiveBody BodyTag\n MaxHealth = 200\n InitialHealth = 200\n End\n Behavior = PhysicsBehavior PhysTag\n  Mass = 1\n End\n Behavior = TransportContain ContainTag\n  ContainMax = 2\n  Slots = 2\n  AllowInsideKindOf = INFANTRY\n End\n Behavior = AIUpdateInterface Brain\n End\nEnd\nObject TeamExitPassenger\n KindOf = SELECTABLE INFANTRY\n TransportSlotCount = 1\n Locomotor = SET_NORMAL BasicHumanLocomotor\n Body = ActiveBody BodyTag\n MaxHealth = 100\n InitialHealth = 100\n End\n Behavior = AIUpdateInterface Brain\n End\nEnd\n".into(),
        )])
        .unwrap();
    world.start_new_game(GameMode::SinglePlayer);
    assert!(world.load_map(map.to_str().unwrap()));
    world.add_player(Player::new(1, Team::USA, "Owner", true).with_script_identity("Owner"));
    let carrier = world
        .create_object_for_player("TeamExitCarrier", 1, glam::Vec3::new(50.0, 0.0, 50.0))
        .unwrap();
    let passenger = world
        .create_object_for_player("TeamExitPassenger", 1, glam::Vec3::new(50.0, 0.0, 50.0))
        .unwrap();
    assert!(world.unit_command_order_enter(passenger, carrier));
    for _ in 0..120 {
        world.update_with_dt(1.0 / 30.0);
        if world.host_object(passenger).unwrap().contained_by == Some(carrier) {
            break;
        }
    }
    assert_eq!(
        world.host_object(passenger).unwrap().contained_by,
        Some(carrier),
        "passenger enters through the public receiving-world command/update path"
    );
    world
        .host_object_mut(carrier)
        .unwrap()
        .set_formation(73, glam::Vec2::new(9.0, -4.0));
    (world, carrier, passenger)
}

fn exit_all_script() -> ScriptEngine {
    let mut action = ScriptAction::new(ScriptActionType::TeamExitAll);
    action
        .add_parameter(Parameter::with_string(
            ParameterType::Team,
            "teamOwner".into(),
        ))
        .unwrap();
    let mut condition = OrCondition::new();
    condition.set_first_and_condition(Some(Box::new(Condition::new(ConditionType::ConditionTrue))));
    let mut script = Script::new();
    script.script_name = "OwnedTeamExitAll".into();
    script.is_one_shot = true;
    script.delay_evaluation_seconds = 0;
    script.condition = Some(Box::new(condition));
    script.action = Some(Box::new(action));
    let mut scripts = ScriptList::new();
    scripts.append_script(Box::new(script));
    let engine = ScriptEngine::new().unwrap();
    engine
        .set_script_list_for_player(0, Some(Box::new(scripts)))
        .unwrap();
    engine
}

#[test]
fn live_team_exit_all_only_changes_the_receiving_world_with_colliding_ids() {
    let _ = gamelogic::scripting::take_host_script_garrison_enter_requests();
    let first_root = tempfile::tempdir().unwrap();
    let second_root = tempfile::tempdir().unwrap();
    let first_map = map_file(first_root.path());
    let second_map = map_file(second_root.path());
    let (mut first, first_carrier, first_passenger) = world(&first_map);
    let (mut second, second_carrier, second_passenger) = world(&second_map);
    assert_eq!(
        (first_carrier, first_passenger),
        (second_carrier, second_passenger)
    );
    assert_eq!(first.host_object(first_carrier).unwrap().formation_id, 73);
    assert_eq!(second.host_object(second_carrier).unwrap().formation_id, 73);
    assert_eq!(
        first.host_object(first_carrier).unwrap().contained_units(),
        vec![first_passenger]
    );
    assert_eq!(
        second
            .host_object(second_carrier)
            .unwrap()
            .contained_units(),
        vec![second_passenger]
    );

    first
        .update_local_script_engine_with_owner(&exit_all_script())
        .unwrap();
    assert_eq!(first.host_object(first_carrier).unwrap().formation_id, 73);
    assert_eq!(
        first.host_object(first_carrier).unwrap().formation_offset,
        glam::Vec2::new(9.0, -4.0)
    );
    assert_eq!(
        first.host_object(first_carrier).unwrap().contained_units(),
        vec![first_passenger]
    );
    assert_eq!(
        first.host_object(first_passenger).unwrap().contained_by,
        Some(first_carrier)
    );
    assert_eq!(
        first.host_object(first_passenger).unwrap().ai_state(),
        generals_main::game_logic::AIState::Exiting
    );
    assert_eq!(second.host_object(second_carrier).unwrap().formation_id, 73);
    assert_eq!(
        second.host_object(second_carrier).unwrap().formation_offset,
        glam::Vec2::new(9.0, -4.0)
    );
    assert_eq!(
        second
            .host_object(second_carrier)
            .unwrap()
            .contained_units(),
        vec![second_passenger]
    );
    assert_eq!(
        second.host_object(second_passenger).unwrap().contained_by,
        Some(second_carrier)
    );
    assert_ne!(
        second.host_object(second_passenger).unwrap().ai_state(),
        generals_main::game_logic::AIState::Exiting
    );
    assert!(gamelogic::scripting::take_host_script_garrison_enter_requests().is_empty());
}

#[test]
fn live_team_exit_all_routes_airborne_transport_through_receiving_world() {
    let _ = gamelogic::scripting::take_host_script_garrison_enter_requests();
    let first_root = tempfile::tempdir().unwrap();
    let second_root = tempfile::tempdir().unwrap();
    let first_map = map_file(first_root.path());
    let second_map = map_file(second_root.path());
    let (mut first, _, _) = world(&first_map);
    let (mut second, _, _) = world(&second_map);
    let first_aircraft = first
        .create_object_for_player("TeamExitAircraft", 1, glam::Vec3::new(50.0, 40.0, 50.0))
        .unwrap();
    let second_aircraft = second
        .create_object_for_player("TeamExitAircraft", 1, glam::Vec3::new(50.0, 40.0, 50.0))
        .unwrap();
    assert_eq!(first_aircraft, second_aircraft);
    first
        .host_object_mut(first_aircraft)
        .unwrap()
        .status
        .airborne_target = true;
    second
        .host_object_mut(second_aircraft)
        .unwrap()
        .status
        .airborne_target = false;

    first
        .update_local_script_engine_with_owner(&exit_all_script())
        .unwrap();

    let owner = first.host_object(first_aircraft).unwrap();
    assert!(owner.pending_evacuate_on_stop);
    assert_eq!(owner.ai_state(), generals_main::game_logic::AIState::Moving);
    assert_eq!(
        owner.last_command_source,
        1, // C++ CMD_FROM_SCRIPT and the Main residual's serialized source value.
    );
    assert!(
        !second
            .host_object(second_aircraft)
            .unwrap()
            .pending_evacuate_on_stop
    );
    assert!(!second.host_object(second_aircraft).unwrap().status.airborne_target);
    assert_ne!(
        second.host_object(second_aircraft).unwrap().ai_state(),
        generals_main::game_logic::AIState::Moving
    );
}
