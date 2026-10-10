//! CPP GameLogic1291–1425: lobby slot identity survives holes and map preparation.
use super::*;
use crate::skirmish_config::{apply_skirmish_config, golden_skirmish_config};
use gamelogic::sides_list::SidesList;

fn isolated(name: &str, run: impl FnOnce()) {
    // Existing native bootstrap is process-scoped. A fixture never clears a
    // foreign registry or takes a test serialization mutex. Its two worlds
    // still run in the same process; broader bootstrap ownership stays open.
    crate::game_logic::game_logic::pose_owner_tests::isolated_at(module_path!(), name, run);
}

fn configured(last_slot: usize) -> GameLogic {
    crate::skirmish_config::lobby_definition_tests::admit_slot_definitions();
    let mut config = golden_skirmish_config("OwnedSlotAdmission.map");
    config.slots[1].slot_index = last_slot;
    let mut closed = config.slots[1].clone();
    closed.slot_index = 1;
    closed.is_active = false;
    config.slots.insert(1, closed);
    let mut world = GameLogic::new();
    apply_skirmish_config(&mut world, &config).unwrap();
    world
}

fn prepared(world: &GameLogic) -> SidesList {
    let mut sides = SidesList::new();
    // Supply authored Civilian directly. This avoids an unrelated global
    // skirmish script-file fallback while preserving prepare's side ordering.
    let mut civilian = Dict::new();
    civilian.set_ascii_string(key_player_name(), "");
    civilian.set_ascii_string(key_player_faction(), "FactionCivilian");
    sides.add_side(&civilian);
    let mut named_civilian = Dict::new();
    named_civilian.set_ascii_string(key_player_name(), "PlyrCivilian");
    named_civilian.set_ascii_string(key_player_faction(), "FactionCivilian");
    sides.add_side(&named_civilian);
    let mut faction = Dict::new();
    faction.set_ascii_string(key_player_name(), "PlyrTemplate");
    faction.set_ascii_string(key_player_faction(), "FactionGLA");
    sides.add_side(&faction);
    let mut source = ScriptList::new();
    source.append_script(Box::new(gamelogic::scripting::core::Script::new()));
    sides
        .get_side_info_mut(2)
        .unwrap()
        .set_script_list(Some(Box::new(source)));
    sides.prepare_for_mp_or_skirmish();
    assert!(sides.find_side_info("PlyrTemplate").is_none());
    assert!(sides.find_skirmish_side_info("PlyrTemplate").is_some());
    world.add_host_players_as_sides(&mut sides);
    sides.validate_sides();
    sides
}

fn side<'a>(sides: &'a SidesList, name: &str) -> &'a Dict {
    sides
        .find_side_info(name)
        .and_then(|index| sides.get_side_info(index))
        .unwrap_or_else(|| panic!("missing authored side {name}"))
        .get_dict()
}

#[test]
fn lobby_slot_admission_calibration() {
    isolated("lobby_slot_admission_calibration", || {
        let world = configured(3);
        assert_eq!(world.players.len(), 3);
        assert!(
            world
                .players
                .values()
                .any(|player| player.name == "ReplayObserver")
        );
        assert!(world.players.contains_key(&0));
        assert!(world.players.contains_key(&3));
        assert!(!world.players.contains_key(&1));
        assert_eq!(world.players[&0].name, "Player");
        assert_eq!(world.players[&3].name, "GLA AI");
        assert_eq!(world.players[&3].start_position, 1);
    });
}

#[test]
fn typed_lobby_admission_retains_exact_script_names_without_display_rename() {
    isolated(
        "typed_lobby_admission_retains_exact_script_names_without_display_rename",
        || {
            let world = configured(3);
            assert_eq!(world.players[&0].map_side.map_player_name, "player0");
            assert_eq!(world.players[&3].map_side.map_player_name, "player3");
            let clone = world.players[&3].clone();
            assert_eq!(clone.map_side.map_player_name, "player3");
            assert_eq!(clone.name, "GLA AI");
            assert_eq!(clone.start_position, 1);
        },
    );
}

