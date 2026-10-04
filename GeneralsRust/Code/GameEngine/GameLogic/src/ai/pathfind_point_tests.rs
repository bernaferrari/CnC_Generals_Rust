use super::*;

// AIPathfind.cpp 732-1013: these exercise the real Path and optimized node
// chain. They establish geometric goals, not scheduler or transport motion.
fn path_with_segments(points: &[(Coord3D, PathfindLayerEnum)], optimized: &[usize]) -> Path {
    let mut path = Path::new();
    for (position, layer) in points {
        path.append_node(position, *layer);
    }
    let keys = path.ordered_keys();
    for pair in optimized.windows(2) {
        path.set_opti_link(keys[pair[0]], Some(keys[pair[1]]));
    }
    for key in keys {
        path.nodes[key].set_can_optimize(true);
    }
    path.mark_optimized();
    path
}

fn straight_path() -> Path {
    path_with_segments(
        &[
            (Coord3D::ZERO, PathfindLayerEnum::Ground),
            (Coord3D::new(20.0, 0.0, 0.0), PathfindLayerEnum::Ground),
        ],
        &[0, 1],
    )
}

fn assert_goal(
    result: ClosestPointOnPathInfo,
    goal: Coord3D,
    remaining: f32,
    layer: PathfindLayerEnum,
) {
    assert_eq!(result.pos_on_path, goal, "forward movement goal");
    assert!(
        (result.dist_along_path - remaining).abs() < 0.001,
        "remaining distance: actual {} expected {remaining}",
        result.dist_along_path
    );
    assert_eq!(result.layer, layer);
}

#[test]
fn point_on_path_leads_forward_and_reports_remaining_distance() {
    let mut path = straight_path();
    let result = path.compute_point_on_path(&Coord3D::new(5.0, 0.0, 0.0), &mut UnobstructedGround);
    assert_goal(
        result,
        Coord3D::new(20.0, 0.0, 0.0),
        15.0,
        PathfindLayerEnum::Ground,
    );
}

#[test]
fn point_on_path_uses_xy_length_and_endpoint_height() {
    let mut path = path_with_segments(
        &[
            (Coord3D::new(0.0, 0.0, 1.0), PathfindLayerEnum::Ground),
            (Coord3D::new(20.0, 0.0, 21.0), PathfindLayerEnum::Ground),
        ],
        &[0, 1],
    );
    let result =
        path.compute_point_on_path(&Coord3D::new(10.0, 0.0, 200.0), &mut UnobstructedGround);
    assert_goal(
        result,
        Coord3D::new(20.0, 0.0, 21.0),
        10.0,
        PathfindLayerEnum::Ground,
    );
}

#[test]
fn point_on_path_empty_and_single_node_keep_cpp_defaults() {
    let mut empty = Path::new();
    assert_goal(
        empty.compute_point_on_path(&Coord3D::new(5.0, 3.0, 1.0), &mut UnobstructedGround),
        Coord3D::ZERO,
        0.0,
        PathfindLayerEnum::Ground,
    );
    assert!(!empty.cpop_valid);
    let endpoint = Coord3D::new(20.0, 3.0, 5.0);
    let mut single = path_with_segments(&[(endpoint, PathfindLayerEnum::Wall)], &[0]);
    assert_goal(
        single.compute_point_on_path(&Coord3D::ZERO, &mut UnobstructedGround),
        endpoint,
        0.0,
        PathfindLayerEnum::Wall,
    );
}

#[test]
fn point_on_path_past_final_node_has_no_remaining_distance() {
    let mut path = straight_path();
    assert_goal(
        path.compute_point_on_path(&Coord3D::new(25.0, 0.0, 0.0), &mut UnobstructedGround),
        Coord3D::new(20.0, 0.0, 0.0),
        0.0,
        PathfindLayerEnum::Ground,
    );
}

