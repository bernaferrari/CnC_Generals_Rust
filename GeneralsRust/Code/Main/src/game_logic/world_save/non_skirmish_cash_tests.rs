//! Original Player.cpp421–435 cash choice through real non-skirmish map admission.
//! These financial witnesses do not establish audio, Academy or full-match parity.
use super::*;
use crate::save_load::{SnapshotBuilder, snapshot::decode_bincode_world_snapshot};
use std::path::{Path, PathBuf};

fn isolated(name: &str, run: impl FnOnce()) {
    crate::game_logic::game_logic::pose_owner_tests::isolated_at(module_path!(), name, run);
}

fn world(game_info: Option<u32>, definition_base: u32) -> GameLogic {
    crate::skirmish_config::lobby_definition_tests::admit_slot_definitions();
    let mut world = GameLogic::new();
    world.start_new_game(GameMode::SinglePlayer);
    world.admit_new_game_starting_cash(game_info, definition_base);
    world
}

fn template_cash(cash: u32) {
    let mut ini = game_engine::common::ini::INI::new();
    ini.with_inline_source(
        &format!(
            "PlayerTemplate FactionCivilian\n Side = Civilian\n BaseSide = Civilian\n StartMoney = {cash}\nEnd\n"
        ),
        |ini| ini.parse_current_file(),
    )
    .unwrap();
}

fn map(dir: &Path, deposit: i32, map_ini: Option<&str>, solo_ini: Option<&str>) -> PathBuf {
    std::fs::create_dir_all(dir).unwrap();
    let path = dir.join("CashAdmission.map");
    let mut out = game_engine::common::system::DataChunkOutput::new();
    out.open_data_chunk("HeightMapData", 3);
    for value in [21, 21, 1, 441] {
        out.write_int(value);
    }
    for _ in 0..441 {
        out.write_byte(0);
    }
    out.close_data_chunk();
    out.open_data_chunk("SidesList", 2);
    out.write_int(2);
    let mut neutral = Dict::new();
    neutral.set_ascii_string(key_player_name(), "");
    neutral.set_ascii_string(key_player_faction(), "FactionCivilian");
    neutral.set_int(key_player_start_money(), 91_000);
    let mut civilian = Dict::new();
    civilian.set_ascii_string(key_player_name(), "CashCivilian");
    civilian.set_ascii_string(key_player_faction(), "FactionCivilian");
    civilian.set_unicode_string(key_player_display_name(), "Authored cash civilian");
    civilian.set_int(key_player_start_money(), deposit);
    for dict in [neutral, civilian] {
        out.write_dict(&dict);
        out.write_int(0);
    }
    out.write_int(0);
    out.close_data_chunk();
    std::fs::write(&path, out.into_ckmp_bytes()).unwrap();
    for (name, contents) in [("Map.ini", map_ini), ("Solo.ini", solo_ini)] {
        if let Some(contents) = contents {
            std::fs::write(dir.join(name), contents).unwrap();
        }
    }
    path
}

fn load(world: &mut GameLogic, path: &Path) -> u32 {
    assert!(
        world.load_map(path.to_str().unwrap()),
        "real no-object CKMP admission"
    );
    assert!(world.terrain.is_some(), "actual HeightMapData loaded");
    let civilian = world
        .players
        .values()
        .find(|p| p.map_side.map_player_name == "CashCivilian")
        .expect("named map civilian admitted without ObjectsList");
    let neutral = world
        .players
        .values()
        .find(|p| p.is_reserved_neutral())
        .expect("reserved neutral admitted");
    assert_eq!(neutral.resources.supplies, 0, "C++ neutral init(NULL)");
    civilian.id
}

#[test]
fn absent_game_info_uses_base_then_map_then_solo_before_authored_deposit() {
    isolated(
        "absent_game_info_uses_base_then_map_then_solo_before_authored_deposit",
        || {
            let dir = tempfile::tempdir().unwrap();
            let mut world = world(None, 17_321);
            template_cash(0);
            for (name, map_ini, solo_ini, expected) in [
                ("base", None, None, 18_021),
                (
                    "map",
                    Some("GameData\n DefaultStartingCash = 23000\nEnd\n"),
                    None,
                    23_700,
                ),
                (
                    "solo",
                    Some("GameData\n DefaultStartingCash = 23000\nEnd\n"),
                    Some("GameData\n DefaultStartingCash = 31000\nEnd\n"),
                    31_700,
                ),
            ] {
                let path = map(&dir.path().join(name), 700, map_ini, solo_ini);
                let id = load(&mut world, &path);
                assert_eq!(world.players[&id].resources.supplies, expected, "{name}");
                assert_eq!(world.skirmish_rules.starting_cash_default, Some(17_321));
                assert_eq!(world.skirmish_rules.starting_cash, expected - 700);
            }
        },
    );
}

