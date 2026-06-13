//! CPU-side parity port for C++ `W3DDevice/GameClient/W3DCustomEdging.cpp`.

use game_engine::map_object::{MAP_HEIGHT_SCALE, MAP_XY_FACTOR};

/// Maximum custom blend quads accepted by C++ `W3DCustomEdging`.
pub const MAX_BLENDS: usize = 2000;
/// C++ `MAX_EDGE_VERTEX`.
pub const MAX_EDGE_VERTEX: usize = 4 * MAX_BLENDS;
/// C++ `MAX_EDGE_INDEX`.
pub const MAX_EDGE_INDEX: usize = 6 * MAX_BLENDS;

/// 2D UV region.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Region2D {
    /// Lower-left UV.
    pub lo: Coord2D,
    /// Upper-right UV.
    pub hi: Coord2D,
}

impl Region2D {
    /// Construct a UV region.
    pub fn new(lo_x: f32, lo_y: f32, hi_x: f32, hi_y: f32) -> Self {
        Self {
            lo: Coord2D { x: lo_x, y: lo_y },
            hi: Coord2D { x: hi_x, y: hi_y },
        }
    }

    /// Region width.
    pub fn width(self) -> f32 {
        self.hi.x - self.lo.x
    }

    /// Region height.
    pub fn height(self) -> f32 {
        self.hi.y - self.lo.y
    }
}

/// 2D coordinate.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Coord2D {
    /// X component.
    pub x: f32,
    /// Y component.
    pub y: f32,
}

/// Blend tile classification used by custom edging.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct CustomBlendTile {
    /// Custom edge class, or negative for alpha blends that do not draw custom edging.
    pub custom_blend_edge_class: i32,
    /// Horizontal edge flag.
    pub horiz: bool,
    /// Vertical edge flag.
    pub vert: bool,
    /// Right diagonal flag.
    pub right_diagonal: bool,
    /// Left diagonal flag.
    pub left_diagonal: bool,
    /// Inverted edge flag.
    pub inverted: bool,
    /// Long diagonal flag.
    pub long_diagonal: bool,
}

/// Alpha UV data returned by the height map for the base terrain layer.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct AlphaUvData {
    /// U coordinates for the four terrain corners.
    pub u: [f32; 4],
    /// V coordinates for the four terrain corners.
    pub v: [f32; 4],
    /// Per-corner alpha values.
    pub alpha: [u8; 4],
    /// Whether the base alpha blend uses flipped triangles.
    pub flip_for_blend: bool,
}

impl Default for AlphaUvData {
    fn default() -> Self {
        Self {
            u: [0.0, 1.0, 1.0, 0.0],
            v: [0.0, 0.0, 1.0, 1.0],
            alpha: [255; 4],
            flip_for_blend: false,
        }
    }
}

/// Height-map access needed by W3D custom edging.
pub trait CustomEdgingHeightMap {
    /// Map width in height samples.
    fn x_extent(&self) -> i32;
    /// Map height in height samples.
    fn y_extent(&self) -> i32;
    /// Raw map height sample.
    fn height(&self, x: i32, y: i32) -> i32;
    /// Static diffuse RGB for the terrain corner.
    fn static_diffuse(&self, x: i32, y: i32) -> u32;
    /// Blend tile index at the map cell.
    fn blend_tile_index(&self, x: i32, y: i32) -> i32;
    /// Blend tile metadata by index.
    fn blend_tile(&self, index: i32) -> Option<CustomBlendTile>;
    /// UV region for a custom blend edge class.
    fn uv_for_blend(&self, custom_blend_edge_class: i32) -> Region2D;
    /// Draw origin X used by `getAlphaUVData`.
    fn draw_origin_x(&self) -> i32 {
        0
    }
    /// Draw origin Y used by `getAlphaUVData`.
    fn draw_origin_y(&self) -> i32 {
        0
    }
    /// Base alpha UV data for a cell.
    fn alpha_uv_data(&self, _x: i32, _y: i32) -> AlphaUvData {
        AlphaUvData::default()
    }
}

