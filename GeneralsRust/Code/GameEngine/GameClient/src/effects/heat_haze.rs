//! C++ `W3DSmudgeManager::render` screen-space heat-haze quads.

use bytemuck::{Pod, Zeroable};

#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Pod, Zeroable)]
pub struct HeatHazeGpuVertex {
    pub view_pos: [f32; 3],
    pub uv: [f32; 2],
    pub opacity: f32,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct HeatHazeVertex {
    pub view_pos: [f32; 3],
    pub uv: [f32; 2],
    pub opacity: f32,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct HeatHazeSmudge {
    pub world_pos: [f32; 3],
    pub size: f32,
    pub offset: [f32; 2],
    pub opacity: f32,
}

const CORNER_OFFSETS: [[f32; 3]; 4] = [
    [-0.5, 0.5, 0.0],
    [-0.5, -0.5, 0.0],
    [0.5, -0.5, 0.0],
    [0.5, 0.5, 0.0],
];

fn transform_point(m: &[[f32; 4]; 4], p: [f32; 3]) -> [f32; 4] {
    [
        m[0][0] * p[0] + m[1][0] * p[1] + m[2][0] * p[2] + m[3][0],
        m[0][1] * p[0] + m[1][1] * p[1] + m[2][1] * p[2] + m[3][1],
        m[0][2] * p[0] + m[1][2] * p[1] + m[2][2] * p[2] + m[3][2],
        m[0][3] * p[0] + m[1][3] * p[1] + m[2][3] * p[2] + m[3][3],
    ]
}

fn project_uv(view_pos: [f32; 3], proj: &[[f32; 4]; 4], tex_scale: [f32; 2]) -> [f32; 2] {
    let clip = transform_point(proj, view_pos);
    let oow = if clip[3].abs() > 1.0e-8 {
        1.0 / clip[3]
    } else {
        0.0
    };
    let ndc_x = clip[0] * oow;
    let ndc_y = clip[1] * oow;
    [(ndc_x + 1.0) * tex_scale[0], (1.0 - ndc_y) * tex_scale[1]]
}

/// C++ `W3DSmudgeManager::render` 5-vertex view-space quad with center UV warp.
pub fn build_heat_haze_quad(
    smudge: HeatHazeSmudge,
    view: &[[f32; 4]; 4],
    proj: &[[f32; 4]; 4],
    tex_scale: [f32; 2],
    tex_clamp: [f32; 2],
) -> Option<[HeatHazeVertex; 5]> {
    if smudge.size <= 0.0 || smudge.opacity <= 0.0 {
        return None;
    }
    let center = transform_point(view, smudge.world_pos);
    let vs_center = [center[0], center[1], center[2]];
    let mut verts = [HeatHazeVertex {
        view_pos: vs_center,
        uv: [0.0, 0.0],
        opacity: smudge.opacity,
    }; 5];
    let mut offset = smudge.offset;
    for i in 0..4 {
        let pos = [
            vs_center[0] + CORNER_OFFSETS[i][0] * smudge.size,
            vs_center[1] + CORNER_OFFSETS[i][1] * smudge.size,
            vs_center[2] + CORNER_OFFSETS[i][2] * smudge.size,
        ];
        let uv = project_uv(pos, proj, tex_scale);
        if uv[0] > tex_clamp[0] || uv[0] < 0.0 {
            offset[0] = 0.0;
        }
        if uv[1] > tex_clamp[1] || uv[1] < 0.0 {
            offset[1] = 0.0;
        }
        verts[i] = HeatHazeVertex {
            view_pos: pos,
            uv,
            opacity: smudge.opacity,
        };
    }
    let uv_span_x = verts[3].uv[0] - verts[0].uv[0];
    let uv_span_y = verts[1].uv[1] - verts[0].uv[1];
    verts[4] = HeatHazeVertex {
        view_pos: vs_center,
        uv: [
            verts[0].uv[0] + uv_span_x * (0.5 + offset[0]),
            verts[0].uv[1] + uv_span_y * (0.5 + offset[0]),
        ],
        opacity: smudge.opacity,
    };
    Some(verts)
}

pub fn collect_heat_haze_smudges(smudges: &[crate::system::smudge::Smudge]) -> Vec<HeatHazeSmudge> {
    smudges
        .iter()
        .filter(|smudge| smudge.size > 0.0 && smudge.opacity > 0.0)
        .map(|smudge| HeatHazeSmudge {
            world_pos: [smudge.pos.x, smudge.pos.y, smudge.pos.z],
            size: smudge.size,
            offset: [smudge.offset.x, smudge.offset.y],
            opacity: smudge.opacity,
        })
        .collect()
}

pub fn heat_haze_gpu_mesh(
    smudges: &[HeatHazeSmudge],
    view: &[[f32; 4]; 4],
    proj: &[[f32; 4]; 4],
    tex_scale: [f32; 2],
    tex_clamp: [f32; 2],
) -> (Vec<HeatHazeGpuVertex>, Vec<u16>) {
    let mut vertices = Vec::new();
    let mut indices = Vec::new();
    for smudge in smudges {
        let Some(quad) = build_heat_haze_quad(*smudge, view, proj, tex_scale, tex_clamp) else {
            continue;
        };
        let base = vertices.len() as u16;
        for vert in quad {
            vertices.push(HeatHazeGpuVertex {
                view_pos: vert.view_pos,
                uv: vert.uv,
                opacity: vert.opacity,
            });
        }
        indices.extend_from_slice(&heat_haze_triangle_indices(base));
    }
    (vertices, indices)
}

pub fn heat_haze_triangle_indices(base: u16) -> [u16; 12] {
    [
        base,
        base + 1,
        base + 4,
        base + 1,
        base + 2,
        base + 4,
        base + 2,
        base + 3,
        base + 4,
        base + 3,
        base,
        base + 4,
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    fn identity() -> [[f32; 4]; 4] {
        [
            [1.0, 0.0, 0.0, 0.0],
            [0.0, 1.0, 0.0, 0.0],
            [0.0, 0.0, 1.0, 0.0],
            [0.0, 0.0, 0.0, 1.0],
        ]
    }

    #[test]
    fn center_uv_warps_by_offset_x_like_cpp() {
        let smudge = HeatHazeSmudge {
            world_pos: [0.0, 0.0, -10.0],
            size: 2.0,
            offset: [0.2, 0.4],
            opacity: 0.7,
        };
        let verts = build_heat_haze_quad(smudge, &identity(), &identity(), [0.5, 0.5], [1.0, 1.0])
            .expect("quad");
        let span_x = verts[3].uv[0] - verts[0].uv[0];
        let span_y = verts[1].uv[1] - verts[0].uv[1];
        let expected_u = verts[0].uv[0] + span_x * 0.7;
        let expected_v = verts[0].uv[1] + span_y * 0.7;
        assert!((verts[4].uv[0] - expected_u).abs() < 1.0e-5);
        assert!((verts[4].uv[1] - expected_v).abs() < 1.0e-5);
        assert!((verts[4].opacity - 0.7).abs() < f32::EPSILON);
        assert_eq!(heat_haze_triangle_indices(0).len(), 12);
    }

    #[test]
    fn empty_smudge_is_skipped() {
        let smudge = HeatHazeSmudge {
            world_pos: [0.0, 0.0, -1.0],
            size: 0.0,
            offset: [0.0, 0.0],
            opacity: 1.0,
        };
        assert!(
            build_heat_haze_quad(smudge, &identity(), &identity(), [0.5, 0.5], [1.0, 1.0])
                .is_none()
        );
    }
}
