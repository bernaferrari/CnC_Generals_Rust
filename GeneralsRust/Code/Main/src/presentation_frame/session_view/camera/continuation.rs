//! Detached complete camera value for the V2 session continuation.
//! This record is not the historical Native CameraState or five-field HAG wire.
use super::SessionCamera;
use super::height::CameraHeightContinuation;
use crate::save_load::camera_codec::CameraCodec;
use glam::{Mat4, Vec3};
use serde::{Deserialize, Serialize};

use super::pose::inverse_camera_transform;

#[derive(Debug, Clone, Serialize, Deserialize)]
struct HeightState {
    legacy: CameraHeightContinuation,
    basis_pitch_degrees: f32,
    basis_yaw_degrees: f32,
}

/// Exactly the session camera's 33 fields. Feature-dependent native filter
/// observations use Option so the V2 record has one layout in either build.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct SessionCameraState {
    camera_position: Vec3,
    scripted_camera_constraint_widen: Option<(f32, f32, f32, f32)>,
    camera_zoom: f32,
    camera_zoom_target: Option<f32>,
    camera_zoom_start: f32,
    camera_zoom_duration: f32,
    camera_zoom_frame: u32,
    camera_zoom_ease_in: f32,
    camera_zoom_ease_out: f32,
    native_camera: HeightState,
    native_filter_scroll: Option<glam::Vec2>,
    native_filter_previous_target: Option<Vec3>,
    camera_pitch_radians: f32,
    camera_pitch_target: Option<f32>,
    camera_pitch_start: f32,
    camera_pitch_duration: f32,
    camera_pitch_frame: u32,
    camera_pitch_ease_in: f32,
    camera_pitch_ease_out: f32,
    camera_fx_pitch: f32,
    camera_yaw_radians: f32,
    camera_yaw_target: Option<f32>,
    camera_yaw_start: f32,
    camera_yaw_duration: f32,
    camera_yaw_frame: u32,
    camera_yaw_ease_in: f32,
    camera_yaw_ease_out: f32,
    camera_shake_rotation: Vec3,
    script_camera_shakers: Vec<super::ScriptCameraShaker>,
    camera_slave_mode: Option<crate::game_logic::mission_scripts::CameraSlaveModeRequest>,
    view_matrix: Mat4,
    projection_matrix: Mat4,
    camera_follow_factor: f32,
}

impl SessionCameraState {
    pub(crate) fn historical_registers(
        &self,
    ) -> crate::presentation_frame::camera::registers::RotationRegisters {
        crate::presentation_frame::camera::registers::RotationRegisters::import_v7_scalar(
            self.camera_yaw_start,
            self.camera_yaw_target,
        )
    }
    pub(crate) fn validate_register_observation(
        &self,
        state: &crate::presentation_frame::camera::registers::RotationRegistersState,
    ) -> Result<(), String> {
        state.validate()?;
        let registers =
            crate::presentation_frame::camera::registers::RotationRegisters::restore(state.clone());
        let (start, target) = registers.legacy_scalar(self.camera_yaw_radians);
        if start.to_bits() != self.camera_yaw_start.to_bits()
            || target.map(f32::to_bits) != self.camera_yaw_target.map(f32::to_bits)
        {
            return Err("camera register projection disagrees with historical observation".into());
        }
        Ok(())
    }

    pub(crate) fn historical_rotation(
        &self,
        time: crate::presentation_frame::camera::rotation::RotationTime,
    ) -> crate::presentation_frame::camera::rotation::RotationTiming {
        crate::presentation_frame::camera::rotation::RotationTiming::historical(
            self.camera_yaw_duration,
            self.camera_yaw_ease_in,
            self.camera_yaw_ease_out,
            time,
        )
    }
    pub(crate) fn validate_rotation_observation(
        &self,
        timing: crate::presentation_frame::camera::rotation::RotationTiming,
    ) -> Result<(), String> {
        timing.validate()?;
        if timing.duration_seconds().to_bits() != self.camera_yaw_duration.to_bits()
            || timing.ease_in().to_bits() != self.camera_yaw_ease_in.to_bits()
            || timing.ease_out().to_bits() != self.camera_yaw_ease_out.to_bits()
        {
            return Err("rotation observation disagrees with canonical timing".into());
        }
        Ok(())
    }

