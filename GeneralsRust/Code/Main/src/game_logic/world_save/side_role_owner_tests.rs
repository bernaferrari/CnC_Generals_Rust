//! C++ GameLogic1291–1463 / PlayerList97–139: admission keys are not display labels.
use super::*;
use crate::save_load::{SnapshotBuilder, snapshot::decode_bincode_world_snapshot};
use crate::skirmish_config::{
    SkirmishPlayerTemplateSelection, apply_skirmish_config, golden_skirmish_config,
};
use gamelogic::sides_list::SidesList;

fn isolated(name: &str, run: impl FnOnce()) {
    crate::game_logic::game_logic::pose_owner_tests::isolated_at(module_path!(), name, run);
}

fn configured(observer_slot: bool, misleading_display: bool) -> GameLogic {
    crate::skirmish_config::lobby_definition_tests::admit_slot_definitions();
    let mut config = golden_skirmish_config("OwnedRoleAdmission.map");
    config.slots[1].slot_index = 3;
    if misleading_display {
        config.slots[0].player_name = "ReplayObserver".into();
    }
    if observer_slot {
        config.slots[1].player_name.clear();
        config.slots[1].player_template =
            SkirmishPlayerTemplateSelection::exact_template_name("FactionObserver");
    }
    let mut world = GameLogic::new();
    apply_skirmish_config(&mut world, &config).unwrap();
    world
}

fn sides(world: &GameLogic) -> SidesList {
    let mut result = SidesList::new();
    // Authored Civilian rows are map inputs, not occupied lobby slots.
    for player in world
        .players
        .values()
        .filter(|p| p.map_side.role == PlayerSideRole::Authored)
    {
        let mut row = Dict::new();
        row.set_ascii_string(key_player_name(), player.map_side.map_player_name.clone());
        row.set_ascii_string(key_player_faction(), "FactionCivilian");
        result.add_side(&row);
    }
    world.add_host_players_as_sides(&mut result);
    result
}

fn side<'a>(sides: &'a SidesList, name: &str) -> &'a Dict {
    sides
        .find_side_info(name)
        .and_then(|i| sides.get_side_info(i))
        .unwrap_or_else(|| panic!("missing side {name}"))
        .get_dict()
}

fn civilian_rows() -> Vec<Dict> {
    let mut neutral = Dict::new();
    neutral.set_ascii_string(key_player_name(), "");
    neutral.set_ascii_string(key_player_faction(), "FactionCivilian");
    // PlayerList::init(NULL) ignores the authored unnamed row's controller/cash.
    neutral.set_bool(key_player_is_human(), true);
    neutral.set_int(key_player_start_money(), 91_000);
    neutral.set_unicode_string(key_player_display_name(), "False neutral label");
    let mut named = Dict::new();
    named.set_ascii_string(key_player_name(), "PlyrCivilian");
    named.set_ascii_string(key_player_faction(), "FactionCivilian");
    named.set_unicode_string(key_player_display_name(), "ReplayObserver");
    vec![neutral, named]
}

#[test]
fn role_admission_calibration() {
    isolated("role_admission_calibration", || {
        let world = configured(false, false);
        assert_eq!(world.players.len(), 3);
        assert_eq!(world.players[&0].map_side.map_player_name, "player0");
        assert_eq!(world.players[&3].map_side.map_player_name, "player3");
    });
}

#[test]
fn participant_display_cannot_capture_replay_observer_identity() {
    isolated(
        "participant_display_cannot_capture_replay_observer_identity",
        || {
            let world = configured(false, true);
            let observer = world.replay_observer_player_id().unwrap();
            assert_ne!(
                observer, 0,
                "occupied display label is not ReplayObserver admission"
            );
            assert_eq!(world.players.len(), 3);
            assert_eq!(world.players[&0].name, "ReplayObserver");
            assert!(
                world.players[&observer].is_human,
                "CPP ReplayObserver is human"
            );
            let authored = sides(&world);
            assert_eq!(
                side(&authored, "player0").get_unicode_string(key_player_display_name()),
                "ReplayObserver"
            );
            assert_eq!(
                side(&authored, "player0").get_ascii_string(key_player_enemies()),
                "player3"
            );
            assert!(authored.find_side_info("ReplayObserver").is_some());
        },
    );
}

#[test]
fn blank_occupied_observer_remains_an_authored_slot() {
    isolated("blank_occupied_observer_remains_an_authored_slot", || {
        let world = configured(true, false);
        assert!(world.players[&3].is_observer);
        assert_ne!(world.replay_observer_player_id(), Some(3));
        let authored = sides(&world);
        let slot = side(&authored, "player3");
        assert_eq!(
            slot.get_ascii_string(key_player_faction()),
            "FactionObserver"
        );
        assert_eq!(slot.get_unicode_string(key_player_display_name()), "");
        assert_eq!(
            side(&authored, "player0").get_ascii_string(key_player_enemies()),
            "player3"
        );
        assert!(authored.find_team_info("teamplayer3").is_some());
    });
}

#[test]
fn observer_identity_survives_display_edits_and_other_world_reset() {
    isolated(
        "observer_identity_survives_display_edits_and_other_world_reset",
        || {
            let mut first = configured(false, false);
            let observer = first.replay_observer_player_id().unwrap();
            first.players.get_mut(&observer).unwrap().name = "Renamed view".into();
            first.players.get_mut(&0).unwrap().name = "ReplayObserver".into();
            let mut foreign = configured(false, false);
            assert_eq!(foreign.replay_observer_player_id(), Some(observer));
            foreign.reset();
            drop(foreign);
            assert_eq!(
                first.ensure_replay_observer_player(),
                observer,
                "admission, not mutable display, identifies the observer"
            );
            assert_eq!(first.players.len(), 3);
            let clone = first.players[&observer].clone();
            first.add_player(clone);
            assert_eq!(first.ensure_replay_observer_player(), observer);
            first.reset();
            assert_eq!(first.replay_observer_player_id(), None);
        },
    );
}