#[test]
fn point_on_path_propagates_next_non_ground_layer() {
    let mut path = path_with_segments(
        &[
            (Coord3D::ZERO, PathfindLayerEnum::Ground),
            (Coord3D::new(20.0, 0.0, 0.0), PathfindLayerEnum::Wall),
        ],
        &[0, 1],
    );
    assert_goal(
        path.compute_point_on_path(&Coord3D::new(5.0, 0.0, 0.0), &mut UnobstructedGround),
        Coord3D::new(20.0, 0.0, 0.0),
        15.0,
        PathfindLayerEnum::Wall,
    );
}

#[test]
fn point_on_path_follows_optimized_successors_and_leads_into_next_segment() {
    // Ground actor and passable next-node/midpoint lines: C++ tries the next
    // segment midpoint after the halfway mark of the closest segment.
    let mut path = path_with_segments(
        &[0.0, 10.0, 20.0, 40.0].map(|x| (Coord3D::new(x, 0.0, 0.0), PathfindLayerEnum::Ground)),
        &[0, 2, 3],
    );
    assert_goal(
        path.compute_point_on_path(&Coord3D::new(15.0, 0.0, 0.0), &mut UnobstructedGround),
        Coord3D::new(30.0, 0.0, 0.0),
        25.0,
        PathfindLayerEnum::Ground,
    );
}

#[test]
fn point_on_path_skips_projection_beyond_intermediate_bend() {
    let mut path = path_with_segments(
        &[
            (Coord3D::ZERO, PathfindLayerEnum::Ground),
            (Coord3D::new(20.0, 0.0, 0.0), PathfindLayerEnum::Ground),
            (Coord3D::new(20.0, 20.0, 0.0), PathfindLayerEnum::Ground),
        ],
        &[0, 1, 2],
    );
    assert_goal(
        path.compute_point_on_path(&Coord3D::new(25.0, 10.0, 0.0), &mut UnobstructedGround),
        Coord3D::new(20.0, 20.0, 0.0),
        125.0_f32.sqrt(),
        PathfindLayerEnum::Ground,
    );
}

#[test]
fn point_on_path_reuses_twenty_close_inputs_then_recalculates() {
    let mut path = straight_path();
    let initial = Coord3D::new(5.0, 0.0, 0.0);
    let near = Coord3D::new(5.05, 0.05, 0.05);
    assert_goal(
        path.compute_point_on_path(&initial, &mut UnobstructedGround),
        Coord3D::new(20.0, 0.0, 0.0),
        15.0,
        PathfindLayerEnum::Ground,
    );
    for countdown in (0..Path::MAX_CPOP).rev() {
        assert_goal(
            path.compute_point_on_path(&near, &mut UnobstructedGround),
            Coord3D::new(20.0, 0.0, 0.0),
            15.0,
            PathfindLayerEnum::Ground,
        );
        assert_eq!(path.cpop_countdown, countdown);
        assert_eq!(path.cpop_in, initial);
    }
    let result = path.compute_point_on_path(&near, &mut UnobstructedGround);
    assert_goal(
        result,
        Coord3D::new(20.0, 0.0, 0.0),
        (14.95_f32.powi(2) + 0.05_f32.powi(2)).sqrt(),
        PathfindLayerEnum::Ground,
    );
    assert_eq!(path.cpop_in, near);
    assert_eq!(path.cpop_countdown, Path::MAX_CPOP);
}

#[derive(Debug, PartialEq)]
enum Query {
    Line(PathfindLayerEnum, Coord3D, Coord3D),
    ObjectLayer,
    Debug(Coord3D),
}

// A witness at the explicit service boundary, not a substitute movement or
// pathfinder implementation. Geometry and optimized links belong to real Path.
struct QueryWitness {
    layer: PathfindLayerEnum,
    answers: VecDeque<bool>,
    queries: Vec<Query>,
}

impl QueryWitness {
    fn new(layer: PathfindLayerEnum, answers: &[bool]) -> Self {
        Self {
            layer,
            answers: answers.iter().copied().collect(),
            queries: Vec::new(),
        }
    }
}

impl PathMovementContext for QueryWitness {
    fn object_layer(&mut self) -> PathfindLayerEnum {
        self.queries.push(Query::ObjectLayer);
        self.layer
    }

