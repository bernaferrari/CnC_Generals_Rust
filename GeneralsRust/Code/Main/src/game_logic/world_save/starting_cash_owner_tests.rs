//! C++ Player.cpp423–435 / 993–1009 through real prepared-side admission.
use super::*;
use crate::save_load::{SnapshotBuilder, snapshot::decode_bincode_world_snapshot};
use crate::skirmish_config::{apply_skirmish_config, golden_skirmish_config};

fn isolated(name: &str, run: impl FnOnce()) {
    crate::game_logic::game_logic::pose_owner_tests::isolated_at(module_path!(), name, run);
}

fn configured(cash: i32) -> GameLogic {
    crate::skirmish_config::lobby_definition_tests::admit_slot_definitions();
    let mut config = golden_skirmish_config("StartingCashAdmission.map");
    config.rules.starting_cash = cash;
    config.slots[1].slot_index = 3;
    let mut world = GameLogic::new();
    apply_skirmish_config(&mut world, &config).unwrap();
    world
}

fn rows(name: &str, money: Option<i32>) -> Vec<Dict> {
    let mut civilian = Dict::new();
    civilian.set_ascii_string(key_player_name(), name);
    civilian.set_ascii_string(key_player_faction(), "FactionCivilian");
    civilian.set_unicode_string(key_player_display_name(), "Authored civilian");
    if let Some(money) = money {
        civilian.set_int(key_player_start_money(), money);
    }
    let mut neutral = Dict::new();
    neutral.set_ascii_string(key_player_name(), "");
    neutral.set_ascii_string(key_player_faction(), "FactionCivilian");
    neutral.set_int(key_player_start_money(), 91_000);
    vec![neutral, civilian]
}

fn prepare(world: &mut GameLogic, rows: &[Dict]) -> u32 {
    world.sync_legacy_sides_list_from_dicts(rows, &[], &[], &[]);
    world
        .players
        .values()
        .find(|p| p.map_side.map_player_name == rows[1].get_ascii_string(key_player_name()))
        .expect("retained civilian")
        .id
}

#[test]
fn default_cash_and_reserved_neutral_calibrate_prepared_admission() {
    isolated(
        "default_cash_and_reserved_neutral_calibrate_prepared_admission",
        || {
            let mut world = configured(10_000);
            let id = prepare(&mut world, &rows("Civilian", None));
            assert_eq!(world.players[&id].resources.supplies, 10_000);
            assert_eq!(world.players[&0].resources.supplies, 10_000);
            assert_eq!(
                world
                    .players
                    .values()
                    .find(|p| p.is_reserved_neutral())
                    .unwrap()
                    .resources
                    .supplies,
                0
            );
        },
    );
}

#[cfg(feature = "game_client")]
#[test]
fn game_info_conversion_preserves_zero_and_unsigned_cash() {
    isolated(
        "game_info_conversion_preserves_zero_and_unsigned_cash",
        || {
            crate::skirmish_config::lobby_definition_tests::admit_slot_definitions();
            for cash in [0, u32::MAX] {
                {
                    let mut setup = game_client::gui::get_skirmish_setup();
                    setup.set_selected_map(String::new());
                    let info = setup.game_info_mut().game_info_mut();
                    info.reset();
                    info.set_map("StartingCashAdmission.map".into());
                    info.set_starting_cash(game_client::Money::new(cash));
                    let slot = info.get_slot_mut(0).unwrap();
                    slot.set_state(game_client::SlotState::Player, "Human".into(), 1);
                    slot.set_player_template(-1);
                }
                let config =
                    crate::skirmish_config::config_from_client_skirmish_setup(None).unwrap();
                assert_eq!(
                    config.rules.starting_cash as u32, cash,
                    "GameInfo cash is present even when zero"
                );
            }
        },
    );
}

