use super::dispatch_test_command;

#[test]
fn switch_weapons_locks_button_slot_not_cycle() {
    // C++ GameLogicDispatch.cpp:583-590 MSG_SWITCH_WEAPONS locks the
    // ControlBar button slot permanently instead of cycling 0→1→2.
    use super::CommandExecutor;
    use crate::command_system::{CommandResult, CommandType};
    use crate::game_logic::{GameLogic, Player, Team, ThingTemplate, Weapon};
    use glam::Vec3;

    let mut logic = GameLogic::new();
    logic
        .get_players_mut()
        .insert(0, Player::new(0, Team::USA, "P0", true));
    logic
        .templates
        .insert("HumveeSlot".to_string(), ThingTemplate::new("HumveeSlot"));
    let id = logic
        .create_object("HumveeSlot", Team::USA, Vec3::ZERO)
        .expect("humvee");
    {
        let obj = logic.host_object_mut(id).expect("obj");
        obj.owner_player_id = Some(0);
        obj.weapon = Some(Weapon {
            damage: 1.0,
            range: 100.0,
            ..Weapon::default()
        });
        obj.secondary_weapon = Some(Weapon {
            damage: 2.0,
            range: 100.0,
            ..Weapon::default()
        });
        obj.tertiary_weapon = Some(Weapon {
            damage: 3.0,
            range: 100.0,
            ..Weapon::default()
        });
        obj.active_weapon_slot = 0;
    }

    let result = {
        let mut exec = CommandExecutor::new(&mut logic, 0);
        exec.execute_command(dispatch_test_command(
            CommandType::SwitchWeapons { slot: 1 },
            0,
            vec![id],
        ))
        .expect("execute")
    };
    assert_eq!(result, CommandResult::Success);
    let obj = logic.host_object(id).expect("obj");
    assert_eq!(obj.weapon_lock_slot, 1, "must lock the button slot");
    assert_eq!(obj.active_weapon_slot, 1);
    assert_ne!(obj.weapon_lock_slot, 2, "must not cycle to the next slot");
}

#[test]
fn enable_retaliation_mode_sets_logical_flag() {
    // C++ GameLogicDispatch.cpp:603-614 MSG_ENABLE_RETALIATION_MODE.
    use super::CommandExecutor;
    use crate::command_system::{CommandResult, CommandType};
    use crate::game_logic::{GameLogic, Player, Team};

    let mut logic = GameLogic::new();
    logic
        .get_players_mut()
        .insert(0, Player::new(0, Team::USA, "P0", true));
    assert!(
        !logic
            .get_player(0)
            .unwrap()
            .logical_retaliation_mode_enabled
    );

    let result = {
        let mut exec = CommandExecutor::new(&mut logic, 0);
        exec.execute_command(dispatch_test_command(
            CommandType::EnableRetaliationMode {
                player_index: 0,
                enabled: true,
            },
            0,
            vec![],
        ))
        .expect("execute")
    };
    assert_eq!(result, CommandResult::Success);
    assert!(
        logic
            .get_player(0)
            .unwrap()
            .logical_retaliation_mode_enabled
    );
}

#[test]
fn self_destruct_transfers_to_living_ally_then_kills() {
    // C++ GameLogicDispatch.cpp:1762-1797 MSG_SELF_DESTRUCT arg0=true.
    use super::CommandExecutor;
    use crate::command_system::{CommandResult, CommandType};
    use crate::game_logic::{GameLogic, KindOf, Player, Team, ThingTemplate};
    use glam::Vec3;

    let mut logic = GameLogic::new();
    let mut p0 = Player::new(0, Team::USA, "P0", true);
    p0.alliance_team = 1;
    logic.get_players_mut().insert(0, p0);
    let mut p1 = Player::new(1, Team::USA, "P1", false);
    p1.alliance_team = 1;
    logic.get_players_mut().insert(1, p1);

    let mut unit_tpl = ThingTemplate::new("Ranger");
    unit_tpl.add_kind_of(KindOf::Infantry).set_health(100.0);
    logic.templates.insert("Ranger".to_string(), unit_tpl);
    let unit = logic
        .create_object_for_player("Ranger", 0, Vec3::ZERO)
        .expect("unit");
    if let Some(obj) = logic.host_object_mut(unit) {
        obj.owner_player_id = Some(0);
    }

    let result = {
        let mut exec = CommandExecutor::new(&mut logic, 0);
        exec.execute_command(dispatch_test_command(
            CommandType::SelfDestruct {
                transfer_to_ally: true,
            },
            0,
            vec![],
        ))
        .expect("execute")
    };
    assert_eq!(result, CommandResult::Success);
    assert!(!logic.get_player(0).unwrap().is_alive);
    assert_eq!(
        logic.host_object(unit).map(|o| o.owner_player_id),
        Some(Some(1)),
        "unit must transfer to the living mutual ally"
    );
}

#[test]
fn place_beacon_spawns_world_object_and_honors_cap() {
    // C++ GameLogicDispatch.cpp:1582-1671 MSG_PLACE_BEACON.
    use super::CommandExecutor;
    use crate::command_system::{CommandResult, CommandType};
    use crate::game_logic::{GameLogic, Player, Team};
    use glam::Vec3;

    let mut logic = GameLogic::new();
    logic
        .get_players_mut()
        .insert(0, Player::new(0, Team::USA, "P0", true));

    let place = |logic: &mut GameLogic| {
        let mut exec = CommandExecutor::new(logic, 0);
        exec.execute_command(dispatch_test_command(
            CommandType::PlaceBeacon {
                location: Vec3::new(10.0, 0.0, 12.0),
                text: "here".into(),
            },
            0,
            vec![],
        ))
        .expect("execute")
    };

    assert_eq!(place(&mut logic), CommandResult::Success);
    let beacons: Vec<_> = logic
        .host_objects()
        .iter()
        .filter(|(_, o)| o.template_name.to_ascii_lowercase().contains("beacon"))
        .map(|(id, _)| *id)
        .collect();
    assert_eq!(beacons.len(), 1);
    let id = beacons[0];
    assert_eq!(
        crate::command_executor::host_beacon_caption(id).as_deref(),
        Some("here")
    );

    let max = game_engine::common::ini::ini_multiplayer::with_multiplayer_settings(|s| {
        s.max_beacons_per_player
    })
    .max(1);
    for _ in 1..max {
        assert_eq!(place(&mut logic), CommandResult::Success);
    }
    assert_eq!(
        place(&mut logic),
        CommandResult::InvalidCommand,
        "cap must refuse extra beacons"
    );
}

