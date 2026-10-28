use game_engine::common::{
    dict::Dict,
    system::DataChunkOutput,
    well_known_keys::{
        key_player_display_name, key_player_faction, key_player_is_human, key_player_name,
        key_team_is_singleton, key_team_name, key_team_owner,
    },
};
use gamelogic::{
    common::Relationship,
    scripting::{
        core::{
            Condition, ConditionType, OrCondition, Parameter, ParameterType, Script, ScriptAction,
            ScriptActionType, ScriptList,
        },
        engine::ScriptEngine,
    },
};
use generals_main::game_logic::{GameLogic, GameMode, ObjectId};
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
    out.open_data_chunk("SidesList", 2);
    out.write_int(3);
    for (name, faction, human) in [
        ("", "FactionCivilian", false),
        ("Owner", "FactionAmerica", true),
        ("Other", "FactionChina", false),
    ] {
        let mut side = Dict::new();
        side.set_ascii_string(key_player_name(), name);
        side.set_ascii_string(key_player_faction(), faction);
        side.set_unicode_string(key_player_display_name(), name);
        side.set_bool(key_player_is_human(), human);
        out.write_dict(&side);
        out.write_int(0);
    }
    out.write_int(2);
    for (name, owner) in [("SourceRoster", "Owner"), ("TargetRoster", "Other")] {
        let mut team = Dict::new();
        team.set_ascii_string(key_team_name(), name);
        team.set_ascii_string(key_team_owner(), owner);
        team.set_bool(key_team_is_singleton(), false);
        out.write_dict(&team);
    }
    out.close_data_chunk();
    out.open_data_chunk("ObjectsList", 3);
    out.close_data_chunk();
    let path = root.join("TeamRelations.map");
    std::fs::write(&path, out.into_ckmp_bytes()).unwrap();
    path
}

fn world(map: &std::path::Path) -> (GameLogic, ObjectId, ObjectId) {
    let mut world = GameLogic::new();
    world
        .admit_object_ini_sources([(
            "TeamRelations.ini".into(),
            "Object RelationUnit\n KindOf = SELECTABLE INFANTRY\n Body = ActiveBody BodyTag\n MaxHealth = 100\n InitialHealth = 100\n End\n Behavior = CostModifierUpgrade MergeCost\n TriggeredBy = Upgrade_Veterancy_VETERAN\n EffectKindOf = VEHICLE AIRCRAFT\n Percentage = -10%\n End\nEnd\nObject RelationDualPurchase\n KindOf = VEHICLE AIRCRAFT\n BuildCost = 1000\n Body = ActiveBody BodyTag\n MaxHealth = 100\n InitialHealth = 100\n End\nEnd\n".into(),
        )])
        .unwrap();
    world.start_new_game(GameMode::SinglePlayer);
    assert!(world.load_map(map.to_str().unwrap()));
    world
        .get_player_mut(1)
        .unwrap()
        .set_map_relationship(2, Relationship::Enemies);
    let source = world
        .create_object_for_player("RelationUnit", 1, Vec3::new(10.0, 0.0, 30.0))
        .unwrap();
    let target = world
        .create_object_for_player("RelationUnit", 2, Vec3::new(30.0, 0.0, 30.0))
        .unwrap();
    for (id, team) in [(source, "SourceRoster"), (target, "TargetRoster")] {
        world.host_object_mut(id).unwrap().team_instance_name = team.into();
        world.activate_leftover_team_for_host_object(id);
    }
    (world, source, target)
}

fn execute(
    world: &mut GameLogic,
    action_type: ScriptActionType,
    target: Option<&str>,
    value: Option<i32>,
) {
    let mut action = ScriptAction::new(action_type);
    action
        .add_parameter(Parameter::with_string(
            ParameterType::Team,
            "SourceRoster".into(),
        ))
        .unwrap();
    if let Some(target) = target {
        let parameter_type = match action_type {
            ScriptActionType::TeamSetOverrideRelationToPlayer
            | ScriptActionType::TeamRemoveOverrideRelationToPlayer => ParameterType::Side,
            _ => ParameterType::Team,
        };
        action
            .add_parameter(Parameter::with_string(parameter_type, target.into()))
            .unwrap();
    }
    if let Some(value) = value {
        action
            .add_parameter(Parameter::with_int(ParameterType::Int, value))
            .unwrap();
    }
    run_action(world, action);
}

