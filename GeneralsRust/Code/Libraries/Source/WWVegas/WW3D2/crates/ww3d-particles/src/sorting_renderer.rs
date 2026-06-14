//! Sorting Renderer Implementation
//!
//! This module implements the SortingRendererClass for rendering transparent
//! geometry in correct depth order, essential for particle systems.

use glam::{Mat4, Vec3, Vec4};
use std::sync::Arc;
use wgpu::{Device, Queue};

/// Triangle indices structure for sorting
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TriangleIndices {
    pub i: u16,
    pub j: u16,
    pub k: u16,
}

/// Temporary index structure for depth sorting
#[derive(Debug, Clone)]
pub struct TempIndexStruct {
    pub tri: TriangleIndices,
    pub node_id: usize,
    pub z_depth: f32,
}

impl TempIndexStruct {
    pub fn new(tri: TriangleIndices, node_id: usize, z_depth: f32) -> Self {
        Self {
            tri,
            node_id,
            z_depth,
        }
    }
}

/// Sorting node representing a batch of geometry to be sorted
#[derive(Debug, Clone)]
pub struct SortingNode {
    pub bounding_sphere: BoundingSphere,
    pub transformed_center: Vec3,
    pub start_index: u16,
    pub polygon_count: u16,
    pub min_vertex_index: u16,
    pub vertex_count: u16,
    pub layer_count: u16,
    pub is_volume_particle: bool,
    transformed_vertices: Vec<Vec3>,
    indices: Vec<TriangleIndices>,
}

impl SortingNode {
    pub fn new(
        bounding_sphere: BoundingSphere,
        start_index: u16,
        polygon_count: u16,
        min_vertex_index: u16,
        vertex_count: u16,
    ) -> Self {
        Self {
            bounding_sphere,
            transformed_center: Vec3::ZERO,
            start_index,
            polygon_count,
            min_vertex_index,
            vertex_count,
            layer_count: 1,
            is_volume_particle: false,
            transformed_vertices: Vec::new(),
            indices: Vec::new(),
        }
    }

    pub fn with_geometry(
        bounding_sphere: BoundingSphere,
        start_index: u16,
        polygon_count: u16,
        min_vertex_index: u16,
        vertex_count: u16,
        vertices: &[Vec3],
        indices: &[TriangleIndices],
        world_view_matrix: &Mat4,
    ) -> Self {
        let mut node = Self::new(
            bounding_sphere,
            start_index,
            polygon_count,
            min_vertex_index,
            vertex_count,
        );
        node.capture_geometry(vertices, indices, world_view_matrix);
        node
    }

    pub fn volume_particle(
        bounding_sphere: BoundingSphere,
        start_index: u16,
        polygon_count: u16,
        min_vertex_index: u16,
        vertex_count: u16,
        layer_count: u16,
    ) -> Self {
        Self {
            bounding_sphere,
            transformed_center: Vec3::ZERO,
            start_index,
            polygon_count: polygon_count * layer_count,
            min_vertex_index,
            vertex_count: vertex_count * layer_count,
            layer_count,
            is_volume_particle: true,
            transformed_vertices: Vec::new(),
            indices: Vec::new(),
        }
    }

    /// Update the transformed center for sorting
    pub fn update_transformed_center(&mut self, world_view_matrix: &Mat4) {
        let center_4d = Vec4::new(
            self.bounding_sphere.center.x,
            self.bounding_sphere.center.y,
            self.bounding_sphere.center.z,
            1.0,
        );
        let transformed = *world_view_matrix * center_4d;
        self.transformed_center = Vec3::new(transformed.x, transformed.y, transformed.z);
    }

    fn capture_geometry(
        &mut self,
        vertices: &[Vec3],
        indices: &[TriangleIndices],
        world_view_matrix: &Mat4,
    ) {
        self.transformed_vertices.clear();
        self.indices.clear();

        let first = self.min_vertex_index as usize;
        let last = first.saturating_add(self.vertex_count as usize);
        if last <= vertices.len() {
            self.transformed_vertices
                .extend(vertices[first..last].iter().map(|vertex| {
                    let transformed = *world_view_matrix * vertex.extend(1.0);
                    Vec3::new(transformed.x, transformed.y, transformed.z)
                }));
        }

        let start = self.start_index as usize / 3;
        let count = self.polygon_count as usize;
        if start.saturating_add(count) <= indices.len() {
            self.indices
                .extend_from_slice(&indices[start..start + count]);
        }
    }
}

