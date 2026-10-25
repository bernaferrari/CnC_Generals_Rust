//! One immutable camera for snow clipping, emission and billboard geometry.
//! C++ W3DSnow uses the submitted render camera, including terrain/HAG and FX.
use crate::display::shadow_pass::clipped_visible_box_z_up;
use crate::snow::{SnowVisibleBoxXy, camera_facing_quad_corners};
use glam::{Mat4, Vec3};

#[derive(Clone, Copy)]
pub(super) struct SnowCamera {
    pub(super) position_y_up: [f32; 3],
    pub(super) visible_xy: SnowVisibleBoxXy,
    right: Vec3,
    up: Vec3,
    source_z_up: bool,
}

impl SnowCamera {
    pub(super) fn native_y_up(view: Mat4, projection: Mat4, min_height: f32) -> Option<Self> {
        Self::capture(view, projection, min_height, false)
    }

    /// Existing standalone clients submit either renderer convention. Preserve
    /// their axis inference here; native Main always selects Y-up explicitly.
    pub(super) fn from_client_view(view: Mat4, projection: Mat4, min_height: f32) -> Option<Self> {
        let y_as_up = view.transform_vector3(Vec3::Y).y.abs();
        let z_as_up = view.transform_vector3(Vec3::Z).y.abs();
        Self::capture(view, projection, min_height, z_as_up > y_as_up)
    }

    fn capture(view: Mat4, projection: Mat4, min_height: f32, source_z_up: bool) -> Option<Self> {
        let axis = Mat4::from_cols(
            Vec3::X.extend(0.),
            Vec3::Z.extend(0.),
            Vec3::Y.extend(0.),
            glam::Vec4::W,
        );
        let z_up_view = if source_z_up { view } else { view * axis };
        let visible = clipped_visible_box_z_up(z_up_view, projection, min_height)?;
        let inverse = view.inverse();
        if !inverse.is_finite() {
            return None;
        }
        let camera = inverse.transform_point3(Vec3::ZERO);
        let position_y_up = if source_z_up {
            axis.transform_point3(camera)
        } else {
            camera
        };
        Some(Self {
            position_y_up: position_y_up.to_array(),
            visible_xy: SnowVisibleBoxXy {
                center_x: visible.center[0],
                center_y: visible.center[1],
                extent_x: visible.extent[0],
                extent_y: visible.extent[1],
            },
            right: inverse.transform_vector3(Vec3::X),
            up: inverse.transform_vector3(Vec3::Y),
            source_z_up,
        })
    }

    pub(super) fn quad(&self, center_y_up: [f32; 3], size: f32) -> [([f32; 3], [f32; 2]); 4] {
        let [x, y, z] = center_y_up;
        let center = if self.source_z_up {
            [x, z, y]
        } else {
            center_y_up
        };
        camera_facing_quad_corners(center, self.right.to_array(), self.up.to_array(), size)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn native_snow_keeps_y_up_even_at_steep_fx_pitch_and_projects_billboards() {
        let eye = Vec3::new(70., 500., 90.);
        let center = Vec3::new(72., 50., 100.);
        let view = Mat4::look_at_rh(eye, center, Vec3::Y);
        let projection = Mat4::perspective_rh(0.9, 1.6, 1., 2000.);
        let captured = SnowCamera::native_y_up(view, projection, 10.).unwrap();
        assert!(!captured.source_z_up);
        assert!(Vec3::from(captured.position_y_up).abs_diff_eq(eye, 0.001));
        let camera_center = view.transform_point3(center);
        let offsets = [(-1., 1.), (-1., -1.), (1., -1.), (1., 1.)];
        for ((point, uv), (x, y)) in captured
            .quad(center.to_array(), 2.)
            .into_iter()
            .zip(offsets)
        {
            let actual = view.transform_point3(Vec3::from(point));
            assert!(actual.abs_diff_eq(camera_center + Vec3::new(x, y, 0.), 0.001));
            assert!(uv.iter().all(|v| *v == 0. || *v == 1.));
        }
    }

    #[test]
    fn equivalent_native_and_client_source_cameras_have_the_same_snow_box() {
        let eye = Vec3::new(70., 500., -90.);
        let target = Vec3::new(90., 10., 90.);
        let view = Mat4::look_at_rh(eye, target, Vec3::Y);
        let projection = Mat4::perspective_rh(0.9, 1.6, 1., 2000.);
        let axis = Mat4::from_cols(
            Vec3::X.extend(0.),
            Vec3::Z.extend(0.),
            Vec3::Y.extend(0.),
            glam::Vec4::W,
        );
        let native = SnowCamera::native_y_up(view, projection, 10.).unwrap();
        let source = SnowCamera::capture(view * axis, projection, 10., true).unwrap();
        assert!(
            Vec3::from(native.position_y_up).abs_diff_eq(Vec3::from(source.position_y_up), 0.001)
        );
        assert_eq!(native.visible_xy.center_x, source.visible_xy.center_x);
        assert_eq!(native.visible_xy.center_y, source.visible_xy.center_y);
        assert_eq!(native.visible_xy.extent_x, source.visible_xy.extent_x);
        assert_eq!(native.visible_xy.extent_y, source.visible_xy.extent_y);
        for ((n, _), (s, _)) in native
            .quad(target.to_array(), 2.)
            .into_iter()
            .zip(source.quad(target.to_array(), 2.))
        {
            assert!(Vec3::from(n).abs_diff_eq(axis.transform_point3(Vec3::from(s)), 0.001));
        }
    }

    #[test]
    fn receiving_snow_box_is_independent_of_foreign_core_camera_changes() {
        if let crate::test_process::TestProcess::ParentVerified = crate::test_process::run_bounded(
            module_path!(),
            "receiving_snow_box_is_independent_of_foreign_core_camera_changes",
        ) {
            return;
        }
        let view = Mat4::look_at_rh(
            Vec3::new(70., 500., -90.),
            Vec3::new(90., 10., 90.),
            Vec3::Y,
        );
        let projection = Mat4::perspective_rh(0.9, 1.6, 1., 2000.);
        let expected = SnowCamera::native_y_up(view, projection, 10.).unwrap();
        for x in [-8000., 15000.] {
            crate::display::view::with_tactical_view(|foreign| {
                foreign.sync_pose_from_host(
                    &crate::display::view::Point3::new(x, x * 0.5, 170.),
                    0.7,
                    0.6,
                    1.,
                );
            });
            let actual = SnowCamera::native_y_up(view, projection, 10.).unwrap();
            assert_eq!(actual.visible_xy.center_x, expected.visible_xy.center_x);
            assert_eq!(actual.visible_xy.center_y, expected.visible_xy.center_y);
            assert_eq!(actual.visible_xy.extent_x, expected.visible_xy.extent_x);
            assert_eq!(actual.visible_xy.extent_y, expected.visible_xy.extent_y);
        }
    }

    #[test]
    fn snow_camera_rejects_invalid_frustum_without_a_foreign_view_fallback() {
        assert!(SnowCamera::native_y_up(Mat4::ZERO, Mat4::IDENTITY, 0.).is_none());
        assert!(SnowCamera::native_y_up(Mat4::IDENTITY, Mat4::IDENTITY, f32::NAN).is_none());
    }
}