/// Vertex emitted by `W3DCustomEdging`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct CustomEdgingVertex {
    /// World X.
    pub x: f32,
    /// World Y.
    pub y: f32,
    /// World Z.
    pub z: f32,
    /// Diffuse color with C++ custom-edge alpha.
    pub diffuse: u32,
    /// Base terrain U.
    pub u1: f32,
    /// Base terrain V.
    pub v1: f32,
    /// Edge texture U.
    pub u2: f32,
    /// Edge texture V.
    pub v2: f32,
    /// Original alpha corner value.
    pub alpha: u8,
}

/// Texture handles consumed by the custom-edging render passes.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct CustomEdgingTextures<'a> {
    /// C++ `terrainTexture`.
    pub terrain: Option<&'a str>,
    /// C++ `pMap->getEdgeTerrainTexture()`.
    pub edge: Option<&'a str>,
    /// Optional cloud overlay texture.
    pub cloud: Option<&'a str>,
    /// Optional noise overlay texture.
    pub noise: Option<&'a str>,
}

/// C++ custom-edging shader selection.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CustomEdgingShader {
    /// C++ `detailAlphaTestShader`.
    DetailAlphaTest,
    /// C++ `detailOpaqueShader`.
    DetailOpaque,
}

/// C++ alpha compare operation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CustomEdgingAlphaFunc {
    /// C++ `D3DCMP_LESSEQUAL`.
    LessEqual,
    /// C++ `D3DCMP_GREATEREQUAL`.
    GreaterEqual,
    /// C++ `D3DCMP_NOTEQUAL`.
    NotEqual,
}

/// C++ blend factor.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CustomEdgingBlend {
    /// C++ `D3DBLEND_DESTCOLOR`.
    DestColor,
    /// C++ `D3DBLEND_ZERO`.
    Zero,
}

/// Logical render pass emitted by `W3DCustomEdging::drawEdging`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CustomEdgingRenderPassKind {
    /// First pass: terrain texture clipped by edge alpha.
    TerrainMask,
    /// Second pass: custom edge texture clipped by inverse alpha.
    CustomEdge,
    /// Optional cloud multiplicative pass.
    Cloud,
    /// Optional noise multiplicative pass.
    Noise,
}

/// One C++ `DX8Wrapper::Draw_Triangles` equivalent.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CustomEdgingRenderPass<'a> {
    /// Pass identity.
    pub kind: CustomEdgingRenderPassKind,
    /// Shader selected before drawing.
    pub shader: CustomEdgingShader,
    /// Texture stage 0 binding.
    pub texture0: Option<&'a str>,
    /// Texture stage 1 binding.
    pub texture1: Option<&'a str>,
    /// C++ `D3DRS_ALPHAREF`.
    pub alpha_ref: u8,
    /// C++ `D3DRS_ALPHAFUNC`.
    pub alpha_func: CustomEdgingAlphaFunc,
    /// C++ `D3DRS_ALPHATESTENABLE`.
    pub alpha_test: bool,
    /// C++ `D3DRS_ALPHABLENDENABLE` when explicitly set by the pass.
    pub alpha_blend: Option<bool>,
    /// C++ `D3DRS_SRCBLEND` when explicitly set by the pass.
    pub src_blend: Option<CustomEdgingBlend>,
    /// C++ `D3DRS_DESTBLEND` when explicitly set by the pass.
    pub dest_blend: Option<CustomEdgingBlend>,
    /// First index offset.
    pub start_index: usize,
    /// Triangle count.
    pub triangle_count: usize,
    /// First vertex.
    pub min_vertex: usize,
    /// Vertex count.
    pub vertex_count: usize,
}

/// Render plan returned after rebuilding custom-edging geometry.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct CustomEdgingDrawPlan<'a> {
    /// Draw calls in C++ emission order.
    pub passes: Vec<CustomEdgingRenderPass<'a>>,
}