/// Bounding sphere for frustum culling and sorting
#[derive(Debug, Clone, Copy)]
pub struct BoundingSphere {
    pub center: Vec3,
    pub radius: f32,
}

impl BoundingSphere {
    pub fn new(center: Vec3, radius: f32) -> Self {
        Self { center, radius }
    }
}

/// CPU-side draw command emitted after sorting.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SortingDrawCommand {
    pub node_id: usize,
    pub start_index: usize,
    pub triangle_count: usize,
    pub min_vertex_index: u16,
    pub vertex_count: u16,
    pub indices: Vec<TriangleIndices>,
}

/// Sorting renderer for transparent geometry
#[allow(dead_code)] // C++ parity
#[derive(Debug)]
pub struct SortingRenderer {
    // Configuration
    pub enable_triangle_draw: bool,
    pub min_vertex_buffer_size: u32,

    // Internal state
    sorted_nodes: Vec<SortingNode>,
    temp_indices: Vec<TempIndexStruct>,
    overlapping_nodes: Vec<SortingNode>,
    overlapping_indices: Vec<TempIndexStruct>,
    draw_commands: Vec<SortingDrawCommand>,

    // GPU resources
    device: Option<Arc<Device>>,
    queue: Option<Arc<Queue>>,
}

impl SortingRenderer {
    /// Create a new sorting renderer
    pub fn new(device: Arc<Device>, queue: Arc<Queue>) -> Self {
        Self {
            enable_triangle_draw: true,
            min_vertex_buffer_size: 32768,
            sorted_nodes: Vec::new(),
            temp_indices: Vec::new(),
            overlapping_nodes: Vec::new(),
            overlapping_indices: Vec::new(),
            draw_commands: Vec::new(),
            device: Some(device),
            queue: Some(queue),
        }
    }

    /// Create a CPU-only sorting renderer for deterministic tests and non-GPU staging.
    pub fn new_cpu_only() -> Self {
        Self {
            enable_triangle_draw: true,
            min_vertex_buffer_size: 32768,
            sorted_nodes: Vec::new(),
            temp_indices: Vec::new(),
            overlapping_nodes: Vec::new(),
            overlapping_indices: Vec::new(),
            draw_commands: Vec::new(),
            device: None,
            queue: None,
        }
    }

    /// Set minimum vertex buffer size
    pub fn set_min_vertex_buffer_size(&mut self, size: u32) {
        self.min_vertex_buffer_size = size;
    }

    /// Enable or disable triangle drawing
    pub fn enable_triangle_draw(&mut self, enable: bool) {
        self.enable_triangle_draw = enable;
    }

    /// Check if triangle drawing is enabled
    pub fn is_triangle_draw_enabled(&self) -> bool {
        self.enable_triangle_draw
    }

    /// Insert triangles into the sorting system
    pub fn insert_triangles(
        &mut self,
        bounding_sphere: BoundingSphere,
        start_index: u16,
        polygon_count: u16,
        min_vertex_index: u16,
        vertex_count: u16,
        world_view_matrix: &Mat4,
    ) {
        let mut node = SortingNode::new(
            bounding_sphere,
            start_index,
            polygon_count,
            min_vertex_index,
            vertex_count,
        );

        node.update_transformed_center(world_view_matrix);

        // Insert into sorted list based on Z depth
        self.insert_sorted_node(node);
    }

    /// Insert triangles with explicit vertex/index data for depth sorting.
    pub fn insert_indexed_triangles(
        &mut self,
        bounding_sphere: BoundingSphere,
        start_index: u16,
        polygon_count: u16,
        min_vertex_index: u16,
        vertex_count: u16,
        vertices: &[Vec3],
        indices: &[TriangleIndices],
        world_view_matrix: &Mat4,
    ) {
        let mut node = SortingNode::with_geometry(
            bounding_sphere,
            start_index,
            polygon_count,
            min_vertex_index,
            vertex_count,
            vertices,
            indices,
            world_view_matrix,
        );
        node.update_transformed_center(world_view_matrix);
        self.insert_sorted_node(node);
    }