#[test]
fn authored_deposit_emits_money_audio_once_and_neutral_emits_none() {
    isolated(
        "authored_deposit_emits_money_audio_once_and_neutral_emits_none",
        || {
            use crate::game_logic::host_economy_log::{self, HostMoneyAudio, HostMoneyAudioEvent};
            let mut world = configured(12_500);
            host_economy_log::take_money_audio();
            let id = prepare(&mut world, &rows("Civilian", Some(700)));
            assert_eq!(
                host_economy_log::take_money_audio(),
                vec![HostMoneyAudioEvent {
                    player_id: id,
                    kind: HostMoneyAudio::Deposit
                }]
            );
            prepare(&mut world, &rows("Civilian", Some(0)));
            assert!(host_economy_log::take_money_audio().is_empty());
        },
    );
}

#[test]
fn custom_cash_reaches_civilian_and_replay_without_reading_a_wallet() {
    isolated(
        "custom_cash_reaches_civilian_and_replay_without_reading_a_wallet",
        || {
            let mut world = configured(12_500);
            world.players.get_mut(&0).unwrap().resources.supplies = 9;
            let id = prepare(&mut world, &rows("Civilian", None));
            assert_eq!(
                world.players[&id].resources.supplies, 12_500,
                "configured admission cash"
            );
            let replay = world.replay_observer_player_id().unwrap();
            assert_eq!(world.players[&replay].resources.supplies, 12_500);
            assert_eq!(
                world.players[&0].resources.supplies, 9,
                "participant wallet is not an input"
            );
        },
    );
}

#[test]
fn chunky_map_load_uses_session_cash_for_retained_civilian() {
    isolated(
        "chunky_map_load_uses_session_cash_for_retained_civilian",
        || {
            let dir = tempfile::tempdir().unwrap();
            let path = dir.path().join("StartingCashAdmission.map");
            let mut out = game_engine::common::system::DataChunkOutput::new();
            out.open_data_chunk("HeightMapData", 3);
            for value in [21, 21, 1, 441] {
                out.write_int(value);
            }
            for _ in 0..441 {
                out.write_byte(0);
            }
            out.close_data_chunk();
            out.open_data_chunk("ObjectsList", 3);
            out.open_data_chunk("Object", 3);
            for value in [10.0, 10.0, 0.0, 0.0] {
                out.write_real(value);
            }
            out.write_int(0);
            out.write_ascii_string("CashAdmissionMarker");
            out.close_data_chunk();
            out.close_data_chunk();
            out.open_data_chunk("SidesList", 2);
            out.write_int(2);
            for dict in rows("Civilian", Some(700)) {
                out.write_dict(&dict);
                out.write_int(0);
            }
            out.write_int(0);
            out.close_data_chunk();
            std::fs::write(&path, out.into_ckmp_bytes()).unwrap();
            let mut world = configured(12_500);
            world.players.get_mut(&0).unwrap().resources.supplies = 9;
            let mut marker = crate::game_logic::ThingTemplate::new("CashAdmissionMarker");
            marker.set_health(10.0);
            world.templates.insert("CashAdmissionMarker".into(), marker);
            assert!(
                world.load_map(path.to_str().unwrap()),
                "actual CKMP map must load"
            );
            assert!(world.terrain.is_some());
            let civilian = world
                .players
                .values()
                .find(|p| p.map_side.map_player_name == "Civilian")
                .unwrap();
            assert_eq!(
                civilian.resources.supplies, 13_200,
                "loaded map deposits onto session cash"
            );
            assert_eq!(world.players[&0].resources.supplies, 9);
            assert_eq!(
                world
                    .players
                    .values()
                    .find(|p| p.is_reserved_neutral())
                    .unwrap()
                    .resources
                    .supplies,
                0
            );
        },
    );
}

#[test]
fn present_zero_cash_is_not_the_default_cash() {
    isolated("present_zero_cash_is_not_the_default_cash", || {
        let mut world = configured(0);
        let id = prepare(&mut world, &rows("Civilian", None));
        assert_eq!(
            world.players[&id].resources.supplies, 0,
            "present zero GameInfo cash"
        );
        assert_eq!(world.players[&0].resources.supplies, 0);
        assert_eq!(
            world.players[&world.replay_observer_player_id().unwrap()]
                .resources
                .supplies,
            0
        );
    });
}

