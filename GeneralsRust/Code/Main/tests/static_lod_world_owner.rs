use game_engine::common::ini::ini_game_data::TerrainTrackRules;
use game_engine::common::ini::ini_game_lod::StaticGameLODLevel;
use generals_main::game_logic::GameLogic;

fn rules(edges: i32) -> TerrainTrackRules {
    TerrainTrackRules {
        max_terrain_tracks: 2,
        max_tank_track_edges: edges,
        max_tank_track_opaque_edges: edges / 2,
        max_tank_track_fade_delay: edges * 10,
        make_track_marks: true,
    }
}

#[test]
fn static_lod_same_level_and_custom_reapply_are_owned_by_each_world() {
    let mut first = GameLogic::new();
    let mut peer = GameLogic::new();

    assert!(first.apply_receiving_static_lod(StaticGameLODLevel::Low, rules(11)));
    assert!(peer.apply_receiving_static_lod(StaticGameLODLevel::Low, rules(7)));

    // A's High must not make B's same High look already applied.
    assert!(first.apply_receiving_static_lod(StaticGameLODLevel::High, rules(20)));
    assert!(peer.apply_receiving_static_lod(StaticGameLODLevel::High, rules(20)));
    assert!(!first.apply_receiving_static_lod(StaticGameLODLevel::High, rules(20)));
    assert!(!peer.apply_receiving_static_lod(StaticGameLODLevel::High, rules(20)));

    let first_rules = first.effective_terrain_track_rules();
    let peer_rules = peer.effective_terrain_track_rules();
    assert!(peer.apply_receiving_static_lod(StaticGameLODLevel::Custom, rules(3)));
    assert!(peer.apply_receiving_static_lod(StaticGameLODLevel::Custom, rules(3)));

    assert_eq!(first.effective_terrain_track_rules(), first_rules);
    assert_eq!(peer.effective_terrain_track_rules(), peer_rules);
}
