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
            "Object RelationUnit\n KindOf = SELECTABLE INFANTRY\n Body = ActiveBody BodyTag\n MaxHealth = 100\n InitialHealth = 100\n End\n Behavior = CostModifierUpgrade MergeCost\n TriggeredBy = Upgrade_Veterancy_VETERAN\n EffectKindOf = VEHICLE AIRCRAFT\n Percentage = -10%\n End\nEnd\nObject RelationDualPurchase\n KindOf = VEHICLE AIRCRAFT\n BuildCost = 1000\n Body = ActiveBody BodyTag\n MaxHealth = 100\n InitialHealth = 100\n End\nEnd\nObject ChinaTankECM\n KindOf = VEHICLE\n Body = ActiveBody BodyTag\n MaxHealth = 300\n InitialHealth = 300\n End\nEnd\nObject AmericaTankCrusader\n KindOf = VEHICLE\n Body = ActiveBody BodyTag\n MaxHealth = 400\n InitialHealth = 400\n End\nEnd\nObject RelationCapturer\n KindOf = SELECTABLE INFANTRY\n Body = ActiveBody BodyTag\n MaxHealth = 100\n InitialHealth = 100\n End\n Behavior = AIUpdateInterface Brain\n End\nEnd\nObject HealCrate\n KindOf = CRATE\nEnd\n".into(),
        ), (
            "Beacon.ini".into(),
            "Object RelationBeacon\n KindOf = SELECTABLE\n Body = ActiveBody BeaconBody\n MaxHealth = 10\n InitialHealth = 10\n End\nEnd\n".into(),
        )])
        .unwrap();
    let mut player_templates = PlayerTemplateStore::new_owned();
    player_templates
        .apply_source(
            "PlayerTemplate FactionCivilian\n Side = Civilian\nEnd\n\
             PlayerTemplate FactionAmerica\n Side = America\n BeaconName = RelationBeacon\nEnd\n\
             PlayerTemplate FactionChina\n Side = China\nEnd\n",
            INILoadType::Overwrite,
        )
        .unwrap();
    world
        .admit_player_template_definitions(player_templates)
        .unwrap();
    world.start_new_game(GameMode::SinglePlayer);
    let mut map_phases = Vec::new();
    let loaded = world.load_map_with_progress(map.to_str().unwrap(), |progress, phase| {
        map_phases.push((progress, phase.to_owned()));
    });
    assert!(loaded, "map load stopped after milestones: {map_phases:?}");
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
fn player_asset_transfer_is_immediate_and_owned_by_the_receiving_world() {
    let root = tempfile::tempdir().unwrap();
    let map = map_file(root.path());
    let (mut owner, source, _) = world(&map);
    let (mut peer, peer_source, _) = world(&map);
    assert_eq!(source, peer_source);
    let beacon = owner
        .create_object_for_player("RelationBeacon", 1, Vec3::new(15.0, 0.0, 30.0))
        .unwrap();
    let peer_beacon = peer
        .create_object_for_player("RelationBeacon", 1, Vec3::new(15.0, 0.0, 30.0))
        .unwrap();
    assert_eq!(beacon, peer_beacon);
    for game in [&mut owner, &mut peer] {
        game.get_player_mut(1).unwrap().resources.supplies = 700;
        game.get_player_mut(2).unwrap().resources.supplies = 80;
    }

    let mut action = ScriptAction::new(ScriptActionType::PlayerTransferOwnershipPlayer);
    for player in ["Owner", "Other"] {
        action
            .add_parameter(Parameter::with_string(ParameterType::Side, player.into()))
            .unwrap();
    }
    run_action(&mut owner, action);
    assert_eq!(owner.host_object(source).unwrap().owner_player_id, Some(2));
    assert_eq!(owner.host_object(beacon).unwrap().owner_player_id, Some(1));
    assert_eq!(owner.get_player(1).unwrap().effective_supplies(), 0);
    assert_eq!(owner.get_player(2).unwrap().effective_supplies(), 780);
    assert_eq!(
        peer.host_object(peer_source).unwrap().owner_player_id,
        Some(1)
    );
    assert_eq!(peer.get_player(1).unwrap().effective_supplies(), 700);
    assert_eq!(peer.get_player(2).unwrap().effective_supplies(), 80);
    peer.update();
    assert_eq!(
        peer.host_object(peer_source).unwrap().owner_player_id,
        Some(1)
    );
    assert_eq!(peer.get_player(1).unwrap().effective_supplies(), 700);

    let (mut missing_default, missing_source, _) = world(&map);
    let mut chunks =
        generals_main::save_load::snapshot::stamp_player_team_chunks(&missing_default).unwrap();
    chunks
        .players
        .as_mut()
        .unwrap()
        .players
        .iter_mut()
        .find(|player| player.player_id == 2)
        .unwrap()
        .host_default_team_id = None;
    generals_main::save_load::snapshot::apply_pending_player_team_chunks(
        &mut missing_default,
        &chunks,
    )
    .unwrap();
    missing_default
        .get_player_mut(1)
        .unwrap()
        .resources
        .supplies = 700;
    missing_default
        .get_player_mut(2)
        .unwrap()
        .resources
        .supplies = 80;
    let mut action = ScriptAction::new(ScriptActionType::PlayerTransferOwnershipPlayer);
    for player in ["Owner", "Other"] {
        action
            .add_parameter(Parameter::with_string(ParameterType::Side, player.into()))
            .unwrap();
    }
    run_action(&mut missing_default, action);
    assert_eq!(
        missing_default
            .host_object(missing_source)
            .unwrap()
            .owner_player_id,
        Some(1)
    );
    assert_eq!(
        missing_default.get_player(1).unwrap().effective_supplies(),
        700
    );
    assert_eq!(
        missing_default.get_player(2).unwrap().effective_supplies(),
        80
    );
}

