//! BaseHeightMap::getMaximumVisibleBox and PlaneClass::Compute_Intersection.
//! Matrices map source Z-up world coordinates to WebGPU clip depth [0, 1].
use super::VisibleBox;
use glam::{Mat4, Vec3, Vec4};

pub fn clipped_visible_box_z_up(
    view: Mat4,
    projection: Mat4,
    min_height: f32,
) -> Option<VisibleBox> {
    let clip = projection * view;
    if !clip.is_finite() || clip.determinant() == 0. {
        return None;
    }
    let inverse = clip.inverse();
    if !inverse.is_finite() || !min_height.is_finite() {
        return None;
    }
    let mut corners = [Vec3::ZERO; 8];
    let signs = [(-1., 1.), (1., 1.), (1., -1.), (-1., -1.)];
    for (plane, depth) in [0., 1.].into_iter().enumerate() {
        for (i, (x, y)) in signs.into_iter().enumerate() {
            let point = inverse * Vec4::new(x, y, depth, 1.);
            let point = point.truncate() / point.w;
            if !point.is_finite() {
                return None;
            }
            corners[plane * 4 + i] = point;
        }
    }
    for i in 0..4 {
        let a = corners[i];
        let delta = corners[i + 4] - a;
        // PlaneClass::Compute_Intersection uses an exact parallel check.
        if delta.z != 0. {
            let fraction = (min_height - a.z) / delta.z;
            if (0. ..=1.).contains(&fraction) {
                corners[i + 4] = a + delta * fraction;
            }
        }
    }
    let min = corners.iter().copied().reduce(Vec3::min)?;
    let max = corners.iter().copied().reduce(Vec3::max)?;
    Some(VisibleBox {
        center: ((min + max) * 0.5).to_array(),
        extent: ((max - min) * 0.5).to_array(),
    })
}
