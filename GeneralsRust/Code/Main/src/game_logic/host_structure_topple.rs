//! Host StructureToppleUpdate residual (buildings fall after HP death).
//!
//! C++: `StructureToppleUpdate::onDie` → `beginStructureTopple` → delayed fall
//! with structural integrity decay, crushing sweep, then done/rubble.
//!
//! Residual playability slice:
//! - States: Standing → WaitingForStart → Toppling → WaitingForDone → Done
//! - Delay frames before fall (default min/max 0 → immediate start residual)
//! - Angular accumulation to π/2 with accel factor 0.02
//! - Presentation lean via `lean_radians` (shared with tree topple presentation)
//! - On done: mark destroyed + DEATH_TOPPLED (rubble phase residual)
//!
//! Fail-closed:
//! - Not full FX burst / OCL rubble / BoneFX / crush sweep damage matrix
//! - Not full DieMux death-type filters
//! - Not deselect-all-players network residual

use serde::{Deserialize, Serialize};

/// C++ TOPPLE_ACCELERATION_FACTOR
pub const STRUCTURE_TOPPLE_ACCEL_FACTOR: f32 = 0.02;
/// Default structural integrity residual (INI StructuralIntegrity).
pub const STRUCTURE_TOPPLE_INTEGRITY_DEFAULT: f32 = 0.5;
/// Default structural decay residual per frame (INI StructuralDecay).
pub const STRUCTURE_TOPPLE_DECAY_DEFAULT: f32 = 0.1;
/// Default min/max topple delay frames when unset.
pub const STRUCTURE_TOPPLE_DELAY_MIN: u32 = 0;
pub const STRUCTURE_TOPPLE_DELAY_MAX: u32 = 0;
/// Waiting-for-done frames residual (brief settle).
pub const STRUCTURE_TOPPLE_DONE_DELAY_FRAMES: u32 = 1;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum HostStructureToppleState {
    #[default]
    Standing = 0,
    WaitingForStart = 1,
    Toppling = 2,
    WaitingForDone = 3,
    Done = 4,
}

/// Per-structure StructureToppleUpdate residual.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HostStructureToppleData {
    pub state: HostStructureToppleState,
    pub topple_start_frame: u32,
    pub dir_x: f32,
    pub dir_y: f32,
    pub topple_velocity: f32,
    pub accumulated_angle: f32,
    pub structural_integrity: f32,
    pub structural_decay: f32,
    pub done_frame: u32,
    /// Presentation lean (radians) — mirrors tree topple lean field consumers.
    pub lean_radians: f32,
}

impl Default for HostStructureToppleData {
    fn default() -> Self {
        Self {
            state: HostStructureToppleState::Standing,
            topple_start_frame: 0,
            dir_x: 1.0,
            dir_y: 0.0,
            topple_velocity: 0.0,
            accumulated_angle: 0.0,
            structural_integrity: STRUCTURE_TOPPLE_INTEGRITY_DEFAULT,
            structural_decay: STRUCTURE_TOPPLE_DECAY_DEFAULT,
            done_frame: 0,
            lean_radians: 0.0,
        }
    }
}

impl HostStructureToppleData {
    pub fn is_standing(&self) -> bool {
        self.state == HostStructureToppleState::Standing
    }

    pub fn is_active(&self) -> bool {
        !matches!(
            self.state,
            HostStructureToppleState::Standing | HostStructureToppleState::Done
        )
    }

    /// C++ beginStructureTopple residual.
    pub fn begin(&mut self, current_frame: u32, dir_x: f32, dir_y: f32, delay_frames: u32) {
        if !self.is_standing() {
            return;
        }
        let mut dx = dir_x;
        let mut dy = dir_y;
        let len = (dx * dx + dy * dy).sqrt();
        if len > 1e-6 {
            dx /= len;
            dy /= len;
        } else {
            dx = 1.0;
            dy = 0.0;
        }
        self.dir_x = dx;
        self.dir_y = dy;
        self.topple_start_frame = current_frame.saturating_add(delay_frames);
        self.topple_velocity = 0.0;
        self.accumulated_angle = 0.0;
        self.lean_radians = 0.0;
        self.structural_integrity = STRUCTURE_TOPPLE_INTEGRITY_DEFAULT;
        self.state = HostStructureToppleState::WaitingForStart;
    }

    /// One logic frame. Returns true when topple completes (doToppleDoneStuff).
    pub fn tick(&mut self, current_frame: u32) -> bool {
        match self.state {
            HostStructureToppleState::Standing | HostStructureToppleState::Done => false,
            HostStructureToppleState::WaitingForStart => {
                if current_frame >= self.topple_start_frame {
                    self.state = HostStructureToppleState::Toppling;
                    self.structural_integrity = STRUCTURE_TOPPLE_INTEGRITY_DEFAULT;
                }
                false
            }
            HostStructureToppleState::Toppling => {
                let integrity_term = (1.0 - self.structural_integrity).max(0.0);
                let topple_acceleration =
                    STRUCTURE_TOPPLE_ACCEL_FACTOR * self.accumulated_angle.sin() * integrity_term;
                // C++ also accelerates from rest: give a small kick if still zero.
                let accel = if self.topple_velocity <= 1e-6 && self.accumulated_angle <= 1e-6 {
                    STRUCTURE_TOPPLE_ACCEL_FACTOR * 0.05
                } else {
                    topple_acceleration.max(STRUCTURE_TOPPLE_ACCEL_FACTOR * 0.01)
                };
                self.topple_velocity += accel;
                if self.structural_integrity > 0.0 {
                    self.structural_integrity *= self.structural_decay;
                    if self.structural_integrity < 0.0 {
                        self.structural_integrity = 0.0;
                    }
                }
                self.accumulated_angle += self.topple_velocity;
                self.lean_radians = self.accumulated_angle;
                if self.accumulated_angle >= std::f32::consts::FRAC_PI_2 {
                    self.accumulated_angle = std::f32::consts::FRAC_PI_2;
                    self.lean_radians = self.accumulated_angle;
                    self.state = HostStructureToppleState::WaitingForDone;
                    self.done_frame =
                        current_frame.saturating_add(STRUCTURE_TOPPLE_DONE_DELAY_FRAMES);
                }
                false
            }
            HostStructureToppleState::WaitingForDone => {
                if current_frame >= self.done_frame {
                    self.state = HostStructureToppleState::Done;
                    return true;
                }
                false
            }
        }
    }
}

/// Name/kind peel: structures that should structure-topple on death.
pub fn is_structure_topple_candidate(template_name: &str, is_structure: bool) -> bool {
    if !is_structure {
        return false;
    }
    let n = template_name.to_ascii_lowercase();
    // Skip pure base pads / holes / walls that may not topple in retail.
    if n.contains("rebuildhole")
        || n.contains("bunker") && n.contains("tunnel")
        || n.contains("supplydock")
        || n.contains("oil")
    {
        return false;
    }
    true
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn structure_topple_reaches_done() {
        let mut t = HostStructureToppleData::default();
        t.begin(0, 1.0, 0.0, 0);
        assert_eq!(t.state, HostStructureToppleState::WaitingForStart);
        let mut done = false;
        for f in 0..600 {
            if t.tick(f) {
                done = true;
                break;
            }
        }
        assert!(done, "should complete topple");
        assert_eq!(t.state, HostStructureToppleState::Done);
        assert!((t.lean_radians - std::f32::consts::FRAC_PI_2).abs() < 1e-3);
    }
}