/// C++ BuildAssistant.cpp:333-334 / :1365-1383 — placement clears trees/props.
#[test]
fn execute_build_clears_removable_and_map_trees() {
    use super::CommandExecutor;
    use crate::command_system::CommandResult;
    use crate::game_logic::{GameLogic, KindOf, Object, ObjectId, Player, Team, ThingTemplate};
    use glam::Vec3;

    let _ = game_client::terrain::terrain_visual::init_terrain_visual();
    {
        let mut guard = game_client::terrain::terrain_visual::get_terrain_visual()
            .expect("terrain visual lock");
        let visual = guard.as_mut().expect("terrain visual");
        visual.tree_buffer_mut().clear_all_trees();
        visual
            .tree_buffer_mut()
            .set_bounds(game_client::terrain::TreeRegion2D::new(
                glam::Vec2::new(-200.0, -200.0),
                glam::Vec2::new(200.0, 200.0),
            ));
        let mut data = game_client::terrain::TreeModuleData::default();
        data.model_name = "Oak".into();
        visual
            .tree_buffer_mut()
            .add_tree(
                88,
                glam::Vec3::new(80.0, 80.0, 0.0),
                1.0,
                0.0,
                1.0,
                data,
                game_client::terrain::TreeSphere {
                    center: glam::Vec3::ZERO,
                    radius: 4.0,
                },
            )
            .expect("add tree");
        assert!(visual.add_prop([80.0, 80.0, 0.0], 0.0, 1.0, "TreeProp"));
    }

    let mut logic = GameLogic::new();
    let mut player = Player::new(0, Team::USA, "USA", true);
    player.resources.supplies = 100_000;
    logic.add_player(player);

    let mut barracks = ThingTemplate::new("TestClearFootprintBarracks");
    barracks
        .add_kind_of(KindOf::Structure)
        .set_cost(50, 0)
        .set_health(1_000.0);
    logic
        .templates
        .insert("TestClearFootprintBarracks".into(), barracks);

    let mut dozer_t = ThingTemplate::new("AmericaVehicleDozer");
    dozer_t
        .add_kind_of(KindOf::Dozer)
        .add_kind_of(KindOf::Vehicle)
        .set_health(200.0);
    logic
        .templates
        .insert("AmericaVehicleDozer".into(), dozer_t.clone());
    let mut dozer = Object::new(dozer_t, ObjectId(9101), Team::USA);
    dozer.set_position(Vec3::new(40.0, 0.0, 80.0));
    dozer.owner_player_id = Some(0);
    logic.objects.insert(ObjectId(9101), dozer);

    let mut shrub_t = ThingTemplate::new("TreeOakShrub");
    shrub_t.set_health(10.0);
    let mut shrub = Object::new(shrub_t, ObjectId(9102), Team::Neutral);
    shrub.set_position(Vec3::new(80.0, 0.0, 80.0));
    shrub.status.effectively_dead = true;
    logic.objects.insert(ObjectId(9102), shrub);

    let site = Vec3::new(80.0, 0.0, 80.0);
    let result = {
        let mut exec = CommandExecutor::new(&mut logic, 0);
        exec.execute_build(&[ObjectId(9101)], "TestClearFootprintBarracks", site, 0.0)
    };
    assert_eq!(result, CommandResult::Success, "placement must succeed");

    let shrub = logic
        .host_object(ObjectId(9102))
        .expect("shrub still rostered");
    assert!(
        shrub.status.destroyed || !shrub.is_alive(),
        "hq-wtzcx: removable shrub under footprint must be destroyed"
    );

    let mut guard =
        game_client::terrain::terrain_visual::get_terrain_visual().expect("terrain visual lock");
    let visual = guard.as_mut().expect("terrain visual");
    assert!(
        !visual.construction_removals().is_empty(),
        "hq-wtzcx: execute_build must call removeTreesAndPropsForConstruction"
    );
    assert!(
        visual.terrain_props().is_empty(),
        "map prop under footprint must be removed"
    );
    assert!(
        visual
            .tree_buffer_mut()
            .trees()
            .iter()
            .all(|tree| tree.tree_type < 0),
        "map tree under footprint must be removed"
    );
}

#[test]
fn execute_build_clears_kindof_not_name_substrings() {
    use super::CommandExecutor;
    use crate::command_system::CommandResult;
    use crate::game_logic::{GameLogic, KindOf, Object, ObjectId, Player, Team, ThingTemplate};
    use glam::Vec3;

    let mut logic = GameLogic::new();
    let mut player = Player::new(0, Team::USA, "USA", true);
    player.resources.supplies = 100_000;
    logic.add_player(player);

    let mut barracks = ThingTemplate::new("TestKindOfClearBarracks");
    barracks
        .add_kind_of(KindOf::Structure)
        .set_cost(50, 0)
        .set_health(1_000.0);
    logic
        .templates
        .insert("TestKindOfClearBarracks".into(), barracks);

    let mut dozer_t = ThingTemplate::new("AmericaVehicleDozer");
    dozer_t
        .add_kind_of(KindOf::Dozer)
        .add_kind_of(KindOf::Vehicle)
        .set_health(200.0);
    logic
        .templates
        .insert("AmericaVehicleDozer".into(), dozer_t.clone());
    let mut dozer = Object::new(dozer_t, ObjectId(9201), Team::USA);
    dozer.set_position(Vec3::new(40.0, 0.0, 80.0));
    dozer.owner_player_id = Some(0);
    logic.objects.insert(ObjectId(9201), dozer);

    let mut cleared_t = ThingTemplate::new("NoTokenProp");
    cleared_t
        .add_kind_of(KindOf::ClearedByBuild)
        .set_health(10.0);
    let mut cleared = Object::new(cleared_t, ObjectId(9202), Team::Neutral);
    cleared.set_position(Vec3::new(80.0, 0.0, 80.0));
    logic.objects.insert(ObjectId(9202), cleared);

    let mut named_tree_t = ThingTemplate::new("AmericaTreeDummy");
    named_tree_t.set_health(10.0);
    let mut named_tree = Object::new(named_tree_t, ObjectId(9203), Team::Neutral);
    named_tree.set_position(Vec3::new(400.0, 0.0, 400.0));
    logic.objects.insert(ObjectId(9203), named_tree);

    let mut inert_t = ThingTemplate::new("TreeDebrisRubble");
    inert_t.add_kind_of(KindOf::Inert).set_health(10.0);
    let mut inert = Object::new(inert_t, ObjectId(9204), Team::Neutral);
    inert.set_position(Vec3::new(78.0, 0.0, 80.0));
    logic.objects.insert(ObjectId(9204), inert);

    let site = Vec3::new(80.0, 0.0, 80.0);
    let result = {
        let mut exec = CommandExecutor::new(&mut logic, 0);
        exec.execute_build(&[ObjectId(9201)], "TestKindOfClearBarracks", site, 0.0)
    };
    assert_eq!(result, CommandResult::Success);

    let cleared = logic.host_object(ObjectId(9202)).expect("cleared");
    assert!(
        cleared.status.destroyed || !cleared.is_alive(),
        "KINDOF_CLEARED_BY_BUILD must be removed even without a name token"
    );
    let named_tree = logic.host_object(ObjectId(9203)).expect("named tree");
    assert!(
        named_tree.is_alive() && !named_tree.status.destroyed,
        "name substring tree must not be removed without KindOf"
    );
    let inert = logic.host_object(ObjectId(9204)).expect("inert");
    assert!(
        inert.is_alive() && !inert.status.destroyed,
        "KINDOF_INERT must not be removable for construction"
    );
}

#[test]
fn execute_build_refuses_human_on_unmovables() {
    use super::CommandExecutor;
    use crate::command_system::CommandResult;
    use crate::game_logic::{GameLogic, KindOf, Object, ObjectId, Player, Team, ThingTemplate};
    use glam::Vec3;

    let mut logic = GameLogic::new();
    let mut player = Player::new(0, Team::USA, "USA", true);
    player.resources.supplies = 10_000;
    logic.add_player(player);

    let mut barracks = ThingTemplate::new("TestUnmovableBarracks");
    barracks
        .add_kind_of(KindOf::Structure)
        .set_cost(50, 0)
        .set_health(1_000.0);
    logic
        .templates
        .insert("TestUnmovableBarracks".into(), barracks);

    let mut dozer_t = ThingTemplate::new("AmericaVehicleDozer");
    dozer_t
        .add_kind_of(KindOf::Dozer)
        .add_kind_of(KindOf::Vehicle)
        .set_health(200.0);
    logic
        .templates
        .insert("AmericaVehicleDozer".into(), dozer_t.clone());
    let mut dozer = Object::new(dozer_t, ObjectId(9301), Team::USA);
    dozer.set_position(Vec3::new(40.0, 0.0, 80.0));
    dozer.owner_player_id = Some(0);
    logic.objects.insert(ObjectId(9301), dozer);

    // Neutral, no AI / not mobile — leftover moveObjects returns FALSE.
    let mut crate_t = ThingTemplate::new("UnmovableCrate");
    crate_t.set_health(10.0);
    let mut crate_obj = Object::new(crate_t, ObjectId(9302), Team::Neutral);
    crate_obj.set_position(Vec3::new(80.0, 0.0, 80.0));
    logic.objects.insert(ObjectId(9302), crate_obj);

    let site = Vec3::new(80.0, 0.0, 80.0);
    let money_before = logic
        .get_player(0)
        .map(|p| p.resources.supplies)
        .unwrap_or(0);
    let result = {
        let mut exec = CommandExecutor::new(&mut logic, 0);
        exec.execute_build(&[ObjectId(9301)], "TestUnmovableBarracks", site, 0.0)
    };
    assert_eq!(
        result,
        CommandResult::InvalidLocation,
        "hq-ys09u: human buildObjectNow must refuse when leftover moveObjects is FALSE"
    );
    let money_after = logic
        .get_player(0)
        .map(|p| p.resources.supplies)
        .unwrap_or(0);
    assert_eq!(money_before, money_after, "human must not be charged");
    assert!(
        !logic
            .objects
            .values()
            .any(|o| o.template_name == "TestUnmovableBarracks"),
        "no structure on un-scootable occupants"
    );
    let crate_obj = logic.host_object(ObjectId(9302)).expect("crate");
    assert!(crate_obj.is_alive() && !crate_obj.status.destroyed);
}

