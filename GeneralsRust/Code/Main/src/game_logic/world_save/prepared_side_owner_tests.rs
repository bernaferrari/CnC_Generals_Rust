//! Actual map preparation boundary: C++ SidesList467 / PlayerList110–121.
use super::*;
use crate::save_load::{SnapshotBuilder, snapshot::decode_bincode_world_snapshot};
use crate::skirmish_config::{apply_skirmish_config, golden_skirmish_config};

fn isolated(name: &str, run: impl FnOnce()) {
    crate::game_logic::game_logic::pose_owner_tests::isolated_at(module_path!(), name, run);
}

fn configured() -> GameLogic {
    crate::skirmish_config::lobby_definition_tests::admit_slot_definitions();
    let mut config = golden_skirmish_config("PreparedSideAdmission.map");
    config.slots[1].slot_index = 3;
    let mut world = GameLogic::new();
    apply_skirmish_config(&mut world, &config).unwrap();
    world
}

fn map_rows(named: &str, include_neutral: bool) -> (Vec<Dict>, Vec<ScriptList>) {
    let mut civilian = Dict::new();
    civilian.set_ascii_string(key_player_name(), named);
    civilian.set_ascii_string(key_player_faction(), "FactionCivilian");
    civilian.set_unicode_string(key_player_display_name(), "ReplayObserver");
    let mut faction = Dict::new();
    faction.set_ascii_string(key_player_name(), "MapFactionSource");
    faction.set_ascii_string(key_player_faction(), "FactionGLA");
    let mut script = ScriptList::new();
    script.append_script(Box::new(gamelogic::scripting::core::Script::new()));
    let mut rows = vec![civilian, faction];
    let mut scripts = vec![ScriptList::new(), script];
    if include_neutral {
        let mut neutral = Dict::new();
        neutral.set_ascii_string(key_player_name(), "");
        neutral.set_ascii_string(key_player_faction(), "FactionCivilian");
        neutral.set_bool(key_player_is_human(), true);
        neutral.set_int(key_player_start_money(), 91_000);
        rows.insert(0, neutral);
        scripts.insert(0, ScriptList::new());
    }
    (rows, scripts)
}

fn prepare(world: &mut GameLogic, name: &str, include_neutral: bool) {
    let (rows, scripts) = map_rows(name, include_neutral);
    let source_index = if include_neutral { 2 } else { 1 };
    let builds = [super::script_loader::SideBuildEntry {
        building_name: "SourceBase".into(),
        template: "GLACommandCenter".into(),
        position: gamelogic::scripting::core::Coord3D::new(40.0, 70.0, 0.0),
        angle: 0.75,
        initially_built: true,
        num_rebuilds: 2,
        side_index: source_index,
        script_name: Some("BuildSource".into()),
        health: Some(80),
        whiner: Some(false),
        unsellable: Some(true),
        repairable: Some(true),
    }];
    world.sync_legacy_sides_list_from_dicts(&rows, &[], &builds, &scripts);
}

fn retained(world: &GameLogic, name: &str) -> u32 {
    world
        .players
        .values()
        .find(|p| p.map_side.map_player_name == name)
        .unwrap_or_else(|| panic!("missing retained owner {name}"))
        .id
}

#[test]
fn prepared_map_adapter_calibration_preserves_source_order_and_scripts() {
    isolated(
        "prepared_map_adapter_calibration_preserves_source_order_and_scripts",
        || {
            let mut world = configured();
            prepare(&mut world, "PlyrCivilian", false);
            let adapter = get_sides_list();
            let sides = adapter.read().unwrap();
            let names: Vec<_> = (0..sides.get_num_sides())
                .map(|i| {
                    sides
                        .get_side_info(i)
                        .unwrap()
                        .get_dict()
                        .get_ascii_string(key_player_name())
                })
                .collect();
            assert_eq!(
                names,
                ["PlyrCivilian", "player0", "player3", "ReplayObserver", ""],
                "missing neutral is appended, never normalized to row zero"
            );
            assert!(sides.find_side_info("MapFactionSource").is_none());
            let source = sides.find_skirmish_side_info("MapFactionSource").unwrap();
            assert!(
                sides
                    .get_skirmish_side_info(source)
                    .unwrap()
                    .get_script_list()
                    .unwrap()
                    .get_script()
                    .is_some()
            );
            let build = sides
                .get_skirmish_side_info(source)
                .unwrap()
                .get_build_list()
                .unwrap();
            assert_eq!(build.get_template_name().as_str(), "GLACommandCenter");
            assert_eq!(build.get_angle(), 0.75);
            assert_eq!(build.get_script().as_str(), "BuildSource");
            assert_eq!(world.players[&3].map_side.map_player_name, "player3");
        },
    );
}

