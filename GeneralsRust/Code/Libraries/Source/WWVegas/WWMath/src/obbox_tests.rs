#[cfg(test)]
mod tests {
    use crate::{OBBox, Vec3, Mat3, Quat};

    #[test]
    fn obbox_from_points_axis_aligned() {
        let center = Vec3::new(1.0, 2.0, 3.0);
        let extent = Vec3::new(2.0, 1.0, 3.0);
        let corners = [
            center + Vec3::new(-extent.x, -extent.y, -extent.z),
            center + Vec3::new( extent.x, -extent.y, -extent.z),
            center + Vec3::new(-extent.x,  extent.y, -extent.z),
            center + Vec3::new( extent.x,  extent.y, -extent.z),
            center + Vec3::new(-extent.x, -extent.y,  extent.z),
            center + Vec3::new( extent.x, -extent.y,  extent.z),
            center + Vec3::new(-extent.x,  extent.y,  extent.z),
            center + Vec3::new( extent.x,  extent.y,  extent.z),
        ];
        let obb = OBBox::from_points(&corners);
        assert!((obb.center - center).length() < 1e-4);
        assert!((obb.extent - extent).length() < 1e-4);
    }

    #[test]
    fn obbox_from_points_rotated() {
        let center = Vec3::ZERO;
        let extent = Vec3::new(2.0, 1.0, 3.0);
        let corners_local = [
            Vec3::new(-extent.x, -extent.y, -extent.z),
            Vec3::new( extent.x, -extent.y, -extent.z),
            Vec3::new(-extent.x,  extent.y, -extent.z),
            Vec3::new( extent.x,  extent.y, -extent.z),
            Vec3::new(-extent.x, -extent.y,  extent.z),
            Vec3::new( extent.x, -extent.y,  extent.z),
            Vec3::new(-extent.x,  extent.y,  extent.z),
            Vec3::new( extent.x,  extent.y,  extent.z),
        ];
        let rot = Quat::from_rotation_z(45f32.to_radians());
        let rotm = Mat3::from_quat(rot);
        let mut corners_world = [Vec3::ZERO; 8];
        for (i, c) in corners_local.iter().enumerate() {
            corners_world[i] = center + rotm * *c;
        }
        let obb = OBBox::from_points(&corners_world);
        assert!(obb.center.length() < 1e-4);
        assert!((obb.extent - extent).length() < 1e-3);
    }
}

