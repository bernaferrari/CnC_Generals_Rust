//! Polygon-Level Transparency Sorting
//!
//! This module implements advanced polygon-level sorting for transparent objects.
//! Port of sortingrenderer.cpp lines 90-516.
//!
//! Key algorithms:
//! - Back-to-front polygon sorting (lines 105-154)
//! - Triangle depth calculation (lines 462-506)
//! - Material-based batching (lines 563-594)

use glam::{Mat4, Vec3, Vec4};
use std::collections::HashMap;
use std::sync::Arc;

/// Default sorting buffer sizes (sortingrenderer.cpp lines 60-61)
pub const DEFAULT_SORTING_POLY_COUNT: usize = 16384;
pub const DEFAULT_SORTING_VERTEX_COUNT: usize = 32768;

/// Sortable polygon with depth information
/// Matches C++ TempIndexStruct (sortingrenderer.cpp lines 76-81)
#[derive(Debug, Clone)]
pub struct SortablePolygon {
    /// Triangle vertex indices
    pub tri_indices: [u16; 3],
    /// Node/batch index this polygon belongs to
    pub node_idx: u16,
    /// Depth value for sorting (view-space Z)
    pub z_depth: f32,
}

impl SortablePolygon {
    /// Create new sortable polygon
    pub fn new(tri_indices: [u16; 3], node_idx: u16, z_depth: f32) -> Self {
        Self {
            tri_indices,
            node_idx,
            z_depth,
        }
    }

    /// Compare by depth (back-to-front order for alpha blending)
    /// Larger Z values come first (furthest from camera)
    pub fn cmp_back_to_front(&self, other: &Self) -> std::cmp::Ordering {
        // Reverse comparison for back-to-front
        other
            .z_depth
            .partial_cmp(&self.z_depth)
            .unwrap_or(std::cmp::Ordering::Equal)
    }
}

/// Vertex format matching C++ VertexFormatXYZNDUV2
#[repr(C)]
#[derive(Debug, Copy, Clone, bytemuck::Pod, bytemuck::Zeroable)]
pub struct VertexXYZNDUV2 {
    pub position: [f32; 3],
    pub normal: [f32; 3],
    pub diffuse: u32,
    pub uv0: [f32; 2],
    pub uv1: [f32; 2],
}

/// Sorting node representing a batch of geometry
/// Matches C++ SortingNodeStruct (sortingrenderer.cpp lines 158-172)
#[derive(Clone)]
pub struct SortingNode {
    /// Vertex data for this batch
    pub vertices: Vec<VertexXYZNDUV2>,
    /// Index data for this batch
    pub indices: Vec<u16>,
    /// Starting polygon index
    pub start_index: u16,
    /// Number of polygons
    pub polygon_count: u16,
    /// Minimum vertex index (for base vertex)
    pub min_vertex_index: u16,
    /// Number of vertices
    pub vertex_count: u16,
    /// Transformed bounding sphere center
    pub transformed_center: Vec3,
    /// Material ID for batching
    pub material_id: u64,
    /// Shader ID for state sorting
    pub shader_id: u64,
}

impl SortingNode {
    /// Create new sorting node
    pub fn new(material_id: u64, shader_id: u64) -> Self {
        Self {
            vertices: Vec::new(),
            indices: Vec::new(),
            start_index: 0,
            polygon_count: 0,
            min_vertex_index: 0,
            vertex_count: 0,
            transformed_center: Vec3::ZERO,
            material_id,
            shader_id,
        }
    }