    fn is_line_passable(&mut self, layer: PathfindLayerEnum, from: &Coord3D, to: &Coord3D) -> bool {
        self.queries.push(Query::Line(layer, *from, *to));
        self.answers
            .pop_front()
            .expect("only the source-ordered queries are allowed")
    }

    fn set_debug_path_position(&mut self, position: &Coord3D) {
        self.queries.push(Query::Debug(*position));
    }
}

pub(super) struct UnobstructedGround;

impl PathMovementContext for UnobstructedGround {
    fn object_layer(&mut self) -> PathfindLayerEnum {
        PathfindLayerEnum::Ground
    }
    fn is_line_passable(&mut self, _: PathfindLayerEnum, _: &Coord3D, _: &Coord3D) -> bool {
        true
    }
    fn set_debug_path_position(&mut self, _: &Coord3D) {}
}

fn three_node_path() -> Path {
    path_with_segments(
        &[0.0, 20.0, 40.0].map(|x| (Coord3D::new(x, 0.0, 0.0), PathfindLayerEnum::Ground)),
        &[0, 1, 2],
    )
}

#[test]
fn passability_queries_keep_next_midpoint_and_debug_order() {
    let mut path = three_node_path();
    let pos = Coord3D::new(15.0, 0.0, 0.0);
    let mut witness = QueryWitness::new(PathfindLayerEnum::Ground, &[true, true]);
    let result = path.compute_point_on_path(&pos, &mut witness);
    assert_goal(
        result,
        Coord3D::new(30.0, 0.0, 0.0),
        25.0,
        PathfindLayerEnum::Ground,
    );
    assert_eq!(
        witness.queries,
        vec![
            Query::Line(PathfindLayerEnum::Ground, pos, Coord3D::new(20.0, 0.0, 0.0)),
            Query::ObjectLayer,
            Query::Line(PathfindLayerEnum::Ground, pos, Coord3D::new(30.0, 0.0, 0.0)),
            Query::Debug(result.pos_on_path),
        ]
    );
}

#[test]
fn no_opt_layer_change_and_object_layer_suppress_ahead_query() {
    for gate in 0..3 {
        let mut path = three_node_path();
        let keys = path.ordered_keys();
        if gate == 0 {
            path.nodes[keys[1]].set_can_optimize(false);
        }
        if gate == 1 {
            path.nodes[keys[1]].set_layer(PathfindLayerEnum::Wall);
        }
        let object_layer = if gate == 2 {
            PathfindLayerEnum::Wall
        } else {
            PathfindLayerEnum::Ground
        };
        let output_layer = if gate == 1 {
            PathfindLayerEnum::Wall
        } else {
            PathfindLayerEnum::Ground
        };
        let mut witness = QueryWitness::new(object_layer, &[true]);
        let pos = Coord3D::new(15.0, 0.0, 0.0);
        let result = path.compute_point_on_path(&pos, &mut witness);
        assert_goal(result, Coord3D::new(20.0, 0.0, 0.0), 25.0, output_layer);
        assert_eq!(
            witness.queries,
            vec![
                Query::Line(output_layer, pos, result.pos_on_path),
                Query::ObjectLayer,
                Query::Debug(result.pos_on_path)
            ]
        );
    }
}

#[test]
fn very_close_overrides_all_ahead_gates_without_second_query() {
    let mut path = three_node_path();
    let keys = path.ordered_keys();
    path.nodes[keys[1]].set_can_optimize(false);
    path.nodes[keys[1]].set_layer(PathfindLayerEnum::Wall);
    let pos = Coord3D::new(19.5, 0.0, 0.0);
    let mut witness = QueryWitness::new(PathfindLayerEnum::Wall, &[true]);
    let result = path.compute_point_on_path(&pos, &mut witness);
    assert_goal(
        result,
        Coord3D::new(30.0, 0.0, 0.0),
        20.5,
        PathfindLayerEnum::Wall,
    );
    assert_eq!(
        witness.queries,
        vec![
            Query::Line(PathfindLayerEnum::Wall, pos, Coord3D::new(20.0, 0.0, 0.0)),
            Query::ObjectLayer,
            Query::Debug(result.pos_on_path)
        ]
    );
}

