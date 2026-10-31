use game_engine::common::dict::Dict;
use game_engine::common::ini::ini_locomotor::LocomotorStore;
use game_engine::common::name_key_generator::NameKeyGenerator;
use game_engine::common::system::DataChunkOutput;
use gamelogic::scripting::core::{
    Condition, ConditionType, OrCondition, Parameter, ParameterType, Script, ScriptAction,
    ScriptActionType, ScriptList,
};
use gamelogic::scripting::engine::ScriptEngine;
use generals_main::command_executor::CommandExecutor;
use generals_main::command_system::{CommandResult, CommandType, GameCommand, ModifierKeys};
use generals_main::game_logic::{AIState, GameLogic, GameMode, ObjectId, Player, Team};
use generals_main::save_load::{
    ObjectGarrisonOriginalTeamSnapshot, Snapshot, SnapshotBuilder,
    WORLD_SNAPSHOT_DIRECT_XFER_VERSION, WorldSnapshot, XferLoad, XferSave,
};
use glam::Vec3;

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
    properties.set_ascii_string(
        NameKeyGenerator::name_to_key("waypointName"),
        "GarrisonOwner",
    );
    out.write_dict(&properties);
    out.close_data_chunk();
    out.close_data_chunk();
    let path = root.join("GarrisonOwner.map");
    std::fs::write(&path, out.into_ckmp_bytes()).unwrap();
    path
}

fn world(map: &std::path::Path, reverse_players: bool) -> (GameLogic, ObjectId, ObjectId) {
    let mut world = GameLogic::new();
    let mut locomotors = LocomotorStore::new_owned();
    locomotors
        .apply_source(
            "Locomotor BasicHumanLocomotor\n\
             Surfaces = GROUND\n\
             Speed = 20\n\
             Acceleration = 100\n\
             TurnRate = 500\n\
             Appearance = TWO_LEGS\n\
             WanderWidthFactor = 1.0\n\
             WanderLengthFactor = 1.0\n\
             SpeedDamaged = 10\n\
             AccelerationDamaged = 50\n\
             End\n",
        )
        .expect("admit authored infantry locomotor");
    world
        .admit_locomotor_definitions(locomotors)
        .expect("admit receiving locomotor definitions");
    world
        .admit_object_ini_sources([(
            "GarrisonOwner.ini".into(),
            "Object OwnedGarrison\n\
 KindOf = STRUCTURE SELECTABLE ALWAYS_VISIBLE\n\
 Geometry = BOX\n\
 GeometryMajorRadius = 20\n\
 GeometryMinorRadius = 10\n\
 Body = ActiveBody BodyTag\n\
  MaxHealth = 1000\n\
  InitialHealth = 1000\n\
 End\n\
 Behavior = GarrisonContain GarrisonTag\n\
  ContainMax = 4\n\
 End\n\
End\n\
Object OwnedGarrisonRider\n\
 KindOf = INFANTRY SELECTABLE\n\
 TransportSlotCount = 1\n\
 Locomotor = SET_NORMAL BasicHumanLocomotor\n\
 Body = ActiveBody BodyTag\n\
  MaxHealth = 100\n\
  InitialHealth = 100\n\
 End\n\
 Behavior = AIUpdateInterface Brain\n\
 End\n\
End\n"
                .into(),
        )])
        .expect("admit authored Garrison definitions");
    world.start_new_game(GameMode::SinglePlayer);
    assert!(world.load_map(map.to_str().unwrap()));
    let mut p1 =
        Player::new(1, Team::USA, "GarrisonHost", true).with_script_identity("GarrisonHost");
    let mut p2 =
        Player::new(2, Team::USA, "GarrisonRider", true).with_script_identity("GarrisonRider");
    p1.alliance_team = 0;
    p2.alliance_team = 0;
    if reverse_players {
        world.add_player(p2);
        world.add_player(p1);
    } else {
        world.add_player(p1);
        world.add_player(p2);
    }
    let host = world
        .create_object_for_player("OwnedGarrison", 1, Vec3::new(50.0, 0.0, 50.0))
        .expect("create receiving Garrison");
    let rider = world
        .create_object_for_player("OwnedGarrisonRider", 2, Vec3::new(50.0, 0.0, 50.0))
        .expect("create receiving rider");
    (world, host, rider)
}