#[test]
fn sparse_slots_author_matching_sides_and_singleton_teams() {
    isolated(
        "sparse_slots_author_matching_sides_and_singleton_teams",
        || {
            let world = configured(3);
            let sides = prepared(&world);
            assert_eq!(
                side(&sides, "player0").get_ascii_string(key_player_name()),
                "player0"
            );
            assert_eq!(
                side(&sides, "player3").get_ascii_string(key_player_name()),
                "player3"
            );
            assert!(sides.find_side_info("player1").is_none());
            let team = sides
                .find_team_info("teamplayer3")
                .and_then(|index| sides.get_team_info(index))
                .expect("singleton team for original slot");
            assert_eq!(
                team.get_dict().get_ascii_string(key_team_owner()),
                "player3"
            );
            assert_eq!(
                side(&sides, "PlyrCivilian").get_ascii_string(key_player_name()),
                "PlyrCivilian"
            );
            assert!(
                side(&sides, "")
                    .get_ascii_string(key_player_name())
                    .is_empty()
            );
        },
    );
}

#[test]
fn sparse_slot_alliance_tokens_use_authored_slot_names() {
    isolated(
        "sparse_slot_alliance_tokens_use_authored_slot_names",
        || {
            let mut world = configured(3);
            let sides = prepared(&world);
            assert_eq!(
                side(&sides, "player0").get_ascii_string(key_player_enemies()),
                "player3"
            );
            assert_eq!(
                side(&sides, "player3").get_ascii_string(key_player_enemies()),
                "player0"
            );
            world.players.get_mut(&3).unwrap().alliance_team = 0;
            let allies = prepared(&world);
            assert_eq!(
                side(&allies, "player0").get_ascii_string(key_player_allies()),
                "player3"
            );
            assert!(
                side(&allies, "player0")
                    .get_ascii_string(key_player_enemies())
                    .is_empty()
            );
        },
    );
}

#[test]
fn side_authoring_preserves_unresolved_start_position_and_observer_name() {
    isolated(
        "side_authoring_preserves_unresolved_start_position_and_observer_name",
        || {
            let mut world = configured(3);
            world.players.get_mut(&0).unwrap().start_position = -1;
            let mut neutral = Player::new(77, Team::Neutral, "", false);
            let mut neutral_dict = Dict::new();
            neutral_dict.set_ascii_string(key_player_name(), "");
            neutral.apply_map_side_dict(&neutral_dict, false);
            world.add_player(neutral);
            world.add_player(Player::new(90, Team::Neutral, "ReplayObserver", true));
            let sides = prepared(&world);
            assert_eq!(
                side(&sides, "player0").get_int(key_multiplayer_start_index()),
                -1
            );
            assert!(sides.find_side_info("player77").is_none());
            assert!(sides.find_side_info("player90").is_some());
            assert_eq!(
                side(&sides, "ReplayObserver").get_ascii_string(key_player_faction()),
                "FactionObserver"
            );
            assert!(sides.find_team_info("teamReplayObserver").is_some());
        },
    );
}

#[test]
fn admitted_names_survive_other_world_construction_reset_and_restart() {
    isolated(
        "admitted_names_survive_other_world_construction_reset_and_restart",
        || {
            let mut first = configured(3);
            let mut foreign = configured(7);
            assert_eq!(first.players[&0].id, foreign.players[&0].id);
            assert_eq!(
                side(&prepared(&first), "player3").get_ascii_string(key_player_name()),
                "player3"
            );
            foreign.reset();
            drop(foreign);
            drop(GameLogic::new());
            assert_eq!(first.players[&3].map_side.map_player_name, "player3");
            first.reset();
            assert!(first.players.is_empty());
            let mut config = golden_skirmish_config("OwnedSlotAdmission.map");
            config.slots[1].slot_index = 3;
            apply_skirmish_config(&mut first, &config).unwrap();
            assert_eq!(first.players[&3].map_side.map_player_name, "player3");
        },
    );
}
