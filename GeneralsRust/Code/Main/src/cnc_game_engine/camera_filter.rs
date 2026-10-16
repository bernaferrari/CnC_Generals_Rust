//! Receiving native viewport filter requests and immediate post-render LookAt.
use super::mouse::ScriptedCameraPlayerCancel;
use super::*;
use crate::game_logic::mission_scripts::CameraMotionBlurRequest;
#[cfg(feature = "game_client")]
use game_client::display::{
    view::{FilterMode, FilterType, Point3},
    view_filter::ViewFilterState,
};

/// W3DView::calcDeltaScroll projects both logical positions at fixed map ground.
/// A rejected point yields zero; input scroll axes are not this screen delta.
fn projected_filter_scroll(
    view: Mat4,
    projection: Mat4,
    previous: Vec3,
    current: Vec3,
    ground: f32,
) -> glam::Vec2 {
    let project = |position: Vec3| {
        let camera = view * Vec3::new(position.x, ground, position.z).extend(1.);
        if camera.z > -DEFAULT_VIEW_NEAR_CLIP {
            return None;
        }
        let clip = projection * camera;
        let projected = clip.truncate() / clip.w;
        if !projected.is_finite()
            || projected.z > 1.
            || projected.x.abs() > 1.
            || projected.y.abs() > 1.
        {
            None
        } else {
            Some(projected.truncate())
        }
    };
    match (project(previous), project(current)) {
        (Some(previous), Some(current)) => current - previous,
        _ => glam::Vec2::ZERO,
    }
}

#[cfg(feature = "game_client")]
fn reset_filter_selection(filter: &mut ViewFilterState) {
    // Source View::reset selects its no-effect default. Shader metadata is not
    // a new owner and is not cleared by assigning the View's filter selector.
    filter.set_filter(FilterType::Null);
}

/// The camera fields borrowed through one current-frame filter completion.
/// HAG and the map basis are observed, never advanced by this draw continuation.
#[cfg(feature = "game_client")]
pub(super) struct NativeFilterCameraContext<'a> {
    pub look_at_modes: &'a mut super::mouse::LookAtHostModes,
    pub yaw_target: &'a mut Option<f32>,
    pub yaw_duration: &'a mut f32,
    pub target: &'a mut Vec3,
    pub impulse: Vec3,
    pub native_camera: &'a super::camera_height::CameraHeight,
    pub zoom: f32,
    pub pitch: f32,
    pub yaw: f32,
    pub fx_pitch: f32,
    pub shake_rotation: Vec3,
    pub slave_mode: bool,
    pub position: &'a mut Vec3,
    pub view: &'a mut Mat4,
    pub projection: Mat4,
    pub world_bounds: (Vec3, Vec3),
    pub constraint_inset: f32,
    pub constraint_widen: &'a mut Option<(f32, f32, f32, f32)>,
    pub env: Option<&'a crate::presentation_frame::PresentationWorldEnv>,
}
#[cfg(feature = "game_client")]
impl NativeFilterCameraContext<'_> {
    pub(super) fn finish(
        &mut self,
        point: Option<Vec3>,
        cancel_scripted_camera: &mut dyn FnMut(),
    ) -> crate::graphics::render_pipeline::FilterCameraMatrices {
        if let Some(point) = point {
            self.look_at_modes
                .note_scripted_camera_player_cancel(ScriptedCameraPlayerCancel::LookAt);
            *self.yaw_target = None;
            *self.yaw_duration = 0.;
            cancel_scripted_camera();
            *self.target = super::start_game::center_camera_target(
                point,
                *self.target,
                *self.position,
                self.world_bounds.0,
                self.world_bounds.1,
                self.constraint_inset,
                self.constraint_widen,
                true,
                self.env,
            );
            if !self.slave_mode {
                let (source, look) = super::start_game::camera_orbit_pose(
                    self.native_camera,
                    *self.target,
                    self.impulse,
                    self.zoom,
                    self.pitch,
                    self.yaw,
                    self.fx_pitch,
                );
                *self.position = source;
                *self.view =
                    super::start_game::camera_orbit_view(source, look, self.shake_rotation);
            }
        }
        crate::graphics::render_pipeline::FilterCameraMatrices {
            view: *self.view,
            projection: self.projection,
            position: *self.position,
        }
    }
}

impl CnCGameEngine {
    #[cfg(feature = "game_client")]
    pub(super) fn native_filter_render_scroll(&self) -> glam::Vec2 {
        projected_filter_scroll(
            self.game_logic.session_view.camera.view_matrix,
            self.game_logic.session_view.camera.projection_matrix,
            self.game_logic
                .session_view
                .camera
                .native_filter_previous_target,
            self.game_logic.session_view.target,
            self.game_logic
                .session_view
                .camera
                .native_camera
                .basis()
                .ground_level(),
        )
    }

