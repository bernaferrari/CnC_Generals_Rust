#![allow(
    unused_imports,
    unused_variables,
    dead_code,
    non_snake_case,
    unused_mut,
    unused_assignments,
    clippy::all
)]
use super::*;

pub(super) const RAY_EPSILON: f32 = 1e-5;
pub(super) const CLIP_EPSILON: f32 = 1e-4;

pub(super) fn normalize_or(vec: Vec3, fallback: Vec3) -> Vec3 {
    if vec.length_squared() > RAY_EPSILON {
        vec.normalize()
    } else {
        fallback
    }
}

#[derive(Clone, Debug)]
pub(super) struct ClipVertex {
    pub(super) obj_pos: Vec3,
    pub(super) world_pos: Vec3,
    pub(super) normal: Vec3,
    pub(super) local: Vec3,
}

pub(super) fn lerp_clip_vertex(a: &ClipVertex, b: &ClipVertex, t: f32) -> ClipVertex {
    let obj_pos = a.obj_pos.lerp(b.obj_pos, t);
    let world_pos = a.world_pos.lerp(b.world_pos, t);
    let blended_normal = a.normal.lerp(b.normal, t);
    let fallback_normal = if a.normal.length_squared() > RAY_EPSILON {
        a.normal
    } else if b.normal.length_squared() > RAY_EPSILON {
        b.normal
    } else {
        Vec3::Z
    };
    let normal = normalize_or(blended_normal, normalize_or(fallback_normal, Vec3::Z));
    let local = a.local.lerp(b.local, t);

    ClipVertex {
        obj_pos,
        world_pos,
        normal,
        local,
    }
}

pub(super) fn clip_polygon_against_plane(
    vertices: &[ClipVertex],
    axis: usize,
    limit: f32,
    keep_less: bool,
) -> Vec<ClipVertex> {
    if vertices.is_empty() {
        return Vec::new();
    }

    let inside = |value: f32| -> bool {
        if keep_less {
            value <= limit + CLIP_EPSILON
        } else {
            value >= limit - CLIP_EPSILON
        }
    };

    let mut output = Vec::new();
    let mut prev = vertices.last().unwrap();
    let mut prev_value = prev.local[axis];
    let mut prev_inside = inside(prev_value);

    for curr in vertices {
        let curr_value = curr.local[axis];
        let curr_inside = inside(curr_value);

        if curr_inside != prev_inside {
            let denom = curr_value - prev_value;
            if denom.abs() > CLIP_EPSILON {
                let t = (limit - prev_value) / denom;
                let t = t.clamp(0.0, 1.0);
                output.push(lerp_clip_vertex(prev, curr, t));
            }
        }

        if curr_inside {
            output.push(curr.clone());
        }

        prev = curr;
        prev_value = curr_value;
        prev_inside = curr_inside;
    }

    output
}

pub(super) fn clip_polygon_to_projector(
    polygon: Vec<ClipVertex>,
    extents: Vec3,
) -> Vec<ClipVertex> {
    if polygon.len() < 3 {
        return Vec::new();
    }

    let mut output = clip_polygon_against_plane(&polygon, 0, extents.x, true);
    if output.len() < 3 {
        return Vec::new();
    }

    output = clip_polygon_against_plane(&output, 0, -extents.x, false);
    if output.len() < 3 {
        return Vec::new();
    }

    output = clip_polygon_against_plane(&output, 1, extents.y, true);
    if output.len() < 3 {
        return Vec::new();
    }

    output = clip_polygon_against_plane(&output, 1, -extents.y, false);
    if output.len() < 3 {
        return Vec::new();
    }

    output
}

pub(super) fn w3d_to_vec3(source: &W3dVectorStruct) -> Vec3 {
    Vec3::new(source.x, source.y, source.z)
}

pub(super) fn w3d_to_mu_vec3(source: &W3dVectorStruct) -> MuVec3 {
    MuVec3::new(source.x, source.y, source.z)
}

pub(super) fn triangle_vertices(
    triangle: &W3dTriangleStruct,
    vertices: &[W3dVectorStruct],
) -> Option<[Vec3; 3]> {
    let idx0 = triangle.vindex[0] as usize;
    let idx1 = triangle.vindex[1] as usize;
    let idx2 = triangle.vindex[2] as usize;
    if idx0 >= vertices.len() || idx1 >= vertices.len() || idx2 >= vertices.len() {
        return None;
    }
    Some([
        w3d_to_vec3(&vertices[idx0]),
        w3d_to_vec3(&vertices[idx1]),
        w3d_to_vec3(&vertices[idx2]),
    ])
}