impl CustomEdgingDrawPlan<'_> {
    /// Whether there is anything to render.
    pub fn is_empty(&self) -> bool {
        self.passes.is_empty()
    }
}

/// Draw buffer for all custom terrain blend edges.
#[derive(Debug, Default)]
pub struct W3DCustomEdging {
    vertices: Vec<CustomEdgingVertex>,
    indices: Vec<u16>,
    anything_changed: bool,
    initialized: bool,
}

impl W3DCustomEdging {
    /// Construct and initialize the edging buffer.
    pub fn new() -> Self {
        let mut edging = Self {
            vertices: Vec::with_capacity(MAX_EDGE_VERTEX),
            indices: Vec::with_capacity(MAX_EDGE_INDEX),
            anything_changed: true,
            initialized: false,
        };
        edging.allocate_edging_buffers();
        edging.initialized = true;
        edging
    }

    /// Allocate CPU backing buffers for custom-edge geometry.
    pub fn allocate_edging_buffers(&mut self) {
        self.vertices.reserve(MAX_EDGE_VERTEX);
        self.indices.reserve(MAX_EDGE_INDEX);
    }

    /// Free all custom-edge geometry.
    pub fn free_edging_buffers(&mut self) {
        self.vertices.clear();
        self.indices.clear();
        self.anything_changed = true;
    }

    /// C++ `clearAllEdging`.
    pub fn clear_all_edging(&mut self) {
        self.vertices.clear();
        self.indices.clear();
        self.anything_changed = true;
    }

    /// C++ `doFullUpdate`.
    pub fn do_full_update(&mut self) {
        self.clear_all_edging();
    }

    /// Build custom-edge vertex and index data for a map region.
    pub fn load_edgings_in_vertex_and_index_buffers<M: CustomEdgingHeightMap>(
        &mut self,
        map: &M,
        mut min_x: i32,
        mut max_x: i32,
        mut min_y: i32,
        mut max_y: i32,
    ) {
        if !self.initialized || !self.anything_changed {
            return;
        }

        self.vertices.clear();
        self.indices.clear();
        self.anything_changed = false;

        min_x = min_x.max(0);
        min_y = min_y.max(0);
        max_x = max_x.min(map.x_extent().saturating_sub(1));
        max_y = max_y.min(map.y_extent().saturating_sub(1));

        for row in min_y..(max_y - 1) {
            for column in min_x..(max_x - 1) {
                let blend_index = map.blend_tile_index(column, row);
                if blend_index == 0 {
                    continue;
                }
                let Some(blend) = map.blend_tile(blend_index) else {
                    continue;
                };
                if blend.custom_blend_edge_class < 0 {
                    continue;
                }

                let Some((u_offset, v_offset)) = Self::edge_uv_offset(blend, row, column) else {
                    continue;
                };
                let range = map.uv_for_blend(blend.custom_blend_edge_class);
                let u_offset = range.lo.x + range.width() * u_offset;
                let v_offset = range.lo.y + range.height() * v_offset;
                let alpha_uv =
                    map.alpha_uv_data(column - map.draw_origin_x(), row - map.draw_origin_y());

                if self.vertices.len() + 4 > MAX_EDGE_VERTEX
                    || self.indices.len() + 6 > MAX_EDGE_INDEX
                {
                    return;
                }

                let start_vertex = self.vertices.len() as u16;
                for j in 0..2 {
                    for i in 0..2 {
                        let x = column + i;
                        let y = row + j;
                        let ndx = if j == 0 { i as usize } else { (3 - i) as usize };
                        let diffuse = 0x8000_0000 | (map.static_diffuse(x, y) & 0x00ff_ffff);
                        self.vertices.push(CustomEdgingVertex {
                            x: x as f32 * MAP_XY_FACTOR,
                            y: y as f32 * MAP_XY_FACTOR,
                            z: map.height(x, y) as f32 * MAP_HEIGHT_SCALE,
                            diffuse,
                            u1: alpha_uv.u[ndx],
                            v1: alpha_uv.v[ndx],
                            u2: u_offset + i as f32 * 0.25 * range.width(),
                            v2: v_offset + (1 - j) as f32 * 0.25 * range.height(),
                            alpha: alpha_uv.alpha[ndx],
                        });
                    }
                }

                self.indices.extend_from_slice(&[
                    start_vertex,
                    start_vertex + 3,
                    start_vertex + 2,
                    start_vertex,
                    start_vertex + 1,
                    start_vertex + 3,
                ]);
            }
        }
    }

