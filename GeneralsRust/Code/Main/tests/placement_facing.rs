use generals_main::ui::hud::ConstructionPanel;

#[test]
fn placement_drag_replaces_facing_and_finish_preserves_it() {
    let mut panel = ConstructionPanel::new(0, 0);
    let mut peer = ConstructionPanel::new(0, 0);
    panel.arm_structure_placement_with_facts("PlacementFixture".into(), 0.0, (30.0, 30.0));
    peer.arm_structure_placement_with_facts("PlacementFixture".into(), 0.0, (30.0, 30.0));

    let first_drag = game_engine::common::system::geometry::coord2d_to_angle(3.0, 4.0);
    let final_drag = game_engine::common::system::geometry::coord2d_to_angle(3.0, -4.0);
    panel.set_structure_placement_facing(first_drag);
    panel.set_structure_placement_facing(final_drag);
    peer.set_structure_placement_facing(first_drag);

    assert_eq!(panel.placement_preview().facing_radians, final_drag);
    assert_eq!(panel.finish_structure_placement(), final_drag);
    assert!(panel.pending_structure_placement().is_none());
    assert!(!panel.placement_preview().is_active());
    assert_eq!(peer.placement_preview().facing_radians, first_drag);
}