#[test]
fn execute_build_source_records_build_slot_and_docks() {
    let src = crate::command_executor::COMMAND_EXECUTOR_SRC;
    let i = src.find("fn execute_build").expect("execute_build");
    let w = &src[i..src.len().min(i + 8000)];
    assert!(
        w.contains("dozer_new_task_build")
            && w.contains("dozer_repair_approach_position")
            && w.contains("path_to_goal_with_state_ignoring"),
        "hq-gkpuk/hq-6gy32: execute_build must newTask BUILD, dock half-radius, ignoreObstacle"
    );
    let snap = include_str!("../../game_logic/world_scripts/ui_production.rs");
    let j = snap
        .find("fn flatten_and_snap_construction")
        .expect("flatten_and_snap");
    let f = &snap[j..snap.len().min(j + 2500)];
    assert!(
        f.contains("flatten_terrain_box_at") && f.contains("HostGeometryType::Box"),
        "hq-6smw3: flatten_and_snap must use GEOMETRY_BOX flatten, not cylinder-only"
    );
}

#[test]
fn dozer_place_and_cancel_use_calc_cost_to_build_handicap() {
    use super::CommandExecutor;
    use crate::command_system::CommandResult;
    use crate::game_logic::{GameLogic, KindOf, Object, ObjectId, Player, Team, ThingTemplate};
    use glam::Vec3;

    let mut logic = GameLogic::new();
    let mut player = Player::new(0, Team::USA, "USA", true);
    player.resources.supplies = 1_000;
    player.map_side.handicap_build_cost_buildings = 0.75;
    logic.add_player(player);

    let mut barracks = ThingTemplate::new("TestHandicapBarracks");
    barracks
        .add_kind_of(KindOf::Structure)
        .add_kind_of(KindOf::Selectable)
        .set_health(1_000.0)
        .set_cost(1_000, 0);
    logic
        .templates
        .insert("TestHandicapBarracks".into(), barracks);

    let mut dozer_t = ThingTemplate::new("AmericaVehicleDozer");
    dozer_t
        .add_kind_of(KindOf::Dozer)
        .add_kind_of(KindOf::Vehicle)
        .set_health(200.0);
    logic
        .templates
        .insert("AmericaVehicleDozer".into(), dozer_t.clone());
    let mut dozer = Object::new(dozer_t, ObjectId(9201), Team::USA);
    dozer.set_position(Vec3::new(0.0, 0.0, 0.0));
    dozer.owner_player_id = Some(0);
    logic.objects.insert(ObjectId(9201), dozer);

    assert_eq!(
        logic.modified_build_cost_supplies(0, "TestHandicapBarracks", 1_000),
        750
    );

    let result = {
        let mut exec = CommandExecutor::new(&mut logic, 0);
        exec.execute_build(
            &[ObjectId(9201)],
            "TestHandicapBarracks",
            Vec3::new(40.0, 0.0, 0.0),
            0.0,
        )
    };
    assert_eq!(
        result,
        CommandResult::Success,
        "handicap place must succeed"
    );
    assert_eq!(
        logic.get_player(0).unwrap().effective_supplies(),
        250,
        "hq-iherw: place charges calcCostToBuild (1000 * 0.75)"
    );

    let building_id = logic
        .objects
        .values()
        .find(|o| o.template_name == "TestHandicapBarracks")
        .map(|o| o.id)
        .expect("placed barracks");

    let cancel = {
        let mut exec = CommandExecutor::new(&mut logic, 0);
        exec.execute_cancel_construction(building_id, 0)
    };
    assert_eq!(cancel, CommandResult::Success);
    assert_eq!(
        logic.get_player(0).unwrap().effective_supplies(),
        1_000,
        "hq-iherw: cancel refunds calcCostToBuild, not raw INI BuildCost"
    );
}

#[test]
fn context_move_plays_voice_move_not_unit_command() {
    use super::CommandExecutor;
    use crate::command_system::CommandResult;
    use crate::game_logic::audio_dispatch_impl::{
        clear_test_template_voices, set_test_template_voice, UnitVoiceSlot,
    };
    use crate::game_logic::{GameLogic, KindOf, Team, ThingTemplate};
    use glam::Vec3;

    clear_test_template_voices();
    set_test_template_voice("CTX_MV", UnitVoiceSlot::Move, "TestContextVoiceMove");
    let mut logic = GameLogic::new();
    let mut tpl = ThingTemplate::new("CTX_MV");
    tpl.add_kind_of(KindOf::Infantry)
        .add_kind_of(KindOf::Selectable)
        .set_health(100.0);
    logic.templates.insert("CTX_MV".into(), tpl);
    let a = logic
        .create_object("CTX_MV", Team::USA, Vec3::ZERO)
        .unwrap();
    logic.queued_audio_events.clear();
    {
        let mut exec = CommandExecutor::new(&mut logic, 0);
        assert_eq!(
            exec.execute_move(&[a], Vec3::new(40.0, 0.0, 0.0)),
            CommandResult::Success
        );
    }
    assert!(
        logic
            .queued_audio_events
            .iter()
            .any(|e| e.event_type == "TestContextVoiceMove"),
        "execute_move must play VoiceMove: {:?}",
        logic
            .queued_audio_events
            .iter()
            .map(|e| e.event_type.clone())
            .collect::<Vec<_>>()
    );
    assert!(
        logic
            .queued_audio_events
            .iter()
            .all(|e| e.event_type != "UnitCommand" && e.event_type != "UnitVoiceMove"),
        "execute_move must not invent UnitCommand"
    );

    logic.queued_audio_events.clear();
    {
        let mut exec = CommandExecutor::new(&mut logic, 0);
        assert_eq!(
            exec.execute_move_to(&[a], Vec3::new(80.0, 0.0, 0.0), &[]),
            CommandResult::Success
        );
    }
    assert!(
        logic
            .queued_audio_events
            .iter()
            .any(|e| e.event_type == "TestContextVoiceMove"),
        "execute_move_to must play VoiceMove"
    );

    logic.queued_audio_events.clear();
    {
        let mut exec = CommandExecutor::new(&mut logic, 0);
        assert_eq!(
            exec.execute_attack_move(&[a], Vec3::new(120.0, 0.0, 0.0), -1),
            CommandResult::Success
        );
    }
    assert!(
        logic
            .queued_audio_events
            .iter()
            .any(|e| e.event_type == "TestContextVoiceMove"),
        "execute_attack_move must play VoiceMove"
    );
    clear_test_template_voices();
}