    /// Calculate depth for triangle
    /// Port of sortingrenderer.cpp lines 462-506
    pub fn calculate_triangle_depth(
        &self,
        tri_idx: usize,
        transform: &Mat4,
    ) -> f32 {
        let idx0 = self.indices[tri_idx * 3] as usize;
        let idx1 = self.indices[tri_idx * 3 + 1] as usize;
        let idx2 = self.indices[tri_idx * 3 + 2] as usize;

        let v0 = Vec3::from(self.vertices[idx0].position);
        let v1 = Vec3::from(self.vertices[idx1].position);
        let v2 = Vec3::from(self.vertices[idx2].position);

        // Common case optimization: identity Z transform (lines 462-483)
        if transform.row(2) == Vec4::new(0.0, 0.0, 1.0, 0.0) {
            // Just average the Z coordinates
            return (v0.z + v1.z + v2.z) / 3.0;
        }

        // General case: transform and average (lines 485-506)
        let m = transform.to_cols_array_2d();
        let avg_x = (v0.x + v1.x + v2.x) / 3.0;
        let avg_y = (v0.y + v1.y + v2.y) / 3.0;
        let avg_z = (v0.z + v1.z + v2.z) / 3.0;

        // Apply matrix transform to get view-space Z
        m[0][2] * avg_x + m[1][2] * avg_y + m[2][2] * avg_z + m[3][2]
    }
}

/// Advanced polygon sorting renderer
/// Port of C++ SortingRendererClass (sortingrenderer.cpp)
pub struct PolygonSortingRenderer {
    /// List of sorting nodes (batches)
    nodes: Vec<SortingNode>,
    /// List of sorted polygons
    sorted_polygons: Vec<SortablePolygon>,
    /// Camera position for depth calculations
    camera_position: Vec3,
    /// View-projection matrix
    view_proj_matrix: Mat4,
    /// Enable triangle drawing
    enable_draw: bool,
    /// Statistics
    total_vertices: usize,
    total_polygons: usize,
}

impl PolygonSortingRenderer {
    /// Create new polygon sorting renderer
    pub fn new() -> Self {
        Self {
            nodes: Vec::with_capacity(256),
            sorted_polygons: Vec::with_capacity(DEFAULT_SORTING_POLY_COUNT),
            camera_position: Vec3::ZERO,
            view_proj_matrix: Mat4::IDENTITY,
            enable_draw: true,
            total_vertices: 0,
            total_polygons: 0,
        }
    }

    /// Set camera parameters
    pub fn set_camera(&mut self, position: Vec3, view_proj: Mat4) {
        self.camera_position = position;
        self.view_proj_matrix = view_proj;
    }

    /// Add triangles to sorting system
    /// Port of sortingrenderer.cpp Insert_Triangles (lines 217-297)
    pub fn insert_triangles(
        &mut self,
        vertices: Vec<VertexXYZNDUV2>,
        indices: Vec<u16>,
        start_index: u16,
        polygon_count: u16,
        min_vertex_index: u16,
        vertex_count: u16,
        bounding_center: Vec3,
        material_id: u64,
        shader_id: u64,
        transform: Mat4,
    ) {
        // Transform bounding sphere center to view space (lines 254-261)
        let view_transform = self.view_proj_matrix;
        let transformed_vec4 = view_transform * Vec4::from((bounding_center, 1.0));
        let transformed_center = transformed_vec4.truncate();

        // Create sorting node
        let mut node = SortingNode::new(material_id, shader_id);
        node.vertices = vertices;
        node.indices = indices;
        node.start_index = start_index;
        node.polygon_count = polygon_count;
        node.min_vertex_index = min_vertex_index;
        node.vertex_count = vertex_count;
        node.transformed_center = transformed_center;

        // Insert node sorted by depth (lines 266-277)
        let insert_pos = self
            .nodes
            .iter()
            .position(|n| transformed_center.z > n.transformed_center.z)
            .unwrap_or(self.nodes.len());

        self.nodes.insert(insert_pos, node);

        self.total_vertices += vertex_count as usize;
        self.total_polygons += polygon_count as usize;
    }

    /// Sort all polygons by depth
    /// Port of sortingrenderer.cpp Sort (lines 105-154) and Flush_Sorting_Pool (lines 412-608)
    pub fn sort_polygons(&mut self, transform: &Mat4) {
        self.sorted_polygons.clear();
        self.sorted_polygons.reserve(self.total_polygons);

        // Build sortable polygon array (lines 434-513)
        for (node_idx, node) in self.nodes.iter().enumerate() {
            for tri_idx in 0..node.polygon_count as usize {
                let base_idx = (node.start_index as usize + tri_idx * 3) as usize;

                let idx0 = node.indices[base_idx];
                let idx1 = node.indices[base_idx + 1];
                let idx2 = node.indices[base_idx + 2];

                // Calculate triangle center depth
                let z_depth = node.calculate_triangle_depth(tri_idx, transform);

                let poly = SortablePolygon::new(
                    [idx0, idx1, idx2],
                    node_idx as u16,
                    z_depth,
                );

                self.sorted_polygons.push(poly);
            }
        }

        // Sort polygons back-to-front (line 516)
        self.sorted_polygons
            .sort_by(|a, b| a.cmp_back_to_front(b));
    }

