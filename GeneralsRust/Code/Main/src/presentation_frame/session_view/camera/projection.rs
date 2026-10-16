/// C++ `View::m_FOV` is the **horizontal** field of view (View.h:173, View.cpp:53).
/// glam `perspective_rh` takes vertical FOV, so convert at matrix build time.
pub(crate) const DEFAULT_VIEW_FOV_RADIANS: f32 = 50.0_f32.to_radians();
/// C++ W3DView (W3DView.cpp:549-563): `nearZ = MAP_XY_FACTOR` (MapObject.h:35,
/// 10.0 world units per height-map cell — "Improves zbuffer resolution").
pub(crate) const DEFAULT_VIEW_NEAR_CLIP: f32 = 10.0;

/// C++ `CameraClass::Set_View_Plane(hfov, -1)` (WW3D2/camera.cpp:257-261):
/// `height_half = tan(hfov/2) / aspect` ⇒ `vfov = 2*atan(tan(hfov/2)/aspect)`.
#[inline]
pub(crate) fn vertical_fov_from_horizontal(hfov_radians: f32, aspect: f32) -> f32 {
    let aspect = aspect.max(0.01);
    2.0 * ((hfov_radians * 0.5).tan() / aspect).atan()
}

#[inline]
pub(crate) fn perspective_rh_from_horizontal_fov(
    hfov_radians: f32,
    aspect: f32,
    near: f32,
    far: f32,
) -> glam::Mat4 {
    // C++ `CameraClass::Get_D3D_Projection_Matrix` (WW3D2/camera.cpp:707-732)
    // overrides rows 2/3 of `ProjectionTransform` with the D3D depth mapping:
    // NDC z spans [0,1] (near plane → 0, far plane → 1). wgpu expects exactly
    // that window; feeding it a GL-style [-1,1] mapping (glam
    // `perspective_rh`) silently clips everything nearer than the midpoint
    // depth and compresses the rest. x/y columns match glam's RH form so
    // framing is unchanged.
    let vfov = vertical_fov_from_horizontal(hfov_radians, aspect);
    let f = 1.0 / (vfov * 0.5).tan();
    let z22 = far / (near - far);
    let z32 = (near * far) / (near - far);
    glam::Mat4::from_cols(
        glam::Vec4::new(f / aspect, 0.0, 0.0, 0.0),
        glam::Vec4::new(0.0, f, 0.0, 0.0),
        glam::Vec4::new(0.0, 0.0, z22, -1.0),
        glam::Vec4::new(0.0, 0.0, z32, 0.0),
    )
}

/// Original baseline; extension is installed at each actual setCameraTransform.
pub(crate) const DEFAULT_VIEW_FAR_CLIP: f32 = 1200.;
/// Verbatim branch/arithmetic of W3DView549–563. The real-zoom branch is
/// component-tested only: source m_useRealZoomCam defaults false and has no
/// production callers in the supplied original tree. Native production uses false.
pub(crate) fn source_clip_planes(
    draw_entire: bool,
    fx_pitch: f32,
    zoom: f32,
    real_zoom: bool,
) -> Result<(f32, f32), String> {
    if !fx_pitch.is_finite() || !zoom.is_finite() {
        return Err("nonfinite camera clip inputs".into());
    }
    let near = DEFAULT_VIEW_NEAR_CLIP;
    let mut far = DEFAULT_VIEW_FAR_CLIP;
    if real_zoom {
        if fx_pitch < 0.95 {
            far /= fx_pitch;
        }
    } else if draw_entire || fx_pitch < 0.95 || f64::from(zoom) > 1.05 {
        far *= 10.;
    }
    if !far.is_finite() || far <= near {
        return Err("invalid source camera clip planes".into());
    }
    Ok((near, far))
}

#[cfg(test)]
#[path = "projection_clip_tests.rs"]
mod clip_tests;
