//! Historical named Players blocks migrate only after envelope validation.
use super::*;

fn named_save(snapshot: &WorldSnapshot, players: &[u8]) -> Vec<u8> {
    let mut header = Vec::new();
    {
        let mut cursor = Cursor::new(&mut header);
        let mut xfer = CommonXferSave::new(&mut cursor, SAVE_FILE_VERSION);
        write_cpp_game_state_header(&mut xfer, &fixture_save_info()).unwrap();
    }
    let logic = bincode_legacy::serialize(snapshot).unwrap();
    let mut bytes = Vec::new();
    for (name, payload) in [
        (CHUNK_GAME_STATE, header.as_slice()),
        (CHUNK_GAME_LOGIC, logic.as_slice()),
        (CHUNK_PLAYERS, players),
    ] {
        bytes.push(u8::try_from(name.len()).unwrap());
        bytes.extend_from_slice(name.as_bytes());
        bytes.extend_from_slice(&i32::try_from(payload.len()).unwrap().to_le_bytes());
        bytes.extend_from_slice(payload);
    }
    bytes.push(u8::try_from(SAVE_FILE_EOF.len()).unwrap());
    bytes.extend_from_slice(SAVE_FILE_EOF.as_bytes());
    bytes
}

#[test]
fn native_world_v26_players_v6_restores_pinned_cash_and_spent_wallet() {
    crate::game_logic::game_logic::pose_owner_tests::isolated_at(
        module_path!(),
        "native_world_v26_players_v6_restores_pinned_cash_and_spent_wallet",
        || {
            let mut source = GameLogic::new();
            source.admit_new_game_starting_cash(None, 17_321);
            let mut player = Player::new(3, Team::USA, "Spent historical wallet", true);
            player.resources.supplies = 3;
            source.add_player(player);
            let builder = SnapshotBuilder::new();
            let mut snapshot = builder.create_world_snapshot(&source).unwrap();
            snapshot.version = 26;
            let chunks = player_team_chunks_from_world(&snapshot).unwrap();
            let mut cursor = Cursor::new(Vec::new());
            {
                let mut xfer = CommonXferSave::new(&mut cursor, SAVE_FILE_VERSION);
                write_players_block(&mut xfer, &chunks).unwrap();
            }
            let mut players = cursor.into_inner();
            assert_eq!(players[0], 7);
            // Players v6 is the same body without the v7 source tag/base.
            assert_eq!(players[players.len() - 5], 1);
            assert_eq!(&players[players.len() - 4..], &17_321u32.to_le_bytes());
            players[0] = 6;
            players.truncate(players.len() - 5);
            let old_chunks = stash_loaded_player_team_chunks(Some(&players), None).unwrap();
            assert!(
                old_chunks
                    .players
                    .unwrap()
                    .host_starting_cash_source
                    .is_none()
            );

            let bytes = named_save(&snapshot, &players);
            let (decoded, _) = SaveFileManager::read_common_sav_chunks(&bytes, Path::new(""))
                .expect("previous World26/Players6 native save stays readable");
            assert_eq!(decoded.version, 26);
            let migrated = player_team_chunks_from_world(&decoded).unwrap();
            assert!(
                migrated
                    .players
                    .as_ref()
                    .unwrap()
                    .host_starting_cash_source
                    .is_some()
            );
            assert_eq!(migrated.players.unwrap().host_starting_cash, Some(17_321));

            let mut receiving = GameLogic::new();
            receiving.admit_new_game_starting_cash(None, 93_000);
            let mut existing = Player::new(3, Team::GLA, "Receiving wallet", false);
            existing.resources.supplies = 987;
            receiving.add_player(existing);
            builder
                .restore_from_snapshot(&decoded, &mut receiving)
                .unwrap();
            assert_eq!(receiving.get_player(3).unwrap().resources.supplies, 3);
            assert_eq!(receiving.skirmish_rules().starting_cash, 17_321);
            assert_eq!(receiving.skirmish_rules().starting_cash_default, None);

            // A current envelope cannot advertise source evidence that the
            // historical outer Players block does not contain.
            snapshot.version = WORLD_SNAPSHOT_BINCODE_VERSION;
            assert!(
                SaveFileManager::read_common_sav_chunks(
                    &named_save(&snapshot, &players),
                    Path::new("")
                )
                .is_err()
            );
        },
    );
}