#[test]
fn salvage_click_plays_voice_salvage_not_voice_move() {
    // C++ CommandXlat.cpp:423-431 — valid VoiceSalvage replaces VoiceMove.
    use super::CommandExecutor;
    use crate::command_system::{CommandResult, CommandType};
    use crate::game_logic::audio_dispatch_impl::{
        clear_test_template_voices, set_test_template_voice, UnitVoiceSlot,
    };
    use crate::game_logic::{GameLogic, KindOf, Player, Team, ThingTemplate};
    use glam::Vec3;

    clear_test_template_voices();
    set_test_template_voice("CTX_SAL", UnitVoiceSlot::Move, "TestContextVoiceMove");
    set_test_template_voice("CTX_SAL", UnitVoiceSlot::Salvage, "TestContextVoiceSalvage");
    let mut logic = GameLogic::new();
    logic
        .get_players_mut()
        .insert(0, Player::new(0, Team::GLA, "P0", true));
    let mut tpl = ThingTemplate::new("CTX_SAL");
    tpl.add_kind_of(KindOf::Vehicle)
        .add_kind_of(KindOf::Selectable)
        .add_kind_of(KindOf::Salvager)
        .set_health(100.0);
    logic.templates.insert("CTX_SAL".into(), tpl);
    let a = logic
        .create_object("CTX_SAL", Team::GLA, Vec3::ZERO)
        .unwrap();
    if let Some(obj) = logic.host_object_mut(a) {
        obj.owner_player_id = Some(0);
    }
    logic.queued_audio_events.clear();
    {
        let mut exec = CommandExecutor::new(&mut logic, 0);
        let cmd = dispatch_test_command(
            CommandType::DoSalvage {
                destination: Vec3::new(40.0, 0.0, 0.0),
            },
            0,
            vec![a],
        );
        assert_eq!(
            exec.execute_command(cmd).expect("salvage"),
            CommandResult::Success
        );
    }
    let names: Vec<_> = logic
        .queued_audio_events
        .iter()
        .map(|e| e.event_type.as_str())
        .collect();
    assert!(
        names.contains(&"TestContextVoiceSalvage"),
        "DoSalvage must play VoiceSalvage: {names:?}"
    );
    assert!(
        !names.contains(&"TestContextVoiceMove"),
        "DoSalvage must not play VoiceMove when VoiceSalvage is valid: {names:?}"
    );
    clear_test_template_voices();
}

#[test]
fn context_attack_plays_voice_attack_and_air() {
    use super::CommandExecutor;
    use crate::command_system::CommandResult;
    use crate::game_logic::audio_dispatch_impl::{
        clear_test_template_voices, set_test_template_voice, UnitVoiceSlot,
    };
    use crate::game_logic::{GameLogic, KindOf, Team, ThingTemplate, Weapon};
    use glam::Vec3;

    clear_test_template_voices();
    set_test_template_voice("CTX_ATK", UnitVoiceSlot::Attack, "TestContextVoiceAttack");
    set_test_template_voice(
        "CTX_ATK",
        UnitVoiceSlot::AttackAir,
        "TestContextVoiceAttackAir",
    );
    let mut logic = GameLogic::new();
    let mut atk = ThingTemplate::new("CTX_ATK");
    atk.add_kind_of(KindOf::Infantry)
        .add_kind_of(KindOf::Selectable)
        .set_health(100.0);
    logic.templates.insert("CTX_ATK".into(), atk);
    let mut ground = ThingTemplate::new("CTX_GND");
    ground
        .add_kind_of(KindOf::Vehicle)
        .add_kind_of(KindOf::Selectable)
        .set_health(100.0);
    logic.templates.insert("CTX_GND".into(), ground);
    let mut air = ThingTemplate::new("CTX_AIR");
    air.add_kind_of(KindOf::Aircraft)
        .add_kind_of(KindOf::Selectable)
        .set_health(100.0);
    logic.templates.insert("CTX_AIR".into(), air);

    let ranger = logic
        .create_object("CTX_ATK", Team::USA, Vec3::ZERO)
        .unwrap();
    let tank = logic
        .create_object("CTX_GND", Team::China, Vec3::new(30.0, 0.0, 0.0))
        .unwrap();
    let jet = logic
        .create_object("CTX_AIR", Team::China, Vec3::new(50.0, 20.0, 0.0))
        .unwrap();
    {
        let u = logic.host_object_mut(ranger).unwrap();
        u.weapon = Some(Weapon {
            damage: 10.0,
            range: 80.0,
            ..Weapon::default()
        });
    }
    {
        let j = logic.host_object_mut(jet).unwrap();
        j.status.airborne_target = true;
    }

    logic.queued_audio_events.clear();
    {
        let mut exec = CommandExecutor::new(&mut logic, 0);
        assert_eq!(exec.execute_attack(&[ranger], tank), CommandResult::Success);
    }
    assert!(
        logic
            .queued_audio_events
            .iter()
            .any(|e| e.event_type == "TestContextVoiceAttack"),
        "execute_attack must play VoiceAttack: {:?}",
        logic
            .queued_audio_events
            .iter()
            .map(|e| e.event_type.clone())
            .collect::<Vec<_>>()
    );
    assert!(
        logic
            .queued_audio_events
            .iter()
            .all(|e| e.event_type != "UnitCommand" && e.event_type != "UnitVoiceAttack"),
        "execute_attack must not invent UnitCommand"
    );

    logic.queued_audio_events.clear();
    {
        let mut exec = CommandExecutor::new(&mut logic, 0);
        assert_eq!(exec.execute_attack(&[ranger], jet), CommandResult::Success);
    }
    assert!(
        logic
            .queued_audio_events
            .iter()
            .any(|e| e.event_type == "TestContextVoiceAttackAir"),
        "air target must play VoiceAttackAir: {:?}",
        logic
            .queued_audio_events
            .iter()
            .map(|e| e.event_type.clone())
            .collect::<Vec<_>>()
    );
    clear_test_template_voices();
}

#[test]
fn host_play_sound_effect_must_not_invent_unit_command_or_select() {
    let src = crate::cnc_game_engine::ENGINE_SRC;
    let i = src
        .find("fn host_play_sound_effect")
        .expect("host_play_sound_effect");
    let body = &src[i..src.len().min(i + 1800)];
    assert!(
        body.contains("SoundType::Select | SoundType::Command")
            && !body.contains("SoundType::Select => \"UnitSelect\"")
            && !body.contains("SoundType::Command => \"UnitCommand\""),
        "host_play_sound_effect must no-op invented UnitSelect/UnitCommand"
    );
}

