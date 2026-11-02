use gamelogic::ai::AI;
use gamelogic::ai::pathfind_astar::PathfindCellType;
use gamelogic::ai::pathfind_complete::{SURFACE_AIR, SURFACE_GROUND, SURFACE_RUBBLE};
use gamelogic::locomotor::{Locomotor, LocomotorSet, LocomotorTemplate};
use glam::Vec3;
use std::sync::Arc;

#[test]
fn core_pathfinder_preserves_the_callers_rubble_surface() {
    let owner = AI::new();
    let peer = AI::new();
    let owner_pathfinder = owner.pathfinder().unwrap();
    let peer_pathfinder = peer.pathfinder().unwrap();
    let pos = Vec3::new(25.0, 25.0, 0.0);
    for finder in [&owner_pathfinder, &peer_pathfinder] {
        finder.write().unwrap().reset_with_size(8, 8);
    }
    owner_pathfinder
        .write()
        .unwrap()
        .set_cell_type_at(&pos, PathfindCellType::Rubble);

    let owner = owner_pathfinder.read().unwrap();
    let peer = peer_pathfinder.read().unwrap();
    for crusher in [false, true] {
        assert!(!owner.valid_movement_position_for_surfaces(SURFACE_GROUND, crusher, &pos, None));
        assert!(owner.valid_movement_position_for_surfaces(SURFACE_RUBBLE, crusher, &pos, None));
        assert!(owner.valid_movement_position_for_surfaces(SURFACE_AIR, crusher, &pos, None));
        assert!(peer.valid_movement_position_for_surfaces(SURFACE_GROUND, crusher, &pos, None));
    }

    let mut ground = LocomotorTemplate::new("ground".into());
    ground.surfaces = SURFACE_GROUND;
    let mut rubble = LocomotorTemplate::new("rubble".into());
    rubble.surfaces = SURFACE_RUBBLE;
    let mut set = LocomotorSet::new();
    set.add_locomotor("ground".into(), Locomotor::new(Arc::new(ground)));
    set.add_locomotor("rubble".into(), Locomotor::new(Arc::new(rubble)));
    assert!(owner.valid_movement_position(&set, false, &pos, None));
}

#[test]
fn core_pathfinder_uses_the_locomotor_sets_surface_mask() {
    let owner = AI::new();
    let pathfinder = owner.pathfinder().unwrap();
    let pos = Vec3::new(25.0, 25.0, 0.0);
    let mut pathfinder = pathfinder.write().unwrap();
    pathfinder.reset_with_size(8, 8);
    pathfinder.set_cell_type_at(&pos, PathfindCellType::Rubble);
    for crusher in [false, true] {
        assert!(!pathfinder.valid_movement_position(
            &LocomotorSet::from_surfaces(SURFACE_GROUND),
            crusher,
            &pos,
            None,
        ));
        assert!(pathfinder.valid_movement_position(
            &LocomotorSet::from_surfaces(SURFACE_RUBBLE),
            crusher,
            &pos,
            None,
        ));
    }
}
