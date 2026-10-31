use gamelogic::common::Coord3D;
use gamelogic::object::collide::partition_manager::ValueOrThreat;
use gamelogic::system::shroud_manager::ShroudManager;

fn grid() -> ShroudManager {
    let mut manager = ShroudManager::new();
    manager
        .init_shroud_grid_at_origin_with_cell_size([0.0, 0.0], 40.0, 40.0, 10.0)
        .unwrap();
    manager
}

#[test]
fn off_map_scalar_circles_return_and_leave_both_worlds_unchanged() {
    let mut owner = grid();
    let peer = grid();
    let outside = Coord3D::new(-100.0, 15.0, 0.0);
    let inside = Coord3D::new(5.0, 15.0, 0.0);
    owner.do_value_affect(&outside, 10.0, 100, 1);
    owner.undo_value_affect(&outside, 10.0, 100, 1);
    owner.do_threat_affect(&outside, 10.0, 100, 1);
    owner.undo_threat_affect(&outside, 10.0, 100, 1);
    for kind in [ValueOrThreat::CashValue, ValueOrThreat::ThreatValue] {
        assert_eq!(owner.host_scalar_value_at(&inside, 0, kind), Some(0));
        assert_eq!(peer.host_scalar_value_at(&inside, 0, kind), Some(0));
    }
}

#[test]
fn partially_clipped_scalar_circles_apply_and_undo_only_in_their_owner() {
    let mut owner = grid();
    let peer = grid();
    let edge = Coord3D::new(-5.0, 15.0, 0.0);
    let inside = Coord3D::new(5.0, 15.0, 0.0);
    owner.do_value_affect(&edge, 20.0, 100, 1);
    owner.do_threat_affect(&edge, 20.0, 100, 1);
    for kind in [ValueOrThreat::CashValue, ValueOrThreat::ThreatValue] {
        assert!(owner.host_scalar_value_at(&inside, 0, kind).unwrap() > 0);
        assert_eq!(peer.host_scalar_value_at(&inside, 0, kind), Some(0));
    }
    owner.undo_value_affect(&edge, 20.0, 100, 1);
    owner.undo_threat_affect(&edge, 20.0, 100, 1);
    for kind in [ValueOrThreat::CashValue, ValueOrThreat::ThreatValue] {
        assert_eq!(owner.host_scalar_value_at(&inside, 0, kind), Some(0));
    }
}
