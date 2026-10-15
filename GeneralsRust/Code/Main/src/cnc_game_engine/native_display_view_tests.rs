use super::*;
use glam::Vec3;

fn camera(height: f32) -> (Mat4, Mat4) {
    (
        Mat4::look_at_rh(Vec3::new(0., height, 0.), Vec3::ZERO, Vec3::Z),
        Mat4::orthographic_rh(-10., 10., -20., 20., 1., 1000.),
    )
}
#[test]
fn final_owned_camera_projects_plane_zero_with_exact_cpp_overscan_and_map_heights() {
    let (view, projection) = camera(100.);
    let result =
        native_view_from_camera(view, projection, 800., 400., [5., 7.], [-3., 8.]).unwrap();
    for (actual, expected) in result.region.lo.into_iter().zip([-90., -162., -1_000_002.]) {
        assert!((actual - expected).abs() < 0.001, "{actual} != {expected}");
    }
    for (actual, expected) in result.region.hi.into_iter().zip([90., 102., 1_000_007.]) {
        assert!((actual - expected).abs() < 0.001, "{actual} != {expected}");
    }
    // A different camera height retains the same plane-zero orthographic box;
    // the native path has no terrain-average-plane input to substitute for it.
    let (other, projection) = camera(500.);
    let other =
        native_view_from_camera(other, projection, 800., 400., [5., 7.], [-3., 8.]).unwrap();
    for (a, b) in other
        .region
        .lo
        .into_iter()
        .chain(other.region.hi)
        .zip(result.region.lo.into_iter().chain(result.region.hi))
    {
        assert!((a - b).abs() < 0.001);
    }
}
#[test]
fn missing_or_parallel_driving_view_facts_error_without_unbounded_fallback() {
    let (view, projection) = camera(100.);
    assert!(native_view_from_camera(view, projection, 0., 400., [0.; 2], [0., 1.]).is_err());
    assert!(native_view_from_camera(view, projection, 800., f32::NAN, [0.; 2], [0., 1.]).is_err());
    assert!(
        native_view_from_camera(Mat4::ZERO, projection, 800., 400., [0.; 2], [0., 1.]).is_err()
    );
    assert!(
        native_view_from_camera(Mat4::IDENTITY, projection, 800., 400., [0.; 2], [0., 1.]).is_err()
    );
    assert!(
        native_view_from_camera(view, projection, 800., 400., [f32::NAN, 0.], [0., 1.]).is_err()
    );
}