#[test]
fn present_zero_and_unsigned_game_info_ignore_map_definition_cash() {
    isolated(
        "present_zero_and_unsigned_game_info_ignore_map_definition_cash",
        || {
            let dir = tempfile::tempdir().unwrap();
            let path = map(
                dir.path(),
                2,
                Some("GameData\n DefaultStartingCash = 23000\nEnd\n"),
                Some("GameData\n DefaultStartingCash = 31000\nEnd\n"),
            );
            for cash in [0, u32::MAX] {
                let mut world = world(Some(cash), 17_321);
                template_cash(0);
                let id = load(&mut world, &path);
                assert_eq!(world.players[&id].resources.supplies, cash.wrapping_add(2));
                assert_eq!(world.skirmish_rules.starting_cash, cash);
                assert_eq!(world.skirmish_rules.starting_cash_default, None);
            }
        },
    );
}

#[test]
fn nonzero_template_cash_wins_before_authored_deposit_for_both_sources() {
    isolated(
        "nonzero_template_cash_wins_before_authored_deposit_for_both_sources",
        || {
            let dir = tempfile::tempdir().unwrap();
            let path = map(
                dir.path(),
                2_500,
                Some("GameData\n DefaultStartingCash = 23000\nEnd\n"),
                None,
            );
            for game_info in [None, Some(0)] {
                let mut world = world(game_info, 17_321);
                template_cash(4_500);
                let id = load(&mut world, &path);
                assert_eq!(world.players[&id].resources.supplies, 7_000);
            }
        },
    );
}

#[test]
fn failed_companion_parse_preserves_admission_input_and_current_wallet() {
    isolated(
        "failed_companion_parse_preserves_admission_input_and_current_wallet",
        || {
            let dir = tempfile::tempdir().unwrap();
            let mut world = world(None, 17_321);
            template_cash(0);
            let admitted = map(&dir.path().join("admitted"), 700, None, None);
            let id = load(&mut world, &admitted);
            world.players.get_mut(&id).unwrap().resources.supplies = 9;
            let failed = map(
                &dir.path().join("failed"),
                2_500,
                Some("GameData\n DefaultStartingCash = 23000\nEnd\n"),
                Some("GameData\n DefaultStartingCash = invalid\nEnd\n"),
            );
            assert!(!world.load_map(failed.to_str().unwrap()));
            assert_eq!(world.players[&id].resources.supplies, 9);
            assert_eq!(world.skirmish_rules.starting_cash, 17_321);
            assert_eq!(world.skirmish_rules.starting_cash_default, Some(17_321));
        },
    );
}

#[test]
fn retry_without_override_uses_definition_base_not_previous_map_cash() {
    isolated(
        "retry_without_override_uses_definition_base_not_previous_map_cash",
        || {
            let dir = tempfile::tempdir().unwrap();
            let mut world = world(None, 17_321);
            template_cash(0);
            let first = map(
                &dir.path().join("first"),
                700,
                Some("GameData\n DefaultStartingCash = 23000\nEnd\n"),
                None,
            );
            let id = load(&mut world, &first);
            assert_eq!(world.players[&id].resources.supplies, 23_700);
            world.players.get_mut(&id).unwrap().resources.supplies = 4;
            let failed = map(
                &dir.path().join("failed"),
                0,
                Some("GameData\n DefaultStartingCash = invalid\nEnd\n"),
                None,
            );
            assert!(!world.load_map(failed.to_str().unwrap()));
            assert_eq!(world.players[&id].resources.supplies, 4);
            assert_eq!(world.skirmish_rules.starting_cash, 23_000);
            let fallback = map(&dir.path().join("fallback"), 700, None, None);
            assert_eq!(load(&mut world, &fallback), id, "retained named identity");
            assert_eq!(world.players[&id].resources.supplies, 18_021);
            assert_eq!(world.skirmish_rules.starting_cash, 17_321);
        },
    );
}

#[test]
fn same_id_worlds_keep_cash_isolated_across_foreign_reset_and_drop() {
    isolated(
        "same_id_worlds_keep_cash_isolated_across_foreign_reset_and_drop",
        || {
            let dir = tempfile::tempdir().unwrap();
            let path = map(dir.path(), 700, None, None);
            let mut first = world(None, 17_321);
            template_cash(0);
            let a = load(&mut first, &path);
            let mut second = world(Some(0), 93_000);
            template_cash(0);
            let b = load(&mut second, &path);
            assert_eq!(a, b, "same host identity in distinct sessions");
            assert_eq!(second.players[&b].resources.supplies, 700);
            let missing = dir.path().join("missing.map");
            assert!(!second.load_map(missing.to_str().unwrap()));
            assert_eq!(first.skirmish_rules.starting_cash_default, Some(17_321));
            second.reset();
            drop(second);
            first.players.get_mut(&a).unwrap().resources.supplies = 5;
            assert_eq!(first.skirmish_rules.starting_cash_default, Some(17_321));
            assert_eq!(load(&mut first, &path), a);
            assert_eq!(first.players[&a].resources.supplies, 18_021);
            first.start_new_game(GameMode::SinglePlayer);
            first.admit_new_game_starting_cash(None, 21_000);
            let next = load(&mut first, &path);
            assert_eq!(first.players[&next].resources.supplies, 21_700);
        },
    );
}

