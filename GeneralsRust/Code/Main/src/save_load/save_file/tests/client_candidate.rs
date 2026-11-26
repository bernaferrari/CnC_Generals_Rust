//! Typed CHUNK_GameClient data belongs to the archive that decoded it, not a pending slot.
use super::*;

fn client_archive(frame: u32) -> GameClientArchive {
    let mut client = game_client::core::game_client::GameClient::new().unwrap();
    client.set_frame(frame);
    capture_game_client_archive(&mut client).unwrap()
}

fn archive(
    client_archive: Option<&GameClientArchive>,
    description: &str,
    directory: &Path,
) -> Vec<u8> {
    let mut info = fixture_save_info();
    info.description = description.into();
    let manager = SaveFileManager::with_save_directory(directory);
    let path = manager.get_save_path("__archive_fixture");
    manager
        .save_to_file(
            &path,
            &WorldSnapshot::default(),
            &info,
            client_archive,
            None,
            SAVED_GAME_MODE_SINGLE_PLAYER,
            &game_engine::System::CampaignManagerXferState::default(),
            MapAdmissionIntent::Authored,
        )
        .unwrap();
    let archive = std::fs::read(&path).unwrap();
    let _ = std::fs::remove_file(path);
    archive
}

#[test]
fn decoded_client_archives_survive_interleaved_files_failures_and_absent_payloads() {
    crate::game_logic::game_logic::pose_owner_tests::isolated_at(
        module_path!(),
        "decoded_client_archives_survive_interleaved_files_failures_and_absent_payloads",
        || {
            let a_dir = tempfile::tempdir().unwrap();
            let b_dir = tempfile::tempdir().unwrap();
            let mut a_manager = SaveFileManager::with_save_directory(a_dir.path());
            let mut b_manager = SaveFileManager::with_save_directory(b_dir.path());
            a_manager.init().unwrap();
            b_manager.init().unwrap();
            let a_client = client_archive(41);
            let b_client = client_archive(82);
            assert_ne!(a_client, b_client);
            let a_archive = archive(Some(&a_client), "client owner A", a_dir.path());
            let b_archive = archive(Some(&b_client), "client owner B", b_dir.path());
            std::fs::write(a_manager.get_save_path("a"), &a_archive).unwrap();
            std::fs::write(b_manager.get_save_path("b"), &b_archive).unwrap();
            let mut live = game_client::core::game_client::GameClient::new().unwrap();
            live.set_frame(777);

            let a = a_manager.load_game_snapshot("a").unwrap();
            let b = b_manager.load_game_snapshot("b").unwrap();
            assert_eq!(a.save_info.description, "client owner A");
            assert_eq!(b.save_info.description, "client owner B");
            assert_eq!(
                a.game_client_archive.as_ref(),
                Some(&a_client),
                "decoded A must own its client archive after B decodes"
            );
            assert_eq!(b.game_client_archive.as_ref(), Some(&b_client));

            // Corruption is contained in that candidate archive. It must not
            // publish companion data that an unrelated valid archive can take.
            let mut invalid = decode_versioned_archive(&b_archive);
            invalid.snapshot.version += 1;
            std::fs::write(
                b_manager.get_save_path("broken"),
                encode_versioned_archive(&invalid),
            )
            .unwrap();
            assert!(b_manager.load_game_snapshot("broken").is_err());
            assert!(b_manager.load_game_snapshot("missing").is_err());

            let null_archive = archive(None, "explicitly absent client", a_dir.path());
            std::fs::write(a_manager.get_save_path("null"), &null_archive).unwrap();
            let null = a_manager.load_game_snapshot("null").unwrap();
            assert!(null.game_client_archive.is_none());
            let mut omitted = decode_versioned_archive(&null_archive);
            omitted.game_client_archive = None;
            std::fs::write(
                b_manager.get_save_path("omitted"),
                encode_versioned_archive(&omitted),
            )
            .unwrap();
            assert!(
                b_manager
                    .load_game_snapshot("omitted")
                    .unwrap()
                    .game_client_archive
                    .is_none()
            );

            validate_game_client_archive(b.game_client_archive.as_ref().unwrap()).unwrap();
            drop(null);
            drop(b_manager);
            assert_eq!(live.get_frame(), 777, "decoding never applies client state");
            assert_eq!(a.game_client_archive.as_ref(), Some(&a_client));
            assert_eq!(b.game_client_archive.as_ref(), Some(&b_client));

            let mut receiver_b = game_client::core::game_client::GameClient::new().unwrap();
            restore_game_client_from_archive(
                &mut receiver_b,
                b.game_client_archive.as_ref().unwrap(),
            )
            .unwrap();
            assert_eq!(receiver_b.get_frame(), 82);
            assert_eq!(live.get_frame(), 777);
            restore_game_client_from_archive(&mut live, a.game_client_archive.as_ref().unwrap())
                .unwrap();
            assert_eq!(live.get_frame(), 41);
            assert_eq!(receiver_b.get_frame(), 82);
        },
    );
}
