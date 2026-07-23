//! Host StructureCollapseUpdate residual (buildings sink/collapse on death).
//!
//! C++: `StructureCollapseUpdate::onDie` → delay shudder → sink with gravity
//! damping → POST_COLLAPSE / done.
//!
//! Residual playability slice:
//! - States: Standing → WaitingForStart → Collapsing → Done
//! - Delay frames (default 15–30 @ 30 FPS ≈ 500–1000 ms retail)
//! - Vertical sink offset for presentation (`collapse_height_offset`)
//! - Shudder residual (horizontal noise magnitude, presentation only)
//! - On done: DEATH_TOPPLED + destroy (rubble/post-collapse residual)
//!
//! Fail-closed:
//! - Not full OCL/FX phase bursts / bone debris
//! - Not full drawable instance-matrix client shudder
//! - Not full DieMux death-type filters

use serde::{Deserialize, Serialize};

/// C++ COLLAPSE_ACCELERATION uses GlobalData gravity residual.
pub const STRUCTURE_COLLAPSE_GRAVITY: f32 = -1.0;
/// Default collapse damping residual (0 = full gravity).
pub const STRUCTURE_COLLAPSE_DAMPING_DEFAULT: f32 = 0.0;
/// Default max shudder residual (client visual).
pub const STRUCTURE_COLLAPSE_MAX_SHUDDER: f32 = 0.6;
/// Default min/max collapse delay frames (500–1000 ms → 15–30 f).
pub const STRUCTURE_COLLAPSE_DELAY_MIN: u32 = 15;
pub const STRUCTURE_COLLAPSE_DELAY_MAX: u32 = 30;
/// Default geometry height residual when unknown.
pub const STRUCTURE_COLLAPSE_DEFAULT_HEIGHT: f32 = 35.0;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum HostStructureCollapseState {
    #[default]
    Standing = 0,
    WaitingForStart = 1,
    Collapsing = 2,
    Done = 3,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HostStructureCollapseData {
    pub state: HostStructureCollapseState,
    pub collapse_start_frame: u32,
    pub collapse_velocity: f32,
    /// C++ m_currentHeight (negative as building sinks into ground).
    pub current_height: f32,
    pub collapse_damping: f32,
    pub max_shudder: f32,
    pub building_height: f32,
    /// Presentation lean unused; use height offset + shudder.
    pub shudder_x: f32,
    pub shudder_z: f32,
}

impl Default for HostStructureCollapseData {
    fn default() -> Self {
        Self {
            state: HostStructureCollapseState::Standing,
            collapse_start_frame: 0,
            collapse_velocity: 0.0,
            current_height: 0.0,
            collapse_damping: STRUCTURE_COLLAPSE_DAMPING_DEFAULT,
            max_shudder: STRUCTURE_COLLAPSE_MAX_SHUDDER,
            building_height: STRUCTURE_COLLAPSE_DEFAULT_HEIGHT,
            shudder_x: 0.0,
            shudder_z: 0.0,
        }
    }
}

impl HostStructureCollapseData {
    pub fn is_standing(&self) -> bool {
        self.state == HostStructureCollapseState::Standing
    }

    pub fn is_active(&self) -> bool {
        matches!(
            self.state,
            HostStructureCollapseState::WaitingForStart | HostStructureCollapseState::Collapsing
        )
    }

    /// Presentation vertical offset (negative sinks mesh).
    pub fn collapse_height_offset(&self) -> f32 {
        self.current_height
    }

    /// C++ beginStructureCollapse residual.
    pub fn begin(&mut self, current_frame: u32, delay_frames: u32) {
        if !self.is_standing() {
            return;
        }
        self.collapse_start_frame = current_frame.saturating_add(delay_frames);
        self.collapse_velocity = 0.0;
        self.current_height = 0.0;
        self.shudder_x = 0.0;
        self.shudder_z = 0.0;
        self.state = HostStructureCollapseState::WaitingForStart;
    }

    /// Deterministic shudder peel (logic-synced residual, not client RNG).
    fn update_shudder(&mut self, frame: u32) {
        if self.max_shudder <= 0.0 {
            self.shudder_x = 0.0;
            self.shudder_z = 0.0;
            return;
        }
        // Cheap deterministic oscillation residual.
        let t = frame as f32 * 0.37;
        self.shudder_x = (t.sin()) * self.max_shudder;
        self.shudder_z = ((t * 1.3).cos()) * self.max_shudder;
    }

    /// One logic frame. Returns true when collapse completes.
    pub fn tick(&mut self, current_frame: u32) -> bool {
        match self.state {
            HostStructureCollapseState::Standing | HostStructureCollapseState::Done => false,
            HostStructureCollapseState::WaitingForStart => {
                self.update_shudder(current_frame);
                if current_frame >= self.collapse_start_frame {
                    self.state = HostStructureCollapseState::Collapsing;
                    self.collapse_velocity = 0.0;
                }
                false
            }
            HostStructureCollapseState::Collapsing => {
                // C++: m_currentHeight -= m_collapseVelocity;
                // m_collapseVelocity -= gravity * (1 - damping);
                // Note gravity is negative → velocity becomes more negative → height decreases.
                self.current_height -= self.collapse_velocity;
                self.collapse_velocity -=
                    STRUCTURE_COLLAPSE_GRAVITY * (1.0 - self.collapse_damping);
                self.update_shudder(current_frame);
                // Done when fully below ground: height + buildingHeight <= 0.
                if self.current_height + self.building_height <= 0.0 {
                    self.current_height = -self.building_height;
                    self.shudder_x = 0.0;
                    self.shudder_z = 0.0;
                    self.state = HostStructureCollapseState::Done;
                    return true;
                }
                false
            }
        }
    }
}

/// Civilian / prop buildings prefer StructureCollapse over StructureTopple.
pub fn prefers_structure_collapse(template_name: &str) -> bool {
    let n = template_name.to_ascii_lowercase();
    if n.contains("warfactory")
        || n.contains("barracks")
        || n.contains("commandcenter")
        || n.contains("command_center")
        || n.contains("airfield")
        || n.contains("helipad")
        || n.contains("strategycenter")
        || n.contains("supplycenter")
        || n.contains("powerplant")
        || n.contains("nuclear")
        || n.contains("scud")
        || n.contains("stinger")
        || n.contains("patriot")
        || n.contains("firebase")
        || n.contains("gattling")
        || n.contains("tunnel")
        || n.contains("bunker") && !n.contains("civilian")
    {
        return false; // military → topple residual
    }
    n.contains("civilian")
        || n.contains("barn")
        || n.contains("house")
        || n.contains("hut")
        || n.contains("shack")
        || n.contains("store")
        || n.contains("shop")
        || n.contains("church")
        || n.contains("temple")
        || n.contains("farm")
        || n.contains("stable")
        || n.contains("garage")
        || n.contains("office")
        || n.contains("apartment")
        || n.contains("building")
        || n.contains("tower") && n.contains("water")
        || n.contains("silo")
        || n.contains("warehouse")
        || n.contains("hangar") && n.contains("civ")
}

pub fn is_structure_collapse_candidate(template_name: &str, is_structure: bool) -> bool {
    is_structure && prefers_structure_collapse(template_name)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn structure_collapse_sinks_and_completes() {
        let mut c = HostStructureCollapseData::default();
        c.building_height = 20.0;
        c.begin(0, 0);
        assert_eq!(c.state, HostStructureCollapseState::WaitingForStart);
        let mut done = false;
        for f in 0..600 {
            if c.tick(f) {
                done = true;
                break;
            }
        }
        assert!(done);
        assert_eq!(c.state, HostStructureCollapseState::Done);
        assert!(c.collapse_height_offset() <= -20.0 + 1e-3);
    }

    #[test]
    fn civilian_prefers_collapse() {
        assert!(prefers_structure_collapse("CivilianBarn01"));
        assert!(!prefers_structure_collapse("AmericaWarFactory"));
    }
}
