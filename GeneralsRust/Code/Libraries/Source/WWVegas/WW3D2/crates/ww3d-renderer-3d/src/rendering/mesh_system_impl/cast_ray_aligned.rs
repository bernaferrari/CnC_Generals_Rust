//! C++ `MeshClass::Cast_Ray` ALIGNED / ORIENTED look-at (`mesh.cpp:1143-1151`).
//!
//! ALIGNED rotates so the mesh looks along `-ray.dir`. ORIENTED looks at the
//! ray origin (`Ray.Get_P0()`).

use super::collect_billboard_xform::obj_look_at;
use glam::{Mat4, Vec3};

pub fn cast_ray_aligned_world(
    authored: Mat4,
    aligned: bool,
    oriented: bool,
    ray_origin: Vec3,
    ray_dir: Vec3,
) -> Mat4 {
    let mesh_position = authored.w_axis.truncate();
    if aligned {
        obj_look_at(mesh_position, mesh_position - ray_dir)
    } else if oriented {
        obj_look_at(mesh_position, ray_origin)
    } else {
        authored
    }
}