fn enter(world: &mut GameLogic, host: ObjectId, rider: ObjectId) {
    assert!(world.can_unit_enter_normal_target(rider, host));
    assert!(
        world.host_object(host).unwrap().selection_radius > 0.0,
        "authored Geometry must provide a nonzero Enter radius: {:?}",
        world.host_object(host).unwrap().selection_radius
    );
    let result = CommandExecutor::new(world, 2)
        .execute_command(GameCommand {
            command_type: CommandType::Enter { target_id: host },
            player_id: 2,
            command_id: 1,
            timestamp: std::time::UNIX_EPOCH,
            selected_units: vec![rider],
            modifier_keys: ModifierKeys::default(),
        })
        .expect("production Enter command executes");
    assert_eq!(result, CommandResult::Success);
    assert_eq!(
        world.host_object(rider).unwrap().ai_state(),
        AIState::Entering
    );
    assert_eq!(world.host_object(rider).unwrap().target, Some(host));
    let mut trace = Vec::new();
    for _ in 0..120 {
        world.update_with_dt(1.0 / 30.0);
        if trace.len() < 4 {
            let rider_obj = world.host_object(rider).unwrap();
            trace.push((
                rider_obj.ai_state(),
                rider_obj.target,
                rider_obj.get_position(),
                world.host_object(host).unwrap().get_position(),
            ));
        }
        if world.host_object(rider).unwrap().contained_by == Some(host) {
            return;
        }
    }
    panic!(
        "authored rider did not enter: rider={:?}, owner={:?}, target={:?}, host_occupants={:?}, trace={:?}",
        world.host_object(rider).map(|object| object.ai_state()),
        world
            .host_object(rider)
            .and_then(|object| object.owner_player_id),
        world.host_object(rider).and_then(|object| object.target),
        world
            .host_object(host)
            .map(|object| object.contained_units()),
        trace
    );
}

fn named_garrison(world: &mut GameLogic, unit_name: &str) {
    let mut action = ScriptAction::new(ScriptActionType::NamedGarrisonNearestBuilding);
    action
        .add_parameter(Parameter::with_string(
            ParameterType::Unit,
            unit_name.into(),
        ))
        .unwrap();
    let mut condition = OrCondition::new();
    condition.set_first_and_condition(Some(Box::new(Condition::new(ConditionType::ConditionTrue))));
    let mut script = Script::new();
    script.script_name = "PlayerIdentityGarrison".into();
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
    world
        .update_local_script_engine_with_owner(&engine)
        .unwrap();
}

#[test]
fn named_garrison_uses_entering_player_identity_not_display_name() {
    let root = tempfile::tempdir().unwrap();
    let map = map_file(root.path());
    let (mut aliased, host, rider) = world(&map, false);
    let (mut distinct, peer_host, peer_rider) = world(&map, false);
    assert_eq!((host, rider), (peer_host, peer_rider));
    enter(&mut aliased, host, rider);
    enter(&mut distinct, peer_host, peer_rider);
    assert_eq!(
        aliased.host_object(host).unwrap().player_who_entered,
        Some(2)
    );
    assert_eq!(
        distinct.host_object(peer_host).unwrap().player_who_entered,
        Some(2)
    );
    aliased.get_player_mut(1).unwrap().name = "GarrisonRider".into();
    let mut subjects = Vec::new();
    for game in [&mut aliased, &mut distinct] {
        let unit = game
            .create_object_for_player("OwnedGarrisonRider", 1, Vec3::new(20.0, 0.0, 50.0))
            .unwrap();
        game.host_object_mut(unit).unwrap().name = "NamedCaster".into();
        let fallback = game
            .create_object_for_player("OwnedGarrison", 1, Vec3::new(150.0, 0.0, 50.0))
            .unwrap();
        subjects.push((unit, fallback));
    }
    assert_eq!(subjects[0], subjects[1]);
    named_garrison(&mut distinct, "NamedCaster");
    assert_eq!(
        distinct.host_object(subjects[1].0).unwrap().target,
        Some(subjects[1].1)
    );
    named_garrison(&mut aliased, "NamedCaster");
    assert_eq!(
        aliased.host_object(subjects[0].0).unwrap().target,
        Some(subjects[0].1)
    );
    aliased.update();
    assert_eq!(aliased.host_object(host).unwrap().player_who_entered, None);
    assert_eq!(
        distinct.host_object(peer_host).unwrap().player_who_entered,
        Some(2)
    );
}

