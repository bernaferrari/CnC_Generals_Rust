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
//! Live path applies leftover `isUnderwater` waterZ (not lakebed) and the
//! C++ instance matrix (`Rotate_Z` heading / `Rotate_Y` yaw / `Rotate_X` pitch)
//! remapped to host Y-up on GameClient drawables and unit meshes.

use serde::{Deserialize, Serialize};

/// C++ sway coefficients residual.
pub const FLOAT_YAW_PHASE: f32 = 0.0291;
pub const FLOAT_PITCH_PHASE: f32 = 0.0515;
pub const FLOAT_SWAY_AMP: f32 = 0.05;

/// Leftover `TerrainLogic::isUnderwater` waterZ at host map XZ (Y-up height).
///
/// C++ writes waterZ only when a water handle exists (polygon / grid).
/// None means no water table — do not snap to lakebed.
pub fn receiving_water_surface_y(
    tl: &gamelogic::terrain::TerrainLogic,
    map_x: f32,
    map_y: f32,
) -> Option<f32> {
    if tl.get_water_handle(map_x, map_y).is_none() {
        return None;
    }
    let mut water_z = 0.0;
    let _ = tl.is_underwater(map_x, map_y, Some(&mut water_z), None);
    Some(water_z)
}

/// FloatUpdate.cpp106 reads current instance Get_Z_Rotation, not Object heading.
pub(crate) fn rebuild_receiving_instance(previous: glam::Mat4, yaw: f32, pitch: f32) -> glam::Mat4 {
    let basis = glam::Mat4::from_cols(glam::Vec4::X, glam::Vec4::Z, glam::Vec4::Y, glam::Vec4::W);
    let cpp = basis * previous * basis;
    let z = cpp.x_axis.y.atan2(cpp.x_axis.x);
    basis
        * (glam::Mat4::from_rotation_z(z)
            * glam::Mat4::from_rotation_y(yaw)
            * glam::Mat4::from_rotation_x(pitch))
        * basis
}

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
        if self.enabled { water_y } else { None }
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
        let mx = rebuild_receiving_instance(glam::Mat4::IDENTITY, d.yaw, d.pitch);
        assert!(mx.is_finite());
    }

    #[test]
    fn source_float_retains_prior_instance_z_rotation_not_object_heading() {
        let basis =
            glam::Mat4::from_cols(glam::Vec4::X, glam::Vec4::Z, glam::Vec4::Y, glam::Vec4::W);
        let cpp = glam::Mat4::from_rotation_z(0.37) * glam::Mat4::from_rotation_y(0.2);
        let result = rebuild_receiving_instance(basis * cpp * basis, 0.12, -0.08);
        let expected = basis
            * (glam::Mat4::from_rotation_z(0.37)
                * glam::Mat4::from_rotation_y(0.12)
                * glam::Mat4::from_rotation_x(-0.08))
            * basis;
        assert!(
            result
                .to_cols_array()
                .iter()
                .zip(expected.to_cols_array())
                .all(|(a, b)| (a - b).abs() < 1e-6)
        );
    }
}