    /// Get sorted rendering batches
    /// Groups consecutive polygons from same node for efficient rendering
    /// Port of sortingrenderer.cpp Flush (lines 563-594)
    pub fn get_render_batches(&self) -> Vec<RenderBatch> {
        let mut batches = Vec::new();

        if self.sorted_polygons.is_empty() {
            return batches;
        }

        let mut batch_start = 0;
        let mut current_node_idx = self.sorted_polygons[0].node_idx;
        let mut batch_count = 0;

        for (i, poly) in self.sorted_polygons.iter().enumerate() {
            if poly.node_idx != current_node_idx {
                // Flush current batch
                let node = &self.nodes[current_node_idx as usize];
                batches.push(RenderBatch {
                    node_idx: current_node_idx,
                    start_poly_idx: batch_start,
                    poly_count: batch_count,
                    material_id: node.material_id,
                    shader_id: node.shader_id,
                });

                // Start new batch
                batch_start = i;
                batch_count = 1;
                current_node_idx = poly.node_idx;
            } else {
                batch_count += 1;
            }
        }

        // Flush final batch
        if batch_count > 0 {
            let node = &self.nodes[current_node_idx as usize];
            batches.push(RenderBatch {
                node_idx: current_node_idx,
                start_poly_idx: batch_start,
                poly_count: batch_count,
                material_id: node.material_id,
                shader_id: node.shader_id,
            });
        }

        batches
    }

    /// Clear all sorting data
    /// Port of sortingrenderer.cpp Flush (lines 612-652)
    pub fn flush(&mut self) {
        self.nodes.clear();
        self.sorted_polygons.clear();
        self.total_vertices = 0;
        self.total_polygons = 0;
    }

    /// Get node by index
    pub fn get_node(&self, node_idx: u16) -> Option<&SortingNode> {
        self.nodes.get(node_idx as usize)
    }

    /// Get sorted polygon by index
    pub fn get_sorted_polygon(&self, poly_idx: usize) -> Option<&SortablePolygon> {
        self.sorted_polygons.get(poly_idx)
    }

    /// Enable/disable triangle drawing
    pub fn set_draw_enabled(&mut self, enabled: bool) {
        self.enable_draw = enabled;
    }

    /// Check if drawing is enabled
    pub fn is_draw_enabled(&self) -> bool {
        self.enable_draw
    }

    /// Get statistics
    pub fn stats(&self) -> PolygonSortingStats {
        PolygonSortingStats {
            node_count: self.nodes.len(),
            polygon_count: self.total_polygons,
            vertex_count: self.total_vertices,
            sorted_polygon_count: self.sorted_polygons.len(),
        }
    }
}

impl Default for PolygonSortingRenderer {
    fn default() -> Self {
        Self::new()
    }
}

/// Render batch from sorted polygons
#[derive(Debug, Clone)]
pub struct RenderBatch {
    /// Index of the source node
    pub node_idx: u16,
    /// Starting index in sorted polygon array
    pub start_poly_idx: usize,
    /// Number of polygons in this batch
    pub poly_count: usize,
    /// Material ID for state sorting
    pub material_id: u64,
    /// Shader ID for state sorting
    pub shader_id: u64,
}

/// Statistics for polygon sorting
#[derive(Debug, Clone, Copy, Default)]
pub struct PolygonSortingStats {
    pub node_count: usize,
    pub polygon_count: usize,
    pub vertex_count: usize,
    pub sorted_polygon_count: usize,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_sortable_polygon_creation() {
        let poly = SortablePolygon::new([0, 1, 2], 0, 10.0);
        assert_eq!(poly.tri_indices, [0, 1, 2]);
        assert_eq!(poly.node_idx, 0);
        assert_eq!(poly.z_depth, 10.0);
    }