#[test]
fn repair_heal_resume_snipe_and_special_play_authored_voices() {
    use super::CommandExecutor;
    use crate::command_system::{CommandResult, PowerTarget, SpecialPowerType};
    use crate::game_logic::audio_dispatch_impl::{
        clear_test_template_voices, set_test_initiate_sound, set_test_template_voice, UnitVoiceSlot,
    };
    use crate::game_logic::{GameLogic, KindOf, Team, ThingTemplate};
    use glam::Vec3;

    clear_test_template_voices();
    set_test_template_voice("VR_DOZ", UnitVoiceSlot::Repair, "TestVoiceRepair");
    set_test_template_voice(
        "VR_DOZ",
        UnitVoiceSlot::BuildResponse,
        "TestVoiceBuildResponse",
    );
    set_test_template_voice("VR_TANK", UnitVoiceSlot::Move, "TestVoiceMove");
    set_test_template_voice("VR_INF", UnitVoiceSlot::Move, "TestVoiceMoveInf");
    set_test_template_voice("VR_KELL", UnitVoiceSlot::SnipePilot, "TestVoiceSnipePilot");
    set_test_initiate_sound("SpySatellite", "TestInitiateSound");

    let mut logic = GameLogic::new();
    logic.add_player(crate::game_logic::Player::new(0, Team::USA, "USA", true));
    let mut doz = ThingTemplate::new("VR_DOZ");
    doz.add_kind_of(KindOf::Vehicle)
        .add_kind_of(KindOf::Dozer)
        .add_kind_of(KindOf::Selectable)
        .set_health(200.0);
    logic.templates.insert("VR_DOZ".into(), doz);
    let mut bld = ThingTemplate::new("VR_BLD");
    bld.add_kind_of(KindOf::Structure)
        .add_kind_of(KindOf::Selectable)
        .set_health(1000.0);
    logic.templates.insert("VR_BLD".into(), bld);
    let mut tank = ThingTemplate::new("VR_TANK");
    tank.add_kind_of(KindOf::Vehicle)
        .add_kind_of(KindOf::Selectable)
        .set_health(200.0);
    logic.templates.insert("VR_TANK".into(), tank);
    let mut pad = ThingTemplate::new("VR_PAD");
    pad.add_kind_of(KindOf::RepairPad)
        .add_kind_of(KindOf::Selectable)
        .set_health(1000.0);
    logic.templates.insert("VR_PAD".into(), pad);
    let mut inf = ThingTemplate::new("VR_INF");
    inf.add_kind_of(KindOf::Infantry)
        .add_kind_of(KindOf::Selectable)
        .set_health(100.0);
    logic.templates.insert("VR_INF".into(), inf);
    let mut heal = ThingTemplate::new("VR_HEAL");
    heal.add_kind_of(KindOf::HealPad)
        .add_kind_of(KindOf::Selectable)
        .set_health(1000.0);
    logic.templates.insert("VR_HEAL".into(), heal);
    let mut kell = ThingTemplate::new("VR_KELL");
    kell.add_kind_of(KindOf::Infantry)
        .add_kind_of(KindOf::Selectable)
        .set_health(100.0);
    logic.templates.insert("VR_KELL".into(), kell);
    let mut veh = ThingTemplate::new("VR_VEH");
    veh.add_kind_of(KindOf::Vehicle)
        .add_kind_of(KindOf::Selectable)
        .set_health(200.0);
    logic.templates.insert("VR_VEH".into(), veh);
    let mut sat = ThingTemplate::new("VR_SAT");
    sat.add_kind_of(KindOf::Structure)
        .add_kind_of(KindOf::Selectable)
        .set_health(1000.0);
    logic.templates.insert("VR_SAT".into(), sat);

    let dozer = logic
        .create_object("VR_DOZ", Team::USA, Vec3::ZERO)
        .unwrap();
    let damaged = logic
        .create_object("VR_BLD", Team::USA, Vec3::new(20.0, 0.0, 0.0))
        .unwrap();
    logic.host_object_mut(damaged).unwrap().health.current = 100.0;
    logic.queued_audio_events.clear();
    {
        let mut exec = CommandExecutor::new(&mut logic, 0);
        assert_eq!(
            exec.execute_repair(&[dozer], damaged),
            CommandResult::Success
        );
    }
    assert!(
        logic
            .queued_audio_events
            .iter()
            .any(|e| e.event_type == "TestVoiceRepair"),
        "execute_repair must play VoiceRepair: {:?}",
        logic.queued_audio_events
    );

    let scaffold = logic
        .create_object("VR_BLD", Team::USA, Vec3::new(40.0, 0.0, 0.0))
        .unwrap();
    logic
        .host_object_mut(scaffold)
        .unwrap()
        .set_status_under_construction(true);
    logic.queued_audio_events.clear();
    {
        let mut exec = CommandExecutor::new(&mut logic, 0);
        assert_eq!(
            exec.execute_resume_construction(&[dozer], scaffold),
            CommandResult::Success
        );
    }
    assert!(
        logic
            .queued_audio_events
            .iter()
            .any(|e| e.event_type == "TestVoiceBuildResponse"),
        "execute_resume_construction must play VoiceBuildResponse: {:?}",
        logic.queued_audio_events
    );

    let tank_id = logic
        .create_object("VR_TANK", Team::USA, Vec3::new(5.0, 0.0, 0.0))
        .unwrap();
    logic.host_object_mut(tank_id).unwrap().health.current = 10.0;
    let pad_id = logic
        .create_object("VR_PAD", Team::USA, Vec3::new(8.0, 0.0, 0.0))
        .unwrap();
    logic.queued_audio_events.clear();
    {
        let mut exec = CommandExecutor::new(&mut logic, 0);
        assert_eq!(
            exec.execute_get_repaired(&[tank_id], pad_id),
            CommandResult::Success
        );
    }
    assert!(
        logic
            .queued_audio_events
            .iter()
            .any(|e| e.event_type == "TestVoiceMove"),
        "execute_get_repaired must play VoiceMove: {:?}",
        logic.queued_audio_events
    );

    let inf_id = logic
        .create_object("VR_INF", Team::USA, Vec3::new(9.0, 0.0, 0.0))
        .unwrap();
    logic.host_object_mut(inf_id).unwrap().health.current = 10.0;
    let heal_id = logic
        .create_object("VR_HEAL", Team::USA, Vec3::new(12.0, 0.0, 0.0))
        .unwrap();
    logic.queued_audio_events.clear();
    {
        let mut exec = CommandExecutor::new(&mut logic, 0);
        assert_eq!(
            exec.execute_get_healed(&[inf_id], heal_id),
            CommandResult::Success
        );
    }
    assert!(
        logic
            .queued_audio_events
            .iter()
            .any(|e| e.event_type == "TestVoiceMoveInf"),
        "execute_get_healed must play VoiceMove: {:?}",
        logic.queued_audio_events
    );

    let kell_id = logic
        .create_object("VR_KELL", Team::USA, Vec3::ZERO)
        .unwrap();
    let enemy = logic
        .create_object("VR_VEH", Team::China, Vec3::new(10.0, 0.0, 0.0))
        .unwrap();
    logic.queued_audio_events.clear();
    {
        let mut exec = CommandExecutor::new(&mut logic, 0);
        assert_eq!(
            exec.execute_snipe_vehicle(&[kell_id], enemy),
            CommandResult::Success
        );
    }
    assert!(
        logic
            .queued_audio_events
            .iter()
            .any(|e| e.event_type == "TestVoiceSnipePilot"),
        "execute_snipe_vehicle must play VoiceSnipePilot: {:?}",
        logic.queued_audio_events
    );

    let sat_id = logic
        .create_object("VR_SAT", Team::USA, Vec3::new(60.0, 0.0, 0.0))
        .unwrap();
    {
        let o = logic.host_object_mut(sat_id).unwrap();
        o.special_power_cooldowns
            .insert(SpecialPowerType::SpySatellite, 0.0);
        o.special_power_ready = true;
    }
    logic.queued_audio_events.clear();
    {
        let mut exec = CommandExecutor::new(&mut logic, 0);
        assert_eq!(
            exec.execute_special_power(
                &[sat_id],
                &SpecialPowerType::SpySatellite,
                &PowerTarget::Location(Vec3::new(80.0, 0.0, 80.0)),
            ),
            CommandResult::Success
        );
    }
    assert!(
        logic
            .queued_audio_events
            .iter()
            .any(|e| e.event_type == "TestInitiateSound"),
        "execute_special_power must play InitiateSound: {:?}",
        logic.queued_audio_events
    );
    clear_test_template_voices();
}

