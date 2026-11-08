//! Per-unit path-following state (goal, repath timestamps, blocked counters).
//!
//! Matches the C++ AIStates.cpp::AIInternalMoveToState members used by
//! AIInternalMoveToState::update() lines 1743-1920.

use crate::common::Coord3D;

/// Path Following Controller State
#[derive(Debug, Clone)]
pub struct PathFollowingState {
    /// Current goal position
    pub goal_position: Coord3D,

    /// Position used for current path computation
    pub path_goal_position: Coord3D,

    /// Last frame path was computed
    pub path_timestamp: u32,

    /// Last frame blocked repath occurred
    pub blocked_repath_timestamp: u32,

    /// Number of frames unit has been blocked
    pub frames_blocked: u32,

    /// Whether to adjust destination during replanning
    pub adjusts_destination: bool,

    /// Waiting for async pathfinding result
    pub waiting_for_path: bool,

    /// Try one more repath attempt
    pub try_one_more_repath: bool,

    /// Current path handle (if any)
    pub current_path_handle: Option<u32>,
}

impl PathFollowingState {
    pub fn new(goal: Coord3D) -> Self {
        Self {
            goal_position: goal,
            path_goal_position: Coord3D::new(-10000.0, -10000.0, -10000.0), // Force initial compute
            path_timestamp: 0,
            blocked_repath_timestamp: 0,
            frames_blocked: 0,
            adjusts_destination: true,
            waiting_for_path: false,
            try_one_more_repath: false,
            current_path_handle: None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_path_following_state_creation() {
        let goal = Coord3D::new(100.0, 100.0, 0.0);
        let state = PathFollowingState::new(goal);

        assert_eq!(state.goal_position, goal);
        assert!(!state.waiting_for_path);
        assert_eq!(state.frames_blocked, 0);
    }
}
