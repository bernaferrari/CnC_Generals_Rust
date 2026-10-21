use super::Matrix3D;

#[test]
fn get_z_rotation_reads_row_one_column_zero_as_atan2_y() {
    assert_eq!(
        Matrix3D::identity().get_z_rotation(),
        0.0,
        "identity heading"
    );
    for (x, y, expected) in [
        (1.0, 0.0, 0.0),
        (0.0, 1.0, std::f32::consts::FRAC_PI_2),
        (-1.0, 0.0, std::f32::consts::PI),
        (0.0, -1.0, -std::f32::consts::FRAC_PI_2),
        (3.0, 4.0, 0.9272952),
        (3.0, -4.0, -0.9272952),
    ] {
        let mut matrix = Matrix3D::identity();
        matrix.m[0][0] = x;
        matrix.m[1][0] = y;
        // Other row values and translation are not angle inputs.
        matrix.m[0][1] = 19.0;
        matrix.m[1][1] = -7.0;
        matrix.set_translation(31.0, -23.0, 11.0);
        assert!((matrix.get_z_rotation() - expected).abs() < 1.0e-6);
        assert_eq!(matrix.get_x_vector().x, x);
        assert_eq!(matrix.get_x_vector().y, y);
    }
}

#[test]
fn base_thing_transform_admission_caches_source_matrix_heading() {
    use crate::common::rts::AsciiString;
    use crate::common::thing::thing::{BaseThing, Thing};
    use crate::common::thing::thing_template::ThingTemplate;
    use std::sync::Arc;

    let mut template = ThingTemplate::new();
    template.set_template_name(AsciiString::from("MatrixHeadingOwner"));
    let mut thing = BaseThing::new(Arc::new(template));
    let mut matrix = Matrix3D::identity();
    matrix.m[0][0] = 3.0;
    matrix.m[1][0] = 4.0;
    matrix.set_translation(31.0, -23.0, 11.0);
    thing.set_transform_matrix(&matrix);
    assert!((thing.get_orientation() - 0.9272952).abs() < 1.0e-6);
    assert_eq!(thing.get_position().x, 31.0);
    assert_eq!(thing.get_position().y, -23.0);
    assert_eq!(thing.get_position().z, 11.0);
    assert_eq!(thing.get_transform_matrix(), &matrix);

    // Reinstalling an identity matrix must cache zero, not pi/2.
    thing.set_transform_matrix(&Matrix3D::identity());
    assert_eq!(thing.get_orientation(), 0.0);
}
