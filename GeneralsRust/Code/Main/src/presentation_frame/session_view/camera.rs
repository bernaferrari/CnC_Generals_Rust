//! Canonical native camera state belongs to one SessionView, including HAG.
//! Constructors are inert; the Engine admits its configured boot pose explicitly.
use glam::{Mat4, Vec3};
pub(crate) mod build;
pub(crate) mod clip;
mod commands;
pub(crate) mod constraints;
pub(crate) mod continuation;
pub(crate) mod height;
pub(crate) mod input_rules;
pub(crate) mod motion;
pub(crate) mod movement_rules;
pub(crate) mod pose;
pub(crate) mod projection;
pub(crate) mod registers;
pub(crate) mod rotation;
pub(crate) mod settings;
pub(crate) mod shake_rules;
pub(crate) mod shaker;
pub(crate) mod viewport;
pub(crate) use shaker::ScriptCameraShaker;

#[derive(Debug)]
pub(crate) struct SessionCamera {
    pub(crate) settings: settings::CameraSettings,
    pub(crate) movement_policy: movement_rules::CameraMovementPolicy,
    pub(crate) input_policy: input_rules::CameraInputPolicy,
    pub(crate) clip_policy: clip::CameraClipPolicy,
    pub(crate) shake_policy: shake_rules::CameraShakePolicy,
    pub(crate) constraints: constraints::CameraConstraints,
    pub(crate) viewport: viewport::TacticalViewport,
    pub(crate) camera_position: Vec3,
    pub(crate) scripted_camera_constraint_widen: Option<(f32, f32, f32, f32)>,
    pub(crate) camera_zoom: f32,
    pub(crate) camera_zoom_target: Option<f32>,
    pub(crate) camera_zoom_start: f32,
    pub(crate) camera_zoom_duration: f32,
    pub(crate) camera_zoom_frame: u32,
    pub(crate) camera_zoom_ease_in: f32,
    pub(crate) camera_zoom_ease_out: f32,
    pub(crate) native_camera: height::CameraHeight,
    #[cfg(feature = "game_client")]
    pub(crate) native_filter_scroll: glam::Vec2,
    #[cfg(feature = "game_client")]
    pub(crate) native_filter_previous_target: Vec3,
    pub(crate) camera_pitch_radians: f32,
    pub(crate) camera_pitch_target: Option<f32>,
    pub(crate) camera_pitch_start: f32,
    pub(crate) camera_pitch_duration: f32,
    pub(crate) camera_pitch_frame: u32,
    pub(crate) camera_pitch_ease_in: f32,
    pub(crate) camera_pitch_ease_out: f32,
    pub(crate) camera_fx_pitch: f32,
    pub(crate) camera_yaw_radians: f32,
    pub(crate) rotation_registers: registers::RotationRegisters,
    pub(crate) rotation: rotation::RotationTiming,
    pub(crate) camera_yaw_frame: u32,
    pub(crate) camera_shake_rotation: Vec3,
    pub(crate) script_camera_shakers: Vec<ScriptCameraShaker>,
    pub(crate) camera_slave_mode:
        Option<crate::game_logic::mission_scripts::CameraSlaveModeRequest>,
    pub(crate) view_matrix: Mat4,
    pub(crate) projection_matrix: Mat4,
    pub(crate) camera_follow_factor: f32,
    pub(crate) camera_follow_snap_immediate: bool,
}
impl Default for SessionCamera {
    fn default() -> Self {
        let native_camera = height::CameraHeight::new(300., 37.5, 0.);
        let (position, look) = native_camera.basis().pose(Vec3::ZERO, 1., 0., 0.);
        Self {
            settings: Default::default(),
            movement_policy: Default::default(),
            input_policy: Default::default(),
            clip_policy: Default::default(),
            shake_policy: Default::default(),
            constraints: Default::default(),
            viewport: Default::default(),
            camera_position: position,
            scripted_camera_constraint_widen: None,
            camera_zoom: 1.,
            camera_zoom_target: None,
            camera_zoom_start: 1.,
            camera_zoom_duration: 0.,
            camera_zoom_frame: 0,
            camera_zoom_ease_in: 0.,
            camera_zoom_ease_out: 0.,
            native_camera: native_camera,
            #[cfg(feature = "game_client")]
            native_filter_scroll: glam::Vec2::ZERO,
            #[cfg(feature = "game_client")]
            native_filter_previous_target: Vec3::ZERO,
            camera_pitch_radians: 0.,
            camera_pitch_target: None,
            camera_pitch_start: 0.,
            camera_pitch_duration: 0.,
            camera_pitch_frame: 0,
            camera_pitch_ease_in: 0.,
            camera_pitch_ease_out: 0.,
            camera_fx_pitch: 1.,
            camera_yaw_radians: 0.,
            rotation_registers: Default::default(),
            rotation: Default::default(),
            camera_yaw_frame: 0,
            camera_shake_rotation: Vec3::ZERO,
            script_camera_shakers: Vec::new(),
            camera_slave_mode: None,
            view_matrix: Mat4::look_at_rh(position, look, Vec3::Y),
            projection_matrix: Mat4::IDENTITY,
            camera_follow_factor: -1.,
            camera_follow_snap_immediate: false,
        }
    }
}

impl SessionCamera {
    /// Object tracking uses the same yaw frame counter with the source's
    /// current-angle recurrence, rather than the fixed start/end interpolation.
    pub(crate) fn advance_scalar_movements(
        &mut self,
        admission: motion::MotionAdmission,
        logical_target: Vec3,
        skip_yaw: bool,
        speed: &mut f32,
    ) -> bool {
        use motion::{AxisKind, AxisMotion, step_axis};
        let zoom = step_axis(
            AxisMotion {
                value: &mut self.camera_zoom,
                target: &mut self.camera_zoom_target,
                frame: &mut self.camera_zoom_frame,
                start: self.camera_zoom_start,
                duration_seconds: self.camera_zoom_duration,
                ease_in_seconds: self.camera_zoom_ease_in,
                ease_out_seconds: self.camera_zoom_ease_out,
            },
            AxisKind::ZoomPitch,
            admission,
        );
        let pitch = step_axis(
            AxisMotion {
                value: &mut self.camera_fx_pitch,
                target: &mut self.camera_pitch_target,
                frame: &mut self.camera_pitch_frame,
                start: self.camera_pitch_start,
                duration_seconds: self.camera_pitch_duration,
                ease_in_seconds: self.camera_pitch_ease_in,
                ease_out_seconds: self.camera_pitch_ease_out,
            },
            AxisKind::ZoomPitch,
            admission,
        );
        #[cfg(feature = "game_client")]
        if self.rotation_registers.active()
            && (admission.fast_camera_only
                || (admission.update_views && !admission.paused && !admission.debug_frozen))
        {
            self.native_filter_previous_target = logical_target;
        }
        if skip_yaw {
            return zoom.was_active || pitch.was_active || self.rotation_registers.active();
        }
        let yaw = motion::step_registered_rotation(
            &mut self.camera_yaw_radians,
            &mut self.camera_yaw_frame,
            &mut self.rotation_registers,
            self.rotation,
            admission,
        );
        if let Some(factor) = yaw.eased_factor {
            *speed = self.rotation.time.sample(factor);
        }
        zoom.was_active || pitch.was_active || yaw.was_active
    }
}

impl SessionCamera {
    pub(crate) fn cancel_scalar_movements(&mut self) {
        self.camera_zoom_target = None;
        self.camera_pitch_target = None;
        self.rotation_registers.finish();
    }
}