#[test]
fn team_transfer_refreshes_the_receiving_players_upgrade_modules() {
    let root = tempfile::tempdir().unwrap();
    let map = map_file(root.path());
    let (mut owner, source, _) = world(&map);
    let (mut peer, peer_source, _) = world(&map);
    assert_eq!(source, peer_source);
    for game in [&mut owner, &mut peer] {
        game.get_player_mut(2)
            .unwrap()
            .add_completed_upgrade("Upgrade_Veterancy_VETERAN");
    }

    let mut action = ScriptAction::new(ScriptActionType::TeamTransferToPlayer);
    action
        .add_parameter(Parameter::with_string(
            ParameterType::Team,
            "SourceRoster".into(),
        ))
        .unwrap();
    action
        .add_parameter(Parameter::with_string(ParameterType::Side, "Other".into()))
        .unwrap();
    run_action(&mut owner, action);
    assert_eq!(owner.host_object(source).unwrap().owner_player_id, Some(2));
    assert_eq!(
        owner.modified_build_cost_supplies(2, "RelationDualPurchase", 1000),
        900
    );
    assert_eq!(
        peer.host_object(peer_source).unwrap().owner_player_id,
        Some(1)
    );
    assert_eq!(
        peer.modified_build_cost_supplies(2, "RelationDualPurchase", 1000),
        1000
    );
}

#[test]
fn named_transfer_refreshes_only_the_receiving_worlds_upgrade_modules() {
    let root = tempfile::tempdir().unwrap();
    let map = map_file(root.path());
    let (mut owner, source, _) = world(&map);
    let (mut peer, peer_source, _) = world(&map);
    assert_eq!(source, peer_source);
    for (game, object) in [(&mut owner, source), (&mut peer, peer_source)] {
        game.host_object_mut(object).unwrap().name = "TransferMember".into();
        game.get_player_mut(2)
            .unwrap()
            .add_completed_upgrade("Upgrade_Veterancy_VETERAN");
    }

    let mut action = ScriptAction::new(ScriptActionType::NamedTransferOwnershipPlayer);
    action
        .add_parameter(Parameter::with_string(
            ParameterType::Unit,
            "TransferMember".into(),
        ))
        .unwrap();
    action
        .add_parameter(Parameter::with_string(ParameterType::Side, "Other".into()))
        .unwrap();
    run_action(&mut owner, action);
    assert_eq!(owner.host_object(source).unwrap().owner_player_id, Some(2));
    assert_eq!(
        owner.modified_build_cost_supplies(2, "RelationDualPurchase", 1000),
        900
    );
    assert_eq!(
        peer.host_object(peer_source).unwrap().owner_player_id,
        Some(1)
    );
    assert_eq!(
        peer.modified_build_cost_supplies(2, "RelationDualPurchase", 1000),
        1000
    );
}

