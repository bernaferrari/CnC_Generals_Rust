//! Original-executed flat-ground traces plus receiving-query lifetime controls.
use super::*;

#[test]
fn raw_group_search_matches_executed_original_cpp() {
    let fixture: serde_json::Value =
        serde_json::from_str(include_str!("group_ground_cpp_cases.json")).unwrap();
    for case in fixture["cases"].as_array().unwrap() {
        let input: Vec<i32> = case["input"]
            .as_array()
            .unwrap()
            .iter()
            .map(|n| n.as_i64().unwrap() as i32)
            .collect();
        let mut grid = PathfindingGrid::new(input[0] as f32 * 10.0, input[1] as f32 * 10.0, 10.0);
        for obstacle in input[8..].chunks_exact(2) {
            grid.set_blocked(GridPos::new(obstacle[0], obstacle[1]), true);
        }
        let start = GridPos::new(input[2], input[3]);
        let requested = GridPos::new(input[4], input[5]);
        let expected = case["raw_original_search"].as_str().unwrap();
        let objects = HashMap::new();
        let from = Vec3::new(start.x as f32 * 10.0, 0.0, start.y as f32 * 10.0);
        let to = Vec3::new(requested.x as f32 * 10.0, 0.0, requested.y as f32 * 10.0);
        if expected == "null" {
            assert!(
                grid.find_group_ground_path_owned(
                    from,
                    to,
                    &objects,
                    |_| true,
                    &mut 0,
                    &crate::game_logic::pathfinding::PathfindingGround::Flat(0.0)
                )
                .is_none(),
                "{}",
                case["name"]
            );
            continue;
        }
        let (chain, cost) = expected.split_once(" cost=").unwrap();
        let original: Vec<GridPos> = chain
            .trim_end_matches(';')
            .split(';')
            .map(|pair| {
                let (x, y) = pair.split_once(',').unwrap();
                GridPos::new(x.parse().unwrap(), y.parse().unwrap())
            })
            .collect();
        // The original destination probe can select a different goal. Raw
        // search starts after that probe; the production query below checks it.
        let goal = *original.last().unwrap();
        let (actual, actual_cost) = grid
            .group_ground_raw_trace(start, goal, &objects)
            .expect("original found a route");
        assert_eq!(actual, original, "{} parent chain", case["name"]);
        assert_eq!(
            actual_cost,
            cost.parse::<u32>().unwrap(),
            "{} search cost",
            case["name"]
        );
        let optimized = grid
            .find_group_ground_path_owned(
                from,
                to,
                &objects,
                |_| true,
                &mut 0,
                &crate::game_logic::pathfinding::PathfindingGround::Flat(0.0),
            )
            .expect("original group query succeeds");
        assert_eq!(
            grid.world_to_grid(*optimized.last().unwrap()),
            goal,
            "{} selected destination",
            case["name"]
        );
    }
}

#[test]
fn first_goal_neighbor_preserves_safe_admitted_ray_cost() {
    let grid = PathfindingGrid::new(240.0, 240.0, 10.0);
    let (chain, cost) = grid
        .group_ground_raw_trace(GridPos::new(12, 12), GridPos::new(13, 12), &HashMap::new())
        .unwrap();
    assert_eq!(chain, [GridPos::new(12, 12), GridPos::new(13, 12)]);
    // Original reads an uninitialized local here (O0 cost1, O2 cost5).
    // Safe Rust retains the ray's defined +5 edge rather than copy stack bytes.
    assert_eq!(cost, 5);
}

#[test]
fn receiving_group_queries_do_not_inherit_individual_ignore_or_peer_cells() {
    let mut first = PathfindingSystem::new(400.0, 400.0);
    let mut peer = PathfindingSystem::new(400.0, 400.0);
    let start = Vec3::new(80.0, 0.0, 200.0);
    let goal = Vec3::new(300.0, 0.0, 200.0);
    let objects = HashMap::new();
    let expected = first
        .find_group_ground_path(
            start,
            goal,
            &objects,
            &crate::game_logic::pathfinding::PathfindingGround::Flat(0.0),
        )
        .unwrap();
    for y in 0..40 {
        peer.grid.set_blocked(GridPos::new(20, y), true);
    }
    assert!(
        peer.find_group_ground_path(
            start,
            goal,
            &objects,
            &crate::game_logic::pathfinding::PathfindingGround::Flat(0.0)
        )
        .is_none()
    );
    first.set_ignore_obstacle(Some(ObjectId(91)));
    assert_eq!(
        first
            .find_group_ground_path(
                start,
                goal,
                &objects,
                &crate::game_logic::pathfinding::PathfindingGround::Flat(0.0)
            )
            .unwrap(),
        expected
    );
    assert_eq!(
        first.ignore_obstacle(),
        Some(ObjectId(91)),
        "group search must not replace the caller's individual request state"
    );
    let _inert = PathfindingSystem::new(600.0, 600.0);
    assert_eq!(
        first
            .find_group_ground_path(
                start,
                goal,
                &objects,
                &crate::game_logic::pathfinding::PathfindingGround::Flat(0.0)
            )
            .unwrap(),
        expected
    );
}

