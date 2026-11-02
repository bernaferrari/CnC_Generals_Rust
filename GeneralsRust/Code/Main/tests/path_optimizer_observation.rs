// Exercise the production optimizer's fallible callback without exposing it
// as a public API solely for this regression.
mod common {
    pub use gamelogic::common::{Coord2D, Coord3D};
}
mod pathfind_astar {
    pub use gamelogic::ai::pathfind_astar::{GridCoord, PATHFIND_CELL_SIZE_F, PathfindLayerEnum};
}
#[allow(dead_code)]
#[path = "../../GameEngine/GameLogic/src/ai/path_optimization.rs"]
mod path_optimization;

use common::Coord3D;
use path_optimization::PathOptimizer;
use pathfind_astar::PathfindLayerEnum;

#[test]
fn optimization_stops_at_the_first_observation_error() {
    let waypoints = [
        Coord3D::new(0.0, 0.0, 0.0),
        Coord3D::new(10.0, 0.0, 0.0),
        Coord3D::new(10.0, 10.0, 0.0),
        Coord3D::new(20.0, 10.0, 0.0),
    ];
    let layers = [PathfindLayerEnum::Ground; 4];
    let mut calls = 0;
    let result = PathOptimizer::new().try_optimize(&waypoints, &layers, |_, _, _| {
        calls += 1;
        Err::<bool, _>("observation failed")
    });
    assert_eq!(result, Err("observation failed"));
    assert_eq!(calls, 1, "no further observation should follow an error");
}
