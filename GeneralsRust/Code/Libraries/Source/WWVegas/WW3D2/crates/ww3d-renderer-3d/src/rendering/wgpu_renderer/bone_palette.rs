//! Dynamic GPU bone palette with no 64-bone uniform cap.
//!
//! C++ `MeshGeometryClass::get_deformed_vertices` (`meshgeometry.cpp:1965`)
//! transforms every vertex by `htree->Get_Transform(bonelink[vi])`. The live
//! GPU path must upload the full HTree palette, not a truncated `[Mat4; 64]`.

use glam::Mat4;

/// Pack every authored bone matrix for a storage-buffer upload.
///
/// An empty palette still emits one identity so the storage array is non-empty
/// and `arrayLength` is defined in WGSL.
pub fn bone_palette_mats(bones: &[Mat4]) -> Vec<Mat4> {
    if bones.is_empty() {
        vec![Mat4::IDENTITY]
    } else {
        bones.to_vec()
    }
}

/// Tight `mat4x4<f32>` bytes for `var<storage, read> bones: array<mat4x4<f32>>`.
pub fn bone_palette_bytes(bones: &[Mat4]) -> Vec<u8> {
    let mats = bone_palette_mats(bones);
    bytemuck::cast_slice(&mats).to_vec()
}
