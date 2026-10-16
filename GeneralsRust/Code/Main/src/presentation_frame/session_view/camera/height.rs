//! W3DView's fixed map basis and height-above-ground policy.
//! Source: W3DView.cpp:115-144, 1308-1345, 1902-1975, 2354-2365.
use glam::Vec3;

/// Original XYZ coordinates. Conversion to Main's X/Z ground plane happens
/// only at the pose boundary; HAG always uses the unrotated source Z.
#[derive(Debug)]
pub(crate) struct CameraBasis {
    offset: Vec3,
    ground_level: f32,
}

impl CameraBasis {
    fn from_height(ground_level: f32, offset_z: f32, pitch: f32, yaw: f32) -> Self {
        // PI is a float macro in BaseType.h; 180.0 promotes the surrounding
        // expression to double before tan, then each stored offset is float.
        let radians = f64::from(std::f32::consts::PI) / 180.0;
        let y = -(f64::from(offset_z) / (f64::from(pitch) * radians).tan()) as f32;
        let x = -(f64::from(y) * (f64::from(yaw) * radians).tan()) as f32;
        Self {
            offset: Vec3::new(x, y, offset_z),
            ground_level,
        }
    }

    pub(crate) fn for_map(center_ground: f32, height: f32, pitch: f32, yaw: f32) -> Self {
        let ground = if center_ground > 120.0 {
            120.0
        } else {
            center_ground
        };
        Self::from_height(ground, ground + height, pitch, yaw)
    }

    pub(crate) fn ground_level(&self) -> f32 {
        self.ground_level
    }

    /// Normal (non-FOV) zoom path from buildCameraTransform. Pitch and heading
    /// are independent View deltas, not absolute spherical-orbit angles.
    pub(crate) fn pose(&self, position: Vec3, zoom: f32, pitch: f32, heading: f32) -> (Vec3, Vec3) {
        let mut source = self.offset * zoom;
        let factor = (1.0_f64 - f64::from(self.ground_level / source.z)) as f32;
        source = rotate_source_axis(source, Vec3::X, pitch);
        source = rotate_source_axis(source, Vec3::Z, heading);
        source *= factor;
        source.x += position.x;
        source.y += position.z;
        source.z += self.ground_level;
        (
            Vec3::new(source.x, source.z, source.y),
            Vec3::new(position.x, self.ground_level, position.z),
        )
    }

    /// Recover native saved pitch against the fixed map basis and saved raw
    /// heading. Known heading preserves the side of the zenith, including
    /// unclamped scripted defaults; an eye can never recalibrate the basis.
    pub(crate) fn pitch_from_pose(
        &self,
        position: Vec3,
        eye: Vec3,
        zoom: f32,
        heading: f32,
    ) -> Option<f32> {
        let start = self.offset * zoom;
        let factor = (1.0_f64 - f64::from(self.ground_level / start.z)) as f32;
        if !factor.is_finite() || factor == 0.0 || !eye.is_finite() || !heading.is_finite() {
            return None;
        }
        let final_offset = Vec3::new(
            eye.x - position.x,
            eye.z - position.z,
            eye.y - self.ground_level,
        ) / factor;
        let unheaded = rotate_source_axis(final_offset, Vec3::Z, -heading);
        if !unheaded.is_finite() || (start.y == 0.0 && start.z == 0.0) {
            return None;
        }
        Some(normalize_angle(
            unheaded.z.atan2(unheaded.y) - start.z.atan2(start.y),
        ))
    }
}

