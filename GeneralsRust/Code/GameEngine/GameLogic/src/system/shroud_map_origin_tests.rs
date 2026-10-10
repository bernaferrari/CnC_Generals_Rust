use super::*;

#[test]
fn map_origin_drives_reveal_cover_and_restored_expiry_without_wire_fields() {
    // CPP PartitionManager.h:1506, init:2565 and xfer into allocated cells.
    for origin in [[120.0, -160.0], [-280.0, 400.0]] {
        let center = Coord3D::new(origin[0] + 20.0, origin[1] + 20.0, 0.0);
        let edge = Coord3D::new(origin[0] - 20.0, center.y, 0.0);
        let mut source = ShroudManager::new();
        source.init_shroud_grid_at_origin(origin, 200.0, 200.0);
        source.do_shroud_reveal(&edge, 80.0, 1 << 1);
        source.do_shroud_reveal(&center, 80.0, 1 << 1);
        source.queue_undo_shroud_reveal(&edge, 80.0, 1 << 1, 150, 0);
        source.queue_undo_shroud_reveal(&center, 80.0, 1 << 1, 150, 0);
        assert_eq!(source.get_shroud_state(1, &center), ShroudState::Visible);
        source.do_shroud_cover(&center, 80.0, 1 << 1);
        // CPP addShrouder only changes an explored cell (counter zero).
        // Both active lookers keep their negative counter and visibility.
        assert_eq!(source.get_shroud_state(1, &center), ShroudState::Visible);
        source.undo_shroud_cover(&center, 80.0, 1 << 1);
        assert_eq!(source.get_shroud_state(1, &center), ShroudState::Visible);
        let saved = source.snapshot_state();
        let mut restored = ShroudManager::new();
        restored.init_shroud_grid_at_origin(origin, 200.0, 200.0);
        restored.replace_state(&saved, 0).unwrap();
        assert_eq!(restored.grid_world_origin(), Some(origin));
        assert_eq!(restored.snapshot_state(), saved);
        let mut bad = saved.clone();
        bad.grid.as_mut().unwrap().cells.pop();
        assert!(restored.replace_state(&bad, 0).is_err());
        assert_eq!(restored.grid_world_origin(), Some(origin));
        assert_eq!(restored.snapshot_state(), saved);
        for frame in [150, 151] {
            for manager in [&mut source, &mut restored] {
                manager.process_pending_undo_shroud_reveals(frame);
                assert_eq!(
                    manager.get_shroud_state(1, &center),
                    if frame == 150 {
                        ShroudState::Visible
                    } else {
                        ShroudState::Explored
                    }
                );
            }
            assert_eq!(source.snapshot_state(), restored.snapshot_state());
        }
        for manager in [&mut source, &mut restored] {
            manager.do_shroud_cover(&center, 80.0, 1 << 1);
            assert_eq!(manager.get_shroud_state(1, &center), ShroudState::Hidden);
            manager.undo_shroud_cover(&center, 80.0, 1 << 1);
            assert_eq!(
                manager.get_shroud_state(1, &center),
                ShroudState::Hidden,
                "CPP removeShrouder does not manufacture explored state",
            );
        }
        assert_eq!(source.snapshot_state(), restored.snapshot_state());
        source.clear_all();
        assert_eq!(source.grid_world_origin(), None);
        assert_eq!(restored.grid_world_origin(), Some(origin));
    }
}