fn same_named_team_member(
    world: &mut GameLogic,
    player: u32,
    team_name: &str,
    template: &str,
    position: Vec3,
) -> (ObjectId, u32) {
    use generals_main::save_load::snapshot::{
        apply_pending_player_team_chunks, stamp_player_team_chunks,
    };

    let member = world
        .create_object_for_player(template, player, position)
        .unwrap();
    let mut chunks = stamp_player_team_chunks(world).unwrap();
    let teams = chunks.teams.as_mut().unwrap();
    let mut sibling = teams
        .teams
        .iter()
        .find(|team| team.team_name == team_name)
        .unwrap()
        .clone();
    for team in &mut teams.teams {
        if let Some(members) = &mut team.members {
            members.retain(|&id| id != member.0);
        }
    }
    let team_id = teams.unique_team_id;
    teams.unique_team_id += 1;
    sibling.team_id = team_id;
    sibling.members = Some(vec![member.0]);
    sibling.team_relations.clear();
    sibling.player_relations.clear();
    teams.teams.push(sibling);
    apply_pending_player_team_chunks(world, &chunks).unwrap();
    (member, team_id)
}

fn duplicate_default_team(world: &mut GameLogic, player: u32) -> (u32, u32) {
    use generals_main::save_load::snapshot::{
        apply_pending_player_team_chunks, stamp_player_team_chunks,
    };

    let mut chunks = stamp_player_team_chunks(world).unwrap();
    let player = chunks
        .players
        .as_mut()
        .unwrap()
        .players
        .iter_mut()
        .find(|saved| saved.player_id == player)
        .unwrap();
    let original_id = player.host_default_team_id.unwrap();
    let teams = chunks.teams.as_mut().unwrap();
    let mut sibling = teams
        .teams
        .iter()
        .find(|team| team.team_id == original_id)
        .unwrap()
        .clone();
    let sibling_id = teams.unique_team_id;
    teams.unique_team_id += 1;
    sibling.team_id = sibling_id;
    sibling.members = Some(Vec::new());
    sibling.team_relations.clear();
    sibling.player_relations.clear();
    teams.teams.push(sibling);
    player.host_default_team_id = Some(sibling_id);
    apply_pending_player_team_chunks(world, &chunks).unwrap();
    (original_id, sibling_id)
}

#[test]
fn strategic_ai_uses_each_players_exact_default_team() {
    use generals_main::ai::{AIDifficulty, AIPlayer};
    use generals_main::game_logic::Team;

    let root = tempfile::tempdir().unwrap();
    let map = map_file(root.path());
    let (mut owner, _, _) = world(&map);
    let (mut peer, _, _) = world(&map);
    let (original, selected) = duplicate_default_team(&mut owner, 2);
    assert_eq!((original, selected), duplicate_default_team(&mut peer, 2));
    for (game, selected_relation, other_relation) in [
        (&mut owner, Relationship::Enemies, Relationship::Allies),
        (&mut peer, Relationship::Allies, Relationship::Enemies),
    ] {
        let player = game.get_player_mut(1).unwrap();
        player.set_team_relationship_override(original, other_relation);
        player.set_team_relationship_override(selected, selected_relation);
    }
    let make_ai = || {
        let mut ai = AIPlayer::new(1, Team::USA, AIDifficulty::Medium);
        ai.enemy_check_time = -5.0;
        ai.next_building_time = f32::MAX;
        ai.next_team_time = f32::MAX;
        ai.next_team_queue_time = f32::MAX;
        ai
    };
    let mut first_ai = make_ai();
    let mut peer_ai = make_ai();
    peer_ai.update(&mut peer, 0.0);
    first_ai.update(&mut owner, 0.0);
    assert_eq!(first_ai.enemy_player_id, Some(2));
    assert_eq!(peer_ai.enemy_player_id, None);

    // A missing default Team is not permission to pick a same-named instance.
    // Player::getRelationship(nullptr) is NEUTRAL in the original.
    let (mut no_default, _, _) = world(&map);
    let mut chunks =
        generals_main::save_load::snapshot::stamp_player_team_chunks(&no_default).unwrap();
    chunks
        .players
        .as_mut()
        .unwrap()
        .players
        .iter_mut()
        .find(|saved| saved.player_id == 2)
        .unwrap()
        .host_default_team_id = None;
    generals_main::save_load::snapshot::apply_pending_player_team_chunks(&mut no_default, &chunks)
        .unwrap();
    let mut no_default_ai = make_ai();
    no_default_ai.update(&mut no_default, 0.0);
    assert_eq!(no_default_ai.enemy_player_id, None);
}

