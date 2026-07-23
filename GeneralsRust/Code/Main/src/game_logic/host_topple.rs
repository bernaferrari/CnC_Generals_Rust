//! Host ToppleUpdate residual (trees / crushable props).
//!
//! C++: `ToppleUpdate` + `Object::topple` + collide-driven topple when crusher_level > 1.
//! Residual playability slice:
//! - Upright → Falling → Down state machine
//! - Angular accumulation to ~π/2 then optional bounce or stop
//! - On down + kill_when_toppled: death DEATH_TOPPLED via unresistable kill
//! - Collide residual: crusher_level > 1 applies topple force away from crusher
//!
//! Fail-closed:
//! - Not full drawable sway stop / shadow disable / stump OCL spawn
//! - Not full script adjustToppleDirection / left-or-right constraint
//! - Not full matrix pre-rotate drawable presentation (presentation may sample pose later)

use serde::{Deserialize, Serialize};

/// C++ TOPPLE_OPTIONS_NONE
pub const TOPPLE_OPTIONS_NONE: u32 = 0;
/// C++ TOPPLE_OPTIONS_NO_BOUNCE
pub const TOPPLE_OPTIONS_NO_BOUNCE: u32 = 0x1;
/// C++ TOPPLE_OPTIONS_NO_FX
pub const TOPPLE_OPTIONS_NO_FX: u32 = 0x2;

/// C++ ANGULAR_LIMIT = PI/2 - PI/64
pub const TOPPLE_ANGULAR_LIMIT: f32 = std::f32::consts::FRAC_PI_2 - std::f32::consts::PI / 64.0;

/// C++ START_VELOCITY_PERCENT
pub const TOPPLE_INITIAL_VELOCITY_PERCENT: f32 = 0.2;
/// C++ START_ACCEL_PERCENT
pub const TOPPLE_INITIAL_ACCEL_PERCENT: f32 = 0.01;
/// C++ VELOCITY_BOUNCE_PERCENT
pub const TOPPLE_BOUNCE_VELOCITY_PERCENT: f32 = 0.3;
/// C++ VELOCITY_BOUNCE_LIMIT
pub const TOPPLE_VELOCITY_BOUNCE_LIMIT: f32 = 0.01;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum HostToppleState {
    #[default]
    Upright = 0,
    Falling = 1,
    Down = 2,
}

/// Per-object ToppleUpdate residual runtime.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HostToppleData {
    pub state: HostToppleState,
    pub dir_x: f32,
    pub dir_y: f32,
    pub angular_velocity: f32,
    pub angular_acceleration: f32,
    pub angular_accumulation: f32,
    pub options: u32,
    pub kill_when_toppled: bool,
    pub kill_when_start_toppled: bool,
    /// Presentation residual: accumulated lean radians about fall axis.
    pub lean_radians: f32,
}

impl Default for HostToppleData {
    fn default() -> Self {
        Self {
            state: HostToppleState::Upright,
            dir_x: 0.0,
            dir_y: 0.0,
            angular_velocity: 0.0,
            angular_acceleration: 0.0,
            angular_accumulation: 0.0,
            options: TOPPLE_OPTIONS_NONE,
            kill_when_toppled: true,
            kill_when_start_toppled: false,
            lean_radians: 0.0,
        }
    }
}

impl HostToppleData {
    pub fn is_able_to_be_toppled(&self) -> bool {
        self.state == HostToppleState::Upright
    }

    /// C++ ToppleUpdate::applyTopplingForce residual.
    /// Returns true if object should be killed immediately (killWhenStartToppled).
    pub fn apply_toppling_force(
        &mut self,
        dir_x: f32,
        dir_y: f32,
        topple_speed: f32,
        options: u32,
    ) -> bool {
        if self.state != HostToppleState::Upright {
            return false;
        }
        if self.kill_when_start_toppled {
            self.state = HostToppleState::Down;
            return true;
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
        let speed = topple_speed.max(0.0);
        self.dir_x = dx;
        self.dir_y = dy;
        self.angular_velocity = speed * TOPPLE_INITIAL_VELOCITY_PERCENT;
        self.angular_acceleration = speed * TOPPLE_INITIAL_ACCEL_PERCENT;
        self.angular_accumulation = 0.0;
        self.lean_radians = 0.0;
        self.options = options;
        self.state = HostToppleState::Falling;
        false
    }

    /// C++ ToppleUpdate::update one logic frame.
    /// Returns true when the object should die via DEATH_TOPPLED this frame.
    pub fn tick(&mut self) -> bool {
        if self.state != HostToppleState::Falling {
            return false;
        }
        let mut cur_vel = self.angular_velocity;
        if self.angular_accumulation + cur_vel > TOPPLE_ANGULAR_LIMIT {
            cur_vel = TOPPLE_ANGULAR_LIMIT - self.angular_accumulation;
        }
        self.lean_radians += cur_vel;
        self.angular_accumulation += cur_vel;

        if self.angular_accumulation >= TOPPLE_ANGULAR_LIMIT - 1e-6 && self.angular_velocity > 0.0 {
            // Hit ground: bounce or stop.
            self.angular_velocity *= -TOPPLE_BOUNCE_VELOCITY_PERCENT;
            let no_bounce = (self.options & TOPPLE_OPTIONS_NO_BOUNCE) != 0;
            if no_bounce || self.angular_velocity.abs() < TOPPLE_VELOCITY_BOUNCE_LIMIT {
                self.angular_velocity = 0.0;
                self.state = HostToppleState::Down;
                return self.kill_when_toppled;
            }
        } else {
            self.angular_velocity += self.angular_acceleration;
        }
        false
    }
}

/// Name peel: trees / shrubs / light poles that accept ToppleUpdate residual.
pub fn is_topple_capable_template(name: &str) -> bool {
    let n = name.to_ascii_lowercase();
    n.contains("tree")
        || n.contains("shrub")
        || n.contains("bush")
        || n.contains("palm")
        || n.contains("pine")
        || n.contains("oak")
        || n.contains("birch")
        || n.contains("lightpole")
        || n.contains("streetlight")
        || n.contains("sign")
        || n.contains("fence")
}

/// C++ onCollide residual: crusher_level > 1 can topple.
pub fn crusher_can_topple(crusher_level: u8) -> bool {
    crusher_level > 1
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn topple_falls_and_kills_without_bounce() {
        let mut t = HostToppleData::default();
        assert!(t.is_able_to_be_toppled());
        assert!(!t.apply_toppling_force(1.0, 0.0, 1.0, TOPPLE_OPTIONS_NO_BOUNCE));
        assert_eq!(t.state, HostToppleState::Falling);
        let mut killed = false;
        for _ in 0..600 {
            if t.tick() {
                killed = true;
                break;
            }
        }
        assert!(killed, "should reach down and request kill");
        assert_eq!(t.state, HostToppleState::Down);
        assert!(t.lean_radians > 1.0);
    }

    #[test]
    fn start_topple_kill_immediate() {
        let mut t = HostToppleData {
            kill_when_start_toppled: true,
            ..Default::default()
        };
        assert!(t.apply_toppling_force(0.0, 1.0, 2.0, TOPPLE_OPTIONS_NONE));
        assert_eq!(t.state, HostToppleState::Down);
    }
}
