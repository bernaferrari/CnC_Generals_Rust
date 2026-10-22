use super::*;
#[test]
fn supplied_geometry_survives_clone_and_receiving_restore_without_peer_selection() {
    let mut a = ShroudManager::new();
    let mut b = ShroudManager::new();
    a.init_shroud_grid_at_origin_with_cell_size([123., -87.], 101., 51., 17.)
        .unwrap();
    b.init_shroud_grid_at_origin_with_cell_size([-400., 230.], 120., 80., 20.)
        .unwrap();
    assert_eq!(a.grid_dimensions(), Some((6, 3, 17.)));
    assert_eq!(b.grid_dimensions(), Some((6, 4, 20.)));
    let peer_before = b.snapshot_state();
    let center = Coord3D::new(131.5, -78.5, 0.);
    a.do_shroud_reveal(&center, 17., 1 << 1);
    a.queue_undo_shroud_reveal(&center, 17., 1 << 1, 10, 0);
    let saved = a.snapshot_state();
    let mut cloned = a.clone();
    assert_eq!(cloned.grid_world_origin(), Some([123., -87.]));
    assert_eq!(cloned.snapshot_state(), saved);
    let mut receiving = ShroudManager::new();
    receiving
        .init_shroud_grid_at_origin_with_cell_size([123., -87.], 101., 51., 17.)
        .unwrap();
    receiving.replace_state_owned(&saved, 0).unwrap();
    assert_eq!(receiving.grid_dimensions(), Some((6, 3, 17.)));
    assert_eq!(receiving.grid_world_origin(), Some([123., -87.]));
    for frame in [10, 11] {
        for owner in [&mut a, &mut cloned, &mut receiving] {
            owner.process_pending_undo_shroud_reveals(frame);
        }
        assert_eq!(a.snapshot_state(), receiving.snapshot_state());
        assert_eq!(a.snapshot_state(), cloned.snapshot_state());
        assert_eq!(b.snapshot_state(), peer_before);
    }
}
#[test]
fn finite_cpp_clamps_and_invalid_initializers_preserve_owner_and_queues() {
    let mut owner = ShroudManager::new();
    owner
        .init_shroud_grid_at_origin_with_cell_size([2., 3.], -5., 0., -20.)
        .unwrap();
    assert_eq!(owner.grid_dimensions(), Some((1, 1, 1.)));
    owner.pending_full_reveal_players.insert(2);
    let before = owner.snapshot_state();
    for (origin, width, height, cell) in [
        ([f32::NAN, 0.], 10., 10., 1.),
        ([0., 0.], f32::INFINITY, 10., 1.),
        ([0., 0.], 10., 10., f32::NAN),
        ([0., 0.], 100_000., 100_000., 1.),
    ] {
        assert!(
            owner
                .init_shroud_grid_at_origin_with_cell_size(origin, width, height, cell)
                .is_err()
        );
        assert_eq!(owner.snapshot_state(), before);
        assert_eq!(owner.grid_world_origin(), Some([2., 3.]));
    }
}
#[test]
fn staged_shape_rejects_equal_total_different_shape_or_size_before_mutation() {
    let mut source = ShroudManager::new();
    source
        .init_shroud_grid_at_origin_with_cell_size([10., 20.], 80., 40., 20.)
        .unwrap();
    let mut receiving = source.clone();
    let saved = source.snapshot_state();
    for (width, height, size) in [(2, 4, 20.), (4, 2, 21.)] {
        let mut bad = saved.clone();
        let grid = bad.grid.as_mut().unwrap();
        grid.width = width;
        grid.height = height;
        grid.cell_size = size;
        assert!(receiving.replace_state_owned(&bad, 99).is_err());
        assert_eq!(receiving.snapshot_state(), saved);
        assert_eq!(receiving.grid_world_origin(), Some([10., 20.]));
    }
    // Existing mapless saved-grid support remains explicit; the snapshot has
    // no origin wire field and retains its historical origin-zero behavior.
    let mut mapless = ShroudManager::new();
    mapless.replace_state_owned(&saved, 0).unwrap();
    assert_eq!(mapless.snapshot_state(), saved);
    assert_eq!(mapless.grid_world_origin(), Some([0., 0.]));
}