    pub(crate) fn capture(camera: &SessionCamera) -> Self {
        Self {
            camera_position: camera.camera_position.clone(),
            scripted_camera_constraint_widen: camera.scripted_camera_constraint_widen.clone(),
            camera_zoom: camera.camera_zoom.clone(),
            camera_zoom_target: camera.camera_zoom_target.clone(),
            camera_zoom_start: camera.camera_zoom_start.clone(),
            camera_zoom_duration: camera.camera_zoom_duration.clone(),
            camera_zoom_frame: camera.camera_zoom_frame.clone(),
            camera_zoom_ease_in: camera.camera_zoom_ease_in.clone(),
            camera_zoom_ease_out: camera.camera_zoom_ease_out.clone(),
            native_camera: HeightState {
                legacy: camera.native_camera.capture_continuation(),
                basis_pitch_degrees: camera.native_camera.basis_pitch_degrees(),
                basis_yaw_degrees: camera.native_camera.basis_yaw_degrees(),
            },
            native_filter_scroll: {
                #[cfg(feature = "game_client")]
                {
                    Some(camera.native_filter_scroll)
                }
                #[cfg(not(feature = "game_client"))]
                {
                    None
                }
            },
            native_filter_previous_target: {
                #[cfg(feature = "game_client")]
                {
                    Some(camera.native_filter_previous_target)
                }
                #[cfg(not(feature = "game_client"))]
                {
                    None
                }
            },
            camera_pitch_radians: camera.camera_pitch_radians.clone(),
            camera_pitch_target: camera.camera_pitch_target.clone(),
            camera_pitch_start: camera.camera_pitch_start.clone(),
            camera_pitch_duration: camera.camera_pitch_duration.clone(),
            camera_pitch_frame: camera.camera_pitch_frame.clone(),
            camera_pitch_ease_in: camera.camera_pitch_ease_in.clone(),
            camera_pitch_ease_out: camera.camera_pitch_ease_out.clone(),
            camera_fx_pitch: camera.camera_fx_pitch.clone(),
            camera_yaw_radians: camera.camera_yaw_radians.clone(),
            camera_yaw_target: camera
                .rotation_registers
                .legacy_scalar(camera.camera_yaw_radians)
                .1,
            camera_yaw_start: camera
                .rotation_registers
                .legacy_scalar(camera.camera_yaw_radians)
                .0,
            camera_yaw_duration: camera.rotation.duration_seconds().clone(),
            camera_yaw_frame: camera.camera_yaw_frame.clone(),
            camera_yaw_ease_in: camera.rotation.ease_in().clone(),
            camera_yaw_ease_out: camera.rotation.ease_out().clone(),
            camera_shake_rotation: camera.camera_shake_rotation.clone(),
            script_camera_shakers: camera.script_camera_shakers.clone(),
            camera_slave_mode: camera.camera_slave_mode.clone(),
            view_matrix: camera.view_matrix.clone(),
            projection_matrix: camera.projection_matrix.clone(),
            camera_follow_factor: camera.camera_follow_factor.clone(),
        }
    }

