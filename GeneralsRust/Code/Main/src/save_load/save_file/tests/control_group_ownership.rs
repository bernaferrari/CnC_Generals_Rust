use super::*;
use crate::save_load::snapshot::{
    CameraPersist, ClientSaveState, ControlGroupSave, read_control_groups,
};
use std::collections::HashMap;

#[test]
#[cfg(feature = "game_client")]
fn owned_control_groups_actual_archives_keep_empty_and_peer_groups_detached() {
    let directory = tempfile::tempdir().unwrap();
    let mut saves = SaveFileManager::with_save_directory(directory.path());
    let mut a = GameLogic::new();
    let mut b = GameLogic::new();
    a.add_player(Player::new(0, Team::USA, "OwnedGroupPlayer", true));
    b.add_player(Player::new(0, Team::USA, "OwnedGroupPlayer", true));
    let info = fixture_save_info();
    for (name, world, ids) in [
        ("groups_a", &a, vec![42, 43]),
        ("groups_b", &b, vec![42, 99]),
        ("groups_empty", &a, Vec::new()),
    ] {
        let mut client = game_client::core::game_client::GameClient::new().unwrap();
        let groups = if ids.is_empty() {
            HashMap::new()
        } else {
            HashMap::from([(3, ids)])
        };
        let camera = CameraPersist {
            angle: if name == "groups_b" { 1.5 } else { 0.25 },
            position: [100.0, 200.0, 300.0],
            target: [10.0, 0.0, 20.0],
            zoom: 1.0,
        };
        saves
            .save_game_with_client_save_state(
                name,
                world,
                ClientDrawableWorldSnapshot::default(),
                &mut client,
                &info,
                ClientSaveState {
                    camera: Some(camera),
                    camera_continuation: None,
                    control_groups: Some(ControlGroupSave {
                        player_index: 0,
                        groups,
                    }),
                },
            )
            .unwrap();
    }
    let crate::save_load::DecodedGameSave {
        snapshot: loaded_b, ..
    } = saves.load_game_snapshot("groups_b").unwrap();
    let crate::save_load::DecodedGameSave {
        snapshot: loaded_a, ..
    } = saves.load_game_snapshot("groups_a").unwrap();
    let crate::save_load::DecodedGameSave {
        snapshot: empty, ..
    } = saves.load_game_snapshot("groups_empty").unwrap();
    assert_eq!(loaded_a.persist_v18.camera().unwrap().angle, 0.25);
    assert_eq!(loaded_b.persist_v18.camera().unwrap().angle, 1.5);
    assert_eq!(
        read_control_groups(&loaded_a.lifecycle_tail)
            .unwrap()
            .unwrap()[&0][&3],
        [42, 43]
    );
    assert_eq!(
        read_control_groups(&loaded_b.lifecycle_tail)
            .unwrap()
            .unwrap()[&0][&3],
        [42, 99]
    );
    assert!(read_control_groups(&empty.lifecycle_tail).unwrap().unwrap()[&0].is_empty());
    let mut corrupted = loaded_b.lifecycle_tail.clone();
    let index = corrupted.windows(4).rposition(|w| w == b"HSQD").unwrap();
    corrupted[index + 4..index + 8].copy_from_slice(&u32::MAX.to_le_bytes());
    assert!(read_control_groups(&corrupted).is_err());
    assert_eq!(
        read_control_groups(&loaded_a.lifecycle_tail)
            .unwrap()
            .unwrap()[&0][&3],
        [42, 43]
    );
    // Reject invalid detached host input before replacing an accepted archive.
    let before = std::fs::read(saves.get_save_path("groups_a")).unwrap();
    let mut client = game_client::core::game_client::GameClient::new().unwrap();
    assert!(
        saves
            .save_game_with_client_save_state(
                "groups_a",
                &a,
                ClientDrawableWorldSnapshot::default(),
                &mut client,
                &info,
                ClientSaveState {
                    camera: None,
                    camera_continuation: None,
                    control_groups: Some(ControlGroupSave {
                        player_index: 0,
                        groups: HashMap::from([(10, vec![42])]),
                    }),
                },
            )
            .is_err()
    );
    assert_eq!(
        std::fs::read(saves.get_save_path("groups_a")).unwrap(),
        before
    );
}