    /// Insert volume particle triangles
    pub fn insert_volume_particle(
        &mut self,
        bounding_sphere: BoundingSphere,
        start_index: u16,
        polygon_count: u16,
        min_vertex_index: u16,
        vertex_count: u16,
        layer_count: u16,
        world_view_matrix: &Mat4,
    ) {
        let mut node = SortingNode::volume_particle(
            bounding_sphere,
            start_index,
            polygon_count,
            min_vertex_index,
            vertex_count,
            layer_count,
        );

        node.update_transformed_center(world_view_matrix);

        // Insert into sorted list based on Z depth
        self.insert_sorted_node(node);
    }

    /// Insert a node into the sorted list
    fn insert_sorted_node(&mut self, node: SortingNode) {
        // Find the insertion point based on Z depth (front to back)
        let insert_pos = self
            .sorted_nodes
            .iter()
            .position(|existing| node.transformed_center.z > existing.transformed_center.z)
            .unwrap_or(self.sorted_nodes.len());

        self.sorted_nodes.insert(insert_pos, node);
    }

    /// Flush all sorted geometry
    pub fn flush(&mut self) {
        if self.sorted_nodes.is_empty() {
            self.draw_commands.clear();
            return;
        }

        // Process nodes and collect overlapping ones
        self.process_sorted_nodes();

        // Render overlapping nodes in sorted order
        self.render_overlapping_nodes();

        // Clear for next frame
        self.sorted_nodes.clear();
        self.overlapping_nodes.clear();
        self.overlapping_indices.clear();
    }

    /// Flush sorted geometry and return the CPU draw command list.
    pub fn flush_to_draw_commands(&mut self) -> Vec<SortingDrawCommand> {
        self.flush();
        self.draw_commands.clone()
    }

    /// Process sorted nodes and collect overlapping geometry
    fn process_sorted_nodes(&mut self) {
        for node in &self.sorted_nodes {
            self.overlapping_nodes.push(node.clone());
        }

        // Generate sorted triangle indices
        self.generate_sorted_indices();
    }

    /// Generate sorted triangle indices for overlapping nodes
    fn generate_sorted_indices(&mut self) {
        self.temp_indices.clear();
        self.draw_commands.clear();

        let mut vertex_offset = 0u16;

        for (node_id, node) in self.overlapping_nodes.iter().enumerate() {
            // For each triangle in this node, calculate its center Z and add to temp indices
            for i in 0..node.polygon_count {
                let tri_indices = node
                    .indices
                    .get(i as usize)
                    .copied()
                    .and_then(|tri| {
                        localize_triangle_indices(tri, node.min_vertex_index, vertex_offset)
                    })
                    .unwrap_or(TriangleIndices {
                        i: vertex_offset + (i * 3),
                        j: vertex_offset + (i * 3) + 1,
                        k: vertex_offset + (i * 3) + 2,
                    });
                let triangle_z = self.calculate_triangle_z(node, tri_indices, vertex_offset);

                self.temp_indices
                    .push(TempIndexStruct::new(tri_indices, node_id, triangle_z));
            }

            vertex_offset = vertex_offset.saturating_add(node.vertex_count);
        }

        // Sort triangles by depth (back to front for transparency)
        self.temp_indices.sort_by(|a, b| {
            b.z_depth
                .partial_cmp(&a.z_depth)
                .unwrap_or(std::cmp::Ordering::Equal)
        });
    }

    /// Calculate the Z depth of a triangle (simplified)
    fn calculate_triangle_z(
        &self,
        node: &SortingNode,
        tri_indices: TriangleIndices,
        vertex_offset: u16,
    ) -> f32 {
        let base = vertex_offset as usize;
        let indices = [
            tri_indices.i as usize,
            tri_indices.j as usize,
            tri_indices.k as usize,
        ];
        let mut z_sum = 0.0;

        for index in indices {
            let Some(local_index) = index.checked_sub(base) else {
                return node.transformed_center.z;
            };
            let Some(vertex) = node.transformed_vertices.get(local_index) else {
                return node.transformed_center.z;
            };
            z_sum += vertex.z;
        }

        z_sum / 3.0
    }