#[test]
fn specialty_attack_voices_replace_voice_attack() {
    use super::CommandExecutor;
    use crate::command_system::{CommandResult, WeaponSlot, WeaponTarget};
    use crate::game_logic::audio_dispatch_impl::{
        clear_test_template_voices, set_test_template_voice, UnitVoiceSlot,
    };
    use crate::game_logic::{GameLogic, KindOf, Team, ThingTemplate, Weapon};
    use glam::Vec3;

    clear_test_template_voices();
    set_test_template_voice("SA_RNG", UnitVoiceSlot::Attack, "TestVoiceAttack");
    set_test_template_voice(
        "SA_RNG",
        UnitVoiceSlot::ClearBuilding,
        "TestVoiceClearBuilding",
    );
    set_test_template_voice("SA_RNG", UnitVoiceSlot::Subdue, "TestVoiceSubdue");
    set_test_template_voice("SA_COM", UnitVoiceSlot::Attack, "TestComancheAttack");
    set_test_template_voice(
        "SA_COM",
        UnitVoiceSlot::FireRocketPods,
        "TestVoiceFireRocketPods",
    );
    set_test_template_voice("SA_DRG", UnitVoiceSlot::Attack, "TestDragonAttack");
    set_test_template_voice(
        "SA_DRG",
        UnitVoiceSlot::FlameLocation,
        "TestVoiceFlameLocation",
    );
    set_test_template_voice("SA_BUR", UnitVoiceSlot::Attack, "TestBurtonAttack");
    set_test_template_voice("SA_BUR", UnitVoiceSlot::Melee, "TestVoiceMelee");

    let mut logic = GameLogic::new();
    let mut ranger = ThingTemplate::new("SA_RNG");
    ranger
        .add_kind_of(KindOf::Infantry)
        .add_kind_of(KindOf::Selectable)
        .set_health(100.0)
        .set_primary_weapon_name("RangerAdvancedCombatRifle")
        .set_secondary_weapon_name("RangerFlashBangGrenadeWeapon");
    logic.templates.insert("SA_RNG".into(), ranger);
    let mut bldg = ThingTemplate::new("SA_BLD");
    bldg.add_kind_of(KindOf::Structure)
        .add_kind_of(KindOf::Selectable)
        .set_health(400.0);
    logic.templates.insert("SA_BLD".into(), bldg);
    let mut inf = ThingTemplate::new("SA_INF");
    inf.add_kind_of(KindOf::Infantry)
        .add_kind_of(KindOf::Selectable)
        .set_health(80.0);
    logic.templates.insert("SA_INF".into(), inf);
    let mut comanche = ThingTemplate::new("SA_COM");
    comanche
        .add_kind_of(KindOf::Aircraft)
        .add_kind_of(KindOf::Selectable)
        .set_health(200.0)
        .set_tertiary_weapon_name("ComancheRocketPodWeapon");
    logic.templates.insert("SA_COM".into(), comanche);
    let mut dragon = ThingTemplate::new("SA_DRG");
    dragon
        .add_kind_of(KindOf::Vehicle)
        .add_kind_of(KindOf::Selectable)
        .set_health(200.0)
        .set_secondary_weapon_name("DragonTankFireWallWeapon");
    logic.templates.insert("SA_DRG".into(), dragon);
    let mut burton = ThingTemplate::new("SA_BUR");
    burton
        .add_kind_of(KindOf::Infantry)
        .add_kind_of(KindOf::Selectable)
        .set_health(200.0)
        .set_tertiary_weapon_name("BurtonKnifeWeapon");
    logic.templates.insert("SA_BUR".into(), burton);

    let ranger_id = logic
        .create_object("SA_RNG", Team::USA, Vec3::ZERO)
        .unwrap();
    let bldg_id = logic
        .create_object("SA_BLD", Team::China, Vec3::new(40.0, 0.0, 0.0))
        .unwrap();
    let inf_id = logic
        .create_object("SA_INF", Team::China, Vec3::new(20.0, 0.0, 0.0))
        .unwrap();
    {
        let u = logic.host_object_mut(ranger_id).unwrap();
        u.weapon = Some(Weapon {
            damage: 5.0,
            range: 150.0,
            ..Weapon::default()
        });
        u.secondary_weapon = Some(Weapon {
            damage: 35.0,
            range: 175.0,
            ..Weapon::default()
        });
        u.set_active_weapon_slot(1);
    }

    logic.queued_audio_events.clear();
    {
        let mut exec = CommandExecutor::new(&mut logic, 0);
        assert_eq!(
            exec.execute_attack(&[ranger_id], bldg_id),
            CommandResult::Success
        );
    }
    assert!(
        logic
            .queued_audio_events
            .iter()
            .any(|e| e.event_type == "TestVoiceClearBuilding"),
        "flashbang vs structure must play VoiceClearBuilding: {:?}",
        logic.queued_audio_events
    );
    assert!(
        !logic
            .queued_audio_events
            .iter()
            .any(|e| e.event_type == "TestVoiceAttack"),
        "specialty line must replace VoiceAttack: {:?}",
        logic.queued_audio_events
    );

    logic.queued_audio_events.clear();
    {
        let mut exec = CommandExecutor::new(&mut logic, 0);
        assert_eq!(
            exec.execute_attack(&[ranger_id], inf_id),
            CommandResult::Success
        );
    }
    assert!(
        logic
            .queued_audio_events
            .iter()
            .any(|e| e.event_type == "TestVoiceSubdue"),
        "flashbang vs infantry must play VoiceSubdue: {:?}",
        logic.queued_audio_events
    );

    let com_id = logic
        .create_object("SA_COM", Team::USA, Vec3::new(0.0, 20.0, 0.0))
        .unwrap();
    {
        let u = logic.host_object_mut(com_id).unwrap();
        u.tertiary_weapon = Some(Weapon {
            damage: 30.0,
            range: 200.0,
            ..Weapon::default()
        });
    }
    logic.queued_audio_events.clear();
    {
        let mut exec = CommandExecutor::new(&mut logic, 0);
        assert_eq!(
            exec.execute_weapon(
                &[com_id],
                &WeaponSlot::Tertiary,
                -1,
                &WeaponTarget::Location(Vec3::new(80.0, 0.0, 80.0)),
            ),
            CommandResult::Success
        );
    }
    assert!(
        logic
            .queued_audio_events
            .iter()
            .any(|e| e.event_type == "TestVoiceFireRocketPods"),
        "Comanche rocket pods must play VoiceFireRocketPods: {:?}",
        logic.queued_audio_events
    );

    let dragon_id = logic
        .create_object("SA_DRG", Team::China, Vec3::new(5.0, 0.0, 5.0))
        .unwrap();
    {
        let u = logic.host_object_mut(dragon_id).unwrap();
        u.secondary_weapon = Some(Weapon {
            damage: 20.0,
            range: 80.0,
            ..Weapon::default()
        });
    }
    logic.queued_audio_events.clear();
    {
        let mut exec = CommandExecutor::new(&mut logic, 0);
        assert_eq!(
            exec.execute_weapon(
                &[dragon_id],
                &WeaponSlot::Secondary,
                -1,
                &WeaponTarget::Location(Vec3::new(60.0, 0.0, 60.0)),
            ),
            CommandResult::Success
        );
    }
    assert!(
        logic
            .queued_audio_events
            .iter()
            .any(|e| e.event_type == "TestVoiceFlameLocation"),
        "non-primary flame ground fire must play VoiceFlameLocation: {:?}",
        logic.queued_audio_events
    );

    let burton_id = logic
        .create_object("SA_BUR", Team::USA, Vec3::new(2.0, 0.0, 2.0))
        .unwrap();
    {
        let u = logic.host_object_mut(burton_id).unwrap();
        u.tertiary_weapon = Some(Weapon {
            damage: 40.0,
            range: 10.0,
            ..Weapon::default()
        });
    }
    logic.queued_audio_events.clear();
    {
        let mut exec = CommandExecutor::new(&mut logic, 0);
        assert_eq!(
            exec.execute_weapon(
                &[burton_id],
                &WeaponSlot::Tertiary,
                -1,
                &WeaponTarget::Object(inf_id),
            ),
            CommandResult::Success
        );
    }
    assert!(
        logic
            .queued_audio_events
            .iter()
            .any(|e| e.event_type == "TestVoiceMelee"),
        "specialty melee must play VoiceMelee: {:?}",
        logic.queued_audio_events
    );

    clear_test_template_voices();
}