pub(super) fn mu_triangle_from_w3d(
    triangle: &W3dTriangleStruct,
    vertices: &[W3dVectorStruct],
) -> Option<MuTriangle> {
    let verts = triangle_vertices(triangle, vertices)?;
    let stored_normal = Vec3::new(triangle.normal.x, triangle.normal.y, triangle.normal.z);
    let mu_normal = if stored_normal.length_squared() > RAY_EPSILON {
        w3d_to_mu_vec3(&triangle.normal).normalize()
    } else {
        let computed = (verts[1] - verts[0]).cross(verts[2] - verts[0]);
        if computed.length_squared() > RAY_EPSILON {
            MuVec3::new(computed.x, computed.y, computed.z).normalize()
        } else {
            MuVec3::new(0.0, 0.0, 1.0)
        }
    };
    Some(MuTriangle::with_normal(
        MuVec3::new(verts[0].x, verts[0].y, verts[0].z),
        MuVec3::new(verts[1].x, verts[1].y, verts[1].z),
        MuVec3::new(verts[2].x, verts[2].y, verts[2].z),
        mu_normal,
    ))
}

pub(super) fn mu_aabox_from_class(aabox: &AABoxClass) -> MuAABox {
    MuAABox::new(
        MuVec3::new(aabox.center.x, aabox.center.y, aabox.center.z),
        MuVec3::new(
            aabox.extent.x.abs(),
            aabox.extent.y.abs(),
            aabox.extent.z.abs(),
        ),
    )
}

pub(super) fn mu_obbox_from_class(obbox: &OBBoxClass) -> MuOBBox {
    let b0 = obbox.basis[0];
    let b1 = obbox.basis[1];
    let b2 = obbox.basis[2];
    let basis = MuMatrix3::from_rows(
        MuVec3::new(b0.x, b1.x, b2.x),
        MuVec3::new(b0.y, b1.y, b2.y),
        MuVec3::new(b0.z, b1.z, b2.z),
    );
    MuOBBox::from_center_extent_basis(
        MuVec3::new(obbox.center.x, obbox.center.y, obbox.center.z),
        MuVec3::new(
            obbox.extent.x.abs(),
            obbox.extent.y.abs(),
            obbox.extent.z.abs(),
        ),
        basis,
    )
}

pub(super) fn ray_triangle_intersection(
    origin: Vec3,
    direction: Vec3,
    max_distance: f32,
    v0: Vec3,
    v1: Vec3,
    v2: Vec3,
) -> Option<(f32, Vec3)> {
    let edge1 = v1 - v0;
    let edge2 = v2 - v0;
    let pvec = direction.cross(edge2);
    let det = edge1.dot(pvec);
    if det.abs() < RAY_EPSILON {
        return None;
    }

    let inv_det = 1.0 / det;
    let tvec = origin - v0;
    let u = tvec.dot(pvec) * inv_det;
    if !(0.0..=1.0).contains(&u) {
        return None;
    }

    let qvec = tvec.cross(edge1);
    let v = direction.dot(qvec) * inv_det;
    if v < 0.0 || u + v > 1.0 {
        return None;
    }

    let t = edge2.dot(qvec) * inv_det;
    if t < RAY_EPSILON || t > max_distance + RAY_EPSILON {
        return None;
    }

    let normal = edge1.cross(edge2);
    if normal.length_squared() <= RAY_EPSILON {
        return None;
    }
    Some((t, normal.normalize()))
}
pub(super) use math_utilities::{
    AABox as MuAABox, CastResult as MuCastResult, CollisionMath as MuCollisionMath,
    Matrix3 as MuMatrix3, OBBox as MuOBBox, Triangle as MuTriangle, Vector3 as MuVec3,
};

/// Compute index ranges for each material pass
/// This maps each pass index to its start index and triangle count
/// Returns a vector where index i contains (start_index, count) for pass i
pub(super) fn compute_pass_index_ranges(
    model: &MeshModelClass,
    index_data: &[u32],
) -> Vec<(u32, u32)> {
    // If we have polygon renderers organized by pass, preserve per-pass ranges.
    if !model.polygon_renderer_list.is_empty() {
        let mut ranges: Vec<(u32, u32)> = vec![(0, 0); model.material_passes.len()];
        let mut current_index = 0;

        for (renderer_index, renderer) in model.polygon_renderer_list.iter().enumerate() {
            let pass_index = renderer
                .material_pass
                .as_ref()
                .map(|pass| pass.get_pass_index())
                .unwrap_or(renderer_index);

            if pass_index >= ranges.len() {
                ranges.resize(pass_index + 1, (0, 0));
            }

            if renderer.index_count > 0 {
                let (range_start, range_count) = &mut ranges[pass_index];
                if *range_count == 0 {
                    *range_start = current_index;
                }
                *range_count = range_count.saturating_add(renderer.index_count);
                current_index = current_index.saturating_add(renderer.index_count);
            }
        }

        if ranges.iter().any(|(_, count)| *count > 0) {
            return ranges;
        }
    }

    // Fallback: create a single range covering all available geometry.
    if !index_data.is_empty() {
        vec![(0, index_data.len() as u32)]
    } else if model.index_count > 0 {
        vec![(0, model.index_count)]
    } else if model.vertex_count > 0 {
        vec![(0, model.vertex_count)]
    } else {
        vec![]
    }
}