    #[test]
    fn test_polygon_sorting_back_to_front() {
        let poly1 = SortablePolygon::new([0, 1, 2], 0, 5.0);
        let poly2 = SortablePolygon::new([3, 4, 5], 1, 10.0);
        let poly3 = SortablePolygon::new([6, 7, 8], 2, 7.5);

        let mut polys = vec![poly1.clone(), poly2.clone(), poly3.clone()];
        polys.sort_by(|a, b| a.cmp_back_to_front(b));

        // Should be sorted furthest first: poly2 (10.0), poly3 (7.5), poly1 (5.0)
        assert_eq!(polys[0].z_depth, 10.0);
        assert_eq!(polys[1].z_depth, 7.5);
        assert_eq!(polys[2].z_depth, 5.0);
    }

    #[test]
    fn test_sorting_node_creation() {
        let node = SortingNode::new(100, 200);
        assert_eq!(node.material_id, 100);
        assert_eq!(node.shader_id, 200);
        assert_eq!(node.vertices.len(), 0);
        assert_eq!(node.indices.len(), 0);
    }

    #[test]
    fn test_polygon_sorting_renderer_creation() {
        let renderer = PolygonSortingRenderer::new();
        assert_eq!(renderer.nodes.len(), 0);
        assert_eq!(renderer.sorted_polygons.len(), 0);
        assert!(renderer.is_draw_enabled());
    }

    #[test]
    fn test_polygon_sorting_renderer_stats() {
        let renderer = PolygonSortingRenderer::new();
        let stats = renderer.stats();
        assert_eq!(stats.node_count, 0);
        assert_eq!(stats.polygon_count, 0);
        assert_eq!(stats.vertex_count, 0);
    }

    #[test]
    fn test_render_batch_creation() {
        let batch = RenderBatch {
            node_idx: 5,
            start_poly_idx: 10,
            poly_count: 20,
            material_id: 100,
            shader_id: 200,
        };
        assert_eq!(batch.node_idx, 5);
        assert_eq!(batch.poly_count, 20);
    }

    #[test]
    fn test_triangle_depth_identity_transform() {
        let mut node = SortingNode::new(0, 0);

        // Create simple triangle
        node.vertices = vec![
            VertexXYZNDUV2 {
                position: [0.0, 0.0, 5.0],
                normal: [0.0, 0.0, 1.0],
                diffuse: 0xFFFFFFFF,
                uv0: [0.0, 0.0],
                uv1: [0.0, 0.0],
            },
            VertexXYZNDUV2 {
                position: [1.0, 0.0, 6.0],
                normal: [0.0, 0.0, 1.0],
                diffuse: 0xFFFFFFFF,
                uv0: [1.0, 0.0],
                uv1: [0.0, 0.0],
            },
            VertexXYZNDUV2 {
                position: [0.0, 1.0, 7.0],
                normal: [0.0, 0.0, 1.0],
                diffuse: 0xFFFFFFFF,
                uv0: [0.0, 1.0],
                uv1: [0.0, 0.0],
            },
        ];
        node.indices = vec![0, 1, 2];

        let identity = Mat4::IDENTITY;
        let depth = node.calculate_triangle_depth(0, &identity);

        // Average of 5.0, 6.0, 7.0 = 6.0
        assert!((depth - 6.0).abs() < 0.001);
    }

    #[test]
    fn test_flush_clears_data() {
        let mut renderer = PolygonSortingRenderer::new();

        // Add some dummy data
        renderer.total_vertices = 100;
        renderer.total_polygons = 50;
        renderer.nodes.push(SortingNode::new(1, 1));

        renderer.flush();

        assert_eq!(renderer.nodes.len(), 0);
        assert_eq!(renderer.sorted_polygons.len(), 0);
        assert_eq!(renderer.total_vertices, 0);
        assert_eq!(renderer.total_polygons, 0);
    }
}