#[test]
fn fixed_occupants_are_resolved_from_each_receiving_object_map() {
    let mut first = PathfindingSystem::new(400.0, 400.0);
    let mut peer = PathfindingSystem::new(400.0, 400.0);
    let start = Vec3::new(80.0, 0.0, 200.0);
    let goal = Vec3::new(200.0, 0.0, 200.0);
    let mut template = ThingTemplate::new("GroupFixedOccupant");
    template.add_kind_of(KindOf::Vehicle).set_health(100.0);
    let mut occupant = Object::new(template, ObjectId(91), Team::USA);
    occupant.set_position(goal);
    occupant.owner_player_id = Some(0);
    occupant.crushable_level = 1;
    let mut counterpart = occupant.clone();
    counterpart.crushable_level = 0;
    let first_objects = HashMap::from([(occupant.id, occupant)]);
    let peer_objects = HashMap::from([(counterpart.id, counterpart)]);
    first.set_ignore_obstacle(Some(ObjectId(91)));
    let shifted = first
        .find_group_ground_path(
            start,
            goal,
            &first_objects,
            &crate::game_logic::pathfinding::PathfindingGround::Flat(0.0),
        )
        .unwrap();
    let peer_path = peer
        .find_group_ground_path(
            start,
            goal,
            &peer_objects,
            &crate::game_logic::pathfinding::PathfindingGround::Flat(0.0),
        )
        .unwrap();
    assert_ne!(
        first.grid.world_to_grid(*shifted.last().unwrap()),
        first.grid.world_to_grid(goal)
    );
    assert_eq!(
        peer.grid.world_to_grid(*peer_path.last().unwrap()),
        peer.grid.world_to_grid(goal)
    );
    assert_eq!(
        first
            .find_group_ground_path(
                start,
                goal,
                &first_objects,
                &crate::game_logic::pathfinding::PathfindingGround::Flat(0.0)
            )
            .unwrap(),
        shifted
    );
    let cleared = first
        .find_group_ground_path(
            start,
            goal,
            &HashMap::new(),
            &crate::game_logic::pathfinding::PathfindingGround::Flat(0.0),
        )
        .unwrap();
    assert_eq!(
        first.grid.world_to_grid(*cleared.last().unwrap()),
        first.grid.world_to_grid(goal)
    );
}

#[test]
fn failed_shared_group_search_preserves_member_movement_admission() {
    crate::game_logic::game_logic::pose_owner_tests::isolated_at(
        module_path!(),
        "failed_shared_group_search_preserves_member_movement_admission",
        || {
            let mut world = GameLogic::new();
            world.override_world_size(320.0, 320.0);
            world.force_map_loaded_for_path_test(true);
            for y in 8..32 {
                for x in 8..32 {
                    world
                        .pathfinding_system
                        .grid
                        .set_blocked(GridPos::new(x, y), true);
                }
            }
            let destination = Vec3::new(20.0, 0.0, 20.0);
            let mut goals = Vec::new();
            for i in 0..8 {
                let id = ObjectId(100 + i);
                let mut template = ThingTemplate::new("GroupSearchMember");
                template.add_kind_of(KindOf::Infantry).set_health(100.0);
                let mut member = Object::new(template, id, Team::USA);
                member.set_position(Vec3::new(-100.0 + i as f32 * 4.0, 0.0, -100.0));
                member.num_frames_blocked = 9;
                member.is_blocked_and_stuck = true;
                member.is_final_goal = false;
                world.objects.insert(id, member);
                goals.push((id, destination));
            }
            assert!(!world.assign_shared_group_paths(&goals, destination));
            for (id, _) in goals {
                let member = &world.objects[&id];
                assert_eq!(member.num_frames_blocked, 9);
                assert!(member.is_blocked_and_stuck);
                assert!(!member.is_final_goal);
                assert!(!member.status.moving);
                assert!(member.movement.path.is_empty());
            }
        },
    );
}

