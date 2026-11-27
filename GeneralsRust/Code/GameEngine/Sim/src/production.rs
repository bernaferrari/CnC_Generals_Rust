//! Integer production progress at the simulation update rate.

use crate::time::LOGIC_FRAMES_PER_SECOND;

/// C++ `ProductionUpdate` recomputes this integer threshold on every update.
///
/// Units use `ThingTemplate::calcTimeToBuild`, which divides the integer base
/// frame count by the current low-power penalty rate and truncates.  Upgrades
/// use `UpgradeTemplate::calcTimeToBuild`, whose retail implementation ignores
/// power.  A zero authored duration remains immediately complete; positive
/// sub-frame durations use one update so the queue cannot divide by zero.
pub fn production_total_logic_frames(total_time: f32, is_upgrade: bool, power_factor: f32) -> u32 {
    if !total_time.is_finite() || total_time <= 0.0 {
        return 0;
    }

    // C++ assigns the product into an Int before applying the later modifiers.
    let base_frames = (total_time * LOGIC_FRAMES_PER_SECOND)
        .trunc()
        .clamp(1.0, u32::MAX as f32) as u32;
    if is_upgrade {
        return base_frames;
    }

    // ThingTemplate::calcTimeToBuild clamps the penalty rate to at least .01
    // and C++ `Int /= Real` truncates the resulting threshold.
    let rate = power_factor.clamp(0.01, 1.0);
    ((base_frames as f32 / rate)
        .trunc()
        .clamp(1.0, u32::MAX as f32)) as u32
}

/// Recover an integer counter from old float-only snapshots exactly once.
/// New queue entries have both zero progress and zero construction frames, so
/// this cannot turn a newly queued item into a partially complete one.
pub fn production_frames_from_legacy_progress(
    progress: f32,
    total_time: f32,
    total_frames: u32,
) -> u32 {
    if total_frames == 0 || !progress.is_finite() || !total_time.is_finite() || total_time <= 0.0 {
        return 0;
    }
    if progress >= total_time {
        return total_frames;
    }
    ((progress.max(0.0) / total_time).clamp(0.0, 1.0) * total_frames as f32).floor() as u32
}

/// Presentation/UI seconds retained for compatibility with existing queue
/// consumers.  Authority always uses the integer counter and threshold above.
pub fn production_progress_from_logic_frames(
    construction_frames: u32,
    total_frames: u32,
    total_time: f32,
) -> f32 {
    if !total_time.is_finite() || total_time <= 0.0 || total_frames == 0 {
        return total_time.max(0.0);
    }
    total_time * (construction_frames.min(total_frames) as f32 / total_frames as f32)
}
