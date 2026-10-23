//! Original off-map admission observes each receiving map's maximum boundary.
use super::*;
fn logical(maximum: i32) -> gamelogic::terrain::TerrainLogic {
    let mut map = gamelogic::system::map_loader::MapData::new();
    map.width = 64;
    map.height = 64;
    map.heightmap = vec![0; 64 * 64];
    map.boundaries = vec![gamelogic::common::ICoord2D::new(maximum, maximum)];
    let mut terrain = gamelogic::terrain::TerrainLogic::new();
    assert!(terrain.load_map_geometry(map).is_empty());
    terrain
}
fn system() -> PathfindingSystem {
    let mut system = PathfindingSystem::new(640., 640.);
    for y in 0..system.grid.height() {
        system
            .grid
            .set_cell_type(GridPos::new(30, y), PathfindCellType::Obstacle);
    }
    system.grid.rebuild_terrain_zones();
    system
}
#[test]
fn receiving_maximum_boundary_changes_real_offmap_path_without_foreign_selection() {
    let mut a = logical(20);
    let mut b = logical(40);
    let mut grid_a = system();
    let mut grid_b = system();
    let start = Vec3::new(250., 0., 100.);
    let goal = Vec3::new(350., 0., 100.);
    let foreign = gamelogic::system::engine_stores::new_for_world();
    *foreign.services().terrain().write().unwrap() = logical(40);
    gamelogic::system::engine_stores::with_active_stores(&foreign, || {
        let ground_a = PathfindingGround::receiving(&a);
        let ground_b = PathfindingGround::receiving(&b);
        assert!(grid_a.leftover_should_force_direct_path_for_off_map_start(start, goal, &ground_a));
        assert!(
            !grid_b.leftover_should_force_direct_path_for_off_map_start(start, goal, &ground_b)
        );
        let objects = HostObjectStore::new();
        assert_eq!(
            grid_a.find_path_ex(start, goal, objects.read(), false, None, &ground_a),
            Some(PathfindingSystem::leftover_compute_quick_path_nodes(
                start, goal
            ))
        );
        assert!(
            grid_b
                .find_path_ex(start, goal, objects.read(), false, None, &ground_b)
                .is_none(),
            "receiving on-map start must honor the sealed pathfind wall"
        );
    });
    b.reset();
    assert!(grid_a.leftover_should_force_direct_path_for_off_map_start(
        start,
        goal,
        &PathfindingGround::receiving(&a)
    ));
    a.reset();
    // Preserve the adapter's existing not-ready/mapless grid branch. This is
    // explicit grid fixture policy, not a fabricated loaded maximum boundary.
    let mapless = PathfindingGround::Flat(0.);
    assert!(!grid_a.leftover_should_force_direct_path_for_off_map_start(start, goal, &mapless));
    assert!(grid_a.leftover_should_force_direct_path_for_off_map_start(
        Vec3::new(-100., 0., -100.),
        Vec3::new(-50., 0., -25.),
        &mapless
    ));
}
