use game_engine::common::{
    dict::Dict,
    ini::INILoadType,
    rts::player_template::PlayerTemplateStore,
    system::DataChunkOutput,
    well_known_keys::{
        key_player_display_name, key_player_faction, key_player_is_human, key_player_name,
        key_team_is_singleton, key_team_name, key_team_owner,
    },
};
use gamelogic::scripting::{
    core::{
        Condition, ConditionType, OrCondition, Parameter, ParameterType, Script, ScriptAction,
        ScriptActionType, ScriptList,
    },
    engine::ScriptEngine,
};
use generals_main::game_logic::{GameLogic, GameMode, ObjectId};
use glam::Vec3;

fn map_file(root: &std::path::Path) -> std::path::PathBuf {
    map_file_with_area(root, 0)
}

fn map_file_with_area(root: &std::path::Path, area_start: i32) -> std::path::PathBuf {
    let mut out = DataChunkOutput::new();
    out.open_data_chunk("HeightMapData", 3);
    for value in [41, 41, 1, 1681] {
        out.write_int(value);
    }
    for _ in 0..1681 {
        out.write_byte(0);
    }
    out.close_data_chunk();
    out.open_data_chunk("PolygonTriggers", 4);
    out.write_int(1);
    out.write_ascii_string("InnerPerimeter1");
    out.write_ascii_string("OwnerLayer");
    out.write_int(1901);
    out.write_byte(1);
    out.write_byte(0);
    out.write_int(0);
    out.write_int(4);
    for (x, y) in [(0, 0), (40, 0), (40, 40), (0, 40)] {
        out.write_int(area_start + x);
        out.write_int(area_start + y);
        out.write_int(0);
    }
    out.close_data_chunk();
    out.open_data_chunk("SidesList", 2);
    out.write_int(2);
    for (name, faction, human) in [
        ("", "FactionCivilian", false),
        ("Owner", "FactionAmerica", true),
    ] {
        let mut side = Dict::new();
        side.set_ascii_string(key_player_name(), name);
        side.set_ascii_string(key_player_faction(), faction);
        side.set_unicode_string(key_player_display_name(), name);
        side.set_bool(key_player_is_human(), human);
        out.write_dict(&side);
        out.write_int(0);
    }
    out.write_int(1);
    let mut team = Dict::new();
    team.set_ascii_string(key_team_name(), "HuntTeam");
    team.set_ascii_string(key_team_owner(), "Owner");
    team.set_bool(key_team_is_singleton(), false);
    out.write_dict(&team);
    out.close_data_chunk();
    out.open_data_chunk("ObjectsList", 3);
    out.close_data_chunk();
    let path = root.join(format!("AiGroupOwner-{area_start}.map"));
    std::fs::write(&path, out.into_ckmp_bytes()).unwrap();
    path
}

fn world(path: &std::path::Path) -> (GameLogic, ObjectId, ObjectId, ObjectId) {
    let mut world = GameLogic::new();
    world.admit_object_ini_sources([(
        "AiGroupOwner.ini".into(),
        "Object GroupUnit\n KindOf = SELECTABLE INFANTRY\n Body = ActiveBody BodyTag\n MaxHealth = 100\n InitialHealth = 100\n End\n Behavior = AIUpdateInterface Brain\n End\nEnd\nObject GroupInert\n KindOf = SELECTABLE\nEnd\n".into(),
    )]).unwrap();
    let mut templates = PlayerTemplateStore::new_owned();
    templates.apply_source(
        "PlayerTemplate FactionCivilian\n Side = Civilian\nEnd\nPlayerTemplate FactionAmerica\n Side = America\nEnd\n",
        INILoadType::Overwrite,
    ).unwrap();
    world.admit_player_template_definitions(templates).unwrap();
    world.start_new_game(GameMode::SinglePlayer);
    assert!(world.load_map(path.to_str().unwrap()));
    let first = world
        .create_object_for_player("GroupUnit", 1, Vec3::new(40.0, 0.0, 40.0))
        .unwrap();
    let second = world
        .create_object_for_player("GroupUnit", 1, Vec3::new(60.0, 0.0, 40.0))
        .unwrap();
    let inert = world
        .create_object_for_player("GroupInert", 1, Vec3::new(80.0, 0.0, 40.0))
        .unwrap();
    for (id, name) in [(first, "First"), (second, "Second"), (inert, "Inert")] {
        let object = world.host_object_mut(id).unwrap();
        object.name = name.into();
        object.team_instance_name = "HuntTeam".into();
        world.activate_leftover_team_for_host_object(id);
    }
    world
        .host_object_mut(first)
        .unwrap()
        .set_formation(17, glam::Vec2::new(5.0, -7.0));
    (world, first, second, inert)
}

