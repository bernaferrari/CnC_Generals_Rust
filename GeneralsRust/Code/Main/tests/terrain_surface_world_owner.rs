use game_engine::common::system::DataChunkOutput;
use generals_main::game_logic::{GameLogic, GameMode};
use glam::Vec3;

fn map(root: &std::path::Path, height: u8) -> std::path::PathBuf {
    let mut out = DataChunkOutput::new();
    out.open_data_chunk("HeightMapData", 3);
    for value in [4, 4, 0, 16] {
        out.write_int(value);
    }
    for _ in 0..16 {
        out.write_byte(height);
    }
    out.close_data_chunk();
    let path = root.join("Surface.map");
    std::fs::write(&path, out.into_ckmp_bytes()).unwrap();
    path
}

#[test]
fn surface_height_uses_each_receiving_map_across_peer_reset() {
    let first_dir = tempfile::tempdir().unwrap();
    let peer_dir = tempfile::tempdir().unwrap();
    let first_map = map(first_dir.path(), 8);
    let peer_map = map(peer_dir.path(), 16);
    let mut first = GameLogic::new();
    first.start_new_game(GameMode::SinglePlayer);
    assert!(first.load_map(first_map.to_str().unwrap()));
    let mut peer = GameLogic::new();
    peer.start_new_game(GameMode::SinglePlayer);
    assert!(peer.load_map(peer_map.to_str().unwrap()));

    let position = Vec3::new(15.0, 0.0, 15.0);
    assert_eq!(first.surface_ht_at(position), Some(5.0));
    assert_eq!(peer.surface_ht_at(position), Some(10.0));
    peer.reset();
    assert_eq!(first.surface_ht_at(position), Some(5.0));
}
