//! Shared completed-pose arithmetic; no clocks or state mutation.
use glam::{Mat4, Vec3};
/// Current map basis and existing shake/FX; no clock advances during redraw.
pub(crate) fn camera_orbit_pose(
    native_camera: &super::height::CameraHeight,
    target: Vec3,
    impulse: Vec3,
    zoom: f32,
    pitch: f32,
    yaw: f32,
    fx_pitch: f32,
) -> (Vec3, Vec3) {
    let (eye, look) = native_camera.basis().pose(target, zoom, pitch, yaw);
    camera_fx_pose(eye + impulse, look, fx_pitch)
}

pub(crate) fn camera_fx_pose(source: Vec3, look: Vec3, fx_pitch: f32) -> (Vec3, Vec3) {
    let fx = if fx_pitch.is_finite() { fx_pitch } else { 1. };
    if (fx - 1.).abs() < 1.0e-6 {
        (source, look)
    } else if fx <= 1. {
        let height = (source.y - look.y) * fx;
        (source, Vec3::new(look.x, source.y - height, look.z))
    } else {
        (
            Vec3::new(
                look.x + (source.x - look.x) / fx,
                source.y,
                look.z + (source.z - look.z) / fx,
            ),
            look,
        )
    }
}

pub(crate) fn camera_orbit_view(source: Vec3, look: Vec3, shake: Vec3) -> Mat4 {
    if shake.length_squared() > 1.0e-8 {
        let forward = (look - source).try_normalize().unwrap_or(Vec3::NEG_Z);
        let rot = glam::Quat::from_rotation_y(shake.y)
            * glam::Quat::from_rotation_x(shake.x)
            * glam::Quat::from_rotation_z(shake.z);
        Mat4::look_at_rh(source, source + rot * forward, rot * Vec3::Y)
    } else {
        Mat4::look_at_rh(source, look, Vec3::Y)
    }
}