#[test]
fn unsigned_cash_and_authored_deposit_wrap_like_cpp_money() {
    isolated(
        "unsigned_cash_and_authored_deposit_wrap_like_cpp_money",
        || {
            let mut world = configured(-1);
            assert_eq!(
                world.players[&0].resources.supplies,
                u32::MAX,
                "GameInfo unsigned bit pattern"
            );
            let id = prepare(&mut world, &rows("Civilian", Some(2)));
            assert_eq!(
                world.players[&id].resources.supplies, 1,
                "unsigned deposit wraps"
            );
        },
    );
}

#[test]
fn authored_negative_money_is_an_unsigned_deposit() {
    isolated("authored_negative_money_is_an_unsigned_deposit", || {
        let mut world = configured(12_500);
        let id = prepare(&mut world, &rows("Civilian", Some(-1)));
        assert_eq!(
            world.players[&id].resources.supplies, 12_499,
            "Int converts before addition"
        );
    });
}

#[test]
fn template_money_precedes_authored_money_and_color() {
    isolated("template_money_precedes_authored_money_and_color", || {
        let mut world = configured(12_500);
        let mut ini = game_engine::common::ini::INI::new();
        ini.with_inline_source("PlayerTemplate FactionCivilian\n Side = Civilian\n BaseSide = Civilian\n StartMoney = 4500\n PreferredColor = R:1 G:2 B:3\nEnd\n", |ini| ini.parse_current_file()).unwrap();
        let mut input = rows("Civilian", Some(2_500));
        input[1].set_int(key_player_color(), 0x00aa_3311);
        input[1].set_int(key_player_night_color(), 0x0011_3355);
        let id = prepare(&mut world, &input);
        assert_eq!(
            world.players[&id].resources.supplies, 7_000,
            "template assignment then authored deposit"
        );
        assert_eq!(world.players[&id].color_rgb, (0xaa, 0x33, 0x11));
        assert_eq!(world.players[&id].color_night_rgb, (0x11, 0x33, 0x55));
    });
}

#[test]
fn same_name_map_initialization_does_not_reuse_spent_cash_or_overrides() {
    isolated(
        "same_name_map_initialization_does_not_reuse_spent_cash_or_overrides",
        || {
            let mut world = configured(12_500);
            let input = rows("Civilian", Some(700));
            let id = prepare(&mut world, &input);
            let neutral = world
                .players
                .values()
                .find(|p| p.is_reserved_neutral())
                .unwrap()
                .id;
            world.players.get_mut(&neutral).unwrap().resources.supplies = 123;
            world.players.get_mut(&id).unwrap().resources.supplies = 2;
            world.players.get_mut(&id).unwrap().color_rgb = (9, 8, 7);
            world
                .players
                .get_mut(&id)
                .unwrap()
                .set_map_relationship(0, gamelogic::common::Relationship::Enemies);
            assert_eq!(prepare(&mut world, &input), id, "identity stays stable");
            assert_eq!(
                world.players[&id].resources.supplies, 13_200,
                "new map init uses admission cash again"
            );
            assert_ne!(world.players[&id].color_rgb, (9, 8, 7));
            assert_eq!(world.players[&id].map_relationship(0), None);
            assert_eq!(
                world.players[&neutral].resources.supplies, 0,
                "neutral init(NULL) resets at new-map admission"
            );
        },
    );
}

#[test]
fn relationship_admission_does_not_deposit_authored_money_twice() {
    isolated(
        "relationship_admission_does_not_deposit_authored_money_twice",
        || {
            let mut world = configured(12_500);
            let input = rows("Civilian", Some(700));
            world.apply_host_players_from_side_dicts(&input, true);
            let id = world
                .players
                .values()
                .find(|p| p.map_side.map_player_name == "Civilian")
                .unwrap()
                .id;
            assert_eq!(
                world.players[&id].resources.supplies, 13_200,
                "one deposit in initFromDict"
            );
        },
    );
}

