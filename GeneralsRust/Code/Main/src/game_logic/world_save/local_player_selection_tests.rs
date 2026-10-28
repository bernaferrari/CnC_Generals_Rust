//! The local view is one receiving-session identity, independent of human roster size.
use super::*;

#[test]
fn local_player_selection_is_single_world_owned_and_survives_receiving_restore() {
    use crate::save_load::{SnapshotBuilder, snapshot::decode_bincode_world_snapshot};

    let mut saved_world = GameLogic::new();
    saved_world.add_player(
        Player::new(7, Team::USA, "same display", true).with_script_identity("player7"),
    );
    saved_world.add_player(
        Player::new(19, Team::China, "same display", true).with_script_identity("player19"),
    );
    // C++ newGame chooses the first human unless a later explicit local
    // override is authored. The override is a world selection, not a Player
    // wire field.
    assert_eq!(saved_world.local_player_id(), Some(7));
    assert!(saved_world.select_local_player(Some(19)));
    assert_eq!(saved_world.local_player_id(), Some(19));
    assert!(!saved_world.select_local_player(Some(999)));
    assert_eq!(saved_world.local_player_id(), Some(19));
    assert_eq!(usize::from(saved_world.local_player_id().is_some()), 1);

    let mut peer = GameLogic::new();
    peer.add_player(
        Player::new(7, Team::USA, "same display", true).with_script_identity("player7"),
    );
    peer.add_player(
        Player::new(19, Team::China, "same display", false).with_script_identity("player19"),
    );
    assert_eq!(peer.local_player_id(), Some(7));
    assert_eq!(saved_world.local_player_id(), Some(19));

    let builder = SnapshotBuilder::new();
    let snapshot = builder.create_world_snapshot(&saved_world).unwrap();
    let encoded = bincode_legacy::serialize(&snapshot).unwrap();
    let decoded = decode_bincode_world_snapshot(&encoded).unwrap();

    // The receiving world owns local identity across restore. Saved local
    // view state is intentionally absent from PlayerSnapshot.
    builder.restore_from_snapshot(&decoded, &mut peer).unwrap();
    assert_eq!(peer.local_player_id(), Some(7));
    assert_eq!(usize::from(peer.local_player_id().is_some()), 1);

    peer.reset();
    assert_eq!(peer.local_player_id(), None);
    assert_eq!(saved_world.local_player_id(), Some(19));
}
