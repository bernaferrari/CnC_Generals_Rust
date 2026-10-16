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

impl CnCGameEngine {
    #[cfg(feature = "game_client")]
    pub(super) fn native_filter_render_scroll(&self) -> glam::Vec2 {
        projected_filter_scroll(
            self.view_matrix,
            self.projection_matrix,
            self.native_filter_previous_target,
            self.game_logic.session_view.target,
            self.native_camera.basis().ground_level(),
        )
    }

    #[cfg(feature = "game_client")]
    pub(super) fn reset_native_filter_selection(&mut self) {
        reset_filter_selection(&mut self.game_logic.session_view.filter);
        self.native_filter_scroll = glam::Vec2::ZERO;
        self.native_filter_previous_target = self.game_logic.session_view.target;
    }

    /// W3D shader postRender calls View::lookAt immediately at the zoom peak.
    /// Keep the existing map basis/HAG goal; this changes the logical position.
    #[cfg(feature = "game_client")]
    pub(super) fn apply_native_filter_look_at(&mut self, point: Vec3) {
        self.look_at_modes
            .note_scripted_camera_player_cancel(ScriptedCameraPlayerCancel::LookAt);
        self.camera_yaw_target = None;
        self.camera_yaw_duration = 0.0;
        self.game_logic.cancel_scripted_camera_from_player_look_at();
        self.host_center_camera_on(point);
    }
}

#[cfg(all(test, feature = "game_client"))]
mod tests {
    use super::*;

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
