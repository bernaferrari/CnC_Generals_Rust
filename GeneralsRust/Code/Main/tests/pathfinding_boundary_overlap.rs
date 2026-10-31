//! C++ PathfindCell::setTypeAsObstacle rejects replacing an existing obstacle.
use generals_main::game_logic::pathfinding::{PathfindingGrid, PathfindingGround};
use generals_main::game_logic::{KindOf, Object, ObjectId, Team, ThingTemplate};
use glam::Vec3;

fn structure(id: u32) -> Object {
    let mut template = ThingTemplate::new("BoundaryObstacle");
    template.add_kind_of(KindOf::Structure);
    let mut object = Object::new(template, ObjectId(id), Team::USA);
    object.set_position(Vec3::new(45.0, 0.0, 45.0));
    object.selection_radius = 20.0;
    object
}

#[test]
fn repeated_and_overlapping_boundary_classification_keeps_first_obstacle_owner() {
    let mut grid = PathfindingGrid::new(100.0, 100.0, 10.0);
    let first = structure(11);
    let second = structure(12);
    let ground = PathfindingGround::Flat(0.0);
    let center = grid.world_to_grid(first.get_position());
    assert!(
        grid.classify_object_footprint(&first, false, &ground)
            .is_some()
    );
    assert!(
        grid.classify_object_footprint(&first, false, &ground)
            .is_some()
    );
    assert!(
        grid.classify_object_footprint(&second, false, &ground)
            .is_some()
    );
    assert!(!grid.clear_cell_obstacle_owned(center, second.id.0));
    assert!(grid.clear_cell_obstacle_owned(center, first.id.0));
}

#[test]
fn overlapping_fences_do_not_replace_an_existing_obstacle_owner() {
    let mut grid = PathfindingGrid::new(100.0, 100.0, 10.0);
    let position = Vec3::new(45.0, 0.0, 45.0);
    let center = grid.world_to_grid(position);
    grid.set_cell_obstacle_owned(center, false, false, 11, Some(1), Some(Team::USA));
    assert!(
        grid.classify_fence_world(
            position,
            0.0,
            20.0,
            0.0,
            false,
            12,
            Some(2),
            Some(Team::GLA),
            false
        )
        .is_some()
    );
    assert!(!grid.clear_cell_obstacle_owned(center, 12));
    assert!(grid.clear_cell_obstacle_owned(center, 11));
}

#[test]
fn structure_reclassification_keeps_existing_rubble() {
    use gamelogic::ai::pathfind_astar::PathfindCellType;
    let mut grid = PathfindingGrid::new(100.0, 100.0, 10.0);
    let object = structure(11);
    let center = grid.world_to_grid(object.get_position());
    grid.set_cell_type(center, PathfindCellType::Rubble);
    assert!(
        grid.classify_object_footprint(&object, false, &PathfindingGround::Flat(0.0))
            .is_some()
    );
    assert_eq!(grid.cell_type(center), PathfindCellType::Rubble);
}

#[test]
fn structure_classification_can_claim_an_impassable_cell() {
    use gamelogic::ai::pathfind_astar::PathfindCellType;
    let mut grid = PathfindingGrid::new(100.0, 100.0, 10.0);
    let object = structure(11);
    let center = grid.world_to_grid(object.get_position());
    grid.set_cell_type(center, PathfindCellType::Impassable);
    assert!(
        grid.classify_object_footprint(&object, false, &PathfindingGround::Flat(0.0))
            .is_some()
    );
    assert_eq!(grid.cell_type(center), PathfindCellType::Obstacle);
    assert!(grid.clear_cell_obstacle_owned(center, object.id.0));
}

#[test]
fn cell_admission_matches_source_pinned_cpp_execution() {
    use gamelogic::ai::pathfind_astar::PathfindCellType;
    let object = structure(11);
    let ground = PathfindingGround::Flat(0.0);
    let mut matched = 0;
    for line in include_str!("oracles/pathfind_cell/expected.txt").lines() {
        let values: Vec<u32> = line
            .split_whitespace()
            .map(|value| value.parse().unwrap())
            .collect();
        assert_eq!(values.len(), 5, "invalid C++ oracle row: {line}");
        let (initial, rubble, expected_accepted, expected_type, expected_owner) = (
            values[0],
            values[1] == 2,
            values[2] != 0,
            values[3],
            values[4],
        );
        let mut grid = PathfindingGrid::new(100.0, 100.0, 10.0);
        let center = grid.world_to_grid(object.get_position());
        let initial_type = match initial {
            0 => PathfindCellType::Clear,
            1 => PathfindCellType::Water,
            2 => PathfindCellType::Cliff,
            3 => PathfindCellType::Rubble,
            4 => PathfindCellType::Obstacle,
            5 => PathfindCellType::BridgeImpassable,
            6 => PathfindCellType::Impassable,
            _ => panic!("invalid original cell type"),
        };
        if initial == 4 {
            grid.set_cell_obstacle_owned(center, false, false, 77, None, None);
        } else {
            grid.set_cell_type(center, initial_type);
        }
        assert!(
            grid.classify_object_footprint(&object, rubble, &ground)
                .is_some()
        );
        let actual_type = grid.cell_type(center) as u32;
        let mut probe = grid.clone();
        let owner = if probe.clear_cell_obstacle_owned(center, 11) {
            11
        } else if probe.clear_cell_obstacle_owned(center, 77) {
            77
        } else {
            0
        };
        let initial_owner = if initial == 4 { 77 } else { 0 };
        let accepted = actual_type != initial || owner != initial_owner;
        assert_eq!(
            (accepted, actual_type, owner),
            (expected_accepted, expected_type, expected_owner),
            "original row: {line}"
        );
        matched += 1;
    }
    assert_eq!(matched, 21);
}

#[test]
fn boundary_classifier_derives_rubble_from_the_live_object_body() {
    use gamelogic::ai::pathfind_astar::PathfindCellType;
    use generals_main::game_logic::host_enum_table_residual::HostBodyDamageType;
    use generals_main::game_logic::pathfinding::PathfindingSystem;
    let mut object = structure(11);
    object.body_damage_state = HostBodyDamageType::Rubble;
    let mut pathfinder = PathfindingSystem::new(100.0, 100.0);
    let center = pathfinder.grid.world_to_grid(object.get_position());
    pathfinder.classify_and_pinch_object(&object, &PathfindingGround::Flat(0.0));
    assert_eq!(pathfinder.grid.cell_type(center), PathfindCellType::Rubble);
}