#[test]
fn two_worlds_with_same_ids_keep_cash_across_foreign_reset_and_clone() {
    isolated(
        "two_worlds_with_same_ids_keep_cash_across_foreign_reset_and_clone",
        || {
            let mut first = configured(12_500);
            let a = prepare(&mut first, &rows("Civilian", None));
            let mut second = configured(0);
            let b = prepare(&mut second, &rows("Civilian", None));
            assert_eq!(a, b);
            second.reset();
            drop(second);
            first.players.get_mut(&a).unwrap().resources.supplies = 7;
            let query = first.players[&a].clone();
            assert_eq!(
                query.resources.supplies, 7,
                "query clone must not reinitialize"
            );
            first.add_player(query);
            let id = prepare(&mut first, &rows("NextCivilian", None));
            assert_eq!(
                first.players[&id].resources.supplies, 12_500,
                "foreign reset cannot change initial cash"
            );
            first.reset();
            first.game_mode = GameMode::Skirmish;
            first.add_player(Player::new(0, Team::USA, "Bootstrap", true));
            let id = prepare(&mut first, &rows("AfterReset", None));
            assert_eq!(
                first.players[&id].resources.supplies, 10_000,
                "own reset clears config cash"
            );
        },
    );
}

#[test]
fn encoded_restore_keeps_spent_balances_and_initial_cash_separately() {
    isolated(
        "encoded_restore_keeps_spent_balances_and_initial_cash_separately",
        || {
            let mut source = configured(12_500);
            let id = prepare(&mut source, &rows("Civilian", Some(700)));
            source.players.get_mut(&id).unwrap().resources.supplies = 2;
            source.players.get_mut(&0).unwrap().resources.supplies = 3;
            let builder = SnapshotBuilder::new();
            let snapshot = builder.create_world_snapshot(&source).unwrap();
            let bytes = bincode_legacy::serialize(&snapshot).unwrap();
            let decoded = decode_bincode_world_snapshot(&bytes).unwrap();
            let mut restored = configured(0);
            builder
                .restore_from_snapshot(&decoded, &mut restored)
                .unwrap();
            assert_eq!(
                restored.players[&id].resources.supplies, 2,
                "restore does not rerun deposits"
            );
            assert_eq!(restored.players[&0].resources.supplies, 3);
            let next = prepare(&mut restored, &rows("NextCivilian", None));
            assert_eq!(
                restored.players[&next].resources.supplies, 12_500,
                "saved admission input survives spent wallets"
            );
        },
    );
}

#[cfg(not(target_arch = "wasm32"))]
#[test]
#[ignore = "build original C++ cash component with scripts/run_cpp_player_cash.py"]
fn prepared_cash_matches_executed_original_cpp_component() {
    // The explicit parity runner launches this exact ignored test in its own
    // process with the freshly executed C++ output. No fixture mutex is needed.
    let path = std::env::var("GENERALS_CPP_PLAYER_CASH_ORACLE")
        .expect("executed original C++ oracle path is required");
    let cases = std::fs::read_to_string(path).unwrap();
    let mut world = configured(10_000);
    let mut count = 0;
    for line in cases.lines() {
        let fields: Vec<_> = line.split(',').collect();
        assert_eq!(fields.len(), 7, "original component record");
        let cash: u32 = fields[0].parse().unwrap();
        let template_cash: u32 = fields[1].parse().unwrap();
        let deposit: i32 = fields[2].parse().unwrap();
        let expected: u32 = fields[3].parse().unwrap();
        let mut config = golden_skirmish_config("StartingCashAdmission.map");
        config.rules.starting_cash = cash as i32;
        apply_skirmish_config(&mut world, &config).unwrap();
        let mut ini = game_engine::common::ini::INI::new();
        ini.with_inline_source(&format!("PlayerTemplate FactionCivilian\n Side = Civilian\n BaseSide = Civilian\n StartMoney = {template_cash}\nEnd\n"), |ini| ini.parse_current_file()).unwrap();
        let id = prepare(&mut world, &rows("Civilian", Some(deposit)));
        assert_eq!(
            world.players[&id].resources.supplies, expected,
            "executed C++ cash input {line}"
        );
        count += 1;
    }
    assert_eq!(count, 6, "all original execution cases must be compared");
    // The remaining columns observe CPP audio/Academy adapters. Owner/ordinal
    // delivery and income timing have their own open Beads, not parity credit.
}
