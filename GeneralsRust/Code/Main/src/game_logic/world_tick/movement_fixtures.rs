//! Admitted movement fixtures and route safety checks for the host tests.
use super::*;

pub(super) fn ranger_at(id: u32, pos: Vec3) -> Object {
    let mut tmpl = ThingTemplate::new("Ranger");
    tmpl.add_kind_of(KindOf::Infantry);
    let mut unit = Object::new(tmpl, ObjectId(id), Team::USA);
    unit.set_position(pos);
    unit
}

// A current locomotor is required by C++ isDoingGroundMovement; appearance
// and surface flags alone do not admit a mobile unit.
pub(super) fn bind_fixture_locomotor(unit: &mut Object, name: &str) {
    use crate::game_logic::locomotor_bootstrap::{
        apply_host_locomotor_binding, resolve_host_locomotor_binding,
    };
    let binding = resolve_host_locomotor_binding(name).expect("authored fixture locomotor");
    unit.locomotor_set_names = vec![name.to_owned()];
    unit.cur_locomotor_name = Some(name.to_owned());
    apply_host_locomotor_binding(unit, &binding);
    assert_eq!(unit.cur_locomotor_name.as_deref(), Some(name));
    assert_eq!(unit.locomotor_surfaces, binding.locomotor_surfaces);
}

pub(super) fn assert_reachable_side_of_wall(logic: &GameLogic, unit: &Object, wall_x: i32) {
    let grid = &logic.pathfinding_system.grid;
    // The request normalizes an unspecified surface mask to ground. An
    // adjusted goal may be reachable without attempting findClosestPath.
    let surfaces = if unit.locomotor_surfaces == 0 {
        gamelogic::ai::pathfind_complete::SURFACE_GROUND
    } else {
        unit.locomotor_surfaces
    };
    assert!(
        !unit.movement.path.is_empty(),
        "reachable prefix must be retained"
    );
    for point in &unit.movement.path {
        let cell = grid.world_to_grid(*point);
        assert!(
            cell.x < wall_x,
            "path node crossed sealed column: {point:?}"
        );
        assert!(
            grid.cell_passable_for_ignoring(cell, surfaces, false, Some(unit.id.0)),
            "path node is impassable: {cell:?}"
        );
    }
    for segment in unit.movement.path.windows(2) {
        let steps = ((segment[1] - segment[0]).length() / (grid.grid_size() * 0.25))
            .ceil()
            .max(1.0) as usize;
        for step in 0..=steps {
            let point = segment[0].lerp(segment[1], step as f32 / steps as f32);
            let cell = grid.world_to_grid(point);
            assert!(cell.x < wall_x, "route crossed sealed column: {point:?}");
            assert!(
                grid.cell_passable_for_ignoring(cell, surfaces, false, Some(unit.id.0)),
                "closest route contains an impassable cell: {cell:?}"
            );
        }
    }
    if let Some(target) = unit.movement.target_position {
        assert!(
            grid.world_to_grid(target).x < wall_x,
            "direct target crosses wall"
        );
    }
}

pub(super) fn seal_column(logic: &mut GameLogic, cell_x: i32) {
    // Cover every valid row, independent of world size and grid resolution.
    for y in 0..logic.pathfinding_system.grid.height() {
        logic
            .pathfinding_system
            .grid
            .set_blocked(GridPos::new(cell_x, y), true);
    }
}