#[test]
fn guard_acquires_by_exact_team_identity_in_each_world() {
    use generals_main::game_logic::Weapon;

    fn prepare(
        world: &mut GameLogic,
        guard: ObjectId,
        target: ObjectId,
        first_is_enemy: bool,
    ) -> ObjectId {
        {
            let object = world.host_object_mut(guard).unwrap();
            object.team_instance_name = "SourceRoster".into();
            object.vision_range = 120.0;
            object.weapon = Some(Weapon {
                range: 120.0,
                damage: 1.0,
                ..Default::default()
            });
        }
        world.activate_leftover_team_for_host_object(guard);
        let (sibling, sibling_team) = same_named_team_member(
            world,
            2,
            "TargetRoster",
            "RelationUnit",
            Vec3::new(35.0, 0.0, 30.0),
        );
        let original_team = team_id_for_member(world, target);
        let player = world.get_player_mut(1).unwrap();
        player.set_team_relationship_override(
            original_team,
            if first_is_enemy {
                Relationship::Enemies
            } else {
                Relationship::Allies
            },
        );
        player.set_team_relationship_override(
            sibling_team,
            if first_is_enemy {
                Relationship::Allies
            } else {
                Relationship::Enemies
            },
        );
        assert!(world.unit_command_guard_position(guard, Vec3::new(10.0, 0.0, 30.0)));
        sibling
    }

    let root = tempfile::tempdir().unwrap();
    let map = map_file(root.path());
    let (mut owner, guard, target) = world(&map);
    let (mut peer, peer_guard, peer_target) = world(&map);
    let sibling = prepare(&mut owner, guard, target, false);
    let peer_sibling = prepare(&mut peer, peer_guard, peer_target, true);
    assert_eq!(
        (guard, target, sibling),
        (peer_guard, peer_target, peer_sibling)
    );

    let mut owner_found = false;
    let mut peer_found = false;
    for _ in 0..120 {
        peer.update();
        owner.update();
        owner_found |= owner
            .host_object(guard)
            .is_some_and(|object| object.target == Some(sibling));
        peer_found |= peer
            .host_object(peer_guard)
            .is_some_and(|object| object.target == Some(peer_target));
        if owner_found && peer_found {
            break;
        }
    }
    assert!(
        owner_found,
        "owner guard must acquire the enemy sibling Team"
    );
    assert!(peer_found, "peer guard must acquire its own enemy Team");
}

fn team_id_for_member(world: &GameLogic, member: ObjectId) -> u32 {
    use generals_main::save_load::snapshot::stamp_player_team_chunks;

    stamp_player_team_chunks(world)
        .unwrap()
        .teams
        .unwrap()
        .teams
        .into_iter()
        .find(|team| {
            team.members
                .as_ref()
                .is_some_and(|members| members.contains(&member.0))
        })
        .unwrap()
        .team_id
}

fn contextual_action_engine(action: ScriptAction, condition_team: &str) -> ScriptEngine {
    let mut branch = OrCondition::new();
    branch.set_first_and_condition(Some(Box::new(Condition::new(ConditionType::ConditionTrue))));
    let mut script = Script::new();
    script.script_name = format!("ContextualRelationship_{condition_team}");
    script.condition_team_name = condition_team.into();
    script.is_one_shot = true;
    script.condition = Some(Box::new(branch));
    script.action = Some(Box::new(action));
    let mut scripts = ScriptList::new();
    scripts.append_script(Box::new(script));
    let engine = ScriptEngine::new().unwrap();
    engine
        .set_script_list_for_player(0, Some(Box::new(scripts)))
        .unwrap();
    engine
}

fn run_contextual_action(world: &mut GameLogic, engine: &ScriptEngine) {
    world.update_local_script_engine_with_owner(engine).unwrap();
}

fn contextual_team_override_action(target: &str, relation: i32) -> ScriptAction {
    let mut action = ScriptAction::new(ScriptActionType::TeamSetOverrideRelationToTeam);
    action
        .add_parameter(Parameter::with_string(
            ParameterType::Team,
            gamelogic::scripting::core::THIS_TEAM.into(),
        ))
        .unwrap();
    action
        .add_parameter(Parameter::with_string(ParameterType::Team, target.into()))
        .unwrap();
    action
        .add_parameter(Parameter::with_int(ParameterType::Int, relation))
        .unwrap();
    action
}