fn replace_gots_rows(snapshot: &mut WorldSnapshot, rows: &[ObjectGarrisonOriginalTeamSnapshot]) {
    let tail = &mut snapshot.lifecycle_tail;
    let magic = tail
        .windows(4)
        .rposition(|window| window == b"GOTS")
        .expect("GOTS capsule marker");
    let old_len = u32::from_le_bytes(tail[magic - 4..magic].try_into().unwrap()) as usize;
    let old_start = magic - 8 - old_len;
    let payload = bincode_legacy::serialize(rows).expect("encode GOTS rows");
    tail.splice(old_start..magic - 8, payload.iter().copied());
    let new_magic = old_start + payload.len() + 8;
    tail[new_magic - 4..new_magic].copy_from_slice(&(payload.len() as u32).to_le_bytes());
}

fn strip_gots_capsule(snapshot: &mut WorldSnapshot) {
    let tail = &mut snapshot.lifecycle_tail;
    let magic = tail
        .windows(4)
        .rposition(|window| window == b"GOTS")
        .expect("GOTS capsule marker");
    let payload_len = u32::from_le_bytes(tail[magic - 4..magic].try_into().unwrap()) as usize;
    let payload_start = magic - 8 - payload_len;
    tail.drain(payload_start..magic + 4);
}

fn clone_bincode_snapshot(snapshot: &WorldSnapshot) -> WorldSnapshot {
    let encoded = bincode_legacy::serialize(snapshot).expect("encode snapshot clone");
    bincode_legacy::deserialize(&encoded).expect("decode snapshot clone")
}

#[test]
fn version_35_bincode_and_append_only_direct_xfer_still_decode() {
    let mut version_35 = WorldSnapshot::default();
    version_35.version = 35;
    let bincode_bytes = bincode_legacy::serialize(&version_35).unwrap();
    let bincode_decoded: WorldSnapshot = bincode_legacy::deserialize(&bincode_bytes).unwrap();
    assert_eq!(bincode_decoded.version, 35);
    assert!(bincode_decoded.garrison_original_team_instances.is_empty());

    // Version35 predates the append-only direct-Xfer Team vector. Emit the
    // current body, then remove only that final zero-length-vector count
    // and restore the historical envelope version to model its exact bytes.
    let mut current_xfer = std::io::Cursor::new(Vec::new());
    {
        let mut writer = XferSave::new(&mut current_xfer);
        version_35.xfer(&mut writer).unwrap();
    }
    let mut version_35_xfer = current_xfer.into_inner();
    assert_eq!(
        u32::from_le_bytes(version_35_xfer[..4].try_into().unwrap()),
        WORLD_SNAPSHOT_DIRECT_XFER_VERSION
    );
    version_35_xfer.truncate(version_35_xfer.len() - 4);
    version_35_xfer[..4].copy_from_slice(&35u32.to_le_bytes());
    let mut direct_decoded = WorldSnapshot::default();
    let mut reader = XferLoad::new(std::io::Cursor::new(version_35_xfer));
    direct_decoded.xfer(&mut reader).unwrap();
    assert_eq!(direct_decoded.version, 35);
    assert!(direct_decoded.garrison_original_team_instances.is_empty());
}

