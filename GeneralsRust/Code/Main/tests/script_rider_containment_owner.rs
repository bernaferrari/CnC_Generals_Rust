//! Script disable follows RiderChangeContain's receiving rider, not ordinary passengers.
use game_engine::common::ini::INILoadType;
use game_engine::common::ini::ini_locomotor::LocomotorStore;
use game_engine::common::rts::player_template::PlayerTemplateStore;
use game_engine::common::system::DataChunkOutput;
use gamelogic::scripting::core::{
    Condition, ConditionType, OrCondition, Parameter, ParameterType, Script, ScriptAction,
    ScriptActionType, ScriptList,
};
use gamelogic::scripting::engine::ScriptEngine;
use generals_main::game_logic::{GameLogic, GameMode, ObjectId, Player, Team};
use generals_main::save_load::snapshot::SnapshotBuilder;
use glam::Vec3;

fn run_toggle(world: &mut GameLogic, action: ScriptActionType) {
    let mut op = ScriptAction::new(action);
    op.add_parameter(Parameter::with_string(ParameterType::Side, "Owner".into()))
        .unwrap();
    op.add_parameter(Parameter::with_string(
        ParameterType::ObjectType,
        "OwnerBike".into(),
    ))
    .unwrap();
    let mut condition = OrCondition::new();
    condition.set_first_and_condition(Some(Box::new(Condition::new(ConditionType::ConditionTrue))));
    let mut script = Script::new();
    script.script_name = "RiderToggle".into();
    script.condition = Some(Box::new(condition));
    script.action = Some(Box::new(op));
    script.is_one_shot = true;
    let mut list = ScriptList::new();
    list.append_script(Box::new(script));
    let engine = ScriptEngine::new().unwrap();
    engine
        .set_script_list_for_player(0, Some(Box::new(list)))
        .unwrap();
    world
        .update_local_script_engine_with_owner(&engine)
        .unwrap();
}

fn make_world(path: &std::path::Path, contain_kind: &str) -> (GameLogic, ObjectId, ObjectId) {
    let mut world = GameLogic::new();
    let mut players = PlayerTemplateStore::new_owned();
    players.apply_source("PlayerTemplate FactionCivilian\n Side = Civilian\nEnd\nPlayerTemplate FactionAmerica\n Side = America\nEnd\n", INILoadType::Overwrite).unwrap();
    world.admit_player_template_definitions(players).unwrap();
    let mut locos = LocomotorStore::new_owned();
    locos.apply_source("Locomotor GroundOwnerLoco\n Surfaces = GROUND\n Speed = 40\n Acceleration = 60\n TurnRate = 120\n Appearance = TWO_LEGS\nEnd\n").unwrap();
    world.admit_locomotor_definitions(locos).unwrap();
    let contain = match contain_kind {
        "rider" => {
            "Behavior = RiderChangeContain RiderTag\n Slots = 1\n Rider1 = OwnerRider RIDER1 WEAPON_RIDER1 STATUS_RIDER1 BikeSet SET_NORMAL\n End\n"
        }
        "overlord" => {
            "Behavior = OverlordContain OverlordTag\n Slots = 1\n ContainMax = 1\n AllowInsideKindOf = INFANTRY\n End\n"
        }
        "helix" => {
            "Behavior = HelixContain HelixTag\n Slots = 2\n ContainMax = 2\n AllowInsideKindOf = INFANTRY\n End\n"
        }
        _ => {
            "Behavior = TransportContain TransportTag\n Slots = 1\n ContainMax = 1\n AllowInsideKindOf = INFANTRY\n End\n"
        }
    };
    let ini = format!(
        "Object OwnerBike\n KindOf = SELECTABLE VEHICLE\n Locomotor = SET_NORMAL GroundOwnerLoco\n Body = ActiveBody BodyTag\n MaxHealth = 100\n InitialHealth = 100\n End\n{contain} Behavior = AIUpdateInterface Brain\n End\nEnd\nObject OwnerRider\n KindOf = SELECTABLE INFANTRY\n TransportSlotCount = 1\n Locomotor = SET_NORMAL GroundOwnerLoco\n Body = ActiveBody BodyTag\n MaxHealth = 100\n InitialHealth = 100\n End\n Behavior = AIUpdateInterface Brain\n End\nEnd\n"
    );
    world
        .admit_object_ini_sources([("RiderOwner.ini".into(), ini)])
        .unwrap();
    world.start_new_game(GameMode::SinglePlayer);
    assert!(world.load_map(path.to_str().unwrap()));
    world.add_player(Player::new(42, Team::USA, "Owner", true).with_script_identity("Owner"));
    let bike = world
        .create_object_for_player("OwnerBike", 42, Vec3::new(80.0, 0.0, 80.0))
        .unwrap();
    let rider = world
        .create_object_for_player("OwnerRider", 42, Vec3::new(81.0, 0.0, 80.0))
        .unwrap();
    if contain_kind == "overlord" || contain_kind == "helix" {
        // Overlord riders are portable add-ons, not normal Enter passengers.
        // Install the reciprocal roster explicitly to exercise the script
        // callback without pretending infantry can enter the bare hull.
        world.host_object_mut(bike).unwrap().occupants.push(rider);
        if contain_kind == "helix" {
            world
                .host_object_mut(bike)
                .unwrap()
                .overlord_portable_occupant = Some(rider);
        }
        world.host_object_mut(rider).unwrap().contained_by = Some(bike);
        return (world, bike, rider);
    }
    assert!(world.can_unit_enter_normal_target(rider, bike));
    assert!(world.unit_command_order_enter(rider, bike));
    for _ in 0..20 {
        world.update_with_dt(1.0 / 30.0);
        if world.host_object(rider).unwrap().contained_by == Some(bike) {
            break;
        }
    }
    assert_eq!(world.host_object(rider).unwrap().contained_by, Some(bike));
    (world, bike, rider)
}