/// Matrix3D::Set(axis,s,c) and mulVector3, retaining row arithmetic instead of
/// replacing the source's independent rotations with a quaternion orbit.
fn rotate_source_axis(v: Vec3, axis: Vec3, angle: f32) -> Vec3 {
    let c = angle.cos();
    let s = angle.sin();
    let row0 = Vec3::new(
        axis.x * axis.x + c * (1.0 - axis.x * axis.x),
        axis.x * axis.y * (1.0 - c) - axis.z * s,
        axis.z * axis.x * (1.0 - c) + axis.y * s,
    );
    let row1 = Vec3::new(
        axis.x * axis.y * (1.0 - c) + axis.z * s,
        axis.y * axis.y + c * (1.0 - axis.y * axis.y),
        axis.y * axis.z * (1.0 - c) - axis.x * s,
    );
    let row2 = Vec3::new(
        axis.z * axis.x * (1.0 - c) - axis.y * s,
        axis.y * axis.z * (1.0 - c) + axis.x * s,
        axis.z * axis.z + c * (1.0 - axis.z * axis.z),
    );
    let dot = |row: Vec3| row.x * v.x + row.y * v.y + row.z * v.z + 0.0;
    Vec3::new(dot(row0), dot(row1), dot(row2))
}

pub(crate) fn clamp_pitch(pitch: f32) -> f32 {
    pitch.clamp(-std::f32::consts::PI / 5.0, std::f32::consts::PI / 5.0)
}
fn normalize_angle(mut angle: f32) -> f32 {
    while angle > std::f32::consts::PI {
        angle -= std::f32::consts::TAU;
    }
    while angle < -std::f32::consts::PI {
        angle += std::f32::consts::TAU;
    }
    angle
}

#[derive(Debug)]
pub(crate) struct CameraHeight {
    basis: CameraBasis,
    desired: f32,
    basis_pitch_degrees: f32,
    basis_yaw_degrees: f32,
    adjustment_enabled: bool,
    zoom_limited: bool,
}

#[derive(Clone, Copy)]
pub(crate) struct HeightRules {
    pub min: f32,
    pub max: f32,
    pub enforce_max: bool,
    pub adjust_speed: f32,
    pub scroll_cutoff: f32,
}

