use game_engine::common::{
    dict::Dict,
    ini::{INILoadType, ini_locomotor::LocomotorStore},
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
use generals_main::command_system::{CommandType, GameCommand, ModifierKeys};
use generals_main::game_logic::{GameLogic, GameMode, ObjectId, Player, Team};
use glam::Vec3;
use std::time::{Duration, UNIX_EPOCH};

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
    out.open_data_chunk("Object", 3);
    for value in [
        (area_start + 80) as f32,
        (area_start + 100) as f32,
        0.0,
        0.0,
    ] {
        out.write_real(value);
    }
    out.write_int(0);
    out.write_ascii_string("Waypoint");
    let mut waypoint = Dict::new();
    waypoint.set_int(
        game_engine::common::name_key_generator::NameKeyGenerator::name_to_key("waypointID"),
        1902,
    );
    waypoint.set_ascii_string(
        game_engine::common::name_key_generator::NameKeyGenerator::name_to_key("waypointName"),
        "OwnedWaypoint",
    );
    out.write_dict(&waypoint);
    out.close_data_chunk();
    out.close_data_chunk();
    let path = root.join(format!("AiGroupOwner-{area_start}.map"));
    std::fs::write(&path, out.into_ckmp_bytes()).unwrap();
    path
}

fn world(path: &std::path::Path) -> (GameLogic, ObjectId, ObjectId, ObjectId) {
    world_with_transport(path, false)
}

