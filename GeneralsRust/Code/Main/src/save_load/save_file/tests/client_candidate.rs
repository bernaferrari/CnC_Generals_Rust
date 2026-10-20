//! CHUNK_GameClient belongs to the archive that decoded it, not a pending slot.
use super::*;

fn client_bytes(frame: u32) -> Vec<u8> {
    let mut client = game_client::core::game_client::GameClient::new().unwrap();
    client.set_frame(frame);
    capture_game_client_xfer_bytes(&mut client).unwrap()
}

fn archive(bytes: &[u8], description: &str, directory: &Path) -> Vec<u8> {
    let mut info = fixture_save_info();
    info.description = description.into();
    SaveFileManager::write_common_sav_chunks_with_client_bytes(
        &WorldSnapshot::default(),
        &info,
        bytes,
        directory,
    )
    .unwrap()
}

fn repack(blocks: Vec<(String, Vec<u8>)>) -> Vec<u8> {
    let mut bytes = Vec::new();
    for (name, payload) in blocks {
        bytes.push(u8::try_from(name.len()).unwrap());
        bytes.extend_from_slice(name.as_bytes());
        bytes.extend_from_slice(&i32::try_from(payload.len()).unwrap().to_le_bytes());
        bytes.extend_from_slice(&payload);
    }
    bytes.push(SAVE_FILE_EOF.len() as u8);
    bytes.extend_from_slice(SAVE_FILE_EOF.as_bytes());
    bytes
}

#[test]
fn decoded_client_payloads_survive_interleaved_files_failures_and_null_chunks() {
    crate::game_logic::game_logic::pose_owner_tests::isolated_at(
        module_path!(),
        "decoded_client_payloads_survive_interleaved_files_failures_and_null_chunks",
        || {
            let a_dir = tempfile::tempdir().unwrap();
            let b_dir = tempfile::tempdir().unwrap();
            let mut a_manager = SaveFileManager::with_save_directory(a_dir.path());
            let mut b_manager = SaveFileManager::with_save_directory(b_dir.path());
            a_manager.init().unwrap();
            b_manager.init().unwrap();
            let a_bytes = client_bytes(41);
            let b_bytes = client_bytes(82);
            assert_ne!(a_bytes, b_bytes);
            let a_archive = archive(&a_bytes, "client owner A", a_dir.path());
            let b_archive = archive(&b_bytes, "client owner B", b_dir.path());
            std::fs::write(a_manager.get_save_path("a"), &a_archive).unwrap();
            std::fs::write(b_manager.get_save_path("b"), &b_archive).unwrap();
            let mut live = game_client::core::game_client::GameClient::new().unwrap();
            live.set_frame(777);

            let a = a_manager.load_game_snapshot("a").unwrap();
            let b = b_manager.load_game_snapshot("b").unwrap();
            assert_eq!(a.save_info.description, "client owner A");
            assert_eq!(b.save_info.description, "client owner B");
            assert_eq!(
                a.game_client_xfer.as_deref(),
                Some(a_bytes.as_slice()),
                "decoded A must own its client chunk after B decodes"
            );
            assert_eq!(b.game_client_xfer.as_deref(), Some(b_bytes.as_slice()));

            // This fails only after the client chunk was visited. It must not
            // publish bytes that an unrelated valid or empty archive can take.
            let mut invalid = walk_named_chunks(&b_archive).unwrap();
            invalid.retain(|(name, _)| name != CHUNK_GAME_LOGIC);
            std::fs::write(b_manager.get_save_path("broken"), repack(invalid)).unwrap();
            assert!(b_manager.load_game_snapshot("broken").is_err());
            assert!(b_manager.load_game_snapshot("missing").is_err());

            let null_archive = archive(&[], "historical null client", a_dir.path());
            std::fs::write(a_manager.get_save_path("null"), &null_archive).unwrap();
            let null = a_manager.load_game_snapshot("null").unwrap();
            assert!(null.game_client_xfer.is_none());
            let mut omitted = walk_named_chunks(&null_archive).unwrap();
            omitted.retain(|(name, _)| name != CHUNK_GAME_CLIENT);
            std::fs::write(b_manager.get_save_path("omitted"), repack(omitted)).unwrap();
            assert!(
                b_manager
                    .load_game_snapshot("omitted")
                    .unwrap()
                    .game_client_xfer
                    .is_none()
            );

            // Preserve payload validity as a separate admission phase. A
            // malformed nested client block never changes the live receiver.
            let mut malformed = walk_named_chunks(&b_archive).unwrap();
            let (_, chunk) = malformed
                .iter_mut()
                .find(|(name, _)| name == CHUNK_GAME_CLIENT)
                .unwrap();
            chunk.truncate(3);
            std::fs::write(b_manager.get_save_path("malformed"), repack(malformed)).unwrap();
            let malformed = b_manager.load_game_snapshot("malformed").unwrap();
            assert!(
                validate_game_client_xfer_bytes(malformed.game_client_xfer.as_deref().unwrap())
                    .is_err()
            );
            drop(malformed);
            drop(null);
            drop(b_manager);
            assert_eq!(live.get_frame(), 777, "decoding never applies client state");
            assert_eq!(a.game_client_xfer.as_deref(), Some(a_bytes.as_slice()));
            assert_eq!(b.game_client_xfer.as_deref(), Some(b_bytes.as_slice()));

            let mut receiver_b = game_client::core::game_client::GameClient::new().unwrap();
            restore_game_client_from_xfer_bytes(
                &mut receiver_b,
                b.game_client_xfer.as_deref().unwrap(),
            )
            .unwrap();
            assert_eq!(receiver_b.get_frame(), 82);
            assert_eq!(live.get_frame(), 777);
            restore_game_client_from_xfer_bytes(&mut live, a.game_client_xfer.as_deref().unwrap())
                .unwrap();
            assert_eq!(live.get_frame(), 41);
            assert_eq!(receiver_b.get_frame(), 82);
        },
    );
}