fn run_action(world: &mut GameLogic, kind: ScriptActionType, parameter: Parameter) {
    run_action_with_parameters(world, kind, [parameter]);
}

fn run_action_with_parameters(
    world: &mut GameLogic,
    kind: ScriptActionType,
    parameters: impl IntoIterator<Item = Parameter>,
) {
    run_action_with_parameters_at(world, kind, parameters, 0);
}

fn run_action_with_parameters_at(
    world: &mut GameLogic,
    kind: ScriptActionType,
    parameters: impl IntoIterator<Item = Parameter>,
    side_index: usize,
) {
    let mut action = ScriptAction::new(kind);
    for parameter in parameters {
        action.add_parameter(parameter).unwrap();
    }
    let mut branch = OrCondition::new();
    branch.set_first_and_condition(Some(Box::new(Condition::new(ConditionType::ConditionTrue))));
    let mut script = Script::new();
    script.script_name = "GroupOwnerAction".into();
    script.is_one_shot = true;
    script.condition = Some(Box::new(branch));
    script.action = Some(Box::new(action));
    let mut scripts = ScriptList::new();
    scripts.append_script(Box::new(script));
    let engine = ScriptEngine::new().unwrap();
    engine
        .set_script_list_for_player(side_index, Some(Box::new(scripts)))
        .unwrap();
    world
        .update_local_script_engine_with_owner(&engine)
        .unwrap();
}

fn hunt(world: &mut GameLogic) {
    run_action(
        world,
        ScriptActionType::TeamHunt,
        Parameter::with_string(ParameterType::Team, "HuntTeam".into()),
    );
}

#[test]
fn team_hunt_membership_named_leave_retirement_and_reset_are_session_owned() {
    let root = tempfile::tempdir().unwrap();
    let map = map_file(root.path());
    let (mut owner, first, second, inert) = world(&map);
    let (mut peer, peer_first, peer_second, peer_inert) = world(&map);
    assert_eq!(
        (first, second, inert),
        (peer_first, peer_second, peer_inert)
    );
    hunt(&mut owner);
    hunt(&mut peer);
    assert_eq!(
        owner.ai_group_members_of(first).as_deref(),
        Some(&[second, first][..])
    );
    assert_eq!(
        peer.ai_group_members_of(first).as_deref(),
        Some(&[second, first][..])
    );
    assert_eq!(owner.ai_group_id_of(inert), None);
    assert!(owner.host_object(first).unwrap().is_hunting());
    assert!(owner.host_object(second).unwrap().is_hunting());
    assert_eq!(owner.ai_group_id_of(first), peer.ai_group_id_of(first));
    let old_group = owner.ai_group_id_of(first).unwrap();
    hunt(&mut owner);
    assert_ne!(owner.ai_group_id_of(first), Some(old_group));
    assert_eq!(
        owner.ai_group_members_of(first).as_deref(),
        Some(&[second, first][..])
    );
    assert_eq!(peer.ai_group_id_of(first), Some(old_group));

    run_action(
        &mut owner,
        ScriptActionType::NamedGuard,
        Parameter::with_string(ParameterType::Unit, "First".into()),
    );
    assert_eq!(owner.ai_group_id_of(first), None);
    assert_eq!(
        owner.ai_group_members_of(second).as_deref(),
        Some(&[second][..])
    );
    assert_eq!(owner.host_object(first).unwrap().formation_id, 17);
    assert_eq!(
        owner.host_object(first).unwrap().formation_offset,
        glam::Vec2::new(5.0, -7.0)
    );
    assert_eq!(
        peer.ai_group_members_of(first).as_deref(),
        Some(&[second, first][..])
    );

    owner.destroy_object(second).unwrap();
    assert!(
        owner.ai_group_id_of(second).is_some(),
        "queued removal retains membership until destructor"
    );
    owner.process_destroy_list();
    assert_eq!(owner.ai_group_id_of(second), None);
    assert_eq!(
        peer.ai_group_members_of(first).as_deref(),
        Some(&[second, first][..])
    );
    // Reusing an ID through the public store must not attach the new birth
    // to the previous object's group.
    let removed = peer.objects.remove(&first).unwrap();
    let replacement = generals_main::game_logic::Object::new(
        generals_main::game_logic::ThingTemplate::new("Replacement"),
        first,
        generals_main::game_logic::Team::USA,
    );
    peer.objects.admit(first, replacement);
    assert_eq!(peer.ai_group_id_of(first), None);
    assert_eq!(
        peer.ai_group_members_of(second).as_deref(),
        Some(&[second][..])
    );
    drop(removed);
    peer.reset();
    assert_eq!(peer.ai_group_id_of(first), None);
    assert_eq!(owner.ai_group_id_of(first), None);
}

