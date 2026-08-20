//! C++ `DX8MeshRenderer` ALIGNED / ORIENTED world-matrix rebuild.
//!
//! `dx8renderer.cpp:1787-1809`:
//! - ALIGNED: `Obj_Look_At(mesh_position, mesh_position + camera_z, 0)`
//! - ORIENTED: `Obj_Look_At(mesh_position, camera_position, 0)`

use glam::{Mat4, Vec3, Vec4};

/// `Matrix3D::Obj_Look_At` — origin at `start`, X toward `end`, twist 0.
pub fn obj_look_at(start: Vec3, end: Vec3) -> Mat4 {
    let direction = (end - start).normalize_or_zero();
    if direction.length_squared() < 1e-6 {
        return Mat4::from_translation(start);
    }
    let up = if direction.y.abs() < 0.999 {
        Vec3::Y
    } else {
        Vec3::Z
    };
    let x_axis = direction;
    let z_axis = x_axis.cross(up).normalize_or_zero();
    let y_axis = z_axis.cross(x_axis);
    Mat4::from_cols(
        Vec4::new(x_axis.x, x_axis.y, x_axis.z, 0.0),
        Vec4::new(y_axis.x, y_axis.y, y_axis.z, 0.0),
        Vec4::new(z_axis.x, z_axis.y, z_axis.z, 0.0),
        Vec4::new(start.x, start.y, start.z, 1.0),
    )
}

/// Camera local +Z, matching `Matrix3D::Get_Z_Vector`.
pub fn camera_z_vector(camera_transform: Mat4) -> Vec3 {
    camera_transform.z_axis.truncate()
}

pub fn camera_aligned_world(mesh_position: Vec3, camera_z: Vec3) -> Mat4 {
    obj_look_at(mesh_position, mesh_position + camera_z)
}

pub fn camera_oriented_world(mesh_position: Vec3, camera_position: Vec3) -> Mat4 {
    obj_look_at(mesh_position, camera_position)
}

/// Rebuild the draw-time world matrix for an ALIGNED / ORIENTED mesh.
pub fn billboard_world_transform(
    authored: Mat4,
    aligned: bool,
    oriented: bool,
    camera_transform: Mat4,
) -> Mat4 {
    let mesh_position = authored.w_axis.truncate();
    if aligned {
        camera_aligned_world(mesh_position, camera_z_vector(camera_transform))
    } else if oriented {
        camera_oriented_world(mesh_position, camera_transform.w_axis.truncate())
    } else {
        authored
    }
}