#[test]
fn current_default_snapshot_roundtrips_and_restores() {
    let snapshot = WorldSnapshot::default();
    assert!(
        snapshot.lifecycle_tail.windows(4).any(|tag| tag == b"GOTS"),
        "a current default snapshot must contain its mandatory empty GOTS frame"
    );
    let bincode_bytes = bincode_legacy::serialize(&snapshot).unwrap();
    let bincode_decoded: WorldSnapshot = bincode_legacy::deserialize(&bincode_bytes).unwrap();
    let mut receiver = GameLogic::new();
    SnapshotBuilder::new()
        .restore_from_snapshot(&bincode_decoded, &mut receiver)
        .expect("valid current default snapshot restores");

    let mut direct_source = WorldSnapshot::default();
    let mut direct_bytes = std::io::Cursor::new(Vec::new());
    direct_source
        .xfer(&mut XferSave::new(&mut direct_bytes))
        .expect("current default snapshot writes as direct Xfer");
    assert_eq!(direct_source.version, WORLD_SNAPSHOT_DIRECT_XFER_VERSION);
    direct_bytes.set_position(0);
    let mut direct_decoded = WorldSnapshot::default();
    direct_decoded
        .xfer(&mut XferLoad::new(direct_bytes))
        .expect("current default snapshot reads as direct Xfer");
    assert_eq!(direct_decoded.version, WORLD_SNAPSHOT_DIRECT_XFER_VERSION);
    assert!(
        direct_decoded
            .lifecycle_tail
            .windows(4)
            .any(|tag| tag == b"GOTS")
    );
    let mut direct_receiver = GameLogic::new();
    SnapshotBuilder::new()
        .restore_from_snapshot(&direct_decoded, &mut direct_receiver)
        .expect("current default direct-Xfer snapshot restores");

    for legacy_version in [34, 35] {
        let mut bytes = std::io::Cursor::new(Vec::new());
        {
            let mut writer = XferSave::new(&mut bytes);
            let mut source = WorldSnapshot::default();
            strip_gots_capsule(&mut source);
            // Re-encoding a current-format default without its known empty
            // GOTS frame is not a valid writer input; model accepted legacy
            // predecessors and require the writer to insert the new frame.
            source.version = legacy_version;
            source.xfer(&mut writer).unwrap();
        }
        bytes.set_position(0);
        let mut direct_decoded = WorldSnapshot::default();
        {
            let mut reader = XferLoad::new(bytes);
            direct_decoded.xfer(&mut reader).unwrap();
        }
        assert_eq!(direct_decoded.version, WORLD_SNAPSHOT_DIRECT_XFER_VERSION);
        let mut direct_receiver = GameLogic::new();
        SnapshotBuilder::new()
            .restore_from_snapshot(&direct_decoded, &mut direct_receiver)
            .unwrap_or_else(|error| {
                panic!("upgraded v{legacy_version} direct-Xfer default restores: {error}")
            });
    }
}

#[test]
fn garrison_entry_retains_occupant_when_receiving_controller_has_no_default_team() {
    let root = tempfile::tempdir().unwrap();
    let map = map_file(root.path());
    let (mut world, host, rider) = world(&map, false);
    let original_team = world.host_object(host).unwrap().team_instance_identity();

    // Exercise the public receiving-session continuation path to represent a
    // valid local Player whose default Team is absent, without exposing a
    // test-only mutator for Player's owned identity.
    let mut chunks = generals_main::save_load::snapshot::stamp_player_team_chunks(&world).unwrap();
    chunks
        .players
        .as_mut()
        .unwrap()
        .players
        .iter_mut()
        .find(|saved| saved.player_id == 2)
        .unwrap()
        .host_default_team_id = None;
    generals_main::save_load::snapshot::apply_pending_player_team_chunks(&mut world, &chunks)
        .unwrap();

    enter(&mut world, host, rider);
    assert_eq!(world.host_object(rider).unwrap().contained_by, Some(host));
    assert_eq!(
        world.host_object(host).unwrap().team_instance_identity(),
        original_team,
        "C++ leaves the Garrison Team unchanged when the rider Player has no default Team"
    );
    assert_eq!(
        world
            .host_object(host)
            .unwrap()
            .building_data
            .as_ref()
            .unwrap()
            .original_team_instance_id,
        original_team
    );
}

fn assert_bad_gots_is_preflighted(
    snapshot: &WorldSnapshot,
    map: &std::path::Path,
    error_context: &str,
) {
    let (mut receiver, host, _) = world(map, false);
    let original_team = receiver.host_object(host).unwrap().team_instance_identity();
    assert!(
        SnapshotBuilder::new()
            .restore_from_snapshot(snapshot, &mut receiver)
            .is_err(),
        "{error_context} must be rejected"
    );
    assert_eq!(
        receiver.host_object(host).unwrap().team_instance_identity(),
        original_team,
        "invalid GOTS data is rejected before outgoing objects are retired"
    );
}

