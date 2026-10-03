// GameState owns deferred block fixups; nested snapshots do not borrow it.
struct OrderedFixupSnapshot {
    id: u32,
    nested: bool,
    events: Arc<Mutex<Vec<(bool, u32)>>>,
}

impl Snapshot for OrderedFixupSnapshot {
    fn crc(&mut self, xfer: &mut dyn Xfer) -> Result<(), XferStatus> {
        self.xfer(xfer)
    }

    fn xfer(&mut self, xfer: &mut dyn Xfer) -> Result<(), XferStatus> {
        xfer.xfer_unsigned_int(&mut self.id)?;
        if xfer.get_xfer_mode() == XferMode::Load {
            self.events.lock().unwrap().push((false, self.id));
        }
        if self.nested {
            let mut child = OrderedFixupSnapshot {
                id: self.id + 100,
                nested: false,
                events: Arc::clone(&self.events),
            };
            xfer.xfer_snapshot(&mut child)?;
        }
        Ok(())
    }

    fn load_post_process(&mut self) -> Result<(), XferStatus> {
        self.events.lock().unwrap().push((true, self.id));
        Ok(())
    }
}

#[test]
fn deferred_fixups_follow_all_nested_transfers_in_block_order() {
    verify_owned_fixup_registration(0);
}

#[test]
fn no_post_processing_keeps_nested_transfers_without_fixups() {
    verify_owned_fixup_registration(xfer_options::NO_POST_PROCESSING);
}

fn verify_owned_fixup_registration(options: u32) {
    let _lock = HOOK_TEST_LOCK.lock().unwrap();
    let directory = unique_temp_save_dir("owned_fixups");
    fs::create_dir_all(&directory).unwrap();
    let path = directory.join("nested.sav");
    let events = Arc::new(Mutex::new(Vec::new()));
    let mut state = GameState::new(directory.clone());
    for (name, id) in [("CHUNK_InGameUI", 1), ("CHUNK_ParticleSystem", 2)] {
        state.add_snapshot_block(
            name.to_string(),
            Box::new(OrderedFixupSnapshot {
                id,
                nested: true,
                events: Arc::clone(&events),
            }),
            SnapshotType::SaveLoad,
        );
    }
    let mut save = XferSave::new();
    save.open(path.to_string_lossy().into_owned()).unwrap();
    state
        .xfer_save_data(&mut save, SnapshotType::SaveLoad)
        .unwrap();
    save.close().unwrap();

    let mut load = XferLoad::new();
    load.open(path.to_string_lossy().into_owned()).unwrap();
    load.set_options(options);
    state
        .xfer_save_data(&mut load, SnapshotType::SaveLoad)
        .unwrap();
    load.close().unwrap();
    assert_eq!(
        *events.lock().unwrap(),
        [(false, 1), (false, 101), (false, 2), (false, 102)]
    );
    if options == 0 {
        assert_eq!(
            state.snapshot_post_process_list.len(),
            SAVELOAD_BLOCK_NAMES.len() - 1
        );
    } else {
        assert!(state.snapshot_post_process_list.is_empty());
    }
    state.game_state_post_process_load().unwrap();
    let mut expected = vec![(false, 1), (false, 101), (false, 2), (false, 102)];
    if options == 0 {
        expected.extend([(true, 1), (true, 2)]);
    }
    assert_eq!(*events.lock().unwrap(), expected);
    assert!(state.snapshot_post_process_list.is_empty());
    fs::remove_dir_all(directory).unwrap();
}

// Shared runtime configuration is restored even if a snapshot assertion fails.
struct TestRuntimeMapName(String);

impl TestRuntimeMapName {
    fn set(name: String) -> Self {
        if crate::common::ini::ini_game_data::get_global_data().is_none() {
            crate::common::ini::ini_game_data::init_global_data();
        }
        let global = crate::common::ini::ini_game_data::get_global_data().unwrap();
        let previous = std::mem::replace(&mut global.write().map_name, name);
        Self(previous)
    }
}

impl Drop for TestRuntimeMapName {
    fn drop(&mut self) {
        if let Some(global) = crate::common::ini::ini_game_data::get_global_data() {
            global.write().map_name = std::mem::take(&mut self.0);
        }
    }
}