#[test]
fn group_cleanup_is_charged_to_the_receiving_queue_budget() {
    let mut first = PathfindingSystem::new(400.0, 400.0);
    let mut peer = PathfindingSystem::new(400.0, 400.0);
    let start = Vec3::new(80.0, 0.0, 200.0);
    let goal = Vec3::new(300.0, 0.0, 200.0);
    first.set_pathfind_cells_per_frame(1);
    assert!(first.pathfind_budget_remaining());
    assert!(
        first
            .find_group_ground_path(
                start,
                goal,
                &HashMap::new(),
                &crate::game_logic::pathfinding::PathfindingGround::Flat(0.0)
            )
            .is_some()
    );
    let spent = first.cumulative_cells_allocated();
    assert!(spent > 0);
    assert!(!first.pathfind_budget_remaining());
    for y in 0..40 {
        peer.grid.set_blocked(GridPos::new(20, y), true);
    }
    assert!(
        peer.find_group_ground_path(
            start,
            goal,
            &HashMap::new(),
            &crate::game_logic::pathfinding::PathfindingGround::Flat(0.0)
        )
        .is_none()
    );
    assert!(
        peer.cumulative_cells_allocated() > 0,
        "failed search also cleans visited cells"
    );
    assert_eq!(first.cumulative_cells_allocated(), spent);
    first.begin_pathfind_queue_frame();
    assert_eq!(first.cumulative_cells_allocated(), 0);
    assert!(peer.cumulative_cells_allocated() > 0);
}

#[test]
fn off_map_segment_still_uses_cpp_clamped_cell_destination_probe() {
    let mut owner = PathfindingSystem::new(400.0, 400.0);
    let path = owner
        .find_group_ground_path(
            Vec3::new(420.0, 0.0, 200.0),
            Vec3::new(460.0, 0.0, 200.0),
            &HashMap::new(),
            &crate::game_logic::pathfinding::PathfindingGround::Flat(0.0),
        )
        .unwrap();
    // ClipLine2D rejects this wholly exterior segment. worldToCell then clamps
    // its endpoint to (39,20); the first full-width probe is NW at radius two.
    assert_eq!(
        owner.grid.world_to_grid(*path.last().unwrap()),
        GridPos::new(37, 22)
    );
    assert_eq!(path[0], Vec3::new(420.0, 0.0, 200.0));
}

#[test]
fn group_search_crosses_owned_bridge_without_ground_pinches_leaking_to_deck() {
    let mut owner = PathfindingSystem::new(400.0, 400.0);
    for y in 0..40 {
        for x in 15..25 {
            owner
                .grid
                .set_cell_type(GridPos::new(x, y), PathfindCellType::Water);
            owner.grid.set_pinched(GridPos::new(x, y), true);
        }
    }
    owner.grid.stamp_bridge_deck(
        Vec3::new(100.0, 20.0, 150.0),
        Vec3::new(100.0, 20.0, 250.0),
        Vec3::new(300.0, 20.0, 150.0),
        Vec3::new(300.0, 20.0, 250.0),
        false,
        &crate::game_logic::pathfinding::PathfindingGround::Flat(0.0),
    );
    owner.grid.rebuild_terrain_zones();
    owner.grid.rebuild_path_zones();
    let goal = Vec3::new(320.0, 0.0, 200.0);
    let path = owner
        .find_group_ground_path(
            Vec3::new(80.0, 0.0, 200.0),
            goal,
            &HashMap::new(),
            &crate::game_logic::pathfinding::PathfindingGround::Flat(0.0),
        )
        .expect("connected owned deck route");
    // C++ retains layer independently from position. In prependCells a
    // duplicate bank/deck coordinate keeps its bank height and gains the deck
    // layer; optimizer can then discard all elevated intermediate positions.
    let (cells, _) = owner
        .grid
        .group_ground_layer_trace(
            owner.grid.world_to_grid(Vec3::new(80.0, 0.0, 200.0)),
            owner.grid.world_to_grid(goal),
            &HashMap::new(),
        )
        .unwrap();
    assert!(
        cells.iter().any(|(cell, layer)| {
            (15..25).contains(&cell.x) && *layer > PathfindLayerEnum::Ground as u8
        }),
        "search must cross the river on its admitted deck layer: {cells:?}"
    );
    assert_eq!(
        owner.grid.world_to_grid(*path.last().unwrap()),
        owner.grid.world_to_grid(goal)
    );
}
