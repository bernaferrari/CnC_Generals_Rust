//! Cost calculations used by `Pathfinder::findClosestPath`.
//!
//! These mirror `PathfindCell::costToGoal` and `costSoFar` in AIPathfind.cpp.

pub(super) fn heuristic(dx: i32, dy: i32) -> i32 {
    let dx = dx.abs();
    let dy = dy.abs();
    10 * dx.max(dy) + 5 * dx.min(dy)
}

pub(super) fn step(
    previous: Option<(i32, i32)>,
    parent: (i32, i32),
    next: (i32, i32),
    pinched: bool,
) -> i32 {
    let direction = (parent.0 - next.0, parent.1 - next.1);
    let mut cost = if direction.0 == 0 || direction.1 == 0 {
        10
    } else {
        14
    };
    if pinched {
        cost += 14;
    }
    if let Some(previous) = previous {
        let prior = (previous.0 - parent.0, previous.1 - parent.1);
        if prior != direction {
            let dot = prior.0 * direction.0 + prior.1 * direction.1;
            cost += if dot > 0 {
                4
            } else if dot == 0 {
                8
            } else {
                16
            };
        }
    }
    cost
}