fn contextual_player_override_action(target: &str, relation: i32) -> ScriptAction {
    let mut action = ScriptAction::new(ScriptActionType::PlayerSetOverrideRelationToTeam);
    action
        .add_parameter(Parameter::with_string(ParameterType::Side, "Owner".into()))
        .unwrap();
    action
        .add_parameter(Parameter::with_string(ParameterType::Team, target.into()))
        .unwrap();
    action
        .add_parameter(Parameter::with_int(ParameterType::Int, relation))
        .unwrap();
    action
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
fn contextual_relationship_actions_use_live_team_ids_and_reset_per_world() {
    let root = tempfile::tempdir().unwrap();
    let map = map_file(root.path());
    let (mut owner, source, target) = world(&map);
    let (mut peer, peer_source, peer_target) = world(&map);
    assert_eq!((source, target), (peer_source, peer_target));

    // Same-name Team instances have distinct live IDs. The script walk below
    // visits each instance and the production driver supplies its condition ID.
    let (source_sibling, source_sibling_team) = same_named_team_member(
        &mut owner,
        1,
        "SourceRoster",
        "RelationUnit",
        Vec3::new(12.0, 0.0, 30.0),
    );
    let (peer_source_sibling, peer_source_sibling_team) = same_named_team_member(
        &mut peer,
        1,
        "SourceRoster",
        "RelationUnit",
        Vec3::new(12.0, 0.0, 30.0),
    );
    assert_eq!(source_sibling, peer_source_sibling);
    assert_eq!(source_sibling_team, peer_source_sibling_team);

    let source_team = team_id_for_member(&owner, source);
    let target_team = team_id_for_member(&owner, target);
    let peer_source_team = team_id_for_member(&peer, peer_source);
    let peer_target_team = team_id_for_member(&peer, peer_target);

    // TEAM_SET_OVERRIDE_RELATION_TO_TEAM uses <This Team> as its source. A
    // single actual ScriptEngine update iterates both same-name source Teams;
    // only the driving world's two exact Team records may change.
    let mut contextual_team_action = contextual_team_override_action("TargetRoster", 2);
    let mut visited = ScriptAction::new(ScriptActionType::IncrementCounter);
    visited
        .add_parameter(Parameter::with_int(ParameterType::Int, 1))
        .unwrap();
    visited
        .add_parameter(Parameter::with_string(
            ParameterType::Counter,
            "Visited".into(),
        ))
        .unwrap();
    contextual_team_action.next_action = Some(Box::new(visited));
    let owner_team_script = contextual_action_engine(contextual_team_action, "SourceRoster");
    run_contextual_action(&mut owner, &owner_team_script);
    assert_eq!(owner_team_script.get_counter("Visited").unwrap().value, 2);
    assert_eq!(relation(&owner, source, target), Relationship::Allies);
    assert_eq!(
        relation(&owner, source_sibling, target),
        Relationship::Allies,
        "the second contextual source instance also receives its own override"
    );
    assert_eq!(
        relation(&peer, peer_source, peer_target),
        Relationship::Enemies
    );
    assert_eq!(
        relation(&peer, peer_source_sibling, peer_target),
        Relationship::Enemies
    );

    // Use the same production script API on the peer in the same OS thread,
    // with a different value, and prove the first world's exact Team entries
    // are unchanged.
    let peer_team_script = contextual_action_engine(
        contextual_team_override_action("TargetRoster", 1),
        "SourceRoster",
    );
    run_contextual_action(&mut peer, &peer_team_script);
    assert_eq!(relation(&owner, source, target), Relationship::Allies);
    assert_eq!(
        relation(&peer, peer_source, peer_target),
        Relationship::Neutral
    );

    // Add same-name target siblings after the Team-to-Team scripts above so
    // the authored target name has one unambiguous receiver. Player overrides
    // below exercise exact condition-Team identity with the duplicate names.
    let (target_sibling, target_sibling_team) = same_named_team_member(
        &mut owner,
        2,
        "TargetRoster",
        "RelationUnit",
        Vec3::new(32.0, 0.0, 30.0),
    );
    let (peer_target_sibling, peer_target_sibling_team) = same_named_team_member(
        &mut peer,
        2,
        "TargetRoster",
        "RelationUnit",
        Vec3::new(32.0, 0.0, 30.0),
    );
    assert_eq!(target_sibling, peer_target_sibling);
    assert_eq!(target_sibling_team, peer_target_sibling_team);

    // PLAYER_SET_OVERRIDE_RELATION_TO_TEAM resolves <This Team> from each
    // target context and stores by exact target Team ID, including siblings.
    let owner_player_script = contextual_action_engine(
        contextual_player_override_action(gamelogic::scripting::core::THIS_TEAM, 2),
        "TargetRoster",
    );
    run_contextual_action(&mut owner, &owner_player_script);
    let owner_player = owner.get_player(1).unwrap();
    assert_eq!(
        owner_player.team_relations.get(&target_team),
        Some(&Relationship::Allies)
    );
    assert_eq!(
        owner_player.team_relations.get(&target_sibling_team),
        Some(&Relationship::Allies),
        "the condition Team identity must not collapse to the first same-name Team"
    );
    assert!(peer.get_player(1).unwrap().team_relations.is_empty());

    let peer_player_script = contextual_action_engine(
        contextual_player_override_action(gamelogic::scripting::core::THIS_TEAM, 1),
        "TargetRoster",
    );
    run_contextual_action(&mut peer, &peer_player_script);
    assert_eq!(
        owner
            .get_player(1)
            .unwrap()
            .team_relations
            .get(&target_team),
        Some(&Relationship::Allies)
    );
    assert_eq!(
        peer.get_player(1)
            .unwrap()
            .team_relations
            .get(&peer_target_team),
        Some(&Relationship::Neutral)
    );

    // New-game reset clears this owner's Player and Team relationship state,
    // while the peer's same-ID session keeps its authored result.
    owner.reset();
    owner.start_new_game(GameMode::SinglePlayer);
    assert!(owner.load_map(map.to_str().unwrap()));
    assert!(owner.get_player(1).unwrap().team_relations.is_empty());
    let reset_source = owner
        .create_object_for_player("RelationUnit", 1, Vec3::new(10.0, 0.0, 30.0))
        .unwrap();
    let reset_target = owner
        .create_object_for_player("RelationUnit", 2, Vec3::new(30.0, 0.0, 30.0))
        .unwrap();
    for (id, team) in [
        (reset_source, "SourceRoster"),
        (reset_target, "TargetRoster"),
    ] {
        owner.host_object_mut(id).unwrap().team_instance_name = team.into();
        owner.activate_leftover_team_for_host_object(id);
    }
    assert_eq!(
        relation(&owner, reset_source, reset_target),
        Relationship::Neutral
    );
    assert_eq!(
        peer.get_player(1)
            .unwrap()
            .team_relations
            .get(&peer_target_team),
        Some(&Relationship::Neutral),
        "resetting the first world must not erase the peer's relation"
    );
    assert_eq!(
        relation(&peer, peer_source, peer_target),
        Relationship::Neutral
    );

    // Keep the original IDs visible in this regression: both worlds began
    // with matching object and Team IDs before the reset/re-admission.
    assert_eq!(source_team, peer_source_team);
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

#[test]
fn ecm_field_uses_exact_same_named_target_team_in_each_world() {
    let root = tempfile::tempdir().unwrap();
    let map = map_file(root.path());
    let (mut first_world, _, _) = world(&map);
    let (mut second_world, _, _) = world(&map);

    fn scenario(world: &mut GameLogic) -> (ObjectId, ObjectId, ObjectId, ObjectId) {
        let jammer = world
            .create_object_for_player("ChinaTankECM", 2, Vec3::new(0.0, 0.0, 30.0))
            .unwrap();
        world.host_object_mut(jammer).unwrap().team_instance_name = "TargetRoster".into();
        world.activate_leftover_team_for_host_object(jammer);
        let (far_jammer, _) = same_named_team_member(
            world,
            2,
            "TargetRoster",
            "ChinaTankECM",
            Vec3::new(500.0, 0.0, 30.0),
        );

        let allied = world
            .create_object_for_player("AmericaTankCrusader", 1, Vec3::new(40.0, 0.0, 30.0))
            .unwrap();
        world.host_object_mut(allied).unwrap().team_instance_name = "SourceRoster".into();
        world.activate_leftover_team_for_host_object(allied);

        let enemy = world
            .create_object_for_player("AmericaTankCrusader", 1, Vec3::new(540.0, 0.0, 30.0))
            .unwrap();
        world.host_object_mut(enemy).unwrap().team_instance_name = "SourceRoster".into();
        world.activate_leftover_team_for_host_object(enemy);
        (jammer, far_jammer, allied, enemy)
    }

    let (first_jammer, first_far_jammer, first_ally, first_enemy) = scenario(&mut first_world);
    let (second_jammer, second_far_jammer, second_ally, second_enemy) = scenario(&mut second_world);
    assert_eq!(
        (first_jammer, first_far_jammer, first_ally, first_enemy),
        (second_jammer, second_far_jammer, second_ally, second_enemy),
        "the worlds deliberately reuse the same numeric IDs"
    );

    let first_near_team = team_id_for_member(&first_world, first_jammer);
    let first_far_team = team_id_for_member(&first_world, first_far_jammer);
    first_world
        .get_player_mut(1)
        .unwrap()
        .set_team_relationship_override(first_near_team, Relationship::Allies);
    first_world
        .get_player_mut(1)
        .unwrap()
        .set_team_relationship_override(first_far_team, Relationship::Enemies);

    let second_near_team = team_id_for_member(&second_world, second_jammer);
    let second_far_team = team_id_for_member(&second_world, second_far_jammer);
    second_world
        .get_player_mut(1)
        .unwrap()
        .set_team_relationship_override(second_near_team, Relationship::Enemies);
    second_world
        .get_player_mut(1)
        .unwrap()
        .set_team_relationship_override(second_far_team, Relationship::Allies);

    assert_eq!(
        relation(&first_world, first_ally, first_jammer),
        Relationship::Allies
    );
    assert_eq!(
        relation(&first_world, first_enemy, first_far_jammer),
        Relationship::Enemies
    );
    assert_eq!(
        relation(&second_world, second_ally, second_jammer),
        Relationship::Enemies
    );
    assert_eq!(
        relation(&second_world, second_enemy, second_far_jammer),
        Relationship::Allies
    );

    first_world.update_ecm_jam_field();
    second_world.update_ecm_jam_field();
    assert_eq!(
        first_world.host_object(first_ally).unwrap().subdual_damage,
        0.0
    );
    assert!(first_world.host_object(first_enemy).unwrap().subdual_damage > 0.0);
    assert!(
        second_world
            .host_object(second_ally)
            .unwrap()
            .subdual_damage
            > 0.0
    );
    assert_eq!(
        second_world
            .host_object(second_enemy)
            .unwrap()
            .subdual_damage,
        0.0
    );
}

#[test]
fn capture_nearest_uses_each_candidate_exact_team_identity() {
    fn prepare(
        world: &mut GameLogic,
        original: ObjectId,
        original_is_enemy: bool,
    ) -> (ObjectId, ObjectId) {
        let (sibling, sibling_team) = same_named_team_member(
            world,
            2,
            "TargetRoster",
            "RelationUnit",
            Vec3::new(35.0, 0.0, 30.0),
        );
        let original_team = team_id_for_member(world, original);
        for id in [original, sibling] {
            world.host_object_mut(id).unwrap().status.disabled_unmanned = true;
        }
        let capturer = world
            .create_object_for_player("RelationCapturer", 1, Vec3::new(10.0, 0.0, 30.0))
            .unwrap();
        world.host_object_mut(capturer).unwrap().team_instance_name = "SourceRoster".into();
        world.activate_leftover_team_for_host_object(capturer);
        let player = world.get_player_mut(1).unwrap();
        player.set_team_relationship_override(
            original_team,
            if original_is_enemy {
                Relationship::Enemies
            } else {
                Relationship::Allies
            },
        );
        player.set_team_relationship_override(
            sibling_team,
            if original_is_enemy {
                Relationship::Allies
            } else {
                Relationship::Enemies
            },
        );
        (capturer, sibling)
    }

    let root = tempfile::tempdir().unwrap();
    let map = map_file(root.path());
    let (mut owner, _, original) = world(&map);
    let (mut peer, _, peer_original) = world(&map);
    let (capturer, sibling) = prepare(&mut owner, original, false);
    let (peer_capturer, peer_sibling) = prepare(&mut peer, peer_original, true);
    assert_eq!(
        (capturer, original, sibling),
        (peer_capturer, peer_original, peer_sibling)
    );

    execute(
        &mut peer,
        ScriptActionType::TeamCaptureNearestUnownedFactionUnit,
        None,
        None,
    );
    execute(
        &mut owner,
        ScriptActionType::TeamCaptureNearestUnownedFactionUnit,
        None,
        None,
    );
    assert_eq!(owner.host_object(capturer).unwrap().target, Some(sibling));
    assert_eq!(
        peer.host_object(peer_capturer).unwrap().target,
        Some(peer_original)
    );
}

#[test]
fn crate_death_uses_the_killers_exact_relationship_to_the_victims_team() {
    use generals_main::game_logic::Team;

    fn prepare(world: &mut GameLogic, original: ObjectId, original_is_enemy: bool) -> ObjectId {
        let (sibling, sibling_team) = same_named_team_member(
            world,
            2,
            "TargetRoster",
            "RelationUnit",
            Vec3::new(35.0, 0.0, 30.0),
        );
        let original_team = team_id_for_member(world, original);
        let player = world.get_player_mut(1).unwrap();
        player.set_team_relationship_override(
            original_team,
            if original_is_enemy {
                Relationship::Enemies
            } else {
                Relationship::Allies
            },
        );
        player.set_team_relationship_override(
            sibling_team,
            if original_is_enemy {
                Relationship::Allies
            } else {
                Relationship::Enemies
            },
        );
        sibling
    }

    let root = tempfile::tempdir().unwrap();
    let map = map_file(root.path());
    let (mut owner, killer, original) = world(&map);
    let (mut peer, peer_killer, peer_original) = world(&map);
    let sibling = prepare(&mut owner, original, false);
    let peer_sibling = prepare(&mut peer, peer_original, true);
    assert_eq!(
        (killer, original, sibling),
        (peer_killer, peer_original, peer_sibling)
    );
    let crate_data = vec!["HealCrateData".to_owned()];
    let spawn = |world: &mut GameLogic, victim, killer| {
        let pos = world.host_object(victim).unwrap().get_position();
        world.try_create_crates_on_die(victim, pos, Team::China, &crate_data, Some(killer))
    };
    assert_eq!(spawn(&mut owner, original, killer), 0);
    assert_eq!(spawn(&mut peer, peer_original, peer_killer), 1);
    assert_eq!(spawn(&mut owner, sibling, killer), 1);
    assert_eq!(spawn(&mut peer, peer_sibling, peer_killer), 0);

    peer.host_object_mut(peer_original)
        .unwrap()
        .begin_undetected_defection(0, 100, false);
    assert_eq!(
        spawn(&mut peer, peer_original, peer_killer),
        0,
        "the killer treats an undetected defector as allied"
    );
    owner
        .host_object_mut(killer)
        .unwrap()
        .begin_undetected_defection(0, 100, false);
    assert_eq!(
        spawn(&mut owner, original, killer),
        1,
        "an undetected defector observes a neutral relationship"
    );
}

#[test]
fn leaflet_impact_uses_victims_exact_directed_team_relationship() {
    use generals_main::command_system::SpecialPowerType;
    use generals_main::save_load::snapshot::{
        apply_pending_player_team_chunks, stamp_player_team_chunks,
    };

    fn prepare(
        world: &mut GameLogic,
        source: ObjectId,
        original: ObjectId,
        original_is_enemy: bool,
    ) -> ObjectId {
        let (sibling, sibling_team) = same_named_team_member(
            world,
            2,
            "TargetRoster",
            "RelationUnit",
            Vec3::new(35.0, 0.0, 30.0),
        );
        let original_team = team_id_for_member(world, original);
        let source_team = team_id_for_member(world, source);
        let mut chunks = stamp_player_team_chunks(world).unwrap();
        for (team_id, is_enemy) in [
            (original_team, original_is_enemy),
            (sibling_team, !original_is_enemy),
        ] {
            let team = chunks
                .teams
                .as_mut()
                .unwrap()
                .teams
                .iter_mut()
                .find(|team| team.team_id == team_id)
                .unwrap();
            team.team_relations.push(Default::default());
            let relationship = team.team_relations.last_mut().unwrap();
            relationship.team_id = source_team;
            relationship.team_name = "SourceRoster".into();
            relationship.relationship = if is_enemy { 0 } else { 2 };
        }
        apply_pending_player_team_chunks(world, &chunks).unwrap();
        world
            .queue_leaflet_drop(
                &SpecialPowerType::LeafletDrop,
                source,
                Vec3::new(30.0, 0.0, 30.0),
            )
            .unwrap();
        sibling
    }

    let root = tempfile::tempdir().unwrap();
    let map = map_file(root.path());
    let (mut owner, source, original) = world(&map);
    let (mut peer, peer_source, peer_original) = world(&map);
    let sibling = prepare(&mut owner, source, original, false);
    let peer_sibling = prepare(&mut peer, peer_source, peer_original, true);
    assert_eq!(
        (source, original, sibling),
        (peer_source, peer_original, peer_sibling)
    );
    for _ in 0..80 {
        peer.update();
        owner.update();
    }
    assert!(!owner.host_object(original).unwrap().is_emp_disabled());
    assert!(owner.host_object(sibling).unwrap().is_emp_disabled());
    assert!(peer.host_object(peer_original).unwrap().is_emp_disabled());
    assert!(!peer.host_object(peer_sibling).unwrap().is_emp_disabled());
}
