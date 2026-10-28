//! Public save path preserves the driving GameLogic mode in each v2 map record.
use game_engine::common::ini::INILoadType;
use game_engine::common::rts::player_template::PlayerTemplateStore;
use generals_main::game_logic::{GameLogic, GameMode};
use generals_main::save_load::{GameDifficulty, SaveFileManager, SaveFileType, SaveGameInfo};
use generals_main::skirmish_config::{
    SkirmishPlayerTemplateSelection, apply_skirmish_config, golden_skirmish_config,
};
use std::time::{Duration, UNIX_EPOCH};
use tempfile::TempDir;

fn raw_game_state_map_payload(bytes: &[u8]) -> &[u8] {
    let mut offset = 0usize;
    loop {
        let token_len = *bytes.get(offset).expect("named chunk token length") as usize;
        offset += 1;
        let token_end = offset.checked_add(token_len).expect("token offset");
        let token = std::str::from_utf8(
            bytes
                .get(offset..token_end)
                .expect("complete named chunk token"),
        )
        .expect("UTF-8 chunk token");
        offset = token_end;
        if token.eq_ignore_ascii_case("SG_EOF") {
            panic!("GameStateMap missing before EOF");
        }
        let size_end = offset.checked_add(4).expect("chunk size offset");
        let size = i32::from_le_bytes(
            bytes
                .get(offset..size_end)
                .expect("complete named chunk size")
                .try_into()
                .unwrap(),
        );
        assert!(size >= 0, "non-negative named chunk size");
        offset = size_end;
        let end = offset.checked_add(size as usize).expect("chunk end");
        let payload = bytes
            .get(offset..end)
            .expect("complete named chunk payload");
        if token.eq_ignore_ascii_case("CHUNK_GameStateMap") {
            return payload;
        }
        offset = end;
    }
}

fn raw_v2_game_mode(map_payload: &[u8]) -> i32 {
    assert_eq!(map_payload.first(), Some(&2), "GameStateMap v2");
    let mut offset = 1usize;
    for _ in 0..2 {
        let size = *map_payload.get(offset).expect("ASCII string length") as usize;
        offset = offset.checked_add(1 + size).expect("ASCII string end");
    }
    i32::from_le_bytes(
        map_payload
            .get(offset..offset + 4)
            .expect("v2 mode i32 after both map strings")
            .try_into()
            .unwrap(),
    )
}

fn admitted_skirmish_world() -> GameLogic {
    let mut definitions = PlayerTemplateStore::new_owned();
    definitions
        .apply_source(
            "PlayerTemplate FactionAmerica\n Side = America\n StartMoney = 10000\n StartingBuilding = AmericaCommandCenter\nEnd\n",
            INILoadType::Overwrite,
        )
        .expect("parse receiving PlayerTemplate definition");

    let mut config = golden_skirmish_config("SaveModeMap");
    config.slots.truncate(1);
    config.slots[0].player_template = SkirmishPlayerTemplateSelection::Exact {
        template_name: "FactionAmerica".to_owned(),
        template_index: 0,
    };

    let mut logic = GameLogic::new();
    logic
        .admit_player_template_definitions(definitions)
        .expect("admit receiving PlayerTemplate definitions");
    apply_skirmish_config(&mut logic, &config).expect("apply resolved Skirmish start config");
    logic
}

fn save_info(filename: &str) -> SaveGameInfo {
    SaveGameInfo {
        filename: filename.to_owned(),
        display_name: filename.to_owned(),
        description: "save-mode ownership regression".to_owned(),
        map_name: String::new(),
        pristine_map_name: None,
        campaign_side: None,
        mission_number: None,
        save_date: UNIX_EPOCH,
        game_version: env!("CARGO_PKG_VERSION").to_owned(),
        play_time: Duration::ZERO,
        difficulty: GameDifficulty::Medium,
        save_type: SaveFileType::Normal,
    }
}

#[test]
fn interleaved_public_saves_keep_each_receiving_world_mode_in_its_archive() {
    let save_dir = TempDir::new().expect("temporary save directory");
    let mut saves = SaveFileManager::with_save_directory(save_dir.path());
    saves.init().expect("initialize save manager");

    // A bare start_new_game(Skirmish) is intentionally insufficient: native
    // saves require the real resolved startup admission that this public path
    // retains in the receiving GameLogic.
    let skirmish = admitted_skirmish_world();
    let mut campaign = GameLogic::new();
    campaign.start_new_game(GameMode::SinglePlayer);

    // Interleave owners and revisit the first owner after the second save.
    saves
        .save_game("skirmish-a", &skirmish, &save_info("skirmish-a"))
        .expect("save first world");
    // A nested name targets a temp path whose parent does not exist. The
    // expected I/O failure must not leave B's mode staged for a later save.
    assert!(
        saves
            .save_game(
                "missing-directory/campaign-b-failed",
                &campaign,
                &save_info("campaign-b-failed"),
            )
            .is_err()
    );
    assert!(
        !saves
            .get_save_path("missing-directory/campaign-b-failed")
            .exists()
    );
    saves
        .save_game("campaign-b", &campaign, &save_info("campaign-b"))
        .expect("save second world after the failed attempt");
    saves
        .save_game(
            "skirmish-a-again",
            &skirmish,
            &save_info("skirmish-a-again"),
        )
        .expect("save first world again");

    let saved_mode = |name: &str| {
        saves
            .load_game_snapshot(name)
            .expect("decode just-written archive")
            .saved_game_mode
            .expect("GameStateMap v2 carries a mode")
    };
    for (name, expected) in [
        ("skirmish-a", 2),
        ("campaign-b", 0),
        ("skirmish-a-again", 2),
    ] {
        assert_eq!(saved_mode(name), expected, "decoded mode for {name}");
        let archive = std::fs::read(saves.get_save_path(name)).expect("read public save archive");
        assert_eq!(
            raw_v2_game_mode(raw_game_state_map_payload(&archive)),
            expected,
            "independent raw v2 mode bytes for {name}"
        );
    }
}