    fn edge_uv_offset(blend: CustomBlendTile, row: i32, column: i32) -> Option<(f32, f32)> {
        if blend.horiz {
            Some((
                if blend.inverted { 0.75 } else { 0.0 },
                0.25 * (1 + (row & 1)) as f32,
            ))
        } else if blend.vert {
            Some((
                0.25 * (1 + (column & 1)) as f32,
                if blend.inverted { 0.0 } else { 0.75 },
            ))
        } else if blend.right_diagonal {
            if blend.long_diagonal {
                Some((0.5, if blend.inverted { 0.5 } else { 0.25 }))
            } else {
                Some((0.0, if blend.inverted { 0.0 } else { 0.75 }))
            }
        } else if blend.left_diagonal {
            if blend.long_diagonal {
                Some((0.25, if blend.inverted { 0.5 } else { 0.25 }))
            } else {
                Some((0.75, if blend.inverted { 0.0 } else { 0.75 }))
            }
        } else {
            None
        }
    }

    /// C++ draw call equivalent: rebuild if dirty and return whether anything should draw.
    pub fn draw_edging<M: CustomEdgingHeightMap>(
        &mut self,
        map: &M,
        min_x: i32,
        max_x: i32,
        min_y: i32,
        max_y: i32,
    ) -> bool {
        self.load_edgings_in_vertex_and_index_buffers(map, min_x, max_x, min_y, max_y);
        !self.indices.is_empty()
    }

    /// C++ `drawEdging` render-pass sequence.
    pub fn draw_edging_plan<'a, M: CustomEdgingHeightMap>(
        &mut self,
        map: &M,
        min_x: i32,
        max_x: i32,
        min_y: i32,
        max_y: i32,
        textures: CustomEdgingTextures<'a>,
    ) -> CustomEdgingDrawPlan<'a> {
        self.load_edgings_in_vertex_and_index_buffers(map, min_x, max_x, min_y, max_y);
        if self.indices.is_empty() {
            return CustomEdgingDrawPlan::default();
        }

        let triangle_count = self.indices.len() / 3;
        let vertex_count = self.vertices.len();
        let common = |kind,
                      shader,
                      texture0,
                      texture1,
                      alpha_ref,
                      alpha_func,
                      alpha_blend,
                      src_blend,
                      dest_blend| {
            CustomEdgingRenderPass {
                kind,
                shader,
                texture0,
                texture1,
                alpha_ref,
                alpha_func,
                alpha_test: true,
                alpha_blend,
                src_blend,
                dest_blend,
                start_index: 0,
                triangle_count,
                min_vertex: 0,
                vertex_count,
            }
        };

        let mut passes = vec![
            common(
                CustomEdgingRenderPassKind::TerrainMask,
                CustomEdgingShader::DetailAlphaTest,
                textures.terrain,
                textures.edge,
                0x7b,
                CustomEdgingAlphaFunc::LessEqual,
                None,
                None,
                None,
            ),
            common(
                CustomEdgingRenderPassKind::CustomEdge,
                CustomEdgingShader::DetailAlphaTest,
                textures.edge,
                None,
                0x84,
                CustomEdgingAlphaFunc::GreaterEqual,
                None,
                None,
                None,
            ),
        ];

        if let Some(cloud) = textures.cloud {
            passes.push(common(
                CustomEdgingRenderPassKind::Cloud,
                CustomEdgingShader::DetailOpaque,
                Some(cloud),
                textures.edge,
                0x80,
                CustomEdgingAlphaFunc::NotEqual,
                Some(true),
                Some(CustomEdgingBlend::DestColor),
                Some(CustomEdgingBlend::Zero),
            ));
        }

        if let Some(noise) = textures.noise {
            passes.push(common(
                CustomEdgingRenderPassKind::Noise,
                CustomEdgingShader::DetailOpaque,
                Some(noise),
                textures.edge,
                0x80,
                CustomEdgingAlphaFunc::NotEqual,
                Some(true),
                Some(CustomEdgingBlend::DestColor),
                Some(CustomEdgingBlend::Zero),
            ));
        }

        CustomEdgingDrawPlan { passes }
    }

    /// Current vertices.
    pub fn vertices(&self) -> &[CustomEdgingVertex] {
        &self.vertices
    }

    /// Current indices.
    pub fn indices(&self) -> &[u16] {
        &self.indices
    }

    /// Whether geometry is dirty.
    pub fn anything_changed(&self) -> bool {
        self.anything_changed
    }
}