#[test]
fn garrison_owner_switch_uses_each_receiving_default_team_and_restores_exact_team() {
    let root_a = tempfile::tempdir().unwrap();
    let root_b = tempfile::tempdir().unwrap();
    let map_a = map_file(root_a.path());
    let map_b = map_file(root_b.path());
    let (mut first, first_host, first_rider) = world(&map_a, false);
    let (mut peer, peer_host, peer_rider) = world(&map_b, true);
    assert_eq!((first_host, first_rider), (peer_host, peer_rider));

    let first_original_team = first
        .host_object(first_host)
        .unwrap()
        .team_instance_identity();
    let peer_original_team = peer
        .host_object(peer_host)
        .unwrap()
        .team_instance_identity();
    let first_rider_team = first
        .host_object(first_rider)
        .unwrap()
        .team_instance_identity();
    let peer_rider_team = peer
        .host_object(peer_rider)
        .unwrap()
        .team_instance_identity();
    assert_ne!(first_original_team, first_rider_team);
    assert_ne!(peer_original_team, peer_rider_team);

    enter(&mut first, first_host, first_rider);
    enter(&mut peer, peer_host, peer_rider);
    assert_eq!(
        first
            .host_object(first_host)
            .unwrap()
            .team_instance_identity(),
        first_rider_team,
        "GarrisonContain uses the first rider controller's receiving default Team"
    );
    assert_eq!(
        peer.host_object(peer_host)
            .unwrap()
            .team_instance_identity(),
        peer_rider_team,
        "the same ObjectIds resolve through the peer receiving factory"
    );

    let builder = SnapshotBuilder::new();
    let original_name = first
        .host_object(first_host)
        .unwrap()
        .building_data
        .as_ref()
        .unwrap()
        .original_team_instance_name
        .clone();
    first
        .host_object_mut(first_host)
        .unwrap()
        .building_data
        .as_mut()
        .unwrap()
        .original_team_instance_name = None;
    assert!(
        builder.create_world_snapshot(&first).is_err(),
        "saving cannot silently drop a half-present original Team identity"
    );
    first
        .host_object_mut(first_host)
        .unwrap()
        .building_data
        .as_mut()
        .unwrap()
        .original_team_instance_name = original_name;
    let snapshot = builder
        .create_world_snapshot(&first)
        .expect("snapshot occupied Garrison");

    let mut duplicate = clone_bincode_snapshot(&snapshot);
    let row = snapshot.garrison_original_team_instances[0].clone();
    duplicate.garrison_original_team_instances.clear();
    replace_gots_rows(&mut duplicate, &[row.clone(), row.clone()]);
    assert_bad_gots_is_preflighted(&duplicate, &map_a, "duplicate GOTS Object rows");

    let mut foreign_team = clone_bincode_snapshot(&snapshot);
    let mut foreign_row = row.clone();
    foreign_row.team_instance_id = u32::MAX;
    foreign_row.team_instance_name = "ForeignGarrisonTeam".into();
    foreign_team.garrison_original_team_instances.clear();
    replace_gots_rows(&mut foreign_team, &[foreign_row]);
    assert_bad_gots_is_preflighted(&foreign_team, &map_a, "foreign GOTS Team identity");

    let mut truncated = clone_bincode_snapshot(&snapshot);
    truncated.garrison_original_team_instances.clear();
    let tail = &mut truncated.lifecycle_tail;
    let magic = tail
        .windows(4)
        .rposition(|window| window == b"GOTS")
        .expect("GOTS capsule marker");
    let payload_len = u32::from_le_bytes(tail[magic - 4..magic].try_into().unwrap()) as usize;
    let payload_start = magic - 8 - payload_len;
    tail.remove(magic - 9);
    assert!(payload_start < magic - 9);
    assert_bad_gots_is_preflighted(&truncated, &map_a, "truncated GOTS payload");

    let mut erased = clone_bincode_snapshot(&snapshot);
    erased.garrison_original_team_instances.clear();
    let tail = &mut erased.lifecycle_tail;
    let magic = tail
        .windows(4)
        .rposition(|window| window == b"GOTS")
        .expect("GOTS capsule marker");
    let payload_len = u32::from_le_bytes(tail[magic - 4..magic].try_into().unwrap()) as usize;
    let payload_start = magic - 8 - payload_len;
    tail.drain(payload_start..magic + 4);
    assert_bad_gots_is_preflighted(&erased, &map_a, "fully erased current GOTS capsule");

    let encoded = bincode_legacy::serialize(&snapshot).expect("encode Garrison snapshot");
    let decoded: WorldSnapshot = bincode_legacy::deserialize(&encoded).expect("decode snapshot");
    let mut xfer_bytes = std::io::Cursor::new(Vec::new());
    {
        let mut writer = XferSave::new(&mut xfer_bytes);
        let mut direct_source = clone_bincode_snapshot(&snapshot);
        direct_source.garrison_original_team_instances =
            snapshot.garrison_original_team_instances.clone();
        direct_source
            .xfer(&mut writer)
            .expect("write direct Xfer snapshot");
    }
    xfer_bytes.set_position(0);
    let mut direct_decoded = WorldSnapshot::default();
    {
        let mut reader = XferLoad::new(xfer_bytes);
        direct_decoded
            .xfer(&mut reader)
            .expect("read direct Xfer snapshot");
    }
    assert_eq!(
        direct_decoded.garrison_original_team_instances, snapshot.garrison_original_team_instances,
        "direct Xfer keeps the same exact original Team rows"
    );
    assert_eq!(direct_decoded.lifecycle_tail, snapshot.lifecycle_tail);

    let (mut restored, _, _) = world(&map_a, false);
    builder
        .restore_from_snapshot(&decoded, &mut restored)
        .expect("restore occupied Garrison and exact original Team identity");
    assert_eq!(
        restored
            .host_object(first_host)
            .unwrap()
            .team_instance_identity(),
        first_rider_team
    );
    let (mut direct_restored, _, _) = world(&map_a, false);
    builder
        .restore_from_snapshot(&direct_decoded, &mut direct_restored)
        .expect("restore direct-Xfer occupied Garrison");
    assert_eq!(
        direct_restored
            .host_object(first_host)
            .unwrap()
            .team_instance_identity(),
        restored
            .host_object(first_host)
            .unwrap()
            .team_instance_identity(),
        "bincode and direct Xfer restore the same receiving Team identity"
    );
    assert!(restored.unit_command_remove_occupant(first_host, first_rider));
    assert_eq!(
        restored
            .host_object(first_host)
            .unwrap()
            .team_instance_identity(),
        first_original_team,
        "emptying after restore restores the exact pre-Garrison Team instance"
    );
    let restored_host = restored.host_object(first_host).unwrap();
    assert!(!restored_host.has_object_status_bit("CAN_ATTACK"));
    let restored_building = restored_host.building_data.as_ref().unwrap();
    assert!(restored_building.garrison_guns.is_empty());
    assert!(restored_building.garrison_point_occupant.is_empty());
    assert_eq!(
        peer.host_object(peer_host)
            .unwrap()
            .team_instance_identity(),
        peer_rider_team,
        "restoring/exiting the first world cannot alter the same-ID peer"
    );
}

