//! Host FloatUpdate residual (boats bob / snap to water surface).
//!
//! C++: `FloatUpdate::update`
//! - When `Enabled`: snap object height to water surface (`isUnderwater` Z)
//! - Always: drawable yaw/pitch sway from frame sine residual
//!
//! Retail peels (`CivilianUnit.ini` ferry/boats):
//! - `Enabled = No` — sway only, do not lift off path height
//!
//! Host Y-up: water height is world Y.
//!
//! Fail-closed: not full Drawable instance matrix scrub / TerrainLogic wave mesh.

use serde::{Deserialize, Serialize};

/// C++ sway coefficients residual.
pub const FLOAT_YAW_PHASE: f32 = 0.0291;
pub const FLOAT_PITCH_PHASE: f32 = 0.0515;
pub const FLOAT_SWAY_AMP: f32 = 0.05;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HostFloatUpdateData {
    pub enabled: bool,
    /// Last computed sway (radians residual for client).
    pub yaw: f32,
    pub pitch: f32,
}

impl Default for HostFloatUpdateData {
    fn default() -> Self {
        Self {
            // Retail boat peel: Enabled = No
            enabled: false,
            yaw: 0.0,
            pitch: 0.0,
        }
    }
}

impl HostFloatUpdateData {
    pub fn new(enabled: bool) -> Self {
        Self {
            enabled,
            ..Self::default()
        }
    }

    pub fn for_template(template_name: &str) -> Option<Self> {
        if is_float_update_template(template_name) {
            // Retail civilian boats: Enabled = No (sway only).
            Some(Self::new(false))
        } else {
            None
        }
    }

    pub fn set_enabled(&mut self, enabled: bool) {
        self.enabled = enabled;
    }

    /// C++ drawable sway residual.
    pub fn tick_sway(&mut self, frame: u32) {
        let angle = frame as f32;
        self.yaw = (angle * FLOAT_YAW_PHASE).sin() * FLOAT_SWAY_AMP;
        self.pitch = (angle * FLOAT_PITCH_PHASE).sin() * FLOAT_SWAY_AMP;
    }

    /// When enabled, return water surface Y to snap to (host Y-up).
    pub fn snap_height_y(&self, water_y: Option<f32>) -> Option<f32> {
        if self.enabled {
            water_y
        } else {
            None
        }
    }
}

/// Civilian ferry / boat templates.
pub fn is_float_update_template(name: &str) -> bool {
    let n = name.to_ascii_lowercase();
    n.contains("ferry")
        || n.contains("civilianvehicleboat")
        || n.contains("civilianboat")
        || (n.contains("boat") && n.contains("civilian"))
        || n.contains("fishingboat")
        || n.contains("tugboat")
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct HostFloatUpdateRegistry {
    pub installed: u32,
    pub sway_ticks: u32,
    pub snaps: u32,
}

impl HostFloatUpdateRegistry {
    pub fn new() -> Self {
        Self::default()
    }
    pub fn clear(&mut self) {
        *self = Self::default();
    }
    pub fn record_install(&mut self) {
        self.installed = self.installed.saturating_add(1);
    }
    pub fn record_sway(&mut self) {
        self.sway_ticks = self.sway_ticks.saturating_add(1);
    }
    pub fn record_snap(&mut self) {
        self.snaps = self.snaps.saturating_add(1);
    }
    pub fn honesty_host_path_ok(&self) -> bool {
        self.installed > 0 || self.sway_ticks > 0
    }
}

pub fn honesty_float_update_residual_ok() -> bool {
    (FLOAT_YAW_PHASE - 0.0291).abs() < 1.0e-6
        && (FLOAT_PITCH_PHASE - 0.0515).abs() < 1.0e-6
        && (FLOAT_SWAY_AMP - 0.05).abs() < 1.0e-6
        && is_float_update_template("CivilianVehicleFerry")
        && !is_float_update_template("AmericaTankCrusader")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn residual_pack_and_sway() {
        assert!(honesty_float_update_residual_ok());
        let mut d = HostFloatUpdateData::new(false);
        d.tick_sway(100);
        assert!(d.yaw.abs() <= FLOAT_SWAY_AMP + 1.0e-5);
        assert!(d.snap_height_y(Some(12.0)).is_none());
        d.set_enabled(true);
        assert_eq!(d.snap_height_y(Some(12.0)), Some(12.0));
    }
}