#[test]
fn current_hunt_and_named_guard_continue_after_save_load() {
    let root = tempfile::tempdir().unwrap();
    let map = map_file(root.path());
    let (mut original, first, second, _) = world(&map);
    hunt(&mut original);
    let builder = generals_main::save_load::snapshot::SnapshotBuilder::new();
    let snapshot = builder.create_world_snapshot(&original).unwrap();
    let bytes = bincode_legacy::serialize(&snapshot).unwrap();
    let decoded = bincode_legacy::deserialize(&bytes).unwrap();
    let (mut restored, _, _, _) = world(&map);
    builder
        .restore_from_snapshot(&decoded, &mut restored)
        .unwrap();

    // The currently migrated gameplay consumers never read a preexisting
    // group's speed or members after load. NamedGuard leaves the group, then
    // TeamHunt builds a fresh one from this session's resident Team roster.
    for game in [&mut original, &mut restored] {
        run_action(
            game,
            ScriptActionType::NamedGuard,
            Parameter::with_string(ParameterType::Unit, "First".into()),
        );
    }
    for id in [first, second] {
        let a = original.host_object(id).unwrap();
        let b = restored.host_object(id).unwrap();
        assert_eq!(a.ai_state(), b.ai_state());
        assert_eq!(a.is_hunting(), b.is_hunting());
        assert_eq!(a.formation_id, b.formation_id);
        assert_eq!(a.formation_offset, b.formation_offset);
    }
    hunt(&mut original);
    hunt(&mut restored);
    assert_eq!(
        original.ai_group_members_of(first),
        restored.ai_group_members_of(first)
    );
    for id in [first, second] {
        let a = original.host_object(id).unwrap();
        let b = restored.host_object(id).unwrap();
        assert_eq!(a.ai_state(), b.ai_state());
        assert_eq!(a.is_hunting(), b.is_hunting());
        assert_eq!(a.formation_id, b.formation_id);
        assert_eq!(a.formation_offset, b.formation_offset);
    }
}

fn guard_object(world: &mut GameLogic, team: &str, target: &str) {
    run_action_with_parameters(
        world,
        ScriptActionType::TeamGuardObject,
        [
            Parameter::with_string(ParameterType::Team, team.into()),
            Parameter::with_string(ParameterType::Unit, target.into()),
        ],
    );
}