    pub(crate) fn validate(&self) -> Result<(), String> {
        self.native_camera.legacy.validate()?;
        if !self.native_camera.basis_pitch_degrees.is_finite()
            || !self.native_camera.basis_yaw_degrees.is_finite()
        {
            return Err("nonfinite camera basis angles".into());
        }
        let scalars = [
            self.camera_zoom,
            self.camera_zoom_start,
            self.camera_zoom_duration,
            self.camera_zoom_ease_in,
            self.camera_zoom_ease_out,
            self.camera_pitch_radians,
            self.camera_pitch_start,
            self.camera_pitch_duration,
            self.camera_pitch_ease_in,
            self.camera_pitch_ease_out,
            self.camera_fx_pitch,
            self.camera_yaw_radians,
            self.camera_yaw_start,
            self.camera_yaw_duration,
            self.camera_yaw_ease_in,
            self.camera_yaw_ease_out,
            self.camera_follow_factor,
        ];
        if scalars.iter().any(|value| !value.is_finite())
            || [
                self.camera_zoom_target,
                self.camera_pitch_target,
                self.camera_yaw_target,
            ]
            .into_iter()
            .flatten()
            .any(|value| !value.is_finite())
            || !self.camera_position.is_finite()
            || !self.camera_shake_rotation.is_finite()
            || self
                .native_filter_scroll
                .is_some_and(|value| !value.is_finite())
            || self
                .native_filter_previous_target
                .is_some_and(|value| !value.is_finite())
            || self.camera_zoom <= 0.0
            || self
                .scripted_camera_constraint_widen
                .is_some_and(|(lo_x, hi_x, lo_y, hi_y)| {
                    ![lo_x, hi_x, lo_y, hi_y].into_iter().all(f32::is_finite)
                        || lo_x > hi_x
                        || lo_y > hi_y
                })
        {
            return Err("invalid session camera pose or axis continuation".into());
        }
        inverse_camera_transform(self.view_matrix)?;
        // Projection is intentionally projective, never an affine view matrix.
        let projection = self.projection_matrix;
        let determinant = projection.determinant();
        if !projection.is_finite() || !determinant.is_finite() || determinant == 0.0 {
            return Err("invalid completed session camera matrix".into());
        }
        if projection
            .try_inverse()
            .is_none_or(|inverse| !inverse.is_finite())
        {
            return Err("nonfinite inverse session camera matrix".into());
        }
        if self.script_camera_shakers.len() > 4096
            || self.script_camera_shakers.iter().any(|value| {
                !value.epicenter.is_finite()
                    || !value.radius.is_finite()
                    || !value.duration_seconds.is_finite()
                    || !value.elapsed_seconds.is_finite()
                    || !value.intensity.is_finite()
                    || !value.omega.is_finite()
                    || !value.phi.is_finite()
            })
            || self.camera_slave_mode.as_ref().is_some_and(|value| {
                value.thing_template_name.len() > 4096 || value.bone_name.len() > 4096
            })
        {
            return Err("invalid session camera effects or slave continuation".into());
        }
        #[cfg(not(feature = "game_client"))]
        if self.native_filter_scroll.is_some() || self.native_filter_previous_target.is_some() {
            return Err("saved native camera filter requires the GameClient feature".into());
        }
        Ok(())
    }

    /// The caller decoded/validated the whole candidate before any live mutation.
    pub(crate) fn admit(self, camera: &mut SessionCamera) {
        camera.camera_position = self.camera_position;
        camera.scripted_camera_constraint_widen = self.scripted_camera_constraint_widen;
        camera.camera_zoom = self.camera_zoom;
        camera.camera_zoom_target = self.camera_zoom_target;
        camera.camera_zoom_start = self.camera_zoom_start;
        camera.camera_zoom_duration = self.camera_zoom_duration;
        camera.camera_zoom_frame = self.camera_zoom_frame;
        camera.camera_zoom_ease_in = self.camera_zoom_ease_in;
        camera.camera_zoom_ease_out = self.camera_zoom_ease_out;
        camera.native_camera.admit_owned_continuation(
            self.native_camera.legacy,
            self.native_camera.basis_pitch_degrees,
            self.native_camera.basis_yaw_degrees,
        );
        #[cfg(feature = "game_client")]
        {
            camera.native_filter_scroll = self.native_filter_scroll.unwrap_or_default();
        }
        #[cfg(feature = "game_client")]
        {
            camera.native_filter_previous_target =
                self.native_filter_previous_target.unwrap_or_default();
        }
        camera.camera_pitch_radians = self.camera_pitch_radians;
        camera.camera_pitch_target = self.camera_pitch_target;
        camera.camera_pitch_start = self.camera_pitch_start;
        camera.camera_pitch_duration = self.camera_pitch_duration;
        camera.camera_pitch_frame = self.camera_pitch_frame;
        camera.camera_pitch_ease_in = self.camera_pitch_ease_in;
        camera.camera_pitch_ease_out = self.camera_pitch_ease_out;
        camera.camera_fx_pitch = self.camera_fx_pitch;
        camera.camera_yaw_radians = self.camera_yaw_radians;
        camera.rotation_registers =
            crate::presentation_frame::camera::registers::RotationRegisters::import_v7_scalar(
                self.camera_yaw_start,
                self.camera_yaw_target,
            );
        camera.rotation = crate::presentation_frame::camera::rotation::RotationTiming::historical(
            self.camera_yaw_duration,
            self.camera_yaw_ease_in,
            self.camera_yaw_ease_out,
            camera.rotation.time,
        );
        camera.camera_yaw_frame = self.camera_yaw_frame;
        camera.camera_shake_rotation = self.camera_shake_rotation;
        camera.script_camera_shakers = self.script_camera_shakers;
        camera.camera_slave_mode = self.camera_slave_mode;
        camera.view_matrix = self.view_matrix;
        camera.projection_matrix = self.projection_matrix;
        camera.camera_follow_factor = self.camera_follow_factor;
    }