    #[cfg(feature = "game_client")]
    pub(super) fn reset_native_filter_selection(&mut self) {
        reset_filter_selection(&mut self.game_logic.session_view.filter);
        self.game_logic.session_view.camera.native_filter_scroll = glam::Vec2::ZERO;
        self.game_logic
            .session_view
            .camera
            .native_filter_previous_target = self.game_logic.session_view.target;
    }
}

#[cfg(all(test, feature = "game_client"))]
mod tests {
    use super::*;

    #[test]
    fn borrowed_post_render_look_at_keeps_authored_axes_and_native_motion_metadata() {
        crate::game_logic::game_logic::pose_owner_tests::isolated_at(
            module_path!(),
            "borrowed_post_render_look_at_keeps_authored_axes_and_native_motion_metadata",
            || {
                use super::super::camera_motion::{
                    AxisKind, AxisMotion, MotionAdmission, step_axis,
                };
                use gamelogic::scripting::core::{
                    Parameter, ParameterType, Script, ScriptAction, ScriptActionType, ScriptList,
                };
                use gamelogic::scripting::engine::ScriptEngine;
                let mut world = crate::game_logic::GameLogic::new();
                let peer = crate::game_logic::GameLogic::new();
                let peer_before = peer.capture_camera_continuation().unwrap();
                let mut next = Some(Box::new(ScriptAction::new(
                    ScriptActionType::CameraModFreezeTime,
                )));
                for (kind, values) in [
                    (ScriptActionType::PitchCamera, [0.7, 4., 0.3, 0.5]),
                    (ScriptActionType::ZoomCamera, [0.8, 3., 0.3, 0.5]),
                ] {
                    let mut action = ScriptAction::new(kind);
                    for value in values {
                        action
                            .add_parameter(Parameter::with_real(ParameterType::Real, value))
                            .unwrap();
                    }
                    action.next_action = next;
                    next = Some(Box::new(action));
                }
                let mut script = Script::new();
                script.script_name = "BorrowedAxisSetup".into();
                script.is_subroutine = true;
                script.action = next;
                let mut list = ScriptList::new();
                list.append_script(Box::new(script));
                let interpreter = ScriptEngine::new().unwrap();
                interpreter
                    .set_script_list_for_player(0, Some(Box::new(list)))
                    .unwrap();
                world.install_owned_script_engine(interpreter);
                world
                    .with_owned_script_engine(|engine, world| {
                        world.execute_owned_ai_condition(engine, "BorrowedAxisSetup", None);
                    })
                    .unwrap();
                assert!(!world.camera_movement_finished());
                assert!(world.is_script_camera_time_frozen());
                let zoom_request = world.peek_pending_camera_zoom().unwrap().clone();
                let pitch_request = world.peek_pending_camera_pitch().unwrap().clone();
                assert_eq!(
                    (
                        zoom_request.duration_seconds,
                        pitch_request.duration_seconds
                    ),
                    (3., 4.)
                );

                // These are the exact existing AxisMotion fields the Engine
                // admits from those requests, not a second camera controller.
                let mut zoom = 1.;
                let mut zoom_target = Some(zoom_request.zoom);
                let mut zoom_frame = 7;
                let zoom_meta = (
                    zoom_target,
                    zoom_frame,
                    zoom_request.duration_seconds,
                    zoom_request.ease_in_seconds,
                    zoom_request.ease_out_seconds,
                );
                let mut pitch = 1.;
                let mut pitch_target = Some(pitch_request.pitch);
                let mut pitch_frame = 9;
                let pitch_meta = (
                    pitch_target,
                    pitch_frame,
                    pitch_request.duration_seconds,
                    pitch_request.ease_in_seconds,
                    pitch_request.ease_out_seconds,
                );
                let height = super::super::camera_height::CameraHeight::new(400., 45., 0.);
                let hag_before = (
                    height.desired(),
                    height.basis().pose(Vec3::ZERO, 1., 0., 0.),
                );
                let mut modes = super::super::mouse::LookAtHostModes::default();
                let mut yaw_target = Some(2.);
                let mut yaw_duration = 5.;
                let mut position = Vec3::new(0., 400., -400.);
                let mut view = Mat4::IDENTITY;
                let mut widen = None;
                let (_, target, impulse, mut cancellation, _, _) =
                    world.split_filter_camera_owner();
                let mut camera = NativeFilterCameraContext {
                    look_at_modes: &mut modes,
                    yaw_target: &mut yaw_target,
                    yaw_duration: &mut yaw_duration,
                    target,
                    impulse,
                    native_camera: &height,
                    zoom,
                    pitch: 0.,
                    yaw: 0.,
                    fx_pitch: pitch,
                    shake_rotation: Vec3::ZERO,
                    slave_mode: false,
                    position: &mut position,
                    view: &mut view,
                    projection: Mat4::IDENTITY,
                    world_bounds: (Vec3::ZERO, Vec3::new(100., 0., 100.)),
                    constraint_inset: 0.,
                    constraint_widen: &mut widen,
                    env: None,
                };
                let completed =
                    camera.finish(Some(Vec3::new(60., 0., 40.)), &mut || cancellation.cancel());
                assert_eq!(*camera.target, Vec3::new(60., 0., 40.));
                assert_ne!(completed.view, Mat4::IDENTITY);
                drop(camera);
                drop(cancellation);
                assert!(
                    !world.camera_movement_finished(),
                    "borrowed cancellation must retain canonical independent clocks"
                );
                assert!(world.is_script_camera_time_frozen());
                assert_eq!(
                    world.peek_pending_camera_zoom().unwrap().duration_seconds,
                    3.
                );
                assert_eq!(
                    world.peek_pending_camera_pitch().unwrap().duration_seconds,
                    4.
                );
                assert_eq!(
                    (
                        zoom_target,
                        zoom_frame,
                        zoom_request.duration_seconds,
                        zoom_request.ease_in_seconds,
                        zoom_request.ease_out_seconds
                    ),
                    zoom_meta
                );
                assert_eq!(
                    (
                        pitch_target,
                        pitch_frame,
                        pitch_request.duration_seconds,
                        pitch_request.ease_in_seconds,
                        pitch_request.ease_out_seconds
                    ),
                    pitch_meta
                );
                assert_eq!(
                    (
                        height.desired(),
                        height.basis().pose(Vec3::ZERO, 1., 0., 0.)
                    ),
                    hag_before
                );
                let admission = MotionAdmission {
                    update_views: true,
                    fast_camera_only: false,
                    paused: false,
                    debug_frozen: false,
                    disable_camera_movement: false,
                };
                for (value, target, frame, request_duration, ease_in, ease_out) in [
                    (
                        &mut zoom,
                        &mut zoom_target,
                        &mut zoom_frame,
                        zoom_request.duration_seconds,
                        zoom_request.ease_in_seconds,
                        zoom_request.ease_out_seconds,
                    ),
                    (
                        &mut pitch,
                        &mut pitch_target,
                        &mut pitch_frame,
                        pitch_request.duration_seconds,
                        pitch_request.ease_in_seconds,
                        pitch_request.ease_out_seconds,
                    ),
                ] {
                    let before = *frame;
                    let step = step_axis(
                        AxisMotion {
                            value,
                            target,
                            frame,
                            start: 1.,
                            duration_seconds: request_duration,
                            ease_in_seconds: ease_in,
                            ease_out_seconds: ease_out,
                        },
                        AxisKind::ZoomPitch,
                        admission,
                    );
                    assert!(step.was_active && step.changed && !step.completed);
                    assert_eq!(*frame, before + 1, "next admitted phase advances once");
                }
                assert_eq!(yaw_target, None);
                assert_eq!(yaw_duration, 0.);
                assert_eq!(peer.capture_camera_continuation().unwrap(), peer_before);
            },
        );
    }

