//! Shared completed-pose arithmetic; no clocks or state mutation.
use glam::{Mat4, Vec3};

// Matrix3D has an implicit (0,0,0,1) homogeneous row. General Mat4 inverse
// computes its last entry as cofactor*(1/det), which can round away from 1.
// Preserve those native bits within the actual glam affine-helper precondition;
// any nonzero XYZ perspective term is rejected, rather than hidden by tolerance.
fn affine_view_row(matrix: Mat4) -> bool {
    matrix.x_axis.w == 0.0
        && matrix.y_axis.w == 0.0
        && matrix.z_axis.w == 0.0
        && (matrix.w_axis.w - 1.0).abs() <= 1.0e-6
}

/// Check and invert the affine camera transform before admission or persistence.
/// Projection has a separate validation path; reflected/scaled affine views are valid.
pub(crate) fn inverse_camera_transform(matrix: Mat4) -> Result<Mat4, String> {
    let determinant = matrix.determinant();
    if !matrix.is_finite() || !determinant.is_finite() || determinant == 0.0 {
        return Err("invalid completed camera view matrix".into());
    }
    if !affine_view_row(matrix) {
        return Err("non-affine completed camera view matrix".into());
    }
    let inverse = matrix
        .try_inverse()
        .ok_or_else(|| "singular completed camera view matrix".to_owned())?;
    if !inverse.is_finite() {
        return Err("nonfinite inverse camera view matrix".into());
    }
    if !affine_view_row(inverse) {
        return Err("non-affine inverse camera view matrix".into());
    }
    Ok(inverse)
}

/// Current map basis and existing shake/FX; no clock advances during redraw.
pub(crate) fn camera_orbit_pose(
    native_camera: &super::height::CameraHeight,
    target: Vec3,
    impulse: Vec3,
    zoom: f32,
    pitch: f32,
    yaw: f32,
    fx_pitch: f32,
    constraints: Option<super::constraints::ConstraintBounds>,
) -> (Vec3, Vec3) {
    // Source buildCameraTransform adds horizontal shake to a local position,
    // then constrains it. Both eye and look inherit that translation.
    let local = target + Vec3::new(impulse.x, 0., impulse.z);
    let local = constraints.map_or(local, |bounds| bounds.constrain(local));
    let (eye, look) = native_camera.basis().pose(local, zoom, pitch, yaw);
    camera_fx_pose(eye, look, fx_pitch)
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
