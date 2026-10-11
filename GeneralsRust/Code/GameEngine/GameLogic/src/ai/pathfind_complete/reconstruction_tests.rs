//! Source witnesses for AIPathfind.cpp:8934-9070 on the actual A* grid and
//! reconstruction implementation. Terrain heights are supplied observations.
use super::*;

fn build(
    system: &mut PathfindingSystem,
    cells: &[(GridCoord, PathfindLayerEnum)],
    feet: Coord3D,
    center: bool,
    sample: impl FnMut(f32, f32, CommonPathfindLayerEnum) -> f32,
) -> PathResult {
    system.reconstruct_layered_path(
        cells,
        &feet,
        &Coord3D::new(999.0, 888.0, 777.0),
        SURFACE_GROUND,
        false,
        false,
        center,
        INVALID_ID,
        None,
        sample,
    )
}

#[test]
fn reconstruction_snaps_goal_stamps_every_reverse_cell_and_keeps_exact_feet() {
    let mut system = PathfindingSystem::new(32, 8);
    let cells: Vec<_> = (0..21)
        .map(|x| (GridCoord::new(x, 0), PathfindLayerEnum::Ground))
        .collect();
    system.pathfinder.mark_all_zone_blocks_impassable();
    // Less than the removed tolerance still needs a native feet node.
    let feet = Coord3D::new(
        1.5 * PATHFIND_CELL_SIZE_F + 0.005,
        0.5 * PATHFIND_CELL_SIZE_F,
        19.0,
    );
    let path = build(&mut system, &cells, feet, true, |_, _, _| 7.0);
    assert!(path.success);
    assert_eq!(path.waypoints.len(), 21);
    assert_eq!(path.waypoints[0].x, feet.x);
    assert_eq!(path.waypoints[0].z, 19.0);
    assert_eq!(path.waypoints[1].x, 1.5 * PATHFIND_CELL_SIZE_F);
    assert_eq!(path.waypoints[20].x, 20.5 * PATHFIND_CELL_SIZE_F);
    assert_eq!(path.waypoints[20].z, 7.0);
    for (cell, _) in cells {
        assert!(system.pathfinder.is_zone_passable(cell));
    }
}

#[test]
fn reconstruction_parentless_corner_uses_cell_fraction_and_exact_layer_height() {
    let mut system = PathfindingSystem::new(8, 8);
    let layer = PathfindLayerEnum::Top;
    let cells = [(GridCoord::new(2, 3), layer)];
    let mut samples = Vec::new();
    let feet = Coord3D::new(
        2.05 * PATHFIND_CELL_SIZE_F,
        3.05 * PATHFIND_CELL_SIZE_F,
        -1.0,
    );
    let path = build(&mut system, &cells, feet, false, |x, y, layer| {
        samples.push((x, y, layer));
        31.0
    });
    assert_eq!(path.waypoints.len(), 1);
    assert_eq!(path.waypoints[0].x, feet.x);
    assert_eq!(path.waypoints[0].y, feet.y);
    assert_eq!(path.waypoints[0].z, 31.0);
    assert_eq!(path.layers, vec![layer]);
    assert_eq!(samples.len(), 1);
    assert_eq!(samples[0].2 as u32, layer as u32);
}

#[test]
fn reconstruction_layer_continue_retains_previous_cliff_and_feet_start_layer() {
    let mut system = PathfindingSystem::new(8, 8);
    let ground = PathfindLayerEnum::Ground;
    let bridge = PathfindLayerEnum::Top;
    let cells = [
        (GridCoord::new(0, 0), ground),
        (GridCoord::new(1, 0), ground),
        (GridCoord::new(2, 0), ground),
        (GridCoord::new(2, 0), bridge),
        (GridCoord::new(3, 0), bridge),
    ];
    system
        .pathfinder
        .set_cell_type_on_layer(GridCoord::new(2, 0), bridge, PathfindCellType::Cliff);
    system
        .pathfinder
        .set_cell_type_on_layer(GridCoord::new(3, 0), bridge, PathfindCellType::Clear);
    let path = build(&mut system, &cells, Coord3D::ZERO, true, |_, _, layer| {
        layer as u32 as f32
    });
    assert_eq!(path.waypoints.len(), 4);
    assert_eq!(path.layers, vec![ground, ground, bridge, bridge]);
    // Reverse goal-clear -> cliff forbids optimization of the goal; the
    // duplicate ground cell's continue must not erase that previous cliff.
    assert_eq!(path.can_optimize, vec![true, false, true, false]);
    assert!(system.pathfinder.is_zone_passable(GridCoord::new(2, 0)));
}

#[test]
fn reconstruction_trims_pinched_goal_only_when_parent_is_not_pinched() {
    let mut system = PathfindingSystem::new(32, 8);
    let cells: Vec<_> = (0..21)
        .map(|x| (GridCoord::new(x, 0), PathfindLayerEnum::Ground))
        .collect();
    system.pathfinder.mark_all_zone_blocks_impassable();
    system.pathfinder.set_pinched(GridCoord::new(20, 0), true);
    let trimmed = build(&mut system, &cells, Coord3D::ZERO, true, |_, _, _| 0.0);
    assert_eq!(
        trimmed.waypoints.last().unwrap().x,
        19.5 * PATHFIND_CELL_SIZE_F
    );
    assert!(!system.pathfinder.is_zone_passable(GridCoord::new(20, 0)));
    system.pathfinder.set_pinched(GridCoord::new(19, 0), true);
    let retained = build(&mut system, &cells, Coord3D::ZERO, true, |_, _, _| 0.0);
    assert_eq!(
        retained.waypoints.last().unwrap().x,
        20.5 * PATHFIND_CELL_SIZE_F
    );
    assert!(system.pathfinder.is_zone_passable(GridCoord::new(20, 0)));
}