    #[test]
    fn borrowed_filter_camera_completes_before_redraw_without_advancing_hag_or_peer() {
        let mut world = crate::game_logic::GameLogic::new();
        let peer = crate::game_logic::GameLogic::new();
        world.install_script_camera_path_for_test();
        world.queue_pending_camera_rotate(1., 3.);
        let peer_target = peer.session_view.target;
        let height = super::super::camera_height::CameraHeight::new(400., 45., 0.);
        let basis_before = height.basis().pose(Vec3::ZERO, 1., 0., 0.);
        let desired = height.desired();
        let mut modes = super::super::mouse::LookAtHostModes::default();
        let mut yaw_target = Some(2.);
        let mut yaw_duration = 3.;
        let mut position = Vec3::new(0., 400., -400.);
        let mut view = Mat4::IDENTITY;
        let mut widen = None;
        let (_, target, impulse, mut cancellation, _, _) = world.split_filter_camera_owner();
        let mut camera = NativeFilterCameraContext {
            look_at_modes: &mut modes,
            yaw_target: &mut yaw_target,
            yaw_duration: &mut yaw_duration,
            target,
            impulse,
            native_camera: &height,
            zoom: 1.,
            pitch: 0.,
            yaw: 0.,
            fx_pitch: 1.,
            shake_rotation: Vec3::ZERO,
            slave_mode: false,
            position: &mut position,
            view: &mut view,
            projection: Mat4::IDENTITY,
            world_bounds: (Vec3::ZERO, Vec3::new(100., 0., 100.)),
            constraint_inset: 0.,
            constraint_widen: &mut widen,
            env: None,
        };
        let unchanged = camera.finish(None, &mut || panic!("no peak cannot cancel"));
        assert_eq!(unchanged.view, Mat4::IDENTITY);
        assert_eq!(unchanged.position, Vec3::new(0., 400., -400.));
        let completed = camera.finish(Some(Vec3::new(200., 0., 40.)), &mut || {
            cancellation.cancel()
        });
        assert_eq!(*camera.target, Vec3::new(100., 0., 40.));
        let (eye, look) = height.basis().pose(*camera.target, 1., 0., 0.);
        assert_eq!(completed.position, eye);
        assert_eq!(completed.view, Mat4::look_at_rh(eye, look, Vec3::Y));
        assert_eq!(height.basis().pose(Vec3::ZERO, 1., 0., 0.), basis_before);
        assert_eq!(height.desired(), desired);
        drop(camera);
        drop(cancellation);
        assert!(!world.script_camera_path_active());
        assert!(world.peek_pending_camera_rotate().is_none());
        assert_eq!(yaw_target, None);
        assert_eq!(yaw_duration, 0.);
        assert_eq!(peer.session_view.target, peer_target);
    }