#[test]
fn prepared_skirmish_admits_retained_owners_without_raw_side_overwrite() {
    isolated(
        "prepared_skirmish_admits_retained_owners_without_raw_side_overwrite",
        || {
            let mut world = configured();
            prepare(&mut world, "PlyrCivilian", true);
            assert_eq!(
                world.players.len(),
                5,
                "final prepared rows admit both retained owners"
            );
            assert_eq!(world.players[&0].map_side.map_player_name, "player0");
            assert_eq!(world.players[&3].map_side.map_player_name, "player3");
            assert!(
                !world
                    .players
                    .values()
                    .any(|p| p.map_side.map_player_name == "MapFactionSource")
            );
            let neutral = retained(&world, "");
            let named = retained(&world, "PlyrCivilian");
            assert_ne!(neutral, named);
            assert!(world.players[&neutral].is_reserved_neutral());
            assert_eq!(world.players[&neutral].resources.supplies, 0);
            assert!(!world.players[&neutral].is_human);
            assert_eq!(world.players[&named].resources.supplies, 10_000);
            assert_eq!(world.players[&named].name, "ReplayObserver");
            assert_ne!(world.replay_observer_player_id(), Some(named));
            prepare(&mut world, "PlyrCivilian", true);
            assert_eq!(
                world.players.len(),
                5,
                "same admitted rows do not allocate again"
            );
            assert_eq!(retained(&world, "PlyrCivilian"), named);
        },
    );
}

#[test]
fn busy_compatibility_adapter_cannot_skip_host_admission() {
    isolated(
        "busy_compatibility_adapter_cannot_skip_host_admission",
        || {
            let mut world = configured();
            let adapter = get_sides_list();
            let mut foreign = adapter.write().unwrap();
            foreign.reset();
            let mut marker = Dict::new();
            marker.set_ascii_string(key_player_name(), "ForeignOwner");
            foreign.add_side(&marker);
            prepare(&mut world, "PlyrCivilian", false);
            assert_eq!(
                world.players.len(),
                5,
                "host admission precedes compatibility publication"
            );
            assert!(foreign.find_side_info("ForeignOwner").is_some());
            assert!(foreign.find_side_info("PlyrCivilian").is_none());
            assert!(world.players[&retained(&world, "")].is_reserved_neutral());
        },
    );
}

#[test]
fn prepared_owners_survive_foreign_same_id_world_reset_and_query_clone() {
    isolated(
        "prepared_owners_survive_foreign_same_id_world_reset_and_query_clone",
        || {
            let mut first = configured();
            prepare(&mut first, "FirstCivilian", false);
            let owner = retained(&first, "FirstCivilian");
            let mut second = configured();
            prepare(&mut second, "SecondCivilian", false);
            assert_eq!(retained(&second, "SecondCivilian"), owner);
            second.players.get_mut(&owner).unwrap().name = "Foreign mutation".into();
            second.reset();
            drop(second);
            assert_eq!(first.players[&owner].name, "ReplayObserver");
            let clone = first.players[&owner].clone();
            first.add_player(clone);
            prepare(&mut first, "FirstCivilian", false);
            assert_eq!(retained(&first, "FirstCivilian"), owner);
            assert_eq!(first.players.len(), 5);
            first.reset();
            assert!(
                !first
                    .players
                    .values()
                    .any(|p| p.map_side.map_player_name == "FirstCivilian")
            );
        },
    );
}

