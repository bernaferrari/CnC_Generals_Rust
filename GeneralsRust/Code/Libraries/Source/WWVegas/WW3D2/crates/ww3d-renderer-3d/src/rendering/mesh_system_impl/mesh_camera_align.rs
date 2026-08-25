//! Apply `CAMERA_ALIGNED` / `CAMERA_ORIENTED` geometry flags from a W3D header.
//!
//! C++ `MeshGeometryClass` load (`meshgeometry.cpp:1552-1566`) sets ALIGNED /
//! ORIENTED from `W3D_MESH_FLAG_GEOMETRY_TYPE_*` when the header version is
//! at least 4.1.

use super::{MeshGeometryClass, MeshModelClass};
use ww3d_core::w3d_format::{
    W3D_MESH_FLAG_GEOMETRY_TYPE_CAMERA_ALIGNED, W3D_MESH_FLAG_GEOMETRY_TYPE_CAMERA_ORIENTED,
    W3D_MESH_FLAG_GEOMETRY_TYPE_MASK, W3dMeshHeader3Struct,
};

/// C++ `W3D_MAKE_VERSION(4, 1)`.
const W3D_GEOMETRY_FLAG_VERSION: u32 = (4 << 16) | 1;

pub fn apply_camera_align_flags_from_header(
    model: &mut MeshModelClass,
    header: &W3dMeshHeader3Struct,
) {
    if header.version < W3D_GEOMETRY_FLAG_VERSION {
        return;
    }
    let geometry_type = header.attrs & W3D_MESH_FLAG_GEOMETRY_TYPE_MASK;
    match geometry_type {
        W3D_MESH_FLAG_GEOMETRY_TYPE_CAMERA_ALIGNED => {
            model.set_flag(MeshGeometryClass::ALIGNED, true);
        }
        W3D_MESH_FLAG_GEOMETRY_TYPE_CAMERA_ORIENTED => {
            model.set_flag(MeshGeometryClass::ORIENTED, true);
        }
        _ => {}
    }
}

pub fn mesh_is_camera_aligned(model: &MeshModelClass) -> bool {
    model.get_flag(MeshGeometryClass::ALIGNED)
}

pub fn mesh_is_camera_oriented(model: &MeshModelClass) -> bool {
    model.get_flag(MeshGeometryClass::ORIENTED)
}

pub fn mesh_is_billboard(model: &MeshModelClass) -> bool {
    mesh_is_camera_aligned(model) || mesh_is_camera_oriented(model)
}
