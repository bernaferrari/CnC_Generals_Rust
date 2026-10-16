//! Operation-local, non-Send owners retain named chunk and deferred fixup order.
use super::*;
use std::{
    cell::{Cell, RefCell},
    rc::Rc,
};

struct RequiredReceiver;
impl Snapshot for RequiredReceiver {
    fn requires_operation_owner(&self) -> bool {
        true
    }
    fn crc(&mut self, _: &mut dyn Xfer) -> Result<(), XferStatus> {
        Err(XferStatus::InvalidData)
    }
    fn xfer(&mut self, _: &mut dyn Xfer) -> Result<(), XferStatus> {
        Err(XferStatus::InvalidData)
    }
    fn load_post_process(&mut self) -> Result<(), XferStatus> {
        Err(XferStatus::InvalidData)
    }
}
struct LocalReceiver {
    value: Rc<Cell<u32>>,
    events: Rc<RefCell<Vec<&'static str>>>,
}
impl Snapshot for LocalReceiver {
    fn crc(&mut self, xfer: &mut dyn Xfer) -> Result<(), XferStatus> {
        self.xfer(xfer)
    }
    fn xfer(&mut self, xfer: &mut dyn Xfer) -> Result<(), XferStatus> {
        let mut value = self.value.get();
        xfer.xfer_unsigned_int(&mut value)?;
        if xfer.get_xfer_mode() == XferMode::Load {
            self.value.set(value);
            self.events.borrow_mut().push("transfer");
        }
        Ok(())
    }
    fn load_post_process(&mut self) -> Result<(), XferStatus> {
        self.events.borrow_mut().push("fixup");
        Ok(())
    }
}
fn state(path: &Path) -> GameState {
    let mut state = GameState::new(path.to_owned());
    state.add_snapshot_block(
        "CHUNK_GameClient".into(),
        Box::new(RequiredReceiver),
        SnapshotType::SaveLoad,
    );
    state
}
#[test]
fn non_send_receiver_round_trips_through_named_save_and_load_with_later_fixup() {
    if let crate::test_process::TestProcess::ParentVerified = crate::test_process::run_bounded(
        module_path!(),
        "non_send_receiver_round_trips_through_named_save_and_load_with_later_fixup",
    ) {
        return;
    }
    let directory = tempfile::tempdir().unwrap();
    let mut writer = state(directory.path());
    let value = Rc::new(Cell::new(37));
    let events = Rc::new(RefCell::new(Vec::new()));
    let mut source = LocalReceiver {
        value: value.clone(),
        events: events.clone(),
    };
    assert_eq!(
        writer.save_game_with_snapshots(
            "owned.sav".into(),
            "owned".into(),
            SaveFileType::Normal,
            SnapshotType::SaveLoad,
            &mut [BorrowedSnapshotBlock {
                block_name: "CHUNK_GameClient",
                snapshot: &mut source
            }]
        ),
        Ok(SaveCode::Ok)
    );
    let peer = Rc::new(Cell::new(91));
    let target = Rc::new(Cell::new(777));
    let mut receiving = LocalReceiver {
        value: target.clone(),
        events: events.clone(),
    };
    let mut reader = state(directory.path());
    let info = AvailableGameInfo {
        filename: "owned.sav".into(),
        save_game_info: SaveGameInfo::default(),
    };
    assert_eq!(
        reader.load_game_with_snapshots(
            info,
            &mut [BorrowedSnapshotBlock {
                block_name: "CHUNK_GameClient",
                snapshot: &mut receiving
            }]
        ),
        Ok(SaveCode::Ok)
    );
    assert_eq!(target.get(), 37);
    assert_eq!(value.get(), 37);
    assert_eq!(peer.get(), 91);
    assert_eq!(&*events.borrow(), &["transfer", "fixup"]);
    assert!(reader.snapshot_post_process_list.is_empty());
    // Context was not retained; the next operation must receive its own loan.
    assert_eq!(
        reader.save_game(
            "missing.sav".into(),
            "missing".into(),
            SaveFileType::Normal,
            SnapshotType::SaveLoad
        ),
        Err(XferStatus::InvalidData)
    );
    assert!(!directory.path().join("missing.sav").exists());
}
#[test]
fn missing_owner_rejects_before_file_or_state_changes() {
    let directory = tempfile::tempdir().unwrap();
    let mut state = state(directory.path());
    state.game_info.description = "unchanged".into();
    assert_eq!(
        state.save_game(
            "missing.sav".into(),
            "changed".into(),
            SaveFileType::Normal,
            SnapshotType::SaveLoad
        ),
        Err(XferStatus::InvalidData)
    );
    assert_eq!(state.game_info.description, "unchanged");
    assert!(!directory.path().join("missing.sav").exists());
    assert_eq!(
        state.load_game(AvailableGameInfo {
            filename: "missing.sav".into(),
            save_game_info: SaveGameInfo::default()
        }),
        Err(XferStatus::InvalidData)
    );
    assert!(!state.is_in_load_game());
}
#[test]
fn ambiguous_unknown_or_metadata_loans_reject_without_writing() {
    let directory = tempfile::tempdir().unwrap();
    let mut state = state(directory.path());
    let mut first = LocalReceiver {
        value: Rc::new(Cell::new(1)),
        events: Rc::new(RefCell::new(Vec::new())),
    };
    let mut second = LocalReceiver {
        value: Rc::new(Cell::new(2)),
        events: Rc::new(RefCell::new(Vec::new())),
    };
    for name in ["CHUNK_Missing", "CHUNK_GameState"] {
        assert_eq!(
            state.save_game_with_snapshots(
                "invalid.sav".into(),
                "invalid".into(),
                SaveFileType::Normal,
                SnapshotType::SaveLoad,
                &mut [BorrowedSnapshotBlock {
                    block_name: name,
                    snapshot: &mut first
                }]
            ),
            Err(XferStatus::InvalidData)
        );
    }
    assert_eq!(
        state.save_game_with_snapshots(
            "invalid.sav".into(),
            "invalid".into(),
            SaveFileType::Normal,
            SnapshotType::SaveLoad,
            &mut [
                BorrowedSnapshotBlock {
                    block_name: "CHUNK_GameClient",
                    snapshot: &mut first
                },
                BorrowedSnapshotBlock {
                    block_name: "CHUNK_GameClient",
                    snapshot: &mut second
                }
            ]
        ),
        Err(XferStatus::InvalidData)
    );
    assert!(!directory.path().join("invalid.sav").exists());
}
