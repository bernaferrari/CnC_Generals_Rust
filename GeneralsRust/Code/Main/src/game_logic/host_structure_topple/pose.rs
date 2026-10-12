//! WWMath matrix3d.h1198–1285: in-place pre-rotation changes the three
//! world-axis basis rows and leaves translation unchanged. Host Y/Z are
//! swapped from CPP, so preserve the mapped row operations, not local yaw.
use glam::Mat4;

#[derive(Clone, Copy)]
enum CppAxis {
    X,
    Y,
    Z,
}

fn in_place_pre_rotate(matrix: &mut Mat4, axis: CppAxis, angle: f32) {
    let c = angle.cos();
    let s = angle.sin();
    // CPP has three basis columns. Its row X/Y/Z map to Host X/Z/Y.
    // Copy only the basis: never rotate the separate translation column or
    // normalize/decompose a saved scale, shear or singular matrix.
    let mut columns = [matrix.x_axis, matrix.y_axis, matrix.z_axis];
    for column in &mut columns {
        match axis {
            CppAxis::X => {
                let (cpp_y, cpp_z) = (column.z, column.y);
                column.z = c * cpp_y - s * cpp_z;
                column.y = s * cpp_y + c * cpp_z;
            }
            CppAxis::Y => {
                let (cpp_x, cpp_z) = (column.x, column.y);
                column.x = c * cpp_x + s * cpp_z;
                column.y = -s * cpp_x + c * cpp_z;
            }
            CppAxis::Z => {
                let (cpp_x, cpp_y) = (column.x, column.z);
                column.x = c * cpp_x - s * cpp_y;
                column.z = s * cpp_x + c * cpp_y;
            }
        }
    }
    matrix.x_axis = columns[0];
    matrix.y_axis = columns[1];
    matrix.z_axis = columns[2];
}

/// CPP StructureToppleUpdate.cpp263–265: X first, then Y, world axes.
pub(crate) fn rotate_toppling_basis(
    mut matrix: Mat4,
    velocity: f32,
    direction_x: f32,
    direction_y: f32,
) -> Mat4 {
    in_place_pre_rotate(&mut matrix, CppAxis::X, -velocity * direction_y);
    in_place_pre_rotate(&mut matrix, CppAxis::Y, velocity * direction_x);
    matrix
}

/// CPP doToppleDoneStuff311: this follows the existing setOrientation
/// operation. Its broader upright/cache convention is tracked separately.
pub(crate) fn rotate_done_basis(mut matrix: Mat4, angle: f32) -> Mat4 {
    in_place_pre_rotate(&mut matrix, CppAxis::Z, angle);
    matrix
}

#[cfg(test)]
#[path = "pose_tests.rs"]
mod tests;
