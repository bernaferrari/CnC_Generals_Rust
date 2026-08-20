//! Single-bone HTree skin deform matching C++ `get_deformed_vertices`.
//!
//! C++ (`meshgeometry.cpp:1965-2007`) walks every vertex and applies
//! `htree->Get_Transform(bonelink[vi])` with no bone-count cap. A missing
//! palette slot is identity, matching an unbound pivot.

use glam::{Mat4, Vec3};
use ww3d_core::w3d_format::W3dVectorStruct;

/// Deform bind-pose positions by one HTree pivot per vertex.
pub fn deform_vertices_single_bone(
    vertices: &[W3dVectorStruct],
    bone_links: &[u16],
    palette: &[Mat4],
) -> Vec<Vec3> {
    vertices
        .iter()
        .enumerate()
        .map(|(index, vertex)| {
            let position = Vec3::new(vertex.x, vertex.y, vertex.z);
            let bone_idx = bone_links.get(index).copied().unwrap_or(0) as usize;
            let matrix = palette.get(bone_idx).copied().unwrap_or(Mat4::IDENTITY);
            matrix.transform_point3(position)
        })
        .collect()
}

/// Four-weight palette skin used when the mesh carries a full influence table.
pub fn deform_vertices_weighted(
    vertices: &[W3dVectorStruct],
    influences: impl Fn(usize) -> ([u32; 4], [f32; 4]),
    palette: &[Mat4],
    bone_links: Option<&[u16]>,
) -> Vec<Vec3> {
    vertices
        .iter()
        .enumerate()
        .map(|(index, vertex)| {
            let position = Vec3::new(vertex.x, vertex.y, vertex.z);
            let (indices, weights) = influences(index);
            let mut skinned = Vec3::ZERO;
            let mut accumulated = 0.0;
            for slot in 0..4 {
                let weight = weights[slot];
                if weight <= f32::EPSILON {
                    continue;
                }
                let matrix = palette
                    .get(indices[slot] as usize)
                    .copied()
                    .unwrap_or(Mat4::IDENTITY);
                skinned += matrix.transform_point3(position) * weight;
                accumulated += weight;
            }
            if accumulated <= f32::EPSILON {
                let fallback = bone_links
                    .and_then(|links| links.get(index))
                    .copied()
                    .map(|idx| idx as usize)
                    .unwrap_or(0);
                let matrix = palette.get(fallback).copied().unwrap_or(Mat4::IDENTITY);
                skinned = matrix.transform_point3(position);
            }
            skinned
        })
        .collect()
}
