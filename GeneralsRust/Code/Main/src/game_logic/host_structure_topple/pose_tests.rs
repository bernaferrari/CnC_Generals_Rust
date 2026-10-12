//! Independent expectations use literal CPP Matrix3D rows from
//! WWMath/matrix3d.h1198/1232/1264, then convert the completed matrix once.
use super::*;
use glam::{Vec3, Vec4};

type CppRows = [[f32; 4]; 3];
fn host_from_cpp(rows: CppRows) -> Mat4 {
    Mat4::from_cols(
        Vec4::new(rows[0][0], rows[2][0], rows[1][0], 0.0),
        Vec4::new(rows[0][2], rows[2][2], rows[1][2], 0.0),
        Vec4::new(rows[0][1], rows[2][1], rows[1][1], 0.0),
        Vec4::new(rows[0][3], rows[2][3], rows[1][3], 1.0),
    )
}
fn cpp_x_then_y(mut rows: CppRows, velocity: f32, x: f32, y: f32) -> CppRows {
    let theta = -velocity * y;
    let (c, s) = (theta.cos(), theta.sin());
    for column in 0..3 {
        let (tmp1, tmp2) = (rows[1][column], rows[2][column]);
        rows[1][column] = c * tmp1 - s * tmp2;
        rows[2][column] = s * tmp1 + c * tmp2;
    }
    let theta = velocity * x;
    let (c, s) = (theta.cos(), theta.sin());
    for column in 0..3 {
        let (tmp1, tmp2) = (rows[0][column], rows[2][column]);
        rows[0][column] = c * tmp1 + s * tmp2;
        rows[2][column] = -s * tmp1 + c * tmp2;
    }
    rows
}
fn identity_at_position() -> CppRows {
    [
        [1.0, 0.0, 0.0, 7.0],
        [0.0, 1.0, 0.0, 11.0],
        [0.0, 0.0, 1.0, 13.0],
    ]
}

#[test]
fn cpp_positive_x_topple_moves_vertical_toward_positive_x_without_orbiting_position() {
    let rows = identity_at_position();
    let velocity = 0.3;
    let actual = rotate_toppling_basis(host_from_cpp(rows), velocity, 1.0, 0.0);
    assert_eq!(
        actual,
        host_from_cpp(cpp_x_then_y(rows, velocity, 1.0, 0.0))
    );
    let vertical = actual.transform_vector3(Vec3::Y);
    assert_eq!(vertical.x.to_bits(), velocity.sin().to_bits());
    assert_eq!(vertical.y.to_bits(), velocity.cos().to_bits());
    assert_eq!(vertical.z, 0.0);
    assert_eq!(actual.w_axis, Vec4::new(7.0, 13.0, 11.0, 1.0));
}

#[test]
fn cpp_positive_y_topple_moves_vertical_toward_positive_host_z() {
    let rows = identity_at_position();
    let velocity = 0.3;
    let actual = rotate_toppling_basis(host_from_cpp(rows), velocity, 0.0, 1.0);
    assert_eq!(
        actual,
        host_from_cpp(cpp_x_then_y(rows, velocity, 0.0, 1.0))
    );
    let vertical = actual.transform_vector3(Vec3::Y);
    assert_eq!(vertical.x, 0.0);
    assert_eq!(vertical.y.to_bits(), velocity.cos().to_bits());
    assert_eq!(vertical.z.to_bits(), velocity.sin().to_bits());
    assert_eq!(actual.w_axis, Vec4::new(7.0, 13.0, 11.0, 1.0));
}

#[test]
fn cpp_topple_world_rows_preserve_nonzero_orientation_scaled_sheared_basis() {
    // Both horizontal facing components are nonzero. This basis includes
    // unequal scale and shear, so local/right multiplication cannot match.
    let rows = [
        [0.6, -2.0, 0.3, 7.0],
        [1.7, 0.9, -0.4, -8.0],
        [0.2, 0.5, 2.9, 11.0],
    ];
    let original = host_from_cpp(rows);
    for (velocity, x, y) in [(0.37, 0.6, 0.8), (0.12, -0.8, 0.6), (-0.2, 0.5, -0.5)] {
        let actual = rotate_toppling_basis(original, velocity, x, y);
        let expected = host_from_cpp(cpp_x_then_y(rows, velocity, x, y));
        assert_eq!(
            actual.to_cols_array().map(f32::to_bits),
            expected.to_cols_array().map(f32::to_bits)
        );
        assert_eq!(actual.w_axis, original.w_axis);
    }
}

#[test]
fn cpp_done_z_pre_rotation_changes_world_rows_and_preserves_translation() {
    let mut rows = [
        [0.6, -2.0, 0.3, 7.0],
        [1.7, 0.9, -0.4, -8.0],
        [0.2, 0.5, 2.9, 11.0],
    ];
    let original = host_from_cpp(rows);
    let theta: f32 = 0.42;
    let (c, s) = (theta.cos(), theta.sin());
    for column in 0..3 {
        let (tmp1, tmp2) = (rows[0][column], rows[1][column]);
        rows[0][column] = c * tmp1 - s * tmp2;
        rows[1][column] = s * tmp1 + c * tmp2;
    }
    let actual = rotate_done_basis(original, theta);
    let expected = host_from_cpp(rows);
    assert_eq!(
        actual.to_cols_array().map(f32::to_bits),
        expected.to_cols_array().map(f32::to_bits)
    );
    assert_eq!(actual.w_axis, original.w_axis);
}

#[test]
fn cpp_row_operations_do_not_normalize_a_singular_saved_basis() {
    let rows = [
        [2.0, 4.0, 0.0, 3.0],
        [0.0, 0.0, 0.0, 5.0],
        [1.0, 2.0, 0.0, 7.0],
    ];
    let original = host_from_cpp(rows);
    let actual = rotate_toppling_basis(original, 0.23, 0.8, -0.6);
    assert_eq!(actual, host_from_cpp(cpp_x_then_y(rows, 0.23, 0.8, -0.6)));
    assert_eq!(actual.w_axis, original.w_axis);
    assert_eq!(actual.determinant(), 0.0);
}
