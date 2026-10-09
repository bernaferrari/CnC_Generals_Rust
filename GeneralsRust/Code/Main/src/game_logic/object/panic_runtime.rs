//! Object-owned continuation for the C++ `AIPanicState` waypoint follower.
//!
//! The Object's ordinary path and movement fields remain the sole live path
//! authority. This value keeps only panic/follow-waypoint state that has no
//! corresponding Object field.
use glam::Vec3;
use serde::{Deserialize, Serialize};

/// C++ `AIPanicState` plus its `AIFollowWaypointPathState` continuation.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub(crate) struct PanicRuntime {
    /// C++ `m_currentWaypoint` identity, restored against the active terrain.
    pub(crate) current_waypoint_id: u32,
    /// C++ `m_priorWaypoint` identity (`INVALID_WAYPOINT_ID` is None).
    pub(crate) prior_waypoint_id: Option<u32>,
    /// C++ `m_groupOffset` (2D, in terrain XY coordinates).
    pub(crate) group_offset: glam::Vec2,
    /// C++ `m_angle`; retained even though `ROTATE_OFFSETS` is disabled.
    pub(crate) angle: f32,
    /// C++ `m_framesSleeping`.
    pub(crate) frames_sleeping: i32,
    /// C++ `m_appendGoalPosition`.
    pub(crate) append_goal_position: bool,
    /// C++ `AIPanicState::m_waitFrames`.
    pub(crate) wait_frames: i32,
    /// C++ `AIPanicState::m_timer`.
    pub(crate) timer: i32,
    /// C++ `AIInternalMoveToState::m_goalLayer` for the current waypoint.
    /// This is a pathfinding layer ordinal (GROUND=1, WALL=15), not an
    /// Object movement field.
    #[serde(default = "default_ground_layer")]
    pub(crate) goal_layer: u8,
    /// C++ `AIInternalMoveToState::m_blockedRepathTimestamp`.
    pub(crate) blocked_repath_timestamp: u32,
}

impl PanicRuntime {
    pub(crate) fn new(current_waypoint_id: u32, group_offset: glam::Vec2, object_id: u32) -> Self {
        Self {
            current_waypoint_id,
            prior_waypoint_id: None,
            group_offset,
            angle: 0.0,
            frames_sleeping: 0,
            append_goal_position: false,
            wait_frames: 10 + (object_id & 0x7) as i32,
            timer: 0,
            goal_layer: 1,
            blocked_repath_timestamp: 0,
        }
    }
}

const fn default_ground_layer() -> u8 {
    1
}

/// Save-time projection of existing Object movement fields needed by the
/// inherited C++ `AIInternalMoveToState::xfer`. These values are not stored a
/// second time in the live PanicRuntime; ObjectSnapshot::movement remains the
/// canonical path and target-position record.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub(crate) struct PanicSaveState {
    pub(crate) panic: PanicRuntime,
    pub(crate) requested_destination: Option<Vec3>,
    pub(crate) path_goal_position: Option<Vec3>,
    pub(crate) waiting_for_path: bool,
    pub(crate) path_timestamp: u32,
    pub(crate) adjust_destinations: bool,
    pub(crate) path_extra_distance: f32,
    #[serde(default)]
    pub(crate) wander_width_factor: Option<f32>,
    pub(crate) retry_path: bool,
    pub(crate) try_one_more_repath: bool,
    pub(crate) is_blocked_and_stuck: bool,
    pub(crate) num_frames_blocked: u32,
}

impl PanicSaveState {
    pub(crate) fn capture(panic: &PanicRuntime, object: &super::Object) -> Self {
        Self {
            panic: panic.clone(),
            requested_destination: object.requested_destination,
            path_goal_position: object.path_goal_position,
            waiting_for_path: object.waiting_for_path,
            path_timestamp: object.path_timestamp,
            adjust_destinations: object.adjust_destinations,
            path_extra_distance: object.path_extra_distance,
            wander_width_factor: Some(object.wander_width_factor),
            retry_path: object.retry_path,
            try_one_more_repath: object.try_one_more_repath,
            is_blocked_and_stuck: object.is_blocked_and_stuck,
            num_frames_blocked: object.num_frames_blocked,
        }
    }

    pub(crate) fn restore_into(self, object: &mut super::Object) {
        object.panic_runtime = Some(self.panic);
        object.requested_destination = self.requested_destination;
        object.path_goal_position = self.path_goal_position;
        object.waiting_for_path = self.waiting_for_path;
        object.path_timestamp = self.path_timestamp;
        object.adjust_destinations = self.adjust_destinations;
        object.path_extra_distance = self.path_extra_distance;
        if let Some(width) = self.wander_width_factor {
            object.wander_width_factor = width;
        }
        object.retry_path = self.retry_path;
        object.try_one_more_repath = self.try_one_more_repath;
        object.is_blocked_and_stuck = self.is_blocked_and_stuck;
        object.num_frames_blocked = self.num_frames_blocked;
        object.is_panicking = true;
    }
}