#[test]
fn encoded_restore_retains_base_resolved_cash_and_spent_wallet_separately() {
    isolated(
        "encoded_restore_retains_base_resolved_cash_and_spent_wallet_separately",
        || {
            let dir = tempfile::tempdir().unwrap();
            let initial = map(
                &dir.path().join("initial"),
                700,
                Some("GameData\n DefaultStartingCash = 23000\nEnd\n"),
                None,
            );
            let mut source = world(None, 17_321);
            template_cash(0);
            let id = load(&mut source, &initial);
            source.players.get_mut(&id).unwrap().resources.supplies = 3;
            let builder = SnapshotBuilder::new();
            let snapshot = builder.create_world_snapshot(&source).unwrap();
            let bytes = bincode_legacy::serialize(&snapshot).unwrap();
            let decoded = decode_bincode_world_snapshot(&bytes).unwrap();
            let mut restored = world(Some(0), 93_000);
            // Production stages the saved map's definitions before restoring
            // its complete roster; source metadata never substitutes for them.
            assert_eq!(load(&mut restored, &initial), id);
            builder
                .restore_from_snapshot(&decoded, &mut restored)
                .unwrap();
            assert_eq!(restored.players[&id].resources.supplies, 3);
            assert_eq!(restored.skirmish_rules.starting_cash, 23_000);
            assert_eq!(restored.skirmish_rules.starting_cash_default, Some(17_321));
            let next = map(&dir.path().join("next"), 700, None, None);
            let next_id = load(&mut restored, &next);
            assert_eq!(restored.players[&next_id].resources.supplies, 18_021);
            assert_eq!(restored.skirmish_rules.starting_cash, 17_321);
        },
    );
}

#[cfg(not(target_arch = "wasm32"))]
#[test]
#[ignore = "requires original C++ cash component execution with scripts/run_cpp_player_cash.py"]
fn no_object_map_cash_matches_executed_original_cpp_component() {
    // The explicit parity runner launches this ignored exact test alone, as in
    // starting_cash_owner_tests; isolated_at's normal child excludes ignores.
    let path = std::env::var("GENERALS_CPP_NON_SKIRMISH_CASH_ORACLE")
        .expect("executed original C++ oracle path required");
    let cases = std::fs::read_to_string(path).unwrap();
    let dir = tempfile::tempdir().unwrap();
    let mut count = 0;
    for (index, line) in cases.lines().enumerate() {
        let fields: Vec<_> = line.split(',').collect();
        assert_eq!(fields.len(), 9, "original financial component record");
        let has_game_info: u32 = fields[0].parse().unwrap();
        assert!(has_game_info <= 1, "explicit GameInfo presence");
        let info_cash: u32 = fields[1].parse().unwrap();
        let definition_base: u32 = fields[2].parse().unwrap();
        let template: u32 = fields[3].parse().unwrap();
        let deposit: i32 = fields[4].parse().unwrap();
        let expected: u32 = fields[5].parse().unwrap();
        let mut world = world((has_game_info == 1).then_some(info_cash), definition_base);
        template_cash(template);
        let path = map(&dir.path().join(index.to_string()), deposit, None, None);
        let id = load(&mut world, &path);
        assert_eq!(
            world.players[&id].resources.supplies, expected,
            "C++ {line}"
        );
        count += 1;
    }
    assert_eq!(count, 8, "all original execution cases compared");
    // Audio/Academy adapter columns are not Main delivery/parity evidence.
}

#[test]
fn reset_and_direct_restart_clear_the_previous_cash_source() {
    isolated(
        "reset_and_direct_restart_clear_the_previous_cash_source",
        || {
            let dir = tempfile::tempdir().unwrap();
            let path = map(
                dir.path(),
                700,
                Some("GameData\nDefaultStartingCash = 14000\nEnd\n"),
                None,
            );
            for info in [None, Some(0)] {
                let mut current = world(info, 17_321);
                current.reset();
                assert_eq!(
                    current.skirmish_rules.starting_cash,
                    Player::DEFAULT_STARTING_MONEY
                );
                assert_eq!(
                    current.skirmish_rules.starting_cash_default,
                    Some(Player::DEFAULT_STARTING_MONEY)
                );
                current.start_new_game(GameMode::SinglePlayer);
                template_cash(0);
                let id = load(&mut current, &path);
                assert_eq!(
                    current.players[&id].resources.supplies, 14_700,
                    "direct new game uses this map, not prior source"
                );
            }
        },
    );
}