    /// Render overlapping nodes in sorted order
    fn render_overlapping_nodes(&mut self) {
        if self.temp_indices.is_empty() {
            return;
        }

        let mut start_triangle = 0;
        while start_triangle < self.temp_indices.len() {
            let node_id = self.temp_indices[start_triangle].node_id;
            let mut end_triangle = start_triangle + 1;
            while end_triangle < self.temp_indices.len()
                && self.temp_indices[end_triangle].node_id == node_id
            {
                end_triangle += 1;
            }

            self.render_node_triangles(node_id, start_triangle, end_triangle - start_triangle);
            start_triangle = end_triangle;
        }
    }

    /// Render triangles from a specific node
    fn render_node_triangles(
        &mut self,
        node_id: usize,
        start_triangle: usize,
        triangle_count: usize,
    ) {
        if node_id >= self.overlapping_nodes.len() || !self.enable_triangle_draw {
            return;
        }

        let node = &self.overlapping_nodes[node_id];
        let end_triangle = start_triangle.saturating_add(triangle_count);
        let indices = self.temp_indices[start_triangle..end_triangle]
            .iter()
            .map(|temp| temp.tri)
            .collect();

        self.draw_commands.push(SortingDrawCommand {
            node_id,
            start_index: start_triangle * 3,
            triangle_count,
            min_vertex_index: node.min_vertex_index,
            vertex_count: node.vertex_count,
            indices,
        });
    }

    /// Deinitialize the sorting renderer
    pub fn deinit(&mut self) {
        self.sorted_nodes.clear();
        self.temp_indices.clear();
        self.overlapping_nodes.clear();
        self.overlapping_indices.clear();
        self.draw_commands.clear();
    }

    /// Get the number of active sorted nodes
    pub fn get_sorted_node_count(&self) -> usize {
        self.sorted_nodes.len()
    }

    /// Get the total number of polygons being sorted
    pub fn get_total_polygon_count(&self) -> usize {
        self.sorted_nodes
            .iter()
            .map(|node| node.polygon_count as usize)
            .sum()
    }

    /// Get the total number of vertices being sorted
    pub fn get_total_vertex_count(&self) -> usize {
        self.sorted_nodes
            .iter()
            .map(|node| node.vertex_count as usize)
            .sum()
    }

    /// Check if sorting is enabled (always true for now)
    pub fn is_sorting_enabled() -> bool {
        true
    }

    /// Get draw commands emitted by the last flush.
    pub fn draw_commands(&self) -> &[SortingDrawCommand] {
        &self.draw_commands
    }
}

fn localize_triangle_indices(
    tri: TriangleIndices,
    min_vertex_index: u16,
    vertex_offset: u16,
) -> Option<TriangleIndices> {
    Some(TriangleIndices {
        i: tri
            .i
            .checked_sub(min_vertex_index)?
            .saturating_add(vertex_offset),
        j: tri
            .j
            .checked_sub(min_vertex_index)?
            .saturating_add(vertex_offset),
        k: tri
            .k
            .checked_sub(min_vertex_index)?
            .saturating_add(vertex_offset),
    })
}

/// Simple insertion sort for small arrays (used in original C++ code)
pub fn insertion_sort(indices: &mut [TempIndexStruct]) {
    for i in 1..indices.len() {
        let mut j = i;
        while j > 0 && indices[j - 1].z_depth < indices[j].z_depth {
            indices.swap(j - 1, j);
            j -= 1;
        }
    }
}