fn world_with_transport(
    path: &std::path::Path,
    with_transport: bool,
) -> (GameLogic, ObjectId, ObjectId, ObjectId) {
    let mut world = GameLogic::new();
    let mut source = "Object GroupUnit\n KindOf = SELECTABLE INFANTRY\n Body = ActiveBody BodyTag\n MaxHealth = 100\n InitialHealth = 100\n End\n Behavior = AIUpdateInterface Brain\n End\nEnd\nObject GroupInert\n KindOf = SELECTABLE\nEnd\n".to_owned();
    source.push_str("Object LockedSpawn\n KindOf = STRUCTURE SELECTABLE\n Body = ActiveBody Body\n MaxHealth = 200\n InitialHealth = 200\n End\n Behavior = SpawnBehavior Spawn\n OneShot = Yes\n SpawnNumber = 1\n SpawnTemplateName = SpawnAttacker\n SlavesHaveFreeWill = No\n End\nEnd\nObject FreeSpawn\n KindOf = STRUCTURE SELECTABLE\n Body = ActiveBody Body\n MaxHealth = 200\n InitialHealth = 200\n End\n Behavior = SpawnBehavior Spawn\n OneShot = Yes\n SpawnNumber = 1\n SpawnTemplateName = SpawnAttacker\n SlavesHaveFreeWill = Yes\n End\nEnd\nObject SpawnAttacker\n KindOf = INFANTRY SELECTABLE\n Body = ActiveBody Body\n MaxHealth = 100\n InitialHealth = 100\n End\n Behavior = AIUpdateInterface Brain\n End\nEnd\n");
    let mut locomotors = LocomotorStore::new_owned();
    locomotors.apply_source("Locomotor GroupGround\n Surfaces = GROUND\n Speed = 40\n Acceleration = 60\n TurnRate = 120\n Appearance = TWO_LEGS\nEnd\nLocomotor GroupHijackerGround\n Surfaces = GROUND\n Speed = 30\n Acceleration = 60\n TurnRate = 120\n Appearance = TWO_LEGS\nEnd\nLocomotor GroupHijackVehicle\n Surfaces = GROUND\n Speed = 30\n Acceleration = 60\n TurnRate = 120\n Appearance = FOUR_WHEELS\nEnd\n").unwrap();
    world.admit_locomotor_definitions(locomotors).unwrap();
    source.push_str("Object GLAInfantryHijacker\n IsTrainable = Yes\n KindOf = INFANTRY SELECTABLE\n Locomotor = SET_NORMAL GroupHijackerGround\n ExperienceRequired = 0 60 150 300\n Body = ActiveBody HijackerBody\n MaxHealth = 100\n InitialHealth = 100\n End\n Behavior = AIUpdateInterface HijackerAI\n End\n Behavior = HijackerUpdate HijackerRider\n End\n Behavior = ConvertToHijackedVehicleCrateCollide HijackerCollide\n End\nEnd\nObject GroupHijackVehicle\n KindOf = VEHICLE SELECTABLE SCORE\n Locomotor = SET_NORMAL GroupHijackVehicle\n Body = ActiveBody VehicleBody\n MaxHealth = 1000\n InitialHealth = 1000\n End\n Behavior = AIUpdateInterface VehicleAI\n End\n Behavior = EjectPilotDie PilotEject\n End\nEnd\n");
    if with_transport {
        source = source.replace(" KindOf = SELECTABLE INFANTRY\n", " KindOf = SELECTABLE INFANTRY\n TransportSlotCount = 1\n Locomotor = SET_NORMAL GroupGround\n");
        source.push_str("Object GroupCarrier\n KindOf = SELECTABLE VEHICLE\n Body = ActiveBody BodyTag\n MaxHealth = 200\n InitialHealth = 200\n End\n Behavior = TransportContain ContainTag\n ContainMax = 2\n Slots = 2\n AllowInsideKindOf = INFANTRY\n End\n Behavior = AIUpdateInterface Brain\n End\nEnd\n");
        source.push_str("Object GroupVisibleGarrison\n KindOf = SELECTABLE STRUCTURE\n Body = ActiveBody BodyTag\n MaxHealth = 200\n InitialHealth = 200\n End\n Behavior = GarrisonContain ContainTag\n ContainMax = 2\n IsEnclosingContainer = No\n End\nEnd\n");
    }
    world
        .admit_object_ini_sources([("AiGroupOwner.ini".into(), source)])
        .unwrap();
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
fn team_attack_orders_only_spawned_slaves_without_free_will() {
    use generals_main::game_logic::Weapon;
    use generals_main::save_load::snapshot::capture_lifecycle_tail;

    fn setup(path: &std::path::Path) -> (GameLogic, ObjectId, ObjectId, ObjectId, Vec<ObjectId>) {
        let (mut game, _, _, _) = world(path);
        game.templates
            .get_mut("SpawnAttacker")
            .unwrap()
            .set_primary_weapon(Weapon {
                damage: 10.0,
                range: 200.0,
                ..Weapon::default()
            });
        let mut enemy = Player::new(2, Team::GLA, "Enemy", false);
        enemy.alliance_team = 2;
        game.add_player(enemy);
        let target = game
            .create_object_for_player("GroupUnit", 2, Vec3::new(100.0, 0.0, 40.0))
            .unwrap();
        game.host_object_mut(target).unwrap().name = "SpawnOrderTarget".into();
        let mut children = Vec::new();
        let mut parents = Vec::new();
        for name in ["LockedSpawn", "FreeSpawn"] {
            let parent = game
                .create_object_for_player(name, 1, Vec3::new(50.0, 0.0, 40.0))
                .unwrap();
            game.host_object_mut(parent).unwrap().team_instance_name = "HuntTeam".into();
            game.activate_leftover_team_for_host_object(parent);
            parents.push(parent);
            let roster = capture_lifecycle_tail(&game)
                .unwrap()
                .spawn_behavior_rosters
                .unwrap();
            let child = ObjectId(
                roster
                    .iter()
                    .find(|entry| entry.producer_id == parent.0)
                    .unwrap()
                    .children[0]
                    .child_id,
            );
            game.host_object_mut(child)
                .unwrap()
                .set_status_disabled_held(false);
            children.push(child);
        }
        (game, target, children[0], children[1], parents)
    }

    let root = tempfile::tempdir().unwrap();
    let path = map_file(root.path());
    let (mut owner, target, locked, free, parents) = setup(&path);
    let (peer, peer_target, peer_locked, peer_free, peer_parents) = setup(&path);
    assert_eq!(
        (target, locked, free),
        (peer_target, peer_locked, peer_free)
    );
    run_action_with_parameters(
        &mut owner,
        ScriptActionType::TeamAttackNamed,
        [
            Parameter::with_string(ParameterType::Team, "HuntTeam".into()),
            Parameter::with_string(ParameterType::Unit, "SpawnOrderTarget".into()),
        ],
    );
    assert_eq!(owner.host_object(locked).unwrap().target, Some(target));
    assert_eq!(owner.host_object(free).unwrap().target, None);
    assert_eq!(peer.host_object(locked).unwrap().target, None);
    assert_eq!(peer.host_object(free).unwrap().target, None);
    assert_eq!(parents, peer_parents);
    let ground = Vec3::new(90.0, 0.0, 60.0);
    generals_main::command_executor::CommandExecutor::new(&mut owner, 1)
        .execute_command(GameCommand {
            command_type: CommandType::AttackPosition {
                location: Some(ground),
                max_shots: 3,
            },
            player_id: 1,
            command_id: 1,
            timestamp: UNIX_EPOCH,
            selected_units: parents.clone(),
            modifier_keys: ModifierKeys::default(),
        })
        .unwrap();
    assert_eq!(
        owner.host_object(locked).unwrap().target_location,
        Some(ground)
    );
    assert_eq!(owner.host_object(free).unwrap().target_location, None);
    assert_eq!(peer.host_object(locked).unwrap().target_location, None);
    assert_eq!(owner.host_object(locked).unwrap().max_shots_to_fire, 3);
    let parent_position = owner.host_object(parents[0]).unwrap().get_position();
    generals_main::command_executor::CommandExecutor::new(&mut owner, 1)
        .execute_command(GameCommand {
            command_type: CommandType::AttackPosition {
                location: None,
                max_shots: 7,
            },
            player_id: 1,
            command_id: 2,
            timestamp: UNIX_EPOCH,
            selected_units: vec![parents[0]],
            modifier_keys: ModifierKeys::default(),
        })
        .unwrap();
    assert_eq!(
        owner.host_object(locked).unwrap().target_location,
        Some(parent_position)
    );
    assert_eq!(owner.host_object(locked).unwrap().max_shots_to_fire, 7);
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

fn guard_position(world: &mut GameLogic, team: &str, waypoint: &str) {
    run_action_with_parameters(
        world,
        ScriptActionType::TeamGuardPosition,
        [
            Parameter::with_string(ParameterType::Team, team.into()),
            Parameter::with_string(ParameterType::Waypoint, waypoint.into()),
        ],
    );
}

#[test]
fn team_guard_position_uses_receiving_waypoint_and_owned_group() {
    let root = tempfile::tempdir().unwrap();
    let owner_map = map_file_with_area(root.path(), 0);
    let peer_map = map_file_with_area(root.path(), 200);
    let (mut owner, first, second, inert) = world(&owner_map);
    let (mut peer, peer_first, peer_second, _) = world(&peer_map);
    assert_eq!((first, second), (peer_first, peer_second));
    hunt(&mut owner);
    hunt(&mut peer);
    let old_group = owner.ai_group_id_of(first).unwrap();
    guard_position(&mut owner, "HuntTeam", "MissingWaypoint");
    guard_position(&mut owner, "MissingTeam", "OwnedWaypoint");
    assert_eq!(owner.ai_group_id_of(first), Some(old_group));
    assert!(owner.host_object(first).unwrap().is_hunting());

    guard_object(&mut owner, "HuntTeam", "Inert");
    assert_eq!(
        owner.host_object(first).unwrap().guard_target(),
        Some(inert)
    );
    let previous = owner.ai_group_id_of(first);
    guard_position(&mut owner, "HuntTeam", "OwnedWaypoint");
    assert_ne!(owner.ai_group_id_of(first), previous);
    assert_eq!(
        owner.ai_group_members_of(first).as_deref(),
        Some(&[second, first][..])
    );
    assert_eq!(owner.ai_group_id_of(inert), None);
    for id in [first, second] {
        let object = owner.host_object(id).unwrap();
        assert_eq!(object.guard_position(), Some(Vec3::new(80.0, 0.0, 100.0)));
        assert_eq!(
            object.ai_state(),
            generals_main::game_logic::AIState::GuardingArea
        );
        // C++ privateGuardPosition does not erase m_objectToGuard.
        assert_eq!(object.guard_target(), Some(inert));
        assert!(!object.is_hunting());
        assert!(peer.host_object(id).unwrap().is_hunting());
    }
    assert_eq!(peer.ai_group_id_of(first), Some(old_group));
    guard_position(&mut peer, "HuntTeam", "OwnedWaypoint");
    assert_eq!(
        peer.host_object(first).unwrap().guard_position(),
        Some(Vec3::new(280.0, 0.0, 300.0))
    );
    assert_eq!(
        owner.host_object(first).unwrap().guard_position(),
        Some(Vec3::new(80.0, 0.0, 100.0))
    );
    assert_eq!(owner.host_object(first).unwrap().formation_id, 17);
}

#[test]
fn enclosing_boarding_leaves_only_the_receiving_worlds_group() {
    let root = tempfile::tempdir().unwrap();
    let map = map_file(root.path());
    let (mut owner, first, second, _) = world_with_transport(&map, true);
    let (mut peer, peer_first, peer_second, _) = world_with_transport(&map, true);
    assert_eq!((first, second), (peer_first, peer_second));
    hunt(&mut owner);
    hunt(&mut peer);
    let group = owner
        .ai_group_id_of(first)
        .expect("first belongs to a live team group");
    let peer_group = peer
        .ai_group_id_of(peer_first)
        .expect("peer owns its own group");
    let owner_carrier = owner
        .create_object_for_player("GroupCarrier", 1, Vec3::new(40.0, 0.0, 40.0))
        .unwrap();
    let peer_carrier = peer
        .create_object_for_player("GroupCarrier", 1, Vec3::new(40.0, 0.0, 40.0))
        .unwrap();
    assert_eq!(owner_carrier, peer_carrier);
    assert!(owner.can_unit_enter_normal_target(first, owner_carrier));
    assert!(owner.unit_command_order_enter(first, owner_carrier));
    // Suppress only this pair's collision so update_support_states performs
    // the first entry; the peer below exercises the collision route directly.
    owner
        .host_object_mut(first)
        .unwrap()
        .set_ignore_collisions_with(Some(owner_carrier));
    assert_eq!(
        owner.ai_group_id_of(first),
        Some(group),
        "ordering Enter is not boarding"
    );
    for _ in 0..120 {
        owner.update_with_dt(1.0 / 30.0);
        if owner.host_object(first).unwrap().contained_by == Some(owner_carrier) {
            break;
        }
    }
    assert_eq!(
        owner.host_object(first).unwrap().contained_by,
        Some(owner_carrier)
    );
    assert_eq!(owner.ai_group_id_of(first), None);
    assert_eq!(
        owner.ai_group_members_of(second).as_deref(),
        Some(&[second][..])
    );
    assert_eq!(owner.host_object(first).unwrap().formation_id, 17);
    assert_eq!(peer.ai_group_id_of(peer_first), Some(peer_group));
    assert_eq!(peer.ai_group_id_of(peer_second), Some(peer_group));
    assert_eq!(peer.host_object(peer_first).unwrap().contained_by, None);

    // The second live boarding path is PhysicsBehavior::onCollide.
    assert!(peer.unit_command_order_enter(peer_first, peer_carrier));
    peer.try_physics_collide(peer_first, peer_carrier, 1.0);
    assert_eq!(
        peer.host_object(peer_first).unwrap().contained_by,
        Some(peer_carrier)
    );
    assert_eq!(peer.ai_group_id_of(peer_first), None);
    assert_eq!(
        peer.ai_group_members_of(peer_second).as_deref(),
        Some(&[peer_second][..])
    );
    assert_eq!(owner.ai_group_id_of(second), Some(group));

    // A non-enclosing garrison does not remove its passenger from the world
    // or from their group (OpenContain::addToContain).
    let (mut visible, visible_first, visible_second, _) = world_with_transport(&map, true);
    hunt(&mut visible);
    let visible_group = visible.ai_group_id_of(visible_first).unwrap();
    let garrison = visible
        .create_object_for_player("GroupVisibleGarrison", 1, Vec3::new(40.0, 0.0, 40.0))
        .unwrap();
    assert!(visible.unit_command_order_enter(visible_first, garrison));
    visible.try_physics_collide(visible_first, garrison, 1.0);
    assert_eq!(
        visible.host_object(visible_first).unwrap().contained_by,
        Some(garrison)
    );
    assert!(!visible.host_object(visible_first).unwrap().status.masked);
    assert_eq!(visible.ai_group_id_of(visible_first), Some(visible_group));
    assert_eq!(visible.ai_group_id_of(visible_second), Some(visible_group));
    assert_eq!(owner.ai_group_id_of(second), Some(group));
}

#[test]
fn live_hijacker_ride_leaves_only_its_receiving_ai_group() {
    let root = tempfile::tempdir().unwrap();
    let map = map_file(root.path());
    let (mut owner, first, second, _) = world(&map);
    let (mut peer, peer_first, peer_second, _) = world(&map);
    owner.add_player(Player::new(2, Team::GLA, "Hijacker", true));
    peer.add_player(Player::new(2, Team::GLA, "Hijacker", true));

    let hijacker = owner
        .create_object_for_player("GLAInfantryHijacker", 1, Vec3::new(100.0, 0.0, 100.0))
        .unwrap();
    let peer_hijacker = peer
        .create_object_for_player("GLAInfantryHijacker", 1, Vec3::new(100.0, 0.0, 100.0))
        .unwrap();
    let vehicle = owner
        .create_object_for_player("GroupHijackVehicle", 2, Vec3::new(102.0, 0.0, 100.0))
        .unwrap();
    let peer_vehicle = peer
        .create_object_for_player("GroupHijackVehicle", 2, Vec3::new(102.0, 0.0, 100.0))
        .unwrap();
    assert_eq!((hijacker, vehicle), (peer_hijacker, peer_vehicle));
    for (world, actor) in [(&mut owner, hijacker), (&mut peer, peer_hijacker)] {
        world.host_object_mut(actor).unwrap().team_instance_name = "HuntTeam".into();
        world.activate_leftover_team_for_host_object(actor);
        hunt(world);
        assert!(world.ai_group_id_of(actor).is_some());
    }
    assert_eq!(owner.ai_group_id_of(hijacker), owner.ai_group_id_of(first));
    assert_eq!(
        peer.ai_group_id_of(peer_hijacker),
        peer.ai_group_id_of(peer_first)
    );
    assert_eq!(owner.ai_group_id_of(first), peer.ai_group_id_of(peer_first));
    assert_eq!(
        owner.ai_group_id_of(second),
        peer.ai_group_id_of(peer_second)
    );

    owner.queue_command(GameCommand {
        command_type: CommandType::Hijack { target_id: vehicle },
        player_id: 1,
        command_id: 9001,
        timestamp: UNIX_EPOCH + Duration::from_secs(1),
        selected_units: vec![hijacker],
        modifier_keys: ModifierKeys::default(),
    });
    owner.process_commands();
    let ordered = owner.host_object(hijacker).expect("ordered hijacker");
    assert_eq!(
        ordered.ai_state(),
        generals_main::game_logic::AIState::SpecialAbility
    );
    assert_eq!(ordered.target, Some(vehicle));
    for _ in 0..180 {
        if owner
            .host_object(hijacker)
            .is_some_and(|actor| actor.hijacker_in_vehicle)
        {
            break;
        }
        owner.update_with_dt(1.0 / 30.0);
    }

    let rider = owner
        .host_object(hijacker)
        .expect("retained riding hijacker");
    assert!(
        rider.hijacker_in_vehicle,
        "the real Hijack ability completed"
    );
    assert_eq!(rider.ai_state(), generals_main::game_logic::AIState::Idle);
    assert!(rider.drawable_hidden);
    assert_eq!(owner.ai_group_id_of(hijacker), None);
    let remaining_owner_members = owner.ai_group_members_of(first).unwrap();
    assert!(remaining_owner_members.contains(&second));
    assert!(!remaining_owner_members.contains(&hijacker));
    assert!(peer.ai_group_id_of(peer_hijacker).is_some());
    assert!(!peer.host_object(peer_hijacker).unwrap().hijacker_in_vehicle);
}