impl Drop for W3DCustomEdging {
    fn drop(&mut self) {
        self.free_edging_buffers();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Default)]
    struct TestMap {
        blend: CustomBlendTile,
    }

    impl CustomEdgingHeightMap for TestMap {
        fn x_extent(&self) -> i32 {
            4
        }

        fn y_extent(&self) -> i32 {
            4
        }

        fn height(&self, x: i32, y: i32) -> i32 {
            x + y * 10
        }

        fn static_diffuse(&self, x: i32, y: i32) -> u32 {
            0x0001_0203 + (x as u32) + ((y as u32) << 8)
        }

        fn blend_tile_index(&self, x: i32, y: i32) -> i32 {
            if x == 1 && y == 1 {
                1
            } else {
                0
            }
        }

        fn blend_tile(&self, index: i32) -> Option<CustomBlendTile> {
            (index == 1).then_some(self.blend)
        }

        fn uv_for_blend(&self, _custom_blend_edge_class: i32) -> Region2D {
            Region2D::new(0.1, 0.2, 0.9, 1.0)
        }

        fn alpha_uv_data(&self, _x: i32, _y: i32) -> AlphaUvData {
            AlphaUvData {
                u: [0.0, 0.25, 0.5, 0.75],
                v: [1.0, 0.75, 0.5, 0.25],
                alpha: [10, 20, 30, 40],
                flip_for_blend: false,
            }
        }
    }

    #[test]
    fn builds_one_custom_horizontal_edge_quad() {
        let map = TestMap {
            blend: CustomBlendTile {
                custom_blend_edge_class: 2,
                horiz: true,
                ..Default::default()
            },
        };
        let mut edging = W3DCustomEdging::new();

        assert!(edging.draw_edging(&map, 0, 4, 0, 4));

        assert_eq!(edging.vertices().len(), 4);
        assert_eq!(edging.indices(), &[0, 3, 2, 0, 1, 3]);
        assert_eq!(edging.vertices()[0].x, MAP_XY_FACTOR);
        assert_eq!(edging.vertices()[0].y, MAP_XY_FACTOR);
        assert_eq!(edging.vertices()[0].z, 11.0 * MAP_HEIGHT_SCALE);
        assert_eq!(edging.vertices()[0].diffuse & 0xff00_0000, 0x8000_0000);
        assert_eq!(edging.vertices()[0].u1, 0.0);
        assert_eq!(edging.vertices()[0].v1, 1.0);
        assert_eq!(edging.vertices()[0].u2, 0.1);
        assert_eq!(edging.vertices()[0].v2, 0.2 + 0.8 * 0.5 + 0.8 * 0.25);
    }

    #[test]
    fn skips_alpha_only_blends_and_unchanged_buffers() {
        let map = TestMap {
            blend: CustomBlendTile {
                custom_blend_edge_class: -1,
                horiz: true,
                ..Default::default()
            },
        };
        let mut edging = W3DCustomEdging::new();

        assert!(!edging.draw_edging(&map, 0, 4, 0, 4));
        assert!(edging.vertices().is_empty());
        assert!(!edging.anything_changed());

        edging.load_edgings_in_vertex_and_index_buffers(&map, 0, 4, 0, 4);
        assert!(edging.vertices().is_empty());
    }

    #[test]
    fn draw_plan_matches_cpp_pass_order_and_optional_modulation() {
        let map = TestMap {
            blend: CustomBlendTile {
                custom_blend_edge_class: 2,
                horiz: true,
                ..Default::default()
            },
        };
        let mut edging = W3DCustomEdging::new();

        let plan = edging.draw_edging_plan(
            &map,
            0,
            4,
            0,
            4,
            CustomEdgingTextures {
                terrain: Some("terrain"),
                edge: Some("edge"),
                cloud: Some("cloud"),
                noise: Some("noise"),
            },
        );

        assert_eq!(plan.passes.len(), 4);
        assert_eq!(plan.passes[0].kind, CustomEdgingRenderPassKind::TerrainMask);
        assert_eq!(plan.passes[0].shader, CustomEdgingShader::DetailAlphaTest);
        assert_eq!(plan.passes[0].texture0, Some("terrain"));
        assert_eq!(plan.passes[0].texture1, Some("edge"));
        assert_eq!(plan.passes[0].alpha_ref, 0x7b);
        assert_eq!(plan.passes[0].alpha_func, CustomEdgingAlphaFunc::LessEqual);

        assert_eq!(plan.passes[1].kind, CustomEdgingRenderPassKind::CustomEdge);
        assert_eq!(plan.passes[1].texture0, Some("edge"));
        assert_eq!(plan.passes[1].texture1, None);
        assert_eq!(plan.passes[1].alpha_ref, 0x84);
        assert_eq!(
            plan.passes[1].alpha_func,
            CustomEdgingAlphaFunc::GreaterEqual
        );

        for (pass, kind, texture) in [
            (&plan.passes[2], CustomEdgingRenderPassKind::Cloud, "cloud"),
            (&plan.passes[3], CustomEdgingRenderPassKind::Noise, "noise"),
        ] {
            assert_eq!(pass.kind, kind);
            assert_eq!(pass.shader, CustomEdgingShader::DetailOpaque);
            assert_eq!(pass.texture0, Some(texture));
            assert_eq!(pass.texture1, Some("edge"));
            assert_eq!(pass.alpha_ref, 0x80);
            assert_eq!(pass.alpha_func, CustomEdgingAlphaFunc::NotEqual);
            assert_eq!(pass.alpha_blend, Some(true));
            assert_eq!(pass.src_blend, Some(CustomEdgingBlend::DestColor));
            assert_eq!(pass.dest_blend, Some(CustomEdgingBlend::Zero));
            assert_eq!(pass.triangle_count, 2);
            assert_eq!(pass.vertex_count, 4);
        }
    }

    #[test]
    fn draw_plan_omits_cloud_and_noise_when_textures_are_absent() {
        let map = TestMap {
            blend: CustomBlendTile {
                custom_blend_edge_class: 2,
                horiz: true,
                ..Default::default()
            },
        };
        let mut edging = W3DCustomEdging::new();

        let plan = edging.draw_edging_plan(
            &map,
            0,
            4,
            0,
            4,
            CustomEdgingTextures {
                terrain: Some("terrain"),
                edge: Some("edge"),
                cloud: None,
                noise: None,
            },
        );

        assert_eq!(
            plan.passes.iter().map(|pass| pass.kind).collect::<Vec<_>>(),
            vec![
                CustomEdgingRenderPassKind::TerrainMask,
                CustomEdgingRenderPassKind::CustomEdge,
            ]
        );
    }
}
