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
/// C++ THETA_CEILING — crush only when remaining angle to ground ≤ this.
pub const STRUCTURE_TOPPLE_THETA_CEILING: f32 = std::f32::consts::PI / 6.0;
/// C++ WEAPON_SPACING_PERPENDICULAR residual.
pub const STRUCTURE_TOPPLE_WEAPON_SPACING: f32 = 25.0;
/// Residual crush damage per sample (fail-closed vs full WeaponTemplate).
pub const STRUCTURE_TOPPLE_CRUSH_DAMAGE: f32 = 99999.0;
/// Default building height residual when geometry missing.
pub const STRUCTURE_TOPPLE_DEFAULT_HEIGHT: f32 = 40.0;
/// Default facing half-width residual.
pub const STRUCTURE_TOPPLE_DEFAULT_FACING_WIDTH: f32 = 20.0;

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
    /// C++ m_lastCrushedLocation residual (distance along fall already crushed).
    pub last_crushed_location: f32,
    /// Building height residual for crush projection.
    pub building_height: f32,
    /// Facing half-width residual for crush line.
    pub facing_width: f32,
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
            last_crushed_location: 0.0,
            building_height: STRUCTURE_TOPPLE_DEFAULT_HEIGHT,
            facing_width: STRUCTURE_TOPPLE_DEFAULT_FACING_WIDTH,
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
        self.last_crushed_location = 0.0;
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

/// World-space crush sample from structure topple sweep.
#[derive(Debug, Clone, Copy)]
pub struct StructureToppleCrushSample {
    pub x: f32,
    pub z: f32,
    pub damage: f32,
}

impl HostStructureToppleData {
    /// Remaining angle to ground (C++ theta passed to applyCrushingDamage).
    pub fn remaining_theta(&self) -> f32 {
        (std::f32::consts::FRAC_PI_2 - self.accumulated_angle).max(0.0)
    }

    /// C++ applyCrushingDamage residual: samples along fall direction when near ground.
    /// Updates `last_crushed_location`. Returns empty if theta still above ceiling.
    pub fn take_crush_sweep_samples(
        &mut self,
        building_x: f32,
        building_z: f32,
    ) -> Vec<StructureToppleCrushSample> {
        let theta = self.remaining_theta();
        if theta > STRUCTURE_TOPPLE_THETA_CEILING
            && self.state == HostStructureToppleState::Toppling
        {
            return Vec::new();
        }
        // When WaitingForDone / just hit ground, force final sweep (theta≈0).
        let theta = if matches!(
            self.state,
            HostStructureToppleState::WaitingForDone | HostStructureToppleState::Done
        ) {
            0.0
        } else {
            theta
        };
        if self.state == HostStructureToppleState::Toppling
            && theta > STRUCTURE_TOPPLE_THETA_CEILING
        {
            return Vec::new();
        }

        let height = self.building_height.max(1.0);
        // maxDistance = height * (1 - sin(theta))
        let max_distance = height * (1.0 - theta.sin()).max(0.0);
        if max_distance <= self.last_crushed_location + 1e-3 {
            return Vec::new();
        }

        let mut samples = Vec::new();
        let topple_angle = self.dir_y.atan2(self.dir_x);
        let cos_t = topple_angle.cos();
        let sin_t = topple_angle.sin();
        let facing = self.facing_width.max(1.0);
        let px = -sin_t; // perpendicular to topple dir
        let pz = cos_t;

        // C++: for (j = last; j < maxDistance; j += spacing) doDamageLine; then final at max.
        let mut j = self.last_crushed_location;
        while j < max_distance {
            let jcos = j * cos_t;
            let jsin = j * sin_t;
            for k in [-1.0_f32, 0.0, 1.0] {
                samples.push(StructureToppleCrushSample {
                    x: building_x + jcos + px * facing * k,
                    z: building_z + jsin + pz * facing * k,
                    damage: STRUCTURE_TOPPLE_CRUSH_DAMAGE,
                });
            }
            let next = j + STRUCTURE_TOPPLE_WEAPON_SPACING;
            if next >= max_distance {
                if (max_distance - j).abs() > 1e-3 {
                    let jcos = max_distance * cos_t;
                    let jsin = max_distance * sin_t;
                    for k in [-1.0_f32, 0.0, 1.0] {
                        samples.push(StructureToppleCrushSample {
                            x: building_x + jcos + px * facing * k,
                            z: building_z + jsin + pz * facing * k,
                            damage: STRUCTURE_TOPPLE_CRUSH_DAMAGE,
                        });
                    }
                }
                break;
            }
            j = next;
        }
        self.last_crushed_location = max_distance;
        samples
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn crush_sweep_emits_near_ground() {
        let mut t = HostStructureToppleData::default();
        t.begin(0, 1.0, 0.0, 0);
        t.state = HostStructureToppleState::Toppling;
        t.accumulated_angle = 0.1;
        assert!(t.take_crush_sweep_samples(0.0, 0.0).is_empty());
        t.accumulated_angle = std::f32::consts::FRAC_PI_2 - 0.1;
        t.last_crushed_location = 0.0;
        let s = t.take_crush_sweep_samples(0.0, 0.0);
        assert!(!s.is_empty(), "expected crush samples near ground");
        assert!(s.iter().all(|p| p.damage > 0.0));
    }

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