#[test]
fn replay_side_is_appended_after_every_participant() {
    isolated("replay_side_is_appended_after_every_participant", || {
        let mut world = configured(false, false);
        let mut late = Player::new(90, Team::GLA, "ReplayObserver", false);
        late.map_side.map_player_name = "player7".into();
        world.add_player(late);
        let authored = sides(&world);
        let names: Vec<_> = (0..authored.get_num_sides())
            .map(|i| {
                authored
                    .get_side_info(i)
                    .unwrap()
                    .get_dict()
                    .get_ascii_string(key_player_name())
            })
            .collect();
        assert_eq!(
            names,
            ["player0", "player3", "player7", "ReplayObserver"],
            "CPP emits replay after occupied slots"
        );
        let replay = side(&authored, "ReplayObserver");
        assert!(replay.get_type(key_multiplayer_start_index()).is_some());
        assert_eq!(replay.get_int(key_multiplayer_start_index()), 0);
    });
}

#[test]
fn unnamed_neutral_and_named_civilian_have_distinct_owned_admissions() {
    isolated(
        "unnamed_neutral_and_named_civilian_have_distinct_owned_admissions",
        || {
            let mut world = configured(false, false);
            world.apply_host_players_from_side_dicts(&civilian_rows(), true);
            assert_eq!(
                world.players[&0].map_side.map_player_name, "player0",
                "neutral cannot overwrite the first lobby owner"
            );
            assert_eq!(
                world.players.len(),
                5,
                "neutral and named Civilian each require an owner"
            );
            let neutral = world
                .players
                .values()
                .find(|p| p.map_side.map_player_name.is_empty())
                .unwrap();
            assert_ne!(neutral.id, 0);
            assert_eq!(neutral.resources.supplies, 0);
            assert!(!neutral.is_human);
            assert!(neutral.name.is_empty());
            assert_eq!(neutral.color_rgb, (255, 255, 255));
            let civilian = world
                .players
                .values()
                .find(|p| p.map_side.map_player_name == "PlyrCivilian")
                .unwrap();
            assert_ne!(civilian.id, neutral.id);
            assert_eq!(civilian.name, "ReplayObserver");
            assert_eq!(
                civilian.resources.supplies, 10_000,
                "named Civilian keeps normal starting cash"
            );
            let authored = sides(&world);
            assert_eq!(
                side(&authored, "player0").get_ascii_string(key_player_enemies()),
                "player3",
                "map Civilian is not a lobby alliance slot"
            );
            assert_eq!(
                world.ensure_replay_observer_player(),
                world.replay_observer_player_id().unwrap()
            );
        },
    );
}

#[test]
fn owned_admission_roles_and_names_survive_encoded_save_continuation() {
    isolated(
        "owned_admission_roles_and_names_survive_encoded_save_continuation",
        || {
            let mut source = configured(true, true);
            source.apply_host_players_from_side_dicts(&civilian_rows(), true);
            source.players.get_mut(&0).unwrap().start_position = -1;
            let observer = source.replay_observer_player_id().unwrap();
            source.players.get_mut(&observer).unwrap().name = "Renamed replay".into();
            let builder = SnapshotBuilder::new();
            let snapshot = builder.create_world_snapshot(&source).unwrap();
            let bytes = bincode_legacy::serialize(&snapshot).unwrap();
            let decoded = decode_bincode_world_snapshot(&bytes).unwrap();
            let mut foreign = configured(false, false);
            foreign.reset();
            let mut restored = GameLogic::new();
            builder
                .restore_from_snapshot(&decoded, &mut restored)
                .unwrap();
            assert_eq!(
                restored.players[&0].map_side.map_player_name, "player0",
                "saved canonical admission name"
            );
            assert_eq!(restored.players[&0].start_position, -1);
            assert_eq!(restored.ensure_replay_observer_player(), observer);
            assert_eq!(restored.players.len(), source.players.len());
            let authored = sides(&restored);
            assert_eq!(
                side(&authored, "player3").get_ascii_string(key_player_faction()),
                "FactionObserver"
            );
            assert!(authored.find_side_info("PlyrCivilian").is_some());
            assert_eq!(
                side(&authored, "player0").get_ascii_string(key_player_enemies()),
                "player3"
            );
            let second = builder.create_world_snapshot(&restored).unwrap();
            let mut restarted = GameLogic::new();
            builder
                .restore_from_snapshot(&second, &mut restarted)
                .unwrap();
            assert_eq!(restarted.ensure_replay_observer_player(), observer);
            assert_eq!(restarted.players[&0].map_side.map_player_name, "player0");
        },
    );
}

#[test]
fn missing_admission_capsule_rejects_before_touching_the_receiving_world() {
    isolated(
        "missing_admission_capsule_rejects_before_touching_the_receiving_world",
        || {
            let source = configured(false, true);
            let builder = SnapshotBuilder::new();
            let mut snapshot = builder.create_world_snapshot(&source).unwrap();
            snapshot.lifecycle_tail.clear();
            let mut receiving = configured(false, false);
            receiving.players.get_mut(&0).unwrap().resources.supplies = 42;
            let observer = receiving.replay_observer_player_id();
            assert!(
                builder
                    .restore_from_snapshot(&snapshot, &mut receiving)
                    .is_err(),
                "missing admission metadata must fail before mutation"
            );
            assert_eq!(receiving.players[&0].resources.supplies, 42);
            assert_eq!(receiving.players[&0].name, "Player");
            assert_eq!(receiving.replay_observer_player_id(), observer);
        },
    );
}