#[test]
fn empty_uninitialized_garrison_keeps_its_current_team_after_last_rider_exit() {
    let root = tempfile::tempdir().unwrap();
    let map = map_file(root.path());
    let (mut world, host, rider) = world(&map, false);
    enter(&mut world, host, rider);
    let current_team = world.host_object(host).unwrap().team_instance_identity();
    assert!(
        world
            .host_object(host)
            .unwrap()
            .has_object_status_bit("CAN_ATTACK")
    );

    // Model the C++ uninitialized m_originalTeam case. Recalc after removal
    // must retain the current Team, then record it as the empty postcondition.
    let building = world
        .host_object_mut(host)
        .unwrap()
        .building_data
        .as_mut()
        .unwrap();
    building.original_team = None;
    building.original_garrison_player_id = None;
    building.original_team_instance_id = None;
    building.original_team_instance_name = None;

    assert!(world.unit_command_remove_occupant(host, rider));
    assert_eq!(
        world.host_object(host).unwrap().team_instance_identity(),
        current_team,
        "without an original Team C++ leaves the current Team unchanged"
    );
    let host_object = world.host_object(host).unwrap();
    assert!(!host_object.has_object_status_bit("CAN_ATTACK"));
    let building = host_object.building_data.as_ref().unwrap();
    assert!(building.garrison_guns.is_empty());
    assert!(building.garrison_point_occupant.is_empty());
    assert_eq!(building.original_team_instance_id, current_team);
}