/// Quick sort with median-of-three partitioning (used in original C++ code)
pub fn quick_sort(indices: &mut [TempIndexStruct]) {
    if indices.len() <= 16 {
        insertion_sort(indices);
        return;
    }

    // Median-of-three partitioning
    let mid = indices.len() / 2;
    let last = indices.len() - 1;

    // Sort the first, middle, and last elements
    if indices[0].z_depth < indices[mid].z_depth {
        indices.swap(0, mid);
    }
    if indices[mid].z_depth < indices[last].z_depth {
        indices.swap(mid, last);
    }
    if indices[0].z_depth < indices[mid].z_depth {
        indices.swap(0, mid);
    }

    let pivot = indices[mid].z_depth;
    let mut i = 0;
    let mut j = last;

    loop {
        while indices[i].z_depth > pivot {
            i += 1;
        }
        while indices[j].z_depth < pivot {
            j -= 1;
        }
        if i >= j {
            break;
        }
        indices.swap(i, j);
        i += 1;
        j -= 1;
    }

    // Recursively sort partitions
    if j > 0 {
        quick_sort(&mut indices[0..j + 1]);
    }
    if j + 1 < indices.len() {
        quick_sort(&mut indices[j + 1..]);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn flush_outputs_triangles_sorted_by_transformed_depth() {
        let mut renderer = SortingRenderer::new_cpu_only();
        let vertices = vec![
            Vec3::new(0.0, 0.0, 1.0),
            Vec3::new(1.0, 0.0, 1.0),
            Vec3::new(0.0, 1.0, 1.0),
            Vec3::new(0.0, 0.0, 3.0),
            Vec3::new(1.0, 0.0, 3.0),
            Vec3::new(0.0, 1.0, 3.0),
        ];
        let indices = vec![
            TriangleIndices { i: 0, j: 1, k: 2 },
            TriangleIndices { i: 3, j: 4, k: 5 },
        ];

        let commands = renderer.flush_to_draw_commands();
        assert!(commands.is_empty());

        renderer.insert_indexed_triangles(
            BoundingSphere::new(Vec3::ZERO, 1.0),
            0,
            2,
            0,
            6,
            &vertices,
            &indices,
            &Mat4::IDENTITY,
        );

        let commands = renderer.flush_to_draw_commands();
        assert_eq!(commands.len(), 1);
        assert_eq!(commands[0].triangle_count, 2);
        assert_eq!(commands[0].indices[0], TriangleIndices { i: 3, j: 4, k: 5 });
        assert_eq!(commands[0].indices[1], TriangleIndices { i: 0, j: 1, k: 2 });
    }

    #[test]
    fn flush_groups_sorted_triangles_by_source_node() {
        let mut renderer = SortingRenderer::new_cpu_only();
        let near_vertices = vec![
            Vec3::new(0.0, 0.0, 1.0),
            Vec3::new(1.0, 0.0, 1.0),
            Vec3::new(0.0, 1.0, 1.0),
        ];
        let far_vertices = vec![
            Vec3::new(0.0, 0.0, 5.0),
            Vec3::new(1.0, 0.0, 5.0),
            Vec3::new(0.0, 1.0, 5.0),
        ];
        let indices = vec![TriangleIndices { i: 0, j: 1, k: 2 }];

        renderer.insert_indexed_triangles(
            BoundingSphere::new(Vec3::new(0.0, 0.0, 1.0), 1.0),
            0,
            1,
            0,
            3,
            &near_vertices,
            &indices,
            &Mat4::IDENTITY,
        );
        renderer.insert_indexed_triangles(
            BoundingSphere::new(Vec3::new(0.0, 0.0, 5.0), 1.0),
            0,
            1,
            0,
            3,
            &far_vertices,
            &indices,
            &Mat4::IDENTITY,
        );

        let commands = renderer.flush_to_draw_commands();
        assert_eq!(commands.len(), 2);
        assert_eq!(commands[0].triangle_count, 1);
        assert_eq!(commands[1].triangle_count, 1);
        assert_ne!(commands[0].node_id, commands[1].node_id);
    }

    #[test]
    fn disabled_triangle_draw_suppresses_draw_commands() {
        let mut renderer = SortingRenderer::new_cpu_only();
        renderer.enable_triangle_draw(false);
        renderer.insert_triangles(
            BoundingSphere::new(Vec3::ZERO, 1.0),
            0,
            1,
            0,
            3,
            &Mat4::IDENTITY,
        );

        let commands = renderer.flush_to_draw_commands();
        assert!(commands.is_empty());
    }
}