#[test]
fn team_guard_object_creates_a_receiving_group_only_after_target_and_team_resolve() {
    let root = tempfile::tempdir().unwrap();
    let map = map_file(root.path());
    let (mut owner, first, second, target) = world(&map);
    let (mut peer, peer_first, peer_second, peer_target) = world(&map);
    assert_eq!(
        (first, second, target),
        (peer_first, peer_second, peer_target)
    );
    hunt(&mut owner);
    hunt(&mut peer);
    let original_group = owner.ai_group_id_of(first).unwrap();

    guard_object(&mut owner, "HuntTeam", "MissingTarget");
    guard_object(&mut owner, "MissingTeam", "Inert");
    assert_eq!(owner.ai_group_id_of(first), Some(original_group));
    assert_eq!(
        owner.ai_group_members_of(first).as_deref(),
        Some(&[second, first][..])
    );

    guard_object(&mut owner, "HuntTeam", "Inert");
    assert_ne!(owner.ai_group_id_of(first), Some(original_group));
    assert_eq!(
        owner.ai_group_members_of(first).as_deref(),
        Some(&[second, first][..])
    );
    assert_eq!(owner.ai_group_id_of(target), None);
    for id in [first, second] {
        assert_eq!(owner.host_object(id).unwrap().guard_target(), Some(target));
        assert!(peer.host_object(id).unwrap().is_hunting());
    }
    assert_eq!(peer.ai_group_id_of(first), Some(original_group));
    assert_eq!(owner.host_object(first).unwrap().formation_id, 17);
    assert_eq!(
        owner.host_object(first).unwrap().formation_offset,
        glam::Vec2::new(5.0, -7.0)
    );

    run_action(
        &mut owner,
        ScriptActionType::NamedGuard,
        Parameter::with_string(ParameterType::Unit, "First".into()),
    );
    assert_eq!(owner.ai_group_id_of(first), None);
    assert_eq!(
        owner.ai_group_members_of(second).as_deref(),
        Some(&[second][..])
    );
    assert_eq!(
        peer.ai_group_members_of(first).as_deref(),
        Some(&[second, first][..])
    );
}

fn guard_area(world: &mut GameLogic, team: &str, area: &str) {
    run_action_with_parameters_at(
        world,
        ScriptActionType::TeamGuardArea,
        [
            Parameter::with_string(ParameterType::Team, team.into()),
            Parameter::with_string(ParameterType::TriggerArea, area.into()),
        ],
        1,
    );
}

#[test]
fn team_guard_area_borrows_the_qualified_polygon_and_group_from_each_world() {
    let root = tempfile::tempdir().unwrap();
    let owner_map = map_file_with_area(root.path(), 0);
    let peer_map = map_file_with_area(root.path(), 200);
    let (mut owner, first, second, inert) = world(&owner_map);
    let (mut peer, peer_first, peer_second, _) = world(&peer_map);
    assert_eq!((first, second), (peer_first, peer_second));
    owner.get_player_mut(1).unwrap().start_position = 0;
    peer.get_player_mut(1).unwrap().start_position = 0;
    hunt(&mut owner);
    hunt(&mut peer);
    let original_group = owner.ai_group_id_of(first).unwrap();
    guard_area(&mut owner, "HuntTeam", "MissingArea");
    guard_area(&mut owner, "MissingTeam", "InnerPerimeter1");
    assert_eq!(owner.ai_group_id_of(first), Some(original_group));
    assert!(owner.host_object(first).unwrap().is_hunting());

    guard_object(&mut owner, "HuntTeam", "Inert");
    assert_eq!(
        owner.host_object(first).unwrap().guard_target(),
        Some(inert)
    );
    guard_area(&mut owner, "HuntTeam", "[Skirmish]MyInnerPerimeter");
    assert_ne!(owner.ai_group_id_of(first), Some(original_group));
    assert_eq!(
        owner.ai_group_members_of(first).as_deref(),
        Some(&[second, first][..])
    );
    assert_eq!(owner.ai_group_id_of(inert), None);
    for id in [first, second] {
        let unit = owner.host_object(id).unwrap();
        assert_eq!(
            unit.guard_area_trigger().as_deref(),
            Some("InnerPerimeter1")
        );
        assert_eq!(unit.guard_position(), Some(Vec3::new(20.0, 0.0, 20.0)));
        assert_eq!(unit.guard_target(), None);
        assert_eq!(
            unit.ai_state(),
            generals_main::game_logic::AIState::GuardingArea
        );
        assert!(!unit.is_hunting());
        assert!(peer.host_object(id).unwrap().is_hunting());
    }
    assert_eq!(peer.ai_group_id_of(first), Some(original_group));
    guard_area(&mut peer, "HuntTeam", "[Skirmish]MyInnerPerimeter");
    assert_eq!(
        peer.host_object(first)
            .unwrap()
            .guard_area_trigger()
            .as_deref(),
        Some("InnerPerimeter1")
    );
    assert_eq!(
        peer.host_object(first).unwrap().guard_position(),
        Some(Vec3::new(220.0, 0.0, 220.0))
    );
    assert_eq!(
        owner.host_object(first).unwrap().guard_position(),
        Some(Vec3::new(20.0, 0.0, 20.0))
    );
    assert_eq!(owner.host_object(first).unwrap().formation_id, 17);
}
