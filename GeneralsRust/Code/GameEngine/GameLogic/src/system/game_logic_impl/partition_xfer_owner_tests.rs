//! Actual Core file Xfer against explicit local receiving owners, no globals.
use super::*;
use crate::system::shroud_manager::{ShroudManager, ShroudSnapshot};
use game_engine::{Xfer, XferLoad, XferSave};
fn path(label: &str) -> std::path::PathBuf {
    std::env::temp_dir().join(format!(
        "owned_partition_{}_{}.bin",
        std::process::id(),
        label
    ))
}
fn save(label: &str, owner: &mut ShroudManager, size: &mut f32) -> Vec<u8> {
    let path = path(label);
    let mut xfer = XferSave::new();
    xfer.open(path.to_string_lossy().into_owned()).unwrap();
    xfer_partition_state_with_owner(owner, size, 7, &mut xfer).unwrap();
    xfer.close().unwrap();
    let bytes = std::fs::read(&path).unwrap();
    std::fs::remove_file(path).unwrap();
    bytes
}
fn load(
    label: &str,
    bytes: &[u8],
    owner: &mut ShroudManager,
    size: &mut f32,
) -> Result<(), XferStatus> {
    let path = path(label);
    std::fs::write(&path, bytes).unwrap();
    let mut xfer = XferLoad::new();
    xfer.open(path.to_string_lossy().into_owned()).unwrap();
    let result = xfer_partition_state_with_owner(owner, size, 23, &mut xfer);
    xfer.close().unwrap();
    std::fs::remove_file(path).unwrap();
    result
}
fn manager() -> ShroudManager {
    let mut owner = ShroudManager::new();
    owner
        .init_shroud_grid_at_origin_with_cell_size([110., -230.], 80., 40., 20.)
        .unwrap();
    owner.do_shroud_reveal(&crate::common::Coord3D::new(120., -220., 0.), 20., 2);
    owner.queue_undo_shroud_reveal(&crate::common::Coord3D::new(120., -220., 0.), 20., 2, 30, 0);
    owner
}
fn assert_unchanged(owner: &ShroudManager, before: &ShroudSnapshot, scalar: f32) {
    assert_eq!(owner.snapshot_state(), *before);
    assert_eq!(owner.grid_world_origin(), Some([110., -230.]));
    assert_eq!(owner.grid_dimensions(), Some((4, 2, 20.)));
    assert_eq!(scalar, 91.);
}
#[test]
fn actual_partition_xfer_roundtrip_uses_receiving_grid_and_preserves_same_thread_peer() {
    let mut source = manager();
    let mut source_size = 20.;
    let saved = source.snapshot_state();
    let bytes = save("roundtrip", &mut source, &mut source_size);
    assert_eq!(bytes[0], 2);
    assert_eq!(f32::from_le_bytes(bytes[1..5].try_into().unwrap()), 20.);
    assert_eq!(i32::from_le_bytes(bytes[5..9].try_into().unwrap()), 8);
    let mut receiving = manager();
    let mut size = 91.;
    let mut peer = ShroudManager::new();
    peer.init_shroud_grid_at_origin_with_cell_size([-90., 50.], 80., 40., 10.)
        .unwrap();
    let peer_before = peer.snapshot_state();
    load("roundtrip_load", &bytes, &mut receiving, &mut size).unwrap();
    assert_eq!(size, 20.);
    assert_eq!(receiving.snapshot_state(), saved);
    assert_eq!(receiving.grid_world_origin(), Some([110., -230.]));
    for frame in [30, 31] {
        source.process_pending_undo_shroud_reveals(frame);
        receiving.process_pending_undo_shroud_reveals(frame);
        assert_eq!(source.snapshot_state(), receiving.snapshot_state());
    }
    assert_eq!(peer.snapshot_state(), peer_before);
    assert_eq!(peer.grid_world_origin(), Some([-90., 50.]));
}
#[test]
fn invalid_partition_headers_and_late_queue_errors_leave_both_owners_unchanged() {
    let mut source = manager();
    let bytes = save("bad_source", &mut source, &mut 20.);
    let mut receiving = manager();
    let before = receiving.snapshot_state();
    let queue_offset = 9 + 8 * (1 + 4 * crate::common::MAX_PLAYER_COUNT);
    let mut cases = Vec::new();
    for cell in [f32::NAN, f32::INFINITY, 0., 21.] {
        let mut bad = bytes.clone();
        bad[1..5].copy_from_slice(&cell.to_le_bytes());
        cases.push(bad);
    }
    for count in [-1i32, 9, i32::MAX] {
        let mut bad = bytes.clone();
        bad[5..9].copy_from_slice(&count.to_le_bytes());
        cases.push(bad);
    }
    let mut truncated = bytes.clone();
    truncated.pop();
    cases.push(truncated);
    let mut bad_queue = bytes.clone();
    bad_queue[queue_offset..queue_offset + 4].copy_from_slice(&i32::MAX.to_le_bytes());
    cases.push(bad_queue);
    let mut nan_queue = bytes.clone();
    nan_queue[queue_offset + 5..queue_offset + 9].copy_from_slice(&f32::NAN.to_le_bytes());
    cases.push(nan_queue);
    for (index, bytes) in cases.into_iter().enumerate() {
        let mut size = 91.;
        assert!(load(&format!("bad_{index}"), &bytes, &mut receiving, &mut size).is_err());
        assert_unchanged(&receiving, &before, size);
    }
}
#[test]
fn empty_mapless_roundtrip_and_v1_queue_semantics_do_not_invent_geometry() {
    let mut empty = ShroudManager::new();
    let bytes = save("empty", &mut empty, &mut 40.);
    let mut receiving = ShroudManager::new();
    let mut size = 40.;
    load("empty_load", &bytes, &mut receiving, &mut size).unwrap();
    assert!(!receiving.has_shroud_grid());
    let zero = save("empty_zero", &mut ShroudManager::new(), &mut 0.);
    load("empty_zero_load", &zero, &mut ShroudManager::new(), &mut 0.).unwrap();
    let mut nonempty_wire = bytes.clone();
    nonempty_wire[5..9].copy_from_slice(&1i32.to_le_bytes());
    assert!(load("no_geometry", &nonempty_wire, &mut receiving, &mut size).is_err());
    assert!(!receiving.has_shroud_grid());
    let mut source = manager();
    let mut v1 = save("v1", &mut source, &mut 20.);
    let end = 9 + 8 * (1 + 4 * crate::common::MAX_PLAYER_COUNT);
    v1[0] = 1;
    v1.truncate(end);
    let mut target = manager();
    let queue = target.snapshot_state().pending_undo_shroud_reveals;
    load("v1_load", &v1, &mut target, &mut 20.).unwrap();
    assert_eq!(target.snapshot_state().pending_undo_shroud_reveals, queue);
    assert_eq!(target.grid_world_origin(), Some([110., -230.]));
}