pub(crate) struct HeightStep {
    pub terrain: f32,
    pub paused: bool,
    pub scrolling: bool,
    pub scroll_amount: f32,
    pub scripted: bool,
    pub replay_camera: bool,
    pub rules: HeightRules,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub(crate) struct CameraHeightContinuation {
    offset: Vec3,
    ground: f32,
    desired: f32,
    enabled: bool,
    limited: bool,
}

impl CameraHeightContinuation {
    pub(crate) fn validate(&self) -> Result<(), String> {
        if !self.offset.is_finite()
            || self.offset.z == 0.0
            || !self.ground.is_finite()
            || !self.desired.is_finite()
        {
            return Err("invalid fixed camera basis or height".into());
        }
        Ok(())
    }
}

impl CameraHeight {
    /// Source follow writes ground plane without recalibrating map offset.
    pub(crate) fn set_follow_ground(&mut self, ground: f32) {
        self.basis.ground_level = ground;
    }
    pub(crate) fn capture_continuation(&self) -> CameraHeightContinuation {
        CameraHeightContinuation {
            offset: self.basis.offset,
            ground: self.basis.ground_level,
            desired: self.desired,
            enabled: self.adjustment_enabled,
            limited: self.zoom_limited,
        }
    }
    pub(crate) fn admit_continuation(&mut self, state: CameraHeightContinuation) {
        self.basis = CameraBasis {
            offset: state.offset,
            ground_level: state.ground,
        };
        self.desired = state.desired;
        self.adjustment_enabled = state.enabled;
        self.zoom_limited = state.limited;
    }
    pub(crate) fn basis_pitch_degrees(&self) -> f32 {
        self.basis_pitch_degrees
    }
    pub(crate) fn basis_yaw_degrees(&self) -> f32 {
        self.basis_yaw_degrees
    }
    pub(crate) fn admit_owned_continuation(
        &mut self,
        state: CameraHeightContinuation,
        pitch: f32,
        yaw: f32,
    ) {
        self.admit_continuation(state);
        self.basis_pitch_degrees = pitch;
        self.basis_yaw_degrees = yaw;
    }
    /// Direct source path update: preserve offset Z, desired HAG and policy flags.
    pub(crate) fn path_parts(&mut self) -> (&mut f32, &mut Vec3, f32, f32) {
        (
            &mut self.basis.ground_level,
            &mut self.basis.offset,
            self.basis_pitch_degrees,
            self.basis_yaw_degrees,
        )
    }
    pub(crate) fn new(height: f32, pitch: f32, yaw: f32) -> Self {
        Self {
            basis: CameraBasis::from_height(10.0, height, pitch, yaw),
            desired: 0.0,
            basis_pitch_degrees: pitch,
            basis_yaw_degrees: yaw,
            adjustment_enabled: false,
            zoom_limited: true,
        }
    }
    pub(crate) fn initialize_map(&mut self, ground: f32, height: f32, pitch: f32, yaw: f32) {
        self.basis = CameraBasis::for_map(ground, height, pitch, yaw);
        self.basis_pitch_degrees = pitch;
        self.basis_yaw_degrees = yaw;
    }
    pub(crate) fn set_adjustment_enabled(&mut self, enabled: bool) {
        self.adjustment_enabled = enabled;
    }
    pub(crate) fn basis(&self) -> &CameraBasis {
        &self.basis
    }
    pub(crate) fn desired(&self) -> f32 {
        self.desired
    }
    pub(crate) fn zoom_limited(&self) -> bool {
        self.zoom_limited
    }
    pub(crate) fn set_zoom_limited(&mut self, limited: bool) {
        self.zoom_limited = limited;
    }
    pub(crate) fn toggle_zoom_limited(&mut self) -> bool {
        self.zoom_limited = !self.zoom_limited;
        self.zoom_limited
    }
    pub(crate) fn set_height(&mut self, goal: f32, min: f32, max: f32) {
        self.desired = goal;
        if self.zoom_limited {
            if self.desired < min {
                self.desired = min;
            }
            if self.desired > max {
                self.desired = max;
            }
        }
    }
    /// Native full-pose saves predate an explicit HAG goal. Admit their visible
    /// height against this map basis, so restoring a pose does not ease toward
    /// an unrelated startup goal. Original View-only loads keep source defaults.
    pub(crate) fn admit_restored_pose_height(&mut self, zoom: f32, terrain: f32) {
        self.desired = self.basis.offset.z * zoom - terrain;
    }
    pub(crate) fn default_zoom(&self, terrain: f32, max_height: f32) -> f32 {
        (terrain + max_height) / self.basis.offset.z
    }
    pub(crate) fn set_zoom_to_default(&mut self, terrain: f32, max_height: f32) -> f32 {
        let zoom = self.default_zoom(terrain, max_height);
        self.desired = max_height;
        zoom
    }
    pub(crate) fn advance(&mut self, zoom: &mut f32, step: HeightStep) -> bool {
        let current = self.basis.offset.z * *zoom - step.terrain;
        if !self.adjustment_enabled || step.paused {
            return false;
        }
        // C++ calculates from the OLD goal before scripted/replay capture.
        let desired_zoom = (step.terrain + self.desired) / self.basis.offset.z;
        if step.scripted || step.replay_camera {
            self.desired = current;
        }
        let rules = step.rules;
        if step.scrolling {
            if step.scroll_amount < rules.scroll_cutoff
                || current < rules.min
                || (rules.enforce_max && current > rules.max)
            {
                let adjustment = (desired_zoom - *zoom) * rules.adjust_speed;
                if adjustment.abs() >= 0.0001 {
                    *zoom += adjustment;
                    return true;
                }
            }
        } else {
            let adjustment = (*zoom - desired_zoom) * rules.adjust_speed;
            if adjustment.abs() >= 0.0001 && !step.scripted {
                *zoom -= adjustment;
                return true;
            }
        }
        false
    }
}

/// C++ getHeightAroundPos: raw maximum, center then four ordered diagonals.
pub(crate) fn height_around(
    position: Vec3,
    mut sample: impl FnMut(f32, f32) -> Option<f32>,
) -> Option<f32> {
    let mut height = sample(position.x, position.z)?;
    for (dx, dz) in [(40.0, -40.0), (-40.0, -40.0), (40.0, 40.0), (-40.0, 40.0)] {
        let next = sample(position.x + dx, position.z + dz)?;
        if height < next {
            height = next;
        }
    }
    Some(height)
}

#[cfg(test)]
#[path = "../../../cnc_game_engine/camera_height_tests.rs"]
mod tests;
