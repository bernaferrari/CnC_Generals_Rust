//! Shared special-power strike clock helpers and common imports.
pub(crate) use crate::command_system::SpecialPowerType;
pub(crate) use crate::game_logic::{ObjectId, Team};
pub(crate) use glam::Vec3;
pub(crate) use serde::{Deserialize, Serialize};
pub(crate) use std::collections::HashMap;

/// Logic frames per second (host fixed step).
pub const SP_LOGIC_FPS: f32 = 30.0;

/// C++ `ConvertDurationFromMsecsToFrames` residual (logic clock @ 30 FPS).
///
/// `ceil(msec * LOGICFRAMES_PER_SECOND / 1000)`. Used by LifetimeUpdate /
/// parseDurationUnsignedInt residual. Integer form: `(msec * 30 + 999) / 1000`.
#[inline]
pub fn duration_ms_to_logic_frames(msec: u32) -> u32 {
    if msec == 0 {
        return 0;
    }
    ((msec as u64 * 30 + 999) / 1000) as u32
}

/// Fixed LifetimeUpdate residual frames when MinLifetime == MaxLifetime (msec).
///
/// Host residual: deterministic die delay = parseDuration frames (ceil).
/// Fail-closed: not full GameLogicRandomValue range when min≠max.
#[inline]
pub fn lifetime_update_fixed_frames(min_ms: u32, max_ms: u32) -> u32 {
    let lo = min_ms.min(max_ms);
    let hi = min_ms.max(max_ms);
    // Equal min/max → fixed lifetime frames (PointDefense 95/95 → 3).
    let frames = duration_ms_to_logic_frames(if lo == hi { lo } else { lo });
    frames.max(1)
}

pub(crate) fn horizontal_distance(a: Vec3, b: Vec3) -> f32 {
    let dx = a.x - b.x;
    let dz = a.z - b.z;
    (dx * dx + dz * dz).sqrt()
}