#[test]
fn script_toggle_affects_rider_change_rider_not_transport_passenger() {
    let mut map = DataChunkOutput::new();
    map.open_data_chunk("HeightMapData", 3);
    for value in [21, 21, 1, 441] {
        map.write_int(value);
    }
    for _ in 0..441 {
        map.write_byte(8);
    }
    map.close_data_chunk();
    map.open_data_chunk("ObjectsList", 3);
    map.close_data_chunk();
    let root = tempfile::tempdir().unwrap();
    let path = root.path().join("RiderOwner.map");
    std::fs::write(&path, map.into_ckmp_bytes()).unwrap();

    let (mut first, bike, rider) = make_world(&path, "rider");
    let (mut peer, peer_bike, passenger) = make_world(&path, "transport");
    let (mut overlord, overlord_id, overlord_rider) = make_world(&path, "overlord");
    let (mut helix, helix_id, helix_rider) = make_world(&path, "helix");
    assert_eq!((bike, rider), (peer_bike, passenger));
    assert_eq!((bike, rider), (overlord_id, overlord_rider));
    assert_eq!((bike, rider), (helix_id, helix_rider));
    let helix_passenger = helix
        .create_object_for_player("OwnerRider", 42, Vec3::ZERO)
        .unwrap();
    helix
        .host_object_mut(helix_id)
        .unwrap()
        .occupants
        .insert(0, helix_passenger);
    helix.host_object_mut(helix_passenger).unwrap().contained_by = Some(helix_id);
    run_toggle(&mut helix, ScriptActionType::PlayerDisableFactories);
    assert!(
        helix
            .host_object(helix_rider)
            .unwrap()
            .status
            .disabled_script_disabled
    );
    assert!(
        !helix
            .host_object(helix_passenger)
            .unwrap()
            .status
            .disabled_script_disabled
    );
    run_toggle(&mut helix, ScriptActionType::PlayerEnableFactories);
    assert!(
        !helix
            .host_object(helix_rider)
            .unwrap()
            .status
            .disabled_script_disabled
    );
    // An ordinary passenger is not a substitute for a missing portable ID.
    helix
        .host_object_mut(helix_id)
        .unwrap()
        .overlord_portable_occupant = None;
    run_toggle(&mut helix, ScriptActionType::PlayerDisableFactories);
    assert!(
        !helix
            .host_object(helix_passenger)
            .unwrap()
            .status
            .disabled_script_disabled
    );
    assert!(
        !helix
            .host_object(helix_rider)
            .unwrap()
            .status
            .disabled_script_disabled
    );
    run_toggle(&mut overlord, ScriptActionType::PlayerDisableFactories);
    assert!(
        overlord
            .host_object(overlord_rider)
            .unwrap()
            .status
            .disabled_script_disabled
    );
    run_toggle(&mut overlord, ScriptActionType::PlayerEnableFactories);
    assert!(
        !overlord
            .host_object(overlord_rider)
            .unwrap()
            .status
            .disabled_script_disabled
    );
    run_toggle(&mut first, ScriptActionType::PlayerDisableFactories);
    assert!(first.host_object(bike).unwrap().is_script_disabled());
    assert!(
        first
            .host_object(rider)
            .unwrap()
            .status
            .disabled_script_disabled
    );
    assert!(!first.host_object(rider).unwrap().is_script_disabled());
    assert!(
        !peer
            .host_object(passenger)
            .unwrap()
            .status
            .disabled_script_disabled
    );

    run_toggle(&mut peer, ScriptActionType::PlayerDisableFactories);
    assert!(peer.host_object(peer_bike).unwrap().is_script_disabled());
    assert!(
        !peer
            .host_object(passenger)
            .unwrap()
            .status
            .disabled_script_disabled
    );

    let builder = SnapshotBuilder::new();
    let snapshot = builder.create_world_snapshot(&first).unwrap();
    let (mut restored, _, _) = make_world(&path, "rider");
    builder
        .restore_from_snapshot(&snapshot, &mut restored)
        .unwrap();
    assert!(
        restored
            .host_object(rider)
            .unwrap()
            .status
            .disabled_script_disabled
    );
    run_toggle(&mut restored, ScriptActionType::PlayerEnableFactories);
    assert!(
        !restored
            .host_object(rider)
            .unwrap()
            .status
            .disabled_script_disabled
    );
    assert!(!restored.host_object(rider).unwrap().is_script_disabled());
    assert!(
        first
            .host_object(rider)
            .unwrap()
            .status
            .disabled_script_disabled
    );
}