#[test]
fn off_path_failed_endpoint_tries_half_remaining_segment_then_falls_back() {
    for second_passes in [false, true] {
        let mut path = straight_path();
        let pos = Coord3D::new(5.0, 30.0, 9.0);
        let mut witness = QueryWitness::new(PathfindLayerEnum::Ground, &[false, second_passes]);
        let result = path.compute_point_on_path(&pos, &mut witness);
        let x = if second_passes { 12.5 } else { 5.0 };
        assert_goal(
            result,
            Coord3D::new(x, 0.0, 0.0),
            ((x - 5.0).powi(2) + 900.0).sqrt(),
            PathfindLayerEnum::Ground,
        );
        assert_eq!(
            witness.queries,
            vec![
                Query::Line(PathfindLayerEnum::Ground, pos, Coord3D::new(20.0, 0.0, 0.0)),
                Query::Line(PathfindLayerEnum::Ground, pos, Coord3D::new(12.5, 0.0, 0.0)),
                Query::Debug(result.pos_on_path),
            ]
        );
    }
}

#[test]
fn modest_offset_blends_forward_without_half_segment_query() {
    let mut path = straight_path();
    let pos = Coord3D::new(5.0, 6.0, 10.0);
    let mut witness = QueryWitness::new(PathfindLayerEnum::Ground, &[false]);
    let result = path.compute_point_on_path(&pos, &mut witness);
    assert_goal(
        result,
        Coord3D::new(5.0 + (1.0 - 6.0 * (1.0 / 30.0)) * 15.0, 0.0, 0.0),
        15.0,
        PathfindLayerEnum::Ground,
    );
    assert_eq!(
        witness.queries,
        vec![
            Query::Line(PathfindLayerEnum::Ground, pos, Coord3D::new(20.0, 0.0, 0.0)),
            Query::Debug(result.pos_on_path)
        ]
    );
}

#[test]
fn failed_endpoint_near_fallback_skips_to_next_next_node() {
    let mut path = three_node_path();
    let pos = Coord3D::new(19.5, 0.0, 0.0);
    let mut witness = QueryWitness::new(PathfindLayerEnum::Ground, &[false]);
    let result = path.compute_point_on_path(&pos, &mut witness);
    assert_goal(
        result,
        Coord3D::new(40.0, 0.0, 0.0),
        20.5,
        PathfindLayerEnum::Ground,
    );
    assert_eq!(
        witness.queries,
        vec![
            Query::Line(PathfindLayerEnum::Ground, pos, Coord3D::new(20.0, 0.0, 0.0)),
            Query::Debug(result.pos_on_path)
        ]
    );
}

#[test]
fn remaining_one_does_not_extend_to_off_path_displacement() {
    let mut path = straight_path();
    let pos = Coord3D::new(19.0, 30.0, 0.0);
    let mut witness = QueryWitness::new(PathfindLayerEnum::Ground, &[false, false]);
    assert_goal(
        path.compute_point_on_path(&pos, &mut witness),
        Coord3D::new(19.0, 0.0, 0.0),
        1.0,
        PathfindLayerEnum::Ground,
    );
}

#[test]
fn cached_result_and_empty_path_do_not_query_or_publish_debug() {
    let mut path = straight_path();
    let pos = Coord3D::new(5.0, 0.0, 0.0);
    let mut witness = QueryWitness::new(PathfindLayerEnum::Ground, &[true]);
    let initial = path.compute_point_on_path(&pos, &mut witness);
    witness.queries.clear();
    assert_goal(
        path.compute_point_on_path(&pos, &mut witness),
        initial.pos_on_path,
        initial.dist_along_path,
        initial.layer,
    );
    assert!(witness.queries.is_empty());
    let mut empty = Path::new();
    empty.compute_point_on_path(&pos, &mut witness);
    assert!(witness.queries.is_empty());
}

