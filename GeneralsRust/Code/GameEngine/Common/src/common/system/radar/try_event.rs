//! C++ `Radar::tryEvent` (Radar.cpp:1269-1315).
//!
//! Throttle is type + distance + 10s, and it does **not** require `active`.
//! Events die at 4s but remain in the ring and keep suppressing new pings.

use super::{Coord3D, RadarEvent, RadarEventType, MAX_RADAR_EVENTS};

/// C++ `closeEnoughDistanceSq = 250 * 250`.
pub const CLOSE_ENOUGH_DISTANCE_SQ: f32 = 250.0 * 250.0;
/// C++ `LOGICFRAMES_PER_SECOND * 10`.
pub const FRAMES_BETWEEN_EVENTS: u32 = 300;

/// True when a matching ring-buffer slot should reject a new ping.
#[must_use]
pub fn should_suppress_event(
    events: &[RadarEvent; MAX_RADAR_EVENTS],
    event_type: RadarEventType,
    world_loc: &Coord3D,
    current_frame: u32,
) -> bool {
    for event in events {
        if event.event_type != event_type {
            continue;
        }
        let dx = event.world_loc.x - world_loc.x;
        let dy = event.world_loc.y - world_loc.y;
        let dist_sq = dx * dx + dy * dy;
        if dist_sq <= CLOSE_ENOUGH_DISTANCE_SQ
            && current_frame.saturating_sub(event.create_frame) < FRAMES_BETWEEN_EVENTS
        {
            return true;
        }
    }
    false
}