#[test]
fn click_to_gather_uses_gamedata_factor_and_cell_cap() {
    // C++ AIGroup.cpp:1559-1608 — retail GroupMoveClickToGatherAreaFactor=0.5
    // and cells<2000 (x-span used twice after ScaleRect2D).
    use super::CommandExecutor;
    use crate::game_logic::{GameLogic, KindOf, Team, ThingTemplate};
    use glam::Vec3;

    let prev_factor = game_engine::common::ini::get_global_data()
        .map(|d| d.read().group_move_click_to_gather_factor);
    if let Some(data) = game_engine::common::ini::get_global_data() {
        data.write().group_move_click_to_gather_factor = 0.5;
    }

    let mut logic = GameLogic::new();
    let mut tpl = ThingTemplate::new("CG_V");
    tpl.add_kind_of(KindOf::Vehicle);
    tpl.add_kind_of(KindOf::Selectable);
    tpl.set_health(200.0);
    logic.templates.insert("CG_V".to_string(), tpl);
    let a = logic
        .create_object("CG_V", Team::USA, Vec3::new(0.0, 0.0, 0.0))
        .unwrap();
    let b = logic
        .create_object("CG_V", Team::USA, Vec3::new(100.0, 0.0, 0.0))
        .unwrap();
    let exec = CommandExecutor::new(&mut logic, 0);
    assert!(
        exec.should_tighten_group_move(&[a, b], Vec3::new(50.0, 0.0, 0.0)),
        "click in the central half of the bbox must tighten"
    );
    assert!(
        !exec.should_tighten_group_move(&[a, b], Vec3::new(5.0, 0.0, 0.0)),
        "click inside the full bbox but outside the 0.5 gather rect must not tighten"
    );

    if let Some(data) = game_engine::common::ini::get_global_data() {
        data.write().group_move_click_to_gather_factor = prev_factor.unwrap_or(0.5);
    }
}

#[test]
fn click_to_gather_skips_wide_bbox_over_2000_cells() {
    // C++ AIGroup.cpp:1602-1608 — widely spread group keeps relative offsets.
    use super::CommandExecutor;
    use crate::game_logic::{GameLogic, KindOf, Team, ThingTemplate};
    use glam::Vec3;

    let mut logic = GameLogic::new();
    let mut tpl = ThingTemplate::new("CW_V");
    tpl.add_kind_of(KindOf::Vehicle);
    tpl.add_kind_of(KindOf::Selectable);
    tpl.set_health(200.0);
    logic.templates.insert("CW_V".to_string(), tpl);
    let a = logic
        .create_object("CW_V", Team::USA, Vec3::new(0.0, 0.0, 0.0))
        .unwrap();
    let b = logic
        .create_object("CW_V", Team::USA, Vec3::new(20000.0, 0.0, 0.0))
        .unwrap();
    let exec = CommandExecutor::new(&mut logic, 0);
    assert!(
        !exec.should_tighten_group_move(&[a, b], Vec3::new(10000.0, 0.0, 0.0)),
        "screen-wide selection must not collapse onto the click"
    );
}

#[test]
fn attack_move_includes_immobile_attacker() {
    // C++ AIGroup.cpp:2260-2273 — no can_move gate. Turret stays engaging.
    use super::CommandExecutor;
    use crate::command_system::CommandResult;
    use crate::game_logic::{AIState, GameLogic, KindOf, Team, ThingTemplate, Weapon};
    use glam::Vec3;

    let mut logic = GameLogic::new();
    let mut turret = ThingTemplate::new("AM_TUR");
    turret.add_kind_of(KindOf::Structure);
    turret.add_kind_of(KindOf::Immobile);
    turret.add_kind_of(KindOf::Selectable);
    turret.set_health(400.0);
    logic.templates.insert("AM_TUR".to_string(), turret);
    let mut tank = ThingTemplate::new("AM_TK");
    tank.add_kind_of(KindOf::Vehicle);
    tank.add_kind_of(KindOf::Selectable);
    tank.set_health(200.0);
    logic.templates.insert("AM_TK".to_string(), tank);
    let tur = logic
        .create_object("AM_TUR", Team::USA, Vec3::new(0.0, 0.0, 0.0))
        .unwrap();
    let tk = logic
        .create_object("AM_TK", Team::USA, Vec3::new(20.0, 0.0, 0.0))
        .unwrap();
    for id in [tur, tk] {
        let o = logic.host_object_mut(id).unwrap();
        o.weapon = Some(Weapon {
            damage: 10.0,
            range: 150.0,
            ..Weapon::default()
        });
    }
    assert!(
        !logic.host_object(tur).unwrap().can_move(),
        "turret fixture must be immobile"
    );
    {
        let mut exec = CommandExecutor::new(&mut logic, 0);
        assert_eq!(
            exec.execute_attack_move(&[tur, tk], Vec3::new(200.0, 0.0, 0.0), 4),
            CommandResult::Success
        );
    }
    let u = logic.host_object(tur).unwrap();
    assert_eq!(u.ai_state, AIState::AttackMoving);
    assert!(u.is_attack_path);
    assert_eq!(u.max_shots_to_fire, 4);
}

#[test]
fn group_path_thresholds_read_aidata_store() {
    // C++ friend_computeGroundPath uses AIData MinDistance/DistanceRequiresGroup.
    let src = crate::command_executor::COMMAND_EXECUTOR_SRC;
    let i = src
        .find("fn group_path_distance_thresholds")
        .expect("group_path_distance_thresholds");
    let w = &src[i..src.len().min(i + 1800)];
    assert!(
        w.contains("get_ai_data_store")
            && w.contains("min_distance_for_group")
            && w.contains("distance_requires_group"),
        "group-path must read leftover AIData thresholds"
    );

    use super::CommandExecutor;
    use crate::game_logic::{GameLogic, KindOf, Team, ThingTemplate};
    use glam::Vec3;

    let prev = {
        let mut store = game_engine::common::ini::get_ai_data_store_mut();
        store.ensure_base();
        let data = store.get_active_mut().expect("aidata");
        let prev = (data.min_distance_for_group, data.distance_requires_group);
        data.min_distance_for_group = 10000.0;
        data.distance_requires_group = 50000.0;
        prev
    };
    let mut logic = GameLogic::new();
    let mut tpl = ThingTemplate::new("GP_TH");
    tpl.add_kind_of(KindOf::Vehicle);
    tpl.add_kind_of(KindOf::Selectable);
    tpl.set_health(200.0);
    logic.templates.insert("GP_TH".to_string(), tpl);
    let a = logic
        .create_object("GP_TH", Team::USA, Vec3::new(0.0, 0.0, 0.0))
        .unwrap();
    let b = logic
        .create_object("GP_TH", Team::USA, Vec3::new(40.0, 0.0, 0.0))
        .unwrap();
    {
        let exec = CommandExecutor::new(&mut logic, 0);
        assert!(
            !exec.compute_ground_path_should_group(&[a, b], Vec3::new(300.0, 0.0, 0.0)),
            "click below AIData MinDistanceForGroup must skip group path"
        );
    }
    {
        let mut store = game_engine::common::ini::get_ai_data_store_mut();
        if let Some(data) = store.get_active_mut() {
            data.min_distance_for_group = prev.0;
            data.distance_requires_group = prev.1;
        }
    }
}

#[test]
fn click_to_gather_source_reads_gamedata_and_caps_cells() {
    let src = crate::command_executor::COMMAND_EXECUTOR_SRC;
    let i = src
        .find("fn should_tighten_group_move")
        .expect("should_tighten_group_move");
    let w = &src[i..src.len().min(i + 2500)];
    assert!(
        w.contains("group_move_click_to_gather_factor")
            && w.contains("cells < 2000")
            && !w.contains("hx.max(20.0)"),
        "tighten must use GameData factor, 2000-cell cap, no 20wu pad"
    );
}