fn execute_player_override(
    world: &mut GameLogic,
    action_type: ScriptActionType,
    value: Option<i32>,
) {
    let mut action = ScriptAction::new(action_type);
    action
        .add_parameter(Parameter::with_string(ParameterType::Side, "Owner".into()))
        .unwrap();
    action
        .add_parameter(Parameter::with_string(
            ParameterType::Team,
            "TargetRoster".into(),
        ))
        .unwrap();
    if let Some(value) = value {
        action
            .add_parameter(Parameter::with_int(ParameterType::Int, value))
            .unwrap();
    }
    run_action(world, action);
}

fn run_action(world: &mut GameLogic, action: ScriptAction) {
    let mut branch = OrCondition::new();
    branch.set_first_and_condition(Some(Box::new(Condition::new(ConditionType::ConditionTrue))));
    let mut script = Script::new();
    script.script_name = "ChangeTeamRelation".into();
    script.is_one_shot = true;
    script.condition = Some(Box::new(branch));
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
fn player_team_overrides_keep_exact_team_identity_across_worlds_and_saves() {
    use generals_main::save_load::snapshot::{
        apply_pending_player_team_chunks, stamp_player_team_chunks,
    };

    fn sibling(world: &mut GameLogic) -> ObjectId {
        let sibling = world
            .create_object_for_player("RelationUnit", 2, Vec3::new(35.0, 0.0, 30.0))
            .unwrap();
        let mut chunks = stamp_player_team_chunks(world).unwrap();
        let teams = chunks.teams.as_mut().unwrap();
        let mut second = teams
            .teams
            .iter()
            .find(|team| team.team_name == "TargetRoster")
            .unwrap()
            .clone();
        for team in &mut teams.teams {
            if let Some(members) = &mut team.members {
                members.retain(|&id| id != sibling.0);
            }
        }
        second.team_id = teams.unique_team_id;
        teams.unique_team_id += 1;
        second.members = Some(vec![sibling.0]);
        second.team_relations.clear();
        second.player_relations.clear();
        teams.teams.push(second);
        apply_pending_player_team_chunks(world, &chunks).unwrap();
        sibling
    }

    let root = tempfile::tempdir().unwrap();
    let map = map_file(root.path());
    let (mut owner, source, target) = world(&map);
    let (mut peer, peer_source, peer_target) = world(&map);
    let other_target = sibling(&mut owner);
    let peer_other = sibling(&mut peer);
    assert_eq!(
        (source, target, other_target),
        (peer_source, peer_target, peer_other)
    );
    // getTeamNamed returns the first instance in the receiving prototype's
    // visitation order. Restoring the sibling may place it before the original.
    let order = stamp_player_team_chunks(&owner).unwrap();
    let first = order
        .teams
        .as_ref()
        .unwrap()
        .teams
        .iter()
        .find(|team| team.team_name == "TargetRoster")
        .unwrap();
    let selected = if first.members.as_ref().unwrap().contains(&target.0) {
        target
    } else {
        assert!(first.members.as_ref().unwrap().contains(&other_target.0));
        other_target
    };
    let unselected = if selected == target {
        other_target
    } else {
        target
    };
    owner.host_object_mut(selected).unwrap().name = "MergeCostSource".into();
    let mut upgrade = ScriptAction::new(ScriptActionType::NamedReceiveUpgrade);
    upgrade
        .add_parameter(Parameter::with_string(
            ParameterType::Unit,
            "MergeCostSource".into(),
        ))
        .unwrap();
    upgrade
        .add_parameter(Parameter::with_string(
            ParameterType::Upgrade,
            "Upgrade_Veterancy_VETERAN".into(),
        ))
        .unwrap();
    run_action(&mut owner, upgrade);
    assert_eq!(
        owner.modified_build_cost_supplies(2, "RelationDualPurchase", 1000),
        900
    );
    execute_player_override(
        &mut owner,
        ScriptActionType::PlayerSetOverrideRelationToTeam,
        Some(2),
    );
    assert_eq!(
        owner
            .get_player(1)
            .unwrap()
            .team_relations
            .get(&first.team_id),
        Some(&Relationship::Allies)
    );
    assert_eq!(relation(&owner, source, selected), Relationship::Allies);
    assert_eq!(relation(&owner, source, unselected), Relationship::Enemies);
    assert_eq!(
        relation(&peer, peer_source, peer_target),
        Relationship::Enemies
    );
    assert_eq!(
        relation(&peer, peer_source, peer_other),
        Relationship::Enemies
    );

    let mut ambiguous = stamp_player_team_chunks(&owner).unwrap();
    let saved_owner = ambiguous
        .players
        .as_mut()
        .unwrap()
        .players
        .iter_mut()
        .find(|player| player.player_id == 1)
        .unwrap();
    saved_owner.team_relations[0].team_id = 0;
    assert!(apply_pending_player_team_chunks(&mut owner, &ambiguous).is_err());
    assert_eq!(
        relation(&owner, source, selected),
        Relationship::Allies,
        "a failed ambiguous name-only restore leaves the running owner intact"
    );

    let builder = generals_main::save_load::SnapshotBuilder::new();
    let snapshot = builder.create_world_snapshot(&owner).unwrap();
    let bytes = bincode_legacy::serialize(&snapshot).unwrap();
    let decoded: generals_main::save_load::snapshot::WorldSnapshot =
        bincode_legacy::deserialize(&bytes).unwrap();
    let (mut restored, restored_source, restored_target) = world(&map);
    builder
        .restore_from_snapshot(&decoded, &mut restored)
        .unwrap();
    assert_eq!(restored_target, target);
    assert_eq!(
        relation(&restored, restored_source, selected),
        Relationship::Allies
    );
    assert_eq!(
        relation(&restored, restored_source, unselected),
        Relationship::Enemies
    );

    execute_player_override(
        &mut owner,
        ScriptActionType::PlayerRemoveOverrideRelationToTeam,
        None,
    );
    assert_eq!(relation(&owner, source, selected), Relationship::Enemies);
    assert_eq!(
        relation(&restored, restored_source, selected),
        Relationship::Allies
    );

    // Team deletion occurs synchronously inside the merge action. It must
    // remove only the deleted Team's Player override in the driving world.
    execute_player_override(
        &mut owner,
        ScriptActionType::PlayerSetOverrideRelationToTeam,
        Some(2),
    );
    let mut merge = ScriptAction::new(ScriptActionType::TeamMergeIntoTeam);
    for name in ["TargetRoster", "SourceRoster"] {
        merge
            .add_parameter(Parameter::with_string(ParameterType::Team, name.into()))
            .unwrap();
    }
    run_action(&mut owner, merge);
    assert!(
        !owner
            .get_player(1)
            .unwrap()
            .team_relations
            .contains_key(&first.team_id)
    );
    let remaining = stamp_player_team_chunks(&owner).unwrap();
    let teams = &remaining.teams.unwrap().teams;
    assert!(!teams.iter().any(|team| team.team_id == first.team_id));
    let destination = teams
        .iter()
        .find(|team| team.team_name == "SourceRoster")
        .unwrap();
    assert!(destination.active);
    assert!(destination.members.as_ref().unwrap().contains(&selected.0));
    assert_eq!(
        owner.host_object(selected).unwrap().team_instance_name,
        "SourceRoster"
    );
    assert_eq!(
        owner.host_object(selected).unwrap().owner_player_id,
        Some(1)
    );
    assert_eq!(
        owner.modified_build_cost_supplies(2, "RelationDualPurchase", 1000),
        1000
    );
    assert_eq!(
        owner.modified_build_cost_supplies(1, "RelationDualPurchase", 1000),
        900,
        "merge must deliver the original onCapture cost-modifier transfer"
    );
    assert_eq!(
        peer.modified_build_cost_supplies(2, "RelationDualPurchase", 1000),
        1000
    );
    assert_eq!(
        restored.modified_build_cost_supplies(2, "RelationDualPurchase", 1000),
        900
    );
    assert_eq!(relation(&owner, source, unselected), Relationship::Enemies);
    assert_eq!(
        relation(&peer, peer_source, peer_target),
        Relationship::Enemies
    );
    assert_eq!(
        relation(&restored, restored_source, selected),
        Relationship::Allies
    );
}

fn relation(world: &GameLogic, source: ObjectId, target: ObjectId) -> Relationship {
    world.object_relationship(
        world.host_object(source).unwrap(),
        world.host_object(target).unwrap(),
    )
}

#[test]
fn scripted_team_overrides_are_immediate_directed_and_world_owned() {
    let root = tempfile::tempdir().unwrap();
    let map = map_file(root.path());
    let (mut owner, source, target) = world(&map);
    let (mut peer, peer_source, peer_target) = world(&map);
    assert_eq!((source, target), (peer_source, peer_target));
    assert_eq!(relation(&owner, source, target), Relationship::Enemies);
    assert_eq!(
        relation(&peer, peer_source, peer_target),
        Relationship::Enemies
    );

    execute(
        &mut owner,
        ScriptActionType::TeamSetOverrideRelationToTeam,
        Some("TargetRoster"),
        Some(2),
    );
    assert_eq!(relation(&owner, source, target), Relationship::Allies);
    assert_eq!(
        relation(&peer, peer_source, peer_target),
        Relationship::Enemies
    );

    let builder = generals_main::save_load::SnapshotBuilder::new();
    let snapshot = builder.create_world_snapshot(&owner).unwrap();
    let encoded = bincode_legacy::serialize(&snapshot).unwrap();
    let decoded: generals_main::save_load::snapshot::WorldSnapshot =
        bincode_legacy::deserialize(&encoded).unwrap();
    let (mut restored, restored_source, restored_target) = world(&map);
    builder
        .restore_from_snapshot(&decoded, &mut restored)
        .unwrap();
    assert_eq!(
        relation(&restored, restored_source, restored_target),
        Relationship::Allies,
        "the owning Team override survives an encoded world restore"
    );

    execute(
        &mut owner,
        ScriptActionType::TeamSetOverrideRelationToPlayer,
        Some("Other"),
        Some(1),
    );
    assert_eq!(
        relation(&owner, source, target),
        Relationship::Allies,
        "Team override has priority"
    );
    execute(
        &mut owner,
        ScriptActionType::TeamRemoveOverrideRelationToTeam,
        Some("TargetRoster"),
        None,
    );
    assert_eq!(relation(&owner, source, target), Relationship::Neutral);
    execute(
        &mut owner,
        ScriptActionType::TeamRemoveOverrideRelationToPlayer,
        Some("Other"),
        None,
    );
    assert_eq!(relation(&owner, source, target), Relationship::Enemies);

    execute(
        &mut peer,
        ScriptActionType::TeamSetOverrideRelationToTeam,
        Some("TargetRoster"),
        Some(2),
    );
    execute(
        &mut owner,
        ScriptActionType::TeamSetOverrideRelationToTeam,
        Some("MissingRoster"),
        Some(2),
    );
    assert_eq!(relation(&owner, source, target), Relationship::Enemies);
    execute(
        &mut peer,
        ScriptActionType::TeamRemoveAllOverrideRelations,
        None,
        None,
    );
    assert_eq!(
        relation(&peer, peer_source, peer_target),
        Relationship::Enemies
    );
    execute(
        &mut owner,
        ScriptActionType::TeamSetOverrideRelationToTeam,
        Some("SourceRoster"),
        Some(0),
    );
    assert_eq!(
        relation(&owner, source, source),
        Relationship::Enemies,
        "C++ consults an explicit self-Team override before the Player relation"
    );
    execute(
        &mut owner,
        ScriptActionType::TeamRemoveOverrideRelationToTeam,
        Some("SourceRoster"),
        None,
    );
}

#[test]
fn restored_same_named_teams_keep_distinct_relationship_owners() {
    use generals_main::save_load::snapshot::{
        apply_pending_player_team_chunks, stamp_player_team_chunks,
    };

    let root = tempfile::tempdir().unwrap();
    let map = map_file(root.path());
    let (mut world, source, target) = world(&map);
    execute(
        &mut world,
        ScriptActionType::TeamSetOverrideRelationToTeam,
        Some("TargetRoster"),
        Some(2),
    );
    let second = world
        .create_object_for_player("RelationUnit", 1, Vec3::new(15.0, 0.0, 30.0))
        .unwrap();

    // Restore two real Team instances from the receiving session's own
    // continuation data. They share a prototype name, but only the first
    // instance has the scripted override.
    let mut chunks = stamp_player_team_chunks(&world).unwrap();
    let teams = chunks.teams.as_mut().unwrap();
    let mut second_team = teams
        .teams
        .iter()
        .find(|team| team.team_name == "SourceRoster")
        .unwrap()
        .clone();
    for team in &mut teams.teams {
        if let Some(members) = team.members.as_mut() {
            members.retain(|&id| id != second.0);
        }
    }
    second_team.team_id = teams.unique_team_id;
    teams.unique_team_id += 1;
    second_team.members = Some(vec![second.0]);
    second_team.team_relations.clear();
    second_team.player_relations.clear();
    teams.teams.push(second_team);
    apply_pending_player_team_chunks(&mut world, &chunks).unwrap();

    assert_eq!(relation(&world, source, target), Relationship::Allies);
    assert_eq!(
        relation(&world, second, target),
        Relationship::Enemies,
        "a same-name sibling must not inherit another Team's override"
    );
}