#[test]
#[cfg(feature = "game_client")]
fn owned_control_groups_new_record_preserves_terminal_player_team_capsule() {
    crate::game_logic::game_logic::pose_owner_tests::isolated_at(
        module_path!(),
        "owned_control_groups_new_record_preserves_terminal_player_team_capsule",
        || {
            let mut source = GameLogic::new();
            source.admit_new_game_starting_cash(None, 17_321);
            let mut player = Player::new(0, Team::USA, "Owned groups player", true);
            player.resources.supplies = 17;
            player.academy_stats.record_control_groups_used();
            source.add_player(player);
            source.named_unit_history.insert("PreviouslySeen".into());
            let builder = SnapshotBuilder::new();
            let before = builder.create_world_snapshot(&source).unwrap();
            assert!(
                read_control_groups(&before.lifecycle_tail)
                    .unwrap()
                    .is_none(),
                "fixture must exercise insertion rather than replacement of an existing HSQD"
            );
            let expected = player_team_chunks_from_world(&before).unwrap();
            assert!(expected.players.is_some() && expected.teams.is_some());
            assert!(before.lifecycle_tail.ends_with(b"PTSC"));
            let directory = tempfile::tempdir().unwrap();
            let mut saves = SaveFileManager::with_save_directory(directory.path());
            let mut client = game_client::core::game_client::GameClient::new().unwrap();
            saves
                .save_game_with_client_save_state(
                    "owned_capsule",
                    &source,
                    ClientDrawableWorldSnapshot::default(),
                    &mut client,
                    &fixture_save_info(),
                    ClientSaveState {
                        camera: None,
                        camera_continuation: None,
                        control_groups: Some(ControlGroupSave {
                            player_index: 0,
                            groups: HashMap::from([(3, vec![42, 43])]),
                        }),
                    },
                )
                .unwrap();
            let crate::save_load::DecodedGameSave {
                snapshot: loaded, ..
            } = saves.load_game_snapshot("owned_capsule").unwrap();
            assert!(loaded.lifecycle_tail.ends_with(b"PTSC"));
            assert_eq!(player_team_chunks_from_world(&loaded).unwrap(), expected);
            assert_eq!(
                read_control_groups(&loaded.lifecycle_tail)
                    .unwrap()
                    .unwrap()[&0][&3],
                [42, 43]
            );
            let mut receiving = GameLogic::new();
            receiving.add_player(Player::new(0, Team::USA, "Owned groups player", true));
            saves
                .restore_game_snapshot(&loaded, &mut receiving)
                .unwrap();
            assert_eq!(receiving.get_player(0).unwrap().resources.supplies, 17);
            assert_eq!(receiving.skirmish_rules().starting_cash, 17_321);
            assert_eq!(receiving.body_damage_policy(), source.body_damage_policy());
            assert!(receiving.named_unit_history.contains("PreviouslySeen"));
            assert_eq!(
                receiving.get_player(0).unwrap().academy_stats.serialize(),
                source.get_player(0).unwrap().academy_stats.serialize()
            );
        },
    );
}

#[test]
fn generic_snapshot_never_imports_or_restores_foreign_core_squads() {
    crate::game_logic::game_logic::pose_owner_tests::isolated_at(
        module_path!(),
        "generic_snapshot_never_imports_or_restores_foreign_core_squads",
        || {
            let mut foreign = gamelogic::player::Player::new(0);
            foreign.init_from_dict_defaults();
            foreign.process_create_team_game_message(3, &[99, 98]);
            let foreign = std::sync::Arc::new(std::sync::RwLock::new(foreign));
            {
                let mut list = gamelogic::player::ThePlayerList().write().unwrap();
                list.clear();
                list.add_player(std::sync::Arc::clone(&foreign));
            }
            let mut owner = GameLogic::new();
            let mut peer = GameLogic::new();
            owner.add_player(Player::new(0, Team::USA, "GroupsOwner", true));
            peer.add_player(Player::new(0, Team::USA, "GroupsOwner", true));
            let builder = SnapshotBuilder::new();
            let mut saved = builder.create_world_snapshot(&owner).unwrap();
            assert!(
                read_control_groups(&saved.lifecycle_tail)
                    .unwrap()
                    .is_none()
            );
            crate::save_load::snapshot::write_control_groups(
                &mut saved.lifecycle_tail,
                &ControlGroupSave {
                    player_index: 0,
                    groups: HashMap::from([(3, vec![42, 43])]),
                },
            )
            .unwrap();
            let encoded = bincode_legacy::serialize(&saved).unwrap();
            let decoded =
                crate::save_load::snapshot::decode_bincode_world_snapshot(&encoded).unwrap();
            builder.restore_from_snapshot(&decoded, &mut peer).unwrap();
            assert_eq!(
                read_control_groups(&decoded.lifecycle_tail)
                    .unwrap()
                    .unwrap()[&0][&3],
                [42, 43]
            );
            assert_eq!(
                foreign
                    .read()
                    .unwrap()
                    .get_hotkey_squad_const(3)
                    .unwrap()
                    .get_object_ids(),
                &vec![99, 98]
            );
            owner.reset();
            assert_eq!(
                read_control_groups(&decoded.lifecycle_tail)
                    .unwrap()
                    .unwrap()[&0][&3],
                [42, 43]
            );
        },
    );
}