#[test]
fn group_attack_object_orders_weaponless_and_dead_victim() {
    // C++ AIGroup::groupAttackObjectPrivate: no isAbleToAttack gate; dead-but-present
    // victim still receives aiAttackObject (AIGroup.cpp:2100-2173).
    use super::CommandExecutor;
    use crate::command_system::CommandResult;
    use crate::game_logic::{AIState, GameLogic, KindOf, Team, ThingTemplate};
    use glam::Vec3;

    let mut logic = GameLogic::new();
    let mut dozer = ThingTemplate::new("ATK_DOZ");
    dozer
        .add_kind_of(KindOf::Vehicle)
        .add_kind_of(KindOf::Dozer)
        .add_kind_of(KindOf::Selectable)
        .set_health(200.0);
    logic.templates.insert("ATK_DOZ".into(), dozer);
    let mut vic_tpl = ThingTemplate::new("ATK_VIC");
    vic_tpl
        .add_kind_of(KindOf::Vehicle)
        .add_kind_of(KindOf::Selectable)
        .set_health(200.0);
    logic.templates.insert("ATK_VIC".into(), vic_tpl);

    let dozer_id = logic
        .create_object("ATK_DOZ", Team::USA, Vec3::new(0.0, 0.0, 0.0))
        .unwrap();
    let victim = logic
        .create_object("ATK_VIC", Team::China, Vec3::new(40.0, 0.0, 0.0))
        .unwrap();
    {
        let d = logic.host_object(dozer_id).unwrap();
        assert!(!d.can_attack(), "weaponless dozer must fail can_attack");
    }
    {
        let v = logic.host_object_mut(victim).unwrap();
        v.health.current = 0.0;
        assert!(!v.is_alive(), "dead-but-present victim");
    }
    {
        let mut exec = CommandExecutor::new(&mut logic, 0);
        assert_eq!(
            exec.execute_attack(&[dozer_id], victim),
            CommandResult::Success
        );
    }
    let d = logic.host_object(dozer_id).unwrap();
    assert_eq!(d.target, Some(victim));
    assert_eq!(d.ai_state, AIState::Attacking);
}

#[test]
fn free_move_dissolves_mixed_formation_stamps() {
    // C++ groupMoveToPosition free-move: setFormationID(NO_FORMATION_ID) (AIGroup.cpp:1681).
    use super::CommandExecutor;
    use crate::command_system::CommandResult;
    use crate::game_logic::{GameLogic, KindOf, Team, ThingTemplate};
    use glam::Vec3;

    let mut logic = GameLogic::new();
    for name in ["FMX_A", "FMX_B", "FMX_C", "FMX_D", "FMX_E"] {
        let mut tpl = ThingTemplate::new(name);
        tpl.add_kind_of(KindOf::Vehicle);
        tpl.add_kind_of(KindOf::Selectable);
        tpl.set_health(200.0);
        logic.templates.insert(name.to_string(), tpl);
    }
    let stamped: Vec<_> = ["FMX_A", "FMX_B", "FMX_C"]
        .iter()
        .enumerate()
        .map(|(i, name)| {
            logic
                .create_object(name, Team::USA, Vec3::new(i as f32 * 10.0, 0.0, 0.0))
                .unwrap()
        })
        .collect();
    let extra: Vec<_> = ["FMX_D", "FMX_E"]
        .iter()
        .enumerate()
        .map(|(i, name)| {
            logic
                .create_object(name, Team::USA, Vec3::new(30.0 + i as f32 * 10.0, 0.0, 0.0))
                .unwrap()
        })
        .collect();
    {
        let mut exec = CommandExecutor::new(&mut logic, 0);
        assert_eq!(
            exec.execute_create_formation(&stamped),
            CommandResult::Success
        );
    }
    assert_ne!(logic.host_object(stamped[0]).unwrap().formation_id, 0);
    let mut all = stamped.clone();
    all.extend(extra);
    // Outside gather bbox, closer than MinDistanceForGroup so no column pack.
    {
        let mut exec = CommandExecutor::new(&mut logic, 0);
        assert_eq!(
            exec.execute_move(&all, Vec3::new(80.0, 0.0, 0.0)),
            CommandResult::Success
        );
    }
    for id in stamped {
        assert_eq!(
            logic.host_object(id).unwrap().formation_id,
            0,
            "free-move must dissolve stale formation stamps"
        );
    }
}

#[test]
fn tighten_scatter_include_stunned_members() {
    // C++ tighten/scatter/bbox: Held/Immobile/no-AI only. Stun still counts.
    use super::CommandExecutor;
    use crate::command_system::CommandResult;
    use crate::game_logic::{AIState, GameLogic, KindOf, Team, ThingTemplate};
    use glam::Vec3;

    let mut logic = GameLogic::new();
    let mut tpl = ThingTemplate::new("STN_V");
    tpl.add_kind_of(KindOf::Vehicle);
    tpl.add_kind_of(KindOf::Selectable);
    tpl.set_health(200.0);
    logic.templates.insert("STN_V".into(), tpl);
    let a = logic
        .create_object("STN_V", Team::USA, Vec3::new(0.0, 0.0, 0.0))
        .unwrap();
    let b = logic
        .create_object("STN_V", Team::USA, Vec3::new(30.0, 0.0, 0.0))
        .unwrap();
    {
        let u = logic.host_object_mut(b).unwrap();
        u.shock_stun_frames = 40;
        assert!(!u.can_move(), "stun must block can_move");
    }
    {
        let exec = CommandExecutor::new(&mut logic, 0);
        assert!(
            exec.should_tighten_group_move(&[a, b], Vec3::new(15.0, 0.0, 0.0)),
            "stunned member must still count in the gather bbox"
        );
    }
    {
        let mut exec = CommandExecutor::new(&mut logic, 0);
        assert_eq!(
            exec.execute_tighten_to_position(&[a, b], Vec3::new(15.0, 0.0, 0.0)),
            CommandResult::Success
        );
    }
    assert_eq!(logic.host_object(b).unwrap().ai_state, AIState::Moving);
    {
        let u = logic.host_object_mut(b).unwrap();
        u.shock_stun_frames = 40;
        u.set_ai_state(AIState::Idle);
    }
    {
        let mut exec = CommandExecutor::new(&mut logic, 0);
        assert_eq!(exec.execute_scatter(&[a, b]), CommandResult::Success);
    }
    assert_eq!(logic.host_object(b).unwrap().ai_state, AIState::Moving);
}

#[test]
fn free_move_clamp_uses_geometry_not_selection_radius() {
    // C++ computeIndividualDestination: 6 * getBoundingCircleRadius (AIGroup.cpp:470-471).
    use super::CommandExecutor;
    use crate::game_logic::{GameLogic, KindOf, Team, ThingTemplate};
    use glam::Vec3;

    let mut logic = GameLogic::new();
    for name in ["CL_A", "CL_B"] {
        let mut tpl = ThingTemplate::new(name);
        tpl.add_kind_of(KindOf::Vehicle);
        tpl.add_kind_of(KindOf::Selectable);
        tpl.set_health(200.0);
        logic.templates.insert(name.to_string(), tpl);
    }
    let a = logic
        .create_object("CL_A", Team::USA, Vec3::new(0.0, 0.0, 0.0))
        .unwrap();
    let b = logic
        .create_object("CL_B", Team::USA, Vec3::new(40.0, 0.0, 0.0))
        .unwrap();
    for id in [a, b] {
        let o = logic.host_object_mut(id).unwrap();
        o.selection_radius = 100.0;
        o.thing.geometry.radius = 2.0;
        o.thing.geometry.bounds_min = Vec3::new(-2.0, 0.0, -2.0);
        o.thing.geometry.bounds_max = Vec3::new(2.0, 0.0, 2.0);
    }
    let click = Vec3::new(100.0, 0.0, 0.0);
    let exec = CommandExecutor::new(&mut logic, 0);
    let goals = exec.group_move_destinations(&[a, b], click);
    let ga = goals.iter().find(|(id, _)| *id == a).unwrap().1;
    // B is nearer the click → lead. A offset 40 clamps to 6 * 2 = 12.
    // selection_radius 100 would have allowed 600 (no clamp).
    assert!(
        (ga.x - 88.0).abs() < 1.0,
        "free-move clamp must use geometry radius, ga={ga:?}"
    );
}
