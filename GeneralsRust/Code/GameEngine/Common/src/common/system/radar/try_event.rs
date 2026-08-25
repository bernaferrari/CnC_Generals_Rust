//! C++ `Radar::tryEvent` (Radar.cpp:1269-1315).
//!
//! C++ writes the 250² close-enough test as:
//! `distSquared = ex - px * ex - px + ey - py * ey - py`
//! (`*` binds tighter than `-`/`+`). That is not `(dx*dx + dy*dy)`.
//! On typical map coords the result is a large negative, so
//! `distSquared <= 250*250` is always true and the distance gate never
//! filters. Same-type events therefore throttle **map-wide** for 10s.
//! Inactive ring-buffer history still counts (events die at 4s).

use super::{Coord3D, MAX_RADAR_EVENTS, RadarEvent, RadarEventType};

/// C++ `closeEnoughDistanceSq = 250 * 250` (kept for the authored constant).
pub const CLOSE_ENOUGH_DISTANCE_SQ: f32 = 250.0 * 250.0;
/// C++ `LOGICFRAMES_PER_SECOND * 10`.
pub const FRAMES_BETWEEN_EVENTS: u32 = 300;

/// True when a matching ring-buffer slot should reject a new ping.
///
/// Matches the C++ operator-precedence quirk: distance is ignored, only
/// event type + 10s window matter.
#[must_use]
pub fn should_suppress_event(
    events: &[RadarEvent; MAX_RADAR_EVENTS],
    event_type: RadarEventType,
    _world_loc: &Coord3D,
    current_frame: u32,
) -> bool {
    let _ = CLOSE_ENOUGH_DISTANCE_SQ;
    for event in events {
        if event.event_type != event_type {
            continue;
        }
        if current_frame.saturating_sub(event.create_frame) < FRAMES_BETWEEN_EVENTS {
            return true;
        }
    }
    false
}
