#[cfg(test)]
mod tests {
    use crate::{AABox, LineSegment, Vec3, Mat3D, Affine3D, AffineExtensions};

    #[test]
    fn aabox_affine_vs_mat3d_parity_rotation_translation() {
        let original = AABox::new(Vec3::new(1.0, 2.0, 3.0), Vec3::new(0.5, 1.0, 1.5));
        let mut a_mat = original;
        let mut a_aff = original;

        // Build Mat3D rotation Z 90° + translation
        let mut m = Mat3D::from_axis_angle(Vec3::new(0.0, 0.0, 1.0), std::f32::consts::FRAC_PI_2);
        m.set_translation(Vec3::new(5.0, -2.0, 1.0));
        a_mat.transform(&m);

        // Equivalent Affine3D
        let aff = Affine3D::from_trs(Vec3::new(5.0, -2.0, 1.0), glam::Quat::from_rotation_z(std::f32::consts::FRAC_PI_2), Vec3::new(1.0, 1.0, 1.0));
        a_aff.transform_affine(&aff);

        assert!((a_mat.center - a_aff.center).length() < 1e-4);
        assert!((a_mat.extent - a_aff.extent).length() < 1e-4);
    }

    #[test]
    fn line_segment_affine_vs_mat3d_parity() {
        let src = LineSegment::new(Vec3::new(1.0, 0.0, 0.0), Vec3::new(2.0, 0.0, 0.0));
        let mut ls_m = LineSegment::default();
        let mut ls_a = LineSegment::default();

        let mut m = Mat3D::from_axis_angle(Vec3::new(0.0, 1.0, 0.0), std::f32::consts::FRAC_PI_2);
        m.set_translation(Vec3::new(0.0, 3.0, 0.0));
        ls_m.set_transformed(&src, &m);

        let aff = Affine3D::from_trs(Vec3::new(0.0, 3.0, 0.0), glam::Quat::from_rotation_y(std::f32::consts::FRAC_PI_2), Vec3::new(1.0, 1.0, 1.0));
        ls_a.set_transformed_affine(&src, &aff);

        assert!((ls_m.get_p0() - ls_a.get_p0()).length() < 1e-4);
        assert!((ls_m.get_p1() - ls_a.get_p1()).length() < 1e-4);
    }
}