    /// A native V2 record must describe this exact completed owner matrix;
    /// eye/forward/up alone cannot establish handedness, shear, or full scale.
    pub(crate) fn validate_completed_view(&self, matrix: Mat4) -> Result<(), String> {
        self.validate()?;
        if self.view_matrix.to_cols_array().map(f32::to_bits)
            != matrix.to_cols_array().map(f32::to_bits)
        {
            return Err("logic/native completed camera matrix disagrees".into());
        }
        Ok(())
    }

    /// Native85 stores these observations instead of the raw completed matrix.
    pub(crate) fn completed_pose(&self) -> Result<(Vec3, Vec3, Vec3), String> {
        self.validate()?;
        let inverse = inverse_camera_transform(self.view_matrix)?;
        Ok((
            inverse.transform_point3(Vec3::ZERO),
            inverse.transform_vector3(Vec3::NEG_Z),
            inverse.transform_vector3(Vec3::Y),
        ))
    }

    /// Compare Native85's detached projection before admission. Its wire omits
    /// basis angles and rebuilds matrices for the receiving viewport, so those
    /// are deliberately checked by the caller's completed-pose contract.
    pub(crate) fn validate_native_projection(&self, camera: &SessionCamera) -> Result<(), String> {
        self.validate()?;
        let mut projected = Self::capture(camera);
        projected.validate()?;
        projected.native_camera.basis_pitch_degrees = self.native_camera.basis_pitch_degrees;
        projected.native_camera.basis_yaw_degrees = self.native_camera.basis_yaw_degrees;
        projected.view_matrix = self.view_matrix;
        projected.projection_matrix = self.projection_matrix;
        let codec = CameraCodec::<{ 4 * 1024 * 1024 }>;
        if codec.serialize(self)? != codec.serialize(&projected)? {
            return Err("logic/native session camera continuation disagrees".into());
        }
        Ok(())
    }

    /// Historical Logic slots are projections, never independent live clocks.
    /// The ordering is rotate, zoom, pitch, matching that positional record.
    pub(crate) fn remaining_axis_seconds(
        &self,
        tracking: Option<&crate::presentation_frame::scripted::CameraObjectTracking>,
        timing: Option<super::rotation::RotationTiming>,
    ) -> [f32; 3] {
        fn remaining(target: Option<f32>, duration: f32, frame: u32) -> f32 {
            if target.is_none() {
                return 0.0;
            }
            let count = (((duration * 1000.0) as i32).max(1) / 33).max(1) as u32;
            count.saturating_sub(frame) as f32 * 0.033
        }
        let count = timing.map_or_else(
            || (((self.camera_yaw_duration * 1000.) as i32).max(1) / 33).max(1) as u32,
            |timing| timing.turn_frames,
        );
        let rotate = if self.camera_yaw_target.is_some() {
            count
                .saturating_add(tracking.map_or(0, |tracking| tracking.hold_frames))
                .saturating_sub(self.camera_yaw_frame) as f32
                * 0.033
        } else {
            0.
        };
        [
            rotate,
            remaining(
                self.camera_zoom_target,
                self.camera_zoom_duration,
                self.camera_zoom_frame,
            ),
            remaining(
                self.camera_pitch_target,
                self.camera_pitch_duration,
                self.camera_pitch_frame,
            ),
        ]
    }
}
