//! Explicit final-camera plane-zero projection for native W3D Draw admission.
use game_client::core::game_client::NativeDrawView;
use glam::Mat4;

pub(super) fn native_view_from_camera(
    view: Mat4,
    projection: Mat4,
    width: f32,
    height: f32,
    guardband: [f32; 2],
    vertical_extent: [f32; 2],
) -> Result<NativeDrawView, String> {
    if !width.is_finite() || !height.is_finite() || width <= 0. || height <= 0. {
        return Err("native view requires a finite positive viewport".into());
    }
    if vertical_extent[0] > vertical_extent[1] {
        return Err("native view terrain extent is inverted".into());
    }
    let mut corners = [[0.; 3]; 4];
    for (index, pixel) in [(0., 0.), (width, 0.), (width, height), (0., height)]
        .into_iter()
        .enumerate()
    {
        let (near, far) =
            super::super::mouse::unproject_mouse_ray(view, projection, pixel, width, height)
                .ok_or("native view camera cannot unproject a corner")?;
        let direction = far - near;
        if direction.y.abs() <= 1.0e-6 {
            return Err("native view corner ray is parallel to plane zero".into());
        }
        // C++ Find_X/Y_At_Z intersects the infinite ray line, not a clamped
        // near/far segment. Main Y-up is converted once into original CPP Z-up.
        let point = near + direction * (-near.y / direction.y);
        if !point.is_finite() {
            return Err("native view corner is nonfinite".into());
        }
        corners[index] = [point.x, point.z, 0.];
    }
    NativeDrawView::from_ground_corners(corners, guardband, vertical_extent)
        .map_err(|error| error.to_string())
}

#[cfg(test)]
#[path = "native_display_view_tests.rs"]
mod tests;
