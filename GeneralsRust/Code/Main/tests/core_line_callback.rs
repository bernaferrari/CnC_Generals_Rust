use gamelogic::ai::pathfind_astar::{GridCoord, PathfindCellType, PathfindLayerEnum};
use gamelogic::ai::pathfind_complete::{PathfindingSystem, SURFACE_GROUND, SURFACE_WATER};
use glam::Vec3;

#[test]
fn core_line_uses_clear_deck_shortcut_after_first_cell() {
    // hq-9tdj6w: PathfindingSystem::new installs then clears WorldServices; bind one
    // for the clear-deck line callback residual.
    let services = std::sync::Arc::new(gamelogic::system::engine_stores::WorldServices::new_for_world());
    gamelogic::system::engine_stores::with_world_services(&services, || {
        let mut owner = PathfindingSystem::new(20, 20);
        let mut peer = PathfindingSystem::new(20, 20);
        let bounds = (GridCoord::new(5, 4), GridCoord::new(11, 6));
        let bridge = PathfindLayerEnum::from_u32(owner.add_bridge(bounds));
        assert_eq!(peer.add_bridge(bounds), bridge as u32);
        let from = Vec3::new(65.0, 55.0, 0.0);
        let to = Vec3::new(85.0, 55.0, 0.0);
        owner.set_cell_type_at_layer(&from, bridge, PathfindCellType::Water);

        assert!(owner.is_line_passable_for_surfaces_on_layer(&from, &to, SURFACE_WATER, bridge, None));
        assert!(!peer.is_line_passable_for_surfaces_on_layer(&from, &to, SURFACE_WATER, bridge, None));
        assert!(owner.is_line_passable_for_surfaces_on_layer(&from, &to, SURFACE_WATER, bridge, None));
        assert!(peer.is_line_passable_for_surfaces_on_layer(&from, &to, SURFACE_GROUND, bridge, None));
        let shore = Vec3::new(45.0, 45.0, 0.0);
        let entry = Vec3::new(55.0, 45.0, 0.0);
        owner.set_cell_type(&shore, PathfindCellType::Water);
        assert!(!owner.is_line_passable_for_surfaces_on_layer(
            &shore,
            &entry,
            SURFACE_WATER,
            bridge,
            None
        ));
    });
}

#[test]
fn core_zero_length_line_checks_the_first_cell() {
    let finder = PathfindingSystem::new(20, 20);
    let cell = Vec3::new(65.0, 55.0, 0.0);
    assert!(!finder.is_line_passable_for_surfaces_on_layer(
        &cell,
        &cell,
        SURFACE_WATER,
        PathfindLayerEnum::Ground,
        None
    ));
    assert!(finder.is_line_passable_for_surfaces_on_layer(
        &cell,
        &cell,
        SURFACE_GROUND,
        PathfindLayerEnum::Ground,
        None
    ));
}

#[test]
fn core_line_stops_successfully_at_the_map_boundary() {
    let finder = PathfindingSystem::new(20, 20);
    let from = Vec3::new(195.0, 55.0, 0.0);
    let to = Vec3::new(205.0, 55.0, 0.0);
    assert!(finder.is_line_passable_for_surfaces_on_layer(
        &from,
        &to,
        SURFACE_GROUND,
        PathfindLayerEnum::Ground,
        None
    ));
}

#[test]
fn core_line_checks_the_original_intermediate_cell() {
    let mut finder = PathfindingSystem::new(20, 20);
    finder.set_cell_type(&Vec3::new(65.0, 65.0, 0.0), PathfindCellType::Obstacle);
    let from = Vec3::new(65.0, 55.0, 0.0);
    let to = Vec3::new(85.0, 65.0, 0.0);
    assert!(!finder.is_line_passable_for_surfaces_on_layer(&from, &to, SURFACE_GROUND, PathfindLayerEnum::Ground, None));
}