#[test]
fn closest_segment_ties_keep_the_first_optimized_segment() {
    let mut path = path_with_segments(
        &[
            (Coord3D::ZERO, PathfindLayerEnum::Ground),
            (Coord3D::new(20.0, 0.0, 0.0), PathfindLayerEnum::Ground),
            (Coord3D::new(20.0, 20.0, 0.0), PathfindLayerEnum::Ground),
        ],
        &[0, 1, 2],
    );
    let pos = Coord3D::new(15.0, 5.0, 0.0);
    let mut witness = QueryWitness::new(PathfindLayerEnum::Ground, &[true, true]);
    let result = path.compute_point_on_path(&pos, &mut witness);
    assert_goal(
        result,
        Coord3D::new(20.0, 10.0, 0.0),
        25.0,
        PathfindLayerEnum::Ground,
    );
    assert_eq!(
        witness.queries[0],
        Query::Line(PathfindLayerEnum::Ground, pos, Coord3D::new(20.0, 0.0, 0.0))
    );
}

#[test]
fn raw_successors_without_optimized_links_are_not_movement_segments() {
    let mut path = path_with_segments(
        &[
            (Coord3D::ZERO, PathfindLayerEnum::Ground),
            (Coord3D::new(20.0, 0.0, 0.0), PathfindLayerEnum::Ground),
        ],
        &[],
    );
    let mut witness = QueryWitness::new(PathfindLayerEnum::Ground, &[]);
    let result = path.compute_point_on_path(&Coord3D::ZERO, &mut witness);
    assert_goal(
        result,
        Coord3D::new(20.0, 0.0, 0.0),
        0.0,
        PathfindLayerEnum::Ground,
    );
    assert_eq!(witness.queries, vec![Query::Debug(result.pos_on_path)]);
}

#[test]
fn degenerate_xy_segment_retains_the_cpp_minimum_length() {
    let mut path = path_with_segments(
        &[
            (Coord3D::ZERO, PathfindLayerEnum::Ground),
            (Coord3D::new(0.0, 0.0, 20.0), PathfindLayerEnum::Ground),
        ],
        &[0, 1],
    );
    let mut witness = QueryWitness::new(PathfindLayerEnum::Ground, &[true]);
    assert_goal(
        path.compute_point_on_path(&Coord3D::ZERO, &mut witness),
        Coord3D::new(0.0, 0.0, 20.0),
        0.01,
        PathfindLayerEnum::Ground,
    );
}

#[test]
fn previous_non_ground_raw_node_resets_layer_when_leaving_it() {
    let mut path = three_node_path();
    let keys = path.ordered_keys();
    path.nodes[keys[0]].set_layer(PathfindLayerEnum::Wall);
    let mut witness = QueryWitness::new(PathfindLayerEnum::Ground, &[true]);
    assert_goal(
        path.compute_point_on_path(&Coord3D::new(25.0, 0.0, 0.0), &mut witness),
        Coord3D::new(40.0, 0.0, 0.0),
        15.0,
        PathfindLayerEnum::Ground,
    );
}

#[test]
fn diagonal_segment_uses_cached_normalized_xy_direction() {
    let mut path = path_with_segments(
        &[
            (Coord3D::ZERO, PathfindLayerEnum::Ground),
            (Coord3D::new(12.0, 16.0, 50.0), PathfindLayerEnum::Ground),
        ],
        &[0, 1],
    );
    let mut witness = QueryWitness::new(PathfindLayerEnum::Ground, &[true]);
    assert_goal(
        path.compute_point_on_path(&Coord3D::new(3.0, 4.0, 200.0), &mut witness),
        Coord3D::new(12.0, 16.0, 50.0),
        15.0,
        PathfindLayerEnum::Ground,
    );
}

#[test]
fn beyond_cpp_closest_sentinel_keeps_endpoint_without_passability_queries() {
    let mut path = straight_path();
    let mut witness = QueryWitness::new(PathfindLayerEnum::Ground, &[]);
    let result = path.compute_point_on_path(&Coord3D::new(0.0, 20000.0, 0.0), &mut witness);
    assert_goal(
        result,
        Coord3D::new(20.0, 0.0, 0.0),
        (20.0_f32.powi(2) + 20000.0_f32.powi(2)).sqrt(),
        PathfindLayerEnum::Ground,
    );
    assert_eq!(witness.queries, vec![Query::Debug(result.pos_on_path)]);
}
