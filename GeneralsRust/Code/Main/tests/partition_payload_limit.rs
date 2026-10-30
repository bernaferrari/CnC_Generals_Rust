use generals_main::game_logic::GameLogic;
use generals_main::save_load::SnapshotBuilder;

#[test]
fn oversized_partition_frame_is_rejected_before_reading_its_body() {
    let builder = SnapshotBuilder::new();
    let mut snapshot = builder.create_world_snapshot(&GameLogic::new()).unwrap();
    let matches: Vec<_> = snapshot
        .lifecycle_tail
        .windows(4)
        .enumerate()
        .filter_map(|(index, tag)| (tag == b"PCOI").then_some(index))
        .collect();
    assert_eq!(matches.len(), 1);
    let offset = matches[0];
    snapshot.lifecycle_tail[offset + 8..offset + 12]
        .copy_from_slice(&(512u32 * 1024 * 1024 + 1).to_le_bytes());
    let mut receiver = GameLogic::new();
    let mut peer = GameLogic::new();
    receiver.update();
    peer.update();
    peer.update();
    let receiver_frame = receiver.get_frame();
    let peer_frame = peer.get_frame();
    let error = builder
        .restore_from_snapshot(&snapshot, &mut receiver)
        .unwrap_err();
    assert!(
        error
            .to_string()
            .contains("lifecycle record payload truncated"),
        "{error}"
    );
    assert_eq!(receiver.get_frame(), receiver_frame);
    assert_eq!(peer.get_frame(), peer_frame);
}