    #[test]
    fn receiving_filter_commands_keep_action_order_and_peer() {
        let mut owner = crate::presentation_frame::SessionView::default();
        let peer = crate::presentation_frame::SessionView::default();
        owner.apply_motion_blur(&CameraMotionBlurRequest::Follow { amount: 17 });
        assert_eq!(
            owner
                .filter
                .post_render(1, game_client::display::view::Vector2::new(1., 0.))
                .motion_blur_count,
            17
        );
        owner.apply_bw(true, 12);
        assert_eq!(owner.filter.filter(), FilterType::BlackAndWhite);
        owner.apply_motion_blur(&CameraMotionBlurRequest::Jump {
            position: Vec3::new(11., 22., 33.),
            saturate: true,
        });
        assert_eq!(owner.filter.mode(), FilterMode::MBInAndOutSaturate);
        let mut peak = None;
        for frame in 2..=13 {
            peak = owner
                .filter
                .post_render(frame, game_client::display::view::Vector2::zero())
                .look_at
                .or(peak);
        }
        let peak = peak.expect("twelve admitted render frames reach source peak");
        assert_eq!((peak.x, peak.y, peak.z), (11., 33., 22.));
        assert_eq!(peer.filter.filter(), FilterType::Null);
    }

    #[test]
    fn default_filter_selection_preserves_owner_phase_and_hag_peer() {
        let mut owner = crate::presentation_frame::SessionView::default();
        let mut peer = crate::presentation_frame::SessionView::default();
        owner.apply_motion_blur(&CameraMotionBlurRequest::Follow { amount: 17 });
        peer.apply_bw(true, 8);
        let target = peer.target;
        let height = super::super::camera_height::CameraHeight::new(300., 37., 0.);
        let desired = height.desired();
        reset_filter_selection(&mut owner.filter);
        assert_eq!(owner.filter.filter(), FilterType::Null);
        assert_eq!(peer.filter.filter(), FilterType::BlackAndWhite);
        assert_eq!(peer.target, target);
        assert_eq!(height.desired(), desired);
    }

    #[test]
    fn blur_scroll_projects_logical_motion_not_manual_input_and_rejects_outside_points() {
        let view = Mat4::look_at_rh(Vec3::new(0., 10., 10.), Vec3::ZERO, Vec3::Y);
        let projection = perspective_rh_from_horizontal_fov(DEFAULT_VIEW_FOV_RADIANS, 1., 1., 100.);
        // Source helper uses actual camera near=10; both points are further.
        let delta =
            projected_filter_scroll(view, projection, Vec3::ZERO, Vec3::new(1., 99., 0.), 0.);
        assert!(delta.x > 0. && delta.y.abs() < 1e-6);
        let equal = projected_filter_scroll(view, projection, Vec3::ZERO, Vec3::ZERO, 0.);
        assert_eq!(equal, glam::Vec2::ZERO);
        let rejected =
            projected_filter_scroll(view, projection, Vec3::new(1000., 0., 0.), Vec3::ZERO, 0.);
        assert_eq!(rejected, glam::Vec2::ZERO);
    }
}
