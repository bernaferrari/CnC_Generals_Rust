#[path = "../src/game_logic/pathfinding/closest_cost.rs"]
mod closest_cost;

#[test]
fn closest_costs_match_original_pathfind_cell_functions() {
    // Values are produced by tests/oracles/closest_cost/oracle.py, which
    // compiles the pinned original C++ methods.
    assert_eq!(closest_cost::heuristic(3, 2), 40);
    let previous = Some((0, 0));
    let parent = (1, 0);
    assert_eq!(10 + closest_cost::step(previous, parent, (2, 0), false), 20);
    assert_eq!(10 + closest_cost::step(previous, parent, (2, 1), false), 28);
    assert_eq!(10 + closest_cost::step(previous, parent, (1, 1), false), 28);
    assert_eq!(10 + closest_cost::step(previous, parent, (0, 1), false), 40);
    assert_eq!(10 + closest_cost::step(previous, parent, (2, 0), true), 34);
}