#[test]
fn prepared_admission_roles_survive_encoded_snapshot_restore() {
    isolated(
        "prepared_admission_roles_survive_encoded_snapshot_restore",
        || {
            let mut source = configured();
            prepare(&mut source, "PlyrCivilian", false);
            let civilian = retained(&source, "PlyrCivilian");
            let neutral = retained(&source, "");
            let builder = SnapshotBuilder::new();
            let snapshot = builder.create_world_snapshot(&source).unwrap();
            let bytes = bincode_legacy::serialize(&snapshot).unwrap();
            let decoded = decode_bincode_world_snapshot(&bytes).unwrap();
            let mut foreign = configured();
            prepare(&mut foreign, "ForeignCivilian", true);
            foreign.reset();
            let mut restored = GameLogic::new();
            builder
                .restore_from_snapshot(&decoded, &mut restored)
                .unwrap();
            assert_eq!(retained(&restored, "PlyrCivilian"), civilian);
            assert!(restored.players[&neutral].is_reserved_neutral());
            assert_eq!(restored.players[&0].map_side.map_player_name, "player0");
            prepare(&mut restored, "PlyrCivilian", false);
            assert_eq!(restored.players.len(), 5);
            assert_eq!(retained(&restored, "PlyrCivilian"), civilian);
        },
    );
}

#[test]
fn empty_map_bootstrap_is_not_mistaken_for_an_admitted_lobby() {
    isolated(
        "empty_map_bootstrap_is_not_mistaken_for_an_admitted_lobby",
        || {
            crate::skirmish_config::lobby_definition_tests::admit_slot_definitions();
            let mut world = GameLogic::new();
            world.game_mode = GameMode::Skirmish;
            prepare(&mut world, "PlyrCivilian", false);
            assert!(
                world.players.is_empty(),
                "empty bootstrap must reach its later map-player admission"
            );
            world.game_mode = GameMode::SinglePlayer;
            prepare(&mut world, "PlyrCivilian", false);
            assert!(
                world.players.is_empty(),
                "campaign bootstrap must reach its later map-player admission"
            );
        },
    );
}

#[test]
fn retry_with_different_map_retires_previous_authored_owners() {
    isolated(
        "retry_with_different_map_retires_previous_authored_owners",
        || {
            let mut world = configured();
            // Seed through the existing dictionary admission boundary so OLD also
            // owns the first map. The control must detect retirement itself.
            let (rows, _) = map_rows("FirstCivilian", true);
            world.apply_host_players_from_side_dicts(&rows, false);
            prepare(&mut world, "FirstCivilian", false);
            let previous = retained(&world, "FirstCivilian");
            world
                .players
                .get_mut(&0)
                .unwrap()
                .set_map_relationship(previous, gamelogic::common::Relationship::Enemies);
            world
                .players
                .get_mut(&0)
                .unwrap()
                .set_team_instance_player_override(
                    "LobbyTeam",
                    previous,
                    gamelogic::common::Relationship::Enemies,
                );
            prepare(&mut world, "SecondCivilian", true);
            assert_eq!(
                world.players.len(),
                5,
                "a new map does not retain prior authored owners"
            );
            assert!(
                !world
                    .players
                    .values()
                    .any(|p| p.map_side.map_player_name == "FirstCivilian"),
                "new map roster must not include FirstCivilian"
            );
            assert_eq!(
                retained(&world, "SecondCivilian"),
                previous,
                "fixture exercises retired identity reuse"
            );

            assert_eq!(
                world.players[&0].map_relationship(previous),
                None,
                "retired map relationship does not attach to reused identity"
            );
            assert_eq!(
                world.players[&0].team_instance_player_override("LobbyTeam", previous),
                None,
                "retired team override does not attach to reused identity"
            );
            assert!(world.players[&0].team_instance_player_relations.is_empty());
            let adapter = get_sides_list();
            let sides = adapter.read().unwrap();
            assert!(
                sides.find_side_info("FirstCivilian").is_none(),
                "old map owner must not be emitted as a lobby slot"
            );
            assert!(sides.find_team_info("teamFirstCivilian").is_none());
            assert!(sides.find_side_info("SecondCivilian").is_some());
            assert_eq!(world.players[&3].map_side.map_player_name, "player3");
        },
    );
}
