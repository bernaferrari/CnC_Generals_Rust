//! GameStateMap.cpp324–329 selects the receiving mode before map creation.
use super::*;
use crate::game_logic::{GameLogic, GameMode};
use crate::save_load::{GameDifficulty, SaveFileManager, SaveFileType};

fn info(slot: &str) -> SaveGameInfo {
    SaveGameInfo {
        pristine_map_name: None,
        filename: slot.into(),
        display_name: slot.into(),
        description: "archive-owned mode".into(),
        map_name: "ModeOwner.map".into(),
        campaign_side: None,
        mission_number: None,
        save_date: std::time::UNIX_EPOCH,
        game_version: env!("CARGO_PKG_VERSION").into(),
        play_time: std::time::Duration::ZERO,
        difficulty: GameDifficulty::Medium,
        save_type: SaveFileType::Normal,
    }
}

// Mutate only this original named chunk's version/mode. The remainder is the
// actual production writer output, including GameLogic and complete counters.
fn map_mode_variant(bytes: &[u8], mode: Option<Option<i32>>) -> Vec<u8> {
    let mut out = Vec::new();
    let mut pos = 0;
    while pos < bytes.len() {
        let length = bytes[pos] as usize;
        pos += 1;
        let token = &bytes[pos..pos + length];
        pos += length;
        if token == b"SG_EOF" {
            out.push(length as u8);
            out.extend_from_slice(token);
            assert_eq!(pos, bytes.len());
            break;
        }
        let size = i32::from_le_bytes(bytes[pos..pos + 4].try_into().unwrap());
        pos += 4;
        let end = pos + usize::try_from(size).unwrap();
        let mut payload = bytes[pos..end].to_vec();
        pos = end;
        if token == b"CHUNK_GameStateMap" {
            let Some(mode) = mode else { continue };
            assert_eq!(payload[0], 2);
            let mut offset = 1;
            for _ in 0..2 {
                offset += 1 + payload[offset] as usize;
            }
            match mode {
                Some(value) => payload[offset..offset + 4].copy_from_slice(&value.to_le_bytes()),
                None => {
                    payload[0] = 1;
                    payload.drain(offset..offset + 4);
                }
            }
        }
        out.push(length as u8);
        out.extend_from_slice(token);
        out.extend_from_slice(&i32::try_from(payload.len()).unwrap().to_le_bytes());
        out.extend_from_slice(&payload);
    }
    out
}

fn selected(decoded: &crate::save_load::DecodedGameSave, active: GameMode) -> GameMode {
    CnCGameEngine::offline_restore_mode_for_save(
        active,
        &decoded.save_info,
        decoded.saved_game_mode,
    )
    .unwrap()
}

#[test]
fn real_archives_keep_receiving_modes_across_peer_decode_failure_reset_and_drop() {
    crate::game_logic::game_logic::pose_owner_tests::isolated_at(
        module_path!(),
        "real_archives_keep_receiving_modes_across_peer_decode_failure_reset_and_drop",
        || {
            crate::skirmish_config::lobby_definition_tests::admit_slot_definitions();
            let a_dir = tempfile::tempdir().unwrap();
            let b_dir = tempfile::tempdir().unwrap();
            let mut saves_a = SaveFileManager::with_save_directory(a_dir.path());
            let mut saves_b = SaveFileManager::with_save_directory(b_dir.path());
            saves_a.init().unwrap();
            saves_b.init().unwrap();
            let mut source_a = GameLogic::new();
            source_a.start_new_game_for_restore(GameMode::SinglePlayer);
            saves_a.save_game("a", &source_a, &info("a")).unwrap();
            let mut source_b = GameLogic::new();
            let config = crate::skirmish_config::golden_skirmish_config("ModeOwner.map");
            crate::skirmish_config::apply_skirmish_config(&mut source_b, &config).unwrap();
            saves_b.save_game("b", &source_b, &info("b")).unwrap();
            let a = saves_a.load_game_snapshot("a").unwrap();
            let b = saves_b.load_game_snapshot("b").unwrap();
            assert_eq!(
                selected(&a, GameMode::Skirmish),
                GameMode::SinglePlayer,
                "archive A must select its saved mode after B decodes"
            );
            assert_eq!(selected(&b, GameMode::SinglePlayer), GameMode::Skirmish);
            std::fs::write(saves_b.get_save_path("broken"), [255, 1, 2]).unwrap();
            assert!(saves_b.load_game_snapshot("broken").is_err());
            assert!(saves_b.load_game_snapshot("missing").is_err());
            source_b.reset();
            drop(source_b);
            drop(saves_b);
            assert_eq!(selected(&a, GameMode::Skirmish), GameMode::SinglePlayer);
            assert_eq!(selected(&b, GameMode::SinglePlayer), GameMode::Skirmish);
            assert_eq!(a.saved_game_mode, Some(0));
            assert_eq!(b.saved_game_mode, Some(2));

            // Without an initial Skirmish capsule the actual writer must still
            // use this receiving world's mode, never a default or prior save.
            source_a.start_new_game_for_restore(GameMode::Shell);
            saves_a
                .save_game("shell", &source_a, &info("shell"))
                .unwrap();
            let shell = saves_a.load_game_snapshot("shell").unwrap();
            assert_eq!(
                shell.saved_game_mode,
                Some(4),
                "writer must use the receiving mode"
            );
            assert_eq!(selected(&a, GameMode::Skirmish), GameMode::SinglePlayer);

            let original = std::fs::read(saves_a.get_save_path("a")).unwrap();
            for (slot, variant) in [("v1", Some(None)), ("omitted", None)] {
                std::fs::write(
                    saves_a.get_save_path(slot),
                    map_mode_variant(&original, variant),
                )
                .unwrap();
                let historical = saves_a.load_game_snapshot(slot).unwrap();
                assert_eq!(historical.saved_game_mode, None);
                assert_eq!(
                    selected(&historical, GameMode::SinglePlayer),
                    GameMode::SinglePlayer
                );
                assert_eq!(
                    selected(&historical, GameMode::Skirmish),
                    GameMode::Skirmish
                );
            }
            assert_eq!(selected(&a, GameMode::Skirmish), GameMode::SinglePlayer);
        },
    );
}

#[test]
fn original_mode_fallbacks_and_deferred_modes_are_explicit_and_do_not_consume_owner() {
    let info = info("fallback");
    for raw in [None, Some(-77)] {
        assert_eq!(
            CnCGameEngine::offline_restore_mode_for_save(GameMode::SinglePlayer, &info, raw)
                .unwrap(),
            GameMode::SinglePlayer
        );
    }
    for raw in [4, 6] {
        assert_eq!(
            CnCGameEngine::offline_restore_mode_for_save(GameMode::SinglePlayer, &info, Some(raw))
                .unwrap(),
            GameMode::Skirmish
        );
    }
    for raw in [1, 3, 5] {
        assert!(
            CnCGameEngine::offline_restore_mode_for_save(GameMode::SinglePlayer, &info, Some(raw))
                .unwrap_err()
                .contains("deferred")
        );
    }
    assert!(
        CnCGameEngine::offline_restore_mode_for_save(GameMode::Replay, &info, Some(0))
            .unwrap_err()
            .contains("deferred")
    );
}
