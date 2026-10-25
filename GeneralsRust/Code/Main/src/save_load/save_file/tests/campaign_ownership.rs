//! Each decoded CHUNK_Campaign stays with the archive that supplied it.
use super::*;

fn campaign_archive(mut rank: i32) -> Vec<u8> {
    let bytes = SaveFileManager::write_common_sav_chunks(
        &WorldSnapshot::default(),
        &fixture_save_info(),
        CPP_GAME_SINGLE_PLAYER,
    )
    .unwrap();
    let mut blocks = walk_named_chunks(&bytes).unwrap();
    let mut payload = Vec::new();
    {
        let mut cursor = Cursor::new(&mut payload);
        let mut xfer = CommonXferSave::new(&mut cursor, SAVE_FILE_VERSION);
        let mut version = 5;
        xfer.xfer_version(&mut version, 5).unwrap();
        write_ascii(&mut xfer, "USA").unwrap();
        write_ascii(&mut xfer, "USA01").unwrap();
        xfer.xfer_int(&mut rank).unwrap();
        xfer.xfer_int(&mut 1).unwrap();
        xfer.xfer_bool(&mut false).unwrap();
        xfer.xfer_int(&mut 0).unwrap();
    }
    blocks
        .iter_mut()
        .find(|(name, _)| name == CHUNK_CAMPAIGN)
        .unwrap()
        .1 = payload;
    repack(blocks)
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
fn interleaved_and_failed_decodes_preserve_each_campaign_candidate() {
    crate::game_logic::game_logic::pose_owner_tests::isolated_at(
        module_path!(),
        "interleaved_and_failed_decodes_preserve_each_campaign_candidate",
        || {
            let prior = capture_live_campaign_state();
            let a = SaveFileManager::read_common_sav_chunks(&campaign_archive(17), Path::new(""))
                .unwrap();
            let b_bytes = campaign_archive(29);
            let b = SaveFileManager::read_common_sav_chunks(&b_bytes, Path::new("")).unwrap();
            let mut failed = walk_named_chunks(&b_bytes).unwrap();
            failed
                .iter_mut()
                .find(|(name, _)| name == CHUNK_GAME_LOGIC)
                .unwrap()
                .1 = vec![0xff];
            assert!(
                SaveFileManager::read_common_sav_chunks(&repack(failed), Path::new("")).is_err()
            );
            assert_eq!(a.campaign_state.as_ref().unwrap().rank_points, 17);
            assert_eq!(b.campaign_state.as_ref().unwrap().rank_points, 29);
            assert_eq!(capture_live_campaign_state().rank_points, prior.rank_points);
            drop(b);
            commit_campaign_state(a.campaign_state);
            assert_eq!(capture_live_campaign_state().rank_points, 17);
            apply_campaign_manager_state(prior);
        },
    );
}
