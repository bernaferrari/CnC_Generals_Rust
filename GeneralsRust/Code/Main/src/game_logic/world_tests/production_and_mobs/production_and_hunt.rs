//! Behavior suite extracted from `production_and_mobs`: production queues, dozer
//! and supply docks, repair pads, path extra distance, and ignore management.
use super::*;

fn assert_reserved_dock_approach_path(
    logic: &GameLogic,
    dock_id: crate::game_logic::ObjectId,
    docker_id: crate::game_logic::ObjectId,
    start: Vec3,
    dock_pos: Vec3,
    dock_radius: f32,
) {
    let (_, queue) = logic
        .host_dock_approach_queue_snapshot()
        .into_iter()
        .find(|(id, queue)| *id == dock_id && queue.index_of(docker_id).is_some())
        .expect("dock must retain the docker's registered approach slot");
    let slot = queue.index_of(docker_id).expect("reserved approach slot") as usize;
    let registered = queue.approach_world_position(slot, start, dock_pos, dock_radius);
    let unit = logic.host_object(docker_id).expect("docker");
    assert!(
        unit.is_approach_path,
        "dock request must keep approach mode"
    );
    assert_eq!(unit.requested_destination, Some(registered));
    assert!(unit.ignored_obstacle_id.is_none());
    let end = *unit.movement.path.last().expect("approach path endpoint");
    let endpoint_cell = logic.pathfinding_system.grid.world_to_grid(end);
    assert!(
        logic.pathfinding_system.grid.valid_movement_position(
            endpoint_cell,
            gamelogic::ai::pathfind_astar::PathfindLayerEnum::Ground,
            unit.locomotor_surfaces,
            unit.crusher_level > 0,
            0,
        ),
        "closest approach endpoint must be a legal ground cell, end={end:?}"
    );
    assert!(
        end.distance(registered) < start.distance(registered),
        "path endpoint must make progress toward registered approach {registered:?}, got {end:?}"
    );
    assert!(
        unit.movement.path.windows(2).all(|segment| logic
            .pathfinding_system
            .line_passable_for_surfaces(segment[0], segment[1], unit.locomotor_surfaces)),
        "dock approach path must not cross the dock obstacle: {:?}",
        unit.movement.path
    );
}

mod construction;
mod movement_orders;
mod movement_paths;
