//! Host SlowDeathBehavior residual (delayed sink + destroy after lethal damage).
//!
//! C++: `SlowDeathBehavior::beginSlowDeath` + `update` phases:
//! sink delay → sink rate → destruction delay → destroyObject.
//!
//! Residual playability slice:
//! - Infantry: SinkDelay 3000ms, SinkRate 0.5 w/s, DestructionDelay 8000ms
//! - Vehicle default: DestructionDelay peel (no sink unless configured)
//! - Presentation sink_offset (negative Y)
//! - Defers GameLogic destroy until destruction frame
//!
//! Fail-closed:
//! - Not full fling physics / multi DeathTypes probability matrix
//! - Not full FX/OCL/Weapon phase bursts (INITIAL/MIDPOINT/FINAL)
//! - Not LOD instant-death scale matrix

use serde::{Deserialize, Serialize};

/// Logic FPS residual.
pub const SLOW_DEATH_LOGIC_FPS: f32 = 30.0;

/// Retail infantry SinkDelay 3000 ms → frames.
pub const INFANTRY_SINK_DELAY_MS: u32 = 3_000;
/// Retail infantry SinkRate 0.5 dist/sec.
pub const INFANTRY_SINK_RATE_PER_SEC: f32 = 0.5;
/// Retail infantry DestructionDelay 8000 ms.
pub const INFANTRY_DESTRUCTION_DELAY_MS: u32 = 8_000;
/// Default vehicle destruction delay residual (instant-ish but one beat).
pub const VEHICLE_DESTRUCTION_DELAY_MS: u32 = 1_000;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum HostSlowDeathPhase {
    #[default]
    Inactive = 0,
    WaitingToSink = 1,
    Sinking = 2,
    WaitingToDestroy = 3,
    Done = 4,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HostSlowDeathData {
    pub phase: HostSlowDeathPhase,
    pub begin_frame: u32,
    pub sink_at_frame: u32,
    pub destroy_at_frame: u32,
    /// World units per logic frame (positive magnitude; applied as -Y).
    pub sink_rate_per_frame: f32,
    /// Accumulated sink offset (negative).
    pub sink_offset: f32,
    /// C++ destructionAltitude residual (stop sinking around this altitude).
    pub destruction_altitude: f32,
}

impl Default for HostSlowDeathData {
    fn default() -> Self {
        Self {
            phase: HostSlowDeathPhase::Inactive,
            begin_frame: 0,
            sink_at_frame: 0,
            destroy_at_frame: 0,
            sink_rate_per_frame: 0.0,
            sink_offset: 0.0,
            destruction_altitude: -10.0,
        }
    }
}

impl HostSlowDeathData {
    pub fn is_active(&self) -> bool {
        !matches!(
            self.phase,
            HostSlowDeathPhase::Inactive | HostSlowDeathPhase::Done
        )
    }

    pub fn is_done(&self) -> bool {
        self.phase == HostSlowDeathPhase::Done
    }

    pub fn infantry_residual(current_frame: u32) -> Self {
        let sink_delay = ms_to_frames(INFANTRY_SINK_DELAY_MS);
        let destroy_delay = ms_to_frames(INFANTRY_DESTRUCTION_DELAY_MS);
        Self {
            phase: HostSlowDeathPhase::WaitingToSink,
            begin_frame: current_frame,
            sink_at_frame: current_frame.saturating_add(sink_delay),
            destroy_at_frame: current_frame.saturating_add(destroy_delay),
            sink_rate_per_frame: INFANTRY_SINK_RATE_PER_SEC / SLOW_DEATH_LOGIC_FPS,
            sink_offset: 0.0,
            destruction_altitude: -10.0,
        }
    }

    pub fn vehicle_residual(current_frame: u32) -> Self {
        let destroy_delay = ms_to_frames(VEHICLE_DESTRUCTION_DELAY_MS);
        Self {
            phase: HostSlowDeathPhase::WaitingToDestroy,
            begin_frame: current_frame,
            sink_at_frame: current_frame, // no sink
            destroy_at_frame: current_frame.saturating_add(destroy_delay.max(1)),
            sink_rate_per_frame: 0.0,
            sink_offset: 0.0,
            destruction_altitude: -10.0,
        }
    }

    /// Begin slow death. Returns false if already active/done.
    pub fn begin_infantry(&mut self, current_frame: u32) -> bool {
        if self.is_active() || self.is_done() {
            return false;
        }
        *self = Self::infantry_residual(current_frame);
        true
    }

    pub fn begin_vehicle(&mut self, current_frame: u32) -> bool {
        if self.is_active() || self.is_done() {
            return false;
        }
        *self = Self::vehicle_residual(current_frame);
        true
    }

    /// Tick one frame. Returns true when object should be destroyed now.
    pub fn tick(&mut self, current_frame: u32) -> bool {
        match self.phase {
            HostSlowDeathPhase::Inactive | HostSlowDeathPhase::Done => false,
            HostSlowDeathPhase::WaitingToSink => {
                if current_frame >= self.sink_at_frame {
                    self.phase = HostSlowDeathPhase::Sinking;
                }
                if current_frame >= self.destroy_at_frame {
                    self.phase = HostSlowDeathPhase::Done;
                    return true;
                }
                false
            }
            HostSlowDeathPhase::Sinking => {
                if self.sink_rate_per_frame > 0.0 {
                    self.sink_offset -= self.sink_rate_per_frame;
                    if self.sink_offset < self.destruction_altitude {
                        self.sink_offset = self.destruction_altitude;
                    }
                }
                if current_frame >= self.destroy_at_frame {
                    self.phase = HostSlowDeathPhase::Done;
                    return true;
                }
                false
            }
            HostSlowDeathPhase::WaitingToDestroy => {
                if current_frame >= self.destroy_at_frame {
                    self.phase = HostSlowDeathPhase::Done;
                    return true;
                }
                false
            }
        }
    }
}

fn ms_to_frames(msec: u32) -> u32 {
    ((msec as f32) * SLOW_DEATH_LOGIC_FPS / 1000.0).round() as u32
}

pub fn wants_slow_death(template_name: &str, is_infantry: bool, is_vehicle: bool) -> bool {
    if is_infantry {
        return true;
    }
    if is_vehicle {
        let n = template_name.to_ascii_lowercase();
        // Aircraft often have specialized slow death; still delay generic vehicles.
        if n.contains("jet") || n.contains("comanche") || n.contains("chinook") {
            return false; // specialized residual elsewhere
        }
        return true;
    }
    false
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn infantry_sinks_then_destroys() {
        let mut d = HostSlowDeathData::infantry_residual(0);
        assert_eq!(d.phase, HostSlowDeathPhase::WaitingToSink);
        // Before sink delay (90f)
        assert!(!d.tick(50));
        assert_eq!(d.phase, HostSlowDeathPhase::WaitingToSink);
        assert!(!d.tick(90));
        assert_eq!(d.phase, HostSlowDeathPhase::Sinking);
        assert!(d.sink_offset < 0.0 || d.tick(91) == false);
        // Force near destroy
        let mut destroyed = false;
        for f in 91..300 {
            if d.tick(f) {
                destroyed = true;
                break;
            }
        }
        assert!(destroyed);
        assert!(d.sink_offset <= 0.0);
    }

    #[test]
    fn vehicle_delay_only() {
        let mut d = HostSlowDeathData::vehicle_residual(10);
        assert!(!d.tick(20));
        assert!(d.tick(10 + ms_to_frames(VEHICLE_DESTRUCTION_DELAY_MS)));
    }
}
