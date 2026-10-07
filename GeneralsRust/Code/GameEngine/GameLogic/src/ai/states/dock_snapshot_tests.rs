//! C++ AIStates.cpp:6024-6044 gates child bytes on the wire presence flag.
use super::*;
use game_engine::common::system::xfer_load::XferLoad;
use game_engine::common::system::xfer_save::XferSave;
use std::io::Cursor;

fn save(state: &mut AIDockState) -> Vec<u8> {
    let mut bytes = Cursor::new(Vec::new());
    state.xfer(&mut XferSave::new(&mut bytes, 1)).unwrap();
    bytes.into_inner()
}

fn save_child(state: &mut AIDockState) -> Vec<u8> {
    let mut bytes = Cursor::new(Vec::new());
    state
        .dock_machine
        .as_mut()
        .unwrap()
        .xfer(&mut XferSave::new(&mut bytes, 1))
        .unwrap();
    bytes.into_inner()
}

fn parent_with_child(child: AIDockMachine) -> AIDockState {
    let machine = StateMachine::new_with_owner_id(INVALID_ID, "dock-parent-wire");
    let mut state = AIDockState::new(&machine);
    state.dock_machine = Some(child);
    state
}

#[test]
fn absent_child_snapshot_skips_existing_child_and_preserves_precision_alignment() {
    for precision in [false, true] {
        crate::ai::dock::with_started_test_machines(|child, _receiving| {
            let machine = StateMachine::new_with_owner_id(INVALID_ID, "empty-dock");
            let mut source = AIDockState::new(&machine);
            source.using_precision_movement = precision;
            let mut bytes = save(&mut source);
            assert_eq!(bytes, vec![1, 0, u8::from(precision)]);
            bytes.extend_from_slice(&0xC0FFEE42_u32.to_le_bytes());

            let mut restored = parent_with_child(child);
            restored.using_precision_movement = !precision;
            let prior_child = save_child(&mut restored);
            let mut load = XferLoad::new(Cursor::new(bytes), 1);
            restored
                .xfer(&mut load)
                .expect("absent child must consume no child bytes");
            assert_eq!(restored.using_precision_movement, precision);
            assert!(
                restored.dock_machine.is_some(),
                "C++ retains an existing child"
            );
            assert_eq!(
                save_child(&mut restored),
                prior_child,
                "load must not advance or reset child"
            );
            let mut sentinel = 0;
            Xfer::xfer_u32(&mut load, &mut sentinel).unwrap();
            assert_eq!(sentinel, 0xC0FFEE42);
        });
    }
}

#[test]
fn present_child_snapshot_roundtrips_into_existing_parent() {
    crate::ai::dock::with_started_test_machines(|child, receiving| {
        let mut source = parent_with_child(child);
        source.using_precision_movement = true;
        let bytes = save(&mut source);
        assert_eq!(&bytes[..2], &[1, 1]);
        let mut restored = parent_with_child(receiving);
        restored
            .xfer(&mut XferLoad::new(Cursor::new(bytes.clone()), 1))
            .unwrap();
        assert!(restored.using_precision_movement);
        assert_eq!(save(&mut restored), bytes);
    });
}
